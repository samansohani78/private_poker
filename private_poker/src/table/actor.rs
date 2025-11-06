//! Table actor implementation with async message handling.

use super::{
    config::TableConfig,
    messages::{TableMessage, TableResponse, TableStateResponse},
};
use crate::{
    game::{entities::User, PokerState},
    wallet::{TableId, WalletManager},
};
use std::sync::Arc;
use tokio::{
    sync::mpsc,
    time::{interval, Duration},
};

/// Table actor handle for sending messages
#[derive(Clone)]
pub struct TableHandle {
    sender: mpsc::Sender<TableMessage>,
    table_id: TableId,
}

impl TableHandle {
    /// Create a new table handle
    pub fn new(sender: mpsc::Sender<TableMessage>, table_id: TableId) -> Self {
        Self { sender, table_id }
    }

    /// Get table ID
    pub fn table_id(&self) -> TableId {
        self.table_id
    }

    /// Send a message to the table
    pub async fn send(&self, message: TableMessage) -> Result<(), String> {
        self.sender
            .send(message)
            .await
            .map_err(|_| "Table is closed".to_string())
    }
}

/// Table actor managing a single poker table
pub struct TableActor {
    /// Table ID
    id: TableId,

    /// Table configuration
    config: TableConfig,

    /// Poker game state (FSM)
    state: PokerState,

    /// Message inbox
    inbox: mpsc::Receiver<TableMessage>,

    /// Wallet manager for buy-ins/cash-outs
    wallet_manager: Arc<WalletManager>,

    /// Is table paused
    is_paused: bool,

    /// Is table closed
    is_closed: bool,

    /// Last top-up times (user_id -> hand_count)
    top_up_tracker: std::collections::HashMap<i64, u32>,

    /// Current hand count
    hand_count: u32,
}

impl TableActor {
    /// Create a new table actor
    ///
    /// # Arguments
    ///
    /// * `id` - Table ID
    /// * `config` - Table configuration
    /// * `wallet_manager` - Wallet manager reference
    ///
    /// # Returns
    ///
    /// * `(TableActor, TableHandle)` - Actor and handle for sending messages
    pub fn new(
        id: TableId,
        config: TableConfig,
        wallet_manager: Arc<WalletManager>,
    ) -> (Self, TableHandle) {
        let (sender, inbox) = mpsc::channel(100);

        // Create initial poker state
        let state = PokerState::new();

        let actor = Self {
            id,
            config,
            state,
            inbox,
            wallet_manager,
            is_paused: false,
            is_closed: false,
            top_up_tracker: std::collections::HashMap::new(),
            hand_count: 0,
        };

        let handle = TableHandle::new(sender, id);

        (actor, handle)
    }

    /// Run the table actor event loop
    pub async fn run(mut self) {
        log::info!("Table {} '{}' starting", self.id, self.config.name);

        // Create tick interval based on table speed
        let tick_duration = Duration::from_secs(1);
        let mut tick_interval = interval(tick_duration);

        loop {
            tokio::select! {
                // Handle incoming messages
                Some(message) = self.inbox.recv() => {
                    if let Err(e) = self.handle_message(message).await {
                        log::error!("Table {}: Error handling message: {}", self.id, e);
                    }

                    if self.is_closed {
                        break;
                    }
                }

                // Handle periodic ticks (game state advancement)
                _ = tick_interval.tick() => {
                    if !self.is_paused && !self.is_closed {
                        self.tick().await;
                    }
                }
            }
        }

        log::info!("Table {} '{}' closed", self.id, self.config.name);
    }

    /// Handle a table message
    async fn handle_message(&mut self, message: TableMessage) -> Result<(), String> {
        match message {
            TableMessage::JoinTable {
                user_id,
                username,
                buy_in_amount,
                passphrase,
                response,
            } => {
                let result = self.handle_join(user_id, username, buy_in_amount, passphrase).await;
                let _ = response.send(result);
            }

            TableMessage::LeaveTable { user_id, response } => {
                let result = self.handle_leave(user_id).await;
                let _ = response.send(result);
            }

            TableMessage::TakeAction {
                user_id,
                action,
                response,
            } => {
                let result = self.handle_action(user_id, action).await;
                let _ = response.send(result);
            }

            TableMessage::GetState { user_id, response } => {
                let result = self.get_state(user_id).await;
                let _ = response.send(result);
            }

            TableMessage::Spectate {
                user_id,
                username,
                response,
            } => {
                let result = self.handle_spectate(user_id, username).await;
                let _ = response.send(result);
            }

            TableMessage::StopSpectating { user_id, response } => {
                let result = self.handle_stop_spectating(user_id).await;
                let _ = response.send(result);
            }

            TableMessage::JoinWaitlist {
                user_id,
                username,
                response,
            } => {
                let result = self.handle_join_waitlist(user_id, username).await;
                let _ = response.send(result);
            }

            TableMessage::LeaveWaitlist { user_id, response } => {
                let result = self.handle_leave_waitlist(user_id).await;
                let _ = response.send(result);
            }

            TableMessage::SendChat {
                user_id,
                message: _message,
                response,
            } => {
                // TODO: Implement chat with rate limiting and profanity filter
                let result = self.handle_chat(user_id).await;
                let _ = response.send(result);
            }

            TableMessage::TopUp {
                user_id,
                amount,
                response,
            } => {
                let result = self.handle_top_up(user_id, amount).await;
                let _ = response.send(result);
            }

            TableMessage::Pause { response } => {
                self.is_paused = true;
                let _ = response.send(TableResponse::Success);
            }

            TableMessage::Resume { response } => {
                self.is_paused = false;
                let _ = response.send(TableResponse::Success);
            }

            TableMessage::Close { response } => {
                self.is_closed = true;
                let _ = response.send(TableResponse::Success);
            }

            TableMessage::Tick => {
                self.tick().await;
            }

            TableMessage::ProcessBotTurn => {
                // TODO: Implement bot turn processing
            }
        }

        Ok(())
    }

    /// Handle join table request
    async fn handle_join(
        &mut self,
        user_id: i64,
        username: String,
        buy_in_amount: i64,
        passphrase: Option<String>,
    ) -> TableResponse {
        // Check if table is private and verify access
        if self.config.is_private {
            if let Some(ref required_hash) = self.config.passphrase_hash {
                // TODO: Verify passphrase hash
                if passphrase.is_none() {
                    return TableResponse::AccessDenied;
                }
            }
        }

        // Validate buy-in amount
        let min_buy_in = self.config.min_buy_in_chips();
        let max_buy_in = self.config.max_buy_in_chips();

        if buy_in_amount < min_buy_in || buy_in_amount > max_buy_in {
            return TableResponse::Error(format!(
                "Buy-in must be between {} and {} chips",
                min_buy_in, max_buy_in
            ));
        }

        // Check wallet balance
        match self.wallet_manager.get_wallet(user_id).await {
            Ok(wallet) => {
                if wallet.balance < buy_in_amount {
                    return TableResponse::InsufficientChips {
                        required: buy_in_amount,
                        available: wallet.balance,
                    };
                }
            }
            Err(e) => {
                return TableResponse::Error(format!("Wallet error: {}", e));
            }
        }

        // Transfer chips to escrow
        let idempotency_key = format!("join_{}_{}", user_id, chrono::Utc::now().timestamp());
        match self
            .wallet_manager
            .transfer_to_escrow(user_id, self.id, buy_in_amount, idempotency_key)
            .await
        {
            Ok(_) => {
                // Add user to game
                let _user = User {
                    name: username.into(),
                    money: buy_in_amount as u32,
                };

                // TODO: Integrate with PokerState to add user
                // For now, just return success
                log::info!(
                    "User {} joined table {} with {} chips",
                    user_id,
                    self.id,
                    buy_in_amount
                );

                TableResponse::Success
            }
            Err(e) => TableResponse::Error(format!("Transfer failed: {}", e)),
        }
    }

    /// Handle leave table request
    async fn handle_leave(&mut self, user_id: i64) -> TableResponse {
        // TODO: Get user's current chip count from PokerState
        let chip_count = 0i64; // Placeholder

        // Transfer chips back from escrow
        let idempotency_key = format!("leave_{}_{}", user_id, chrono::Utc::now().timestamp());
        match self
            .wallet_manager
            .transfer_from_escrow(user_id, self.id, chip_count, idempotency_key)
            .await
        {
            Ok(_) => {
                // Remove user from game
                // TODO: Integrate with PokerState to remove user
                log::info!("User {} left table {}", user_id, self.id);
                TableResponse::Success
            }
            Err(e) => TableResponse::Error(format!("Transfer failed: {}", e)),
        }
    }

    /// Handle player action
    async fn handle_action(&mut self, _user_id: i64, _action: crate::game::entities::Action) -> TableResponse {
        // TODO: Validate it's user's turn and action is valid
        // TODO: Apply action to PokerState
        TableResponse::Success
    }

    /// Get current table state
    async fn get_state(&self, _user_id: Option<i64>) -> TableStateResponse {
        // TODO: Extract state from PokerState
        TableStateResponse {
            table_id: self.id,
            table_name: self.config.name.clone(),
            player_count: 0,
            max_players: self.config.max_players,
            waitlist_count: 0,
            spectator_count: 0,
            small_blind: self.config.small_blind,
            big_blind: self.config.big_blind,
            pot_size: 0,
            is_active: !self.is_paused,
            phase: "Lobby".to_string(),
            players: vec![],
            is_private: self.config.is_private,
            speed: self.config.speed.to_string(),
        }
    }

    /// Handle spectate request
    async fn handle_spectate(&mut self, _user_id: i64, _username: String) -> TableResponse {
        // TODO: Add spectator to PokerState
        TableResponse::Success
    }

    /// Handle stop spectating request
    async fn handle_stop_spectating(&mut self, _user_id: i64) -> TableResponse {
        // TODO: Remove spectator from PokerState
        TableResponse::Success
    }

    /// Handle join waitlist request
    async fn handle_join_waitlist(&mut self, _user_id: i64, _username: String) -> TableResponse {
        // TODO: Add to waitlist in PokerState
        TableResponse::Success
    }

    /// Handle leave waitlist request
    async fn handle_leave_waitlist(&mut self, _user_id: i64) -> TableResponse {
        // TODO: Remove from waitlist in PokerState
        TableResponse::Success
    }

    /// Handle chat message
    async fn handle_chat(&mut self, _user_id: i64) -> TableResponse {
        // TODO: Implement chat with rate limiting
        TableResponse::Success
    }

    /// Handle top-up request
    async fn handle_top_up(&mut self, user_id: i64, amount: i64) -> TableResponse {
        // Check top-up cooldown
        if let Some(&last_hand) = self.top_up_tracker.get(&user_id) {
            let hands_since = self.hand_count - last_hand;
            if hands_since < self.config.top_up_cooldown_hands as u32 {
                let remaining = self.config.top_up_cooldown_hands as u32 - hands_since;
                return TableResponse::RateLimited {
                    retry_after_secs: remaining as u64 * 60, // Rough estimate
                };
            }
        }

        // Validate amount
        if amount <= 0 {
            return TableResponse::Error("Amount must be positive".to_string());
        }

        // TODO: Transfer chips and update player stack
        self.top_up_tracker.insert(user_id, self.hand_count);
        TableResponse::Success
    }

    /// Advance game state (called periodically)
    async fn tick(&mut self) {
        if self.is_paused || self.is_closed {
            return;
        }

        // Advance poker state FSM (take ownership and replace)
        let state = std::mem::replace(&mut self.state, PokerState::new());
        self.state = state.step();

        // TODO: Check if hand completed and increment hand_count
        // TODO: Handle bot turns
        // TODO: Handle timeouts
    }
}
