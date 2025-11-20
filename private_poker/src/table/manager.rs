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

    /// Cached player counts (avoids N+1 query on list_tables)
    player_count_cache: Arc<RwLock<HashMap<TableId, usize>>>,
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
            player_count_cache: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Load existing tables from database and spawn actors
    ///
    /// Queries the database for all active tables and spawns table actors for them.
    /// Updates next_table_id to be one more than the highest existing table ID.
    ///
    /// # Returns
    ///
    /// * `Result<usize, String>` - Number of tables loaded, or error
    pub async fn load_existing_tables(&self) -> Result<usize, String> {
        use crate::table::config::{BotDifficulty, TableSpeed};

        // Query all active tables from database
        let rows = sqlx::query(
            r#"
            SELECT id, name, max_players, small_blind, big_blind,
                   min_buy_in_bb, max_buy_in_bb, absolute_chip_cap, top_up_cooldown_hands,
                   speed, bots_enabled, target_bot_count, bot_difficulty,
                   is_private, passphrase_hash, invite_token, invite_expires_at
            FROM tables
            WHERE is_active = true
            ORDER BY id ASC
            "#,
        )
        .fetch_all(self.pool.as_ref())
        .await
        .map_err(|e| format!("Failed to load tables from database: {}", e))?;

        if rows.is_empty() {
            return Ok(0);
        }

        let mut max_id = 0i64;
        let mut loaded_count = 0;

        for row in rows {
            let table_id: i64 = row.get("id");
            max_id = max_id.max(table_id);

            // Parse table configuration from database row
            let speed_str: String = row.get("speed");
            let speed = match speed_str.as_str() {
                "turbo" => TableSpeed::Turbo,
                "hyper" => TableSpeed::Hyper,
                _ => TableSpeed::Normal,
            };

            let difficulty_str: String = row.get("bot_difficulty");
            let bot_difficulty = match difficulty_str.as_str() {
                "easy" => BotDifficulty::Easy,
                "tag" => BotDifficulty::Tag,
                _ => BotDifficulty::Standard,
            };

            let config = TableConfig {
                name: row.get("name"),
                max_players: row.get::<i32, _>("max_players") as usize,
                small_blind: row.get("small_blind"),
                big_blind: row.get("big_blind"),
                min_buy_in_bb: row.get::<i16, _>("min_buy_in_bb") as u8,
                max_buy_in_bb: row.get::<i16, _>("max_buy_in_bb") as u8,
                absolute_chip_cap: row.get("absolute_chip_cap"),
                top_up_cooldown_hands: row.get::<i16, _>("top_up_cooldown_hands") as u8,
                speed,
                bots_enabled: row.get("bots_enabled"),
                target_bot_count: row.get::<i16, _>("target_bot_count") as u8,
                bot_difficulty,
                is_private: row.get("is_private"),
                passphrase_hash: row.get("passphrase_hash"),
                invite_token: row.get("invite_token"),
                invite_expires_at: row
                    .get::<Option<chrono::NaiveDateTime>, _>("invite_expires_at")
                    .map(|dt| chrono::DateTime::from_naive_utc_and_offset(dt, chrono::Utc)),
            };

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

            // Initialize player count cache to 0 (will be updated by table state queries)
            let mut cache = self.player_count_cache.write().await;
            cache.insert(table_id, 0);
            drop(cache);

            // Spawn actor task
            tokio::spawn(async move {
                actor.run().await;
            });

            log::info!("Loaded and spawned existing table {}", table_id);
            loaded_count += 1;
        }

        // Update next_table_id to be one more than the highest existing ID
        let mut next_id = self.next_table_id.write().await;
        *next_id = max_id + 1;
        drop(next_id);

        Ok(loaded_count)
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

        // Initialize player count cache to 0
        let mut cache = self.player_count_cache.write().await;
        cache.insert(table_id, 0);
        drop(cache);

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

        // Read player count cache once (avoids N+1 query problem)
        let cache = self.player_count_cache.read().await;

        let mut metadata_list = Vec::new();

        for row in rows {
            let table_id: i64 = row.get("id");

            // Get player count from cache (O(1) lookup vs N async message calls)
            let player_count = cache.get(&table_id).copied().unwrap_or(0);

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

        // Remove from player count cache
        let mut cache = self.player_count_cache.write().await;
        cache.remove(&table_id);
        drop(cache);

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

        let response = rx.await
            .map_err(|_| "Failed to receive response".to_string())?;

        // Update cache on successful join
        if response.is_success()
            && let Ok(state) = self.get_table_state(table_id, None).await {
                self.update_player_count_cache(table_id, state.player_count).await;
            }

        Ok(response)
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

        let response = rx.await
            .map_err(|_| "Failed to receive response".to_string())?;

        // Update cache on successful leave
        if response.is_success()
            && let Ok(state) = self.get_table_state(table_id, None).await {
                self.update_player_count_cache(table_id, state.player_count).await;
            }

        Ok(response)
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

    /// Update player count cache for a table
    ///
    /// This should be called by table actors when player count changes
    /// (on join, leave, or state updates) to keep the cache fresh.
    ///
    /// # Arguments
    ///
    /// * `table_id` - Table ID
    /// * `player_count` - Current player count
    pub async fn update_player_count_cache(&self, table_id: TableId, player_count: usize) {
        let mut cache = self.player_count_cache.write().await;
        cache.insert(table_id, player_count);
    }
}
