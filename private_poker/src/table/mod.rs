//! Table module providing multi-table support with async actor model.
//!
//! This module implements:
//! - TableActor: Async actor managing a single poker table
//! - TableManager: Actor managing multiple table instances
//! - Message-based communication with tokio channels
//! - Table configuration and lifecycle management
//!
//! ## Architecture
//!
//! Each table runs in a separate Tokio task with an mpsc message inbox.
//! The TableManager spawns and manages TableActor instances, providing
//! table discovery, join/leave coordination, and resource cleanup.
//!
//! ## Example
//!
//! ```no_run
//! use private_poker::table::{TableActor, TableConfig};
//! use private_poker::wallet::WalletManager;
//! use private_poker::db::Database;
//! use std::sync::Arc;
//!
//! #[tokio::main]
//! async fn main() {
//!     // Example: Create database and wallet manager
//!     # let config = private_poker::db::DatabaseConfig {
//!     #     database_url: "postgres://localhost/test".to_string(),
//!     #     max_connections: 5,
//!     #     min_connections: 1,
//!     #     connection_timeout_secs: 5,
//!     #     idle_timeout_secs: 300,
//!     #     max_lifetime_secs: 1800,
//!     # };
//!     # let db = Database::new(&config).await.unwrap();
//!     let pool = Arc::new(db.pool().clone());
//!     let wallet_manager = Arc::new(WalletManager::new(pool.clone()));
//!     let table_config = TableConfig::default();
//!
//!     let (actor, handle) = TableActor::new(1, table_config, wallet_manager, pool);
//!
//!     // Spawn table actor
//!     tokio::spawn(actor.run());
//!
//!     // Use handle to send messages
//!     // handle.send(TableMessage::JoinTable { ... }).await;
//! }
//! ```

pub mod actor;
pub mod config;
pub mod manager;
pub mod messages;

pub use actor::{TableActor, TableHandle};
pub use config::{BotDifficulty, TableConfig, TableSpeed};
pub use manager::{TableManager, TableMetadata};
pub use messages::{TableMessage, TableResponse, TableStateResponse};
