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
