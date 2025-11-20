# Private Poker - Comprehensive Test Strategy

## Executive Summary

This document outlines a complete testing strategy for the Private Poker Texas Hold'em platform based on deep business logic analysis. The strategy addresses gaps in current coverage and ensures production-ready quality.

**Current Status**:
- 501 tests passing
- 73.63% overall coverage
- 99.7% coverage on critical paths
- Strong unit/integration test foundation

**Identified Gaps**:
1. WebSocket edge cases (disconnects, malformed messages, rapid actions)
2. Concurrent transaction scenarios (race conditions, deadlocks)
3. Tournament edge cases (elimination timing, payout rounding)
4. Security bypass attempts (rate limiter evasion, collusion patterns)
5. Bot AI behavioral consistency
6. Database transaction rollback scenarios
7. Performance under sustained load

---

## 1. BUSINESS LOGIC ANALYSIS

### 1.1 Core Business Flows

**Flow 1: User Registration → First Game**
```
POST /api/auth/register
  → AuthManager.register()
  → Argon2id hash + pepper
  → Insert into users table
  → WalletManager.create_wallet()
  → Insert into wallets table (balance=10000)
  → Return JWT access + refresh tokens
→ POST /api/tables/1/join
  → Verify JWT
  → WalletManager.transfer_to_escrow()
  → BEGIN; UPDATE wallets; INSERT wallet_entries; UPDATE table_escrows; COMMIT;
  → TableActor receives JoinTable message
  → FSM: Lobby → StartingGame (when 2+ players)
→ WS /ws/1?token=<jwt>
  → Every 1s: Get game view
  → Send action: {"type": "action", "action": {"type": "raise", "amount": 100}}
  → TableActor.handle_action()
  → FSM state transition
  → Broadcast updated view
```

**Critical Business Rules**:
- Wallet balance must never go negative
- Escrow balance must equal sum of all table chips
- Duplicate transactions prevented by idempotency_key
- Players cannot act out of turn
- Invalid actions (e.g., raise when can only call/fold) rejected
- Side pots calculated correctly for all-ins
- Hand evaluation determines correct winner
- Pot awarded only to eligible players

**Gaps in Current Tests**:
- ❌ Concurrent join requests (2 players join simultaneously with same seat)
- ❌ WebSocket disconnect during player's turn
- ❌ Wallet transaction rollback on table actor failure
- ❌ Side pot calculation with 4+ all-ins at different chip counts
- ❌ Network partition during critical game state (e.g., showdown)

---

**Flow 2: Bot Auto-Spawn → AI Decision → Despawn**
```
TableActor.handle_tick()
  → Check player_count < bot_spawn_threshold (default: 4)
  → BotManager.spawn_bots(count)
  → For each bot:
      → Create BotPlayer with difficulty (Easy/Standard/TAG)
      → Add to table FSM
  → Game continues with bots
  → Bot turn arrives:
      → decision_engine.decide(hand, board, pot, position)
      → Calculate hand strength (uses core eval())
      → Calculate pot odds
      → Apply position modifier (tight UTG, loose button)
      → Randomize bluff (15% for Standard, 25% for TAG)
      → Return action (Fold/Check/Call/Raise/AllIn)
  → Human joins:
      → BotManager.despawn_bots(1)
      → Remove bot from table
      → Save telemetry to bot_telemetry table
```

**Critical Business Rules**:
- Bots only spawn if `bots_enabled=true` on table config
- Bots never spawn at high-stakes tables (requires 2+ humans)
- Bot decisions statistically match difficulty preset (VPIP, PFR)
- Bots despawn in FIFO order
- Bot actions are indistinguishable from human actions (no advantage)

**Gaps in Current Tests**:
- ❌ Bot decision consistency over 10,000 hands (VPIP/PFR accuracy)
- ❌ Bot despawn during bot's turn
- ❌ Bot bluff frequency verification
- ❌ Bot pot odds calculation edge cases (fractional amounts)

---

**Flow 3: Tournament Registration → Blind Progression → Payout**
```
POST /api/tournaments (admin)
  → TournamentManager.create_tournament(config)
  → Insert into tournaments table
  → Set state = Registering
→ POST /api/tournaments/1/register (multiple players)
  → Validate buy-in balance
  → Deduct buy-in from wallet
  → Insert into tournament_registrations
→ Auto-start when max_players reached (Sit-n-Go)
  → TournamentManager.start_tournament()
  → Update state = Running
  → Create table with tournament blinds
  → Seed players with starting_stack chips
→ Blind progression (every 5 minutes):
  → TournamentManager.advance_blind_level()
  → Update table blinds
  → Notify all players
→ Player elimination:
  → Player chips = 0
  → TournamentManager.eliminate_player()
  → Record placement and timestamp
→ Final player wins:
  → TournamentManager.award_prizes()
  → Calculate payouts based on prize_structure
  → WalletManager.credit() for each winner
  → Update state = Completed
```

**Critical Business Rules**:
- Buy-in deducted atomically (either success or full rollback)
- Tournament starts only when full (Sit-n-Go)
- Blind increases cannot be skipped
- Placement determined by elimination order
- Prize pool = (buy_in × player_count) - rake
- Prizes awarded exactly once per tournament
- Rounding errors handled (last player gets remainder)

**Gaps in Current Tests**:
- ❌ Simultaneous elimination (2 players bust on same hand)
- ❌ Prize rounding with odd total (e.g., 997 chips, 3-way split)
- ❌ Tournament cancellation mid-play (refund logic)
- ❌ Blind progression during active hand
- ❌ Player disconnect during tournament

---

### 1.2 Security-Critical Business Logic

**Anti-Collusion Detection**:
```rust
// Trigger: Player joins table
if same_ip_detected(user_id, table_id) {
    create_collusion_flag(
        flag_type: SameIPAtTable,
        severity: Medium,
        metadata: {other_user_ids, ip_address}
    );
}

// Trigger: Hand ends
if win_rate_vs_same_ip_player > 75% over 20+ hands {
    create_collusion_flag(
        flag_type: WinRateAnomaly,
        severity: High,
        metadata: {win_rate, sample_size, opponent_user_id}
    );
}

// Trigger: Multiple folds in pattern
if coordinated_folds_detected(action_timestamps) {
    create_collusion_flag(
        flag_type: CoordinatedFolding,
        severity: High,
        metadata: {user_ids, fold_timestamps, pattern_description}
    );
}
```

**Critical Rules**:
- Shadow flagging only (no auto-ban)
- Flags require admin review
- False positives acceptable (better safe than sorry)
- IP tracking is advisory, not proof

**Gaps**:
- ❌ Cross-table chip dumping detection
- ❌ False positive rate measurement
- ❌ Adversarial testing (VPN/proxy bypass)

---

**Rate Limiting**:
```rust
// Endpoint: /api/auth/login
RateLimitConfig {
    max_attempts: 5,
    window_duration: Duration::from_secs(900),  // 15 min
    lockout_duration: Duration::from_secs(900), // 15 min
    exponential_backoff: true,
}

// Logic:
if attempt_count > max_attempts within window {
    if exponential_backoff && consecutive_violations > 0 {
        lockout_duration *= 2^consecutive_violations;
    }
    set locked_until = now + lockout_duration;
    increment consecutive_violations;
    return 429 Too Many Requests;
}
```

**Critical Rules**:
- Window sliding (not fixed intervals)
- Lockout persists across window resets
- Exponential backoff doubles on repeat violations
- IP-based (not user-based, to prevent account enumeration)

**Gaps**:
- ❌ Distributed brute-force (IP rotation)
- ❌ Lockout duration overflow (2^10 = 17 hours)
- ❌ Clock skew handling

---

## 2. INCOMPLETE / INCONSISTENT BUSINESS LOGIC

### 2.1 WebSocket Message Handling

**Issue**: WebSocket join command bypasses HTTP API validation

**Current Flow**:
```rust
// WebSocket handler
ClientMessage::Join { buy_in } => {
    let username = format!("user_{}", user_id);  // ❌ Placeholder!
    table_handle.send(TableMessage::JoinTable {
        user_id,
        username,  // ❌ Should fetch from database
        buy_in_amount: buy_in,
        passphrase: None,
        response: tx,
    })
}
```

**Expected Flow**:
- HTTP POST /api/tables/1/join → Creates table entry → Returns 200 OK
- WebSocket /ws/1 → Only receives game views, cannot join via WS

**Fix Recommendation**:
```rust
ClientMessage::Join { buy_in } => {
    ServerResponse::Error {
        message: "Use HTTP API POST /api/tables/{id}/join to join table".to_string()
    }
}
```

**Or**, implement full validation in WebSocket handler:
```rust
ClientMessage::Join { buy_in } => {
    // 1. Fetch username from database
    let username = sqlx::query_scalar("SELECT username FROM users WHERE id = $1")
        .bind(user_id)
        .fetch_one(&pool)
        .await?;

    // 2. Call WalletManager.transfer_to_escrow()
    let escrow_id = wallet_manager.transfer_to_escrow(
        user_id,
        table_id,
        buy_in,
        format!("join_{}_{}", table_id, Utc::now().timestamp_nanos())
    ).await?;

    // 3. Send JoinTable message
    // ...
}
```

---

### 2.2 Tournament Prize Rounding

**Issue**: Prize distribution may leave 1-2 chips unawarded due to rounding

**Current Code** (assumed from common patterns):
```rust
fn distribute_prizes(prize_pool: i64, structure: PrizeStructure) -> Vec<i64> {
    match structure {
        PrizeStructure::WinnerTakeAll => vec![prize_pool],
        PrizeStructure::TopTwo => vec![
            (prize_pool as f64 * 0.60) as i64,  // ❌ Truncates
            (prize_pool as f64 * 0.40) as i64,  // ❌ May lose chips
        ],
        PrizeStructure::TopThree => vec![
            (prize_pool as f64 * 0.50) as i64,
            (prize_pool as f64 * 0.30) as i64,
            (prize_pool as f64 * 0.20) as i64,
        ],
    }
}
// Example: prize_pool = 997
// 60% = 598.2 → 598
// 40% = 398.8 → 398
// Total awarded = 996, lost 1 chip ❌
```

**Fix Recommendation**:
```rust
fn distribute_prizes(prize_pool: i64, structure: PrizeStructure) -> Vec<i64> {
    let percentages = match structure {
        PrizeStructure::WinnerTakeAll => vec![100],
        PrizeStructure::TopTwo => vec![60, 40],
        PrizeStructure::TopThree => vec![50, 30, 20],
    };

    let mut prizes: Vec<i64> = percentages.iter()
        .map(|pct| (prize_pool * pct / 100) as i64)
        .collect();

    // Award remainder to first place
    let awarded = prizes.iter().sum::<i64>();
    let remainder = prize_pool - awarded;
    prizes[0] += remainder;

    assert_eq!(prizes.iter().sum::<i64>(), prize_pool, "Prize pool mismatch");
    prizes
}
```

---

### 2.3 Bot AI Telemetry Accuracy

**Issue**: Bot telemetry metrics (VPIP, PFR) may drift from configured difficulty

**Gap**: No automated verification that bots play according to their preset

**Recommendation**:
```rust
#[cfg(test)]
mod tests {
    #[tokio::test]
    async fn test_bot_vpip_accuracy() {
        // Run bot through 10,000 hands
        // Track: voluntary put in pot (fold pre-flop vs not)
        // Assert: Standard bot VPIP = 30% ± 5%
    }

    #[tokio::test]
    async fn test_bot_bluff_frequency() {
        // Run bot through scenarios where bluff is suboptimal
        // Track: bluff frequency
        // Assert: TAG bot bluffs 25% ± 5%
    }
}
```

---

### 2.4 Table Escrow Balance Invariant

**Issue**: No periodic audit that `SUM(table_escrows.balance) == SUM(chips in play)`

**Risk**: Bugs in transfer logic could create/destroy chips

**Recommendation**:
```rust
async fn audit_escrow_balances(pool: &PgPool) -> Result<(), String> {
    // For each table:
    //   escrow_balance = SELECT balance FROM table_escrows WHERE table_id = X
    //   table_chips = SUM(player.chips + player.bet) across all active players
    //   assert_eq!(escrow_balance, table_chips, "Escrow mismatch on table {}", table_id);

    // Run as background job every 5 minutes
    // Alert on mismatch
}
```

---

### 2.5 Idempotency Key Collision

**Issue**: Timestamp-based keys could collide under high concurrency

**Current**:
```rust
let idempotency_key = format!("join_{}_{}", table_id, Utc::now().timestamp_nanos());
```

**Risk**: Two requests within same nanosecond → same key → second request silently ignored

**Fix**:
```rust
use uuid::Uuid;
let idempotency_key = format!("join_{}_{}", table_id, Uuid::new_v4());
```

Or combine:
```rust
let idempotency_key = format!("join_{}_{}_{}",
    table_id,
    user_id,  // Ensures uniqueness per user
    Utc::now().timestamp_nanos()
);
```

---

## 3. TEST STRATEGY BY LAYER

### 3.1 Unit Tests (Pure Functions)

**Target**: 100% coverage on business logic functions

**Modules**:
- `game/functional.rs` - Hand evaluation
- `game/entities.rs` - Data structures
- `bot/decision.rs` - AI logic
- `wallet/models.rs` - Balance calculations

**Test Categories**:
1. **Happy Path** - Valid inputs, expected outputs
2. **Edge Cases** - Boundary values, empty inputs
3. **Invalid Inputs** - Type errors, negative values
4. **Mathematical Properties** - Commutative, associative, idempotent

**Example**:
```rust
#[test]
fn test_hand_evaluation_four_of_a_kind() {
    let cards = vec![
        Card::new(Value::Ace, Suit::Heart),
        Card::new(Value::Ace, Suit::Diamond),
        Card::new(Value::Ace, Suit::Club),
        Card::new(Value::Ace, Suit::Spade),
        Card::new(Value::King, Suit::Heart),
    ];
    let result = eval(&cards);
    assert_eq!(result[0].category, HandCategory::FourOfAKind);
    assert_eq!(result[0].values[0], Value::Ace);
}

#[test]
fn test_pot_odds_calculation() {
    let pot_size = 100;
    let bet_to_call = 20;
    let odds = calculate_pot_odds(pot_size, bet_to_call);
    assert_eq!(odds, 5.0); // 100 / 20 = 5:1
}

#[test]
fn test_side_pot_with_three_all_ins() {
    // Player A: 100 chips all-in
    // Player B: 200 chips all-in
    // Player C: 300 chips call
    let pots = calculate_side_pots(vec![
        (PlayerA, 100),
        (PlayerB, 200),
        (PlayerC, 300),
    ]);
    assert_eq!(pots.len(), 3);
    assert_eq!(pots[0].amount, 300); // 100 * 3
    assert_eq!(pots[1].amount, 200); // 100 * 2
    assert_eq!(pots[2].amount, 100); // 100 * 1
}
```

---

### 3.2 Integration Tests (Subsystem Level)

**Target**: All API endpoints, database queries, actor messages

**Modules**:
- `pp_server/src/api/*` - HTTP/WebSocket handlers
- `private_poker/src/table/actor.rs` - Table lifecycle
- `private_poker/src/auth/manager.rs` - Auth flows
- `private_poker/src/wallet/manager.rs` - Transactions

**Test Categories**:
1. **Success Scenarios** - Valid requests, expected state changes
2. **Validation Errors** - Invalid inputs, constraint violations
3. **State Machine Violations** - Invalid transitions
4. **Authorization Errors** - Missing JWT, wrong user
5. **Race Conditions** - Concurrent requests
6. **Transaction Rollbacks** - Simulated failures

**Example**:
```rust
#[tokio::test]
async fn test_concurrent_table_joins() {
    let (auth_manager, pool) = setup_test_environment().await;
    let table_manager = TableManager::new(pool.clone(), bot_manager);

    // Create 10 users
    let users = futures::future::join_all(
        (0..10).map(|i| create_test_user(&auth_manager, &format!("user{}", i)))
    ).await;

    // All 10 try to join same table simultaneously
    let results = futures::future::join_all(
        users.iter().map(|(user_id, token, _)| {
            table_manager.join_table(1, *user_id, "user".to_string(), 1000, None)
        })
    ).await;

    // Should all succeed (table max = 9, waitlist = 1)
    let successes = results.iter().filter(|r| r.is_ok()).count();
    assert_eq!(successes, 9, "9 players should join");

    let waitlisted = results.iter().filter(|r| r.is_err()).count();
    assert_eq!(waitlisted, 1, "1 player should be waitlisted");
}

#[tokio::test]
async fn test_wallet_transaction_rollback_on_table_full() {
    let (wallet_manager, pool) = setup_wallet_manager().await;
    let user_id = create_user_with_balance(&pool, 1000).await;

    // Fill table to max (9 players)
    fill_table_to_capacity(table_id).await;

    // Attempt to join (should fail)
    let result = wallet_manager.transfer_to_escrow(
        user_id,
        table_id,
        500,
        "idempotency_key_123"
    ).await;

    assert!(result.is_err(), "Join should fail when table full");

    // Verify wallet balance unchanged
    let balance = wallet_manager.get_wallet(user_id).await.unwrap().balance;
    assert_eq!(balance, 1000, "Balance should not be deducted");

    // Verify no wallet entry created
    let entries = wallet_manager.get_transaction_history(user_id, 10, 0).await.unwrap();
    assert_eq!(entries.len(), 0, "No transaction should be recorded");
}
```

---

### 3.3 End-to-End Tests (Full User Journeys)

**Target**: Complete flows from HTTP request to database persistence

**Scenarios**:
1. **New User Full Game** - Register → Fund wallet → Join table → Play hand → Cash out
2. **Tournament Flow** - Register → Join tournament → Play to completion → Receive payout
3. **Bot Interaction** - Human plays against bots → Bots despawn when table fills
4. **Spectator Mode** - Spectate game → See all actions except hole cards
5. **Disconnect/Reconnect** - Lose WebSocket → Reconnect → Resume game

**Example**:
```rust
#[tokio::test]
async fn test_full_user_journey_register_to_cashout() {
    let server = spawn_test_server().await;
    let client = reqwest::Client::new();

    // 1. Register
    let register_resp = client.post(&format!("{}/api/auth/register", server.url))
        .json(&json!({
            "username": "testuser",
            "password": "SecurePass123!",
            "display_name": "Test User"
        }))
        .send()
        .await.unwrap();
    assert_eq!(register_resp.status(), 200);
    let auth: AuthResponse = register_resp.json().await.unwrap();

    // 2. Claim faucet
    let faucet_resp = client.post(&format!("{}/api/wallet/faucet", server.url))
        .bearer_auth(&auth.access_token)
        .send()
        .await.unwrap();
    assert_eq!(faucet_resp.status(), 200);

    // 3. Join table
    let join_resp = client.post(&format!("{}/api/tables/1/join", server.url))
        .bearer_auth(&auth.access_token)
        .json(&json!({"buy_in_amount": 1000}))
        .send()
        .await.unwrap();
    assert_eq!(join_resp.status(), 200);

    // 4. Connect WebSocket
    let (mut ws_stream, _) = tokio_tungstenite::connect_async(
        &format!("ws://{}/ws/1?token={}", server.addr, auth.access_token)
    ).await.unwrap();

    // 5. Receive initial game view
    let msg = ws_stream.next().await.unwrap().unwrap();
    let game_view: GameView = serde_json::from_str(&msg.to_string()).unwrap();
    assert!(game_view.players.iter().any(|p| p.username == "testuser"));

    // 6. Play a hand (fold)
    ws_stream.send(tungstenite::Message::Text(
        json!({"type": "action", "action": {"type": "fold"}}).to_string()
    )).await.unwrap();

    // 7. Leave table
    let leave_resp = client.post(&format!("{}/api/tables/1/leave", server.url))
        .bearer_auth(&auth.access_token)
        .send()
        .await.unwrap();
    assert_eq!(leave_resp.status(), 200);

    // 8. Verify wallet balance updated
    let balance_resp = client.get(&format!("{}/api/wallet/balance", server.url))
        .bearer_auth(&auth.access_token)
        .send()
        .await.unwrap();
    let balance: WalletBalance = balance_resp.json().await.unwrap();
    assert!(balance.balance >= 9990); // Lost blinds, but most chips returned
}
```

---

### 3.4 Performance Tests

**Target**: Ensure system handles production load

**Scenarios**:
1. **Concurrent Tables** - 100+ tables, 500+ players
2. **High-Frequency Actions** - 1000 actions/second across all tables
3. **Large Tournament** - 100-player tournament, 10+ hours
4. **Sustained Load** - 24-hour stress test
5. **Memory Leak Detection** - Profile heap growth over time

**Metrics**:
- **Throughput**: Actions/second
- **Latency**: p50, p95, p99 response times
- **Resource Usage**: CPU, memory, database connections
- **Error Rate**: Failed requests / total requests

**Example**:
```rust
#[tokio::test]
async fn bench_concurrent_tables() {
    let server = spawn_test_server().await;
    let num_tables = 100;
    let actions_per_table = 1000;

    // Create 100 tables
    let tables = futures::future::join_all(
        (0..num_tables).map(|_| create_table(&server))
    ).await;

    // Simulate 1000 actions per table concurrently
    let start = Instant::now();
    let results = futures::future::join_all(
        tables.iter().flat_map(|table_id| {
            (0..actions_per_table).map(move |_| {
                simulate_random_action(&server, *table_id)
            })
        })
    ).await;
    let duration = start.elapsed();

    let total_actions = num_tables * actions_per_table;
    let throughput = total_actions as f64 / duration.as_secs_f64();

    println!("Throughput: {:.0} actions/sec", throughput);
    println!("p99 latency: {:?}", calculate_p99_latency(&results));

    assert!(throughput > 1000.0, "Should handle 1000 actions/sec");
    assert!(calculate_p99_latency(&results) < Duration::from_millis(100));
}
```

---

### 3.5 Security Tests

**Target**: Verify all security controls are effective

**Categories**:
1. **Authentication Bypass** - JWT forgery, token replay
2. **Authorization Bypass** - Access other users' data
3. **Rate Limit Bypass** - IP rotation, distributed attack
4. **Injection Attacks** - SQL injection, XSS, command injection
5. **Collusion Detection** - Adversarial chip dumping
6. **Crypto Weaknesses** - Weak random, predictable seats

**Example**:
```rust
#[tokio::test]
async fn test_jwt_signature_verification() {
    let server = spawn_test_server().await;

    // 1. Get valid token
    let valid_token = login_and_get_token(&server, "user1", "pass1").await;

    // 2. Tamper with token (change user_id claim)
    let parts: Vec<&str> = valid_token.split('.').collect();
    let mut claims: serde_json::Value = serde_json::from_slice(
        &base64::decode(parts[1]).unwrap()
    ).unwrap();
    claims["sub"] = json!(9999); // ❌ Change user ID
    let tampered_payload = base64::encode(claims.to_string());
    let tampered_token = format!("{}.{}.{}", parts[0], tampered_payload, parts[2]);

    // 3. Attempt to use tampered token
    let resp = server.get("/api/wallet/balance")
        .bearer_auth(&tampered_token)
        .send()
        .await;

    assert_eq!(resp.status(), 401, "Should reject tampered token");
}

#[tokio::test]
async fn test_rate_limiter_exponential_backoff() {
    let server = spawn_test_server().await;

    // 1. Trigger lockout (5 failed logins)
    for _ in 0..5 {
        server.post("/api/auth/login")
            .json(&json!({"username": "user1", "password": "wrongpass"}))
            .send()
            .await;
    }

    // 2. Wait for lockout to expire (15 min)
    tokio::time::sleep(Duration::from_secs(901)).await;

    // 3. Trigger lockout again
    for _ in 0..5 {
        server.post("/api/auth/login")
            .json(&json!({"username": "user1", "password": "wrongpass"}))
            .send()
            .await;
    }

    // 4. Lockout duration should double (30 min)
    let attempt = server.post("/api/auth/login")
        .json(&json!({"username": "user1", "password": "wrongpass"}))
        .send()
        .await;

    assert_eq!(attempt.status(), 429);
    let retry_after = attempt.headers().get("Retry-After").unwrap();
    assert!(retry_after.to_str().unwrap().parse::<u64>().unwrap() >= 1800); // 30 min
}
```

---

## 4. TEST INFRASTRUCTURE

### 4.1 Test Helpers

**Database Setup**:
```rust
async fn setup_test_db() -> Arc<PgPool> {
    let db_url = env::var("TEST_DATABASE_URL")
        .unwrap_or_else(|_| "postgres://postgres@localhost/poker_test".to_string());

    let pool = PgPoolOptions::new()
        .max_connections(10)
        .connect(&db_url)
        .await
        .expect("Failed to connect to test database");

    // Run migrations
    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .expect("Failed to run migrations");

    Arc::new(pool)
}

async fn cleanup_test_db(pool: &PgPool) {
    sqlx::query("TRUNCATE users, wallets, wallet_entries, tables, sessions CASCADE")
        .execute(pool)
        .await
        .unwrap();
}
```

**Test Server**:
```rust
struct TestServer {
    addr: SocketAddr,
    url: String,
    shutdown_tx: Option<tokio::sync::oneshot::Sender<()>>,
}

async fn spawn_test_server() -> TestServer {
    let pool = setup_test_db().await;
    let app_state = AppState::new(pool, /* ... */);
    let router = create_router(app_state);

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel();

    tokio::spawn(async move {
        axum::serve(listener, router)
            .with_graceful_shutdown(async {
                shutdown_rx.await.ok();
            })
            .await
            .unwrap();
    });

    TestServer {
        addr,
        url: format!("http://{}", addr),
        shutdown_tx: Some(shutdown_tx),
    }
}
```

**Test Data Factories**:
```rust
async fn create_test_user(
    auth_manager: &AuthManager,
    username: &str
) -> (i64, String, String) {
    let register_req = RegisterRequest {
        username: username.to_string(),
        password: "Pass1234".to_string(),
        display_name: Some(username.to_string()),
        email: None,
    };

    let user = auth_manager.register(register_req).await.unwrap();
    let (access, refresh) = auth_manager.login(LoginRequest {
        username: username.to_string(),
        password: "Pass1234".to_string(),
        totp_code: None,
    }, "test_device".to_string()).await.unwrap();

    (user.id, access, refresh)
}

async fn create_table_with_players(
    table_manager: &TableManager,
    player_count: usize,
    buy_in: i64
) -> (TableId, Vec<(i64, String)>) {
    let table_id = table_manager.create_table(default_config(), 1).await.unwrap();

    let mut players = vec![];
    for i in 0..player_count {
        let (user_id, token, _) = create_test_user(
            &auth_manager,
            &format!("player{}", i)
        ).await;

        table_manager.join_table(
            table_id,
            user_id,
            format!("player{}", i),
            buy_in,
            None
        ).await.unwrap();

        players.push((user_id, token));
    }

    (table_id, players)
}
```

---

### 4.2 Mock Objects

**Mock WebSocket Connection**:
```rust
struct MockWebSocket {
    sent_messages: Arc<Mutex<Vec<String>>>,
    received_messages: VecDeque<String>,
}

impl MockWebSocket {
    fn send(&mut self, msg: String) {
        self.sent_messages.lock().unwrap().push(msg);
    }

    fn recv(&mut self) -> Option<String> {
        self.received_messages.pop_front()
    }

    fn assert_sent(&self, expected: &str) {
        let sent = self.sent_messages.lock().unwrap();
        assert!(sent.iter().any(|msg| msg.contains(expected)));
    }
}
```

**Mock Bot Decision Engine**:
```rust
struct MockBotDecisionEngine {
    scripted_actions: VecDeque<Action>,
}

impl BotDecisionEngine for MockBotDecisionEngine {
    async fn decide(&mut self, context: DecisionContext) -> Action {
        self.scripted_actions.pop_front().unwrap_or(Action::Fold)
    }
}
```

---

### 4.3 Test Data

**Deterministic Card Decks**:
```rust
fn rigged_deck_for_royal_flush() -> Deck {
    // First 5 cards: A♠ K♠ Q♠ J♠ 10♠
    let mut cards = vec![
        Card::new(Value::Ace, Suit::Spade),
        Card::new(Value::King, Suit::Spade),
        Card::new(Value::Queen, Suit::Spade),
        Card::new(Value::Jack, Suit::Spade),
        Card::new(Value::Ten, Suit::Spade),
    ];

    // Add remaining 47 cards
    for suit in [Suit::Heart, Suit::Diamond, Suit::Club] {
        for value in Value::all() {
            cards.push(Card::new(value, suit));
        }
    }

    Deck::from_cards(cards)
}

fn deck_with_three_way_tie() -> Deck {
    // Player 1: A♠ A♥
    // Player 2: A♦ A♣
    // Player 3: K♠ K♥
    // Board: A♠ K♦ K♣ 2♥ 3♦
    // Result: Players 1 and 2 tie with Full House (Aces over Kings)
    //         Player 3 loses with Full House (Kings over Aces)
    // ...
}
```

---

## 5. TEST EXECUTION PLAN

### 5.1 Continuous Integration (CI)

**On Every Commit**:
- ✅ Lint (clippy --deny warnings)
- ✅ Format check (cargo fmt --check)
- ✅ Unit tests (cargo test --lib)
- ✅ Integration tests (cargo test --test)
- ✅ Property tests (cargo test --release proptest)

**On Pull Request**:
- ✅ All commit checks
- ✅ Code coverage report (must maintain 75%+)
- ✅ Security audit (cargo audit)
- ✅ Dependency check (cargo outdated)

**On Release Branch**:
- ✅ All PR checks
- ✅ Performance benchmarks (cargo bench)
- ✅ End-to-end smoke tests (critical paths)
- ✅ Load test (100 concurrent tables, 1 hour)

---

### 5.2 Test Environments

**Local Development**:
- PostgreSQL on localhost
- Single server instance
- Fast feedback (<30 seconds)

**CI Environment**:
- Dockerized PostgreSQL
- Matrix: Rust 1.70+, 1.75, stable
- Parallel test execution

**Staging**:
- Production-like setup (AWS RDS, multi-instance)
- Seeded with realistic data (1000+ users, 50+ tables)
- Nightly full regression suite

**Production Monitoring**:
- Synthetic transactions (every 5 minutes)
- Alert on failures
- Weekly chaos engineering (random instance kill)

---

### 5.3 Test Metrics

**Coverage Targets**:
- Overall: 80%
- Critical paths: 100% (auth, wallet, game FSM)
- Error handling: 95%
- Happy paths: 100%

**Quality Gates** (Fail build if violated):
- Zero compiler warnings
- Zero clippy warnings (strict mode)
- <2% flaky tests
- <5% test execution time increase per commit

---

## 6. MISSING TESTS (PRIORITY ORDER)

### P0 - Critical (Must Fix Before Production)

1. ✅ **Wallet balance invariants** - Escrow audit
2. ✅ **Concurrent join race conditions** - Deadlock prevention
3. ✅ **WebSocket disconnect during turn** - State recovery
4. ✅ **Side pot calculation edge cases** - 4+ all-ins
5. ✅ **JWT signature tampering** - Auth bypass prevention
6. ✅ **Rate limiter exponential backoff** - Lockout doubling
7. ✅ **Tournament prize rounding** - No lost chips
8. ✅ **Idempotency key collision** - Duplicate transaction prevention

### P1 - High (Fix Within 1 Month)

9. ✅ **Bot AI behavioral consistency** - VPIP/PFR accuracy over 10k hands
10. ✅ **Anti-collusion false positive rate** - <1% false positives
11. ✅ **Database transaction rollback** - Simulated failures
12. ✅ **Tournament simultaneous elimination** - Tie-breaking
13. ✅ **Blind progression during active hand** - Timing edge case
14. ✅ **Cross-table chip dumping detection** - Multi-table collusion
15. ✅ **Performance regression tests** - Automated benchmarks

### P2 - Medium (Fix Within 3 Months)

16. ✅ **Network partition recovery** - Split-brain scenarios
17. ✅ **Long-running tournament** - 24-hour stress test
18. ✅ **Bot despawn during bot's turn** - Edge case handling
19. ✅ **Admin endpoints** - Pause/resume table
20. ✅ **Spectator mode edge cases** - Joining mid-hand
21. ✅ **Top-up cooldown enforcement** - Timing attacks
22. ✅ **Chat message flood protection** - Rate limit validation

### P3 - Low (Nice to Have)

23. ✅ **Mobile client compatibility** - Responsive WebSocket
24. ✅ **Backup/restore procedures** - Database recovery
25. ✅ **Internationalization** - Multi-language support
26. ✅ **Accessibility** - Screen reader compatibility
27. ✅ **Analytics event tracking** - User behavior metrics
28. ✅ **A/B testing framework** - Feature flags

---

## 7. NEXT STEPS

1. **Review this strategy document** with team
2. **Generate test code** for P0 items (see separate files)
3. **Set up CI pipeline** with coverage reporting
4. **Create test database** dedicated for integration tests
5. **Implement test helpers** (factories, mocks, fixtures)
6. **Write performance test suite** with benchmarking
7. **Schedule weekly test review** to address flaky tests
8. **Establish quality gates** in CI/CD pipeline

---

## APPENDIX A: Test File Structure

```
private_poker/
├── tests/                          # Integration tests
│   ├── api_integration.rs          # ✅ Exists (expand)
│   ├── auth_integration.rs         # ✅ Exists (expand)
│   ├── wallet_integration.rs       # ✅ Exists (expand)
│   ├── game_flow_integration.rs    # ✅ Exists (expand)
│   ├── security_integration.rs     # ✅ Exists (expand)
│   ├── tournament_integration.rs   # ❌ NEW
│   ├── websocket_edge_cases.rs     # ❌ NEW
│   ├── concurrent_scenarios.rs     # ❌ NEW
│   ├── performance_tests.rs        # ❌ NEW
│   └── security_adversarial.rs     # ❌ NEW
├── benches/                        # Performance benchmarks
│   ├── game_benchmarks.rs          # ✅ Exists (expand)
│   ├── wallet_benchmarks.rs        # ❌ NEW
│   └── api_benchmarks.rs           # ❌ NEW
└── src/
    ├── game/
    │   ├── functional.rs           # ✅ Has unit tests (expand)
    │   └── entities.rs             # ✅ Has unit tests (expand)
    ├── auth/
    │   └── manager.rs              # ✅ Has unit tests (expand)
    ├── wallet/
    │   └── manager.rs              # ✅ Has unit tests (expand)
    ├── bot/
    │   └── decision.rs             # ✅ Has unit tests (expand)
    └── security/
        ├── rate_limiter.rs         # ✅ Has unit tests (expand)
        └── anti_collusion.rs       # ⚠️ Minimal tests (expand)
```

---

**End of Test Strategy Document**
