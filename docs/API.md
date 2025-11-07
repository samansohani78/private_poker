# Private Poker API Documentation

## Overview

This document provides comprehensive API documentation for the Private Poker multi-table poker platform. The system is built with Rust and uses PostgreSQL for data persistence.

## Table of Contents

1. [Authentication API](#authentication-api)
2. [Wallet API](#wallet-api)
3. [Table Management API](#table-management-api)
4. [Bot Management API](#bot-management-api)
5. [Security API](#security-api)
6. [Network Protocol](#network-protocol)

---

## Authentication API

The authentication system provides secure user registration, login, session management, and 2FA support.

### Module: `private_poker::auth`

#### Registration

```rust
pub async fn register(
    &self,
    username: String,
    password: String,
    email: Option<String>,
) -> Result<i64, AuthError>
```

**Parameters:**
- `username`: Unique username (3-20 characters, alphanumeric + underscore)
- `password`: Password (minimum 8 characters, must include uppercase, lowercase, number)
- `email`: Optional email address for password recovery

**Returns:**
- `i64`: User ID on success
- `AuthError::UsernameTaken`: Username already exists
- `AuthError::EmailTaken`: Email already in use
- `AuthError::WeakPassword`: Password doesn't meet requirements
- `AuthError::InvalidUsername`: Username format invalid

**Example:**
```rust
let auth = AuthManager::new(pool, jwt_secret);
let user_id = auth.register(
    "player123".to_string(),
    "SecurePass123!".to_string(),
    Some("player@example.com".to_string()),
).await?;
```

#### Login

```rust
pub async fn login(
    &self,
    username: String,
    password: String,
    device_fingerprint: String,
) -> Result<(SessionTokens, User), AuthError>
```

**Parameters:**
- `username`: User's username
- `password`: User's password
- `device_fingerprint`: Unique device identifier for session tracking

**Returns:**
- `SessionTokens`: Access and refresh tokens
- `User`: User information
- `AuthError::UserNotFound`: User doesn't exist
- `AuthError::InvalidPassword`: Incorrect password
- `AuthError::TwoFactorRequired`: 2FA enabled, code required
- `AuthError::RateLimited`: Too many failed attempts

**Example:**
```rust
let (tokens, user) = auth.login(
    "player123".to_string(),
    "SecurePass123!".to_string(),
    "device_abc123".to_string(),
).await?;

println!("Access token: {}", tokens.access_token);
println!("User ID: {}", user.id);
```

#### Token Refresh

```rust
pub async fn refresh_token(
    &self,
    refresh_token: String,
    device_fingerprint: String,
) -> Result<SessionTokens, AuthError>
```

**Parameters:**
- `refresh_token`: Valid refresh token from previous login
- `device_fingerprint`: Same device fingerprint used during login

**Returns:**
- `SessionTokens`: New access and refresh tokens
- `AuthError::InvalidRefreshToken`: Token invalid or expired
- `AuthError::SessionNotFound`: Session doesn't exist
- `AuthError::SessionExpired`: Session has expired

#### Logout

```rust
pub async fn logout(
    &self,
    user_id: i64,
    refresh_token: &str,
) -> Result<(), AuthError>
```

**Parameters:**
- `user_id`: User ID
- `refresh_token`: Refresh token to invalidate

**Returns:**
- `Ok(())`: Successfully logged out
- `AuthError::SessionNotFound`: No matching session

#### Token Validation

```rust
pub async fn validate_access_token(
    &self,
    access_token: &str,
) -> Result<User, AuthError>
```

**Parameters:**
- `access_token`: JWT access token

**Returns:**
- `User`: User information if token valid
- `AuthError::JwtError`: Invalid token format or signature
- `AuthError::SessionExpired`: Token has expired

---

## Wallet API

The wallet system manages user balances, transactions, and maintains a complete ledger.

### Module: `private_poker::wallet`

#### Get Balance

```rust
pub async fn get_balance(&self, user_id: i64) -> Result<i64, WalletError>
```

**Parameters:**
- `user_id`: User ID

**Returns:**
- `i64`: Current balance in chips
- `WalletError::WalletNotFound`: Wallet doesn't exist

**Example:**
```rust
let wallet = WalletManager::new(pool, 1000);
let balance = wallet.get_balance(user_id).await?;
println!("Balance: {} chips", balance);
```

#### Update Balance

```rust
pub async fn update_balance(
    &self,
    user_id: i64,
    amount: i64,
    idempotency_key: String,
) -> Result<(), WalletError>
```

**Parameters:**
- `user_id`: User ID
- `amount`: Amount to add (positive) or subtract (negative)
- `idempotency_key`: Unique key to prevent duplicate transactions

**Returns:**
- `Ok(())`: Balance updated successfully
- `WalletError::InsufficientBalance`: Not enough chips
- `WalletError::WalletNotFound`: Wallet doesn't exist

**Example:**
```rust
// Add chips
wallet.update_balance(user_id, 500, "bonus_2024_01".to_string()).await?;

// Remove chips
wallet.update_balance(user_id, -200, "buyin_table_42".to_string()).await?;
```

#### Claim Faucet

```rust
pub async fn claim_faucet(
    &self,
    user_id: i64,
) -> Result<(i64, DateTime<Utc>), WalletError>
```

**Parameters:**
- `user_id`: User ID

**Returns:**
- `i64`: Amount claimed
- `DateTime<Utc>`: Next claim time
- `WalletError::FaucetOnCooldown`: Faucet not yet available
- `WalletError::BalanceTooHigh`: User has sufficient balance

**Example:**
```rust
let (amount, next_claim) = wallet.claim_faucet(user_id).await?;
println!("Claimed {} chips, next claim at {:?}", amount, next_claim);
```

#### Transaction History

```rust
pub async fn get_transaction_history(
    &self,
    user_id: i64,
    limit: usize,
    offset: usize,
) -> Result<Vec<WalletEntry>, WalletError>
```

**Parameters:**
- `user_id`: User ID
- `limit`: Maximum number of entries to return
- `offset`: Number of entries to skip

**Returns:**
- `Vec<WalletEntry>`: Transaction history
- `WalletError::WalletNotFound`: Wallet doesn't exist

**Example:**
```rust
let history = wallet.get_transaction_history(user_id, 50, 0).await?;
for entry in history {
    println!("{}: {} chips ({})", entry.created_at, entry.amount, entry.description);
}
```

---

## Table Management API

Manages poker tables, player seating, and game configuration.

### Module: `private_poker::table`

#### Create Table

```rust
pub async fn create_table(
    &self,
    owner_id: i64,
    config: TableConfig,
) -> Result<i64, TableError>
```

**Parameters:**
- `owner_id`: User ID of table creator
- `config`: Table configuration (blinds, limits, speed, etc.)

**Returns:**
- `i64`: Table ID
- `TableError::InvalidConfig`: Configuration validation failed

**Example:**
```rust
let config = TableConfig {
    name: "High Stakes Table".to_string(),
    max_players: 9,
    small_blind: 10,
    big_blind: 20,
    min_buy_in_bb: 20,
    max_buy_in_bb: 100,
    absolute_chip_cap: 100_000,
    top_up_cooldown_hands: 20,
    speed: TableSpeed::Standard,
    bots_enabled: true,
    target_bot_count: 5,
    bot_difficulty: BotDifficulty::Standard,
    is_private: false,
    passphrase_hash: None,
    invite_token: None,
};

let table_id = table_actor.create_table(owner_id, config).await?;
```

#### Join Table

```rust
pub async fn join_table(
    &self,
    table_id: i64,
    user_id: i64,
    buy_in: i64,
) -> Result<(), TableError>
```

**Parameters:**
- `table_id`: Table ID
- `user_id`: User ID
- `buy_in`: Buy-in amount in chips

**Returns:**
- `Ok(())`: Successfully joined
- `TableError::TableFull`: No seats available
- `TableError::InsufficientBuyIn`: Buy-in below minimum
- `TableError::ExcessiveBuyIn`: Buy-in above maximum

#### Leave Table

```rust
pub async fn leave_table(
    &self,
    table_id: i64,
    user_id: i64,
) -> Result<i64, TableError>
```

**Parameters:**
- `table_id`: Table ID
- `user_id`: User ID

**Returns:**
- `i64`: Chips returned to wallet
- `TableError::UserNotAtTable`: User not seated

---

## Bot Management API

Controls bot players with configurable difficulty levels.

### Module: `private_poker::bot`

#### Bot Difficulty Presets

```rust
pub enum BotDifficulty {
    Easy,      // Loose-passive (45% VPIP, 0.5 aggression)
    Standard,  // Tight-aggressive (30% VPIP, 1.5 aggression)
    Tag,       // Very tight-aggressive (20% VPIP, 2.5 aggression)
}
```

#### Decision Making

```rust
pub fn make_decision(
    &self,
    game_state: &GameState,
    player: &BotPlayer,
) -> Action
```

**Parameters:**
- `game_state`: Current game state
- `player`: Bot player state

**Returns:**
- `Action`: Bot's decision (Fold, Check, Call, Raise, AllIn)

**Bot Characteristics:**

**Easy (Loose-Passive):**
- VPIP: 45% (plays many hands)
- PFR: 10% (rarely raises pre-flop)
- Aggression: 0.5 (passive)
- Bluffs: Never
- Think time: ~1.5s ±1s

**Standard (Balanced TAG):**
- VPIP: 30%
- PFR: 20%
- Aggression: 1.5 (moderately aggressive)
- Bluffs: 15% of time
- Think time: ~2s ±1.5s

**TAG (Tight-Aggressive):**
- VPIP: 20% (very selective)
- PFR: 18%
- Aggression: 2.5 (very aggressive)
- Bluffs: 25% of time
- Think time: ~2.5s ±2s

---

## Security API

Rate limiting, anti-collusion detection, and seat randomization.

### Module: `private_poker::security`

#### Rate Limiting

```rust
pub async fn check_rate_limit(
    &self,
    endpoint: &str,
    identifier: &str,
) -> Result<RateLimitResult, String>
```

**Endpoints and Limits:**
- `login`: 5 attempts per 5 minutes, 15-minute lockout
- `register`: 3 attempts per hour, 1-hour lockout
- `password_reset`: 3 attempts per hour, 2-hour lockout
- `chat`: 10 messages per minute, 5-minute lockout

**Returns:**
- `RateLimitResult::Allowed { remaining }`: Action allowed
- `RateLimitResult::Locked { retry_after }`: Rate limited

**Example:**
```rust
let limiter = RateLimiter::new(pool);
let result = limiter.check_rate_limit("login", "192.168.1.100").await?;

if result.is_allowed() {
    limiter.record_attempt("login", "192.168.1.100").await?;
    // Process login
} else {
    let retry = result.retry_after().unwrap();
    println!("Rate limited, retry after {} seconds", retry);
}
```

#### Anti-Collusion Detection

```rust
pub async fn check_same_ip_at_table(
    &self,
    table_id: i64,
    user_id: i64,
) -> Result<bool, String>
```

**Parameters:**
- `table_id`: Table ID
- `user_id`: User attempting to join

**Returns:**
- `bool`: True if same-IP player detected
- Creates shadow flag for admin review if detected

**Flag Types:**
- `SameIpTable`: Multiple players from same IP (Medium severity)
- `WinRateAnomaly`: >80% win rate against same-IP players (High severity)
- `CoordinatedFolding`: Always folding to same player (Low severity)

**Example:**
```rust
let detector = AntiCollusionDetector::new(pool);
detector.register_user_ip(user_id, "192.168.1.100".to_string()).await;

if detector.check_same_ip_at_table(table_id, user_id).await? {
    // Allow join but flag for review
    log::warn!("Same-IP detected at table {}", table_id);
}
```

#### Seat Randomization

```rust
pub fn assign_seats(
    &mut self,
    user_ids: &[i64],
    max_seats: usize,
) -> HashMap<i64, usize>
```

**Parameters:**
- `user_ids`: List of user IDs to seat
- `max_seats`: Maximum table capacity

**Returns:**
- `HashMap<i64, usize>`: User ID to seat index mapping

**Example:**
```rust
let mut randomizer = SeatRandomizer::new();
let seats = randomizer.assign_seats(&[1, 2, 3, 4, 5], 9);
// seats = {1: 3, 2: 7, 3: 1, 4: 5, 5: 0} (randomized)
```

---

## Network Protocol

Binary protocol using bincode serialization over TCP.

### Module: `private_poker::net`

#### Protocol Version

```rust
pub enum ProtocolVersion {
    V1,  // Original single-table
    V2,  // Multi-table with auth
}
```

Current version: **V2** (backward compatible with V1)

#### Client Messages

See [`messages.rs`](../private_poker/src/net/messages.rs) for full protocol specification.

**V1 Commands (Legacy):**
- `Connect`, `Disconnect`
- `ChangeState(UserState)`
- `TakeAction(Action)`
- `CastVote(Vote)`
- `ShowHand`, `StartGame`

**V2 Commands (Multi-Table):**
- **Auth**: `Register`, `Login`, `RefreshToken`, `Logout`
- **2FA**: `Enable2FA`, `Verify2FA`
- **Tables**: `CreateTable`, `JoinTable`, `LeaveTable`, `ListTables`
- **Wallet**: `GetBalance`, `ClaimFaucet`, `GetTransactionHistory`
- **Chat**: `SendChatMessage`, `MuteUser`, `KickUser`
- **Multi-Table**: `TakeActionAtTable`, `ShowHandAtTable`, etc.

#### Server Messages

**V1 Responses:**
- `Ack(ClientMessage)`
- `GameView(GameView)`
- `TurnSignal(ActionChoices)`
- `GameEvent(GameEvent)`
- `UserError(UserError)`
- `ClientError(ClientError)`

**V2 Responses:**
- **Auth**: `LoginSuccess`, `RegisterSuccess`, `RefreshSuccess`
- **Tables**: `TableCreated`, `TableList`, `JoinedTable`
- **Wallet**: `Balance`, `FaucetClaimed`, `TransactionHistory`
- **Chat**: `ChatMessage`
- **Errors**: `AuthError`, `WalletError`, `RateLimitError`

---

## Error Handling

All APIs use Result types with descriptive errors:

### AuthError
- `UsernameTaken`, `EmailTaken`
- `InvalidPassword`, `UserNotFound`
- `WeakPassword`, `InvalidUsername`
- `SessionExpired`, `InvalidRefreshToken`
- `TwoFactorRequired`, `InvalidTwoFactorCode`
- `RateLimited`

### WalletError
- `InsufficientBalance`
- `WalletNotFound`
- `FaucetOnCooldown`
- `BalanceTooHigh`
- `TransactionFailed`

### TableError
- `TableNotFound`
- `TableFull`
- `InsufficientBuyIn`
- `ExcessiveBuyIn`
- `UserNotAtTable`
- `InvalidConfig`

---

## Type Reference

### Common Types

```rust
// User ID type
pub type UserId = i64;

// Table ID type
pub type TableId = i64;

// Chip amounts (always integers, never floats)
pub type Chips = i64;

// Timestamps
use chrono::{DateTime, Utc};

// Cards
pub struct Card(pub Value, pub Suit);
pub type Value = u8; // 1-14 (Ace low to Ace high)
pub enum Suit { Clubs, Diamonds, Hearts, Spades }

// Actions
pub enum Action {
    Fold,
    Check,
    Call,
    Raise(Option<u32>),
    AllIn,
}
```

---

## Best Practices

### Authentication
1. Always validate tokens on protected endpoints
2. Implement device fingerprinting for session tracking
3. Use refresh tokens to minimize access token exposure
4. Enable 2FA for high-value accounts

### Wallet Operations
1. Use idempotency keys for all transactions
2. Never use floating point for chip amounts
3. Always check balance before operations
4. Maintain complete audit trail

### Rate Limiting
1. Check rate limits before expensive operations
2. Return retry-after time to clients
3. Implement exponential backoff on client side

### Security
1. Register user IPs on connection
2. Check for same-IP players before joining tables
3. Review collusion flags regularly
4. Use seat randomization always

## 7. Tournament API (Phase 7)

### Create Tournament

```rust
pub async fn create_tournament(
    &self,
    config: TournamentConfig,
) -> TournamentResult<TournamentId>
```

**Parameters**:
- `config`: Tournament configuration (see TournamentConfig)

**Returns**: `TournamentId` on success

**Example**:
```rust
let config = TournamentConfig::sit_and_go("Sunday Special".to_string(), 9, 100);
let tournament_id = tournament_mgr.create_tournament(config).await?;
```

### Register for Tournament

```rust
pub async fn register_player(
    &self,
    tournament_id: TournamentId,
    user_id: i64,
    username: String,
) -> TournamentResult<()>
```

**Checks**:
- Tournament must be in Registering state
- Tournament must not be full
- Player must not already be registered

**Auto-start**: Sit-n-Go tournaments start automatically when full

### Get Tournament Info

```rust
pub async fn get_tournament_info(
    &self,
    tournament_id: TournamentId,
) -> TournamentResult<TournamentInfo>
```

**Returns**: Complete tournament information including:
- Current state and blind level
- Registered players count
- Prize structure
- Time to next blind level

### Blind Level Management

```rust
pub async fn advance_blind_level(
    &self,
    tournament_id: TournamentId,
) -> TournamentResult<u32>
```

**Note**: Automatically called by tournament timer. Returns new level number.

### Player Elimination

```rust
pub async fn eliminate_player(
    &self,
    tournament_id: TournamentId,
    user_id: i64,
    position: usize,
) -> TournamentResult<()>
```

**Prize Calculation**: Automatically calculates prize based on:
- 2-5 players: Winner takes all
- 6-9 players: 60/40 split
- 10+ players: 50/30/20 split

### Tournament Configuration

```rust
// Standard 9-player Sit-n-Go
let config = TournamentConfig::sit_and_go("Test SNG".to_string(), 9, 100);

// Turbo (faster blinds)
let config = TournamentConfig::turbo_sit_and_go("Turbo SNG".to_string(), 6, 50);

// Custom blind structure
let config = TournamentConfig {
    name: "Custom Tournament".to_string(),
    tournament_type: TournamentType::SitAndGo,
    buy_in: 200,
    min_players: 4,
    max_players: 10,
    starting_stack: 10000,
    blind_levels: vec![
        BlindLevel::new(1, 25, 50, 600),
        BlindLevel::new(2, 50, 100, 600),
        // ... more levels
    ],
    starting_level: 1,
    scheduled_start: None,
    late_registration_secs: None,
};
```

**Blind Level Structure**:
- Standard: 5-minute levels, blinds double every 2 levels
- Turbo: 3-minute levels
- Starting stack: 50x buy-in

---

## Support

For issues or questions:
- GitHub: https://github.com/anthropics/private_poker
- Documentation: See `/docs` directory
- Tests: See `/tests` directory for usage examples

---

Generated with Claude Code
