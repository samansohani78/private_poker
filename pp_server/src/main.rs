//! Multi-table poker server using async actor model.
//!
//! This server spawns TableActor instances managed by TableManager,
//! with database-backed authentication and wallet systems.

mod api;

use std::net::SocketAddr;
use std::sync::Arc;

use anyhow::Error;
use ctrlc::set_handler;
use log::info;
use pico_args::Arguments;
use private_poker::{
    auth::AuthManager,
    db::{Database, DatabaseConfig},
    table::{BotDifficulty, TableConfig, TableManager, TableSpeed},
    wallet::WalletManager,
};

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
  DATABASE_URL             PostgreSQL connection string
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
    // Load .env file if it exists
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

    env_logger::builder().format_target(false).init();
    info!("Starting multi-table poker server at {}", args.bind);

    // Initialize database
    info!("Connecting to database: {}", args.database_url);
    let db_config = DatabaseConfig {
        database_url: args.database_url,
        max_connections: std::env::var("DB_MAX_CONNECTIONS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(100),
        min_connections: std::env::var("DB_MIN_CONNECTIONS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(5),
        connection_timeout_secs: std::env::var("DB_CONNECTION_TIMEOUT_SECS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(5),
        idle_timeout_secs: std::env::var("DB_IDLE_TIMEOUT_SECS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(300),
        max_lifetime_secs: std::env::var("DB_MAX_LIFETIME_SECS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(1800),
    };

    let db = Database::new(&db_config)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to connect to database: {}", e))?;

    info!("Database connected successfully");

    // Create managers
    let pool = Arc::new(db.pool().clone());
    let wallet_manager = Arc::new(WalletManager::new(pool.clone()));
    let table_manager = Arc::new(TableManager::new(pool.clone(), wallet_manager.clone()));

    let jwt_secret = std::env::var("JWT_SECRET")
        .unwrap_or_else(|_| "default_jwt_secret_change_in_production".to_string());
    let pepper = std::env::var("PASSWORD_PEPPER")
        .unwrap_or_else(|_| "default_pepper_change_in_production".to_string());

    let auth_manager = Arc::new(AuthManager::new(pool.clone(), pepper, jwt_secret));

    info!("Creating {} initial table(s)...", args.num_tables);

    // Parse bot difficulty from env
    let bot_difficulty = std::env::var("DEFAULT_BOT_DIFFICULTY")
        .ok()
        .and_then(|v| match v.to_lowercase().as_str() {
            "easy" => Some(BotDifficulty::Easy),
            "standard" => Some(BotDifficulty::Standard),
            "tag" => Some(BotDifficulty::Tag),
            _ => None,
        })
        .unwrap_or(BotDifficulty::Standard);

    // Create initial tables
    for i in 0..args.num_tables {
        let config = TableConfig {
            name: format!("Table {}", i + 1),
            max_players: std::env::var("TABLE_MAX_PLAYERS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(9),
            small_blind: std::env::var("TABLE_SMALL_BLIND")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(10),
            big_blind: std::env::var("TABLE_BIG_BLIND")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(20),
            min_buy_in_bb: std::env::var("TABLE_MIN_BUY_IN_BB")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(50),
            max_buy_in_bb: std::env::var("TABLE_MAX_BUY_IN_BB")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(200),
            absolute_chip_cap: std::env::var("ABSOLUTE_CHIP_CAP")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(100_000),
            top_up_cooldown_hands: std::env::var("TABLE_TOP_UP_COOLDOWN_HANDS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(20),
            speed: TableSpeed::Normal,
            bots_enabled: std::env::var("BOTS_ENABLED")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(true),
            target_bot_count: std::env::var("TARGET_BOT_COUNT")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(6),
            bot_difficulty,
            is_private: false,
            passphrase_hash: None,
            invite_token: None,
            invite_expires_at: None,
        };

        match table_manager.create_table(config, None).await {
            Ok(table_id) => {
                info!("✓ Created table {} with ID {}", i + 1, table_id);
            }
            Err(e) => {
                log::error!("Failed to create table {}: {}", i + 1, e);
            }
        }
    }

    let active_count = table_manager.active_table_count().await;
    info!("Server ready with {} active table(s)", active_count);

    // List tables
    match table_manager.list_tables().await {
        Ok(tables) => {
            info!("Active tables:");
            for table in tables {
                info!(
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
            log::error!("Failed to list tables: {}", e);
        }
    }

    // Create API state
    let api_state = api::AppState {
        auth_manager,
        table_manager,
        wallet_manager,
    };

    // Create router
    let app = api::create_router(api_state);

    // Start HTTP server
    info!("Starting HTTP/WebSocket server on {}", args.bind);
    let listener = tokio::net::TcpListener::bind(args.bind)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to bind to {}: {}", args.bind, e))?;

    info!(
        "Server is running at http://{}. Press Ctrl+C to stop.",
        args.bind
    );

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .map_err(|e| anyhow::anyhow!("Server error: {}", e))?;

    info!("Shutting down server...");

    Ok(())
}

/// Graceful shutdown signal
async fn shutdown_signal() {
    tokio::signal::ctrl_c()
        .await
        .expect("Failed to install CTRL+C signal handler");
}
