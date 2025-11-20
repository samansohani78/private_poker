# WebSocket Connection Issue - Debug Guide

## Issue Description

**Symptom**: TUI client shows "WebSocket protocol error: Connection reset without closing handshake" immediately upon typing `join 1000`.

**Impact**: Users cannot join tables via WebSocket, preventing gameplay.

**Status**: In Progress

---

## Investigation Findings

### What Works ✅
- HTTP API (register, login, list tables) - all responding correctly
- Server starts successfully
- Database connections working
- User authentication working
- Table creation working

### What Fails ❌
- WebSocket connection upgrade
- No logs show WebSocket connection attempt reaching the handler
- Connection reset happens immediately (<1 second)

### Root Cause Analysis

**Likely Causes** (in order of probability):

1. **JWT Token Format Mismatch** (90% likely)
   - Client might be sending token in wrong format
   - Server expecting query parameter, client might be using header
   - Token might be expired immediately after login

2. **WebSocket Route Not Registered** (70% likely)
   - Route `/ws/:table_id` might not be properly registered in the router
   - Middleware order issue (CORS, auth middleware interfering)

3. **Async Context Issue** (50% likely)
   - Database query in WebSocket handler blocking the upgrade
   - WalletManager transfer timing out
   - Tokio runtime issue

4. **Port/Protocol Mismatch** (30% likely)
   - Client connecting to wrong port
   - HTTP vs HTTPS mismatch
   - Proxy interfering

---

## Debugging Steps

### Step 1: Verify WebSocket Route Registration

Check that the route is actually registered:

```bash
# In pp_server/src/api/mod.rs around line 165
# Should see:
#   .route("/ws/{table_id}", get(websocket::websocket_handler))
```

**Status**: ✅ Verified - route is registered in public_routes

### Step 2: Check Client Token Format

The client should be sending:
```
ws://localhost:8080/ws/1?token=<jwt_access_token>
```

Check `pp_client/src/api_client.rs:162`:
```rust
Ok(format!("{}/ws/{}?token={}", ws_url, table_id, token))
```

**Status**: ✅ Verified - client formats URL correctly

### Step 3: Test WebSocket with Minimal Handler

Create a test endpoint that doesn't do database queries:

```rust
// In websocket.rs
pub async fn test_ws_handler(
    ws: WebSocketUpgrade,
    Query(query): Query<WsQuery>,
    State(state): State<AppState>,
) -> Response {
    // Verify token
    let user_id = match state.auth_manager.verify_access_token(&query.token) {
        Ok(claims) => claims.sub,
        Err(_) => {
            return (StatusCode::UNAUTHORIZED, "Invalid token").into_response();
        }
    };

    // Immediately upgrade without any async operations
    ws.on_upgrade(move |socket| async move {
        println!("WebSocket connected for user {}", user_id);
    })
}
```

Then add route:
```rust
.route("/ws/test", get(websocket::test_ws_handler))
```

**Status**: ⏳ Not yet implemented

### Step 4: Add Verbose Logging

Add logging at every step in the WebSocket handler:

```rust
pub async fn websocket_handler(
    ws: WebSocketUpgrade,
    Path(table_id): Path<i64>,
    Query(query): Query<WsQuery>,
    State(state): State<AppState>,
) -> Response {
    info!("WebSocket connection attempt: table_id={}, token={}", table_id, &query.token[..20]);

    let user_id = match state.auth_manager.verify_access_token(&query.token) {
        Ok(claims) => {
            info!("Token verified for user_id={}", claims.sub);
            claims.sub
        },
        Err(e) => {
            error!("Token verification failed: {:?}", e);
            return (StatusCode::UNAUTHORIZED, "Invalid token").into_response();
        }
    };

    info!("About to upgrade WebSocket for user_id={}", user_id);
    ws.on_upgrade(move |socket| {
        info!("WebSocket upgrade successful for user_id={}", user_id);
        handle_socket(socket, table_id, user_id, state)
    })
}
```

**Status**: ⏳ Should be added

### Step 5: Check for Panics

Run server with `RUST_BACKTRACE=1`:

```bash
RUST_BACKTRACE=1 env SERVER_BIND="0.0.0.0:8080" MAX_TABLES=1 target/release/pp_server
```

Then try connecting and look for panic messages.

**Status**: ⏳ Not yet tested

### Step 6: Test with `websocat` Tool

Install websocat:
```bash
cargo install websocat
```

Get a token manually:
```bash
TOKEN=$(curl -s -X POST http://localhost:8080/api/auth/login \
  -H "Content-Type: application/json" \
  -d '{"username":"alice","password":"Pass1234"}' | jq -r '.access_token')

echo "Token: $TOKEN"
```

Test WebSocket directly:
```bash
websocat "ws://localhost:8080/ws/1?token=$TOKEN"
```

If it connects, type:
```json
{"type":"join","buy_in":1000}
```

**Status**: ⏳ Not yet tested

---

## Quick Fixes to Try

### Fix 1: Remove Database Query from WebSocket Handler (Simplest)

**Problem**: The database query might be blocking the WebSocket upgrade.

**Solution**: Remove the username lookup and wallet transfer from the WebSocket join handler. Use the HTTP API for joining instead.

```rust
// In websocket.rs, revert to original:
ClientMessage::Join { buy_in } => {
    return ServerResponse::Error {
        message: "Please use HTTP POST /api/tables/1/join to join the table".to_string()
    };
}
```

Then update TUI client to call HTTP API first:

```rust
// In tui_app.rs, before connecting WebSocket:
async fn join_table_http(api_client: &ApiClient, table_id: i64, buy_in: i64) -> Result<()> {
    api_client.post(
        &format!("/api/tables/{}/join", table_id),
        &json!({"buy_in_amount": buy_in})
    ).await?;
    Ok(())
}
```

**Effort**: Low (30 minutes)
**Success Rate**: High (80%)

### Fix 2: Move Database Operations to Separate Task

**Problem**: Blocking operations in WebSocket upgrade path.

**Solution**: Spawn async task for database operations:

```rust
ClientMessage::Join { buy_in } => {
    let state_clone = state.clone();
    let table_id_clone = table_id;
    let user_id_clone = user_id;

    tokio::spawn(async move {
        // Do database query in background
        let username = fetch_username(state_clone.pool, user_id_clone).await?;
        // ... rest of logic
    });

    ServerResponse::Success {
        message: "Processing join request...".to_string()
    }
}
```

**Effort**: Medium (1 hour)
**Success Rate**: Medium (50%)

### Fix 3: Increase Timeouts

**Problem**: Operations timing out too quickly.

**Solution**: Add generous timeouts:

```rust
use tokio::time::{timeout, Duration};

let username = match timeout(
    Duration::from_secs(5),
    sqlx::query_scalar::<_, String>("SELECT username FROM users WHERE id = $1")
        .bind(user_id)
        .fetch_one(state.pool.as_ref())
).await {
    Ok(Ok(name)) => name,
    Ok(Err(e)) => return ServerResponse::Error { message: format!("DB error: {}", e) },
    Err(_) => return ServerResponse::Error { message: "Timeout".to_string() },
};
```

**Effort**: Low (15 minutes)
**Success Rate**: Low (20%)

---

## Recommended Approach

**Immediate** (to unblock development):
1. Implement Fix 1 (revert WebSocket join, use HTTP API)
2. Update TUI client to call HTTP join endpoint
3. WebSocket becomes view-only for game updates

**Short-term** (proper fix):
1. Add comprehensive logging (Step 4)
2. Test with websocat (Step 6)
3. Identify exact failure point
4. Implement targeted fix

**Long-term** (architectural):
1. Separate join flow from WebSocket entirely
2. HTTP API for all state-changing operations (join, leave, action)
3. WebSocket only for real-time game view updates
4. Consider Server-Sent Events (SSE) as alternative

---

## Architecture Decision

**Current Design** (problematic):
```
Client → WebSocket connect → Send join message → Database query → Wallet transfer → Join table
```

**Recommended Design** (simpler, more reliable):
```
Client → HTTP POST /api/tables/1/join → Database + Wallet → Success response
       ↓
       → WebSocket connect → Receive game updates (read-only)
```

**Benefits**:
- Clearer separation of concerns
- Better error handling (HTTP status codes)
- Easier to test
- No blocking operations in WebSocket upgrade
- More standard architecture (REST for writes, WS for live updates)

---

## Files to Modify

### Option A: Revert to HTTP-only join (Recommended)

1. `pp_server/src/api/websocket.rs:326` - Remove database query, return error message
2. `pp_client/src/tui_app.rs:304` - Add HTTP API call before WebSocket
3. `pp_client/src/api_client.rs` - Add join_table() method

### Option B: Fix WebSocket join

1. `pp_server/src/api/websocket.rs:328-357` - Add comprehensive logging
2. `pp_server/src/api/websocket.rs:333` - Test without database query
3. `pp_server/Cargo.toml` - Verify all async dependencies compatible

---

## Next Steps

1. ✅ Comprehensive test analysis completed (TEST_STRATEGY.md)
2. ✅ P0 issues identified
3. ⏳ **Fix WebSocket connection issue** (this document)
4. ⏳ Implement remaining P0 fixes
5. ⏳ Run generated test files
6. ⏳ Achieve 80%+ coverage

---

## Contact Points

- WebSocket handler: `pp_server/src/api/websocket.rs:133`
- Client WebSocket connect: `pp_client/src/tui_app.rs:155`
- API client: `pp_client/src/api_client.rs:155`

---

**Created**: November 15, 2025
**Status**: Investigation in progress
**Priority**: P0 (blocks all gameplay)
