//! TUI application for WebSocket-based poker client.
//!
//! This module provides a rich terminal UI using ratatui that connects
//! to the poker server via WebSocket for real-time game updates.

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use futures_util::{SinkExt, StreamExt};
use private_poker::{
    entities::{Card, GameView, Suit, Username},
    functional,
};
use ratatui::{
    DefaultTerminal, Frame,
    crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers},
    layout::{Alignment, Constraint, Flex, Layout, Margin, Position},
    style::{Style, Stylize},
    symbols::scrollbar,
    text::{Line, Span, Text},
    widgets::{
        Block, Cell, Clear, List, ListDirection, ListItem, Padding, Paragraph, Row, Scrollbar,
        ScrollbarOrientation, Table, block,
    },
};
use serde::Serialize;
use std::time::{Duration, Instant};
use tokio::sync::mpsc;
use tokio_tungstenite::{connect_async, tungstenite::Message};

mod widgets;

use widgets::{ScrollableList, UserInput};

const HELP: &str = "\
all-in
        Go all-in, betting all your money on the hand.
call
        Match the investment required to stay in the hand.
check
        Check, voting to move to the next card reveal(s).
fold
        Fold, forfeiting your hand.
join <buy_in>
        Join the table with the specified buy-in amount.
leave
        Leave the table.
raise <amount>
        Raise the investment required to stay in the hand. Entering without a value
        defaults to the min raise amount. Entering AMOUNT will raise by AMOUNT, but
        AMOUNT must be >= the min raise.
show
        Show your hand. Only possible during the showdown.
spectate
        Join as a spectator.
stop
        Stop spectating.
";
const MAX_LOG_RECORDS: usize = 1024;
const POLL_TIMEOUT: Duration = Duration::from_millis(100);

/// Client command matching WebSocket protocol
#[derive(Debug, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum ClientCommand {
    Join { buy_in: i64 },
    Leave,
    Action { action: ActionData },
    Spectate,
    StopSpectating,
}

/// Action data for game moves
#[derive(Debug, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum ActionData {
    Fold,
    Check,
    Call,
    Raise { amount: Option<u32> },
    AllIn,
}

fn make_board_spans(view: &GameView) -> Vec<Span<'_>> {
    (!view.board.is_empty())
        .then(|| {
            std::iter::once(" board: ".into()).chain(
                view.board
                    .iter()
                    .flat_map(|card| vec![make_card_span(card), "  ".into()]),
            )
        })
        .into_iter()
        .flatten()
        .collect()
}

fn make_card_span(card: &Card) -> Span<'static> {
    let Card(.., suit) = card;
    let repr = card.to_string();
    match suit {
        Suit::Club => Span::styled(repr, Style::default().light_green()),
        Suit::Diamond => Span::styled(repr, Style::default().light_blue()),
        Suit::Heart => Span::styled(repr, Style::default().light_red()),
        Suit::Spade => Span::raw(repr),
        Suit::Wild => Span::styled(repr, Style::default().light_magenta()),
    }
}

fn make_user_row(username: &Username, user: &private_poker::entities::User) -> Row<'static> {
    let mut row = Row::new(vec![
        Cell::new(Text::from(user.name.to_string()).alignment(Alignment::Left)),
        Cell::new(Text::from(format!("${}", user.money)).alignment(Alignment::Right)),
    ]);

    if username == &user.name {
        row = row.bold().white();
    }

    row
}

#[derive(Clone)]
#[allow(dead_code)]
enum RecordKind {
    Ack,
    Alert,
    Error,
    Game,
    You,
}

#[derive(Clone, Copy, PartialEq)]
enum ConnectionStatus {
    Connected,
    Disconnected,
}

/// A timestamped terminal message with an importance label to help
/// direct user attention.
#[derive(Clone)]
struct Record {
    datetime: DateTime<Utc>,
    kind: RecordKind,
    content: String,
}

impl Record {
    fn new(kind: RecordKind, content: String) -> Self {
        Self {
            datetime: Utc::now(),
            kind,
            content,
        }
    }
}

impl From<Record> for ListItem<'_> {
    fn from(val: Record) -> Self {
        let repr = match val.kind {
            RecordKind::Ack => "ACK".light_blue(),
            RecordKind::Alert => "ALERT".light_magenta(),
            RecordKind::Error => "ERROR".light_red(),
            RecordKind::Game => "GAME".light_yellow(),
            RecordKind::You => "YOU".light_green(),
        };

        let msg = vec![
            format!("[{} ", val.datetime.format("%H:%M:%S")).into(),
            Span::styled(format!("{repr:5}"), repr.style),
            format!("]: {}", val.content).into(),
        ];

        let content = Line::from(msg);
        ListItem::new(content)
    }
}

/// Provides turn time remaining warnings at specific intervals when it's
/// the player's turn.
struct TurnWarnings {
    t: Instant,
    idx: usize,
    warnings: [u8; 8],
}

impl TurnWarnings {
    /// Check for a new warning.
    fn check(&mut self) -> Option<u8> {
        if self.idx > 0 {
            let ceiling = self.warnings.last().expect("warnings should be immutable");
            let warning = self.warnings[self.idx - 1];
            let dt = self.t.elapsed();
            let remaining = ceiling.saturating_sub(dt.as_secs() as u8);
            if remaining <= warning {
                self.idx -= 1;
                return Some(warning);
            }
        }
        None
    }

    #[allow(dead_code)]
    fn clear(&mut self) {
        self.idx = 0;
    }

    fn new() -> Self {
        Self {
            t: Instant::now(),
            idx: 0,
            warnings: [1, 2, 3, 4, 5, 10, 20, 30],
        }
    }

    fn reset(&mut self) {
        self.t = Instant::now();
        self.idx = self.warnings.len();
    }
}

/// TUI App state
pub struct TuiApp {
    username: Username,
    table_name: String,
    /// Whether to display the help menu window
    show_help_menu: bool,
    /// Helps scroll through the help menu window if the terminal is small
    help_handle: ScrollableList,
    /// History of recorded messages
    log_handle: ScrollableList,
    /// Current value of the input box
    user_input: UserInput,
    /// Connection status indicator
    connection_status: ConnectionStatus,
    /// Current game view
    view: GameView,
    /// Turn warnings
    turn_warnings: TurnWarnings,
}

impl TuiApp {
    pub fn new(username: String, table_name: String, initial_view: GameView) -> Self {
        // Fill help menu with help text lines
        let mut help_handle = ScrollableList::new(MAX_LOG_RECORDS);
        help_handle.push("".into());
        for line in HELP.lines() {
            help_handle.push(line.into());
        }
        help_handle.push("".into());
        help_handle.jump_to_first();

        Self {
            username: Username::new(&username),
            table_name,
            show_help_menu: false,
            help_handle,
            log_handle: ScrollableList::new(MAX_LOG_RECORDS),
            user_input: UserInput::new(),
            connection_status: ConnectionStatus::Connected,
            view: initial_view,
            turn_warnings: TurnWarnings::new(),
        }
    }

    /// Parse and create a client command from user input
    fn parse_command(&self, input: &str) -> Result<ClientCommand> {
        let parts: Vec<&str> = input.split_whitespace().collect();
        if parts.is_empty() {
            anyhow::bail!("Empty command");
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
                    parts[1]
                        .parse::<i64>()
                        .context("Invalid buy-in amount")?
                } else {
                    1000
                };
                ClientCommand::Join { buy_in }
            }
            "leave" => ClientCommand::Leave,
            "spectate" | "watch" => ClientCommand::Spectate,
            "stop" | "unwatch" => ClientCommand::StopSpectating,

            _ => {
                anyhow::bail!("Unknown command: '{}'. Press Tab for help.", parts[0]);
            }
        };

        Ok(command)
    }

    /// Handle user input and send command
    fn handle_command(
        &mut self,
        user_input: &str,
        tx: &mpsc::UnboundedSender<ClientCommand>,
    ) -> Result<()> {
        match self.parse_command(user_input) {
            Ok(command) => {
                tx.send(command)?;
                let record = Record::new(RecordKind::You, user_input.to_string());
                self.log_handle.push(record.into());
            }
            Err(e) => {
                let record = Record::new(RecordKind::Error, e.to_string());
                self.log_handle.push(record.into());
            }
        }
        Ok(())
    }

    /// Update game view
    fn update_view(&mut self, new_view: GameView) {
        self.view = new_view;
    }

    /// Add log message
    fn add_log(&mut self, kind: RecordKind, content: String) {
        let record = Record::new(kind, content);
        self.log_handle.push(record.into());
    }

    /// Render the spectators table
    fn draw_spectators(&self, frame: &mut Frame, area: ratatui::layout::Rect) {
        let mut spectators = Vec::from_iter(self.view.spectators.iter());
        spectators.sort_unstable();
        let spectators = Table::new(
            spectators
                .iter()
                .map(|user| make_user_row(&self.username, user)),
            [Constraint::Percentage(50), Constraint::Percentage(50)],
        )
        .block(
            Block::bordered()
                .padding(Padding::uniform(1))
                .title(" spectators  "),
        );
        frame.render_widget(spectators, area);
    }

    /// Render the waitlist table
    fn draw_waitlist(&self, frame: &mut Frame, area: ratatui::layout::Rect) {
        let waitlisters = Table::new(
            self.view
                .waitlist
                .iter()
                .map(|user| make_user_row(&self.username, user)),
            [Constraint::Percentage(50), Constraint::Percentage(50)],
        )
        .block(
            Block::bordered()
                .padding(Padding::uniform(1))
                .title(" waitlisters  "),
        );
        frame.render_widget(waitlisters, area);
    }

    /// Create a table row for a single player
    fn make_player_row(&self, player_idx: usize) -> Row<'static> {
        let player = &self.view.players[player_idx];

        // Indicator if it's the player's move
        let move_repr = if self.view.play_positions.next_action_idx == Some(player_idx) {
            "→"
        } else {
            ""
        };

        // Indicator for blind position
        let button_repr = match player_idx {
            idx if idx == self.view.play_positions.big_blind_idx => "BB",
            idx if idx == self.view.play_positions.small_blind_idx => "SB",
            _ => "",
        };

        // Build the row cells
        let mut row = vec![
            Cell::new(Text::from(move_repr).alignment(Alignment::Center)),
            Cell::new(Text::from(button_repr).alignment(Alignment::Left)),
            Cell::new(Text::from(player.user.name.to_string()).alignment(Alignment::Left)),
            Cell::new(Text::from(format!("${}", player.user.money)).alignment(Alignment::Right)),
            Cell::new(Text::from(player.state.to_string()).alignment(Alignment::Center)),
        ];

        // Add player cards
        for card_idx in 0..2 {
            let card_repr = player
                .cards
                .get(card_idx)
                .map_or_else(|| "".into(), make_card_span);
            row.push(Cell::new(Text::from(card_repr).alignment(Alignment::Right)));
        }

        // Add player's best hand
        let hand_repr = if !player.cards.is_empty() {
            let mut cards = self.view.board.as_ref().clone();
            cards.extend(player.cards.clone());
            functional::prepare_hand(&mut cards);
            let hand = functional::eval(&cards);
            hand.first()
                .map_or_else(String::new, |subhand| format!("({})", subhand.rank))
        } else {
            String::new()
        };
        row.push(Cell::new(Text::from(hand_repr).alignment(Alignment::Right)));

        // Highlight current player
        let mut row = Row::new(row);
        if self.username == player.user.name {
            row = row.bold().white();
        }
        row
    }

    /// Render the main game table with players
    fn draw_table(&self, frame: &mut Frame, area: ratatui::layout::Rect) {
        let table = Table::new(
            (0..self.view.players.len()).map(|idx| self.make_player_row(idx)),
            [
                Constraint::Max(3),
                Constraint::Fill(1),
                Constraint::Fill(2),
                Constraint::Fill(2),
                Constraint::Fill(2),
                Constraint::Fill(1),
                Constraint::Fill(1),
                Constraint::Fill(1),
            ],
        )
        .block(
            block::Block::bordered()
                .padding(Padding::uniform(1))
                .title_top(make_board_spans(&self.view))
                .title_bottom(format!(
                    " blinds: {}  pot: {}  ",
                    self.view.blinds, self.view.pot
                )),
        );
        frame.render_widget(table, area);
    }

    /// Render the log/history window with scrollbar
    fn draw_log(&mut self, frame: &mut Frame, area: ratatui::layout::Rect) {
        let log_records = self.log_handle.list_items.clone();
        let log_records = List::new(log_records)
            .direction(ListDirection::BottomToTop)
            .block(block::Block::bordered().title(" history  "));
        frame.render_stateful_widget(log_records, area, &mut self.log_handle.list_state);

        // Render log window scrollbar
        frame.render_stateful_widget(
            Scrollbar::new(ScrollbarOrientation::VerticalRight)
                .symbols(scrollbar::VERTICAL)
                .begin_symbol(None)
                .end_symbol(None),
            area.inner(Margin {
                vertical: 1,
                horizontal: 1,
            }),
            &mut self.log_handle.scroll_state,
        );
    }

    /// Render the user input area
    fn draw_user_input(&self, frame: &mut Frame, area: ratatui::layout::Rect) {
        let user_input = Paragraph::new(self.user_input.value.as_str())
            .style(Style::default())
            .block(
                block::Block::bordered().title(
                    format!(" {}@{}  ", self.username, self.table_name).light_green(),
                ),
            );
        frame.render_widget(user_input, area);
        frame.set_cursor_position(Position::new(
            area.x + self.user_input.char_idx as u16 + 1,
            area.y + 1,
        ));
    }

    /// Render the help/status bar at the bottom
    fn draw_help_bar(&self, frame: &mut Frame, area: ratatui::layout::Rect) {
        let status_indicator = match self.connection_status {
            ConnectionStatus::Connected => "● Connected".green(),
            ConnectionStatus::Disconnected => "● Disconnected".red(),
        };

        let help_message = vec![
            status_indicator,
            " | press ".into(),
            "Tab".bold().white(),
            " to view help, press ".into(),
            "Enter".bold().white(),
            " to record a command, or press ".into(),
            "Esc".bold().white(),
            " to exit".into(),
        ];
        let help_message = Paragraph::new(Line::from(help_message));
        frame.render_widget(help_message, area);
    }

    /// Render the help menu overlay
    fn draw_help_menu(&mut self, frame: &mut Frame) {
        let vertical = Layout::vertical([Constraint::Max(29)]).flex(Flex::Center);
        let horizontal = Layout::horizontal([Constraint::Max(92)]).flex(Flex::Center);
        let [help_menu_area] = vertical.areas(frame.area());
        let [help_menu_area] = horizontal.areas(help_menu_area);
        frame.render_widget(Clear, help_menu_area);

        // Render help text
        let help_items = self.help_handle.list_items.clone();
        let help_items = List::new(help_items)
            .direction(ListDirection::BottomToTop)
            .block(block::Block::bordered().title(" commands  "));
        frame.render_stateful_widget(
            help_items,
            help_menu_area,
            &mut self.help_handle.list_state,
        );

        // Render help scrollbar
        frame.render_stateful_widget(
            Scrollbar::new(ScrollbarOrientation::VerticalRight)
                .symbols(scrollbar::VERTICAL)
                .begin_symbol(None)
                .end_symbol(None),
            help_menu_area.inner(Margin {
                vertical: 1,
                horizontal: 1,
            }),
            &mut self.help_handle.scroll_state,
        );
    }

    /// Main draw function - orchestrates rendering of all UI components
    fn draw(&mut self, frame: &mut Frame) {
        // Define the main layout structure
        let window = Layout::vertical([
            Constraint::Min(6),    // Top area (view + log)
            Constraint::Length(3), // User input area
            Constraint::Length(1), // Help bar
        ]);
        let [top_area, user_input_area, help_area] = window.areas(frame.area());

        // Split top area into view and log
        let [view_area, log_area] =
            Layout::vertical([Constraint::Percentage(55), Constraint::Percentage(45)])
                .areas(top_area);

        // Split view area into lobby and table
        let [lobby_area, table_area] =
            Layout::horizontal([Constraint::Percentage(40), Constraint::Percentage(60)])
                .areas(view_area);

        // Split lobby into spectators and waitlisters
        let [spectator_area, waitlister_area] =
            Layout::horizontal([Constraint::Percentage(50), Constraint::Percentage(50)])
                .areas(lobby_area);

        // Render all components
        self.draw_spectators(frame, spectator_area);
        self.draw_waitlist(frame, waitlister_area);
        self.draw_table(frame, table_area);
        self.draw_log(frame, log_area);
        self.draw_user_input(frame, user_input_area);
        self.draw_help_bar(frame, help_area);

        // Render help menu overlay if active
        if self.show_help_menu {
            self.draw_help_menu(frame);
        }
    }

    /// Run the TUI application
    pub async fn run(mut self, ws_url: String, mut terminal: DefaultTerminal) -> Result<()> {
        // Connect to WebSocket
        let (ws_stream, _) = connect_async(&ws_url)
            .await
            .context("Failed to connect to WebSocket")?;

        let (mut write, mut read) = ws_stream.split();

        // Channel for sending commands to WebSocket
        let (tx_command, mut rx_command) = mpsc::unbounded_channel::<ClientCommand>();

        // Spawn task to handle outgoing messages
        let write_handle = tokio::spawn(async move {
            while let Some(command) = rx_command.recv().await {
                if let Ok(json) = serde_json::to_string(&command)
                    && write.send(Message::Text(json.into())).await.is_err() {
                        break;
                    }
            }
        });

        // Channel for incoming game views
        let (tx_view, mut rx_view) = mpsc::unbounded_channel::<GameView>();
        let (tx_error, mut rx_error) = mpsc::unbounded_channel::<String>();

        // Spawn task to handle incoming messages
        let read_handle = tokio::spawn(async move {
            while let Some(msg) = read.next().await {
                match msg {
                    Ok(Message::Text(text)) => {
                        if let Ok(view) = serde_json::from_str::<GameView>(&text) {
                            let _ = tx_view.send(view);
                        }
                    }
                    Ok(Message::Close(_)) => {
                        let _ = tx_error.send("Server closed connection".to_string());
                        break;
                    }
                    Err(e) => {
                        let _ = tx_error.send(format!("WebSocket error: {}", e));
                        break;
                    }
                    _ => {}
                }
            }
        });

        // Main UI loop
        loop {
            terminal.draw(|frame| self.draw(frame))?;

            // Check for keyboard input
            if event::poll(POLL_TIMEOUT)?
                && let Event::Key(KeyEvent {
                    code,
                    modifiers,
                    kind,
                    ..
                }) = event::read()?
                    && kind == KeyEventKind::Press {
                        match modifiers {
                            KeyModifiers::CONTROL => match code {
                                KeyCode::Home => self.log_handle.jump_to_first(),
                                KeyCode::End => self.log_handle.jump_to_last(),
                                _ => {}
                            },
                            KeyModifiers::NONE => match code {
                                KeyCode::Enter => {
                                    let user_input = self.user_input.submit();
                                    self.handle_command(&user_input, &tx_command)?;
                                }
                                KeyCode::Char(to_insert) => self.user_input.input(to_insert),
                                KeyCode::Backspace => self.user_input.backspace(),
                                KeyCode::Delete => self.user_input.delete(),
                                KeyCode::Left => self.user_input.move_left(),
                                KeyCode::Right => self.user_input.move_right(),
                                KeyCode::Up => {
                                    if self.show_help_menu {
                                        self.help_handle.move_up();
                                    } else {
                                        self.log_handle.move_up();
                                    }
                                }
                                KeyCode::Down => {
                                    if self.show_help_menu {
                                        self.help_handle.move_down();
                                    } else {
                                        self.log_handle.move_down();
                                    }
                                }
                                KeyCode::Home => self.user_input.jump_to_first(),
                                KeyCode::End => self.user_input.jump_to_last(),
                                KeyCode::Tab => self.show_help_menu = !self.show_help_menu,
                                KeyCode::Esc => {
                                    write_handle.abort();
                                    read_handle.abort();
                                    return Ok(());
                                }
                                _ => {}
                            },
                            _ => {}
                        }
                    }

            // Check for new game views
            if let Ok(new_view) = rx_view.try_recv() {
                self.update_view(new_view);
                // Check if it's our turn
                if let Some(next_idx) = self.view.play_positions.next_action_idx
                    && let Some(player) = self.view.players.get(next_idx)
                        && player.user.name == self.username {
                            self.turn_warnings.reset();
                            self.add_log(RecordKind::Alert, "It's your turn!".to_string());
                        }
            }

            // Check for connection errors
            if let Ok(error_msg) = rx_error.try_recv() {
                self.connection_status = ConnectionStatus::Disconnected;
                self.add_log(RecordKind::Error, error_msg);
                terminal.draw(|frame| self.draw(frame))?;
                tokio::time::sleep(Duration::from_secs(2)).await;
                write_handle.abort();
                read_handle.abort();
                return Ok(());
            }

            // Check for turn warnings
            if let Some(warning) = self.turn_warnings.check() {
                self.add_log(
                    RecordKind::Alert,
                    format!("{warning:>2} second(s) left"),
                );
            }
        }
    }
}
