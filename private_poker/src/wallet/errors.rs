//! Wallet error types.

use thiserror::Error;

/// Wallet errors
#[derive(Debug, Error)]
pub enum WalletError {
    /// Database error
    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),

    /// Insufficient balance
    #[error("Insufficient balance: available {available}, required {required}")]
    InsufficientBalance { available: i64, required: i64 },

    /// Wallet not found
    #[error("Wallet not found for user {0}")]
    WalletNotFound(i64),

    /// Escrow not found
    #[error("Escrow not found for table {0}")]
    EscrowNotFound(i64),

    /// Duplicate transaction (idempotency key already used)
    #[error("Duplicate transaction: {0}")]
    DuplicateTransaction(String),

    /// Invalid amount (must be positive)
    #[error("Invalid amount: {0}")]
    InvalidAmount(i64),

    /// Faucet claim not available
    #[error("Faucet claim not available until {0}")]
    FaucetNotAvailable(chrono::DateTime<chrono::Utc>),

    /// Currency mismatch
    #[error("Currency mismatch: expected {expected}, got {got}")]
    CurrencyMismatch { expected: String, got: String },

    /// Transaction failed
    #[error("Transaction failed: {0}")]
    TransactionFailed(String),
}

impl WalletError {
    /// Get a client-safe error message that doesn't leak sensitive information
    ///
    /// Database errors are sanitized to prevent information disclosure about
    /// the internal system structure, and user IDs/table IDs are redacted.
    pub fn client_message(&self) -> String {
        match self {
            // Sanitize database errors - don't expose SQL details
            WalletError::Database(_) => "Internal server error".to_string(),
            // Sanitize wallet not found - don't expose user IDs
            WalletError::WalletNotFound(_) => "Wallet not found".to_string(),
            // Sanitize escrow not found - don't expose table IDs
            WalletError::EscrowNotFound(_) => "Escrow not found".to_string(),
            // All other errors are safe to expose
            _ => self.to_string(),
        }
    }
}

/// Result type for wallet operations
pub type WalletResult<T> = Result<T, WalletError>;
