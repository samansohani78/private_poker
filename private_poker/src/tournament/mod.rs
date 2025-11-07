//! Tournament module for Sit-n-Go and scheduled tournaments.
//!
//! This module provides tournament management functionality including:
//! - Tournament creation and configuration
//! - Player registration and buy-ins
//! - Blind level progression
//! - Prize pool calculation and distribution
//! - Player elimination tracking
//!
//! ## Example
//!
//! ```no_run
//! use private_poker::tournament::{TournamentManager, TournamentConfig};
//! use private_poker::db::Database;
//! use std::sync::Arc;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let db = Database::new(&Default::default()).await?;
//!     let tournament_mgr = TournamentManager::new(Arc::new(db.pool().clone()));
//!
//!     // Create a 9-player Sit-n-Go with 100 chip buy-in
//!     let config = TournamentConfig::sit_and_go(
//!         "Sunday Special".to_string(),
//!         9,
//!         100
//!     );
//!
//!     let tournament_id = tournament_mgr.create_tournament(config).await?;
//!     println!("Created tournament: {}", tournament_id);
//!
//!     Ok(())
//! }
//! ```

pub mod manager;
pub mod models;

pub use manager::{TournamentError, TournamentManager, TournamentResult};
pub use models::{
    BlindLevel, PrizeStructure, TournamentConfig, TournamentId, TournamentInfo,
    TournamentRegistration, TournamentState, TournamentType,
};
