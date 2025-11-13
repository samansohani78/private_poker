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

use super::AppState;

#[derive(Debug, Deserialize)]
pub struct WsQuery {
    token: String,
}

/// Client messages received via WebSocket
#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum ClientMessage {
    Join { buy_in: i64 },
    Leave,
    Action { action: ActionData },
    Spectate,
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

    // Create channel for sending responses from message handler
    let (response_tx, mut response_rx) = tokio::sync::mpsc::channel::<String>(32);

    // Spawn task to send table updates and responses
    let send_state = state.clone();
    let send_task = tokio::spawn(async move {
        let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(1));
        loop {
            tokio::select! {
                _ = interval.tick() => {
                    // Get game view for this user
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
                info!("Received message from user {}: {}", user_id, text);

                // Parse and process message
                let response = match serde_json::from_str::<ClientMessage>(&text) {
                    Ok(client_msg) => {
                        handle_client_message(client_msg, table_id, user_id, &state).await
                    }
                    Err(e) => {
                        warn!("Failed to parse client message: {}", e);
                        ServerResponse::Error {
                            message: format!("Invalid message format: {}", e),
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

    // Cleanup
    send_task.abort();
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
        ClientMessage::Join { buy_in } => {
            let (tx, rx) = tokio::sync::oneshot::channel();

            // Get username from user_id (simplified - in production, fetch from database)
            let username = format!("user_{}", user_id);

            if table_handle
                .send(TableMessage::JoinTable {
                    user_id,
                    username,
                    buy_in_amount: buy_in,
                    passphrase: None,
                    response: tx,
                })
                .await
                .is_err()
            {
                return ServerResponse::Error {
                    message: "Failed to send join request".to_string(),
                };
            }

            match rx.await {
                Ok(TableResponse::Success) => ServerResponse::Success {
                    message: "Joined table successfully".to_string(),
                },
                Ok(TableResponse::Error(e)) => ServerResponse::Error { message: e },
                _ => ServerResponse::Error {
                    message: "Unexpected response".to_string(),
                },
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
