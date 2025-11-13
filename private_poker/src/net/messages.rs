use serde::{Deserialize, Serialize};
use std::fmt;

use super::super::game::{
    GameEvent, UserError,
    entities::{Action, ActionChoices, GameView, Username, Vote},
};

// Import types from other modules
use crate::auth::{SessionTokens, User};
use crate::table::{TableConfig, TableSpeed};
use crate::wallet::WalletEntry;
use chrono::{DateTime, Utc};

/// Errors due to the poker client's interaction with the poker server
/// and not from the user's particular action.
#[derive(Debug, Deserialize, Eq, thiserror::Error, PartialEq, Serialize)]
pub enum ClientError {
    #[error("already associated")]
    AlreadyAssociated,
    #[error("does not exist")]
    DoesNotExist,
    #[error("expired")]
    Expired,
    #[error("unassociated")]
    Unassociated,
}

/// Table ID type
pub type TableId = i64;

/// Stakes tier classification based on big blind size
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum StakesTier {
    /// BB ≤ 10
    Micro,
    /// BB 10-100
    Low,
    /// BB 100-1000
    Mid,
    /// BB > 1000
    High,
}

impl StakesTier {
    /// Determine stakes tier from big blind amount
    pub fn from_big_blind(bb: i64) -> Self {
        match bb {
            0..=10 => StakesTier::Micro,
            11..=100 => StakesTier::Low,
            101..=1000 => StakesTier::Mid,
            _ => StakesTier::High,
        }
    }
}

/// Table filter criteria for discovery
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TableFilter {
    pub stakes_tier: Option<StakesTier>,
    pub min_players: Option<usize>,
    pub max_players: Option<usize>,
    pub has_waitlist_space: bool,
    pub speed: Option<TableSpeed>,
    pub bots_enabled: Option<bool>,
    pub is_private: bool,
}

/// Table information for discovery/listing
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TableInfo {
    pub id: TableId,
    pub name: String,
    pub config: TableConfig,
    pub player_count: usize,
    pub waitlist_count: usize,
    pub stakes_tier: StakesTier,
    pub is_private: bool,
    pub requires_passphrase: bool,
    pub has_invite: bool,
}

/// Type of user state change requests.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum UserState {
    Play,
    Spectate,
}

impl fmt::Display for UserState {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let repr = match self {
            Self::Play => "waitlister",
            Self::Spectate => "spectator",
        };
        write!(f, "{repr}")
    }
}

/// A user command.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum UserCommand {
    // === Legacy Commands (V1) ===
    /// The user wants to change their state (play or spectate).
    ChangeState(UserState),
    /// A new user wants to connect to the game.
    Connect,
    /// User disconnected. This is really just a
    /// friendly courtesy and doesn't need to be sent by
    /// clients.
    Disconnect,
    /// User wants to show their hand. Can only occur if they're
    /// a player and the game is in a state that allows hands to
    /// be shown.
    ShowHand,
    /// User wants to start the game. Can only start a game when
    /// there are 2+ potential players.
    StartGame,
    /// User wants to make a bet. Can only occur if they're a
    /// player and it's their turn.
    TakeAction(Action),
    /// User wants to cast a vote.
    CastVote(Vote),

    // === Authentication Commands (V2) ===
    /// Register a new account
    Register {
        username: String,
        password: String,
        email: Option<String>,
    },
    /// Login to existing account
    Login {
        username: String,
        password: String,
        device_fingerprint: String,
    },
    /// Refresh access token using refresh token
    RefreshToken {
        refresh_token: String,
        device_fingerprint: String,
    },
    /// Logout from current session
    Logout,

    // === 2FA Commands ===
    /// Enable two-factor authentication
    Enable2FA { secret: String, code: String },
    /// Verify 2FA code
    Verify2FA { code: String },

    // === Password Reset Commands ===
    /// Request password reset email
    RequestPasswordReset { email: String },
    /// Reset password with code
    ResetPassword {
        email: String,
        code: String,
        new_password: String,
    },

    // === Table Management Commands (V2) ===
    /// Create a new table
    CreateTable { config: TableConfig },
    /// List available tables with optional filter
    ListTables { filter: Option<TableFilter> },
    /// Join a table with buy-in
    JoinTable {
        table_id: TableId,
        buy_in: i64,
        passphrase: Option<String>,
    },
    /// Leave a table
    LeaveTable { table_id: TableId },
    /// Join table waitlist
    JoinWaitlist { table_id: TableId },
    /// Leave table waitlist
    LeaveWaitlist { table_id: TableId },
    /// Start spectating a table
    SpectateTable { table_id: TableId },
    /// Stop spectating
    StopSpectating { table_id: TableId },

    // === Wallet Commands (V2) ===
    /// Get current wallet balance
    GetBalance,
    /// Claim daily faucet
    ClaimFaucet,
    /// Get transaction history
    GetTransactionHistory { limit: usize, offset: usize },

    // === Chat Commands (V2) ===
    /// Send chat message to table
    SendChatMessage { table_id: TableId, message: String },
    /// Mute a user at table (owner/mod only)
    MuteUser { table_id: TableId, user_id: i64 },
    /// Kick a user from table (owner/mod only)
    KickUser { table_id: TableId, user_id: i64 },

    // === Multi-Table Game Commands (V2) ===
    /// Take action at specific table
    TakeActionAtTable { table_id: TableId, action: Action },
    /// Cast vote at specific table
    CastVoteAtTable { table_id: TableId, vote: Vote },
    /// Start game at specific table
    StartGameAtTable { table_id: TableId },
    /// Show hand at specific table
    ShowHandAtTable { table_id: TableId },

    // === Tournament Commands (V2) ===
    /// Create a new tournament
    CreateTournament {
        config: crate::tournament::TournamentConfig,
    },
    /// List tournaments
    ListTournaments {
        state_filter: Option<crate::tournament::TournamentState>,
    },
    /// Register for tournament
    RegisterTournament { tournament_id: i64 },
    /// Unregister from tournament
    UnregisterTournament { tournament_id: i64 },
    /// Get tournament info
    GetTournamentInfo { tournament_id: i64 },
    /// Get tournament standings
    GetTournamentStandings { tournament_id: i64 },
}

impl fmt::Display for UserCommand {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let repr = match &self {
            // Legacy commands
            Self::ChangeState(state) => format!("requested to join the {state}s"),
            Self::Connect => "connected".to_string(),
            Self::Disconnect => "disconnected".to_string(),
            Self::ShowHand => "showed their hand".to_string(),
            Self::StartGame => "started the game".to_string(),
            Self::TakeAction(action) => action.to_string(),
            Self::CastVote(vote) => format!("voted to {vote}"),

            // Auth commands
            Self::Register { username, .. } => format!("registered as {username}"),
            Self::Login { username, .. } => format!("logged in as {username}"),
            Self::RefreshToken { .. } => "refreshed token".to_string(),
            Self::Logout => "logged out".to_string(),

            // 2FA commands
            Self::Enable2FA { .. } => "enabled 2FA".to_string(),
            Self::Verify2FA { .. } => "verified 2FA code".to_string(),

            // Password reset
            Self::RequestPasswordReset { email } => format!("requested password reset for {email}"),
            Self::ResetPassword { email, .. } => format!("reset password for {email}"),

            // Table management
            Self::CreateTable { config } => format!("created table '{}'", config.name),
            Self::ListTables { .. } => "listed tables".to_string(),
            Self::JoinTable {
                table_id, buy_in, ..
            } => {
                format!("joined table {} with buy-in {}", table_id, buy_in)
            }
            Self::LeaveTable { table_id } => format!("left table {}", table_id),
            Self::JoinWaitlist { table_id } => format!("joined waitlist for table {}", table_id),
            Self::LeaveWaitlist { table_id } => format!("left waitlist for table {}", table_id),
            Self::SpectateTable { table_id } => format!("spectating table {}", table_id),
            Self::StopSpectating { table_id } => format!("stopped spectating table {}", table_id),

            // Wallet
            Self::GetBalance => "requested balance".to_string(),
            Self::ClaimFaucet => "claimed faucet".to_string(),
            Self::GetTransactionHistory { .. } => "requested transaction history".to_string(),

            // Chat
            Self::SendChatMessage { table_id, .. } => format!("sent chat to table {}", table_id),
            Self::MuteUser { user_id, .. } => format!("muted user {}", user_id),
            Self::KickUser { user_id, .. } => format!("kicked user {}", user_id),

            // Multi-table game commands
            Self::TakeActionAtTable { table_id, action } => {
                format!("took action {} at table {}", action, table_id)
            }
            Self::CastVoteAtTable { table_id, vote } => {
                format!("voted to {} at table {}", vote, table_id)
            }
            Self::StartGameAtTable { table_id } => format!("started game at table {}", table_id),
            Self::ShowHandAtTable { table_id } => format!("showed hand at table {}", table_id),

            // Tournament commands
            Self::CreateTournament { config } => format!("created tournament '{}'", config.name),
            Self::ListTournaments { .. } => "listed tournaments".to_string(),
            Self::RegisterTournament { tournament_id } => {
                format!("registered for tournament {}", tournament_id)
            }
            Self::UnregisterTournament { tournament_id } => {
                format!("unregistered from tournament {}", tournament_id)
            }
            Self::GetTournamentInfo { tournament_id } => {
                format!("requested tournament {} info", tournament_id)
            }
            Self::GetTournamentStandings { tournament_id } => {
                format!("requested tournament {} standings", tournament_id)
            }
        };
        write!(f, "{}", repr)
    }
}

/// A message from a poker client to the poker server, indicating some
/// type of user action or command request.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ClientMessage {
    /// User the message is from.
    pub username: Username,
    /// Action the user is taking.
    pub command: UserCommand,
}

impl fmt::Display for ClientMessage {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{} {}", self.username, self.command)
    }
}

/// A message from the poker server to a poker client.
#[derive(Debug, Deserialize, Serialize)]
pub enum ServerMessage {
    // === Legacy Messages (V1) ===
    /// An acknowledgement of a client message, signaling that the client's
    /// command was successfully processed by the game thread.
    Ack(ClientMessage),
    /// An indication that the poker client caused an error, resulting in
    /// the client's message not being processed correctly.
    ClientError(ClientError),
    /// An internal game event that can be shared with all clients.
    GameEvent(GameEvent),
    /// The game state as viewed from the client's perspective.
    GameView(GameView),
    /// The game state represented as a string.
    Status(String),
    /// A signal indicating that it's the user's turn.
    TurnSignal(ActionChoices),
    /// An indication that the poker client sent a message that was read
    /// properly, but the type of action that it relayed was invalid
    /// for the game state, resulting in a user error.
    UserError(UserError),

    // === Authentication Responses (V2) ===
    /// Registration successful
    RegisterSuccess { user_id: i64 },
    /// Login successful with session tokens
    LoginSuccess {
        session: SessionTokens,
        user: User,
        wallet_balance: i64,
    },
    /// Token refresh successful
    RefreshSuccess { session: SessionTokens },
    /// Logout successful
    LogoutSuccess,

    // === 2FA Responses ===
    /// 2FA is required for this account
    TwoFactorRequired,
    /// 2FA enabled successfully
    TwoFactorEnabled { backup_codes: Vec<String> },
    /// 2FA code verified successfully
    TwoFactorVerified,

    // === Password Reset Responses ===
    /// Password reset code sent to email
    PasswordResetCodeSent,
    /// Password reset successful
    PasswordResetSuccess,

    // === Table Responses (V2) ===
    /// Table created successfully
    TableCreated { table_id: TableId },
    /// List of tables matching filter
    TableList { tables: Vec<TableInfo> },
    /// Successfully joined table
    JoinedTable { table_id: TableId },
    /// Successfully left table
    LeftTable {
        table_id: TableId,
        chips_returned: i64,
    },
    /// Joined table waitlist
    JoinedWaitlist { table_id: TableId, position: usize },
    /// Left table waitlist
    LeftWaitlist { table_id: TableId },
    /// Now spectating table
    SpectatingTable { table_id: TableId },
    /// Stopped spectating table
    StoppedSpectating { table_id: TableId },

    // === Wallet Responses (V2) ===
    /// Current wallet balance
    Balance { amount: i64, currency: String },
    /// Faucet claimed successfully
    FaucetClaimed {
        amount: i64,
        next_claim: DateTime<Utc>,
    },
    /// Transaction history
    TransactionHistory { entries: Vec<WalletEntry> },

    // === Chat Messages (V2) ===
    /// Chat message from a user
    ChatMessage {
        table_id: TableId,
        user_id: i64,
        username: String,
        message: String,
        timestamp: DateTime<Utc>,
    },
    /// User was muted
    UserMuted { table_id: TableId, user_id: i64 },
    /// User was kicked
    UserKicked { table_id: TableId, user_id: i64 },

    // === Multi-Table Game Messages (V2) ===
    /// Game view for a specific table
    TableGameView { table_id: TableId, view: GameView },
    /// Turn signal for a specific table
    TableTurnSignal {
        table_id: TableId,
        action_choices: ActionChoices,
    },
    /// Game event at a specific table
    TableGameEvent { table_id: TableId, event: GameEvent },
    /// Status message for a specific table
    TableStatus { table_id: TableId, message: String },

    // === Tournament Messages (V2) ===
    /// Tournament created
    TournamentCreated { tournament_id: i64 },
    /// List of tournaments
    TournamentList {
        tournaments: Vec<crate::tournament::TournamentInfo>,
    },
    /// Tournament information
    TournamentInfo {
        info: crate::tournament::TournamentInfo,
    },
    /// Tournament standings
    TournamentStandings {
        tournament_id: i64,
        standings: Vec<crate::tournament::TournamentRegistration>,
    },
    /// Registered for tournament
    TournamentRegistered { tournament_id: i64 },
    /// Unregistered for tournament
    TournamentUnregistered { tournament_id: i64 },
    /// Tournament started
    TournamentStarted { tournament_id: i64 },
    /// Tournament finished
    TournamentFinished { tournament_id: i64, winner_id: i64 },
    /// Blind level increased
    BlindLevelIncreased { tournament_id: i64, new_level: u32 },
    /// Player eliminated from tournament
    PlayerEliminated {
        tournament_id: i64,
        user_id: i64,
        position: usize,
        prize: Option<i64>,
    },
    /// Tournament error
    TournamentError(String),

    // === Error Responses (V2) ===
    /// Authentication error
    AuthError(String),
    /// Wallet error
    WalletError(String),
    /// Table error
    TableError(String),
    /// Rate limit exceeded
    RateLimitError { retry_after: u64 },
}

impl fmt::Display for ServerMessage {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let repr = match &self {
            // Legacy messages
            Self::Ack(msg) => msg.to_string(),
            Self::ClientError(error) => error.to_string(),
            Self::GameEvent(event) => event.to_string(),
            Self::GameView(_) => "game view".to_string(),
            Self::Status(status) => status.to_string(),
            Self::TurnSignal(action_choices) => action_choices.to_string(),
            Self::UserError(error) => error.to_string(),

            // Auth responses
            Self::RegisterSuccess { user_id } => {
                format!("registration successful (user_id: {})", user_id)
            }
            Self::LoginSuccess { user, .. } => {
                format!("login successful (username: {})", user.username)
            }
            Self::RefreshSuccess { .. } => "token refreshed".to_string(),
            Self::LogoutSuccess => "logout successful".to_string(),

            // 2FA responses
            Self::TwoFactorRequired => "2FA required".to_string(),
            Self::TwoFactorEnabled { .. } => "2FA enabled".to_string(),
            Self::TwoFactorVerified => "2FA verified".to_string(),

            // Password reset
            Self::PasswordResetCodeSent => "password reset code sent".to_string(),
            Self::PasswordResetSuccess => "password reset successful".to_string(),

            // Table responses
            Self::TableCreated { table_id } => format!("table {} created", table_id),
            Self::TableList { tables } => format!("{} tables available", tables.len()),
            Self::JoinedTable { table_id } => format!("joined table {}", table_id),
            Self::LeftTable {
                table_id,
                chips_returned,
            } => {
                format!("left table {} with {} chips", table_id, chips_returned)
            }
            Self::JoinedWaitlist { table_id, position } => {
                format!(
                    "joined waitlist for table {} (position: {})",
                    table_id, position
                )
            }
            Self::LeftWaitlist { table_id } => format!("left waitlist for table {}", table_id),
            Self::SpectatingTable { table_id } => format!("spectating table {}", table_id),
            Self::StoppedSpectating { table_id } => {
                format!("stopped spectating table {}", table_id)
            }

            // Wallet responses
            Self::Balance { amount, currency } => format!("balance: {} {}", amount, currency),
            Self::FaucetClaimed { amount, .. } => format!("faucet claimed: {} chips", amount),
            Self::TransactionHistory { entries } => format!("{} transactions", entries.len()),

            // Chat messages
            Self::ChatMessage {
                username, message, ..
            } => format!("{}: {}", username, message),
            Self::UserMuted { user_id, .. } => format!("user {} muted", user_id),
            Self::UserKicked { user_id, .. } => format!("user {} kicked", user_id),

            // Multi-table game messages
            Self::TableGameView { table_id, .. } => format!("game view for table {}", table_id),
            Self::TableTurnSignal {
                table_id,
                action_choices,
            } => {
                format!("your turn at table {} ({})", table_id, action_choices)
            }
            Self::TableGameEvent { table_id, event } => {
                format!("table {}: {}", table_id, event)
            }
            Self::TableStatus { table_id, message } => {
                format!("table {}: {}", table_id, message)
            }

            // Tournament messages
            Self::TournamentCreated { tournament_id } => {
                format!("tournament {} created", tournament_id)
            }
            Self::TournamentList { tournaments } => {
                format!("{} tournaments available", tournaments.len())
            }
            Self::TournamentInfo { info } => {
                format!("tournament {}: {}", info.id, info.config.name)
            }
            Self::TournamentStandings {
                tournament_id,
                standings,
            } => {
                format!(
                    "tournament {} standings ({} players)",
                    tournament_id,
                    standings.len()
                )
            }
            Self::TournamentRegistered { tournament_id } => {
                format!("registered for tournament {}", tournament_id)
            }
            Self::TournamentUnregistered { tournament_id } => {
                format!("unregistered from tournament {}", tournament_id)
            }
            Self::TournamentStarted { tournament_id } => {
                format!("tournament {} started", tournament_id)
            }
            Self::TournamentFinished {
                tournament_id,
                winner_id,
            } => {
                format!(
                    "tournament {} finished, winner: {}",
                    tournament_id, winner_id
                )
            }
            Self::BlindLevelIncreased {
                tournament_id,
                new_level,
            } => {
                format!(
                    "tournament {}: blinds increased to level {}",
                    tournament_id, new_level
                )
            }
            Self::PlayerEliminated {
                tournament_id,
                user_id,
                position,
                prize,
            } => {
                if let Some(amount) = prize {
                    format!(
                        "tournament {}: player {} eliminated ({}{} place, won {})",
                        tournament_id,
                        user_id,
                        position,
                        match position {
                            1 => "st",
                            2 => "nd",
                            3 => "rd",
                            _ => "th",
                        },
                        amount
                    )
                } else {
                    format!(
                        "tournament {}: player {} eliminated ({}{})",
                        tournament_id,
                        user_id,
                        position,
                        match position {
                            1 => "st",
                            2 => "nd",
                            3 => "rd",
                            _ => "th",
                        }
                    )
                }
            }
            Self::TournamentError(error) => format!("tournament error: {}", error),

            // Error responses
            Self::AuthError(error) => format!("auth error: {}", error),
            Self::WalletError(error) => format!("wallet error: {}", error),
            Self::TableError(error) => format!("table error: {}", error),
            Self::RateLimitError { retry_after } => {
                format!("rate limited: retry after {} seconds", retry_after)
            }
        };
        write!(f, "{}", repr)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::entities::{Action, Vote};

    use bincode::config;
    use bincode::serde::{decode_from_slice, encode_to_vec};
    use serde::{Serialize, de::DeserializeOwned};

    // Helper functions to use bincode 2 + serde consistently
    fn serialize_value<T: Serialize>(value: &T) -> Vec<u8> {
        encode_to_vec(value, config::standard()).unwrap()
    }

    fn deserialize_value<T: DeserializeOwned>(bytes: &[u8]) -> T {
        decode_from_slice(bytes, config::standard()).unwrap().0
    }

    // === ClientError Tests ===

    #[test]
    fn test_client_error_already_associated() {
        let error = ClientError::AlreadyAssociated;
        assert_eq!(format!("{}", error), "already associated");
    }

    #[test]
    fn test_client_error_does_not_exist() {
        let error = ClientError::DoesNotExist;
        assert_eq!(format!("{}", error), "does not exist");
    }

    #[test]
    fn test_client_error_expired() {
        let error = ClientError::Expired;
        assert_eq!(format!("{}", error), "expired");
    }

    #[test]
    fn test_client_error_unassociated() {
        let error = ClientError::Unassociated;
        assert_eq!(format!("{}", error), "unassociated");
    }

    #[test]
    fn test_client_error_equality() {
        assert_eq!(
            ClientError::AlreadyAssociated,
            ClientError::AlreadyAssociated
        );
        assert_ne!(ClientError::AlreadyAssociated, ClientError::DoesNotExist);
    }

    #[test]
    fn test_client_error_serialization() {
        let error = ClientError::AlreadyAssociated;
        let serialized = serialize_value(&error);
        let deserialized: ClientError = deserialize_value(&serialized);
        assert_eq!(error, deserialized);
    }

    // === UserState Tests ===

    #[test]
    fn test_user_state_play_display() {
        let state = UserState::Play;
        assert_eq!(format!("{}", state), "waitlister");
    }

    #[test]
    fn test_user_state_spectate_display() {
        let state = UserState::Spectate;
        assert_eq!(format!("{}", state), "spectator");
    }

    #[test]
    fn test_user_state_equality() {
        assert_eq!(UserState::Play, UserState::Play);
        assert_ne!(UserState::Play, UserState::Spectate);
    }

    #[test]
    fn test_user_state_clone() {
        let state = UserState::Play;
        let cloned = state.clone();
        assert_eq!(state, cloned);
    }

    #[test]
    fn test_user_state_serialization() {
        let state = UserState::Spectate;
        let serialized = serialize_value(&state);
        let deserialized: UserState = deserialize_value(&serialized);
        assert_eq!(state, deserialized);
    }

    // === UserCommand Tests ===

    #[test]
    fn test_user_command_change_state_play() {
        let cmd = UserCommand::ChangeState(UserState::Play);
        assert!(format!("{}", cmd).contains("waitlister"));
    }

    #[test]
    fn test_user_command_change_state_spectate() {
        let cmd = UserCommand::ChangeState(UserState::Spectate);
        assert!(format!("{}", cmd).contains("spectator"));
    }

    #[test]
    fn test_user_command_connect() {
        let cmd = UserCommand::Connect;
        assert_eq!(format!("{}", cmd), "connected");
    }

    #[test]
    fn test_user_command_disconnect() {
        let cmd = UserCommand::Disconnect;
        assert_eq!(format!("{}", cmd), "disconnected");
    }

    #[test]
    fn test_user_command_show_hand() {
        let cmd = UserCommand::ShowHand;
        assert_eq!(format!("{}", cmd), "showed their hand");
    }

    #[test]
    fn test_user_command_start_game() {
        let cmd = UserCommand::StartGame;
        assert_eq!(format!("{}", cmd), "started the game");
    }

    #[test]
    fn test_user_command_take_action_fold() {
        let cmd = UserCommand::TakeAction(Action::Fold);
        assert!(format!("{}", cmd).contains("fold"));
    }

    #[test]
    fn test_user_command_take_action_call() {
        let cmd = UserCommand::TakeAction(Action::Call);
        assert!(format!("{}", cmd).contains("call"));
    }

    #[test]
    fn test_user_command_take_action_all_in() {
        let cmd = UserCommand::TakeAction(Action::AllIn);
        assert!(format!("{}", cmd).contains("all-in"));
    }

    #[test]
    fn test_user_command_cast_vote_kick() {
        let username = Username::new("player1");
        let cmd = UserCommand::CastVote(Vote::Kick(username));
        assert!(format!("{}", cmd).contains("voted to kick"));
    }

    #[test]
    fn test_user_command_cast_vote_reset() {
        let cmd = UserCommand::CastVote(Vote::Reset(None));
        assert!(format!("{}", cmd).contains("voted to reset"));
    }

    #[test]
    fn test_user_command_equality() {
        assert_eq!(UserCommand::Connect, UserCommand::Connect);
        assert_ne!(UserCommand::Connect, UserCommand::Disconnect);
    }

    #[test]
    fn test_user_command_clone() {
        let cmd = UserCommand::StartGame;
        let cloned = cmd.clone();
        assert_eq!(cmd, cloned);
    }

    #[test]
    fn test_user_command_serialization_connect() {
        let cmd = UserCommand::Connect;
        let serialized = serialize_value(&cmd);
        let deserialized: UserCommand = deserialize_value(&serialized);
        assert_eq!(cmd, deserialized);
    }

    #[test]
    fn test_user_command_serialization_take_action() {
        let cmd = UserCommand::TakeAction(Action::Raise(Some(100)));
        let serialized = serialize_value(&cmd);
        let deserialized: UserCommand = deserialize_value(&serialized);
        assert_eq!(cmd, deserialized);
    }

    // === ClientMessage Tests ===

    #[test]
    fn test_client_message_creation() {
        let username = Username::new("alice");
        let msg = ClientMessage {
            username: username.clone(),
            command: UserCommand::Connect,
        };
        assert_eq!(msg.username, username);
        assert_eq!(msg.command, UserCommand::Connect);
    }

    #[test]
    fn test_client_message_display() {
        let username = Username::new("bob");
        let msg = ClientMessage {
            username,
            command: UserCommand::StartGame,
        };
        let display = format!("{}", msg);
        assert!(display.contains("bob"));
        assert!(display.contains("started the game"));
    }

    #[test]
    fn test_client_message_clone() {
        let msg = ClientMessage {
            username: Username::new("charlie"),
            command: UserCommand::Disconnect,
        };
        let cloned = msg.clone();
        assert_eq!(msg.username, cloned.username);
        assert_eq!(msg.command, cloned.command);
    }

    #[test]
    fn test_client_message_serialization() {
        let msg = ClientMessage {
            username: Username::new("dave"),
            command: UserCommand::TakeAction(Action::Check),
        };
        let serialized = serialize_value(&msg);
        let deserialized: ClientMessage = deserialize_value(&serialized);
        assert_eq!(msg.username, deserialized.username);
        assert_eq!(msg.command, deserialized.command);
    }

    #[test]
    fn test_client_message_with_vote() {
        let msg = ClientMessage {
            username: Username::new("eve"),
            command: UserCommand::CastVote(Vote::Reset(Some(Username::new("frank")))),
        };
        let display = format!("{}", msg);
        assert!(display.contains("eve"));
        assert!(display.contains("voted to reset"));
    }

    // === ServerMessage Tests ===

    #[test]
    fn test_server_message_ack() {
        let client_msg = ClientMessage {
            username: Username::new("alice"),
            command: UserCommand::Connect,
        };
        let server_msg = ServerMessage::Ack(client_msg.clone());
        let display = format!("{}", server_msg);
        assert!(display.contains("alice"));
    }

    #[test]
    fn test_server_message_client_error() {
        let server_msg = ServerMessage::ClientError(ClientError::AlreadyAssociated);
        assert_eq!(format!("{}", server_msg), "already associated");
    }

    #[test]
    fn test_server_message_status() {
        let server_msg = ServerMessage::Status("Game in progress".to_string());
        assert_eq!(format!("{}", server_msg), "Game in progress");
    }

    #[test]
    fn test_server_message_user_error() {
        let server_msg = ServerMessage::UserError(UserError::NotEnoughPlayers);
        assert!(!format!("{}", server_msg).is_empty());
    }

    #[test]
    fn test_server_message_serialization_ack() {
        let client_msg = ClientMessage {
            username: Username::new("test"),
            command: UserCommand::Connect,
        };
        let server_msg = ServerMessage::Ack(client_msg);
        let serialized = serialize_value(&server_msg);
        let _deserialized: ServerMessage = deserialize_value(&serialized);
    }

    #[test]
    fn test_server_message_serialization_client_error() {
        let server_msg = ServerMessage::ClientError(ClientError::Expired);
        let serialized = serialize_value(&server_msg);
        let _deserialized: ServerMessage = deserialize_value(&serialized);
    }

    #[test]
    fn test_server_message_serialization_status() {
        let server_msg = ServerMessage::Status("test status".to_string());
        let serialized = serialize_value(&server_msg);
        let _deserialized: ServerMessage = deserialize_value(&serialized);
    }

    #[test]
    fn test_all_client_errors_unique() {
        let errors = [
            ClientError::AlreadyAssociated,
            ClientError::DoesNotExist,
            ClientError::Expired,
            ClientError::Unassociated,
        ];

        for i in 0..errors.len() {
            for j in (i + 1)..errors.len() {
                assert_ne!(errors[i], errors[j]);
            }
        }
    }

    #[test]
    fn test_user_command_variants() {
        let commands = [
            UserCommand::Connect,
            UserCommand::Disconnect,
            UserCommand::ShowHand,
            UserCommand::StartGame,
            UserCommand::ChangeState(UserState::Play),
            UserCommand::ChangeState(UserState::Spectate),
            UserCommand::TakeAction(Action::Fold),
            UserCommand::CastVote(Vote::Reset(None)),
        ];

        // Each should have different display strings
        for i in 0..commands.len() {
            for j in (i + 1)..commands.len() {
                if commands[i] != commands[j] {
                    assert_ne!(format!("{}", commands[i]), format!("{}", commands[j]));
                }
            }
        }
    }

    #[test]
    fn test_complex_client_message_serialization() {
        let msg = ClientMessage {
            username: Username::new("complex_user_123"),
            command: UserCommand::TakeAction(Action::Raise(Some(500))),
        };

        let serialized = serialize_value(&msg);
        let deserialized: ClientMessage = deserialize_value(&serialized);

        assert_eq!(msg.username, deserialized.username);
        assert_eq!(msg.command, deserialized.command);
    }

    // === Network Property Tests (Sprint 6 Stage 6) ===

    #[test]
    fn property_message_roundtrip_basic_commands() {
        let commands = vec![
            UserCommand::Connect,
            UserCommand::Disconnect,
            UserCommand::TakeAction(Action::Fold),
            UserCommand::TakeAction(Action::Check),
            UserCommand::TakeAction(Action::Call),
            UserCommand::TakeAction(Action::AllIn),
            UserCommand::TakeAction(Action::Raise(Some(100))),
            UserCommand::CastVote(Vote::Reset(None)),
            UserCommand::CastVote(Vote::Kick(Username::new("player"))),
        ];

        for command in commands {
            let msg = ClientMessage {
                username: Username::new("test"),
                command: command.clone(),
            };
            let serialized = serialize_value(&msg);
            let deserialized: ClientMessage = deserialize_value(&serialized);
            assert_eq!(msg.username, deserialized.username);
            assert_eq!(msg.command, deserialized.command);
        }
    }

    #[test]
    fn property_client_error_roundtrip() {
        let errors = vec![ClientError::AlreadyAssociated];

        for error in errors {
            let serialized = serialize_value(&error);
            let deserialized: ClientError = deserialize_value(&serialized);
            assert_eq!(error, deserialized);
        }
    }

    #[test]
    fn property_username_serialization_preserves_content() {
        let long_name = "a".repeat(100);
        let usernames = vec![
            "alice",
            "bob123",
            "player_with_underscores",
            "日本語",
            "emoji🎮",
            "",
            long_name.as_str(),
        ];

        for name in usernames {
            let username = Username::new(name);
            let serialized = serialize_value(&username);
            let deserialized: Username = deserialize_value(&serialized);
            assert_eq!(username, deserialized);
        }
    }

    #[test]
    fn property_action_serialization_bijective() {
        let actions = vec![
            Action::Fold,
            Action::Check,
            Action::Call,
            Action::AllIn,
            Action::Raise(Some(0)),
            Action::Raise(Some(u32::MAX)),
            Action::Raise(None),
        ];

        for action in actions {
            let ser1 = serialize_value(&action);
            let deser: Action = deserialize_value(&ser1);
            let ser2 = serialize_value(&deser);
            assert_eq!(ser1, ser2, "Serialization should be bijective");
        }
    }

    #[test]
    fn property_vote_serialization_deterministic() {
        let votes = vec![
            Vote::Reset(None),
            Vote::Reset(Some(Username::new("player1"))),
            Vote::Kick(Username::new("player2")),
        ];

        for vote in votes {
            let ser1 = serialize_value(&vote);
            let ser2 = serialize_value(&vote);
            assert_eq!(ser1, ser2, "Same vote should serialize identically");
        }
    }
}
