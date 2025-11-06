//! Wallet module providing chip management with double-entry ledger and escrow.
//!
//! This module implements:
//! - Double-entry ledger for all wallet transactions
//! - Table escrow system (chips locked during gameplay)
//! - Idempotency keys to prevent duplicate transactions
//! - ACID-compliant atomic transfers
//! - Daily faucet for demo/testing
//!
//! ## Example
//!
//! ```no_run
//! use private_poker::wallet::WalletManager;
//! use private_poker::db::Database;
//! use std::sync::Arc;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let db = Database::new(&Default::default()).await?;
//!     let wallet = WalletManager::new(Arc::new(db.pool().clone()));
//!
//!     // Transfer to escrow (buy-in)
//!     let new_balance = wallet
//!         .transfer_to_escrow(1, 101, 5000, "buy_in_unique_key".to_string())
//!         .await?;
//!     println!("New balance after buy-in: {}", new_balance);
//!
//!     // Claim daily faucet
//!     let claim = wallet.claim_faucet(1).await?;
//!     println!("Claimed {} chips from faucet", claim.amount);
//!
//!     Ok(())
//! }
//! ```

pub mod errors;
pub mod manager;
pub mod models;

pub use errors::{WalletError, WalletResult};
pub use manager::WalletManager;
pub use models::{
    EntryDirection, EntryType, FaucetClaim, TableEscrow, TableId, Wallet, WalletEntry,
};
