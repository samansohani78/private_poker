//! Wallet manager implementation with double-entry ledger and escrow.
#![allow(clippy::needless_raw_string_hashes)]

use super::{
    errors::{WalletError, WalletResult},
    models::{EntryDirection, EntryType, FaucetClaim, TableEscrow, TableId, Wallet, WalletEntry},
};
use chrono::{Duration, Utc};
use sqlx::{PgPool, Postgres, Row, Transaction};
use std::sync::Arc;

/// Wallet manager
#[derive(Clone)]
pub struct WalletManager {
    pool: Arc<PgPool>,
    #[allow(dead_code)]
    default_balance: i64,
    faucet_amount: i64,
    faucet_cooldown: Duration,
}

impl WalletManager {
    /// Create a new wallet manager
    ///
    /// # Arguments
    ///
    /// * `pool` - Database connection pool
    ///
    /// # Returns
    ///
    /// * `WalletManager` - New wallet manager instance
    pub fn new(pool: Arc<PgPool>) -> Self {
        let default_balance = std::env::var("DEFAULT_WALLET_BALANCE")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(10000);

        let faucet_amount = std::env::var("FAUCET_AMOUNT")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(1000);

        let faucet_cooldown_hours = std::env::var("FAUCET_COOLDOWN_HOURS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(24);

        Self {
            pool,
            default_balance,
            faucet_amount,
            faucet_cooldown: Duration::hours(faucet_cooldown_hours),
        }
    }

    /// Get wallet balance for a user
    ///
    /// # Arguments
    ///
    /// * `user_id` - User ID
    ///
    /// # Returns
    ///
    /// * `WalletResult<Wallet>` - Wallet information or error
    pub async fn get_wallet(&self, user_id: i64) -> WalletResult<Wallet> {
        let row = sqlx::query(
            r#"
            SELECT user_id, balance, currency, created_at, updated_at
            FROM wallets
            WHERE user_id = $1
            "#,
        )
        .bind(user_id)
        .fetch_optional(self.pool.as_ref())
        .await?
        .ok_or(WalletError::WalletNotFound(user_id))?;

        Ok(Wallet {
            user_id: row.get("user_id"),
            balance: row.get("balance"),
            currency: row.get("currency"),
            created_at: row.get::<chrono::NaiveDateTime, _>("created_at").and_utc(),
            updated_at: row.get::<chrono::NaiveDateTime, _>("updated_at").and_utc(),
        })
    }

    /// Get table escrow balance
    ///
    /// # Arguments
    ///
    /// * `table_id` - Table ID
    ///
    /// # Returns
    ///
    /// * `WalletResult<TableEscrow>` - Escrow information or error
    pub async fn get_escrow(&self, table_id: TableId) -> WalletResult<TableEscrow> {
        let row = sqlx::query(
            r#"
            SELECT table_id, balance, created_at, updated_at
            FROM table_escrows
            WHERE table_id = $1
            "#,
        )
        .bind(table_id)
        .fetch_optional(self.pool.as_ref())
        .await?
        .ok_or(WalletError::EscrowNotFound(table_id))?;

        Ok(TableEscrow {
            table_id: row.get("table_id"),
            balance: row.get("balance"),
            created_at: row.get::<chrono::NaiveDateTime, _>("created_at").and_utc(),
            updated_at: row.get::<chrono::NaiveDateTime, _>("updated_at").and_utc(),
        })
    }

    /// Transfer chips from user wallet to table escrow (buy-in)
    ///
    /// # Arguments
    ///
    /// * `user_id` - User ID
    /// * `table_id` - Table ID
    /// * `amount` - Amount to transfer
    /// * `idempotency_key` - Unique key to prevent duplicate transactions
    ///
    /// # Returns
    ///
    /// * `WalletResult<i64>` - New wallet balance or error
    ///
    /// # Errors
    ///
    /// * `WalletError::InsufficientBalance` - Not enough chips
    /// * `WalletError::DuplicateTransaction` - Idempotency key already used
    pub async fn transfer_to_escrow(
        &self,
        user_id: i64,
        table_id: TableId,
        amount: i64,
        idempotency_key: String,
    ) -> WalletResult<i64> {
        if amount <= 0 {
            return Err(WalletError::InvalidAmount(amount));
        }

        // Start transaction
        let mut tx = self.pool.begin().await?;

        // Check for duplicate transaction (idempotency)
        let existing = sqlx::query("SELECT id FROM wallet_entries WHERE idempotency_key = $1")
            .bind(&idempotency_key)
            .fetch_optional(&mut *tx)
            .await?;

        if existing.is_some() {
            return Err(WalletError::DuplicateTransaction(idempotency_key));
        }

        // Atomically debit wallet with balance check
        // This prevents race conditions by checking and updating in a single atomic operation
        let wallet_result = sqlx::query(
            "UPDATE wallets
             SET balance = balance - $1, updated_at = NOW()
             WHERE user_id = $2 AND balance >= $1
             RETURNING balance",
        )
        .bind(amount)
        .bind(user_id)
        .fetch_optional(&mut *tx)
        .await?;

        let new_balance: i64 = match wallet_result {
            Some(row) => row.get("balance"),
            None => {
                // Either wallet doesn't exist or insufficient balance
                // Check which case it is
                let check_wallet = sqlx::query("SELECT balance FROM wallets WHERE user_id = $1")
                    .bind(user_id)
                    .fetch_optional(&mut *tx)
                    .await?;

                match check_wallet {
                    Some(row) => {
                        let current_balance: i64 = row.get("balance");
                        return Err(WalletError::InsufficientBalance {
                            user_id,
                            available: current_balance,
                            required: amount,
                        });
                    }
                    None => return Err(WalletError::WalletNotFound(user_id)),
                }
            }
        };

        // Create debit entry
        self.create_entry(
            &mut tx,
            user_id,
            Some(table_id),
            -amount,
            new_balance,
            EntryDirection::Debit,
            EntryType::BuyIn,
            idempotency_key.clone(),
            Some(format!("Buy-in to table {table_id}")),
        )
        .await?;

        // Credit table escrow (create if doesn't exist)
        // Fixed: Use single atomic operation to insert/update and return NEW balance
        let escrow_row = sqlx::query(
            "INSERT INTO table_escrows (table_id, balance, updated_at)
             VALUES ($1, $2, NOW())
             ON CONFLICT (table_id)
             DO UPDATE SET
                balance = table_escrows.balance + EXCLUDED.balance,
                updated_at = NOW()
             RETURNING balance",
        )
        .bind(table_id)
        .bind(amount)
        .fetch_one(&mut *tx)
        .await?;

        let _new_escrow_balance: i64 = escrow_row.get("balance");

        // Commit transaction
        tx.commit().await?;

        Ok(new_balance)
    }

    /// Transfer chips from table escrow to user wallet (cash-out)
    ///
    /// # Arguments
    ///
    /// * `user_id` - User ID
    /// * `table_id` - Table ID
    /// * `amount` - Amount to transfer
    /// * `idempotency_key` - Unique key to prevent duplicate transactions
    ///
    /// # Returns
    ///
    /// * `WalletResult<i64>` - New wallet balance or error
    pub async fn transfer_from_escrow(
        &self,
        user_id: i64,
        table_id: TableId,
        amount: i64,
        idempotency_key: String,
    ) -> WalletResult<i64> {
        if amount <= 0 {
            return Err(WalletError::InvalidAmount(amount));
        }

        // Start transaction
        let mut tx = self.pool.begin().await?;

        // Check for duplicate transaction
        let existing = sqlx::query("SELECT id FROM wallet_entries WHERE idempotency_key = $1")
            .bind(&idempotency_key)
            .fetch_optional(&mut *tx)
            .await?;

        if existing.is_some() {
            return Err(WalletError::DuplicateTransaction(idempotency_key));
        }

        // Atomically debit escrow with balance check
        let escrow_result = sqlx::query(
            "UPDATE table_escrows
             SET balance = balance - $1, updated_at = NOW()
             WHERE table_id = $2 AND balance >= $1
             RETURNING balance",
        )
        .bind(amount)
        .bind(table_id)
        .fetch_optional(&mut *tx)
        .await?;

        let _new_escrow_balance: i64 = match escrow_result {
            Some(row) => row.get("balance"),
            None => {
                // Either escrow doesn't exist or insufficient balance
                let check_escrow =
                    sqlx::query("SELECT balance FROM table_escrows WHERE table_id = $1")
                        .bind(table_id)
                        .fetch_optional(&mut *tx)
                        .await?;

                match check_escrow {
                    Some(row) => {
                        let current_balance: i64 = row.get("balance");
                        return Err(WalletError::InsufficientBalance {
                            user_id,
                            available: current_balance,
                            required: amount,
                        });
                    }
                    None => return Err(WalletError::EscrowNotFound(table_id)),
                }
            }
        };

        // Get current balance first for overflow check
        let current_wallet = sqlx::query("SELECT balance FROM wallets WHERE user_id = $1 FOR UPDATE")
            .bind(user_id)
            .fetch_optional(&mut *tx)
            .await?
            .ok_or(WalletError::WalletNotFound(user_id))?;

        let current_balance: i64 = current_wallet.get("balance");

        // Check for overflow before updating
        let new_balance = current_balance
            .checked_add(amount)
            .ok_or(WalletError::BalanceOverflow)?;

        // Atomically credit user wallet
        sqlx::query(
            "UPDATE wallets
             SET balance = $1, updated_at = NOW()
             WHERE user_id = $2",
        )
        .bind(new_balance)
        .bind(user_id)
        .execute(&mut *tx)
        .await?;

        // Create credit entry
        self.create_entry(
            &mut tx,
            user_id,
            Some(table_id),
            amount,
            new_balance,
            EntryDirection::Credit,
            EntryType::CashOut,
            idempotency_key.clone(),
            Some(format!("Cash-out from table {table_id}")),
        )
        .await?;

        // Commit transaction
        tx.commit().await?;

        Ok(new_balance)
    }

    /// Claim daily faucet
    ///
    /// # Arguments
    ///
    /// * `user_id` - User ID
    ///
    /// # Returns
    ///
    /// * `WalletResult<FaucetClaim>` - Faucet claim information or error
    ///
    /// # Errors
    ///
    /// * `WalletError::FaucetNotAvailable` - Cooldown period not elapsed
    pub async fn claim_faucet(&self, user_id: i64) -> WalletResult<FaucetClaim> {
        // Start transaction
        let mut tx = self.pool.begin().await?;

        // Check last claim with row lock to prevent race conditions
        // This prevents two concurrent claims from both passing the cooldown check
        let last_claim = sqlx::query(
            "SELECT next_claim_at FROM faucet_claims
             WHERE user_id = $1
             ORDER BY claimed_at DESC
             LIMIT 1
             FOR UPDATE",
        )
        .bind(user_id)
        .fetch_optional(&mut *tx)
        .await?;

        if let Some(row) = last_claim {
            let next_claim_at = row
                .get::<chrono::NaiveDateTime, _>("next_claim_at")
                .and_utc();
            if Utc::now() < next_claim_at {
                return Err(WalletError::FaucetNotAvailable(next_claim_at));
            }
        }

        // Get current wallet balance (with row lock)
        let wallet_row = sqlx::query("SELECT balance FROM wallets WHERE user_id = $1 FOR UPDATE")
            .bind(user_id)
            .fetch_optional(&mut *tx)
            .await?
            .ok_or(WalletError::WalletNotFound(user_id))?;

        let current_balance: i64 = wallet_row.get("balance");

        // Credit wallet with faucet amount (with overflow protection)
        let new_balance = current_balance
            .checked_add(self.faucet_amount)
            .ok_or(WalletError::BalanceOverflow)?;
        sqlx::query("UPDATE wallets SET balance = $1, updated_at = NOW() WHERE user_id = $2")
            .bind(new_balance)
            .bind(user_id)
            .execute(&mut *tx)
            .await?;

        // Create credit entry with collision-resistant idempotency key
        // Using millisecond timestamp for better precision than second-level timestamp
        let idempotency_key = format!("faucet_{}_{}", user_id, Utc::now().timestamp_millis());
        self.create_entry(
            &mut tx,
            user_id,
            None,
            self.faucet_amount,
            new_balance,
            EntryDirection::Credit,
            EntryType::Bonus,
            idempotency_key,
            Some("Daily faucet claim".to_string()),
        )
        .await?;

        // Record faucet claim
        let claimed_at = Utc::now();
        let next_claim_at = claimed_at + self.faucet_cooldown;

        let claim_row = sqlx::query(
            r#"
            INSERT INTO faucet_claims (user_id, amount, claimed_at, next_claim_at)
            VALUES ($1, $2, $3, $4)
            RETURNING id, user_id, amount, claimed_at, next_claim_at
            "#,
        )
        .bind(user_id)
        .bind(self.faucet_amount)
        .bind(claimed_at.naive_utc())
        .bind(next_claim_at.naive_utc())
        .fetch_one(&mut *tx)
        .await?;

        // Commit transaction
        tx.commit().await?;

        Ok(FaucetClaim {
            id: claim_row.get("id"),
            user_id: claim_row.get("user_id"),
            amount: claim_row.get("amount"),
            claimed_at: claim_row
                .get::<chrono::NaiveDateTime, _>("claimed_at")
                .and_utc(),
            next_claim_at: claim_row
                .get::<chrono::NaiveDateTime, _>("next_claim_at")
                .and_utc(),
        })
    }

    /// Create a wallet entry (double-entry ledger)
    #[allow(clippy::too_many_arguments)]
    async fn create_entry(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        user_id: i64,
        table_id: Option<TableId>,
        amount: i64,
        balance_after: i64,
        direction: EntryDirection,
        entry_type: EntryType,
        idempotency_key: String,
        description: Option<String>,
    ) -> WalletResult<i64> {
        let row = sqlx::query(
            r#"
            INSERT INTO wallet_entries (user_id, table_id, amount, balance_after, direction, entry_type, idempotency_key, description)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
            RETURNING id
            "#,
        )
        .bind(user_id)
        .bind(table_id)
        .bind(amount)
        .bind(balance_after)
        .bind(direction.to_string())
        .bind(entry_type.to_string())
        .bind(idempotency_key)
        .bind(description)
        .fetch_one(&mut **tx)
        .await?;

        Ok(row.get("id"))
    }

    /// Get wallet entries for a user
    ///
    /// # Arguments
    ///
    /// * `user_id` - User ID
    /// * `limit` - Maximum number of entries to return
    ///
    /// # Returns
    ///
    /// * `WalletResult<Vec<WalletEntry>>` - List of wallet entries
    pub async fn get_entries(&self, user_id: i64, limit: i64) -> WalletResult<Vec<WalletEntry>> {
        let rows = sqlx::query(
            r#"
            SELECT id, user_id, table_id, amount, balance_after, direction, entry_type, idempotency_key, description, created_at
            FROM wallet_entries
            WHERE user_id = $1
            ORDER BY created_at DESC
            LIMIT $2
            "#,
        )
        .bind(user_id)
        .bind(limit)
        .fetch_all(self.pool.as_ref())
        .await?;

        let entries = rows
            .into_iter()
            .map(|row| WalletEntry {
                id: row.get("id"),
                user_id: row.get("user_id"),
                table_id: row.get("table_id"),
                amount: row.get("amount"),
                balance_after: row.get("balance_after"),
                direction: match row.get::<String, _>("direction").as_str() {
                    "debit" => EntryDirection::Debit,
                    "credit" => EntryDirection::Credit,
                    _ => EntryDirection::Credit,
                },
                entry_type: match row.get::<String, _>("entry_type").as_str() {
                    "buy_in" => EntryType::BuyIn,
                    "cash_out" => EntryType::CashOut,
                    "rake" => EntryType::Rake,
                    "bonus" => EntryType::Bonus,
                    "admin_adjust" => EntryType::AdminAdjust,
                    "transfer" => EntryType::Transfer,
                    _ => EntryType::Transfer,
                },
                idempotency_key: row.get("idempotency_key"),
                description: row.get("description"),
                created_at: row.get::<chrono::NaiveDateTime, _>("created_at").and_utc(),
            })
            .collect();

        Ok(entries)
    }
}
