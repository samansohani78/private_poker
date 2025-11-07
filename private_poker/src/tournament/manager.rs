//! Tournament manager for creating and managing Sit-n-Go tournaments.

use super::models::{
    PrizeStructure, TournamentConfig, TournamentId, TournamentInfo, TournamentRegistration,
    TournamentState, TournamentType,
};
use chrono::Utc;
use sqlx::{PgPool, Row};
use std::sync::Arc;
use thiserror::Error;

/// Tournament errors
#[derive(Debug, Error)]
pub enum TournamentError {
    #[error("Tournament not found: {0}")]
    NotFound(TournamentId),

    #[error("Tournament is full")]
    TournamentFull,

    #[error("Tournament already started")]
    AlreadyStarted,

    #[error("Player already registered")]
    AlreadyRegistered,

    #[error("Tournament not in correct state: expected {expected:?}, got {actual:?}")]
    InvalidState {
        expected: TournamentState,
        actual: TournamentState,
    },

    #[error("Insufficient players: need {needed}, have {current}")]
    InsufficientPlayers { needed: usize, current: usize },

    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
}

pub type TournamentResult<T> = Result<T, TournamentError>;

/// Tournament manager
#[derive(Clone)]
pub struct TournamentManager {
    pool: Arc<PgPool>,
}

impl TournamentManager {
    /// Create a new tournament manager
    pub fn new(pool: Arc<PgPool>) -> Self {
        Self { pool }
    }

    /// Create a new tournament
    pub async fn create_tournament(
        &self,
        config: TournamentConfig,
    ) -> TournamentResult<TournamentId> {
        let config_json = serde_json::to_value(&config)?;

        let row = sqlx::query(
            r#"
            INSERT INTO tournaments (name, tournament_type, config, state, buy_in, min_players, max_players, starting_stack, current_level)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
            RETURNING id
            "#,
        )
        .bind(&config.name)
        .bind(match config.tournament_type {
            TournamentType::SitAndGo => "sit_and_go",
            TournamentType::Scheduled => "scheduled",
        })
        .bind(config_json)
        .bind("registering")
        .bind(config.buy_in)
        .bind(config.min_players as i32)
        .bind(config.max_players as i32)
        .bind(config.starting_stack)
        .bind(config.starting_level as i32)
        .fetch_one(self.pool.as_ref())
        .await?;

        Ok(row.get("id"))
    }

    /// Register a player for a tournament
    pub async fn register_player(
        &self,
        tournament_id: TournamentId,
        user_id: i64,
        username: String,
    ) -> TournamentResult<()> {
        // Get tournament info
        let tournament = self.get_tournament_info(tournament_id).await?;

        // Check state
        if tournament.state != TournamentState::Registering {
            return Err(TournamentError::AlreadyStarted);
        }

        // Check if full
        if tournament.registered_count >= tournament.config.max_players {
            return Err(TournamentError::TournamentFull);
        }

        // Check if already registered
        let existing = sqlx::query(
            "SELECT user_id FROM tournament_registrations WHERE tournament_id = $1 AND user_id = $2",
        )
        .bind(tournament_id)
        .bind(user_id)
        .fetch_optional(self.pool.as_ref())
        .await?;

        if existing.is_some() {
            return Err(TournamentError::AlreadyRegistered);
        }

        // Insert registration
        sqlx::query(
            r#"
            INSERT INTO tournament_registrations (tournament_id, user_id, username, chip_count)
            VALUES ($1, $2, $3, $4)
            "#,
        )
        .bind(tournament_id)
        .bind(user_id)
        .bind(&username)
        .bind(tournament.config.starting_stack)
        .execute(self.pool.as_ref())
        .await?;

        // Update registered count
        sqlx::query("UPDATE tournaments SET registered_count = registered_count + 1 WHERE id = $1")
            .bind(tournament_id)
            .execute(self.pool.as_ref())
            .await?;

        // Check if we should start (for Sit-n-Go)
        if tournament.config.tournament_type == TournamentType::SitAndGo {
            let new_count = tournament.registered_count + 1;
            if new_count >= tournament.config.max_players {
                self.start_tournament(tournament_id).await?;
            }
        }

        Ok(())
    }

    /// Unregister a player from a tournament
    pub async fn unregister_player(
        &self,
        tournament_id: TournamentId,
        user_id: i64,
    ) -> TournamentResult<()> {
        let tournament = self.get_tournament_info(tournament_id).await?;

        if tournament.state != TournamentState::Registering {
            return Err(TournamentError::AlreadyStarted);
        }

        let result = sqlx::query(
            "DELETE FROM tournament_registrations WHERE tournament_id = $1 AND user_id = $2",
        )
        .bind(tournament_id)
        .bind(user_id)
        .execute(self.pool.as_ref())
        .await?;

        if result.rows_affected() == 0 {
            return Err(TournamentError::NotFound(tournament_id));
        }

        sqlx::query("UPDATE tournaments SET registered_count = registered_count - 1 WHERE id = $1")
            .bind(tournament_id)
            .execute(self.pool.as_ref())
            .await?;

        Ok(())
    }

    /// Start a tournament
    pub async fn start_tournament(&self, tournament_id: TournamentId) -> TournamentResult<()> {
        let tournament = self.get_tournament_info(tournament_id).await?;

        if tournament.state != TournamentState::Registering {
            return Err(TournamentError::InvalidState {
                expected: TournamentState::Registering,
                actual: tournament.state,
            });
        }

        if tournament.registered_count < tournament.config.min_players {
            return Err(TournamentError::InsufficientPlayers {
                needed: tournament.config.min_players,
                current: tournament.registered_count,
            });
        }

        sqlx::query(
            "UPDATE tournaments SET state = $1, started_at = NOW(), level_started_at = NOW() WHERE id = $2",
        )
        .bind("running")
        .bind(tournament_id)
        .execute(self.pool.as_ref())
        .await?;

        Ok(())
    }

    /// Advance to next blind level
    pub async fn advance_blind_level(&self, tournament_id: TournamentId) -> TournamentResult<u32> {
        let tournament = self.get_tournament_info(tournament_id).await?;

        if tournament.state != TournamentState::Running {
            return Err(TournamentError::InvalidState {
                expected: TournamentState::Running,
                actual: tournament.state,
            });
        }

        let next_level = tournament.current_level + 1;

        // Check if next level exists
        if tournament.config.get_blind_level(next_level).is_none() {
            // No more levels, keep current level
            return Ok(tournament.current_level);
        }

        sqlx::query(
            "UPDATE tournaments SET current_level = $1, level_started_at = NOW() WHERE id = $2",
        )
        .bind(next_level as i32)
        .bind(tournament_id)
        .execute(self.pool.as_ref())
        .await?;

        Ok(next_level)
    }

    /// Record player elimination
    pub async fn eliminate_player(
        &self,
        tournament_id: TournamentId,
        user_id: i64,
        position: usize,
    ) -> TournamentResult<()> {
        let tournament = self.get_tournament_info(tournament_id).await?;

        // Calculate prize (if in the money)
        let prize_structure =
            PrizeStructure::standard(tournament.registered_count, tournament.config.buy_in);

        let prize_amount = prize_structure.payout_for_position(position);

        sqlx::query(
            r#"
            UPDATE tournament_registrations
            SET finish_position = $1, prize_amount = $2, finished_at = NOW()
            WHERE tournament_id = $3 AND user_id = $4
            "#,
        )
        .bind(position as i32)
        .bind(prize_amount)
        .bind(tournament_id)
        .bind(user_id)
        .execute(self.pool.as_ref())
        .await?;

        Ok(())
    }

    /// Finish tournament
    pub async fn finish_tournament(&self, tournament_id: TournamentId) -> TournamentResult<()> {
        sqlx::query("UPDATE tournaments SET state = $1, finished_at = NOW() WHERE id = $2")
            .bind("finished")
            .bind(tournament_id)
            .execute(self.pool.as_ref())
            .await?;

        Ok(())
    }

    /// Cancel tournament
    pub async fn cancel_tournament(&self, tournament_id: TournamentId) -> TournamentResult<()> {
        let tournament = self.get_tournament_info(tournament_id).await?;

        if tournament.state == TournamentState::Finished {
            return Err(TournamentError::InvalidState {
                expected: TournamentState::Registering,
                actual: TournamentState::Finished,
            });
        }

        sqlx::query("UPDATE tournaments SET state = $1, finished_at = NOW() WHERE id = $2")
            .bind("cancelled")
            .bind(tournament_id)
            .execute(self.pool.as_ref())
            .await?;

        Ok(())
    }

    /// Get tournament information
    pub async fn get_tournament_info(
        &self,
        tournament_id: TournamentId,
    ) -> TournamentResult<TournamentInfo> {
        let row = sqlx::query(
            r#"
            SELECT id, name, tournament_type, config, state, buy_in, registered_count,
                   current_level, level_started_at, created_at, started_at, finished_at
            FROM tournaments
            WHERE id = $1
            "#,
        )
        .bind(tournament_id)
        .fetch_optional(self.pool.as_ref())
        .await?
        .ok_or(TournamentError::NotFound(tournament_id))?;

        let config: TournamentConfig = serde_json::from_value(row.get("config"))?;
        let state_str: String = row.get("state");
        let state = match state_str.as_str() {
            "registering" => TournamentState::Registering,
            "running" => TournamentState::Running,
            "finished" => TournamentState::Finished,
            "cancelled" => TournamentState::Cancelled,
            _ => TournamentState::Registering,
        };

        let registered_count: i32 = row.get("registered_count");
        let current_level: i32 = row.get("current_level");

        // Calculate time to next level
        let time_to_next_level = if state == TournamentState::Running {
            if let Some(level_started_at) =
                row.get::<Option<chrono::NaiveDateTime>, _>("level_started_at")
            {
                let level_started_at = level_started_at.and_utc();
                if let Some(current_blind) = config.get_blind_level(current_level as u32) {
                    let elapsed = (Utc::now() - level_started_at).num_seconds();
                    let remaining = current_blind.duration_secs as i64 - elapsed;
                    Some(remaining.max(0) as u32)
                } else {
                    None
                }
            } else {
                None
            }
        } else {
            None
        };

        let prize_structure = PrizeStructure::standard(registered_count as usize, config.buy_in);

        Ok(TournamentInfo {
            id: row.get("id"),
            config,
            state,
            registered_count: registered_count as usize,
            current_level: current_level as u32,
            time_to_next_level,
            prize_structure,
            created_at: row.get::<chrono::NaiveDateTime, _>("created_at").and_utc(),
            started_at: row
                .get::<Option<chrono::NaiveDateTime>, _>("started_at")
                .map(|dt| dt.and_utc()),
            finished_at: row
                .get::<Option<chrono::NaiveDateTime>, _>("finished_at")
                .map(|dt| dt.and_utc()),
        })
    }

    /// List all tournaments
    pub async fn list_tournaments(
        &self,
        state_filter: Option<TournamentState>,
    ) -> TournamentResult<Vec<TournamentInfo>> {
        let query = if let Some(state) = state_filter {
            let state_str = match state {
                TournamentState::Registering => "registering",
                TournamentState::Running => "running",
                TournamentState::Finished => "finished",
                TournamentState::Cancelled => "cancelled",
            };
            sqlx::query(
                r#"
                SELECT id, name, tournament_type, config, state, buy_in, registered_count,
                       current_level, level_started_at, created_at, started_at, finished_at
                FROM tournaments
                WHERE state = $1
                ORDER BY created_at DESC
                "#,
            )
            .bind(state_str)
        } else {
            sqlx::query(
                r#"
                SELECT id, name, tournament_type, config, state, buy_in, registered_count,
                       current_level, level_started_at, created_at, started_at, finished_at
                FROM tournaments
                ORDER BY created_at DESC
                "#,
            )
        };

        let rows = query.fetch_all(self.pool.as_ref()).await?;

        let mut tournaments = Vec::new();
        for row in rows {
            let config: TournamentConfig = serde_json::from_value(row.get("config"))?;
            let state_str: String = row.get("state");
            let state = match state_str.as_str() {
                "registering" => TournamentState::Registering,
                "running" => TournamentState::Running,
                "finished" => TournamentState::Finished,
                "cancelled" => TournamentState::Cancelled,
                _ => TournamentState::Registering,
            };

            let registered_count: i32 = row.get("registered_count");
            let current_level: i32 = row.get("current_level");

            let time_to_next_level = if state == TournamentState::Running {
                if let Some(level_started_at) =
                    row.get::<Option<chrono::NaiveDateTime>, _>("level_started_at")
                {
                    let level_started_at = level_started_at.and_utc();
                    if let Some(current_blind) = config.get_blind_level(current_level as u32) {
                        let elapsed = (Utc::now() - level_started_at).num_seconds();
                        let remaining = current_blind.duration_secs as i64 - elapsed;
                        Some(remaining.max(0) as u32)
                    } else {
                        None
                    }
                } else {
                    None
                }
            } else {
                None
            };

            let prize_structure =
                PrizeStructure::standard(registered_count as usize, config.buy_in);

            tournaments.push(TournamentInfo {
                id: row.get("id"),
                config,
                state,
                registered_count: registered_count as usize,
                current_level: current_level as u32,
                time_to_next_level,
                prize_structure,
                created_at: row.get::<chrono::NaiveDateTime, _>("created_at").and_utc(),
                started_at: row
                    .get::<Option<chrono::NaiveDateTime>, _>("started_at")
                    .map(|dt| dt.and_utc()),
                finished_at: row
                    .get::<Option<chrono::NaiveDateTime>, _>("finished_at")
                    .map(|dt| dt.and_utc()),
            });
        }

        Ok(tournaments)
    }

    /// Get tournament registrations
    pub async fn get_registrations(
        &self,
        tournament_id: TournamentId,
    ) -> TournamentResult<Vec<TournamentRegistration>> {
        let rows = sqlx::query(
            r#"
            SELECT user_id, username, registered_at, chip_count, finish_position, prize_amount
            FROM tournament_registrations
            WHERE tournament_id = $1
            ORDER BY registered_at
            "#,
        )
        .bind(tournament_id)
        .fetch_all(self.pool.as_ref())
        .await?;

        let registrations = rows
            .into_iter()
            .map(|row| TournamentRegistration {
                user_id: row.get("user_id"),
                username: row.get("username"),
                registered_at: row
                    .get::<chrono::NaiveDateTime, _>("registered_at")
                    .and_utc(),
                chip_count: row.get("chip_count"),
                finish_position: row
                    .get::<Option<i32>, _>("finish_position")
                    .map(|p| p as usize),
                prize_amount: row.get("prize_amount"),
            })
            .collect();

        Ok(registrations)
    }

    /// Update player chip count
    pub async fn update_chip_count(
        &self,
        tournament_id: TournamentId,
        user_id: i64,
        new_chip_count: i64,
    ) -> TournamentResult<()> {
        sqlx::query(
            "UPDATE tournament_registrations SET chip_count = $1 WHERE tournament_id = $2 AND user_id = $3",
        )
        .bind(new_chip_count)
        .bind(tournament_id)
        .bind(user_id)
        .execute(self.pool.as_ref())
        .await?;

        Ok(())
    }
}
