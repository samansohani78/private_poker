//! Bot module providing automatic bot players with difficulty presets.
//!
//! This module implements:
//! - BotPlayer: Individual bot with statistics tracking
//! - BotManager: Auto-spawn/despawn bots to reach target players
//! - Difficulty presets (Easy, Standard, TAG) with distinct play styles
//! - Human-like pacing with randomized delays
//! - Telemetry tracking and anomaly detection
//! - Bot ratio caps for high-stakes tables
//!
//! ## Difficulty Presets
//!
//! ### Easy (Loose-Passive)
//! - VPIP: 45% (plays many hands)
//! - PFR: 10% (rarely raises)
//! - Aggression: 0.5 (passive)
//! - Never bluffs
//!
//! ### Standard (Balanced TAG)
//! - VPIP: 30% (moderate range)
//! - PFR: 20% (raises decent hands)
//! - Aggression: 1.5 (moderately aggressive)
//! - Bluffs 15% of time
//!
//! ### TAG (Tight-Aggressive)
//! - VPIP: 20% (very tight)
//! - PFR: 18% (raises most hands played)
//! - Aggression: 2.5 (very aggressive)
//! - Bluffs 25% of time
//!
//! ## Example
//!
//! ```no_run
//! use private_poker::bot::BotManager;
//! use private_poker::table::TableConfig;
//! use private_poker::db::Database;
//! use std::sync::Arc;
//!
//! #[tokio::main]
//! async fn main() {
//!     // Example: Create database and bot manager
//!     # let config = private_poker::db::DatabaseConfig {
//!     #     database_url: "postgres://localhost/test".to_string(),
//!     #     max_connections: 5,
//!     #     min_connections: 1,
//!     #     connection_timeout_secs: 5,
//!     #     idle_timeout_secs: 300,
//!     #     max_lifetime_secs: 1800,
//!     # };
//!     # let db = Database::new(&config).await.unwrap();
//!     let db_pool = Arc::new(db.pool().clone());
//!     let table_config = TableConfig::default();
//!     let mut bot_manager = BotManager::new(1, table_config, db_pool);
//!
//!     // Auto-adjust bots based on human count
//!     let human_count = 3;
//!     bot_manager.adjust_bot_count(human_count).await.unwrap();
//!
//!     // Get bot count
//!     let bot_count = bot_manager.bot_count().await;
//!     println!("Active bots: {}", bot_count);
//! }
//! ```

pub mod decision;
pub mod manager;
pub mod models;

pub use decision::BotDecisionMaker;
pub use manager::BotManager;
pub use models::{BotConfig, BotId, BotPlayer, BotStats, BotTelemetry, DifficultyParams};
