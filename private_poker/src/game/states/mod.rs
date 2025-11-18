//! Game state definitions for the poker FSM.
//!
//! Each state represents a specific phase of the poker game lifecycle.

use serde::{Deserialize, Serialize};

use crate::game::entities::ActionChoices;

/// Lobby state - waiting for players to join and game to start
#[derive(Debug)]
pub struct Lobby {
    pub(crate) start_game: bool,
}

impl Default for Lobby {
    fn default() -> Self {
        Self::new()
    }
}

impl Lobby {
    #[must_use]
    pub fn new() -> Self {
        Self { start_game: false }
    }
}

/// Seating players from waitlist into open seats
#[derive(Debug)]
pub struct SeatPlayers {}

/// Moving the button and blind positions
#[derive(Debug)]
pub struct MoveButton {}

/// Collecting blinds from players
#[derive(Debug)]
pub struct CollectBlinds {}

/// Dealing hole cards to players
#[derive(Debug)]
pub struct Deal {}

/// Waiting for player action (bet, check, fold, etc.)
#[derive(Clone, Debug)]
pub struct TakeAction {
    pub action_choices: Option<ActionChoices>,
}

/// Dealing the flop (3 community cards)
#[derive(Debug)]
pub struct Flop {}

/// Dealing the turn (4th community card)
#[derive(Debug)]
pub struct Turn {}

/// Dealing the river (5th community card)
#[derive(Debug)]
pub struct River {}

/// Showing player hands during showdown
#[derive(Clone, Debug)]
pub struct ShowHands {}

/// Distributing pot to winner(s)
#[derive(Debug)]
pub struct DistributePot {}

/// Removing players who left or were kicked
#[derive(Debug)]
pub struct RemovePlayers {}

/// Updating blind levels (for tournaments)
#[derive(Debug)]
pub struct UpdateBlinds {}

/// Removing players who can't afford the big blind
#[derive(Debug)]
pub struct BootPlayers {}
