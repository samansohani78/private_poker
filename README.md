# Private Poker - Texas Hold'em Platform

A production-ready, multi-table Texas Hold'em poker platform built in Rust with real-time WebSocket gameplay.

**Author**: Saman Sohani
**Status**: 100% Production-Ready ✅
**License**: Apache-2.0

---

## Features

### Core Gameplay
- ♠️ Complete Texas Hold'em implementation
- 🎰 Type-safe Finite State Machine (14 game states)
- ⚡ Lightning-fast hand evaluation (1.35 µs per hand)
- 🎲 Multiple table support with concurrent games
- 🏆 Sit-n-Go tournament mode

### Player Experience
- 🖥️ **Rich TUI Mode**: Beautiful terminal interface with colored cards
- 📱 **CLI Mode**: Simple command-line interface
- 🌐 **WebSocket**: Real-time game updates
- 🤖 **Bot Opponents**: Smart AI with bluffing (Easy/Standard/TAG difficulty)

### Backend Features
- 🔐 Secure authentication (Argon2id + JWT + 2FA)
- 💰 Double-entry ledger wallet system
- 🛡️ Anti-collusion detection
- ⏱️ Rate limiting and security
- 🗄️ PostgreSQL database with migrations
- 🔄 Actor-based concurrent table management

---

## Quick Start

### Prerequisites

- Rust 1.70+ (install from [rustup.rs](https://rustup.rs))
- PostgreSQL 14+
- (Optional) Docker for containerized deployment

### Installation

```bash
# Clone the repository
git clone https://github.com/samansohani/private_poker.git
cd private_poker

# Build the project
cargo build --release

# Set up database
createdb poker_db
sqlx migrate run --database-url "postgres://localhost/poker_db"
```

### Running the Server

```bash
# Start the poker server
cargo run --bin pp_server --release
```

Server will start on `http://localhost:6969`

### Running the Client

**TUI Mode (Rich Terminal UI)**:
```bash
cargo run --bin pp_client --release -- --username alice --tui
```

**CLI Mode (Simple)**:
```bash
cargo run --bin pp_client --release -- --username bob
```

### Running Bots

```bash
# Start bot manager
cargo run --bin pp_bots --release
```

---

## How It Works

### Architecture Overview

```
┌─────────────────────────────────────────────────────────────┐
│                        HTTP/WebSocket API                    │
│                    (Axum + tokio-tungstenite)               │
├─────────────────────────────────────────────────────────────┤
│                      TableManager (Actor)                    │
│          Coordinates multiple concurrent poker tables        │
├─────────────────────────────────────────────────────────────┤
│                    TableActor × N (Actors)                   │
│        Each table runs as independent async actor            │
├─────────────────────────────────────────────────────────────┤
│                    Poker Engine (FSM)                        │
│         Type-safe state machine with 14 states              │
├─────────────────────────────────────────────────────────────┤
│             PostgreSQL Database (sqlx)                       │
│      Users, Wallets, Tables, History, Security              │
└─────────────────────────────────────────────────────────────┘
```

### Game Flow

1. **Lobby**: Players join and wait for game start (min 2 players)
2. **SeatPlayers**: Random seat assignment (anti-collusion)
3. **MoveButton**: Rotate dealer button
4. **CollectBlinds**: Small blind and big blind posted
5. **Deal**: 2 hole cards dealt to each player
6. **TakeAction**: Pre-flop betting round
7. **Flop**: 3 community cards dealt
8. **TakeAction**: Flop betting round
9. **Turn**: 4th community card dealt
10. **TakeAction**: Turn betting round
11. **River**: 5th community card dealt
12. **TakeAction**: River betting round
13. **ShowHands**: Reveal cards
14. **DistributePot**: Distribute winnings (with side pots)
15. **RemovePlayers**: Remove broke/disconnected players
16. **UpdateBlinds**: Increase blinds (tournament mode)

### API Endpoints

**Authentication**:
- `POST /api/auth/register` - Create new account
- `POST /api/auth/login` - Login and get JWT
- `POST /api/auth/refresh` - Refresh access token
- `POST /api/auth/logout` - Logout

**Tables**:
- `GET /api/tables` - List all tables
- `GET /api/tables/:id` - Get table state
- `POST /api/tables/:id/join` - Join table
- `POST /api/tables/:id/leave` - Leave table
- `POST /api/tables/:id/action` - Take poker action

**WebSocket**:
- `GET /ws/:table_id?token=<jwt>` - Real-time game connection

### Client Commands

**Game Actions**:
- `fold` - Fold your hand
- `check` - Check (if no bet)
- `call` - Match current bet
- `raise <amount>` - Raise bet
- `all-in` - Bet all chips

**Table Management**:
- `join <buy_in>` - Join table with chips
- `leave` - Leave table
- `spectate` - Watch as spectator
- `stop` - Stop spectating

---

## Configuration

### Environment Variables

```bash
# Database
DATABASE_URL=postgres://user:pass@localhost/poker_db

# Server
BIND_ADDR=0.0.0.0:6969
RUST_LOG=info

# Security
JWT_SECRET=your-secret-key-here
PEPPER=your-pepper-for-password-hashing
```

### Server Options

```bash
cargo run --bin pp_server -- \
  --bind 127.0.0.1:6969 \
  --db-url postgres://localhost/poker_db \
  --tables 10
```

---

## Development

### Running Tests

```bash
# All tests
cargo test --workspace

# Specific package
cargo test -p private_poker

# With output
cargo test -- --nocapture
```

**Test Results**: 501 tests passing, 0 failures ✅

### Code Quality

```bash
# Check formatting
cargo fmt --all -- --check

# Auto-format
cargo fmt --all

# Run clippy (strict mode)
cargo clippy --workspace -- -D warnings

# Run benchmarks
cargo bench
```

### Database Migrations

```bash
# Run migrations
sqlx migrate run

# Create new migration
sqlx migrate add <name>

# Revert last migration
sqlx migrate revert
```

---

## Deployment

### Using Docker

```bash
# Build image
docker build -t private-poker .

# Run container
docker run -d \
  -p 6969:6969 \
  -e DATABASE_URL=postgres://host/db \
  --name poker-server \
  private-poker

# View logs
docker logs -f poker-server
```

### Production Checklist

- ✅ Set strong `JWT_SECRET` and `PEPPER`
- ✅ Use HTTPS/WSS in production
- ✅ Configure PostgreSQL connection pooling
- ✅ Set `RUST_LOG=info` (not debug/trace)
- ✅ Enable database backups
- ✅ Configure firewall rules
- ✅ Set up monitoring (optional)

---

## Project Structure

```
private_poker/
├── private_poker/      # Core library
│   ├── src/
│   │   ├── game/       # Poker engine (FSM, hand eval)
│   │   ├── table/      # Multi-table actors
│   │   ├── auth/       # Authentication
│   │   ├── wallet/     # Wallet & escrow
│   │   ├── bot/        # Bot AI
│   │   ├── security/   # Security features
│   │   ├── tournament/ # Tournament mode
│   │   └── db/         # Database layer
│   └── tests/          # Integration tests
│
├── pp_server/          # Server binary
│   └── src/api/        # HTTP/WebSocket API
│
├── pp_client/          # Client binary
│   └── src/
│       ├── tui_app.rs  # Rich TUI mode
│       ├── api_client.rs
│       └── websocket_client.rs
│
├── pp_bots/            # Bot manager binary
│
└── migrations/         # Database migrations
```

---

## Performance

- **Hand Evaluation**: 1.35 microseconds per 7-card hand
- **Concurrent Tables**: Hundreds tested successfully
- **Test Coverage**: 73.63% overall, 99.71% on critical paths
- **Memory**: Optimized with Arc-based view sharing

---

## Security Features

- **Password Hashing**: Argon2id with server pepper
- **Authentication**: JWT with 15-min access + 7-day refresh tokens
- **2FA**: TOTP support with backup codes
- **Rate Limiting**: Per-endpoint, IP-based
- **Anti-Collusion**: IP tracking, win rate anomalies, pattern detection
- **SQL Injection**: Prevented via prepared statements
- **Seat Randomization**: Cryptographic shuffle

---

## Tech Stack

**Core**:
- Rust 2024 Edition
- Tokio (async runtime)
- Axum (web framework)
- sqlx (PostgreSQL)

**Game Logic**:
- enum_dispatch (zero-cost FSM)
- rand (cryptographic randomness)
- Custom hand evaluation algorithm

**Security**:
- argon2 (password hashing)
- jsonwebtoken (JWT)
- totp-rs (2FA)

**UI**:
- ratatui (terminal UI)
- crossterm (terminal control)
- tokio-tungstenite (WebSocket)

---

## Contributing

This is a personal project by Saman Sohani. If you'd like to contribute:

1. Fork the repository
2. Create a feature branch
3. Make your changes with tests
4. Run `cargo fmt` and `cargo clippy`
5. Submit a pull request

---

## License

Apache License 2.0 - See LICENSE file for details

Copyright © 2025 Saman Sohani

---

## Contact

**Developer**: Saman Sohani
**GitHub**: [github.com/samansohani](https://github.com/samansohani)
**Project**: Private Poker - Texas Hold'em Platform

---

## Acknowledgments

Built with Rust 🦀 - A language empowering everyone to build reliable and efficient software.

---

**Status**: Production-Ready ✅
**Version**: 3.0.1
**Tests**: 501 passing, 0 failures
**Last Updated**: 2025-11-13
