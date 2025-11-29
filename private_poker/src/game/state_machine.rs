//! Poker game state machine implementation.
//!
//! This module contains the core FSM logic that was previously in game.rs.
//! It provides the state management, user management traits, and game data structures.

use enum_dispatch::enum_dispatch;
use log::error;
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, HashSet, VecDeque},
    fmt,
    sync::Arc,
};
use thiserror::Error;

use super::constants::{DEFAULT_MAX_USERS, MAX_PLAYERS};
use super::entities::{
    Bet, Blinds, Card, DEFAULT_BUY_IN, DEFAULT_MIN_BIG_BLIND, DEFAULT_MIN_SMALL_BLIND, Deck,
    GameViews, PlayPositions, Player, PlayerCounts, PlayerQueues, Pot, PotView, SeatIndex, Usd,
    User, Username, Vote,
};

/// Errors that can occur during user operations
#[derive(Debug, Deserialize, Eq, Error, PartialEq, Serialize)]
pub enum UserError {
    #[error("can't show hand")]
    CannotShowHand,
    #[error("can't start unless you're waitlisted or a player")]
    CannotStartGame,
    #[error("can't vote on yourself")]
    CannotVoteOnSelf,
    #[error("game is full")]
    CapacityReached,
    #[error("game already in progress")]
    GameAlreadyInProgress,
    #[error("game already starting")]
    GameAlreadyStarting,
    #[error("need >= ${big_blind} for the big blind")]
    InsufficientFunds { big_blind: Usd },
    #[error("invalid action")]
    InvalidAction,
    #[error("illegal {bet}")]
    InvalidBet { bet: Bet },
    #[error("need 2+ players")]
    NotEnoughPlayers,
    #[error("not your turn")]
    OutOfTurnAction,
    #[error("user already exists")]
    UserAlreadyExists,
    #[error("user does not exist")]
    UserDoesNotExist,
    #[error("not playing")]
    UserNotPlaying,
    #[error("already showing hand")]
    UserAlreadyShowingHand,
    #[error("invalid game state: waitlist index {0} out of bounds")]
    InvalidWaitlistIndex(usize),
    #[error("invalid game state: player index {0} out of bounds")]
    InvalidPlayerIndex(usize),
    #[error("invalid game state: position not found")]
    InvalidPosition,
    #[error("invalid game state: seat not found")]
    InvalidSeat,
    #[error("invalid game state: internal consistency error")]
    InternalStateError,
}

/// Events that occur during gameplay
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub enum GameEvent {
    KickQueue(Username),
    Kicked(Username),
    RemoveQueue(Username),
    Removed(Username),
    SpectateQueue(Username),
    Spectated(Username),
    Waitlisted(Username),
    ResetUserMoneyQueue(Username),
    ResetUserMoney(Username),
    ResetAllMoneyQueue,
    ResetAllMoney,
    PassedVote(Vote),
    SplitPot(Username, Usd),
    JoinedTable(Username),
}

impl fmt::Display for GameEvent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let repr = match self {
            Self::KickQueue(username) => {
                format!("{username} will be kicked after the game")
            }
            Self::Kicked(username) => format!("{username} kicked from the game"),
            Self::RemoveQueue(username) => {
                format!("{username} will be removed after the game")
            }
            Self::Removed(username) => format!("{username} removed from the game"),
            Self::SpectateQueue(username) => {
                format!("{username} will move to spectate after the game")
            }
            Self::Spectated(username) => format!("{username} moved to spectate"),
            Self::Waitlisted(username) => format!("{username} waitlisted"),
            Self::ResetUserMoneyQueue(username) => {
                format!("{username}'s money will be reset after the game")
            }
            Self::ResetUserMoney(username) => format!("reset {username}'s money"),
            Self::ResetAllMoneyQueue => "everyone's money will be reset after the game".to_string(),
            Self::ResetAllMoney => "reset everyone's money".to_string(),
            Self::PassedVote(vote) => format!("vote to {vote} passed"),
            Self::SplitPot(username, amount) => format!("{username} won ${amount}"),
            Self::JoinedTable(username) => format!("{username} joined the table"),
        };
        write!(f, "{repr}")
    }
}

/// Game configuration settings
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct GameSettings {
    pub buy_in: Usd,
    pub min_small_blind: Usd,
    pub min_big_blind: Usd,
    pub max_users: usize,
    pub max_players: usize,
}

impl Default for GameSettings {
    fn default() -> Self {
        Self::new(
            DEFAULT_BUY_IN,
            DEFAULT_MIN_SMALL_BLIND,
            DEFAULT_MIN_BIG_BLIND,
            DEFAULT_MAX_USERS,
            MAX_PLAYERS,
        )
    }
}

impl GameSettings {
    #[must_use]
    pub const fn new(
        buy_in: Usd,
        min_small_blind: Usd,
        min_big_blind: Usd,
        max_users: usize,
        max_players: usize,
    ) -> Self {
        Self {
            buy_in,
            min_small_blind,
            min_big_blind,
            max_users,
            max_players,
        }
    }
}

/// Mutable game data shared across all states
#[derive(Debug)]
pub struct GameData {
    /// Deck of cards. This is instantiated once and reshuffled
    /// each deal.
    pub(super) deck: Deck,
    pub blinds: Blinds,
    pub spectators: HashSet<User>,
    pub waitlist: VecDeque<User>,
    pub open_seats: VecDeque<SeatIndex>,
    pub players: Vec<Player>,
    /// Community cards shared amongst all players.
    pub board: Vec<Card>,
    /// Mapping of running votes to users that are for those running votes.
    pub(super) votes: HashMap<Vote, HashSet<Username>>,
    pub(super) player_counts: PlayerCounts,
    pub pot: Pot,
    /// Queues of players to do things with at a later point of
    /// an active game.
    pub(super) player_queues: PlayerQueues,
    pub play_positions: PlayPositions,
    /// Stack of game events that give more insight as to what kind
    /// of game updates occur due to user actions or game state
    /// changes.
    pub(super) events: VecDeque<GameEvent>,
    /// Mapping of username to money they had when they were
    /// last connected to the game. This is updated when a
    /// user is removed/kicked from the game. When a user reconnects,
    /// the value in here is used as their starting money stack.
    pub(super) ledger: HashMap<Username, Usd>,
    /// If this is set, then users voted to reset everyone's
    /// money, but a game was in progress, so everyone's money
    /// will be reset after the game is over.
    pub(super) reset_all_money_after_game: bool,
    pub(super) settings: GameSettings,
}

impl Default for GameData {
    fn default() -> Self {
        let settings = GameSettings::default();
        settings.into()
    }
}

impl GameData {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
}

impl From<GameSettings> for GameData {
    fn from(value: GameSettings) -> Self {
        Self {
            deck: Deck::default(),
            blinds: Blinds {
                small: value.min_small_blind,
                big: value.min_big_blind,
            },
            spectators: HashSet::with_capacity(value.max_users),
            waitlist: VecDeque::with_capacity(value.max_users),
            open_seats: VecDeque::from_iter(0..value.max_players),
            players: Vec::with_capacity(value.max_players),
            board: Vec::with_capacity(5),
            votes: HashMap::with_capacity(2 * value.max_users + 1),
            player_counts: PlayerCounts::default(),
            pot: Pot::new(value.max_players),
            player_queues: PlayerQueues::default(),
            play_positions: PlayPositions::default(),
            events: VecDeque::new(),
            ledger: HashMap::with_capacity(value.max_users),
            reset_all_money_after_game: false,
            settings: value,
        }
    }
}

/// Trait for managing game state (views, events)
#[enum_dispatch]
pub trait GameStateManagement {
    fn drain_events(&mut self) -> VecDeque<GameEvent>;

    /// Get game views for all players
    ///
    /// # Important
    /// This function's return value should be used - ignoring it wastes computation
    #[must_use]
    fn get_views(&self) -> GameViews;
}

/// Trait for user management operations that depend on game phase
#[enum_dispatch]
pub trait PhaseDependentUserManagement {
    fn kick_user(&mut self, username: &Username) -> Result<Option<bool>, UserError>;
    fn remove_user(&mut self, username: &Username) -> Result<Option<bool>, UserError>;
    fn reset_all_money(&mut self) -> bool;
    fn reset_user_money(&mut self, username: &Username) -> Result<Option<bool>, UserError>;
    fn spectate_user(&mut self, username: &Username) -> Result<Option<bool>, UserError>;
}

/// Trait for user management operations independent of game phase
#[enum_dispatch]
pub trait PhaseIndependentUserManagement {
    fn cast_vote(&mut self, username: &Username, vote: Vote) -> Result<Option<Vote>, UserError>;
    fn new_user(&mut self, username: &Username) -> Result<bool, UserError>;
    fn waitlist_user(&mut self, username: &Username) -> Result<Option<bool>, UserError>;
}

/// A poker game with data and logic for running a poker game end-to-end.
///
/// This struct wraps game data and the current state, providing the core
/// game loop functionality.
#[derive(Debug)]
pub struct Game<T> {
    pub data: GameData,
    pub state: T,
}

/// Shared read-only data that can be reused across all views
#[derive(Debug)]
pub struct SharedViewData {
    pub(super) blinds: Arc<Blinds>,
    pub(super) spectators: Arc<HashSet<User>>,
    pub(super) waitlist: Arc<VecDeque<User>>,
    pub(super) open_seats: Arc<VecDeque<usize>>,
    pub(super) board: Arc<Vec<Card>>,
    pub(super) pot: Arc<PotView>,
    pub(super) play_positions: Arc<PlayPositions>,
}
