//! Table actor message types.

use crate::game::entities::{Action, GameView};
use crate::wallet::TableId;
use tokio::sync::oneshot;

/// Messages that can be sent to a TableActor
#[derive(Debug)]
pub enum TableMessage {
    /// Join table request
    JoinTable {
        user_id: i64,
        username: String,
        buy_in_amount: i64,
        passphrase: Option<String>,
        response: oneshot::Sender<TableResponse>,
    },

    /// Leave table request
    LeaveTable {
        user_id: i64,
        response: oneshot::Sender<TableResponse>,
    },

    /// Player action (fold, check, call, raise, all-in)
    TakeAction {
        user_id: i64,
        action: Action,
        response: oneshot::Sender<TableResponse>,
    },

    /// Get current table state
    GetState {
        user_id: Option<i64>,
        response: oneshot::Sender<TableStateResponse>,
    },

    /// Get game view for a specific user
    GetGameView {
        user_id: i64,
        response: oneshot::Sender<Option<GameView>>,
    },

    /// Spectate table (read-only)
    Spectate {
        user_id: i64,
        username: String,
        response: oneshot::Sender<TableResponse>,
    },

    /// Stop spectating
    StopSpectating {
        user_id: i64,
        response: oneshot::Sender<TableResponse>,
    },

    /// Join waitlist
    JoinWaitlist {
        user_id: i64,
        username: String,
        response: oneshot::Sender<TableResponse>,
    },

    /// Leave waitlist
    LeaveWaitlist {
        user_id: i64,
        response: oneshot::Sender<TableResponse>,
    },

    /// Send chat message
    SendChat {
        user_id: i64,
        message: String,
        response: oneshot::Sender<TableResponse>,
    },

    /// Top-up chips from wallet to table
    TopUp {
        user_id: i64,
        amount: i64,
        response: oneshot::Sender<TableResponse>,
    },

    /// Pause table (admin only)
    Pause {
        response: oneshot::Sender<TableResponse>,
    },

    /// Resume table (admin only)
    Resume {
        response: oneshot::Sender<TableResponse>,
    },

    /// Close table (admin only)
    Close {
        response: oneshot::Sender<TableResponse>,
    },

    /// Internal: Advance game state (called by timer)
    Tick,

    /// Subscribe to state change notifications
    Subscribe {
        user_id: i64,
        sender: tokio::sync::mpsc::Sender<StateChangeNotification>,
    },

    /// Unsubscribe from state change notifications
    Unsubscribe { user_id: i64 },
}

/// Notification sent when table state changes
#[derive(Debug, Clone)]
pub enum StateChangeNotification {
    /// Game state changed (action taken, new round, etc.)
    StateChanged,
    /// Player joined or left
    PlayerListChanged,
    /// Pot size changed
    PotChanged,
}

/// Response from table operations
#[derive(Debug, Clone)]
pub enum TableResponse {
    /// Operation succeeded
    Success,

    /// Operation succeeded with message
    SuccessWithMessage(String),

    /// Operation failed
    Error(String),

    /// Table is full
    TableFull,

    /// Insufficient chips for buy-in
    InsufficientChips { required: i64, available: i64 },

    /// Not your turn
    NotYourTurn,

    /// Invalid action for current game state
    InvalidAction(String),

    /// Access denied (private table, wrong passphrase)
    AccessDenied,

    /// Player not at table
    NotAtTable,

    /// Rate limited (chat, top-up cooldown)
    RateLimited { retry_after_secs: u64 },
}

/// Table state response
#[derive(Debug, Clone, serde::Serialize)]
pub struct TableStateResponse {
    /// Table ID
    pub table_id: TableId,

    /// Table name
    pub table_name: String,

    /// Current player count
    pub player_count: usize,

    /// Maximum players
    pub max_players: usize,

    /// Waitlist count
    pub waitlist_count: usize,

    /// Spectator count
    pub spectator_count: usize,

    /// Small blind
    pub small_blind: i64,

    /// Big blind
    pub big_blind: i64,

    /// Current pot size
    pub pot_size: i64,

    /// Is table active (not paused)
    pub is_active: bool,

    /// Current game phase
    pub phase: String,

    /// Player usernames at table
    pub players: Vec<String>,

    /// Is private table
    pub is_private: bool,

    /// Table speed
    pub speed: String,
}

impl TableResponse {
    /// Check if response is success
    pub fn is_success(&self) -> bool {
        matches!(
            self,
            TableResponse::Success | TableResponse::SuccessWithMessage(_)
        )
    }

    /// Get error message if response is error
    pub fn error_message(&self) -> Option<String> {
        match self {
            TableResponse::Error(msg) => Some(msg.clone()),
            TableResponse::TableFull => Some("Table is full".to_string()),
            TableResponse::InsufficientChips {
                required,
                available,
            } => Some(format!(
                "Insufficient chips: need {}, have {}",
                required, available
            )),
            TableResponse::NotYourTurn => Some("Not your turn".to_string()),
            TableResponse::InvalidAction(msg) => Some(format!("Invalid action: {}", msg)),
            TableResponse::AccessDenied => Some("Access denied".to_string()),
            TableResponse::NotAtTable => Some("Not at table".to_string()),
            TableResponse::RateLimited { retry_after_secs } => Some(format!(
                "Rate limited, retry after {} seconds",
                retry_after_secs
            )),
            _ => None,
        }
    }
}
