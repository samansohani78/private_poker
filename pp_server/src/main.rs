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
    table::{TableConfig, TableManager, TableSpeed, BotDifficulty},
    wallet::WalletManager,
};

const HELP: &str = "\
Run a multi-table private poker server

USAGE:
  pp_server [OPTIONS]

OPTIONS:
  --bind       IP:PORT     Server socket bind address  [default: 127.0.0.1:6969]
  --db-url     URL         Database connection string  [default: postgres://poker_test:test_password@localhost/poker_test]
  --tables     N           Number of tables to create  [default: 1]

FLAGS:
  -h, --help               Print help information
";

struct Args {
    bind: SocketAddr,
    database_url: String,
    num_tables: usize,
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    let mut pargs = Arguments::from_env();

    // Help has a higher priority and should be handled separately.
    if pargs.contains(["-h", "--help"]) {
        print!("{HELP}");
        std::process::exit(0);
    }

    let args = Args {
        bind: pargs
            .value_from_str("--bind")
            .unwrap_or("127.0.0.1:6969".parse()?),
        database_url: pargs
            .value_from_str("--db-url")
            .unwrap_or_else(|_| {
                std::env::var("DATABASE_URL").unwrap_or_else(|_| {
                    "postgres://poker_test:test_password@localhost/poker_test".to_string()
                })
            }),
        num_tables: pargs.value_from_str("--tables").unwrap_or(1),
    };

    // Catching signals for exit.
    set_handler(|| std::process::exit(0))?;

    env_logger::builder().format_target(false).init();
    info!("Starting multi-table poker server at {}", args.bind);

    // Initialize database
    info!("Connecting to database: {}", args.database_url);
    let db_config = DatabaseConfig {
        database_url: args.database_url,
        max_connections: 10,
        min_connections: 2,
        connection_timeout_secs: 5,
        idle_timeout_secs: 300,
        max_lifetime_secs: 1800,
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

    // Create initial tables
    for i in 0..args.num_tables {
        let config = TableConfig {
            name: format!("Table {}", i + 1),
            max_players: 9,
            small_blind: 10,
            big_blind: 20,
            min_buy_in_bb: 50,
            max_buy_in_bb: 200,
            absolute_chip_cap: 100_000,
            top_up_cooldown_hands: 20,
            speed: TableSpeed::Normal,
            bots_enabled: true,
            target_bot_count: 6,
            bot_difficulty: BotDifficulty::Standard,
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

    info!("Server is running at http://{}. Press Ctrl+C to stop.", args.bind);

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
