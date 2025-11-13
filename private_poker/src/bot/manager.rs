//! Bot manager for automatic bot spawning and management.

use super::models::{BotConfig, BotPlayer, BotTelemetry};
use crate::table::config::TableConfig;
use sqlx::PgPool;
use std::{collections::HashMap, sync::Arc};
use tokio::sync::RwLock;

/// Bot manager for a single table
pub struct BotManager {
    /// Table ID
    table_id: i64,

    /// Table configuration
    config: TableConfig,

    /// Active bots (bot_id -> BotPlayer)
    bots: Arc<RwLock<HashMap<i32, BotPlayer>>>,

    /// Next bot ID
    next_bot_id: i32,

    /// Database pool for telemetry
    db_pool: Arc<PgPool>,
}

impl BotManager {
    /// Create a new bot manager
    ///
    /// # Arguments
    ///
    /// * `table_id` - Table ID
    /// * `config` - Table configuration
    /// * `db_pool` - Database connection pool
    ///
    /// # Returns
    ///
    /// * `BotManager` - New bot manager instance
    pub fn new(table_id: i64, config: TableConfig, db_pool: Arc<PgPool>) -> Self {
        Self {
            table_id,
            config,
            bots: Arc::new(RwLock::new(HashMap::new())),
            next_bot_id: 1,
            db_pool,
        }
    }

    /// Adjust bot count to reach target player count
    ///
    /// # Arguments
    ///
    /// * `current_human_count` - Current number of human players
    ///
    /// # Returns
    ///
    /// * `Result<usize, String>` - Number of bots spawned/despawned
    pub async fn adjust_bot_count(&mut self, current_human_count: usize) -> Result<usize, String> {
        if !self.config.bots_enabled {
            return Ok(0);
        }

        let target_total = self.config.target_bot_count as usize;
        let current_bot_count = self.bots.read().await.len();
        let current_total = current_human_count + current_bot_count;

        // Check bot ratio caps for high stakes
        let stakes_tier = self.get_stakes_tier();
        if matches!(stakes_tier, "Mid" | "High" | "Nosebleed") {
            // Require at least 2 humans at higher stakes
            if current_human_count < 2 {
                // Despawn all bots if not enough humans
                let count = self.despawn_all_bots().await?;
                return Ok(count);
            }
        }

        if current_total < target_total {
            // Spawn bots
            let to_spawn = target_total - current_total;
            self.spawn_bots(to_spawn).await
        } else if current_total > target_total && current_bot_count > 0 {
            // Despawn bots
            let to_despawn = current_total - target_total;
            let actual_despawn = to_despawn.min(current_bot_count);
            self.despawn_bots(actual_despawn).await
        } else {
            Ok(0)
        }
    }

    /// Spawn new bots
    ///
    /// # Arguments
    ///
    /// * `count` - Number of bots to spawn
    ///
    /// # Returns
    ///
    /// * `Result<usize, String>` - Number of bots spawned
    pub async fn spawn_bots(&mut self, count: usize) -> Result<usize, String> {
        let mut bots = self.bots.write().await;
        let mut spawned = 0;

        for _ in 0..count {
            let bot_id = self.next_bot_id;
            self.next_bot_id += 1;

            let config = BotConfig {
                id: bot_id,
                name: self.generate_bot_name(bot_id),
                difficulty: self.config.bot_difficulty,
                table_id: self.table_id,
                starting_chips: self.config.min_buy_in_chips(),
            };

            let bot = BotPlayer::new(config);
            bots.insert(bot_id, bot);
            spawned += 1;

            log::info!(
                "Spawned bot {} ({:?}) at table {}",
                bot_id,
                self.config.bot_difficulty,
                self.table_id
            );
        }

        Ok(spawned)
    }

    /// Despawn bots
    ///
    /// # Arguments
    ///
    /// * `count` - Number of bots to despawn
    ///
    /// # Returns
    ///
    /// * `Result<usize, String>` - Number of bots despawned
    pub async fn despawn_bots(&mut self, count: usize) -> Result<usize, String> {
        let mut bots = self.bots.write().await;
        let bot_ids: Vec<i32> = bots.keys().take(count).copied().collect();

        let mut despawned = 0;
        for bot_id in bot_ids {
            if let Some(bot) = bots.remove(&bot_id) {
                // Save telemetry before despawning
                if let Err(e) = self.save_telemetry(&bot).await {
                    log::error!("Failed to save telemetry for bot {}: {}", bot_id, e);
                }

                log::info!("Despawned bot {} from table {}", bot_id, self.table_id);
                despawned += 1;
            }
        }

        Ok(despawned)
    }

    /// Despawn all bots
    async fn despawn_all_bots(&mut self) -> Result<usize, String> {
        let count = self.bots.read().await.len();
        self.despawn_bots(count).await
    }

    /// Get a bot player
    ///
    /// # Arguments
    ///
    /// * `bot_id` - Bot ID
    ///
    /// # Returns
    ///
    /// * `Option<BotPlayer>` - Bot player if found
    pub async fn get_bot(&self, bot_id: i32) -> Option<BotPlayer> {
        let bots = self.bots.read().await;
        bots.get(&bot_id).cloned()
    }

    /// Get all active bot IDs
    pub async fn get_bot_ids(&self) -> Vec<i32> {
        let bots = self.bots.read().await;
        bots.keys().copied().collect()
    }

    /// Get current bot count
    pub async fn bot_count(&self) -> usize {
        self.bots.read().await.len()
    }

    /// Get a bot player by username
    ///
    /// # Arguments
    ///
    /// * `username` - Bot username (e.g., "PokerPro_1")
    ///
    /// # Returns
    ///
    /// * `Option<BotPlayer>` - Bot player if found
    pub async fn get_bot_by_username(&self, username: &str) -> Option<BotPlayer> {
        let bots = self.bots.read().await;
        bots.values()
            .find(|bot| bot.config.name == username)
            .cloned()
    }

    /// Update bot statistics
    ///
    /// # Arguments
    ///
    /// * `bot_id` - Bot ID
    /// * `updater` - Function to update bot stats
    pub async fn update_bot_stats<F>(&self, bot_id: i32, updater: F) -> Result<(), String>
    where
        F: FnOnce(&mut BotPlayer),
    {
        let mut bots = self.bots.write().await;
        if let Some(bot) = bots.get_mut(&bot_id) {
            updater(bot);
            Ok(())
        } else {
            Err(format!("Bot {} not found", bot_id))
        }
    }

    /// Save bot telemetry to database
    async fn save_telemetry(&self, bot: &BotPlayer) -> Result<(), String> {
        let telemetry = BotTelemetry {
            bot_id: bot.config.id,
            table_id: bot.config.table_id,
            stakes_tier: self.get_stakes_tier().to_string(),
            difficulty: format!("{:?}", bot.config.difficulty),
            hands_played: bot.stats.hands_played as i32,
            win_rate: bot.stats.win_rate(),
            vpip: bot.stats.vpip(),
            pfr: bot.stats.pfr(),
            aggression_factor: bot.stats.aggression_factor(),
            showdown_rate: bot.stats.showdown_rate(),
            updated_at: chrono::Utc::now(),
        };

        sqlx::query(
            r#"
            INSERT INTO bot_telemetry (
                bot_id, table_id, stakes_tier, difficulty, hands_played,
                win_rate, vpip, pfr, aggression_factor, showdown_rate, updated_at
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
            "#,
        )
        .bind(telemetry.bot_id)
        .bind(telemetry.table_id)
        .bind(&telemetry.stakes_tier)
        .bind(&telemetry.difficulty)
        .bind(telemetry.hands_played)
        .bind(telemetry.win_rate)
        .bind(telemetry.vpip)
        .bind(telemetry.pfr)
        .bind(telemetry.aggression_factor)
        .bind(telemetry.showdown_rate)
        .bind(telemetry.updated_at.naive_utc())
        .execute(self.db_pool.as_ref())
        .await
        .map_err(|e| format!("Database error: {}", e))?;

        Ok(())
    }

    /// Get stakes tier based on big blind
    fn get_stakes_tier(&self) -> &'static str {
        let bb = self.config.big_blind;
        match bb {
            0..=10 => "Micro",
            11..=50 => "Low",
            51..=200 => "Mid",
            201..=1000 => "High",
            _ => "Nosebleed",
        }
    }

    /// Generate a bot name
    fn generate_bot_name(&self, bot_id: i32) -> String {
        let prefixes = [
            "Bot", "AI", "Chip", "Card", "Poker", "Stack", "River", "Flop", "Turn", "Dealer",
        ];
        let suffixes = [
            "Master", "Pro", "King", "Queen", "Ace", "Jack", "Shark", "Fish", "Whale", "Player",
        ];

        use rand::Rng;
        let mut rng = rand::rng();
        let prefix_idx = rng.random_range(0..prefixes.len());
        let suffix_idx = rng.random_range(0..suffixes.len());

        format!(
            "{}{}_{}",
            prefixes[prefix_idx], suffixes[suffix_idx], bot_id
        )
    }

    /// Check if telemetry shows anomalies
    ///
    /// # Returns
    ///
    /// * `Result<Vec<String>, String>` - List of anomalies detected
    pub async fn check_telemetry_anomalies(&self) -> Result<Vec<String>, String> {
        let mut anomalies = Vec::new();

        let bots = self.bots.read().await;
        for (bot_id, bot) in bots.iter() {
            // Check if bot stats deviate significantly from difficulty params
            let expected = &bot.params;
            let actual_vpip = bot.stats.vpip();
            let actual_aggr = bot.stats.aggression_factor();

            // VPIP deviation check (±15%)
            if (actual_vpip - expected.vpip).abs() > 0.15 {
                anomalies.push(format!(
                    "Bot {}: VPIP anomaly (expected {:.1}%, actual {:.1}%)",
                    bot_id,
                    expected.vpip * 100.0,
                    actual_vpip * 100.0
                ));
            }

            // Aggression factor deviation check (±0.5)
            if (actual_aggr - expected.aggression_factor).abs() > 0.5 {
                anomalies.push(format!(
                    "Bot {}: Aggression anomaly (expected {:.2}, actual {:.2})",
                    bot_id, expected.aggression_factor, actual_aggr
                ));
            }

            // Win rate check (should not be too high)
            let win_rate = bot.stats.win_rate();
            if win_rate > self.config.big_blind as f32 * 3.0 {
                anomalies.push(format!(
                    "Bot {}: Excessive win rate ({:.2} chips/hand)",
                    bot_id, win_rate
                ));
            }
        }

        Ok(anomalies)
    }
}
