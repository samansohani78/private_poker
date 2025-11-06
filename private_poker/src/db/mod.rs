//! Database module providing PostgreSQL connection pooling and utilities.
//!
//! This module manages the database connection pool using sqlx and provides
//! utilities for database operations across the application.

use sqlx::postgres::{PgPool, PgPoolOptions};
use std::time::Duration;

pub mod config;

pub use config::DatabaseConfig;

/// Database connection pool wrapper
#[derive(Clone)]
pub struct Database {
    pool: PgPool,
}

impl Database {
    /// Create a new database connection pool
    ///
    /// # Arguments
    ///
    /// * `config` - Database configuration
    ///
    /// # Returns
    ///
    /// * `Result<Database, sqlx::Error>` - Database instance or error
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use private_poker::db::{Database, DatabaseConfig};
    ///
    /// #[tokio::main]
    /// async fn main() -> Result<(), sqlx::Error> {
    ///     let config = DatabaseConfig::from_env();
    ///     let db = Database::new(&config).await?;
    ///     Ok(())
    /// }
    /// ```
    pub async fn new(config: &DatabaseConfig) -> Result<Self, sqlx::Error> {
        let pool = PgPoolOptions::new()
            .max_connections(config.max_connections)
            .min_connections(config.min_connections)
            .acquire_timeout(Duration::from_secs(config.connection_timeout_secs))
            .idle_timeout(Duration::from_secs(config.idle_timeout_secs))
            .max_lifetime(Duration::from_secs(config.max_lifetime_secs))
            .connect(&config.database_url)
            .await?;

        Ok(Self { pool })
    }

    /// Get a reference to the connection pool
    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    /// Check if the database connection is healthy
    ///
    /// # Returns
    ///
    /// * `Result<(), sqlx::Error>` - Ok if healthy, error otherwise
    pub async fn health_check(&self) -> Result<(), sqlx::Error> {
        sqlx::query("SELECT 1")
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Close the database connection pool
    pub async fn close(self) {
        self.pool.close().await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    #[ignore] // Requires database to be running
    async fn test_database_connection() {
        let config = DatabaseConfig {
            database_url: "postgres://postgres@localhost/poker_db".to_string(),
            max_connections: 5,
            min_connections: 1,
            connection_timeout_secs: 5,
            idle_timeout_secs: 300,
            max_lifetime_secs: 1800,
        };

        let db = Database::new(&config).await.expect("Failed to connect to database");
        db.health_check().await.expect("Health check failed");
        db.close().await;
    }
}
