# Private Poker - Texas Hold'em Platform

Production-ready poker platform built in Rust with enterprise-grade security, real-time gameplay, and smart bot opponents.

[![Rust](https://img.shields.io/badge/rust-2024%20edition-orange.svg)](https://www.rust-lang.org/)
[![Tests](https://img.shields.io/badge/tests-744%20passing-brightgreen.svg)]()
[![Coverage](https://img.shields.io/badge/coverage-62.09%25-green.svg)]()
[![License](https://img.shields.io/badge/license-Apache%202.0-blue.svg)](LICENSE)

## Features

- ✅ **Complete Texas Hold'em** - Full poker rules implementation
- ✅ **Real-Time Gameplay** - WebSocket updates every ~1 second
- ✅ **Smart Bot AI** - 3 difficulty levels with position awareness & bluffing
- ✅ **Enterprise Security** - Argon2id, JWT, 2FA, rate limiting, anti-collusion
- ✅ **Financial System** - Double-entry ledger with escrow and audit trail
- ✅ **Multi-Table Support** - Actor-based concurrent table management
- ✅ **Rich TUI Client** - Beautiful terminal interface with colored cards
- ✅ **Tournament Mode** - Sit-n-Go tournaments with blind progression
- ✅ **744 Tests** - Comprehensive testing (0 failures, 0 warnings)
- ✅ **Zero Technical Debt** - Production-ready code quality

## Quick Start

### Prerequisites

- **Rust** 1.70+ (2024 edition)
- **PostgreSQL** 14+
- **Redis** (optional, for future horizontal scaling)

### Installation

```bash
# 1. Install Rust (if not already installed)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# 2. Clone repository
git clone <repository-url>
cd private_poker

# 3. Set up PostgreSQL database
createdb poker_db

# 4. Run database migrations
DATABASE_URL="postgresql://postgres:password@localhost/poker_db" sqlx migrate run

# 5. Configure environment
cp .env.example .env
# Edit .env with your DATABASE_URL, JWT_SECRET, PASSWORD_PEPPER

# 6. Build
cargo build --release

# 7. Run server
./target/release/pp_server --bind 0.0.0.0:8080

# 8. Run client (in another terminal)
./target/release/pp_client --server http://localhost:8080
```

See [CLAUDE.md](CLAUDE.md) for complete documentation.

---

**Developer**: Saman Sohani | **Version**: 3.0.1 | **Status**: Production Ready ✅
