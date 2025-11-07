//! Table manager for spawning and managing multiple table actors.

use super::{
    actor::{TableActor, TableHandle},
    config::TableConfig,
    messages::{TableMessage, TableResponse, TableStateResponse},
};
use crate::wallet::{TableId, WalletManager};
use sqlx::{PgPool, Row};
use std::{collections::HashMap, sync::Arc};
use tokio::sync::{RwLock, oneshot};

/// Table metadata for discovery
#[derive(Debug, Clone)]
pub struct TableMetadata {
    pub id: TableId,
    pub name: String,
    pub player_count: usize,
    pub max_players: usize,
    pub small_blind: i64,
    pub big_blind: i64,
    pub is_private: bool,
    pub speed: String,
    pub is_active: bool,
}

/// Table manager for managing multiple table instances
pub struct TableManager {
    /// Database connection pool
    pool: Arc<PgPool>,

    /// Wallet manager
    wallet_manager: Arc<WalletManager>,

    /// Active table handles
    tables: Arc<RwLock<HashMap<TableId, TableHandle>>>,

    /// Next table ID (for in-memory tables)
    next_table_id: Arc<RwLock<TableId>>,
}

impl TableManager {
    /// Create a new table manager
    ///
    /// # Arguments
    ///
    /// * `pool` - Database connection pool
    /// * `wallet_manager` - Wallet manager
    ///
    /// # Returns
    ///
    /// * `TableManager` - New table manager instance
    pub fn new(pool: Arc<PgPool>, wallet_manager: Arc<WalletManager>) -> Self {
        Self {
            pool,
            wallet_manager,
            tables: Arc::new(RwLock::new(HashMap::new())),
            next_table_id: Arc::new(RwLock::new(1)),
        }
    }

    /// Create and spawn a new table
    ///
    /// # Arguments
    ///
    /// * `config` - Table configuration
    /// * `creator_user_id` - User creating the table
    ///
    /// # Returns
    ///
    /// * `Result<TableId, String>` - Table ID or error
    pub async fn create_table(
        &self,
        config: TableConfig,
        creator_user_id: Option<i64>,
    ) -> Result<TableId, String> {
        // Validate configuration
        config.validate()?;

        // Get next table ID
        let mut next_id = self.next_table_id.write().await;
        let table_id = *next_id;
        *next_id += 1;
        drop(next_id);

        // Insert table into database
        sqlx::query(
            r#"
            INSERT INTO tables (
                id, name, max_players, small_blind, big_blind,
                min_buy_in_bb, max_buy_in_bb, absolute_chip_cap, top_up_cooldown_hands,
                speed, bots_enabled, target_bot_count, bot_difficulty,
                is_private, passphrase_hash, invite_token, invite_expires_at, creator_user_id
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18)
            "#,
        )
        .bind(table_id)
        .bind(&config.name)
        .bind(config.max_players as i32)
        .bind(config.small_blind)
        .bind(config.big_blind)
        .bind(config.min_buy_in_bb as i16)
        .bind(config.max_buy_in_bb as i16)
        .bind(config.absolute_chip_cap)
        .bind(config.top_up_cooldown_hands as i16)
        .bind(config.speed.to_string())
        .bind(config.bots_enabled)
        .bind(config.target_bot_count as i16)
        .bind(config.bot_difficulty.to_string())
        .bind(config.is_private)
        .bind(&config.passphrase_hash)
        .bind(&config.invite_token)
        .bind(config.invite_expires_at.map(|dt| dt.naive_utc()))
        .bind(creator_user_id)
        .execute(self.pool.as_ref())
        .await
        .map_err(|e| format!("Database error: {}", e))?;

        // Create table escrow entry
        sqlx::query("INSERT INTO table_escrows (table_id, balance) VALUES ($1, 0)")
            .bind(table_id)
            .execute(self.pool.as_ref())
            .await
            .map_err(|e| format!("Failed to create escrow: {}", e))?;

        // Create and spawn table actor
        let (actor, handle) = TableActor::new(
            table_id,
            config,
            self.wallet_manager.clone(),
            self.pool.clone(),
        );

        // Store handle
        let mut tables = self.tables.write().await;
        tables.insert(table_id, handle.clone());
        drop(tables);

        // Spawn actor task
        tokio::spawn(async move {
            actor.run().await;
        });

        log::info!("Created and spawned table {}", table_id);

        Ok(table_id)
    }

    /// Get a table handle
    ///
    /// # Arguments
    ///
    /// * `table_id` - Table ID
    ///
    /// # Returns
    ///
    /// * `Option<TableHandle>` - Table handle if found
    pub async fn get_table(&self, table_id: TableId) -> Option<TableHandle> {
        let tables = self.tables.read().await;
        tables.get(&table_id).cloned()
    }

    /// List all active tables
    ///
    /// # Returns
    ///
    /// * `Vec<TableMetadata>` - List of table metadata
    pub async fn list_tables(&self) -> Result<Vec<TableMetadata>, String> {
        let rows = sqlx::query(
            r#"
            SELECT id, name, max_players, small_blind, big_blind, is_private, speed, is_active
            FROM tables
            WHERE is_active = true
            ORDER BY created_at DESC
            "#,
        )
        .fetch_all(self.pool.as_ref())
        .await
        .map_err(|e| format!("Database error: {}", e))?;

        let mut metadata_list = Vec::new();

        for row in rows {
            let table_id: i64 = row.get("id");

            // Get current player count from table state
            let player_count = if let Some(handle) = self.get_table(table_id).await {
                let (tx, rx) = oneshot::channel();
                let _ = handle
                    .send(TableMessage::GetState {
                        user_id: None,
                        response: tx,
                    })
                    .await;

                if let Ok(state) = rx.await {
                    state.player_count
                } else {
                    0
                }
            } else {
                0
            };

            metadata_list.push(TableMetadata {
                id: table_id,
                name: row.get("name"),
                player_count,
                max_players: row.get::<i32, _>("max_players") as usize,
                small_blind: row.get("small_blind"),
                big_blind: row.get("big_blind"),
                is_private: row.get("is_private"),
                speed: row.get("speed"),
                is_active: row.get("is_active"),
            });
        }

        Ok(metadata_list)
    }

    /// Close a table
    ///
    /// # Arguments
    ///
    /// * `table_id` - Table ID
    ///
    /// # Returns
    ///
    /// * `Result<(), String>` - Success or error
    pub async fn close_table(&self, table_id: TableId) -> Result<(), String> {
        // Send close message to table
        if let Some(handle) = self.get_table(table_id).await {
            let (tx, rx) = oneshot::channel();
            handle
                .send(TableMessage::Close { response: tx })
                .await
                .map_err(|e| format!("Failed to send close message: {}", e))?;

            // Wait for response
            rx.await
                .map_err(|_| "Failed to receive response".to_string())?;
        }

        // Remove from active tables
        let mut tables = self.tables.write().await;
        tables.remove(&table_id);
        drop(tables);

        // Mark as inactive in database
        sqlx::query("UPDATE tables SET is_active = false WHERE id = $1")
            .bind(table_id)
            .execute(self.pool.as_ref())
            .await
            .map_err(|e| format!("Database error: {}", e))?;

        log::info!("Closed table {}", table_id);

        Ok(())
    }

    /// Join a table
    ///
    /// # Arguments
    ///
    /// * `table_id` - Table ID
    /// * `user_id` - User ID
    /// * `username` - Username
    /// * `buy_in_amount` - Buy-in amount in chips
    /// * `passphrase` - Optional passphrase for private tables
    ///
    /// # Returns
    ///
    /// * `Result<TableResponse, String>` - Response or error
    pub async fn join_table(
        &self,
        table_id: TableId,
        user_id: i64,
        username: String,
        buy_in_amount: i64,
        passphrase: Option<String>,
    ) -> Result<TableResponse, String> {
        let handle = self
            .get_table(table_id)
            .await
            .ok_or_else(|| "Table not found".to_string())?;

        let (tx, rx) = oneshot::channel();
        handle
            .send(TableMessage::JoinTable {
                user_id,
                username,
                buy_in_amount,
                passphrase,
                response: tx,
            })
            .await
            .map_err(|e| format!("Failed to send message: {}", e))?;

        rx.await
            .map_err(|_| "Failed to receive response".to_string())
    }

    /// Leave a table
    ///
    /// # Arguments
    ///
    /// * `table_id` - Table ID
    /// * `user_id` - User ID
    ///
    /// # Returns
    ///
    /// * `Result<TableResponse, String>` - Response or error
    pub async fn leave_table(
        &self,
        table_id: TableId,
        user_id: i64,
    ) -> Result<TableResponse, String> {
        let handle = self
            .get_table(table_id)
            .await
            .ok_or_else(|| "Table not found".to_string())?;

        let (tx, rx) = oneshot::channel();
        handle
            .send(TableMessage::LeaveTable {
                user_id,
                response: tx,
            })
            .await
            .map_err(|e| format!("Failed to send message: {}", e))?;

        rx.await
            .map_err(|_| "Failed to receive response".to_string())
    }

    /// Get table state
    ///
    /// # Arguments
    ///
    /// * `table_id` - Table ID
    /// * `user_id` - Optional user ID for personalized view
    ///
    /// # Returns
    ///
    /// * `Result<TableStateResponse, String>` - Table state or error
    pub async fn get_table_state(
        &self,
        table_id: TableId,
        user_id: Option<i64>,
    ) -> Result<TableStateResponse, String> {
        let handle = self
            .get_table(table_id)
            .await
            .ok_or_else(|| "Table not found".to_string())?;

        let (tx, rx) = oneshot::channel();
        handle
            .send(TableMessage::GetState {
                user_id,
                response: tx,
            })
            .await
            .map_err(|e| format!("Failed to send message: {}", e))?;

        rx.await
            .map_err(|_| "Failed to receive response".to_string())
    }

    /// Get active table count
    pub async fn active_table_count(&self) -> usize {
        let tables = self.tables.read().await;
        tables.len()
    }
}
