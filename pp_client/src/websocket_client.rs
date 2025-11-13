//! WebSocket client for real-time poker game connection.

use anyhow::{Context, Result};
use futures_util::{SinkExt, StreamExt};
use private_poker::entities::GameView;
use serde::Serialize;
use tokio_tungstenite::{connect_async, tungstenite::Message};

/// Client command to send to server
#[allow(dead_code)]
#[derive(Debug, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ClientCommand {
    Join { buy_in: i64 },
    Leave,
    Action { action: ActionData },
    Spectate,
    StopSpectating,
}

/// Action data matching server's ActionData enum
#[derive(Debug, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ActionData {
    Fold,
    Check,
    Call,
    Raise { amount: Option<u32> },
    AllIn,
}

/// WebSocket game client
pub struct WebSocketClient {
    ws_url: String,
}

impl WebSocketClient {
    /// Create a new WebSocket client
    pub fn new(ws_url: String) -> Self {
        Self { ws_url }
    }

    /// Connect to the WebSocket and run the game session
    pub async fn connect_and_play(self) -> Result<()> {
        println!("Connecting to {}...", self.ws_url);

        // Connect to WebSocket
        let (ws_stream, _) = connect_async(&self.ws_url)
            .await
            .context("Failed to connect to WebSocket")?;

        println!("Connected! Receiving table updates...\n");

        let (mut write, mut read) = ws_stream.split();

        // Spawn task to handle incoming messages
        let read_handle = tokio::spawn(async move {
            while let Some(msg) = read.next().await {
                match msg {
                    Ok(Message::Text(text)) => {
                        // Parse and display game view
                        match serde_json::from_str::<GameView>(&text) {
                            Ok(view) => {
                                display_game_view(&view);
                            }
                            Err(e) => {
                                // Might be a response message, try parsing that
                                eprintln!("Failed to parse game view: {}", e);
                            }
                        }
                    }
                    Ok(Message::Close(_)) => {
                        println!("Server closed connection");
                        break;
                    }
                    Err(e) => {
                        eprintln!("WebSocket error: {}", e);
                        break;
                    }
                    _ => {}
                }
            }
        });

        // Handle user input
        let mut stdin = tokio::io::BufReader::new(tokio::io::stdin());
        let mut line = String::new();

        loop {
            use tokio::io::AsyncBufReadExt;

            line.clear();
            match stdin.read_line(&mut line).await {
                Ok(0) => break, // EOF
                Ok(_) => {
                    let input = line.trim();
                    if input.is_empty() {
                        continue;
                    }

                    // Parse and send commands
                    if input == "quit" || input == "exit" {
                        println!("Disconnecting...");
                        break;
                    }

                    match parse_and_send_command(input, &mut write).await {
                        Ok(_) => {
                            // Command sent successfully
                        }
                        Err(e) => {
                            eprintln!("Error: {}", e);
                        }
                    }
                }
                Err(e) => {
                    eprintln!("Error reading input: {}", e);
                    break;
                }
            }
        }

        // Clean up
        let _ = write.close().await;
        read_handle.abort();

        Ok(())
    }
}

/// Parse user input and send command to server
async fn parse_and_send_command<W>(input: &str, write: &mut W) -> Result<()>
where
    W: SinkExt<Message> + Unpin,
    W::Error: std::error::Error + Send + Sync + 'static,
{
    let parts: Vec<&str> = input.split_whitespace().collect();
    if parts.is_empty() {
        return Ok(());
    }

    let command = match parts[0].to_lowercase().as_str() {
        // Game actions
        "fold" => ClientCommand::Action {
            action: ActionData::Fold,
        },
        "check" => ClientCommand::Action {
            action: ActionData::Check,
        },
        "call" => ClientCommand::Action {
            action: ActionData::Call,
        },
        "allin" | "all-in" => ClientCommand::Action {
            action: ActionData::AllIn,
        },
        "raise" => {
            let amount = if parts.len() > 1 {
                parts[1].parse::<u32>().ok()
            } else {
                None
            };
            ClientCommand::Action {
                action: ActionData::Raise { amount },
            }
        }

        // Table management
        "join" => {
            let buy_in = if parts.len() > 1 {
                parts[1].parse::<i64>().unwrap_or(1000)
            } else {
                1000
            };
            ClientCommand::Join { buy_in }
        }
        "leave" => ClientCommand::Leave,
        "spectate" | "watch" => ClientCommand::Spectate,
        "stop" | "unwatch" => ClientCommand::StopSpectating,

        // Help
        "help" | "?" => {
            println!("\nAvailable commands:");
            println!("  Game actions: fold, check, call, raise <amount>, allin");
            println!("  Table: join <buy_in>, leave, spectate, stop");
            println!("  Other: help, quit");
            return Ok(());
        }

        _ => {
            eprintln!(
                "Unknown command: '{}'. Type 'help' for available commands.",
                parts[0]
            );
            return Ok(());
        }
    };

    // Serialize and send command
    let json = serde_json::to_string(&command)?;
    write
        .send(Message::Text(json.into()))
        .await
        .context("Failed to send command")?;

    Ok(())
}

/// Display game view in a readable format
fn display_game_view(view: &GameView) {
    use std::fmt::Write;

    // Clear screen and move cursor to top
    print!("\x1B[2J\x1B[1;1H");

    println!("\n{}", "═".repeat(80));
    println!("POKER TABLE");
    println!("{}", "═".repeat(80));

    // Blinds
    println!("Blinds: ${}/{}", view.blinds.small, view.blinds.big);

    // Board (community cards)
    if !view.board.is_empty() {
        let mut board_str = String::new();
        for card in view.board.iter() {
            write!(&mut board_str, "{} ", format_card(card)).unwrap();
        }
        println!("Board: {}", board_str);
    }

    // Pot
    println!("Pot: ${}", view.pot.size);

    println!("{}", "─".repeat(80));

    // Players
    if !view.players.is_empty() {
        println!("Players:");
        for (i, player) in view.players.iter().enumerate() {
            // Determine position markers
            let mut position_markers = Vec::new();
            if i == view.play_positions.small_blind_idx {
                position_markers.push("SB");
            }
            if i == view.play_positions.big_blind_idx {
                position_markers.push("BB");
            }
            if let Some(next_idx) = view.play_positions.next_action_idx
                && i == next_idx
            {
                position_markers.push("→");
            }

            let position_str = if !position_markers.is_empty() {
                format!(" ({})", position_markers.join("/"))
            } else {
                String::new()
            };

            let cards_str = if !player.cards.is_empty() {
                let mut s = String::new();
                for card in &player.cards {
                    write!(&mut s, "{} ", format_card(card)).unwrap();
                }
                s.trim().to_string()
            } else {
                "??".to_string()
            };

            println!(
                "  {}. {}{} - ${} - {:?}",
                i + 1,
                player.user.name,
                position_str,
                player.user.money,
                player.state
            );

            println!("     Cards: {}", cards_str);
        }
    } else {
        println!("No players at table");
    }

    // Waitlist and spectators
    if !view.waitlist.is_empty() {
        println!("\nWaitlist: {} players", view.waitlist.len());
    }

    if !view.spectators.is_empty() {
        println!("Spectators: {}", view.spectators.len());
    }

    println!("{}", "═".repeat(80));
    println!("Commands: fold, check, call, raise <amount>, allin, quit");
    println!("{}\n", "═".repeat(80));
}

/// Format a card for display
fn format_card(card: &private_poker::entities::Card) -> String {
    let value_str = match card.0 {
        1 | 14 => "A",
        11 => "J",
        12 => "Q",
        13 => "K",
        n => return format!("{}{}", n, suit_char(&card.1)),
    };
    format!("{}{}", value_str, suit_char(&card.1))
}

/// Get suit character
fn suit_char(suit: &private_poker::entities::Suit) -> &'static str {
    match suit {
        private_poker::entities::Suit::Heart => "♥",
        private_poker::entities::Suit::Diamond => "♦",
        private_poker::entities::Suit::Club => "♣",
        private_poker::entities::Suit::Spade => "♠",
        private_poker::entities::Suit::Wild => "*",
    }
}
