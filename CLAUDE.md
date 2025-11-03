# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Overview

This is a Rust-based poker application implementing Texas Hold'em as a finite state machine (FSM). The project consists of a core library (`private_poker`) and three binaries: server (`pp_server`), client (`pp_client`), and bots (`pp_bots`). The server-client architecture uses TCP with custom binary message protocol (bincode serialization).

## Development Commands

### Building and Testing
```bash
# Build all workspace members (optimized for size in release)
cargo build
cargo build --release

# Run tests (includes unit and integration tests)
cargo test

# Run tests for a specific package
cargo test -p private_poker
cargo test -p pp_server
```

### Linting and Formatting
```bash
# Run clippy with CI settings (treats warnings as errors)
cargo clippy -- -D warnings

# Check formatting
cargo fmt --all -- --check

# Auto-format code
cargo fmt --all
```

### Running Binaries

Server:
```bash
# Run server with logging
RUST_LOG=info cargo run --bin pp_server -r -- --bind <host>
```

Client:
```bash
# Run client
cargo run --bin pp_client -r -- <username> --connect <host>
```

Bots:
```bash
# Run bots manager
cargo run --bin pp_bots -r -- --connect <host>
```

### Docker Development
```bash
# Build Docker image
docker build -t poker .

# Run container
docker run -d --name poker -p <port>:22 --rm poker

# Create user in container
docker exec poker ./create_user <username>

# Delete user
docker exec poker ./delete_user <username>

# Manage bots in tmux session
docker exec poker tmux new-session -d -s bots ./pp_bots
docker exec -it poker tmux attach -t bots
```

## Architecture

### Finite State Machine Design

The core poker game (`private_poker/src/game.rs`) is implemented as a type-safe FSM using `enum_dispatch`. The `PokerState` enum represents 14 distinct game phases:

1. **Lobby** - Waiting for players to join
2. **SeatPlayers** - Assigning table positions
3. **MoveButton** - Rotating dealer button
4. **CollectBlinds** - Collecting small/big blinds
5. **Deal** - Dealing hole cards
6. **TakeAction** - Player betting rounds
7. **Flop** - Dealing first 3 community cards
8. **Turn** - Dealing 4th community card
9. **River** - Dealing 5th community card
10. **ShowHands** - Revealing cards
11. **DistributePot** - Distributing winnings
12. **RemovePlayers** - Removing broke/disconnected players
13. **UpdateBlinds** - Adjusting blind levels
14. **BootPlayers** - Kicking voted players

Each state is a `Game<T>` struct wrapping game data and state-specific logic. State transitions are handled by implementing `Into<PokerState>` for each state type.

### Trait-Based Behavior with enum_dispatch

Three core traits define game behavior, dispatched via `enum_dispatch` for zero-cost abstractions:

- **`GameStateManagement`**: Event draining and view generation (all states)
- **`PhaseDependentUserManagement`**: User removal, kicks, money resets (behavior varies by phase)
- **`PhaseIndependentUserManagement`**: Voting, joining, waitlisting (consistent across phases)

This design ensures type-safe state transitions while maintaining unified interfaces.

### Networking Layer

**Server** (`private_poker/src/net/server.rs`):
- Multi-threaded: separate game logic thread + event loop thread using `mio` for async I/O
- Token-based client identification
- Message passing via `mpsc` channels between threads
- Configurable timeouts: action (30s), connect (5s), poll (1s), step (5s)
- Binary protocol using length-prefixed messages (see `utils.rs`)

**Client** (`private_poker/src/net/client.rs`):
- Synchronous TCP client
- Sends `ClientMessage` (username + command)
- Receives `ServerMessage` (acks, errors, game views, turn signals)

**Message Types** (`private_poker/src/net/messages.rs`):
- `ClientMessage`: User commands (Connect, TakeAction, CastVote, etc.)
- `ServerMessage`: Server responses (Ack, GameView, TurnSignal, errors)
- All serialized with `bincode` for compact binary format

### Hand Evaluation

**Functional poker logic** (`private_poker/src/game/functional.rs`):
- `eval()`: Evaluates any number of cards, returns best 5-card hand as `Vec<SubHand>`
- `argmax()`: Compares hands and returns indices of winner(s)
- Uses `SubHand` ranking system (Rank + Values for tiebreaking)
- Efficient algorithms using `BTreeMap`, `BTreeSet`, `BinaryHeap` for card grouping

### Game Entities

Key types in `private_poker/src/game/entities.rs`:
- **Card**: `(Value, Suit)` tuple where Value is `u8` (1-14, ace low/high)
- **Deck**: 52-card deck with shuffle and deal methods
- **Player**: User, cards, chips, betting state, position
- **Pot**: Side pots with eligibility tracking
- **User**: Username + money balance
- **Action**: Fold, Check, Call, Raise, AllIn
- **Vote**: Kick, Reset (consensus-based game management)

## Project Structure

```
.
├── private_poker/     # Core library (FSM, hand evaluation, networking primitives)
│   ├── src/game/      # Game logic FSM
│   │   ├── game.rs        # State machine, 3000+ lines
│   │   ├── entities.rs    # Cards, players, pots, actions
│   │   ├── functional.rs  # Hand evaluation algorithms
│   │   └── constants.rs   # Game constants
│   └── src/net/       # Networking layer
│       ├── server.rs      # TCP server with mio
│       ├── client.rs      # TCP client
│       ├── messages.rs    # Protocol messages
│       └── utils.rs       # Binary serialization helpers
├── pp_server/         # Server binary (thin wrapper around library)
├── pp_client/         # Client binary with TUI (ratatui-based)
├── pp_bots/           # Bot management binary
└── pp_admin/          # Docker user management scripts
```

## Important Notes

### Testing
- Integration tests:
  - `private_poker/tests/client_server.rs` - Full client-server interactions
  - `private_poker/tests/game_flow_integration.rs` - Complete game flow testing
- Property-based tests: `private_poker/tests/hand_evaluation_proptest.rs` uses `proptest` to verify hand evaluation correctness across random inputs
- Unit tests embedded in source files with `#[cfg(test)]`
- Use `cargo test` to run all tests
- Property test regressions stored in `proptest-regressions/` (gitignored)

### Release Profile
The workspace uses aggressive size optimizations:
- `opt-level = "z"` (optimize for size)
- `lto = true` (link-time optimization)
- `strip = true` (remove debug symbols)
- `codegen-units = 1` (better optimization, slower compile)
- `panic = "abort"` (smaller binary)

### Dependencies
- `mio`: Async I/O for server event loop
- `bincode`: Binary serialization for network protocol
- `enum_dispatch`: Zero-cost trait dispatch for FSM
- `ratatui`: TUI framework (client only)
- `serde`: Serialization traits
- `thiserror`: Error type macros
- `proptest`: Property-based testing framework (dev dependency)

### Logging
Server uses `env_logger` controlled by `RUST_LOG` environment variable. Set to `info`, `debug`, or `trace` for different verbosity levels.

### Non-goals
Per the README, the following are explicitly out of scope:
- Server orchestration or scaling
- Persistent storage or backups
- UIs beyond TUI

---

## Code Quality Guidelines

### General Principles

1. **Favor clarity over cleverness**: Code should be self-documenting where possible
2. **Keep functions focused**: Each function should have a single, clear responsibility
3. **Use the type system**: Leverage Rust's type system for compile-time guarantees
4. **Test thoroughly**: Every new feature should include appropriate tests

### Function Size Limits

- **Maximum function length**: 80 lines
- **Target length**: 20-40 lines for most functions
- If a function exceeds 80 lines, extract logical sub-components into helper functions

**Good Example** (from Sprint 5 refactoring):
```rust
// Before: 235-line monolithic draw() function
fn draw(&mut self, view: &GameView, frame: &mut Frame) { /* ... 235 lines ... */ }

// After: Orchestration with focused helpers
fn draw(&mut self, view: &GameView, frame: &mut Frame) {
    self.draw_spectators(view, frame, spectator_area);
    self.draw_waitlist(view, frame, waitlister_area);
    self.draw_table(view, frame, table_area);
    // ... etc
}

fn draw_spectators(&self, view: &GameView, frame: &mut Frame, area: Rect) {
    // 14 focused lines for spectator rendering
}
```

### Module Organization

1. **Separate concerns**: Command parsing, UI rendering, and business logic should be in different modules
2. **Public API**: Only expose what's necessary; keep implementation details private
3. **Module documentation**: Every public module should have module-level documentation (`//!`)

**Good Example** (from Sprint 5):
```rust
// pp_client/src/commands.rs - Dedicated command parser module
pub fn parse_command(input: &str) -> Result<UserCommand, ParseError> {
    // Clear, testable, single responsibility
}
```

### Error Handling

1. **Use Result types**: Prefer `Result<T, E>` over panics for recoverable errors
2. **Descriptive errors**: Error messages should help users understand what went wrong

```rust
// Good: Descriptive error with context
Err(ParseError::InvalidRaiseAmount(value.to_string()))

// Bad: Generic error
Err("Invalid input")
```

### Code Duplication

- **DRY principle**: Don't repeat yourself
- **Extract common patterns**: If you see the same code twice, consider extracting it
- **Use macros judiciously**: Macros are powerful but can reduce clarity

**Good Example** (from Sprint 5):
```rust
// Before: Two nearly identical macros with 80% duplication
macro_rules! impl_user_managers { /* ... */ }
macro_rules! impl_user_managers_with_queue { /* ... */ }

// After: Unified macro with mode parameter
impl_user_managers!(immediate: Game<Lobby>, /* ... */);
impl_user_managers!(queued: Game<Deal>, /* ... */);
```

### Testing Standards

1. **Test coverage**: Aim for comprehensive coverage of public APIs
2. **Test naming**: Use descriptive names that explain what's being tested
3. **Test organization**: Group related tests with clear comments

```rust
// Good: Clear test structure
#[cfg(test)]
mod tests {
    use super::*;

    // === Single-word command tests ===

    #[test]
    fn test_parse_call() {
        let result = parse_command("call");
        assert!(matches!(result, Ok(UserCommand::TakeAction(Action::Call))));
    }

    // === Error cases ===

    #[test]
    fn test_parse_unrecognized_command() {
        let result = parse_command("invalid");
        assert!(matches!(result, Err(ParseError::UnrecognizedCommand(_))));
    }
}
```

### Documentation Standards

1. **Public APIs**: All public functions, structs, and modules must have rustdoc comments
2. **Examples**: Include examples in documentation when helpful
3. **Errors**: Document error conditions with `# Errors` section

```rust
/// Parse a command string into a UserCommand.
///
/// # Arguments
///
/// * `input` - The raw command string from user input
///
/// # Returns
///
/// * `Ok(UserCommand)` - Successfully parsed command
/// * `Err(ParseError)` - Parse error with descriptive message
///
/// # Examples
///
/// ```
/// use pp_client::commands::parse_command;
/// let cmd = parse_command("call")?;
/// ```
pub fn parse_command(input: &str) -> Result<UserCommand, ParseError> {
    // ...
}
```

### Performance Considerations

1. **Measure before optimizing**: Use benchmarks to identify actual bottlenecks
2. **Arc for shared data**: Use `Arc` to avoid cloning large structures
3. **Iterator chains**: Prefer iterator methods over manual loops for better compiler optimization

**Good Example** (from Sprint 4):
```rust
// Optimized: Using iterator chain
let players: Vec<PlayerView> = self.data.players
    .iter()
    .map(|player| /* ... */)
    .collect();

// Less optimal: Manual loop
let mut players = Vec::new();
for player in &self.data.players {
    players.push(/* ... */);
}
```

### Clippy and Formatting

1. **Zero clippy warnings**: Production code should have no clippy warnings
2. **Auto-format**: Always run `cargo fmt` before committing
3. **Fix suggestions**: Address clippy suggestions unless there's a good reason not to

```bash
# Check for warnings
cargo clippy -- -D warnings

# Auto-fix issues
cargo clippy --fix --allow-dirty

# Format code
cargo fmt --all
```

### Git Commit Practices

1. **Atomic commits**: Each commit should represent a single logical change
2. **Clear messages**: Describe what and why, not just what
3. **Test before committing**: Ensure all tests pass

```
Good commit message:
refactor: Extract command parser into dedicated module

Moved command parsing logic from app.rs to commands.rs
to improve testability and separation of concerns.
Added 30 tests for all command variants.

Bad commit message:
updated code
```

---

## Common Patterns

### State Machine Pattern

When adding a new game state:

1. Define zero-sized type marker
2. Implement `Into<PokerState>`
3. Implement required traits via `enum_dispatch`
4. Add to `PokerState` enum
5. Add tests for state transitions

### User Management Pattern

Choose the appropriate mode based on whether the operation should be immediate or queued:

```rust
// Immediate: For non-gameplay phases
impl_user_managers!(immediate: Game<Lobby>, Game<RemovePlayers>);

// Queued: For gameplay phases to avoid disrupting hands
impl_user_managers!(queued: Game<Deal>, Game<TakeAction>);
```

### View Generation Pattern

Use Arc-based sharing for read-only data that's common across all views:

```rust
let shared = SharedViewData {
    blinds: Arc::new(self.data.blinds.clone()),
    board: Arc::new(self.data.board.clone()),
    // ... other shared fields
};

users.map(|username| {
    GameView {
        blinds: shared.blinds.clone(),  // Cheap Arc clone
        players: personalize_for(username),  // Personalized data
        // ...
    }
})
```

---

## Resources

- **Architecture Documentation**: See `ARCHITECTURE.md` for detailed system design
- **Sprint Summaries**: `SPRINT_4_SUMMARY.md`, `SPRINT_5_PLAN.md` for recent improvements
- **Generated Docs**: Run `cargo doc --no-deps --open` to view rustdoc

---

**Last Updated:** Sprint 5 Completion
