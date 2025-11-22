//! Server configuration management.
//!
//! Consolidates all environment variable reads and provides validated configuration.

use private_poker::{db::DatabaseConfig, table::BotDifficulty};
use std::net::SocketAddr;

/// Complete server configuration loaded from environment variables
#[derive(Debug, Clone)]
pub struct ServerConfig {
    /// Server bind address
    pub bind: SocketAddr,
    /// Database configuration
    pub database: DatabaseConfig,
    /// Security configuration
    pub security: SecurityConfig,
    /// Table defaults configuration
    pub table_defaults: TableDefaultsConfig,
    /// Number of tables to create on startup
    pub num_tables: usize,
}

/// Security-related configuration
#[derive(Debug, Clone)]
pub struct SecurityConfig {
    /// JWT signing secret (required)
    pub jwt_secret: String,
    /// Password hashing pepper (required)
    pub password_pepper: String,
}

/// Default table configuration
#[derive(Debug, Clone)]
pub struct TableDefaultsConfig {
    /// Maximum players per table
    pub max_players: usize,
    /// Small blind amount
    pub small_blind: i64,
    /// Big blind amount
    pub big_blind: i64,
    /// Minimum buy-in (in big blinds)
    pub min_buy_in_bb: u8,
    /// Maximum buy-in (in big blinds)
    pub max_buy_in_bb: u8,
    /// Absolute chip cap per player
    pub absolute_chip_cap: i64,
    /// Top-up cooldown in hands
    pub top_up_cooldown_hands: u8,
    /// Whether bots are enabled
    pub bots_enabled: bool,
    /// Target number of bots per table
    pub target_bot_count: u8,
    /// Default bot difficulty
    pub bot_difficulty: BotDifficulty,
}

impl ServerConfig {
    /// Load configuration from environment variables
    ///
    /// # Arguments
    ///
    /// * `bind_override` - Optional bind address override (from CLI args)
    /// * `database_url_override` - Optional database URL override (from CLI args)
    /// * `num_tables_override` - Optional number of tables override (from CLI args)
    ///
    /// # Returns
    ///
    /// * `Result<ServerConfig, ConfigError>` - Loaded configuration or error
    ///
    /// # Errors
    ///
    /// Returns error if required variables are missing or invalid
    pub fn from_env(
        bind_override: Option<SocketAddr>,
        database_url_override: Option<String>,
        num_tables_override: Option<usize>,
    ) -> Result<Self, ConfigError> {
        // Bind address
        let bind = bind_override
            .or_else(|| {
                std::env::var("SERVER_BIND")
                    .ok()
                    .and_then(|s| s.parse().ok())
            })
            .unwrap_or_else(|| {
                "127.0.0.1:6969"
                    .parse()
                    .expect("Default bind address is valid")
            });

        // Database configuration
        let database_url = database_url_override
            .or_else(|| std::env::var("DATABASE_URL").ok())
            .unwrap_or_else(|| {
                "postgres://poker_test:test_password@localhost/poker_test".to_string()
            });

        let database = DatabaseConfig {
            database_url,
            max_connections: parse_env_or("DB_MAX_CONNECTIONS", 100),
            min_connections: parse_env_or("DB_MIN_CONNECTIONS", 5),
            connection_timeout_secs: parse_env_or("DB_CONNECTION_TIMEOUT_SECS", 5),
            idle_timeout_secs: parse_env_or("DB_IDLE_TIMEOUT_SECS", 300),
            max_lifetime_secs: parse_env_or("DB_MAX_LIFETIME_SECS", 1800),
        };

        // Security configuration (REQUIRED)
        let jwt_secret = std::env::var("JWT_SECRET").map_err(|_| ConfigError::MissingRequired {
            var: "JWT_SECRET".to_string(),
            hint: "Generate with: openssl rand -hex 32".to_string(),
        })?;

        let password_pepper =
            std::env::var("PASSWORD_PEPPER").map_err(|_| ConfigError::MissingRequired {
                var: "PASSWORD_PEPPER".to_string(),
                hint: "Generate with: openssl rand -hex 16".to_string(),
            })?;

        // Validate security params
        if jwt_secret.len() < 32 {
            return Err(ConfigError::Invalid {
                var: "JWT_SECRET".to_string(),
                reason: "Must be at least 32 characters (128-bit security)".to_string(),
            });
        }

        if password_pepper.len() < 16 {
            return Err(ConfigError::Invalid {
                var: "PASSWORD_PEPPER".to_string(),
                reason: "Must be at least 16 characters (64-bit security)".to_string(),
            });
        }

        let security = SecurityConfig {
            jwt_secret,
            password_pepper,
        };

        // Table defaults
        let bot_difficulty = std::env::var("DEFAULT_BOT_DIFFICULTY")
            .ok()
            .and_then(|v| match v.to_lowercase().as_str() {
                "easy" => Some(BotDifficulty::Easy),
                "standard" => Some(BotDifficulty::Standard),
                "tag" => Some(BotDifficulty::Tag),
                _ => None,
            })
            .unwrap_or(BotDifficulty::Standard);

        let table_defaults = TableDefaultsConfig {
            max_players: parse_env_or("TABLE_MAX_PLAYERS", 9),
            small_blind: parse_env_or("TABLE_SMALL_BLIND", 10),
            big_blind: parse_env_or("TABLE_BIG_BLIND", 20),
            min_buy_in_bb: parse_env_or("TABLE_MIN_BUY_IN_BB", 50),
            max_buy_in_bb: parse_env_or("TABLE_MAX_BUY_IN_BB", 200),
            absolute_chip_cap: parse_env_or("ABSOLUTE_CHIP_CAP", 100_000),
            top_up_cooldown_hands: parse_env_or("TABLE_TOP_UP_COOLDOWN_HANDS", 20),
            bots_enabled: parse_env_or("BOTS_ENABLED", true),
            target_bot_count: parse_env_or("TARGET_BOT_COUNT", 6),
            bot_difficulty,
        };

        // Number of tables
        let num_tables = num_tables_override.unwrap_or_else(|| parse_env_or("MAX_TABLES", 1));

        Ok(ServerConfig {
            bind,
            database,
            security,
            table_defaults,
            num_tables,
        })
    }

    /// Validate configuration after loading
    ///
    /// # Returns
    ///
    /// * `Result<(), ConfigError>` - Success or validation error
    pub fn validate(&self) -> Result<(), ConfigError> {
        // Validate blinds
        if self.table_defaults.small_blind == 0 {
            return Err(ConfigError::Invalid {
                var: "TABLE_SMALL_BLIND".to_string(),
                reason: "Must be greater than 0".to_string(),
            });
        }

        if self.table_defaults.big_blind <= self.table_defaults.small_blind {
            return Err(ConfigError::Invalid {
                var: "TABLE_BIG_BLIND".to_string(),
                reason: format!(
                    "Must be greater than small blind ({})",
                    self.table_defaults.small_blind
                ),
            });
        }

        // Validate buy-ins
        if self.table_defaults.min_buy_in_bb == 0 {
            return Err(ConfigError::Invalid {
                var: "TABLE_MIN_BUY_IN_BB".to_string(),
                reason: "Must be greater than 0".to_string(),
            });
        }

        if self.table_defaults.max_buy_in_bb <= self.table_defaults.min_buy_in_bb {
            return Err(ConfigError::Invalid {
                var: "TABLE_MAX_BUY_IN_BB".to_string(),
                reason: format!(
                    "Must be greater than min buy-in ({})",
                    self.table_defaults.min_buy_in_bb
                ),
            });
        }

        // Validate player count
        if self.table_defaults.max_players < 2 {
            return Err(ConfigError::Invalid {
                var: "TABLE_MAX_PLAYERS".to_string(),
                reason: "Must be at least 2".to_string(),
            });
        }

        if self.table_defaults.max_players > 23 {
            return Err(ConfigError::Invalid {
                var: "TABLE_MAX_PLAYERS".to_string(),
                reason: "Must be at most 23 (max players with 52-card deck)".to_string(),
            });
        }

        // Validate bot count
        if self.table_defaults.target_bot_count as usize > self.table_defaults.max_players {
            return Err(ConfigError::Invalid {
                var: "TARGET_BOT_COUNT".to_string(),
                reason: format!(
                    "Cannot exceed max players ({})",
                    self.table_defaults.max_players
                ),
            });
        }

        Ok(())
    }
}

/// Configuration error types
#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("Missing required environment variable: {var}\nHint: {hint}")]
    MissingRequired { var: String, hint: String },

    #[error("Invalid configuration for {var}: {reason}")]
    Invalid { var: String, reason: String },
}

/// Helper to parse environment variable with default fallback
fn parse_env_or<T>(key: &str, default: T) -> T
where
    T: std::str::FromStr,
{
    std::env::var(key)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_error_display() {
        let err = ConfigError::MissingRequired {
            var: "JWT_SECRET".to_string(),
            hint: "Use openssl".to_string(),
        };
        let msg = err.to_string();
        assert!(msg.contains("JWT_SECRET"));
        assert!(msg.contains("Use openssl"));
    }

    #[test]
    fn test_config_validation_blind_zero() {
        let config = ServerConfig {
            bind: "127.0.0.1:8080".parse().unwrap(),
            database: DatabaseConfig {
                database_url: "test".to_string(),
                max_connections: 10,
                min_connections: 1,
                connection_timeout_secs: 5,
                idle_timeout_secs: 300,
                max_lifetime_secs: 1800,
            },
            security: SecurityConfig {
                jwt_secret: "a".repeat(32),
                password_pepper: "a".repeat(16),
            },
            table_defaults: TableDefaultsConfig {
                max_players: 9,
                small_blind: 0, // Invalid
                big_blind: 20,
                min_buy_in_bb: 50,
                max_buy_in_bb: 200,
                absolute_chip_cap: 100_000,
                top_up_cooldown_hands: 20,
                bots_enabled: true,
                target_bot_count: 6,
                bot_difficulty: BotDifficulty::Standard,
            },
            num_tables: 1,
        };

        let err = config.validate().unwrap_err();
        assert!(matches!(err, ConfigError::Invalid { .. }));
    }

    #[test]
    fn test_config_validation_big_blind_too_small() {
        let config = ServerConfig {
            bind: "127.0.0.1:8080".parse().unwrap(),
            database: DatabaseConfig {
                database_url: "test".to_string(),
                max_connections: 10,
                min_connections: 1,
                connection_timeout_secs: 5,
                idle_timeout_secs: 300,
                max_lifetime_secs: 1800,
            },
            security: SecurityConfig {
                jwt_secret: "a".repeat(32),
                password_pepper: "a".repeat(16),
            },
            table_defaults: TableDefaultsConfig {
                max_players: 9,
                small_blind: 20,
                big_blind: 10, // Invalid: less than small blind
                min_buy_in_bb: 50,
                max_buy_in_bb: 200,
                absolute_chip_cap: 100_000,
                top_up_cooldown_hands: 20,
                bots_enabled: true,
                target_bot_count: 6,
                bot_difficulty: BotDifficulty::Standard,
            },
            num_tables: 1,
        };

        let err = config.validate().unwrap_err();
        assert!(matches!(err, ConfigError::Invalid { .. }));
    }
}
