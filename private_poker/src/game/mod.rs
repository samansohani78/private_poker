//! Poker game engine - core FSM and game logic.
//!
//! This module provides the foundational poker game implementation including:
//! - Type-safe finite state machine with 14 game states
//! - User management (players, spectators, waitlist)
//! - Game flow and state transitions
//! - Event generation and views

// Submodules
pub mod constants;
pub mod entities;
pub mod functional;

// Temporary: include the full implementation until refactoring is complete
mod implementation;

// Re-export everything from implementation for backward compatibility
pub use implementation::*;
