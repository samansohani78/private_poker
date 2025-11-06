//! Wallet data models.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Table ID type
pub type TableId = i64;

/// Wallet model
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Wallet {
    pub user_id: i64,
    pub balance: i64,
    pub currency: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Table escrow model
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TableEscrow {
    pub table_id: TableId,
    pub balance: i64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Wallet entry model (double-entry ledger)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WalletEntry {
    pub id: i64,
    pub user_id: i64,
    pub table_id: Option<TableId>,
    pub amount: i64,
    pub balance_after: i64,
    pub direction: EntryDirection,
    pub entry_type: EntryType,
    pub idempotency_key: String,
    pub description: Option<String>,
    pub created_at: DateTime<Utc>,
}

/// Entry direction
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum EntryDirection {
    Debit,
    Credit,
}

impl std::fmt::Display for EntryDirection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EntryDirection::Debit => write!(f, "debit"),
            EntryDirection::Credit => write!(f, "credit"),
        }
    }
}

/// Entry type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EntryType {
    BuyIn,
    CashOut,
    Rake,
    Bonus,
    AdminAdjust,
    Transfer,
}

impl std::fmt::Display for EntryType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EntryType::BuyIn => write!(f, "buy_in"),
            EntryType::CashOut => write!(f, "cash_out"),
            EntryType::Rake => write!(f, "rake"),
            EntryType::Bonus => write!(f, "bonus"),
            EntryType::AdminAdjust => write!(f, "admin_adjust"),
            EntryType::Transfer => write!(f, "transfer"),
        }
    }
}

/// Faucet claim model
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FaucetClaim {
    pub id: i64,
    pub user_id: i64,
    pub amount: i64,
    pub claimed_at: DateTime<Utc>,
    pub next_claim_at: DateTime<Utc>,
}

/// Transfer request (chips from wallet to escrow or vice versa)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransferRequest {
    pub user_id: i64,
    pub table_id: TableId,
    pub amount: i64,
    pub idempotency_key: String,
    pub description: Option<String>,
}
