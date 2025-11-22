//! A poker client TUI for multi-table poker server.
//!
//! The client connects to an HTTP/WebSocket poker server,
//! authenticates, browses tables, and joins a selected table.

use anyhow::{Context, Result};
use pico_args::Arguments;
use std::io::{self, Write};

#[allow(dead_code)]
mod api_client;
#[allow(dead_code)]
mod app;
#[allow(dead_code)]
mod commands;
#[allow(dead_code)]
mod websocket_client;

use pp_client::{api_client::ApiClient, tui_app::TuiApp, websocket_client::WebSocketClient};

const HELP: &str = "\
Connect to a private poker server

USAGE:
  pp_client [OPTIONS]

OPTIONS:
  --server URL          Server URL  [default: http://localhost:8080]
  --username NAME       Username for login
  --password PASS       Password for login
  --tui                 Use TUI (Terminal UI) mode [default: false]

FLAGS:
  -h, --help            Print help information
";

struct Args {
    server_url: String,
    username: Option<String>,
    password: Option<String>,
    use_tui: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    let mut pargs = Arguments::from_env();

    if pargs.contains(["-h", "--help"]) {
        print!("{HELP}");
        std::process::exit(0);
    }

    let args = Args {
        server_url: pargs
            .value_from_str("--server")
            .unwrap_or_else(|_| "http://localhost:8080".to_string()),
        username: pargs.opt_value_from_str("--username").ok().flatten(),
        password: pargs.opt_value_from_str("--password").ok().flatten(),
        use_tui: pargs.contains("--tui"),
    };

    run(args).await
}

async fn run(args: Args) -> Result<()> {
    let mut api_client = ApiClient::new(args.server_url.clone());

    // Get credentials
    let username = match args.username {
        Some(u) => u,
        None => {
            print!("Username: ");
            io::stdout().flush()?;
            let mut input = String::new();
            io::stdin().read_line(&mut input)?;
            input.trim().to_string()
        }
    };

    let password = match args.password {
        Some(p) => p,
        None => {
            print!("Password: ");
            io::stdout().flush()?;
            let mut input = String::new();
            io::stdin().read_line(&mut input)?;
            input.trim().to_string()
        }
    };

    // Try to login
    println!("Logging in as {}...", username);
    if let Err(e) = api_client.login(username.clone(), password.clone()).await {
        println!("Login failed: {}. Trying to register...", e);
        api_client
            .register(username.clone(), password, username.clone())
            .await
            .context("Failed to register")?;
        println!("Registered successfully!");
    } else {
        println!("Login successful!");
    }

    // List tables
    println!("\nAvailable tables:");
    let tables = api_client
        .list_tables()
        .await
        .context("Failed to list tables")?;

    if tables.is_empty() {
        println!("No tables available!");
        return Ok(());
    }

    for (i, table) in tables.iter().enumerate() {
        println!(
            "  {}. {} - {}/{} players - Blinds: {}/{}{}",
            i + 1,
            table.name,
            table.player_count,
            table.max_players,
            table.small_blind,
            table.big_blind,
            if table.is_private { " (Private)" } else { "" }
        );
    }

    // Select table
    print!("\nSelect table (1-{}): ", tables.len());
    io::stdout().flush()?;
    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    let table_index: usize = input.trim().parse().context("Invalid table number")?;

    if table_index == 0 || table_index > tables.len() {
        anyhow::bail!("Invalid table selection");
    }

    let selected_table = &tables[table_index - 1];
    println!("\nConnecting to table: {}", selected_table.name);

    // Get WebSocket URL
    let ws_url = api_client.get_websocket_url(selected_table.id)?;

    if args.use_tui {
        // TUI mode - create a rich terminal UI
        println!("Starting TUI mode...");

        // Create initial empty view
        use std::collections::{HashSet, VecDeque};
        use std::sync::Arc;
        let initial_view = private_poker::entities::GameView {
            blinds: Arc::new(private_poker::entities::Blinds { small: 0, big: 0 }),
            spectators: Arc::new(HashSet::new()),
            waitlist: Arc::new(VecDeque::new()),
            open_seats: Arc::new(VecDeque::new()),
            players: Vec::new(),
            board: Arc::new(Vec::new()),
            pot: Arc::new(private_poker::entities::PotView { size: 0 }),
            play_positions: Arc::new(private_poker::entities::PlayPositions::default()),
        };

        // Initialize terminal
        let terminal = ratatui::init();

        // Create and run TUI app
        let tui_app = TuiApp::new(
            username.clone(),
            selected_table.name.clone(),
            selected_table.id,
            api_client,
            initial_view,
        );

        let result = tui_app.run(ws_url, terminal).await;

        // Restore terminal
        ratatui::restore();

        result?;
    } else {
        // CLI mode - simple text-based client
        let ws_client = WebSocketClient::new(ws_url);
        ws_client.connect_and_play().await?;
    }

    println!("\nDisconnected from table.");
    Ok(())
}
