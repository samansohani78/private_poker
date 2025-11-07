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
//! ```ignore
//! use private_poker::table::{TableActor, TableConfig};
//! use private_poker::wallet::WalletManager;
//! use std::sync::Arc;
//!
//! #[tokio::main]
//! async fn main() {
//!     // Create database pool (example)
//!     let pool = create_database_pool().await;
//!     let wallet_manager = Arc::new(WalletManager::new(Arc::new(pool)));
//!     let config = TableConfig::default();
//!
//!     let (actor, handle) = TableActor::new(1, config, wallet_manager);
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
