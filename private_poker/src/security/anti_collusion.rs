//! Anti-collusion detection system with shadow flagging.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{PgPool, Row};
use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

/// Collusion flag severity levels
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FlagSeverity {
    Low,
    Medium,
    High,
}

impl std::fmt::Display for FlagSeverity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FlagSeverity::Low => write!(f, "low"),
            FlagSeverity::Medium => write!(f, "medium"),
            FlagSeverity::High => write!(f, "high"),
        }
    }
}

/// Collusion flag types
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FlagType {
    /// Same IP players at same table
    SameIpTable,

    /// Suspiciously high win rate against same IP
    WinRateAnomaly,

    /// Coordinated folding pattern
    CoordinatedFolding,

    /// Unusual chip transfers between players
    SuspiciousTransfers,

    /// Rapid seat changes to sit near target
    SeatManipulation,
}

impl std::fmt::Display for FlagType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FlagType::SameIpTable => write!(f, "same_ip_table"),
            FlagType::WinRateAnomaly => write!(f, "win_rate_anomaly"),
            FlagType::CoordinatedFolding => write!(f, "coordinated_folding"),
            FlagType::SuspiciousTransfers => write!(f, "suspicious_transfers"),
            FlagType::SeatManipulation => write!(f, "seat_manipulation"),
        }
    }
}

/// Collusion flag record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CollusionFlag {
    pub id: i64,
    pub user_id: i64,
    pub table_id: i64,
    pub flag_type: String,
    pub severity: String,
    pub details: serde_json::Value,
    pub created_at: DateTime<Utc>,
    pub reviewed: bool,
    pub reviewer_user_id: Option<i64>,
    pub reviewed_at: Option<DateTime<Utc>>,
}

/// Anti-collusion detector
pub struct AntiCollusionDetector {
    /// Database pool
    pool: Arc<PgPool>,

    /// IP tracking: user_id -> IP address
    user_ips: Arc<tokio::sync::RwLock<HashMap<i64, String>>>,

    /// Table player tracking: table_id -> set of user_ids
    table_players: Arc<tokio::sync::RwLock<HashMap<i64, HashSet<i64>>>>,
}

impl AntiCollusionDetector {
    /// Create a new anti-collusion detector
    ///
    /// # Arguments
    ///
    /// * `pool` - Database connection pool
    ///
    /// # Returns
    ///
    /// * `AntiCollusionDetector` - New detector instance
    pub fn new(pool: Arc<PgPool>) -> Self {
        Self {
            pool,
            user_ips: Arc::new(tokio::sync::RwLock::new(HashMap::new())),
            table_players: Arc::new(tokio::sync::RwLock::new(HashMap::new())),
        }
    }

    /// Register user IP address
    ///
    /// # Arguments
    ///
    /// * `user_id` - User ID
    /// * `ip_address` - IP address
    pub async fn register_user_ip(&self, user_id: i64, ip_address: String) {
        let mut ips = self.user_ips.write().await;
        ips.insert(user_id, ip_address);
    }

    /// Check for same-IP players at table
    ///
    /// # Arguments
    ///
    /// * `table_id` - Table ID
    /// * `user_id` - User trying to join
    ///
    /// # Returns
    ///
    /// * `Result<bool, String>` - Whether same-IP player detected
    pub async fn check_same_ip_at_table(
        &self,
        table_id: i64,
        user_id: i64,
    ) -> Result<bool, String> {
        let ips = self.user_ips.read().await;
        let user_ip = match ips.get(&user_id) {
            Some(ip) => ip.clone(),
            None => return Ok(false), // No IP registered, allow
        };
        let players = self.table_players.read().await;

        if let Some(player_ids) = players.get(&table_id) {
            for &other_user_id in player_ids {
                if other_user_id == user_id {
                    continue;
                }

                if let Some(other_ip) = ips.get(&other_user_id) {
                    if other_ip == &user_ip {
                        // Same IP detected - create shadow flag
                        let user_ip_owned = user_ip.clone();
                        drop(ips);
                        drop(players);

                        self.create_flag(
                            user_id,
                            table_id,
                            FlagType::SameIpTable,
                            FlagSeverity::Medium,
                            serde_json::json!({
                                "other_user_id": other_user_id,
                                "ip_address": user_ip_owned
                            }),
                        )
                        .await?;

                        return Ok(true);
                    }
                }
            }
        }

        Ok(false)
    }

    /// Add player to table tracking
    ///
    /// # Arguments
    ///
    /// * `table_id` - Table ID
    /// * `user_id` - User ID
    pub async fn add_player_to_table(&self, table_id: i64, user_id: i64) {
        let mut players = self.table_players.write().await;
        players.entry(table_id).or_insert_with(HashSet::new).insert(user_id);
    }

    /// Remove player from table tracking
    ///
    /// # Arguments
    ///
    /// * `table_id` - Table ID
    /// * `user_id` - User ID
    pub async fn remove_player_from_table(&self, table_id: i64, user_id: i64) {
        let mut players = self.table_players.write().await;
        if let Some(player_set) = players.get_mut(&table_id) {
            player_set.remove(&user_id);
            if player_set.is_empty() {
                players.remove(&table_id);
            }
        }
    }

    /// Analyze win rate against specific players
    ///
    /// # Arguments
    ///
    /// * `user_id` - User ID
    /// * `opponent_id` - Opponent ID
    /// * `table_id` - Table ID
    /// * `win_rate` - Win rate against opponent (0.0 to 1.0)
    ///
    /// # Returns
    ///
    /// * `Result<(), String>` - Success or error
    pub async fn analyze_win_rate(
        &self,
        user_id: i64,
        opponent_id: i64,
        table_id: i64,
        win_rate: f32,
    ) -> Result<(), String> {
        // Check if users share same IP
        let ips = self.user_ips.read().await;
        let user_ip = ips.get(&user_id);
        let opponent_ip = ips.get(&opponent_id);

        let same_ip = match (user_ip, opponent_ip) {
            (Some(ip1), Some(ip2)) => ip1 == ip2,
            _ => false,
        };
        drop(ips);

        // Flag if win rate > 80% against same-IP player
        if same_ip && win_rate > 0.80 {
            self.create_flag(
                user_id,
                table_id,
                FlagType::WinRateAnomaly,
                FlagSeverity::High,
                serde_json::json!({
                    "opponent_id": opponent_id,
                    "win_rate": win_rate,
                    "same_ip": true
                }),
            )
            .await?;
        }

        Ok(())
    }

    /// Analyze folding patterns
    ///
    /// # Arguments
    ///
    /// * `table_id` - Table ID
    /// * `user_id` - User who folded
    /// * `beneficiary_id` - User who benefited from fold
    ///
    /// # Returns
    ///
    /// * `Result<(), String>` - Success or error
    pub async fn analyze_folding_pattern(
        &self,
        table_id: i64,
        user_id: i64,
        beneficiary_id: i64,
    ) -> Result<(), String> {
        // Check if coordinated folding (always folding to same player)
        // This would require historical tracking - placeholder for now

        // Check if same IP
        let ips = self.user_ips.read().await;
        let user_ip = ips.get(&user_id);
        let beneficiary_ip = ips.get(&beneficiary_id);

        if let (Some(ip1), Some(ip2)) = (user_ip, beneficiary_ip) {
            if ip1 == ip2 {
                drop(ips);
                self.create_flag(
                    user_id,
                    table_id,
                    FlagType::CoordinatedFolding,
                    FlagSeverity::Low,
                    serde_json::json!({
                        "beneficiary_id": beneficiary_id,
                        "same_ip": true
                    }),
                )
                .await?;
            }
        }

        Ok(())
    }

    /// Create a collusion flag
    async fn create_flag(
        &self,
        user_id: i64,
        table_id: i64,
        flag_type: FlagType,
        severity: FlagSeverity,
        details: serde_json::Value,
    ) -> Result<(), String> {
        sqlx::query(
            r#"
            INSERT INTO collusion_flags (user_id, table_id, flag_type, severity, details)
            VALUES ($1, $2, $3, $4, $5)
            "#,
        )
        .bind(user_id)
        .bind(table_id)
        .bind(flag_type.to_string())
        .bind(severity.to_string())
        .bind(details)
        .execute(self.pool.as_ref())
        .await
        .map_err(|e| format!("Database error: {}", e))?;

        log::warn!(
            "Collusion flag created: user={}, table={}, type={}, severity={}",
            user_id,
            table_id,
            flag_type,
            severity
        );

        Ok(())
    }

    /// Get unreviewed flags
    ///
    /// # Returns
    ///
    /// * `Result<Vec<CollusionFlag>, String>` - List of unreviewed flags
    pub async fn get_unreviewed_flags(&self) -> Result<Vec<CollusionFlag>, String> {
        let rows = sqlx::query(
            r#"
            SELECT id, user_id, table_id, flag_type, severity, details, created_at,
                   reviewed, reviewer_user_id, reviewed_at
            FROM collusion_flags
            WHERE NOT reviewed
            ORDER BY created_at DESC
            LIMIT 100
            "#,
        )
        .fetch_all(self.pool.as_ref())
        .await
        .map_err(|e| format!("Database error: {}", e))?;

        let flags = rows
            .into_iter()
            .map(|row| CollusionFlag {
                id: row.get("id"),
                user_id: row.get("user_id"),
                table_id: row.get("table_id"),
                flag_type: row.get("flag_type"),
                severity: row.get("severity"),
                details: row.get("details"),
                created_at: row.get::<chrono::NaiveDateTime, _>("created_at").and_utc(),
                reviewed: row.get("reviewed"),
                reviewer_user_id: row.get("reviewer_user_id"),
                reviewed_at: row
                    .get::<Option<chrono::NaiveDateTime>, _>("reviewed_at")
                    .map(|dt| dt.and_utc()),
            })
            .collect();

        Ok(flags)
    }

    /// Mark flag as reviewed
    ///
    /// # Arguments
    ///
    /// * `flag_id` - Flag ID
    /// * `reviewer_user_id` - Reviewer user ID
    ///
    /// # Returns
    ///
    /// * `Result<(), String>` - Success or error
    pub async fn mark_flag_reviewed(
        &self,
        flag_id: i64,
        reviewer_user_id: i64,
    ) -> Result<(), String> {
        sqlx::query(
            r#"
            UPDATE collusion_flags
            SET reviewed = true, reviewer_user_id = $1, reviewed_at = NOW()
            WHERE id = $2
            "#,
        )
        .bind(reviewer_user_id)
        .bind(flag_id)
        .execute(self.pool.as_ref())
        .await
        .map_err(|e| format!("Database error: {}", e))?;

        Ok(())
    }

    /// Get flags for a specific user
    ///
    /// # Arguments
    ///
    /// * `user_id` - User ID
    ///
    /// # Returns
    ///
    /// * `Result<Vec<CollusionFlag>, String>` - List of flags
    pub async fn get_user_flags(&self, user_id: i64) -> Result<Vec<CollusionFlag>, String> {
        let rows = sqlx::query(
            r#"
            SELECT id, user_id, table_id, flag_type, severity, details, created_at,
                   reviewed, reviewer_user_id, reviewed_at
            FROM collusion_flags
            WHERE user_id = $1
            ORDER BY created_at DESC
            LIMIT 50
            "#,
        )
        .bind(user_id)
        .fetch_all(self.pool.as_ref())
        .await
        .map_err(|e| format!("Database error: {}", e))?;

        let flags = rows
            .into_iter()
            .map(|row| CollusionFlag {
                id: row.get("id"),
                user_id: row.get("user_id"),
                table_id: row.get("table_id"),
                flag_type: row.get("flag_type"),
                severity: row.get("severity"),
                details: row.get("details"),
                created_at: row.get::<chrono::NaiveDateTime, _>("created_at").and_utc(),
                reviewed: row.get("reviewed"),
                reviewer_user_id: row.get("reviewer_user_id"),
                reviewed_at: row
                    .get::<Option<chrono::NaiveDateTime>, _>("reviewed_at")
                    .map(|dt| dt.and_utc()),
            })
            .collect();

        Ok(flags)
    }
}

/// IP-based table restrictions
pub struct IpTableRestrictions {
    /// Whether to enforce single-human-per-IP rule
    enforce_single_ip: bool,
}

impl IpTableRestrictions {
    /// Create new IP restrictions
    pub fn new(enforce_single_ip: bool) -> Self {
        Self { enforce_single_ip }
    }

    /// Check if user can join table based on IP
    ///
    /// # Arguments
    ///
    /// * `detector` - Anti-collusion detector
    /// * `table_id` - Table ID
    /// * `user_id` - User ID
    ///
    /// # Returns
    ///
    /// * `Result<bool, String>` - Whether user can join
    pub async fn can_join_table(
        &self,
        detector: &AntiCollusionDetector,
        table_id: i64,
        user_id: i64,
    ) -> Result<bool, String> {
        if !self.enforce_single_ip {
            return Ok(true);
        }

        let same_ip = detector.check_same_ip_at_table(table_id, user_id).await?;
        Ok(!same_ip)
    }
}
