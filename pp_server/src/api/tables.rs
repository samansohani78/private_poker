//! Table management API handlers.
//!
//! This module provides HTTP REST endpoints for poker table operations including:
//! - Listing all active tables with player counts and blind levels
//! - Getting detailed state of a specific table
//! - Joining tables with buy-in amounts
//! - Leaving tables and cashing out chips
//! - Taking poker actions (fold, check, call, raise, all-in)
//!
//! Most endpoints require authentication via JWT bearer token except for listing tables.
//!
//! # Examples
//!
//! List all tables:
//! ```bash
//! curl http://localhost:3000/api/tables
//! ```
//!
//! Join a table:
//! ```bash
//! curl -X POST http://localhost:3000/api/tables/1/join \
//!   -H "Authorization: Bearer TOKEN" \
//!   -H "Content-Type: application/json" \
//!   -d '{"buy_in_amount": 1000, "passphrase": null}'
//! ```

use axum::{
    extract::{Path, State, Extension},
    http::StatusCode,
    Json,
};
use serde::{Deserialize, Serialize};
use private_poker::{
    game::entities::Action,
    table::messages::TableMessage,
};

use super::AppState;

#[derive(Debug, Serialize)]
pub struct TableListItem {
    pub id: i64,
    pub name: String,
    pub max_players: i32,
    pub player_count: usize,
    pub small_blind: i64,
    pub big_blind: i64,
    pub is_private: bool,
}

#[derive(Debug, Serialize)]
pub struct TableStateResponse {
    pub id: i64,
    pub name: String,
    pub players: Vec<String>,
    pub pot_size: i64,
    pub phase: String,
}

#[derive(Debug, Deserialize)]
pub struct JoinTableRequest {
    pub buy_in_amount: i64,
    pub passphrase: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct TakeActionRequest {
    pub action: ActionPayload,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type", content = "amount")]
pub enum ActionPayload {
    Fold,
    Check,
    Call,
    Raise(u32),
    AllIn,
}

impl From<ActionPayload> for Action {
    fn from(payload: ActionPayload) -> Self {
        match payload {
            ActionPayload::Fold => Action::Fold,
            ActionPayload::Check => Action::Check,
            ActionPayload::Call => Action::Call,
            ActionPayload::Raise(amount) => Action::Raise(Some(amount)),
            ActionPayload::AllIn => Action::AllIn,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct ErrorResponse {
    pub error: String,
}

/// List all active poker tables.
///
/// Returns a list of all tables currently running on the server with basic information
/// including name, player count, blinds, and privacy status. This endpoint does not
/// require authentication.
///
/// # Response
///
/// Returns `200 OK` with array of table summaries:
/// ```json
/// [
///   {
///     "id": 1,
///     "name": "High Stakes Table",
///     "max_players": 9,
///     "player_count": 5,
///     "small_blind": 10,
///     "big_blind": 20,
///     "is_private": false
///   }
/// ]
/// ```
///
/// # Errors
///
/// - `500 Internal Server Error`: Database or server error
pub async fn list_tables(
    State(state): State<AppState>,
) -> Result<Json<Vec<TableListItem>>, (StatusCode, Json<ErrorResponse>)> {
    match state.table_manager.list_tables().await {
        Ok(tables) => {
            let items = tables
                .into_iter()
                .map(|t| TableListItem {
                    id: t.id,
                    name: t.name,
                    max_players: t.max_players as i32,
                    player_count: t.player_count,
                    small_blind: t.small_blind,
                    big_blind: t.big_blind,
                    is_private: t.is_private,
                })
                .collect();
            Ok(Json(items))
        }
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse { error: e }),
        )),
    }
}

/// Get detailed state of a specific table.
///
/// Returns current game state for the specified table including players, pot size,
/// and game phase. Requires authentication to view table details.
///
/// # Path Parameters
///
/// - `table_id`: Table ID (integer)
///
/// # Authentication
///
/// Requires valid JWT bearer token in `Authorization` header.
///
/// # Response
///
/// Returns `200 OK` with table state:
/// ```json
/// {
///   "id": 1,
///   "name": "High Stakes Table",
///   "players": ["player123", "player456"],
///   "pot_size": 450,
///   "phase": "TakeAction"
/// }
/// ```
///
/// # Errors
///
/// - `401 Unauthorized`: Missing or invalid authentication token
/// - `404 Not Found`: Table doesn't exist
pub async fn get_table(
    State(state): State<AppState>,
    Extension(user_id): Extension<i64>,
    Path(table_id): Path<i64>,
) -> Result<Json<TableStateResponse>, (StatusCode, Json<ErrorResponse>)> {
    match state.table_manager.get_table_state(table_id, Some(user_id)).await {
        Ok(table_state) => Ok(Json(TableStateResponse {
            id: table_id,
            name: table_state.table_name,
            players: table_state.players,
            pot_size: table_state.pot_size,
            phase: table_state.phase,
        })),
        Err(e) => Err((
            StatusCode::NOT_FOUND,
            Json(ErrorResponse { error: e }),
        )),
    }
}

/// Join a poker table with a buy-in.
///
/// Seats the authenticated user at the specified table with the provided buy-in amount.
/// The buy-in must be within the table's configured minimum and maximum limits.
/// For private tables, a valid passphrase is required.
///
/// # Path Parameters
///
/// - `table_id`: Table ID (integer)
///
/// # Authentication
///
/// Requires valid JWT bearer token in `Authorization` header.
///
/// # Request Body
///
/// ```json
/// {
///   "buy_in_amount": 1000,
///   "passphrase": null  // Required for private tables
/// }
/// ```
///
/// # Response
///
/// Returns `200 OK` with empty body on success.
///
/// # Errors
///
/// - `400 Bad Request`: Table full, invalid buy-in, or wrong passphrase
/// - `401 Unauthorized`: Missing or invalid authentication token
///
/// # Notes
///
/// - Buy-in amount is deducted from user's wallet balance
/// - User must have sufficient balance in their wallet
/// - User is added to waitlist if table is full
pub async fn join_table(
    State(state): State<AppState>,
    Extension(user_id): Extension<i64>,
    Path(table_id): Path<i64>,
    Json(request): Json<JoinTableRequest>,
) -> Result<StatusCode, (StatusCode, Json<ErrorResponse>)> {
    // Get username from user_id
    // For now, use placeholder
    let username = format!("user_{}", user_id);

    match state
        .table_manager
        .join_table(table_id, user_id, username, request.buy_in_amount, request.passphrase)
        .await
    {
        Ok(_) => Ok(StatusCode::OK),
        Err(e) => Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse { error: e }),
        )),
    }
}

/// Leave a poker table and cash out.
///
/// Removes the authenticated user from the specified table and returns their
/// remaining chips to their wallet balance.
///
/// # Path Parameters
///
/// - `table_id`: Table ID (integer)
///
/// # Authentication
///
/// Requires valid JWT bearer token in `Authorization` header.
///
/// # Response
///
/// Returns `200 OK` with empty body on success.
///
/// # Errors
///
/// - `400 Bad Request`: User is not seated at this table
/// - `401 Unauthorized`: Missing or invalid authentication token
///
/// # Notes
///
/// - Player cannot leave during an active hand (must wait until hand completes)
/// - Remaining chips are automatically returned to wallet
/// - Other players are notified of the departure
pub async fn leave_table(
    State(state): State<AppState>,
    Extension(user_id): Extension<i64>,
    Path(table_id): Path<i64>,
) -> Result<StatusCode, (StatusCode, Json<ErrorResponse>)> {
    match state.table_manager.leave_table(table_id, user_id).await {
        Ok(_) => Ok(StatusCode::OK),
        Err(e) => Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse { error: e }),
        )),
    }
}

/// Take a poker action (fold, check, call, raise, all-in).
///
/// Submits the authenticated user's action for the current betting round.
/// The action must be legal given the current game state.
///
/// # Path Parameters
///
/// - `table_id`: Table ID (integer)
///
/// # Authentication
///
/// Requires valid JWT bearer token in `Authorization` header.
///
/// # Request Body
///
/// See [`ActionPayload`] for action format. Examples:
///
/// **Fold:**
/// ```json
/// {"action": {"type": "Fold"}}
/// ```
///
/// **Raise:**
/// ```json
/// {"action": {"type": "Raise", "amount": 100}}
/// ```
///
/// # Response
///
/// Returns `200 OK` with empty body on success.
///
/// # Errors
///
/// - `400 Bad Request`: Not your turn, invalid action, or insufficient chips
/// - `401 Unauthorized`: Missing or invalid authentication token
/// - `404 Not Found`: Table doesn't exist
///
/// # Valid Actions
///
/// - **Fold**: Give up the hand
/// - **Check**: Pass action (only when no bet to call)
/// - **Call**: Match current bet
/// - **Raise**: Increase the bet (must specify amount)
/// - **AllIn**: Bet all remaining chips
pub async fn take_action(
    State(state): State<AppState>,
    Extension(user_id): Extension<i64>,
    Path(table_id): Path<i64>,
    Json(request): Json<TakeActionRequest>,
) -> Result<StatusCode, (StatusCode, Json<ErrorResponse>)> {
    let action: Action = request.action.into();

    // Get table handle
    let table_handle = match state.table_manager.get_table(table_id).await {
        Some(handle) => handle,
        None => {
            return Err((
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    error: "Table not found".to_string(),
                }),
            ))
        }
    };

    // Send action message
    let (response_tx, response_rx) = tokio::sync::oneshot::channel();
    let message = TableMessage::TakeAction {
        user_id,
        action,
        response: response_tx,
    };

    if let Err(e) = table_handle.send(message).await {
        return Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse { error: e }),
        ));
    }

    match response_rx.await {
        Ok(response) => match response {
            private_poker::table::messages::TableResponse::Success => Ok(StatusCode::OK),
            private_poker::table::messages::TableResponse::SuccessWithMessage(_) => Ok(StatusCode::OK),
            private_poker::table::messages::TableResponse::Error(e) => Err((
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse { error: e }),
            )),
            private_poker::table::messages::TableResponse::TableFull => Err((
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse {
                    error: "Table is full".to_string(),
                }),
            )),
            private_poker::table::messages::TableResponse::NotYourTurn => Err((
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse {
                    error: "Not your turn".to_string(),
                }),
            )),
            private_poker::table::messages::TableResponse::InvalidAction(e) => Err((
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse { error: e }),
            )),
            _ => Err((
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse {
                    error: "Operation failed".to_string(),
                }),
            )),
        },
        Err(_) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: "Response channel closed".to_string(),
            }),
        )),
    }
}
