# Multi-Table Poker Implementation Plan

**Created:** 2025-01-06
**Updated:** 2025-01-06
**Status:** DRAFT - Under Review
**Version:** 2.0 (Revised)

---

## Table of Contents

1. [Executive Summary](#executive-summary)
2. [Current Architecture Analysis](#current-architecture-analysis)
3. [Requirements Overview](#requirements-overview)
4. [Proposed Architecture](#proposed-architecture)
5. [Database Schema](#database-schema)
6. [Implementation Phases](#implementation-phases)
7. [API Changes](#api-changes)
8. [Security Considerations](#security-considerations)
9. [Anti-Collusion & Fairness](#anti-collusion--fairness)
10. [Testing Strategy](#testing-strategy)
11. [Migration Strategy](#migration-strategy)

---

## Executive Summary

This document proposes a comprehensive redesign of the Private Poker system to support:

- **Multi-table gameplay**: Multiple concurrent tables with async actor model (separate task per table with message inbox)
- **User authentication**: Argon2id + server pepper, JWT access/refresh tokens, 2FA optional
- **Persistent wallets**: Double-entry ledger with idempotency keys, escrow-based table sessions
- **Automatic bot filling**: Configurable difficulty presets (easy/standard/TAG), human-like pacing, anti-ratholing

**Key Design Principles:**
- **Async-first**: Each table runs in its own Tokio task with message-passing
- **ACID guarantees**: PostgreSQL transactions for all wallet mutations
- **Security-first**: Argon2id, JWT rotation, rate limiting, anti-collusion guardrails
- **Scalability**: Actor model supports hundreds of concurrent tables

---

## Current Architecture Analysis

### 1. Single-Table Design

**Current State:**
```rust
// server.rs (line 869)
let mut state: PokerState = config.game_settings.into();
loop {
    state = state.step();
    // ... single game loop
}
```

**Limitations:**
- Server runs ONE PokerState FSM in the main thread
- All users connect to the same game
- No concept of multiple tables or concurrent games

### 2. User Management

**Current State:**
```rust
// entities.rs
pub struct Username(String);  // Simple string wrapper
pub struct User {
    pub name: Username,
    pub money: Usd,
}

// game.rs (line 170)
ledger: HashMap<Username, Usd>  // In-memory reconnection tracking
```

**Limitations:**
- No authentication (username is sole identity)
- No password or session management
- Ephemeral: users recreated on each connection
- Ledger only persists within server lifetime

### 3. Wallet System

**Current State:**
```rust
// entities.rs
pub type Usd = u32;

// User gets DEFAULT_BUY_IN (600) when joining
pub const DEFAULT_BUY_IN: Usd = 600;
```

**Limitations:**
- Money resets to buy-in on reconnect
- No persistence across server restarts
- No transaction history or audit trail
- No escrow/locking mechanism

### 4. Bot System

**Current State:**
```rust
// pp_bots/src/bot.rs
pub struct Bot {
    client: Client,  // TCP client connection
    hand: State,
    starting_money: Usd,
    view: GameView,
}
```

**Limitations:**
- Bots managed via separate TUI application (pp_bots)
- Manual bot creation by operator
- No automatic filling based on player count
- Bots are just TCP clients (overhead)

---

## Requirements Overview

### 1. Multi-Table Support

**Core Features:**

✅ **Table Actor Model**
- Each table runs in separate async Tokio task
- Message inbox for commands (Join, Leave, TakeAction, etc.)
- Independent FSM execution per table
- Unique numeric `table_id`

✅ **Waitlist System**
- Per-table FIFO waitlist to avoid race conditions
- Automatic seat assignment when spot opens
- Clear waitlist position feedback

✅ **Per-Table Configuration**
```rust
pub struct TableConfig {
    pub name: String,
    pub max_players: usize,              // Default: 10
    pub small_blind: Usd,
    pub big_blind: Usd,
    pub min_buy_in_bb: u8,               // e.g., 20 BB
    pub max_buy_in_bb: u8,               // e.g., 100 BB
    pub absolute_chip_cap: Usd,          // Hard cap: 100,000
    pub top_up_cooldown_hands: u8,       // e.g., 20 hands
    pub speed: TableSpeed,               // Normal | Turbo | Hyper
    pub bots_enabled: bool,
    pub is_private: bool,
    pub passphrase_hash: Option<String>, // Argon2id
    pub invite_token: Option<String>,    // Expiring token
}

pub enum TableSpeed {
    Normal,   // 30s turn timeout
    Turbo,    // 15s turn timeout
    Hyper,    // 10s turn timeout
}
```

✅ **Discovery & Filtering**
```rust
pub struct TableFilter {
    pub stakes_tier: Option<StakesTier>,  // Micro, Low, Mid, High
    pub min_players: Option<usize>,
    pub max_players: Option<usize>,
    pub has_waitlist_space: bool,
    pub speed: Option<TableSpeed>,
    pub bots_enabled: Option<bool>,
    pub is_private: bool,
}

pub enum StakesTier {
    Micro,    // BB ≤ 10
    Low,      // BB 10-100
    Mid,      // BB 100-1000
    High,     // BB > 1000
}
```

✅ **Spectating**
- Read-only view of table state
- Throttled updates (e.g., 1 update per 5 seconds)
- No hole card visibility (unless player showed)
- Spectator count limit per table

✅ **Private Tables**
- **Passphrase-protected**: Argon2id hash stored
- **Invite token**: Expiring UUID (e.g., 24-hour validity)
- Creator becomes table owner with mod powers

✅ **Table Chat**
```rust
pub struct ChatMessage {
    pub user_id: UserId,
    pub username: String,
    pub message: String,
    pub timestamp: DateTime<Utc>,
}

pub struct ChatConfig {
    pub rate_limit_seconds: u8,      // e.g., 3 seconds between messages
    pub profanity_filter_enabled: bool,
    pub muted_users: HashSet<UserId>,
}
```
- Rate limiting: max 1 message per 3 seconds
- Profanity filter (basic word blacklist)
- Owner/mod can mute/kick users

✅ **Money Limits**
- Enforce `min_buy_in_bb` and `max_buy_in_bb` at table join
- Absolute chip cap: 100,000 chips (prevent overflow)
- **Top-up cooldown**: Can't add chips for 20 hands after last top-up (anti-ratholing)
- Clear error messages with CTA when limits hit

✅ **Tournament Mode (Phased)**

**Phase A: Sit-n-Go (Single Table)**
```rust
pub struct SitNGoConfig {
    pub max_players: usize,         // e.g., 6, 9
    pub starting_chips: Usd,        // e.g., 1500
    pub blind_schedule: Vec<BlindLevel>,
    pub payout_structure: PayoutStructure,  // 50/30/20
}

pub struct BlindLevel {
    pub small_blind: Usd,
    pub big_blind: Usd,
    pub duration_hands: u16,        // e.g., 10 hands per level
}
```
- Starts when all seats filled
- Blind increases on schedule
- Payouts to top N finishers

**Phase B: Multi-Table Tournament (MTT)**
```rust
pub struct MTTConfig {
    pub tournament_id: TournamentId,
    pub starting_tables: usize,
    pub players_per_table: usize,
    pub late_registration_hands: u16,
    pub break_schedule: Vec<BreakTime>,
    pub table_balancing_enabled: bool,
}

pub struct BreakTime {
    pub after_level: u8,
    pub duration_minutes: u8,
}
```
- Table balancing: move players to keep tables even
- Table merges as players eliminated
- Scheduled breaks (5 min every hour)

✅ **Anti-Collusion Guardrails**
- **Seat randomization**: Players assigned random seats on join
- **IP restrictions**: Optional single-human-per-IP per table
- **Shadow flagging**: Track suspicious patterns (win rate vs same IP, fold rate correlation)
- Flags reviewed by admin (no auto-bans initially)

### 2. User Management & Authentication

✅ **Registration & Login**
```rust
pub struct UserCredentials {
    pub username: String,       // 3-20 alphanumeric + underscore
    pub password: String,       // Min 8 chars, strength check
}

pub struct AuthConfig {
    pub pepper: String,         // Server-side secret
    pub jwt_access_ttl: Duration,    // 15 minutes
    pub jwt_refresh_ttl: Duration,   // 7 days
}
```
- **Password hashing**: Argon2id with unique salt per user + server pepper
- **Password strength**: Min 8 chars, require uppercase + lowercase + digit
- **Rate limiting**: Max 5 login attempts per minute per IP

✅ **Session Model**
```rust
pub struct SessionToken {
    pub access_token: String,   // JWT, short-lived (15m)
    pub refresh_token: String,  // UUID, stored in DB (7d)
}

pub struct RefreshTokenEntry {
    pub token: String,
    pub user_id: UserId,
    pub device_fingerprint: String,  // User-Agent + IP hash
    pub created_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub last_used: DateTime<Utc>,
}
```
- **Access token**: JWT with claims `{user_id, exp, iat}`
- **Refresh token**: Stored in DB with device metadata
- **Token rotation**: On refresh, issue new access + refresh, invalidate old refresh
- **Revocation**: On logout, delete refresh token from DB

✅ **User Profiles**
```rust
pub struct UserProfile {
    pub user_id: UserId,
    pub username: String,           // Unique, immutable
    pub display_name: String,       // Changeable, shown in UI
    pub avatar_url: Option<String>, // URL to avatar image
    pub country: Option<String>,    // ISO country code
    pub timezone: Option<String>,   // IANA timezone
    pub tos_version: u16,           // ToS version accepted
    pub privacy_version: u16,       // Privacy policy version
    pub created_at: DateTime<Utc>,
}
```

✅ **Password Reset**
```rust
pub struct PasswordResetRequest {
    pub email: String,
    pub code: String,           // 6-digit random code
    pub expires_at: DateTime<Utc>,  // 10 minutes
}
```
- Email optional at registration but required for reset
- Email code sent, 10-minute expiry
- Rate limit: 1 reset request per 5 minutes per email

✅ **Two-Factor Authentication (2FA)**
```rust
pub struct TwoFactorAuth {
    pub user_id: UserId,
    pub secret: String,         // TOTP secret (base32)
    pub enabled: bool,
    pub backup_codes: Vec<String>,  // 10 single-use codes
}
```
- Optional at launch
- **Required for admin accounts**
- TOTP (Google Authenticator compatible)
- Backup codes for recovery

✅ **User Statistics**
```rust
pub struct UserStats {
    pub user_id: UserId,
    pub hands_played: u32,
    pub vpip: f32,              // Voluntarily Put $ In Pot %
    pub pfr: f32,               // Pre-Flop Raise %
    pub wtsd: f32,              // Went To ShowDown %
    pub w_usd: f32,             // Won $ at ShowDown %
    pub net_chips: i64,         // Lifetime profit/loss
    pub last_updated: DateTime<Utc>,
}
```
- Aggregate daily for efficiency (not per-hand)
- Displayed on profile
- Used for skill-based matchmaking (future)

✅ **Rate Limiting & Security**
- **Login**: Max 5 attempts per minute per IP, exponential backoff
- **Registration**: Max 10 per hour per IP
- **Password reset**: Max 1 per 5 minutes per email
- **Failed login lockout**: 15 minutes after 10 failed attempts

### 3. Persistent Wallet System

✅ **Design Principles**
- **Chips are integers (BIGINT)**: No floats, ever
- **Double-entry ledger**: Every mutation creates two entries (debit + credit)
- **Idempotency keys**: Prevent duplicate transactions
- **Escrow model**: Chips locked in table escrow during play

✅ **Core Schema**
```rust
pub struct Wallet {
    pub user_id: UserId,
    pub balance: i64,           // Current balance
    pub currency: CurrencyCode, // Default: CHIP
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

pub struct WalletEntry {
    pub id: EntryId,
    pub user_id: UserId,
    pub table_id: Option<TableId>,
    pub amount: i64,            // Positive = credit, negative = debit
    pub balance_after: i64,
    pub direction: Direction,   // Debit | Credit
    pub entry_type: EntryType,
    pub idempotency_key: String,
    pub description: String,
    pub created_at: DateTime<Utc>,
}

pub enum Direction {
    Debit,      // Money leaving wallet
    Credit,     // Money entering wallet
}

pub enum EntryType {
    BuyIn,      // Join table
    CashOut,    // Leave table
    Rake,       // House fee
    Bonus,      // Faucet/promo
    AdminAdjust,
    Transfer,   // User-to-user (disabled initially)
}

pub type CurrencyCode = String; // "CHIP", "USD", "BTC" (future)
```

✅ **Escrow Model**
```rust
pub struct TableEscrow {
    pub table_id: TableId,
    pub balance: i64,           // Total chips in play at table
}

// On join:
async fn join_table_with_buy_in(
    user_id: UserId,
    table_id: TableId,
    buy_in: Usd,
    idempotency_key: String,
) -> Result<(), WalletError> {
    // Single atomic transaction:
    // 1. Debit user wallet
    // 2. Credit table escrow
    // 3. Create ledger entries
    // 4. Add player to table with chips
}

// On leave:
async fn leave_table_with_cash_out(
    user_id: UserId,
    table_id: TableId,
    chips: Usd,
    idempotency_key: String,
) -> Result<(), WalletError> {
    // Single atomic transaction:
    // 1. Debit table escrow
    // 2. Credit user wallet
    // 3. Create ledger entries
    // 4. Remove player from table
}
```

✅ **Initial Balance & Faucet**
- **Initial balance**: 10,000 chips (configurable via config)
- **Daily faucet**: 1,000 chips per day (24-hour quota)
```rust
pub struct FaucetClaim {
    pub user_id: UserId,
    pub amount: Usd,
    pub claimed_at: DateTime<Utc>,
    pub next_claim_at: DateTime<Utc>,  // +24 hours
}
```

✅ **Minimum Balance & Validation**
- Prevent negative balances (DB constraint + app logic)
- Clear error messages:
  - `InsufficientFunds { required: 600, available: 200 }`
  - CTA: "Claim daily faucet" or "Contact support"

✅ **Wallet Transfers**
- **Disabled at launch** (collusion risk)
- **Admin-only adjustments**: Require 2FA
- Future: P2P transfers with daily limits + fraud detection

✅ **Multi-Currency Ready**
```sql
CREATE TABLE wallets (
    user_id BIGINT PRIMARY KEY,
    currency VARCHAR(10) NOT NULL DEFAULT 'CHIP',
    balance BIGINT NOT NULL,
    ...
);
```
- Store `currency_code` in wallet
- Default: `CHIP` (play money)
- Future: `USD`, `BTC`, etc.

✅ **Future Crypto Integration (Not Now)**
```rust
pub trait PaymentProvider {
    async fn create_deposit(&self, user_id: UserId, amount: Decimal) -> Result<DepositId, PaymentError>;
    async fn check_confirmations(&self, deposit_id: DepositId) -> Result<u8, PaymentError>;
    async fn credit_wallet_idempotent(&self, user_id: UserId, deposit_id: DepositId) -> Result<(), PaymentError>;
}
```
- KYC/AML compliance required before real money
- Custody/HSM for key management
- Regulated entity required

### 4. Automatic Bot Filling

✅ **Core Behavior**
```rust
pub struct BotManager {
    pub table_id: TableId,
    pub target_player_count: usize,  // Default: 5
    pub bots: Vec<InternalBot>,
    pub enabled: bool,
    pub difficulty: BotDifficulty,
    pub next_bot_id: u32,
}

pub enum BotDifficulty {
    Easy,       // Loose-passive, high VPIP (45%), low aggression
    Standard,   // Balanced, moderate VPIP (30%), TAG-style
    TAG,        // Tight-aggressive, low VPIP (20%), high aggression
}

pub struct InternalBot {
    pub bot_id: BotId,
    pub username: Username,     // "Bot_Easy_1"
    pub difficulty: BotDifficulty,
    pub policy: BotPolicy,
    pub chips: Usd,             // Infinite (not from wallet)
}
```

✅ **Auto-Spawning Logic**
```rust
impl BotManager {
    pub fn adjust_bot_count(&mut self, human_count: usize) {
        if !self.enabled {
            return;
        }

        // Cap bot ratio at higher stakes
        let required_humans = self.get_required_human_count();
        if human_count < required_humans {
            // Not enough humans, don't spawn bots
            self.despawn_all_bots();
            return;
        }

        let current_bot_count = self.bots.len();
        let target_bot_count = self.target_player_count.saturating_sub(human_count);

        match current_bot_count.cmp(&target_bot_count) {
            Ordering::Less => {
                for _ in 0..(target_bot_count - current_bot_count) {
                    self.spawn_bot();
                }
            }
            Ordering::Greater => {
                for _ in 0..(current_bot_count - target_bot_count) {
                    self.despawn_bot();
                }
            }
            Ordering::Equal => {}
        }
    }

    fn get_required_human_count(&self) -> usize {
        // Higher stakes require more humans
        match self.get_stakes_tier() {
            StakesTier::Micro => 1,
            StakesTier::Low => 1,
            StakesTier::Mid => 2,
            StakesTier::High => 2,
        }
    }
}
```

✅ **Bot Labeling & Identification**
- Clearly labeled: `Bot_Easy_1`, `Bot_Standard_2`, `Bot_TAG_3`
- Special icon/badge in UI
- Don't use wallet balance (infinite chips)
- Can't chat

✅ **Human-Like Pacing**
```rust
pub struct BotTiming {
    pub min_delay_ms: u64,      // e.g., 800ms
    pub max_delay_ms: u64,      // e.g., 5000ms
    pub timeout_ms: u64,        // e.g., 15000ms (never stall)
}

impl InternalBot {
    pub async fn decide_action(&self, state: &GameState) -> Action {
        // Randomized delay to simulate thinking
        let delay = rand::thread_rng().gen_range(
            self.timing.min_delay_ms..=self.timing.max_delay_ms
        );
        tokio::time::sleep(Duration::from_millis(delay)).await;

        // AI decision
        self.policy.sample(state)
    }
}
```

✅ **Bot Telemetry**
```rust
pub struct BotTelemetry {
    pub bot_id: BotId,
    pub stakes_tier: StakesTier,
    pub hands_played: u32,
    pub win_rate: f32,          // BB/100 hands
    pub vpip: f32,
    pub pfr: f32,
    pub aggression_factor: f32, // (Bets + Raises) / Calls
    pub showdown_rate: f32,
    pub updated_at: DateTime<Utc>,
}
```
- Track bot performance by stakes tier
- Alert if anomalous (e.g., win rate > 15 BB/100)
- Adjust difficulty if needed

✅ **Difficulty Presets**
```rust
pub struct BotPreset {
    pub vpip_range: (f32, f32),     // Easy: (40, 50), Standard: (25, 35), TAG: (18, 24)
    pub pfr_range: (f32, f32),      // Easy: (5, 10), Standard: (15, 20), TAG: (16, 22)
    pub aggression: f32,            // Easy: 0.5, Standard: 1.5, TAG: 2.5
    pub bluff_frequency: f32,       // Easy: 0.1, Standard: 0.2, TAG: 0.25
}
```

✅ **Future: Adaptive Learning**
- Train in sandbox (never from live players' hole cards)
- Reinforcement learning with privacy-preserving data
- Personality variations (Maniac, Nit, LAG, etc.)

---

## Proposed Architecture

### 1. Multi-Table Actor Model

**High-Level Design:**

```
                    ┌─────────────────────────────────────┐
                    │         Main Server Thread          │
                    │  ┌───────────────────────────────┐  │
                    │  │    TableManagerActor          │  │
                    │  │  - Spawns table actors        │  │
                    │  │  - Routes messages            │  │
                    │  │  - Handles discovery          │  │
                    │  └───────────┬───────────────────┘  │
                    └──────────────┼──────────────────────┘
                                   │
           ┌───────────────────────┼───────────────────────┐
           │                       │                       │
    ┌──────▼──────┐         ┌─────▼──────┐         ┌─────▼──────┐
    │  Table 1    │         │  Table 2   │         │  Table 3   │
    │  Actor      │         │  Actor     │         │  Actor     │
    │             │         │            │         │            │
    │ PokerState  │         │ PokerState │         │ PokerState │
    │ BotManager  │         │ BotManager │         │ BotManager │
    │ Inbox       │         │ Inbox      │         │ Inbox      │
    └─────────────┘         └────────────┘         └────────────┘
```

**Core Components:**

```rust
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

pub struct TableManagerActor {
    tables: HashMap<TableId, TableHandle>,
    next_table_id: AtomicU64,
    db_pool: PgPool,
    auth_manager: Arc<AuthManager>,
    wallet_manager: Arc<WalletManager>,
}

pub struct TableHandle {
    pub id: TableId,
    pub sender: mpsc::Sender<TableMessage>,
    pub handle: JoinHandle<()>,
    pub metadata: TableMetadata,
}

pub struct TableMetadata {
    pub name: String,
    pub config: TableConfig,
    pub player_count: usize,
    pub waitlist_count: usize,
    pub created_at: DateTime<Utc>,
}

pub enum TableMessage {
    Join { user_id: UserId, buy_in: Usd, reply: oneshot::Sender<Result<(), GameError>> },
    Leave { user_id: UserId, reply: oneshot::Sender<Result<Usd, GameError>> },
    TakeAction { user_id: UserId, action: Action, reply: oneshot::Sender<Result<(), GameError>> },
    SendChat { user_id: UserId, message: String },
    GetView { user_id: UserId, reply: oneshot::Sender<GameView> },
    // ... other messages
}
```

**Table Actor Implementation:**

```rust
pub struct TableActor {
    pub id: TableId,
    pub state: PokerState,
    pub config: TableConfig,
    pub bot_manager: BotManager,
    pub chat_manager: ChatManager,
    pub inbox: mpsc::Receiver<TableMessage>,
    pub wallet_manager: Arc<WalletManager>,
}

impl TableActor {
    pub async fn run(mut self) {
        loop {
            tokio::select! {
                // Handle incoming messages
                Some(msg) = self.inbox.recv() => {
                    self.handle_message(msg).await;
                }

                // Step game state (timeouts, bot actions)
                _ = tokio::time::sleep(Duration::from_secs(1)) => {
                    self.state = self.state.step();
                    self.handle_bot_turns().await;
                }
            }
        }
    }

    async fn handle_message(&mut self, msg: TableMessage) {
        match msg {
            TableMessage::Join { user_id, buy_in, reply } => {
                let result = self.handle_join(user_id, buy_in).await;
                let _ = reply.send(result);
            }
            TableMessage::Leave { user_id, reply } => {
                let result = self.handle_leave(user_id).await;
                let _ = reply.send(result);
            }
            // ... handle other messages
        }
    }

    async fn handle_join(&mut self, user_id: UserId, buy_in: Usd) -> Result<(), GameError> {
        // Validate buy-in
        self.validate_buy_in(buy_in)?;

        // Reserve chips from wallet to table escrow
        let idempotency_key = format!("join_{}_{}", user_id, self.id);
        self.wallet_manager.transfer_to_escrow(
            user_id,
            self.id,
            buy_in,
            idempotency_key
        ).await?;

        // Add user to game
        let username = self.get_username(user_id)?;
        self.state.new_user(&username)?;

        // Adjust bot count
        let human_count = self.get_human_player_count();
        self.bot_manager.adjust_bot_count(human_count);

        Ok(())
    }

    async fn handle_leave(&mut self, user_id: UserId) -> Result<Usd, GameError> {
        // Get player chips
        let username = self.get_username(user_id)?;
        let chips = self.get_player_chips(&username)?;

        // Remove from game
        self.state.remove_user(&username)?;

        // Return chips from escrow to wallet
        let idempotency_key = format!("leave_{}_{}", user_id, self.id);
        self.wallet_manager.transfer_from_escrow(
            user_id,
            self.id,
            chips,
            idempotency_key
        ).await?;

        // Adjust bot count
        let human_count = self.get_human_player_count();
        self.bot_manager.adjust_bot_count(human_count);

        Ok(chips)
    }

    async fn handle_bot_turns(&mut self) {
        if let Some(next_username) = self.state.get_next_action_username() {
            if self.is_bot(&next_username) {
                let bot = self.bot_manager.get_bot(&next_username);
                let action = bot.decide_action(&self.state).await;
                let _ = self.state.take_action(&next_username, action);
            }
        }
    }
}
```

### 2. Authentication Manager

```rust
use argon2::{Argon2, PasswordHash, PasswordHasher, PasswordVerifier};
use argon2::password_hash::{SaltString, rand_core::OsRng};
use jsonwebtoken::{encode, decode, Header, Validation, EncodingKey, DecodingKey};

pub struct AuthManager {
    db_pool: PgPool,
    pepper: String,
    jwt_secret: String,
    access_ttl: Duration,
    refresh_ttl: Duration,
}

#[derive(Serialize, Deserialize)]
pub struct JwtClaims {
    pub sub: UserId,    // Subject (user_id)
    pub exp: i64,       // Expiration timestamp
    pub iat: i64,       // Issued at timestamp
}

impl AuthManager {
    pub async fn register(
        &self,
        username: &str,
        password: &str
    ) -> Result<UserId, AuthError> {
        // Validate username
        self.validate_username(username)?;

        // Check password strength
        self.validate_password_strength(password)?;

        // Hash password with Argon2id
        let password_hash = self.hash_password(password)?;

        // Insert user into database
        let user_id = sqlx::query_scalar!(
            "INSERT INTO users (username, password_hash) VALUES ($1, $2) RETURNING id",
            username,
            password_hash
        )
        .fetch_one(&self.db_pool)
        .await
        .map_err(|_| AuthError::UsernameAlreadyExists)?;

        // Create wallet with initial balance
        sqlx::query!(
            "INSERT INTO wallets (user_id, balance) VALUES ($1, $2)",
            user_id,
            10000i64
        )
        .execute(&self.db_pool)
        .await?;

        Ok(user_id)
    }

    pub async fn login(
        &self,
        username: &str,
        password: &str,
        device_fingerprint: String
    ) -> Result<SessionToken, AuthError> {
        // Rate limiting check (separate component)
        self.check_rate_limit(username).await?;

        // Fetch user from database
        let user = sqlx::query!(
            "SELECT id, password_hash FROM users WHERE username = $1",
            username
        )
        .fetch_optional(&self.db_pool)
        .await?
        .ok_or(AuthError::InvalidCredentials)?;

        // Verify password
        if !self.verify_password(password, &user.password_hash)? {
            self.record_failed_login(username).await;
            return Err(AuthError::InvalidCredentials);
        }

        // Generate tokens
        let access_token = self.create_access_token(user.id)?;
        let refresh_token = self.create_refresh_token(user.id, device_fingerprint).await?;

        // Update last login
        sqlx::query!(
            "UPDATE users SET last_login = NOW() WHERE id = $1",
            user.id
        )
        .execute(&self.db_pool)
        .await?;

        Ok(SessionToken {
            access_token,
            refresh_token,
        })
    }

    fn hash_password(&self, password: &str) -> Result<String, AuthError> {
        // Add pepper to password
        let peppered = format!("{}{}", password, self.pepper);

        let salt = SaltString::generate(&mut OsRng);
        let argon2 = Argon2::default();

        Ok(argon2
            .hash_password(peppered.as_bytes(), &salt)
            .map_err(|_| AuthError::HashingFailed)?
            .to_string())
    }

    fn verify_password(&self, password: &str, hash: &str) -> Result<bool, AuthError> {
        let peppered = format!("{}{}", password, self.pepper);
        let parsed_hash = PasswordHash::new(hash)
            .map_err(|_| AuthError::InvalidHash)?;

        Ok(Argon2::default()
            .verify_password(peppered.as_bytes(), &parsed_hash)
            .is_ok())
    }

    fn create_access_token(&self, user_id: UserId) -> Result<String, AuthError> {
        let now = Utc::now().timestamp();
        let claims = JwtClaims {
            sub: user_id,
            exp: now + self.access_ttl.as_secs() as i64,
            iat: now,
        };

        encode(
            &Header::default(),
            &claims,
            &EncodingKey::from_secret(self.jwt_secret.as_bytes())
        )
        .map_err(|_| AuthError::TokenGenerationFailed)
    }

    async fn create_refresh_token(
        &self,
        user_id: UserId,
        device_fingerprint: String
    ) -> Result<String, AuthError> {
        let token = uuid::Uuid::new_v4().to_string();
        let expires_at = Utc::now() + chrono::Duration::from_std(self.refresh_ttl).unwrap();

        sqlx::query!(
            "INSERT INTO sessions (token, user_id, device_fingerprint, expires_at)
             VALUES ($1, $2, $3, $4)",
            token,
            user_id,
            device_fingerprint,
            expires_at
        )
        .execute(&self.db_pool)
        .await?;

        Ok(token)
    }

    pub fn validate_access_token(&self, token: &str) -> Result<UserId, AuthError> {
        let token_data = decode::<JwtClaims>(
            token,
            &DecodingKey::from_secret(self.jwt_secret.as_bytes()),
            &Validation::default()
        )
        .map_err(|_| AuthError::InvalidToken)?;

        Ok(token_data.claims.sub)
    }

    pub async fn refresh_tokens(
        &self,
        refresh_token: &str,
        device_fingerprint: String
    ) -> Result<SessionToken, AuthError> {
        // Validate refresh token
        let session = sqlx::query!(
            "SELECT user_id, device_fingerprint, expires_at
             FROM sessions
             WHERE token = $1",
            refresh_token
        )
        .fetch_optional(&self.db_pool)
        .await?
        .ok_or(AuthError::InvalidToken)?;

        // Check expiry
        if session.expires_at < Utc::now() {
            return Err(AuthError::TokenExpired);
        }

        // Check device fingerprint
        if session.device_fingerprint != device_fingerprint {
            return Err(AuthError::DeviceMismatch);
        }

        // Delete old refresh token
        sqlx::query!("DELETE FROM sessions WHERE token = $1", refresh_token)
            .execute(&self.db_pool)
            .await?;

        // Create new tokens
        let access_token = self.create_access_token(session.user_id)?;
        let new_refresh_token = self.create_refresh_token(session.user_id, device_fingerprint).await?;

        Ok(SessionToken {
            access_token,
            refresh_token: new_refresh_token,
        })
    }

    fn validate_username(&self, username: &str) -> Result<(), AuthError> {
        if username.len() < 3 || username.len() > 20 {
            return Err(AuthError::InvalidUsername);
        }

        if !username.chars().all(|c| c.is_alphanumeric() || c == '_') {
            return Err(AuthError::InvalidUsername);
        }

        Ok(())
    }

    fn validate_password_strength(&self, password: &str) -> Result<(), AuthError> {
        if password.len() < 8 {
            return Err(AuthError::WeakPassword);
        }

        let has_uppercase = password.chars().any(|c| c.is_uppercase());
        let has_lowercase = password.chars().any(|c| c.is_lowercase());
        let has_digit = password.chars().any(|c| c.is_numeric());

        if !has_uppercase || !has_lowercase || !has_digit {
            return Err(AuthError::WeakPassword);
        }

        Ok(())
    }
}
```

### 3. Wallet Manager with Escrow

```rust
pub struct WalletManager {
    db_pool: PgPool,
    cache: Arc<RwLock<HashMap<UserId, i64>>>,  // In-memory cache
}

impl WalletManager {
    pub async fn get_balance(&self, user_id: UserId) -> Result<i64, WalletError> {
        // Try cache first
        if let Some(balance) = self.cache.read().await.get(&user_id) {
            return Ok(*balance);
        }

        // Fetch from DB
        let balance = sqlx::query_scalar!(
            "SELECT balance FROM wallets WHERE user_id = $1",
            user_id
        )
        .fetch_one(&self.db_pool)
        .await?;

        // Update cache
        self.cache.write().await.insert(user_id, balance);

        Ok(balance)
    }

    pub async fn transfer_to_escrow(
        &self,
        user_id: UserId,
        table_id: TableId,
        amount: i64,
        idempotency_key: String
    ) -> Result<i64, WalletError> {
        // Check for duplicate transaction
        let existing = sqlx::query!(
            "SELECT id FROM wallet_entries WHERE idempotency_key = $1",
            idempotency_key
        )
        .fetch_optional(&self.db_pool)
        .await?;

        if existing.is_some() {
            return Err(WalletError::DuplicateTransaction);
        }

        // Begin transaction
        let mut tx = self.db_pool.begin().await?;

        // Debit user wallet
        let user_balance = sqlx::query_scalar!(
            "UPDATE wallets
             SET balance = balance - $1, updated_at = NOW()
             WHERE user_id = $2 AND balance >= $1
             RETURNING balance",
            amount,
            user_id
        )
        .fetch_optional(&mut *tx)
        .await?
        .ok_or(WalletError::InsufficientFunds { required: amount, available: 0 })?;

        // Credit table escrow
        sqlx::query!(
            "INSERT INTO table_escrows (table_id, balance)
             VALUES ($1, $2)
             ON CONFLICT (table_id) DO UPDATE
             SET balance = table_escrows.balance + $2",
            table_id as i64,
            amount
        )
        .execute(&mut *tx)
        .await?;

        // Create ledger entries (double-entry)
        // Debit from user
        sqlx::query!(
            "INSERT INTO wallet_entries
             (user_id, table_id, amount, balance_after, direction, entry_type, idempotency_key, description)
             VALUES ($1, $2, $3, $4, 'debit', 'buy_in', $5, $6)",
            user_id,
            table_id as i64,
            -amount,
            user_balance,
            idempotency_key,
            format!("Buy-in to table {}", table_id)
        )
        .execute(&mut *tx)
        .await?;

        // Credit to escrow (stored with negative user_id for escrow account)
        sqlx::query!(
            "INSERT INTO wallet_entries
             (user_id, table_id, amount, balance_after, direction, entry_type, idempotency_key, description)
             VALUES ($1, $2, $3, $4, 'credit', 'buy_in', $5, $6)",
            -(table_id as i64), // Negative ID for escrow account
            table_id as i64,
            amount,
            0i64,  // Escrow doesn't have a persistent balance_after
            idempotency_key,
            format!("Buy-in from user {}", user_id)
        )
        .execute(&mut *tx)
        .await?;

        // Commit transaction
        tx.commit().await?;

        // Update cache
        self.cache.write().await.insert(user_id, user_balance);

        Ok(user_balance)
    }

    pub async fn transfer_from_escrow(
        &self,
        user_id: UserId,
        table_id: TableId,
        amount: i64,
        idempotency_key: String
    ) -> Result<i64, WalletError> {
        // Check for duplicate
        let existing = sqlx::query!(
            "SELECT id FROM wallet_entries WHERE idempotency_key = $1",
            idempotency_key
        )
        .fetch_optional(&self.db_pool)
        .await?;

        if existing.is_some() {
            return Err(WalletError::DuplicateTransaction);
        }

        // Begin transaction
        let mut tx = self.db_pool.begin().await?;

        // Debit table escrow
        sqlx::query!(
            "UPDATE table_escrows
             SET balance = balance - $1
             WHERE table_id = $2 AND balance >= $1",
            amount,
            table_id as i64
        )
        .execute(&mut *tx)
        .await?;

        // Credit user wallet
        let user_balance = sqlx::query_scalar!(
            "UPDATE wallets
             SET balance = balance + $1, updated_at = NOW()
             WHERE user_id = $2
             RETURNING balance",
            amount,
            user_id
        )
        .fetch_one(&mut *tx)
        .await?;

        // Create ledger entries
        // Debit from escrow
        sqlx::query!(
            "INSERT INTO wallet_entries
             (user_id, table_id, amount, balance_after, direction, entry_type, idempotency_key, description)
             VALUES ($1, $2, $3, $4, 'debit', 'cash_out', $5, $6)",
            -(table_id as i64),
            table_id as i64,
            -amount,
            0i64,
            idempotency_key,
            format!("Cash-out to user {}", user_id)
        )
        .execute(&mut *tx)
        .await?;

        // Credit to user
        sqlx::query!(
            "INSERT INTO wallet_entries
             (user_id, table_id, amount, balance_after, direction, entry_type, idempotency_key, description)
             VALUES ($1, $2, $3, $4, 'credit', 'cash_out', $5, $6)",
            user_id,
            table_id as i64,
            amount,
            user_balance,
            idempotency_key,
            format!("Cash-out from table {}", table_id)
        )
        .execute(&mut *tx)
        .await?;

        // Commit
        tx.commit().await?;

        // Update cache
        self.cache.write().await.insert(user_id, user_balance);

        Ok(user_balance)
    }

    pub async fn claim_daily_faucet(&self, user_id: UserId) -> Result<i64, WalletError> {
        // Check last claim time
        let last_claim = sqlx::query_scalar!(
            "SELECT claimed_at FROM faucet_claims
             WHERE user_id = $1
             ORDER BY claimed_at DESC
             LIMIT 1",
            user_id
        )
        .fetch_optional(&self.db_pool)
        .await?;

        if let Some(last) = last_claim {
            let elapsed = Utc::now() - last;
            if elapsed < chrono::Duration::hours(24) {
                return Err(WalletError::FaucetCooldown {
                    next_claim: last + chrono::Duration::hours(24)
                });
            }
        }

        // Credit wallet
        let amount = 1000i64;
        let idempotency_key = format!("faucet_{}_{}", user_id, Utc::now().timestamp());

        let balance = sqlx::query_scalar!(
            "UPDATE wallets
             SET balance = balance + $1, updated_at = NOW()
             WHERE user_id = $2
             RETURNING balance",
            amount,
            user_id
        )
        .fetch_one(&self.db_pool)
        .await?;

        // Record claim
        sqlx::query!(
            "INSERT INTO faucet_claims (user_id, amount, claimed_at, next_claim_at)
             VALUES ($1, $2, NOW(), NOW() + INTERVAL '24 hours')",
            user_id,
            amount
        )
        .execute(&self.db_pool)
        .await?;

        // Create ledger entry
        sqlx::query!(
            "INSERT INTO wallet_entries
             (user_id, amount, balance_after, direction, entry_type, idempotency_key, description)
             VALUES ($1, $2, $3, 'credit', 'bonus', $4, 'Daily faucet')",
            user_id,
            amount,
            balance,
            idempotency_key
        )
        .execute(&self.db_pool)
        .await?;

        // Update cache
        self.cache.write().await.insert(user_id, balance);

        Ok(balance)
    }
}
```

---

## Database Schema

### PostgreSQL 18 Complete Schema

```sql
-- ======================
-- USERS & AUTHENTICATION
-- ======================

CREATE TABLE users (
    id BIGSERIAL PRIMARY KEY,
    username VARCHAR(20) UNIQUE NOT NULL,
    password_hash VARCHAR(255) NOT NULL,

    -- Profile
    display_name VARCHAR(50) NOT NULL,
    avatar_url VARCHAR(500),
    email VARCHAR(255),
    country CHAR(2),              -- ISO country code
    timezone VARCHAR(50),          -- IANA timezone

    -- Consent
    tos_version SMALLINT NOT NULL DEFAULT 1,
    privacy_version SMALLINT NOT NULL DEFAULT 1,

    -- Status
    is_active BOOLEAN NOT NULL DEFAULT TRUE,
    is_admin BOOLEAN NOT NULL DEFAULT FALSE,

    -- Timestamps
    created_at TIMESTAMP NOT NULL DEFAULT NOW(),
    last_login TIMESTAMP,

    CONSTRAINT username_length CHECK (char_length(username) BETWEEN 3 AND 20),
    CONSTRAINT username_format CHECK (username ~ '^[a-zA-Z0-9_]+$')
);

CREATE INDEX idx_users_username ON users(username);
CREATE INDEX idx_users_email ON users(email) WHERE email IS NOT NULL;
CREATE INDEX idx_users_created_at ON users(created_at);

-- ======================
-- SESSIONS
-- ======================

CREATE TABLE sessions (
    token VARCHAR(255) PRIMARY KEY,
    user_id BIGINT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    device_fingerprint VARCHAR(255) NOT NULL,
    created_at TIMESTAMP NOT NULL DEFAULT NOW(),
    expires_at TIMESTAMP NOT NULL,
    last_used TIMESTAMP NOT NULL DEFAULT NOW(),

    CONSTRAINT valid_expiry CHECK (expires_at > created_at)
);

CREATE INDEX idx_sessions_user_id ON sessions(user_id);
CREATE INDEX idx_sessions_expires_at ON sessions(expires_at);

-- Auto-delete expired sessions
CREATE OR REPLACE FUNCTION delete_expired_sessions()
RETURNS void AS $$
BEGIN
    DELETE FROM sessions WHERE expires_at < NOW();
END;
$$ LANGUAGE plpgsql;

-- ======================
-- TWO-FACTOR AUTH
-- ======================

CREATE TABLE two_factor_auth (
    user_id BIGINT PRIMARY KEY REFERENCES users(id) ON DELETE CASCADE,
    secret VARCHAR(255) NOT NULL,
    enabled BOOLEAN NOT NULL DEFAULT FALSE,
    backup_codes TEXT[],  -- Array of hashed backup codes
    created_at TIMESTAMP NOT NULL DEFAULT NOW(),
    enabled_at TIMESTAMP
);

-- ======================
-- PASSWORD RESET
-- ======================

CREATE TABLE password_reset_requests (
    id BIGSERIAL PRIMARY KEY,
    user_id BIGINT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    email VARCHAR(255) NOT NULL,
    code VARCHAR(6) NOT NULL,
    created_at TIMESTAMP NOT NULL DEFAULT NOW(),
    expires_at TIMESTAMP NOT NULL,
    used BOOLEAN NOT NULL DEFAULT FALSE,

    CONSTRAINT valid_expiry CHECK (expires_at > created_at)
);

CREATE INDEX idx_password_reset_user_id ON password_reset_requests(user_id);
CREATE INDEX idx_password_reset_expires_at ON password_reset_requests(expires_at);

-- ======================
-- WALLETS
-- ======================

CREATE TABLE wallets (
    user_id BIGINT PRIMARY KEY REFERENCES users(id) ON DELETE CASCADE,
    balance BIGINT NOT NULL DEFAULT 10000,
    currency VARCHAR(10) NOT NULL DEFAULT 'CHIP',
    created_at TIMESTAMP NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMP NOT NULL DEFAULT NOW(),

    CONSTRAINT positive_balance CHECK (balance >= 0)
);

CREATE INDEX idx_wallets_currency ON wallets(currency);

-- ======================
-- TABLE ESCROWS
-- ======================

CREATE TABLE table_escrows (
    table_id BIGINT PRIMARY KEY,
    balance BIGINT NOT NULL DEFAULT 0,
    created_at TIMESTAMP NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMP NOT NULL DEFAULT NOW(),

    CONSTRAINT positive_balance CHECK (balance >= 0)
);

-- ======================
-- WALLET LEDGER (Double-Entry)
-- ======================

CREATE TABLE wallet_entries (
    id BIGSERIAL PRIMARY KEY,
    user_id BIGINT NOT NULL,  -- Can be negative for escrow accounts
    table_id BIGINT,
    amount BIGINT NOT NULL,   -- Positive = credit, negative = debit
    balance_after BIGINT NOT NULL,
    direction VARCHAR(10) NOT NULL CHECK (direction IN ('debit', 'credit')),
    entry_type VARCHAR(20) NOT NULL CHECK (entry_type IN ('buy_in', 'cash_out', 'rake', 'bonus', 'admin_adjust', 'transfer')),
    idempotency_key VARCHAR(255) UNIQUE NOT NULL,
    description TEXT,
    created_at TIMESTAMP NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_wallet_entries_user_id ON wallet_entries(user_id);
CREATE INDEX idx_wallet_entries_table_id ON wallet_entries(table_id);
CREATE INDEX idx_wallet_entries_created_at ON wallet_entries(created_at);
CREATE INDEX idx_wallet_entries_idempotency_key ON wallet_entries(idempotency_key);

-- ======================
-- FAUCET CLAIMS
-- ======================

CREATE TABLE faucet_claims (
    id BIGSERIAL PRIMARY KEY,
    user_id BIGINT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    amount BIGINT NOT NULL,
    claimed_at TIMESTAMP NOT NULL DEFAULT NOW(),
    next_claim_at TIMESTAMP NOT NULL,

    CONSTRAINT positive_amount CHECK (amount > 0)
);

CREATE INDEX idx_faucet_claims_user_id ON faucet_claims(user_id);
CREATE INDEX idx_faucet_claims_next_claim ON faucet_claims(next_claim_at);

-- ======================
-- TABLES
-- ======================

CREATE TABLE tables (
    id BIGSERIAL PRIMARY KEY,
    name VARCHAR(255) NOT NULL,

    -- Config
    max_players INT NOT NULL DEFAULT 10,
    small_blind BIGINT NOT NULL,
    big_blind BIGINT NOT NULL,
    min_buy_in_bb SMALLINT NOT NULL DEFAULT 20,
    max_buy_in_bb SMALLINT NOT NULL DEFAULT 100,
    absolute_chip_cap BIGINT NOT NULL DEFAULT 100000,
    top_up_cooldown_hands SMALLINT NOT NULL DEFAULT 20,

    -- Features
    speed VARCHAR(10) NOT NULL DEFAULT 'normal' CHECK (speed IN ('normal', 'turbo', 'hyper')),
    bots_enabled BOOLEAN NOT NULL DEFAULT TRUE,
    target_bot_count SMALLINT NOT NULL DEFAULT 5,
    bot_difficulty VARCHAR(10) NOT NULL DEFAULT 'standard' CHECK (bot_difficulty IN ('easy', 'standard', 'tag')),

    -- Privacy
    is_private BOOLEAN NOT NULL DEFAULT FALSE,
    passphrase_hash VARCHAR(255),
    invite_token VARCHAR(255),
    invite_expires_at TIMESTAMP,

    -- Status
    is_active BOOLEAN NOT NULL DEFAULT TRUE,
    created_at TIMESTAMP NOT NULL DEFAULT NOW(),
    creator_user_id BIGINT REFERENCES users(id),

    CONSTRAINT valid_blinds CHECK (big_blind > small_blind),
    CONSTRAINT valid_buy_in CHECK (max_buy_in_bb > min_buy_in_bb)
);

CREATE INDEX idx_tables_is_active ON tables(is_active);
CREATE INDEX idx_tables_speed ON tables(speed);
CREATE INDEX idx_tables_bots_enabled ON tables(bots_enabled);
CREATE INDEX idx_tables_is_private ON tables(is_private);
CREATE INDEX idx_tables_invite_token ON tables(invite_token) WHERE invite_token IS NOT NULL;

-- ======================
-- USER STATISTICS
-- ======================

CREATE TABLE user_stats (
    user_id BIGINT PRIMARY KEY REFERENCES users(id) ON DELETE CASCADE,
    hands_played INT NOT NULL DEFAULT 0,
    vpip REAL NOT NULL DEFAULT 0.0,
    pfr REAL NOT NULL DEFAULT 0.0,
    wtsd REAL NOT NULL DEFAULT 0.0,
    w_usd REAL NOT NULL DEFAULT 0.0,
    net_chips BIGINT NOT NULL DEFAULT 0,
    last_updated TIMESTAMP NOT NULL DEFAULT NOW()
);

-- ======================
-- GAME HISTORY
-- ======================

CREATE TABLE game_history (
    id BIGSERIAL PRIMARY KEY,
    table_id BIGINT NOT NULL,
    game_number BIGINT NOT NULL,
    small_blind BIGINT NOT NULL,
    big_blind BIGINT NOT NULL,
    started_at TIMESTAMP NOT NULL,
    ended_at TIMESTAMP,
    winner_user_id BIGINT REFERENCES users(id),
    pot_size BIGINT,
    num_players INT,

    UNIQUE (table_id, game_number)
);

CREATE INDEX idx_game_history_table_id ON game_history(table_id);
CREATE INDEX idx_game_history_started_at ON game_history(started_at);
CREATE INDEX idx_game_history_winner ON game_history(winner_user_id);

-- ======================
-- HAND HISTORY
-- ======================

CREATE TABLE hand_history (
    id BIGSERIAL PRIMARY KEY,
    game_id BIGINT NOT NULL REFERENCES game_history(id) ON DELETE CASCADE,
    user_id BIGINT NOT NULL REFERENCES users(id),
    hole_cards JSONB,
    position INT NOT NULL,
    actions JSONB,
    final_chips BIGINT NOT NULL,
    showed_hand BOOLEAN NOT NULL DEFAULT FALSE
);

CREATE INDEX idx_hand_history_game_id ON hand_history(game_id);
CREATE INDEX idx_hand_history_user_id ON hand_history(user_id);

-- ======================
-- CHAT MESSAGES
-- ======================

CREATE TABLE chat_messages (
    id BIGSERIAL PRIMARY KEY,
    table_id BIGINT NOT NULL,
    user_id BIGINT NOT NULL REFERENCES users(id),
    message TEXT NOT NULL,
    created_at TIMESTAMP NOT NULL DEFAULT NOW(),

    CONSTRAINT message_length CHECK (char_length(message) <= 500)
);

CREATE INDEX idx_chat_messages_table_id ON chat_messages(table_id);
CREATE INDEX idx_chat_messages_created_at ON chat_messages(created_at);

-- ======================
-- BOT TELEMETRY
-- ======================

CREATE TABLE bot_telemetry (
    id BIGSERIAL PRIMARY KEY,
    bot_id INT NOT NULL,
    table_id BIGINT NOT NULL,
    stakes_tier VARCHAR(10) NOT NULL,
    difficulty VARCHAR(10) NOT NULL,
    hands_played INT NOT NULL,
    win_rate REAL NOT NULL,
    vpip REAL NOT NULL,
    pfr REAL NOT NULL,
    aggression_factor REAL NOT NULL,
    showdown_rate REAL NOT NULL,
    updated_at TIMESTAMP NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_bot_telemetry_table_id ON bot_telemetry(table_id);
CREATE INDEX idx_bot_telemetry_stakes ON bot_telemetry(stakes_tier);

-- ======================
-- RATE LIMITING
-- ======================

CREATE TABLE rate_limit_attempts (
    id BIGSERIAL PRIMARY KEY,
    endpoint VARCHAR(50) NOT NULL,
    identifier VARCHAR(255) NOT NULL,  -- IP or username
    attempts INT NOT NULL DEFAULT 1,
    window_start TIMESTAMP NOT NULL DEFAULT NOW(),
    locked_until TIMESTAMP
);

CREATE INDEX idx_rate_limit_endpoint_identifier ON rate_limit_attempts(endpoint, identifier);
CREATE INDEX idx_rate_limit_window ON rate_limit_attempts(window_start);

-- ======================
-- ANTI-COLLUSION FLAGS
-- ======================

CREATE TABLE collusion_flags (
    id BIGSERIAL PRIMARY KEY,
    user_id BIGINT NOT NULL REFERENCES users(id),
    table_id BIGINT NOT NULL,
    flag_type VARCHAR(50) NOT NULL,
    severity VARCHAR(10) NOT NULL CHECK (severity IN ('low', 'medium', 'high')),
    details JSONB,
    created_at TIMESTAMP NOT NULL DEFAULT NOW(),
    reviewed BOOLEAN NOT NULL DEFAULT FALSE,
    reviewer_user_id BIGINT REFERENCES users(id),
    reviewed_at TIMESTAMP
);

CREATE INDEX idx_collusion_flags_user_id ON collusion_flags(user_id);
CREATE INDEX idx_collusion_flags_table_id ON collusion_flags(table_id);
CREATE INDEX idx_collusion_flags_reviewed ON collusion_flags(reviewed) WHERE NOT reviewed;

-- ======================
-- TOURNAMENTS (Phase B)
-- ======================

CREATE TABLE tournaments (
    id BIGSERIAL PRIMARY KEY,
    name VARCHAR(255) NOT NULL,
    tournament_type VARCHAR(20) NOT NULL CHECK (tournament_type IN ('sit_n_go', 'mtt')),
    buy_in BIGINT NOT NULL,
    starting_chips BIGINT NOT NULL,
    max_players INT NOT NULL,
    current_players INT NOT NULL DEFAULT 0,
    status VARCHAR(20) NOT NULL DEFAULT 'registering' CHECK (status IN ('registering', 'running', 'finished')),
    started_at TIMESTAMP,
    finished_at TIMESTAMP,
    created_at TIMESTAMP NOT NULL DEFAULT NOW()
);

CREATE TABLE tournament_tables (
    id BIGSERIAL PRIMARY KEY,
    tournament_id BIGINT NOT NULL REFERENCES tournaments(id) ON DELETE CASCADE,
    table_number INT NOT NULL,
    table_id BIGINT NOT NULL,
    is_active BOOLEAN NOT NULL DEFAULT TRUE,

    UNIQUE (tournament_id, table_number)
);

CREATE TABLE tournament_players (
    id BIGSERIAL PRIMARY KEY,
    tournament_id BIGINT NOT NULL REFERENCES tournaments(id) ON DELETE CASCADE,
    user_id BIGINT NOT NULL REFERENCES users(id),
    current_table_id BIGINT,
    chips BIGINT NOT NULL,
    placement INT,
    prize BIGINT,
    eliminated_at TIMESTAMP,

    UNIQUE (tournament_id, user_id)
);

-- ======================
-- INITIAL DATA
-- ======================

-- Create default admin user (password: change_me_123)
INSERT INTO users (username, password_hash, display_name, is_admin)
VALUES ('admin', '$argon2id$v=19$m=19456,t=2,p=1$...', 'Administrator', TRUE);

-- Create default wallet for admin
INSERT INTO wallets (user_id, balance)
VALUES (1, 1000000);
```

---

## Implementation Phases

### Phase 1: Foundation (Weeks 1-3)

**Milestone 1.1: Database & Schema (Week 1)**
- ✅ Set up PostgreSQL 18
- ✅ Create complete schema with migrations
- ✅ Set up sqlx + database connection pooling
- ✅ Create seed data script

**Deliverables:**
- `private_poker/src/db/schema.sql`
- `private_poker/src/db/migrations/`
- Database initialization scripts
- Connection pool configuration

**Milestone 1.2: Authentication System (Week 2)**
- ✅ Implement `AuthManager`
- ✅ Registration with Argon2id + pepper
- ✅ Login with JWT access + refresh tokens
- ✅ Password reset flow
- ✅ 2FA (TOTP) setup

**Deliverables:**
- `private_poker/src/auth/manager.rs`
- `private_poker/src/auth/models.rs`
- `private_poker/src/auth/errors.rs`
- `private_poker/src/auth/jwt.rs`
- `private_poker/src/auth/totp.rs`
- Unit tests for auth flows

**Milestone 1.3: Wallet System (Week 3)**
- ✅ Implement `WalletManager`
- ✅ Double-entry ledger operations
- ✅ Escrow transfer logic
- ✅ Idempotency key handling
- ✅ Daily faucet

**Deliverables:**
- `private_poker/src/wallet/manager.rs`
- `private_poker/src/wallet/models.rs`
- `private_poker/src/wallet/errors.rs`
- `private_poker/src/wallet/ledger.rs`
- Integration tests for wallet transactions

**Dependencies:**
```toml
[dependencies]
sqlx = { version = "0.7", features = ["runtime-tokio-native-tls", "postgres", "chrono", "uuid"] }
argon2 = "0.5"
jsonwebtoken = "9.2"
totp-rs = "5.0"
uuid = { version = "1.6", features = ["v4", "serde"] }
chrono = { version = "0.4", features = ["serde"] }
```

### Phase 2: Multi-Table Infrastructure (Weeks 4-6)

**Milestone 2.1: Table Actor Model (Week 4)**
- ✅ Implement `TableActor` with async inbox
- ✅ Implement `TableManagerActor`
- ✅ Message-passing infrastructure
- ✅ Table lifecycle management

**Deliverables:**
- `private_poker/src/tables/actor.rs`
- `private_poker/src/tables/manager.rs`
- `private_poker/src/tables/messages.rs`
- `private_poker/src/tables/config.rs`

**Milestone 2.2: Table Operations (Week 5)**
- ✅ Create/List/Join/Leave table
- ✅ Waitlist system
- ✅ Buy-in validation (BB + absolute cap)
- ✅ Top-up cooldown tracking
- ✅ Private table access (passphrase/invite)

**Deliverables:**
- `private_poker/src/tables/operations.rs`
- `private_poker/src/tables/waitlist.rs`
- `private_poker/src/tables/validation.rs`
- `private_poker/src/tables/access_control.rs`

**Milestone 2.3: Discovery & Filtering (Week 6)**
- ✅ Table discovery API
- ✅ Filter by stakes/speed/bots/privacy
- ✅ Spectating with throttled updates
- ✅ Table chat with rate limiting

**Deliverables:**
- `private_poker/src/tables/discovery.rs`
- `private_poker/src/tables/spectate.rs`
- `private_poker/src/tables/chat.rs`
- `private_poker/src/tables/profanity_filter.rs`

### Phase 3: Bot System (Weeks 7-8)

**Milestone 3.1: Bot Manager (Week 7)**
- ✅ Implement `BotManager` per table
- ✅ Auto-spawn/despawn logic
- ✅ Bot ratio caps by stakes tier
- ✅ Human-like pacing with timeouts

**Deliverables:**
- `private_poker/src/bots/manager.rs`
- `private_poker/src/bots/internal_bot.rs`
- `private_poker/src/bots/timing.rs`

**Milestone 3.2: Bot Intelligence (Week 8)**
- ✅ Difficulty presets (Easy/Standard/TAG)
- ✅ Q-learning policy per difficulty
- ✅ Bot telemetry tracking
- ✅ Anomaly detection alerts

**Deliverables:**
- `private_poker/src/bots/policy.rs`
- `private_poker/src/bots/presets.rs`
- `private_poker/src/bots/telemetry.rs`
- Bot behavior tests

### Phase 4: Security & Anti-Collusion (Weeks 9-10)

**Milestone 4.1: Rate Limiting (Week 9)**
- ✅ Implement rate limiting middleware
- ✅ Per-endpoint limits
- ✅ Exponential backoff on failed logins
- ✅ IP-based throttling

**Deliverables:**
- `private_poker/src/security/rate_limiter.rs`
- `private_poker/src/security/middleware.rs`

**Milestone 4.2: Anti-Collusion (Week 10)**
- ✅ Seat randomization on join
- ✅ Single-human-per-IP enforcement (optional)
- ✅ Shadow flagging system
- ✅ Admin review dashboard (basic)

**Deliverables:**
- `private_poker/src/security/anti_collusion.rs`
- `private_poker/src/security/flagging.rs`
- Admin CLI tool for flag review

### Phase 5: Protocol & Client Integration (Weeks 11-12)

**Milestone 5.1: Protocol Updates (Week 11)**
- ✅ Update message types for multi-table
- ✅ Add authentication messages
- ✅ Add table management messages
- ✅ Versioned protocol negotiation

**Deliverables:**
- `private_poker/src/net/messages.rs` (updated)
- `private_poker/src/net/protocol_version.rs`
- Protocol compatibility tests

**Milestone 5.2: Client Updates (Week 12)**
- ✅ Login screen UI
- ✅ Table browser UI
- ✅ Wallet display
- ✅ Table chat UI
- ✅ Tournament lobby (Phase A)

**Deliverables:**
- `pp_client/src/screens/login.rs`
- `pp_client/src/screens/table_browser.rs`
- `pp_client/src/screens/wallet.rs`
- `pp_client/src/widgets/chat.rs`

### Phase 6: Testing & Polish (Weeks 13-14)

**Milestone 6.1: Integration Testing (Week 13)**
- ✅ End-to-end multi-table tests
- ✅ Wallet transaction tests
- ✅ Auth flow tests
- ✅ Bot behavior tests
- ✅ Load testing (100 users, 20 tables)

**Milestone 6.2: Documentation & Deployment (Week 14)**
- ✅ API documentation
- ✅ User manual
- ✅ Admin guide
- ✅ Deployment scripts
- ✅ Monitoring setup (Prometheus/Grafana)

### Phase 7: Tournament Mode (Weeks 15-16) - Phase A Only

**Milestone 7.1: Sit-n-Go (Week 15)**
- ✅ Single-table tournament logic
- ✅ Blind schedule progression
- ✅ Payout distribution
- ✅ Tournament lobby UI

**Milestone 7.2: Testing & Polish (Week 16)**
- ✅ Tournament integration tests
- ✅ Blind schedule testing
- ✅ Payout calculation tests

**Phase B (MTT) deferred to future sprint**

---

## API Changes

### Authentication Messages

```rust
pub enum ClientMessage {
    // Authentication (NEW)
    Register {
        username: String,
        password: String,
        email: Option<String>,
    },
    Login {
        username: String,
        password: String,
        device_fingerprint: String,
    },
    RefreshToken {
        refresh_token: String,
        device_fingerprint: String,
    },
    Logout,

    // 2FA (NEW)
    Enable2FA { secret: String, code: String },
    Verify2FA { code: String },

    // Password Reset (NEW)
    RequestPasswordReset { email: String },
    ResetPassword { email: String, code: String, new_password: String },

    // Table Management (NEW)
    CreateTable { config: TableConfig },
    ListTables { filter: Option<TableFilter> },
    JoinTable { table_id: TableId, buy_in: Usd, passphrase: Option<String> },
    LeaveTable { table_id: TableId },
    JoinWaitlist { table_id: TableId },
    LeaveWaitlist { table_id: TableId },
    SpectateTable { table_id: TableId },

    // Wallet (NEW)
    GetBalance,
    ClaimFaucet,
    GetTransactionHistory { limit: usize, offset: usize },

    // Chat (NEW)
    SendChatMessage { table_id: TableId, message: String },
    MuteUser { table_id: TableId, user_id: UserId },
    KickUser { table_id: TableId, user_id: UserId },

    // Existing (updated with table_id)
    TakeAction { table_id: TableId, action: Action },
    CastVote { table_id: TableId, vote: Vote },
    StartGame { table_id: TableId },
    ShowHand { table_id: TableId },
}

pub enum ServerMessage {
    // Authentication responses (NEW)
    RegisterSuccess { user_id: UserId },
    LoginSuccess {
        session: SessionToken,
        user_profile: UserProfile,
        wallet_balance: i64,
    },
    RefreshSuccess { session: SessionToken },
    LogoutSuccess,

    // 2FA responses (NEW)
    TwoFactorRequired,
    TwoFactorEnabled { backup_codes: Vec<String> },
    TwoFactorVerified,

    // Password reset (NEW)
    PasswordResetCodeSent,
    PasswordResetSuccess,

    // Table responses (NEW)
    TableCreated { table_id: TableId },
    TableList { tables: Vec<TableInfo> },
    JoinedTable { table_id: TableId },
    LeftTable { table_id: TableId, chips_returned: i64 },
    JoinedWaitlist { table_id: TableId, position: usize },
    LeftWaitlist { table_id: TableId },
    SpectatingTable { table_id: TableId },

    // Wallet responses (NEW)
    Balance { amount: i64, currency: String },
    FaucetClaimed { amount: i64, next_claim: DateTime<Utc> },
    TransactionHistory { entries: Vec<WalletEntry> },

    // Chat (NEW)
    ChatMessage {
        table_id: TableId,
        user_id: UserId,
        username: String,
        message: String,
        timestamp: DateTime<Utc>,
    },
    UserMuted { table_id: TableId, user_id: UserId },
    UserKicked { table_id: TableId, user_id: UserId },

    // Error responses (NEW)
    AuthError(AuthError),
    WalletError(WalletError),
    TableError(TableError),
    RateLimitError { retry_after: Duration },

    // Existing (updated with table_id)
    Ack { table_id: TableId, message: ClientMessage },
    GameView { table_id: TableId, view: GameView },
    TurnSignal { table_id: TableId, action_choices: ActionChoices },
    GameEvent { table_id: TableId, event: GameEvent },
    Status { table_id: TableId, message: String },
    UserError { table_id: TableId, error: UserError },
    ClientError(ClientError),
}

pub struct TableInfo {
    pub id: TableId,
    pub name: String,
    pub config: TableConfig,
    pub player_count: usize,
    pub waitlist_count: usize,
    pub stakes_tier: StakesTier,
    pub is_private: bool,
    pub requires_passphrase: bool,
    pub has_invite: bool,
}
```

---

## Security Considerations

### 1. Password Security
- **Argon2id** with default parameters (19MB memory, 2 iterations)
- Unique salt per user (generated with `OsRng`)
- Server-side pepper (stored in config, rotated yearly)
- Minimum password strength enforced

### 2. Session Security
- **Access tokens**: JWT, 15-minute expiry, signed with HS256
- **Refresh tokens**: UUID v4, stored in DB, 7-day expiry
- **Token rotation**: On refresh, invalidate old refresh token
- **Device fingerprinting**: User-Agent + IP hash
- **Revocation**: Delete refresh token on logout

### 3. Rate Limiting
```rust
// Login: 5 attempts/minute per IP
// Registration: 10/hour per IP
// Chat: 1 message/3 seconds per user
// Faucet: 1 claim/24 hours per user

pub struct RateLimitConfig {
    pub max_attempts: usize,
    pub window_duration: Duration,
    pub lockout_duration: Duration,
}
```

### 4. Database Security
- **Prepared statements**: Prevent SQL injection
- **Connection pooling**: Max 20 connections
- **SSL/TLS**: Required for production
- **Row-level security**: Future enhancement
- **Audit logging**: All wallet mutations logged

### 5. Anti-Collusion
- **Seat randomization**: Random seat assignment
- **IP restrictions**: Optional single-human-per-IP per table
- **Pattern detection**: Track win rates vs same IP, fold rate correlation
- **Shadow flags**: Log suspicious patterns for admin review

---

## Anti-Collusion & Fairness

### 1. Seat Randomization

```rust
impl TableActor {
    pub fn assign_random_seat(&mut self, user_id: UserId) -> SeatIndex {
        let mut open_seats: Vec<SeatIndex> = self.config.open_seats.clone();
        open_seats.shuffle(&mut thread_rng());
        open_seats.pop().unwrap()
    }
}
```

### 2. IP-Based Detection

```rust
pub struct IpTracker {
    table_id: TableId,
    user_ips: HashMap<UserId, IpAddr>,
}

impl IpTracker {
    pub fn check_single_ip_rule(&self, user_id: UserId, ip: IpAddr) -> Result<(), TableError> {
        for (other_user, other_ip) in &self.user_ips {
            if user_id != *other_user && ip == *other_ip {
                return Err(TableError::MultipleUsersFromSameIp);
            }
        }
        Ok(())
    }
}
```

### 3. Pattern Flagging

```rust
pub struct CollusionDetector {
    db_pool: PgPool,
}

impl CollusionDetector {
    pub async fn analyze_patterns(&self, table_id: TableId) -> Vec<CollusionFlag> {
        let mut flags = Vec::new();

        // Detect: High win rate vs same IP
        let same_ip_pairs = self.find_same_ip_pairs(table_id).await;
        for (user_a, user_b, ip) in same_ip_pairs {
            let win_rate = self.calculate_win_rate_vs(user_a, user_b).await;
            if win_rate > 0.7 {
                flags.push(CollusionFlag {
                    user_id: user_a,
                    table_id,
                    flag_type: "high_win_rate_same_ip".to_string(),
                    severity: "high".to_string(),
                    details: json!({
                        "other_user": user_b,
                        "ip": ip.to_string(),
                        "win_rate": win_rate
                    }),
                });
            }
        }

        // Detect: Fold rate correlation
        let fold_correlation = self.calculate_fold_correlation(table_id).await;
        if fold_correlation > 0.9 {
            flags.push(CollusionFlag {
                user_id: 0, // Group flag
                table_id,
                flag_type: "fold_correlation".to_string(),
                severity: "medium".to_string(),
                details: json!({ "correlation": fold_correlation }),
            });
        }

        flags
    }
}
```

### 4. Admin Review Dashboard

```rust
// Admin CLI tool
pub async fn review_collusion_flags(db_pool: &PgPool) {
    let flags = sqlx::query_as!(
        CollusionFlag,
        "SELECT * FROM collusion_flags WHERE NOT reviewed ORDER BY created_at DESC"
    )
    .fetch_all(db_pool)
    .await
    .unwrap();

    for flag in flags {
        println!("Flag #{}: {:?}", flag.id, flag);
        println!("Review? [approve/reject/skip]: ");
        // ... handle admin input
    }
}
```

---

## Testing Strategy

### 1. Unit Tests

**Authentication:**
```rust
#[tokio::test]
async fn test_register_user() {
    let auth = AuthManager::new(test_db_pool(), "test_pepper".to_string()).await;
    let user_id = auth.register("alice", "Password123").await.unwrap();
    assert!(user_id > 0);
}

#[tokio::test]
async fn test_login_invalid_password() {
    let auth = AuthManager::new(test_db_pool(), "test_pepper".to_string()).await;
    auth.register("alice", "Password123").await.unwrap();
    let result = auth.login("alice", "WrongPassword", "fingerprint".to_string()).await;
    assert!(matches!(result, Err(AuthError::InvalidCredentials)));
}
```

**Wallet:**
```rust
#[tokio::test]
async fn test_wallet_escrow_transfer() {
    let wallet_mgr = WalletManager::new(test_db_pool()).await;
    let user_id = 1;
    let table_id = 1;
    let buy_in = 600;

    let balance = wallet_mgr.transfer_to_escrow(
        user_id,
        table_id,
        buy_in,
        "idempotency_key_1".to_string()
    ).await.unwrap();

    assert_eq!(balance, 10000 - 600);
}

#[tokio::test]
async fn test_wallet_idempotency() {
    let wallet_mgr = WalletManager::new(test_db_pool()).await;
    let key = "idempotency_key_2".to_string();

    wallet_mgr.transfer_to_escrow(1, 1, 600, key.clone()).await.unwrap();
    let result = wallet_mgr.transfer_to_escrow(1, 1, 600, key).await;

    assert!(matches!(result, Err(WalletError::DuplicateTransaction)));
}
```

**Multi-Table:**
```rust
#[tokio::test]
async fn test_table_actor_join_leave() {
    let (table_actor, mut handle) = create_test_table(1).await;

    // Join table
    let (tx, rx) = oneshot::channel();
    table_actor.send(TableMessage::Join {
        user_id: 1,
        buy_in: 600,
        reply: tx,
    }).await.unwrap();

    let result = rx.await.unwrap();
    assert!(result.is_ok());

    // Leave table
    let (tx, rx) = oneshot::channel();
    table_actor.send(TableMessage::Leave {
        user_id: 1,
        reply: tx,
    }).await.unwrap();

    let chips = rx.await.unwrap().unwrap();
    assert_eq!(chips, 600);
}
```

**Bot Manager:**
```rust
#[test]
fn test_bot_manager_auto_spawn() {
    let mut bot_mgr = BotManager::new(5, BotDifficulty::Standard);

    // 2 humans join
    bot_mgr.adjust_bot_count(2);
    assert_eq!(bot_mgr.bots.len(), 3); // Should spawn 3 bots

    // 3rd human joins
    bot_mgr.adjust_bot_count(3);
    assert_eq!(bot_mgr.bots.len(), 2); // Should despawn 1 bot

    // All humans leave
    bot_mgr.adjust_bot_count(0);
    assert_eq!(bot_mgr.bots.len(), 5); // Should spawn 5 bots
}
```

### 2. Integration Tests

```rust
#[tokio::test]
async fn test_full_multi_table_flow() {
    // 1. Register 3 users
    let auth = AuthManager::new(test_db_pool(), "pepper".to_string()).await;
    let alice = auth.register("alice", "Password123").await.unwrap();
    let bob = auth.register("bob", "Password123").await.unwrap();
    let charlie = auth.register("charlie", "Password123").await.unwrap();

    // 2. Create 2 tables
    let table_mgr = TableManagerActor::new(test_config()).await;
    let table1 = table_mgr.create_table("Table 1", default_config()).await.unwrap();
    let table2 = table_mgr.create_table("Table 2", default_config()).await.unwrap();

    // 3. Alice joins table 1, Bob joins table 2, Charlie joins table 1
    table_mgr.join_table(alice, table1, 600).await.unwrap();
    table_mgr.join_table(bob, table2, 600).await.unwrap();
    table_mgr.join_table(charlie, table1, 600).await.unwrap();

    // 4. Check bot auto-fill
    let table1_info = table_mgr.get_table_info(table1).await.unwrap();
    assert_eq!(table1_info.player_count, 5); // 2 humans + 3 bots

    let table2_info = table_mgr.get_table_info(table2).await.unwrap();
    assert_eq!(table2_info.player_count, 5); // 1 human + 4 bots

    // 5. Play some hands
    table_mgr.start_game(table1).await.unwrap();
    // ... simulate gameplay

    // 6. Users leave
    let alice_chips = table_mgr.leave_table(alice, table1).await.unwrap();
    let bob_chips = table_mgr.leave_table(bob, table2).await.unwrap();

    // 7. Check wallet balances updated
    let wallet_mgr = WalletManager::new(test_db_pool()).await;
    let alice_balance = wallet_mgr.get_balance(alice).await.unwrap();
    assert_eq!(alice_balance, 10000 - 600 + alice_chips);
}
```

### 3. Load Testing

```bash
# Simulate 100 concurrent users across 20 tables
cargo test --release --test load_test_multi_table -- --nocapture

# Expected metrics:
# - Table creation: < 50ms
# - Join table: < 100ms (wallet + escrow transaction)
# - Leave table: < 100ms
# - Game step: < 10ms
# - P95 response time: < 500ms
# - CPU usage: < 80% on 4 cores
# - Memory: < 2GB for 100 users
# - No deadlocks or race conditions
```

### 4. Security Testing

```bash
# SQL injection attempts
cargo test test_sql_injection_username
cargo test test_sql_injection_chat

# Password brute force
cargo test test_rate_limit_login

# Session hijacking
cargo test test_token_rotation
cargo test test_device_fingerprint_mismatch

# Wallet race conditions
cargo test test_concurrent_wallet_transactions
```

---

## Migration Strategy

### 1. Backward Compatibility

**Challenge:** Existing clients expect single-table, no-auth protocol.

**Solution:** Protocol versioning with legacy mode.

```rust
pub enum ProtocolVersion {
    V1,  // Original (single-table, no auth)
    V2,  // New (multi-table, auth)
}

impl Server {
    async fn handle_new_connection(&mut self, stream: TcpStream) {
        // Detect version from first message
        let first_msg = read_message(&stream).await;

        let version = match first_msg {
            ClientMessage::Login { .. } => ProtocolVersion::V2,
            ClientMessage::Register { .. } => ProtocolVersion::V2,
            ClientMessage::Connect { .. } => ProtocolVersion::V1,
            _ => ProtocolVersion::V1,
        };

        if version == ProtocolVersion::V1 {
            // Legacy mode: auto-create temp user, assign to default table
            self.handle_legacy_client(stream).await;
        } else {
            // Normal V2 flow
            self.handle_v2_client(stream).await;
        }
    }

    async fn handle_legacy_client(&mut self, stream: TcpStream) {
        // Create anonymous user with temp username
        let temp_username = format!("Guest_{}", Uuid::new_v4());
        let temp_password = Uuid::new_v4().to_string();

        let user_id = self.auth_manager
            .register(&temp_username, &temp_password)
            .await
            .unwrap();

        // Auto-join default table
        let default_table = self.get_or_create_default_table().await;
        self.table_manager
            .join_table(user_id, default_table, 600)
            .await
            .unwrap();

        // Handle as normal client, but strip table_id from messages
        self.handle_legacy_session(stream, user_id, default_table).await;
    }
}
```

### 2. Phased Rollout

**Phase 1:** Deploy with V1 compatibility (Week 15)
- Old clients continue working (single default table, no registration)
- New clients can use multi-table + auth features
- Monitor metrics: % V1 vs V2 clients

**Phase 2:** Encourage migration (Week 16-18)
- Send in-game notification to V1 clients: "Upgrade to unlock multi-table!"
- Provide migration guide
- Track user migration rate

**Phase 3:** Deprecate V1 (Week 20+)
- Announce deprecation timeline (e.g., 30 days)
- Restrict V1 to default table only (no bot fill)
- Force upgrade after deadline

### 3. Data Migration

**Current State:**
- No persistent data (in-memory only)
- Server restart = clean slate

**Migration Steps:**

1. **Initial Deployment:**
   ```bash
   # Set up database
   psql -U postgres -c "CREATE DATABASE poker_db;"
   psql -U postgres poker_db < schema.sql

   # Insert default admin user
   cargo run --bin admin -- create-user admin change_me_123 --admin

   # Start server
   cargo run --bin pp_server --release
   ```

2. **Existing Users (if needed):**
   ```rust
   // If server had in-memory ledger, can export to CSV before shutdown
   async fn export_ledger_to_csv(ledger: &HashMap<Username, Usd>) {
       let mut wtr = csv::Writer::from_path("ledger_export.csv").unwrap();
       for (username, balance) in ledger {
           wtr.write_record(&[username.to_string(), balance.to_string()]).unwrap();
       }
   }

   // Then import into database
   async fn import_ledger_from_csv(db_pool: &PgPool) {
       let mut rdr = csv::Reader::from_path("ledger_export.csv").unwrap();
       for result in rdr.records() {
           let record = result.unwrap();
           let username = &record[0];
           let balance: i64 = record[1].parse().unwrap();

           // Create user with balance
           sqlx::query!(
               "INSERT INTO users (username, password_hash, display_name)
                VALUES ($1, $2, $1)",
               username,
               "$argon2id$..." // Default password
           )
           .execute(db_pool)
           .await.unwrap();

           let user_id = sqlx::query_scalar!(
               "SELECT id FROM users WHERE username = $1",
               username
           )
           .fetch_one(db_pool)
           .await.unwrap();

           sqlx::query!(
               "UPDATE wallets SET balance = $1 WHERE user_id = $2",
               balance,
               user_id
           )
           .execute(db_pool)
           .await.unwrap();
       }
   }
   ```

---

**End of Document**

This plan is now ready for review and implementation. All requirements have been incorporated with detailed designs, database schemas, code examples, and phased rollout strategy.
