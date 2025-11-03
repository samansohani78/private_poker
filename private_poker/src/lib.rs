//! # Private Poker
//!
//! A Texas Hold'em poker implementation using a type-safe finite state machine (FSM) design.
//!
//! This library provides a complete poker game engine with networking capabilities,
//! hand evaluation, and game state management. The core game is implemented as an FSM
//! using `enum_dispatch` for zero-cost trait dispatch.
//!
//! ## Architecture
//!
//! The game consists of 14 distinct phases (states), each representing a specific point
//! in the poker game lifecycle:
//!
//! - **Lobby**: Waiting for players to join
//! - **SeatPlayers**: Assigning table positions
//! - **MoveButton**: Rotating the dealer button
//! - **CollectBlinds**: Collecting small and big blinds
//! - **Deal**: Dealing hole cards to players
//! - **TakeAction**: Player betting rounds
//! - **Flop/Turn/River**: Dealing community cards
//! - **ShowHands**: Revealing cards at showdown
//! - **DistributePot**: Distributing winnings
//! - **RemovePlayers/BootPlayers**: Player management
//! - **UpdateBlinds**: Adjusting blind levels
//!
//! ## Core Modules
//!
//! - [`game`]: Game state machine, entities, and poker logic
//! - [`net`]: Networking components (server, client, message protocol)
//!
//! ## Example
//!
//! ```
//! use private_poker::PokerState;
//!
//! // Create a new game in the lobby state
//! let game = PokerState::new();
//! ```

/// Networking components for client-server communication.
pub mod net;
pub use net::{client::Client, messages, server, utils};

/// Core game logic, entities, and state machine.
pub mod game;
pub use game::{
    GameSettings, PokerState, UserError,
    constants::{self, DEFAULT_MAX_USERS, MAX_PLAYERS},
    entities::{self, DEFAULT_BUY_IN, DEFAULT_MIN_BIG_BLIND, DEFAULT_MIN_SMALL_BLIND},
    functional,
};
