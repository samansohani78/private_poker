//! Table actor implementation with async message handling.

use super::{
    config::TableConfig,
    messages::{TableMessage, TableResponse, TableStateResponse},
};
use crate::{
    bot::BotManager,
    game::{
        GameStateManagement, PhaseDependentUserManagement, PhaseIndependentUserManagement,
        PokerState,
        entities::{Action, GameView, Username},
    },
    wallet::{TableId, WalletManager},
};
use sqlx::PgPool;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::{
    sync::mpsc,
    time::{Duration, interval},
};
use uuid::Uuid;

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

    /// Bot manager for automatic bot spawning
    bot_manager: BotManager,

    /// User ID to Username mapping
    user_mapping: HashMap<i64, Username>,

    /// Username to User ID reverse mapping
    username_mapping: HashMap<Username, i64>,

    /// Is table paused
    is_paused: bool,

    /// Is table closed
    is_closed: bool,

    /// Last top-up times (`user_id` -> `hand_count`)
    top_up_tracker: HashMap<i64, u32>,

    /// Current hand count
    hand_count: u32,

    /// Subscribers for state change notifications (for efficient WebSocket updates)
    subscribers: HashMap<i64, mpsc::Sender<super::messages::StateChangeNotification>>,
}

impl TableActor {
    /// Create a new table actor
    ///
    /// # Arguments
    ///
    /// * `id` - Table ID
    /// * `config` - Table configuration
    /// * `wallet_manager` - Wallet manager reference
    /// * `db_pool` - Database connection pool
    ///
    /// # Returns
    ///
    /// * `(TableActor, TableHandle)` - Actor and handle for sending messages
    pub fn new(
        id: TableId,
        config: TableConfig,
        wallet_manager: Arc<WalletManager>,
        db_pool: Arc<PgPool>,
    ) -> (Self, TableHandle) {
        let (sender, inbox) = mpsc::channel(100);

        // Create initial poker state
        let state = PokerState::new();

        // Create bot manager
        let bot_manager = BotManager::new(id, config.clone(), db_pool);

        let actor = Self {
            id,
            config,
            state,
            inbox,
            wallet_manager,
            bot_manager,
            user_mapping: HashMap::new(),
            username_mapping: HashMap::new(),
            is_paused: false,
            is_closed: false,
            top_up_tracker: HashMap::new(),
            hand_count: 0,
            subscribers: HashMap::new(),
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
                let result = self
                    .handle_join(user_id, username, buy_in_amount, passphrase)
                    .await;
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

            TableMessage::GetGameView { user_id, response } => {
                let result = self.get_game_view(user_id);
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
                user_id: _user_id,
                message: _message,
                response,
            } => {
                // Chat not implemented yet - requires rate limiting and profanity filter
                let _ = response.send(TableResponse::Error("Chat not implemented".to_string()));
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

            TableMessage::Subscribe { user_id, sender } => {
                self.subscribers.insert(user_id, sender);
                log::debug!(
                    "User {} subscribed to table {} state changes",
                    user_id,
                    self.id
                );
            }

            TableMessage::Unsubscribe { user_id } => {
                self.subscribers.remove(&user_id);
                log::debug!(
                    "User {} unsubscribed from table {} state changes",
                    user_id,
                    self.id
                );
            }
        }

        Ok(())
    }

    /// Broadcast state change notification to all subscribers
    fn notify_state_change(&mut self, notification: super::messages::StateChangeNotification) {
        self.subscribers.retain(|user_id, sender| {
            match sender.try_send(notification.clone()) {
                Ok(_) => true, // Keep subscriber
                Err(mpsc::error::TrySendError::Full(_)) => {
                    log::warn!("Subscriber {} channel full, dropping notification", user_id);
                    true // Keep subscriber but drop this notification
                }
                Err(mpsc::error::TrySendError::Closed(_)) => {
                    log::debug!("Subscriber {} disconnected, removing", user_id);
                    false // Remove subscriber
                }
            }
        });
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
        if self.config.is_private
            && let Some(ref required_hash) = self.config.passphrase_hash
        {
            match passphrase {
                Some(ref pass) => {
                    // Verify passphrase using argon2 with constant-time comparison
                    use argon2::{Argon2, PasswordHash, PasswordVerifier};

                    let parsed_hash = match PasswordHash::new(required_hash) {
                        Ok(h) => h,
                        Err(_) => {
                            log::error!("Invalid passphrase hash format for table {}", self.id);
                            return TableResponse::Error(
                                "Internal server error: invalid passphrase configuration"
                                    .to_string(),
                            );
                        }
                    };

                    let argon2 = Argon2::default();
                    if argon2
                        .verify_password(pass.as_bytes(), &parsed_hash)
                        .is_err()
                    {
                        return TableResponse::AccessDenied;
                    }
                }
                None => {
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

        // Enforce that buy-in must cover at least one big blind
        // This prevents players from joining with insufficient chips to play
        let big_blind = self.config.big_blind;
        if buy_in_amount < big_blind {
            return TableResponse::Error(format!(
                "Buy-in ({}) must be at least the big blind ({})",
                buy_in_amount, big_blind
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

        // Transfer chips to escrow with collision-resistant idempotency key
        let idempotency_key = format!(
            "join_{}_{}_{}",
            user_id,
            chrono::Utc::now().timestamp_millis(),
            Uuid::new_v4()
        );
        match self
            .wallet_manager
            .transfer_to_escrow(user_id, self.id, buy_in_amount, idempotency_key)
            .await
        {
            Ok(_) => {
                // Add user to game state
                let poker_username: Username = username.clone().into();

                match self.state.new_user(&poker_username) {
                    Ok(_) => {
                        // Store mappings
                        self.user_mapping.insert(user_id, poker_username.clone());
                        self.username_mapping.insert(poker_username, user_id);

                        // Adjust bot count now that a human joined
                        let human_count = self.user_mapping.len();
                        let _ = self.bot_manager.adjust_bot_count(human_count).await;

                        log::info!(
                            "User {} ({}) joined table {} with {} chips",
                            user_id,
                            username,
                            self.id,
                            buy_in_amount
                        );

                        // Notify subscribers that player list changed
                        self.notify_state_change(
                            super::messages::StateChangeNotification::PlayerListChanged,
                        );

                        TableResponse::Success
                    }
                    Err(e) => {
                        // Rollback the transfer with collision-resistant idempotency key
                        // Using millisecond timestamp for better precision than second-level timestamp
                        let rollback_key = format!(
                            "rollback_join_{}_{}",
                            user_id,
                            chrono::Utc::now().timestamp_millis()
                        );
                        match self
                            .wallet_manager
                            .transfer_from_escrow(user_id, self.id, buy_in_amount, rollback_key)
                            .await
                        {
                            Ok(_) => {
                                log::info!(
                                    "Successfully rolled back join transfer for user {} on table {}",
                                    user_id,
                                    self.id
                                );
                            }
                            Err(rollback_err) => {
                                log::error!(
                                    "CRITICAL: Failed to rollback join transfer for user {} on table {}: {}. Chips may be stuck in escrow!",
                                    user_id,
                                    self.id,
                                    rollback_err
                                );
                            }
                        }

                        TableResponse::Error(format!("Failed to join game: {:?}", e))
                    }
                }
            }
            Err(e) => TableResponse::Error(format!("Transfer failed: {}", e)),
        }
    }

    /// Handle leave table request
    async fn handle_leave(&mut self, user_id: i64) -> TableResponse {
        // Get username from mapping
        let username = match self.user_mapping.get(&user_id) {
            Some(u) => u.clone(),
            None => {
                return TableResponse::Error("User not at table".to_string());
            }
        };

        // Get user's current chip count from game state
        let views = self.state.get_views();
        let user_view = views.get(&username);

        let chip_count = match user_view {
            Some(view) => {
                // Find the player in the view
                view.players
                    .iter()
                    .find(|p| p.user.name == username)
                    .map(|p| p.user.money as i64)
                    .unwrap_or(0)
            }
            None => 0,
        };

        // Remove user from game state
        match self.state.remove_user(&username) {
            Ok(_) => {
                // Transfer chips back from escrow with collision-resistant idempotency key
                let idempotency_key = format!(
                    "leave_{}_{}_{}",
                    user_id,
                    chrono::Utc::now().timestamp_millis(),
                    Uuid::new_v4()
                );
                match self
                    .wallet_manager
                    .transfer_from_escrow(user_id, self.id, chip_count, idempotency_key)
                    .await
                {
                    Ok(_) => {
                        // Remove mappings
                        self.user_mapping.remove(&user_id);
                        self.username_mapping.remove(&username);

                        // Adjust bot count now that a human left
                        let human_count = self.user_mapping.len();
                        let _ = self.bot_manager.adjust_bot_count(human_count).await;

                        log::info!(
                            "User {} left table {} with {} chips",
                            user_id,
                            self.id,
                            chip_count
                        );

                        // Notify subscribers that player list changed
                        self.notify_state_change(
                            super::messages::StateChangeNotification::PlayerListChanged,
                        );

                        TableResponse::Success
                    }
                    Err(e) => TableResponse::Error(format!("Transfer failed: {}", e)),
                }
            }
            Err(e) => TableResponse::Error(format!("Failed to leave game: {:?}", e)),
        }
    }

    /// Handle player action
    async fn handle_action(&mut self, user_id: i64, action: Action) -> TableResponse {
        // Get username from mapping
        let username = match self.user_mapping.get(&user_id) {
            Some(u) => u.clone(),
            None => {
                return TableResponse::Error("User not at table".to_string());
            }
        };

        // Verify user is actually a player (not just a spectator)
        if !self.state.contains_player(&username) {
            return TableResponse::Error(
                "You must be seated at the table to take actions".to_string(),
            );
        }

        // Validate it's the user's turn
        if let Some(next_username) = self.state.get_next_action_username() {
            if next_username != username {
                return TableResponse::Error("Not your turn".to_string());
            }
        } else {
            return TableResponse::Error("No actions allowed right now".to_string());
        }

        // Apply action to game state
        match self.state.take_action(&username, action) {
            Ok(_) => {
                // Notify all subscribers that state changed
                self.notify_state_change(super::messages::StateChangeNotification::StateChanged);
                TableResponse::Success
            }
            Err(e) => TableResponse::Error(format!("Invalid action: {:?}", e)),
        }
    }

    /// Get current table state
    async fn get_state(&self, user_id: Option<i64>) -> TableStateResponse {
        // Get game views
        let views = self.state.get_views();

        // Extract summary information
        let player_count = views.len();
        let mut pot_size = 0;
        let mut phase = "Lobby".to_string();
        let mut players = vec![];
        let mut waitlist_count = 0;
        let mut spectator_count = 0;

        // Get view for requesting user or any view if no user specified
        if let Some(uid) = user_id
            && let Some(username) = self.user_mapping.get(&uid)
            && let Some(view) = views.get(username)
        {
            pot_size = view.pot.size as i64;
            phase = if player_count > 0 {
                "Playing".to_string()
            } else {
                "Lobby".to_string()
            };
            players = view
                .players
                .iter()
                .map(|p| p.user.name.to_string())
                .collect();
            waitlist_count = view.waitlist.len();
            spectator_count = view.spectators.len();
        } else if let Some((_, view)) = views.iter().next() {
            pot_size = view.pot.size as i64;
            phase = if player_count > 0 {
                "Playing".to_string()
            } else {
                "Lobby".to_string()
            };
            players = view
                .players
                .iter()
                .map(|p| p.user.name.to_string())
                .collect();
            waitlist_count = view.waitlist.len();
            spectator_count = view.spectators.len();
        }

        TableStateResponse {
            table_id: self.id,
            table_name: self.config.name.clone(),
            player_count,
            max_players: self.config.max_players,
            waitlist_count,
            spectator_count,
            small_blind: self.config.small_blind,
            big_blind: self.config.big_blind,
            pot_size,
            is_active: !self.is_paused,
            phase,
            players,
            is_private: self.config.is_private,
            speed: self.config.speed.to_string(),
        }
    }

    /// Get game view for a specific user
    fn get_game_view(&self, user_id: i64) -> Option<GameView> {
        // Get username from user_id
        let username = self.user_mapping.get(&user_id)?;

        // Get all views
        let views = self.state.get_views();

        // Return view for this user (Arc clones are cheap)
        views.get(username).map(|view| GameView {
            blinds: view.blinds.clone(),
            spectators: view.spectators.clone(),
            waitlist: view.waitlist.clone(),
            open_seats: view.open_seats.clone(),
            players: view.players.clone(),
            board: view.board.clone(),
            pot: view.pot.clone(),
            play_positions: view.play_positions.clone(),
        })
    }

    /// Handle spectate request
    async fn handle_spectate(&mut self, user_id: i64, username: String) -> TableResponse {
        let poker_username: Username = username.into();

        match self.state.spectate_user(&poker_username) {
            Ok(_) => {
                // Store mapping
                self.user_mapping.insert(user_id, poker_username.clone());
                self.username_mapping.insert(poker_username, user_id);

                // Notify subscribers that player list changed (spectator added)
                self.notify_state_change(
                    super::messages::StateChangeNotification::PlayerListChanged,
                );

                TableResponse::Success
            }
            Err(e) => TableResponse::Error(format!("Failed to spectate: {:?}", e)),
        }
    }

    /// Handle stop spectating request
    async fn handle_stop_spectating(&mut self, user_id: i64) -> TableResponse {
        // Get username from mapping
        let username = match self.user_mapping.get(&user_id) {
            Some(u) => u.clone(),
            None => {
                return TableResponse::Error("User not spectating".to_string());
            }
        };

        // Remove spectator
        match self.state.remove_user(&username) {
            Ok(_) => {
                // Remove mappings
                self.user_mapping.remove(&user_id);
                self.username_mapping.remove(&username);

                // Notify subscribers that player list changed (spectator removed)
                self.notify_state_change(
                    super::messages::StateChangeNotification::PlayerListChanged,
                );

                TableResponse::Success
            }
            Err(e) => TableResponse::Error(format!("Failed to stop spectating: {:?}", e)),
        }
    }

    /// Handle join waitlist request
    async fn handle_join_waitlist(&mut self, user_id: i64, username: String) -> TableResponse {
        let poker_username: Username = username.into();

        match self.state.waitlist_user(&poker_username) {
            Ok(_) => {
                // Store mapping
                self.user_mapping.insert(user_id, poker_username.clone());
                self.username_mapping.insert(poker_username, user_id);
                TableResponse::Success
            }
            Err(e) => TableResponse::Error(format!("Failed to join waitlist: {:?}", e)),
        }
    }

    /// Handle leave waitlist request
    async fn handle_leave_waitlist(&mut self, user_id: i64) -> TableResponse {
        // Get username from mapping
        let username = match self.user_mapping.get(&user_id) {
            Some(u) => u.clone(),
            None => {
                return TableResponse::Error("User not on waitlist".to_string());
            }
        };

        // Remove from waitlist
        match self.state.remove_user(&username) {
            Ok(_) => {
                // Remove mappings
                self.user_mapping.remove(&user_id);
                self.username_mapping.remove(&username);
                TableResponse::Success
            }
            Err(e) => TableResponse::Error(format!("Failed to leave waitlist: {:?}", e)),
        }
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

        // Get username
        let username = match self.user_mapping.get(&user_id) {
            Some(u) => u.clone(),
            None => {
                return TableResponse::Error("User not at table".to_string());
            }
        };

        // Transfer chips from wallet to escrow with collision-resistant idempotency key
        let idempotency_key = format!(
            "topup_{}_{}_{}",
            user_id,
            chrono::Utc::now().timestamp_millis(),
            Uuid::new_v4()
        );
        match self
            .wallet_manager
            .transfer_to_escrow(user_id, self.id, amount, idempotency_key)
            .await
        {
            Ok(_) => {
                // Update player stack in PokerState
                if let Err(e) = self.state.add_chips_to_player(&username, amount as u32) {
                    log::error!("Failed to add chips to player {}: {:?}", username, e);
                    return TableResponse::Error("Failed to update player stack".to_string());
                }

                self.top_up_tracker.insert(user_id, self.hand_count);
                TableResponse::Success
            }
            Err(e) => TableResponse::Error(format!("Transfer failed: {}", e)),
        }
    }

    /// Handle bot turns
    async fn handle_bot_turns(&mut self) {
        use crate::bot::decision::BotDecisionMaker;

        // Check if it's a bot's turn
        if let Some(next_username) = self.state.get_next_action_username() {
            // Check if this username is a bot (not in username_mapping means it's a bot)
            if !self.username_mapping.contains_key(&next_username) {
                // Look up bot player to get difficulty parameters
                if let Some(bot_player) = self
                    .bot_manager
                    .get_bot_by_username(next_username.as_str())
                    .await
                {
                    // Get action choices and game view
                    if let Some(action_choices) = self.state.get_action_choices() {
                        let views = self.state.get_views();
                        if let Some(bot_view) = views.get(&next_username) {
                            // Get bot's player view
                            let bot_player_view = bot_view
                                .players
                                .iter()
                                .find(|p| p.user.name == next_username);

                            if let Some(player_view) = bot_player_view {
                                let hole_cards = &player_view.cards;
                                let board_cards = bot_view.board.as_slice();
                                let pot_size = bot_view.pot.size;
                                let bot_chips = player_view.user.money;

                                // Get the actual amount the bot needs to call from the pot
                                let current_bet = self
                                    .state
                                    .get_call_amount_for_player(&next_username)
                                    .unwrap_or(0);

                                // Use decision maker with bot's difficulty parameters
                                let mut decision_maker = BotDecisionMaker::new();
                                let can_check = action_choices.contains(&Action::Check);

                                // Get position and player count from game views
                                let views = self.state.get_views();
                                let (position, players_in_hand) =
                                    if let Some((_, view)) = views.iter().next() {
                                        // Find bot's position among active players
                                        let bot_position = view
                                            .players
                                            .iter()
                                            .position(|p| p.user.name == next_username);
                                        let active_count = view.players.len();
                                        (bot_position, active_count)
                                    } else {
                                        (None, 2) // Default to heads-up if no view
                                    };

                                let ctx = crate::bot::decision::BotDecisionContext {
                                    hole_cards,
                                    board_cards,
                                    pot_size,
                                    current_bet,
                                    bot_chips,
                                    can_check,
                                    position,
                                    players_remaining: players_in_hand,
                                };
                                let action = decision_maker.decide_action(&bot_player, &ctx);

                                log::debug!(
                                    "Bot {} ({:?}) at position {:?} taking action: {:?}",
                                    next_username,
                                    bot_player.config.difficulty,
                                    position,
                                    action
                                );
                                let _ = self.state.take_action(&next_username, action);

                                // Notify subscribers that state changed after bot action
                                self.notify_state_change(
                                    super::messages::StateChangeNotification::StateChanged,
                                );
                            } else {
                                // Fallback: check or fold
                                self.take_fallback_action(&next_username, &action_choices)
                                    .await;
                            }
                        } else {
                            // Fallback: check or fold
                            self.take_fallback_action(&next_username, &action_choices)
                                .await;
                        }
                    }
                } else {
                    // Bot not found in manager, use simple fallback
                    if let Some(action_choices) = self.state.get_action_choices() {
                        self.take_fallback_action(&next_username, &action_choices)
                            .await;
                    }
                }
            }
        }
    }

    /// Take fallback action (check if possible, otherwise fold)
    async fn take_fallback_action(
        &mut self,
        username: &crate::entities::Username,
        action_choices: &crate::entities::ActionChoices,
    ) {
        let action = if action_choices.contains(&Action::Check) {
            Action::Check
        } else {
            Action::Fold
        };
        log::debug!("Bot {} taking fallback action: {:?}", username, action);
        let _ = self.state.take_action(username, action);
    }

    /// Advance game state (called periodically)
    async fn tick(&mut self) {
        if self.is_paused || self.is_closed {
            return;
        }

        // Track previous state to detect hand completion
        let prev_is_lobby = matches!(self.state, crate::game::PokerState::Lobby(_));

        // Advance poker state FSM (take ownership and replace)
        let state = std::mem::take(&mut self.state);
        self.state = state.step();

        // Check if hand completed by detecting transition TO Lobby state
        // This is more reliable than counting players, which can change mid-hand
        let curr_is_lobby = matches!(self.state, crate::game::PokerState::Lobby(_));

        // Hand completion = we were NOT in lobby, but now we ARE
        // This happens after BootPlayers -> Lobby transition at end of hand
        if !prev_is_lobby && curr_is_lobby {
            self.hand_count += 1;
            log::debug!("Table {} hand {} completed", self.id, self.hand_count);
        }

        // Notify subscribers that state changed after tick
        self.notify_state_change(super::messages::StateChangeNotification::StateChanged);

        // Process bot turns if needed
        self.handle_bot_turns().await;

        // Drain events (logging only for now)
        let events = self.state.drain_events();
        if !events.is_empty() {
            log::debug!("Table {} generated {} events", self.id, events.len());
        }
    }
}
