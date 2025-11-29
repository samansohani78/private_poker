//! Multi-table poker server using async actor model.
//!
//! This server spawns TableActor instances managed by TableManager,
//! with database-backed authentication and wallet systems.

mod api;
mod config;
mod logging;
mod metrics;

use std::net::SocketAddr;
use std::sync::Arc;

use anyhow::Error;
use ctrlc::set_handler;
use pico_args::Arguments;
use private_poker::{
    auth::AuthManager,
    db::Database,
    table::{TableConfig, TableManager, TableSpeed},
    wallet::WalletManager,
};

use config::ServerConfig;

const HELP: &str = "\
Run a multi-table private poker server

USAGE:
  pp_server [OPTIONS]

OPTIONS:
  --bind       IP:PORT     Server socket bind address  [default: env SERVER_BIND or 127.0.0.1:6969]
  --db-url     URL         Database connection string  [default: env DATABASE_URL or postgres://poker_test:test_password@localhost/poker_test]
  --tables     N           Number of tables to create  [default: 1]

FLAGS:
  -h, --help               Print help information

ENVIRONMENT:
  SERVER_BIND              Server bind address (e.g., 0.0.0.0:8080)
  DATABASE_URL             PostgresSQL connection string
  JWT_SECRET               JWT signing secret
  PASSWORD_PEPPER          Password hashing pepper
  (See .env file for all configuration options)
";

struct Args {
    bind: SocketAddr,
    database_url: String,
    num_tables: usize,
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    // Load .env file if it exists (searches current dir and parent dirs)
    // Note: dotenvy does NOT override existing environment variables
    let _ = dotenvy::dotenv();

    let mut pargs = Arguments::from_env();

    // Help has a higher priority and should be handled separately.
    if pargs.contains(["-h", "--help"]) {
        print!("{HELP}");
        std::process::exit(0);
    }

    let args = Args {
        bind: pargs.value_from_str("--bind").unwrap_or_else(|_| {
            std::env::var("SERVER_BIND")
                .unwrap_or_else(|_| "127.0.0.1:6969".to_string())
                .parse()
                .expect("Invalid SERVER_BIND address")
        }),
        database_url: pargs.value_from_str("--db-url").unwrap_or_else(|_| {
            std::env::var("DATABASE_URL").unwrap_or_else(|_| {
                "postgres://poker_test:test_password@localhost/poker_test".to_string()
            })
        }),
        num_tables: pargs.value_from_str("--tables").unwrap_or_else(|_| {
            std::env::var("MAX_TABLES")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(1)
        }),
    };

    // Catching signals for exit.
    set_handler(|| std::process::exit(0))?;

    // Initialize structured logging
    logging::init();

    // Initialize Prometheus metrics exporter
    let metrics_addr: SocketAddr = std::env::var("PP_METRICS_BIND")
        .unwrap_or_else(|_| "127.0.0.1:9090".to_string())
        .parse()
        .expect("Invalid PP_METRICS_BIND address");

    if let Err(e) = metrics::init_metrics(metrics_addr) {
        tracing::warn!(
            "Failed to initialize metrics: {}. Metrics will not be available.",
            e
        );
    } else {
        tracing::info!(
            "Metrics endpoint available at http://{}/metrics",
            metrics_addr
        );
    }

    // Load and validate configuration
    tracing::info!("Loading configuration from environment...");
    let config = ServerConfig::from_env(
        Some(args.bind),
        Some(args.database_url),
        Some(args.num_tables),
    )
    .map_err(|e| anyhow::anyhow!("Configuration error: {}", e))?;

    config
        .validate()
        .map_err(|e| anyhow::anyhow!("Configuration validation failed: {}", e))?;

    tracing::info!("Configuration loaded and validated successfully");
    tracing::info!("Starting multi-table poker server at {}", config.bind);

    // Initialize database
    tracing::info!("Connecting to database: {}", config.database.database_url);
    let db = Database::new(&config.database)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to connect to database: {}", e))?;

    tracing::info!("Database connected successfully");

    // Create managers
    let pool = Arc::new(db.pool().clone());
    let wallet_manager = Arc::new(WalletManager::new(pool.clone()));
    let table_manager = Arc::new(TableManager::new(pool.clone(), wallet_manager.clone()));

    let auth_manager = Arc::new(AuthManager::new(
        pool.clone(),
        config.security.password_pepper.clone(),
        config.security.jwt_secret.clone(),
    ));

    // Load existing tables from database first
    tracing::info!("Loading existing tables from database...");
    match table_manager.load_existing_tables().await {
        Ok(count) => {
            tracing::info!("✓ Loaded {} existing table(s) from database", count);
        }
        Err(e) => {
            tracing::error!("Failed to load existing tables: {}", e);
        }
    }

    tracing::info!("Creating {} new table(s)...", config.num_tables);

    // Create initial tables
    for i in 0..config.num_tables {
        let table_config = TableConfig {
            name: format!("Table {}", i + 1),
            max_players: config.table_defaults.max_players,
            small_blind: config.table_defaults.small_blind,
            big_blind: config.table_defaults.big_blind,
            min_buy_in_bb: config.table_defaults.min_buy_in_bb,
            max_buy_in_bb: config.table_defaults.max_buy_in_bb,
            absolute_chip_cap: config.table_defaults.absolute_chip_cap,
            top_up_cooldown_hands: config.table_defaults.top_up_cooldown_hands,
            speed: TableSpeed::Normal,
            bots_enabled: config.table_defaults.bots_enabled,
            target_bot_count: config.table_defaults.target_bot_count,
            bot_difficulty: config.table_defaults.bot_difficulty,
            is_private: false,
            passphrase_hash: None,
            invite_token: None,
            invite_expires_at: None,
        };

        match table_manager.create_table(table_config, None).await {
            Ok(table_id) => {
                tracing::info!("✓ Created table {} with ID {}", i + 1, table_id);
            }
            Err(e) => {
                tracing::error!("Failed to create table {}: {}", i + 1, e);
            }
        }
    }

    let active_count = table_manager.active_table_count().await;
    tracing::info!("Server ready with {} active table(s)", active_count);

    // List tables
    match table_manager.list_tables().await {
        Ok(tables) => {
            tracing::info!("Active tables:");
            for table in tables {
                tracing::info!(
                    "  - {} (ID: {}) - {}/{} players, blinds: {}/{}",
                    table.name,
                    table.id,
                    table.player_count,
                    table.max_players,
                    table.small_blind,
                    table.big_blind
                );
            }
        }
        Err(e) => {
            tracing::error!("Failed to list tables: {}", e);
        }
    }

    // Spawn background task for session cleanup
    // Runs every hour to prevent database bloat from expired sessions
    let cleanup_auth_manager = auth_manager.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(3600)); // 1 hour
        loop {
            interval.tick().await;
            match cleanup_auth_manager.cleanup_expired_sessions().await {
                Ok(count) => {
                    if count > 0 {
                        tracing::info!("Cleaned up {} expired session(s)", count);
                    }
                }
                Err(e) => {
                    tracing::error!("Session cleanup failed: {}", e);
                }
            }
        }
    });

    tracing::info!("Background session cleanup task started (runs every hour)");

    // Create API state
    let api_state = api::AppState {
        auth_manager,
        table_manager,
        wallet_manager,
        pool: pool.clone(),
    };

    // Create router
    let app = api::create_router(api_state);

    // Start HTTP server
    tracing::info!("Starting HTTP/WebSocket server on {}", config.bind);
    let listener = tokio::net::TcpListener::bind(config.bind)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to bind to {}: {}", config.bind, e))?;

    tracing::info!(
        "Server is running at http://{}. Press Ctrl+C to stop.",
        config.bind
    );

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .map_err(|e| anyhow::anyhow!("Server error: {}", e))?;

    tracing::info!("Shutting down server...");

    Ok(())
}

/// Graceful shutdown signal
async fn shutdown_signal() {
    tokio::signal::ctrl_c()
        .await
        .expect("Failed to install CTRL+C signal handler");
}
