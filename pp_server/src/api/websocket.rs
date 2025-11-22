//! WebSocket handler for real-time table updates.
//!
//! This module implements a bidirectional WebSocket connection for live poker game communication.
//! Once connected, clients receive automatic game state updates every ~1 second and can send
//! commands to interact with the table in real-time.
//!
//! # Connection Flow
//!
//! 1. Client connects via `GET /ws/:table_id?token=<jwt_token>`
//! 2. Server validates JWT and establishes WebSocket
//! 3. Server spawns two tasks:
//!    - Send task: Pushes game view updates every 1 second
//!    - Receive task: Processes incoming client commands
//! 4. On disconnect, both tasks are cleaned up
//!
//! # Client Messages
//!
//! Clients can send JSON messages to:
//! - Join the table with a buy-in
//! - Leave the table
//! - Take actions (fold, check, call, raise, all-in)
//! - Start/stop spectating
//!
//! # Server Messages
//!
//! Server sends two types of messages:
//! - **Game View Updates**: Complete game state (automatic every ~1s)
//! - **Command Responses**: Success or error responses to client commands
//!
//! # Example
//!
//! ```javascript
//! const ws = new WebSocket('ws://localhost:3000/ws/1?token=eyJhbGc...');
//!
//! ws.onmessage = (event) => {
//!   const data = JSON.parse(event.data);
//!   if (data.blinds) {
//!     // Game view update
//!     updateGameUI(data);
//!   } else {
//!     // Command response
//!     handleResponse(data);
//!   }
//! };
//!
//! // Take action
//! ws.send(JSON.stringify({
//!   type: "action",
//!   action: { type: "raise", amount: 100 }
//! }));
//! ```

use axum::{
    extract::{
        Path, Query, State,
        ws::{Message, WebSocket, WebSocketUpgrade},
    },
    http::StatusCode,
    response::{IntoResponse, Response},
};
use futures_util::{SinkExt, StreamExt};
use log::{error, info, warn};
use private_poker::entities::Action;
use serde::{Deserialize, Serialize};

use super::{AppState, rate_limiter::RateLimiter};

#[derive(Debug, Deserialize)]
pub struct WsQuery {
    token: String,
}

/// Client messages received via WebSocket
///
/// Note: Join functionality is intentionally disabled via WebSocket.
/// Clients should use the HTTP API (POST /api/tables/{id}/join) for joining tables
/// as it provides better error handling, idempotency, and atomic wallet operations.
#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum ClientMessage {
    /// Join table (DISABLED - use HTTP API instead)
    ///
    /// This variant is kept for backwards compatibility with existing clients
    /// but always returns an error directing users to the HTTP endpoint.
    Join {
        #[allow(dead_code)]
        buy_in: i64,
    },
    /// Leave the current table
    Leave,
    /// Take a poker action (fold, check, call, raise, all-in)
    Action { action: ActionData },
    /// Start spectating the table
    Spectate,
    /// Stop spectating the table
    StopSpectating,
}

/// Action data from client
#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum ActionData {
    Fold,
    Check,
    Call,
    Raise { amount: Option<u32> },
    AllIn,
}

/// Response messages sent to client
#[derive(Debug, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum ServerResponse {
    Success { message: String },
    Error { message: String },
}

/// Upgrade HTTP connection to WebSocket for real-time table communication.
///
/// Validates the JWT access token and establishes a WebSocket connection to the specified table.
/// Once connected, the client receives periodic game view updates and can send commands.
///
/// # Path Parameters
///
/// - `table_id`: Table ID to connect to
///
/// # Query Parameters
///
/// - `token`: JWT access token for authentication
///
/// # Response
///
/// On success, upgrades connection to WebSocket protocol (101 Switching Protocols).
/// On authentication failure, returns `401 Unauthorized`.
///
/// # WebSocket Lifecycle
///
/// 1. Connection established and authenticated
/// 2. Server spawns send task (game view updates every 1s)
/// 3. Server processes incoming client messages
/// 4. On disconnect, cleanup both tasks
///
/// # Example
///
/// ```javascript
/// const ws = new WebSocket('ws://localhost:3000/ws/1?token=eyJhbGc...');
/// ```
pub async fn websocket_handler(
    ws: WebSocketUpgrade,
    Path(table_id): Path<i64>,
    Query(query): Query<WsQuery>,
    State(state): State<AppState>,
) -> Response {
    // Verify token
    let user_id = match state.auth_manager.verify_access_token(&query.token) {
        Ok(claims) => claims.sub,
        Err(_) => {
            return (StatusCode::UNAUTHORIZED, "Invalid token").into_response();
        }
    };

    ws.on_upgrade(move |socket| handle_socket(socket, table_id, user_id, state))
}

/// Handle an established WebSocket connection.
///
/// Manages the WebSocket lifecycle including:
/// - Spawning send task for periodic game view updates
/// - Processing incoming client messages
/// - Cleanup on disconnect
///
/// The send task runs continuously until the connection closes, sending:
/// - Game view updates every 1 second (if user has joined table)
/// - Command responses (success/error messages)
///
/// # Arguments
///
/// - `socket`: The WebSocket connection
/// - `table_id`: Table the user is connected to
/// - `user_id`: Authenticated user ID
/// - `state`: Shared application state (table manager, auth manager, etc.)
async fn handle_socket(socket: WebSocket, table_id: i64, user_id: i64, state: AppState) {
    let (mut sender, mut receiver) = socket.split();

    info!("WebSocket connected: table={}, user={}", table_id, user_id);

    // Create rate limiters for DoS protection
    let mut burst_limiter = RateLimiter::burst(); // 10 messages per second
    let mut sustained_limiter = RateLimiter::sustained(); // 100 messages per minute

    // Create channel for sending responses from message handler
    let (response_tx, mut response_rx) = tokio::sync::mpsc::channel::<String>(32);

    // Subscribe to table state change notifications
    let (notification_tx, mut notification_rx) =
        tokio::sync::mpsc::channel::<private_poker::table::messages::StateChangeNotification>(32);

    // Send Subscribe message to table actor
    let table_handle = match state.table_manager.get_table(table_id).await {
        Some(h) => h,
        None => {
            error!("Table {} not found", table_id);
            return;
        }
    };

    if table_handle
        .send(private_poker::table::messages::TableMessage::Subscribe {
            user_id,
            sender: notification_tx,
        })
        .await
        .is_err()
    {
        error!("Failed to subscribe to table {} notifications", table_id);
        return;
    }

    // Spawn task to send table updates and responses (event-driven)
    let send_state = state.clone();
    let send_task = tokio::spawn(async move {
        loop {
            tokio::select! {
                // Receive state change notification from table actor
                Some(_notification) = notification_rx.recv() => {
                    // Get updated game view for this user
                    let table_handle = match send_state.table_manager.get_table(table_id).await {
                        Some(h) => h,
                        None => {
                            error!("Table {} not found", table_id);
                            break;
                        }
                    };

                    let (tx, rx) = tokio::sync::oneshot::channel();
                    if table_handle
                        .send(private_poker::table::messages::TableMessage::GetGameView {
                            user_id,
                            response: tx,
                        })
                        .await
                        .is_err()
                    {
                        error!("Failed to send GetGameView message");
                        break;
                    }

                    match rx.await {
                        Ok(Some(game_view)) => {
                            let json = match serde_json::to_string(&game_view) {
                                Ok(j) => j,
                                Err(e) => {
                                    error!("Failed to serialize game view: {}", e);
                                    continue;
                                }
                            };

                            if sender.send(Message::Text(json.into())).await.is_err() {
                                break;
                            }
                        }
                        Ok(None) => {
                            // User doesn't have a view yet (not joined)
                        }
                        Err(e) => {
                            error!("Failed to receive game view: {}", e);
                            break;
                        }
                    }
                }
                Some(response_json) = response_rx.recv() => {
                    // Send response from message handler
                    if sender.send(Message::Text(response_json.into())).await.is_err() {
                        break;
                    }
                }
            }
        }
    });

    // Receive messages from client
    while let Some(msg) = receiver.next().await {
        match msg {
            Ok(Message::Text(text)) => {
                // Check rate limits before processing
                if !burst_limiter.check() {
                    warn!(
                        "Burst rate limit exceeded for user {} (table {}). Blocking message.",
                        user_id, table_id
                    );
                    let error_response = ServerResponse::Error {
                        message: "Rate limit exceeded. Please slow down.".to_string(),
                    };
                    if let Ok(json) = serde_json::to_string(&error_response) {
                        let _ = response_tx.send(json).await;
                    }
                    continue;
                }

                if !sustained_limiter.check() {
                    warn!(
                        "Sustained rate limit exceeded for user {} (table {}). Blocking message.",
                        user_id, table_id
                    );
                    let error_response = ServerResponse::Error {
                        message: "Too many messages. Please wait before sending more.".to_string(),
                    };
                    if let Ok(json) = serde_json::to_string(&error_response) {
                        let _ = response_tx.send(json).await;
                    }
                    continue;
                }

                info!("Received message from user {}: {}", user_id, text);

                // Parse and process message
                let response = match serde_json::from_str::<ClientMessage>(&text) {
                    Ok(client_msg) => {
                        handle_client_message(client_msg, table_id, user_id, &state).await
                    }
                    Err(e) => {
                        warn!("Failed to parse client message: {}", e);
                        ServerResponse::Error {
                            message: "Invalid message format".to_string(),
                        }
                    }
                };

                // Send response through channel
                if let Ok(json) = serde_json::to_string(&response)
                    && response_tx.send(json).await.is_err()
                {
                    break;
                }
            }
            Ok(Message::Close(_)) => {
                info!("WebSocket closed: table={}, user={}", table_id, user_id);
                break;
            }
            Err(e) => {
                error!("WebSocket error: {}", e);
                break;
            }
            _ => {}
        }
    }

    // Cleanup - automatically leave table on disconnect
    send_task.abort();

    // Unsubscribe from table notifications
    if let Some(table_handle) = state.table_manager.get_table(table_id).await {
        let _ = table_handle
            .send(private_poker::table::messages::TableMessage::Unsubscribe { user_id })
            .await;
    }

    // Attempt to leave table if user was playing
    if let Some(table_handle) = state.table_manager.get_table(table_id).await {
        let (tx, rx) = tokio::sync::oneshot::channel();
        let leave_msg = private_poker::table::messages::TableMessage::LeaveTable {
            user_id,
            response: tx,
        };

        if let Ok(()) = table_handle.send(leave_msg).await {
            match rx.await {
                Ok(private_poker::table::messages::TableResponse::Success) => {
                    info!(
                        "User {} automatically left table {} on WebSocket disconnect",
                        user_id, table_id
                    );
                }
                Ok(_) => {
                    // User wasn't at table or already left - this is fine
                }
                Err(e) => {
                    warn!(
                        "Failed to get leave response for user {} on disconnect: {}",
                        user_id, e
                    );
                }
            }
        }
    }

    info!(
        "WebSocket disconnected: table={}, user={}",
        table_id, user_id
    );
}

/// Process a client command message and return a response.
///
/// Parses the client's command and forwards it to the appropriate table actor via
/// message passing. Waits for the table actor's response and converts it to a
/// WebSocket response message.
///
/// # Arguments
///
/// - `msg`: Parsed client message (Join, Leave, Action, Spectate, StopSpectating)
/// - `table_id`: Table ID the command applies to
/// - `user_id`: User ID making the request
/// - `state`: Application state with table manager
///
/// # Returns
///
/// - `ServerResponse::Success`: Command executed successfully
/// - `ServerResponse::Error`: Command failed with error message
///
/// # Supported Commands
///
/// - **Join**: Join table with buy-in amount
/// - **Leave**: Leave table and cash out
/// - **Action**: Take poker action (fold, check, call, raise, all-in)
/// - **Spectate**: Start spectating the table
/// - **StopSpectating**: Stop spectating
async fn handle_client_message(
    msg: ClientMessage,
    table_id: i64,
    user_id: i64,
    state: &AppState,
) -> ServerResponse {
    use private_poker::table::messages::{TableMessage, TableResponse};

    let table_handle = match state.table_manager.get_table(table_id).await {
        Some(handle) => handle,
        None => {
            return ServerResponse::Error {
                message: "Table not found".to_string(),
            };
        }
    };

    match msg {
        ClientMessage::Join { buy_in: _ } => {
            // Join via WebSocket is disabled - use HTTP API instead
            ServerResponse::Error {
                message: "Please join via HTTP API: POST /api/tables/{id}/join with {\"buy_in_amount\": 1000}".to_string(),
            }
        }

        ClientMessage::Leave => {
            let (tx, rx) = tokio::sync::oneshot::channel();

            if table_handle
                .send(TableMessage::LeaveTable {
                    user_id,
                    response: tx,
                })
                .await
                .is_err()
            {
                return ServerResponse::Error {
                    message: "Failed to send leave request".to_string(),
                };
            }

            match rx.await {
                Ok(TableResponse::Success) => ServerResponse::Success {
                    message: "Left table successfully".to_string(),
                },
                Ok(TableResponse::Error(e)) => ServerResponse::Error { message: e },
                _ => ServerResponse::Error {
                    message: "Unexpected response".to_string(),
                },
            }
        }

        ClientMessage::Action { action } => {
            let game_action = match action {
                ActionData::Fold => Action::Fold,
                ActionData::Check => Action::Check,
                ActionData::Call => Action::Call,
                ActionData::Raise { amount } => Action::Raise(amount),
                ActionData::AllIn => Action::AllIn,
            };

            let (tx, rx) = tokio::sync::oneshot::channel();

            if table_handle
                .send(TableMessage::TakeAction {
                    user_id,
                    action: game_action,
                    response: tx,
                })
                .await
                .is_err()
            {
                return ServerResponse::Error {
                    message: "Failed to send action".to_string(),
                };
            }

            match rx.await {
                Ok(TableResponse::Success) => ServerResponse::Success {
                    message: "Action processed successfully".to_string(),
                },
                Ok(TableResponse::Error(e)) => ServerResponse::Error { message: e },
                _ => ServerResponse::Error {
                    message: "Unexpected response".to_string(),
                },
            }
        }

        ClientMessage::Spectate => {
            let (tx, rx) = tokio::sync::oneshot::channel();
            let username = format!("user_{}", user_id);

            if table_handle
                .send(TableMessage::Spectate {
                    user_id,
                    username,
                    response: tx,
                })
                .await
                .is_err()
            {
                return ServerResponse::Error {
                    message: "Failed to spectate".to_string(),
                };
            }

            match rx.await {
                Ok(TableResponse::Success) => ServerResponse::Success {
                    message: "Now spectating".to_string(),
                },
                Ok(TableResponse::Error(e)) => ServerResponse::Error { message: e },
                _ => ServerResponse::Error {
                    message: "Unexpected response".to_string(),
                },
            }
        }

        ClientMessage::StopSpectating => {
            let (tx, rx) = tokio::sync::oneshot::channel();

            if table_handle
                .send(TableMessage::StopSpectating {
                    user_id,
                    response: tx,
                })
                .await
                .is_err()
            {
                return ServerResponse::Error {
                    message: "Failed to stop spectating".to_string(),
                };
            }

            match rx.await {
                Ok(TableResponse::Success) => ServerResponse::Success {
                    message: "Stopped spectating".to_string(),
                },
                Ok(TableResponse::Error(e)) => ServerResponse::Error { message: e },
                _ => ServerResponse::Error {
                    message: "Unexpected response".to_string(),
                },
            }
        }
    }
}
