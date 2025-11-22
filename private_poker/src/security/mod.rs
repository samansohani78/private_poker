//! Security module providing rate limiting and anti-collusion features.
//!
//! This module implements comprehensive security features:
//! - Rate limiting with exponential backoff for auth endpoints
//! - Anti-collusion detection with shadow flagging
//! - IP-based table restrictions
//! - Seat randomization to prevent manipulation
//!
//! ## Rate Limiting
//!
//! Protects endpoints from abuse with configurable limits:
//! - **Login**: 5 attempts per 5 minutes, 15-minute lockout with exponential backoff
//! - **Registration**: 3 attempts per hour, 1-hour lockout
//! - **Password Reset**: 3 attempts per hour, 2-hour lockout with exponential backoff
//! - **Chat**: 10 messages per minute, 5-minute lockout
//!
//! ## Anti-Collusion
//!
//! Detects suspicious patterns without auto-banning:
//! - **Same-IP detection**: Flags players from same IP at same table
//! - **Win rate anomalies**: Flags >80% win rate against same-IP players
//! - **Coordinated folding**: Detects always folding to same player
//! - **Shadow flagging**: All flags require admin review
//!
//! ## Example
//!
//! ```no_run
//! use private_poker::security::{RateLimiter, AntiCollusionDetector};
//! use private_poker::db::Database;
//! use std::sync::Arc;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     // Example: Create database connection
//!     # let config = private_poker::db::DatabaseConfig {
//!     #     database_url: "postgres://localhost/test".to_string(),
//!     #     max_connections: 5,
//!     #     min_connections: 1,
//!     #     connection_timeout_secs: 5,
//!     #     idle_timeout_secs: 300,
//!     #     max_lifetime_secs: 1800,
//!     # };
//!     # let db = Database::new(&config).await?;
//!     let db_pool = Arc::new(db.pool().clone());
//!
//!     // Rate limiting
//!     let rate_limiter = RateLimiter::new(db_pool.clone());
//!     let result = rate_limiter.check_rate_limit("login", "192.168.1.1").await?;
//!
//!     if result.is_allowed() {
//!         println!("Login allowed, {} attempts remaining", result.remaining().unwrap());
//!         rate_limiter.record_attempt("login", "192.168.1.1").await?;
//!     }
//!
//!     // Anti-collusion
//!     let detector = AntiCollusionDetector::new(db_pool);
//!     detector.register_user_ip(1, "192.168.1.1".to_string()).await;
//!
//!     let same_ip = detector.check_same_ip_at_table(101, 1).await?;
//!     if same_ip {
//!         println!("Warning: Same IP detected at table");
//!     }
//!
//!     Ok(())
//! }
//! ```

pub mod anti_collusion;
pub mod errors;
pub mod rate_limiter;
pub mod seat_randomizer;

pub use anti_collusion::{
    AntiCollusionDetector, CollusionFlag, FlagSeverity, FlagType, IpTableRestrictions, normalize_ip,
};
pub use errors::{AntiCollusionError, AntiCollusionResult, RateLimitError, RateLimiterResult};
pub use rate_limiter::{RateLimitConfig, RateLimitResult, RateLimiter};
pub use seat_randomizer::SeatRandomizer;
