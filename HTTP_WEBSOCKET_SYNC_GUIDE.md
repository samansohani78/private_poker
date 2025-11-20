# HTTP/WebSocket State Synchronization Guide

**Purpose**: Document expected client behavior for maintaining consistent state between HTTP and WebSocket APIs.

---

## Architecture Overview

Private Poker uses a **dual-protocol architecture**:

1. **HTTP API** - For state-changing operations (join, leave, registration)
2. **WebSocket API** - For real-time updates and in-game actions

This separation provides:
- ✅ Better error handling on HTTP (proper status codes)
- ✅ Idempotency for critical operations (join/leave)
- ✅ Atomic wallet operations via HTTP
- ✅ Real-time low-latency updates via WebSocket

---

## Protocol Responsibilities

### HTTP API Handles

1. **User Registration & Authentication**
   - `POST /api/auth/register` - Create new account
   - `POST /api/auth/login` - Get JWT tokens
   - `POST /api/auth/refresh` - Refresh access token
   - `POST /api/auth/logout` - Invalidate session

2. **Table Discovery**
   - `GET /api/tables` - List all active tables
   - `GET /api/tables/{id}` - Get table details

3. **Join/Leave Operations**
   - `POST /api/tables/{id}/join` - Join a table (atomic wallet transfer)
   - `POST /api/tables/{id}/leave` - Leave a table (atomic wallet return)

**Why HTTP for Join/Leave?**
- Atomic wallet operations (transfer to escrow)
- Proper error responses (insufficient funds, table full, etc.)
- Idempotency key support
- Better retry semantics

### WebSocket API Handles

1. **Real-Time Game State**
   - Broadcasts game view every ~1 second
   - Current pot, community cards, player stacks
   - Whose turn it is

2. **Player Actions**
   - `Action { action: "fold" }` - Fold hand
   - `Action { action: "check" }` - Check
   - `Action { action: "call" }` - Call current bet
   - `Action { action: "raise", amount: 100 }` - Raise
   - `Action { action: "all_in" }` - All-in

3. **Spectating**
   - `Spectate` - Start spectating table
   - `StopSpectating` - Stop spectating
   - `Leave` - Leave table (if already joined via HTTP)

4. **Disabled: Join via WebSocket**
   - `Join { buy_in }` - **DISABLED**, returns error
   - Clients must use HTTP API instead

**Why WebSocket for Actions?**
- Low latency for turn-based actions
- Real-time updates to all players
- Persistent connection during gameplay

---

## Client State Machine

### Recommended Client Flow

```
┌─────────────────┐
│   Disconnected  │
└────────┬────────┘
         │ POST /api/auth/login
         ▼
┌─────────────────┐
│  Authenticated  │
└────────┬────────┘
         │ GET /api/tables (discover tables)
         ▼
┌─────────────────┐
│   Browsing      │
└────────┬────────┘
         │ User selects table
         │ POST /api/tables/{id}/join
         ▼
┌─────────────────┐
│   Joined        │ ◄──────┐
└────────┬────────┘        │
         │ ws://server/ws/table/{id}?token={jwt}
         ▼                  │
┌─────────────────┐        │
│   Connected     │        │
└────────┬────────┘        │
         │                  │
         │ Receive game state updates
         │ Send actions via WebSocket
         │                  │
         │ On disconnect: Auto-leave
         │ implemented on server side
         │                  │
         │ POST /api/tables/{id}/leave
         ▼                  │
┌─────────────────┐        │
│   Left Table    │────────┘
└─────────────────┘
```

---

## Synchronization Scenarios

### Scenario 1: Normal Join Flow

**Client Actions**:
```javascript
// 1. Join via HTTP
const joinResponse = await fetch('POST /api/tables/1/join', {
  headers: { 'Authorization': 'Bearer ${accessToken}' },
  body: JSON.stringify({ buy_in_amount: 1000 })
});

if (joinResponse.status === 200) {
  // 2. Connect WebSocket
  const ws = new WebSocket(`ws://server/ws/table/1?token=${accessToken}`);

  ws.onmessage = (event) => {
    const gameState = JSON.parse(event.data);
    // Update UI with game state
  };
}
```

**Server State**:
- HTTP handler: Transfers chips from wallet to escrow (atomic)
- HTTP handler: Sends `JoinTable` message to TableActor
- TableActor: Adds player to game state
- WebSocket: Broadcasts updated game state to all connections

**Guarantees**:
- ✅ Chips deducted atomically before join
- ✅ If join fails, chips returned via rollback
- ✅ WebSocket sees updated state immediately

### Scenario 2: WebSocket Disconnect During Gameplay

**What Happens**:
```javascript
// Client connection drops
ws.close();
```

**Server Behavior** (implemented in Session 2):
```rust
// On WebSocket cleanup (pp_server/src/api/websocket.rs:278-314)
// Automatically send LeaveTable message to TableActor
if let Some(table_handle) = state.table_manager.get_table(table_id).await {
    let leave_msg = TableMessage::LeaveTable { user_id, response: tx };
    table_handle.send(leave_msg).await;
}
```

**Result**:
- ✅ Player automatically removed from table
- ✅ Chips returned to wallet (atomic)
- ✅ Other players see updated game state
- ✅ No "stuck" tables waiting for disconnected player

**Client Responsibility**:
- Detect disconnect: `ws.onerror` or `ws.onclose`
- Attempt reconnect or navigate to table list
- **Do NOT** call HTTP leave endpoint (server already handled it)

### Scenario 3: Concurrent Join Attempts

**Problem**: User double-clicks "Join Table" button

**HTTP Request 1**:
```
POST /api/tables/1/join
Idempotency-Key: 2025-01-16T12:00:00.123Z-uuid-1
```

**HTTP Request 2** (concurrent):
```
POST /api/tables/1/join
Idempotency-Key: 2025-01-16T12:00:00.456Z-uuid-2  (different key)
```

**Server Behavior**:
- Request 1: Acquires wallet lock, debits chips, joins table
- Request 2: Waits for wallet lock, sees player already at table, returns error

**Result**:
- ✅ Only one join succeeds
- ✅ No duplicate chip deduction
- ✅ Second request gets clear error

**Client Best Practice**:
- Disable "Join" button after first click
- Show loading indicator
- Wait for HTTP response before enabling WebSocket

### Scenario 4: Taking Action While Not Your Turn

**Client Sends**:
```json
{
  "type": "action",
  "action": { "type": "raise", "amount": 100 }
}
```

**Server Check** (in TableActor):
```rust
// Check if it's user's turn
if !self.state.is_turn(&username) {
    return TableResponse::NotYourTurn;
}
```

**WebSocket Response**:
```json
{
  "type": "error",
  "message": "Not your turn"
}
```

**Client Responsibility**:
- Display error to user
- **Do NOT** update local state
- Wait for server's game state broadcast
- Disable action buttons when not player's turn

### Scenario 5: HTTP Leave While WebSocket Active

**Client Actions**:
```javascript
// User clicks "Leave Table" button
await fetch('POST /api/tables/1/leave', {
  headers: { 'Authorization': 'Bearer ${accessToken}' }
});

// Then close WebSocket
ws.close();
```

**Server Behavior**:
1. HTTP handler: Sends `LeaveTable` to TableActor
2. TableActor: Removes player, returns chips to wallet
3. HTTP response: 200 OK
4. WebSocket cleanup: Sends another `LeaveTable` (idempotent, no-op)

**Result**:
- ✅ Chips returned correctly (only once)
- ✅ Double leave is harmless (second is no-op)
- ✅ Clean state on server

**Client Best Practice**:
- Call HTTP leave first (guarantees wallet return)
- Then close WebSocket
- Handle both possible orderings gracefully

---

## Error Handling

### HTTP Errors

**Join Failures**:
```json
// 400 Bad Request
{ "error": "Insufficient chips: need 1000, have 500" }

// 403 Forbidden
{ "error": "Access denied: incorrect passphrase" }

// 409 Conflict
{ "error": "Table is full" }
```

**Client Response**:
- Display error to user
- Do NOT attempt WebSocket connection
- Update UI to show current wallet balance

### WebSocket Errors

**Action Failures**:
```json
{ "type": "error", "message": "Not your turn" }
{ "type": "error", "message": "Invalid action: cannot check with active bet" }
{ "type": "error", "message": "You must be seated at the table to take actions" }
```

**Client Response**:
- Display error in game UI
- Do NOT update local game state
- Wait for next server broadcast

**Connection Errors**:
- `ws.onerror`: Network failure, attempt reconnect
- `ws.onclose`: Clean disconnect, navigate away or show reconnect option

---

## State Consistency Rules

### Rule 1: HTTP is Source of Truth for Join/Leave

**Correct**:
```javascript
// Join via HTTP first
await fetch('POST /api/tables/1/join');
// Then connect WebSocket
const ws = new WebSocket('ws://...');
```

**Incorrect**:
```javascript
// ❌ Connect WebSocket first
const ws = new WebSocket('ws://...');
// ❌ Attempt join via WebSocket
ws.send(JSON.stringify({ type: 'join', buy_in: 1000 }));
// This will return an error!
```

### Rule 2: Server Game State Overrides Client

**Client Must**:
- Accept all game state broadcasts as authoritative
- Never assume action succeeded until server confirms
- Handle rollbacks gracefully (server may reject action)

**Example**:
```javascript
// Client sends raise
ws.send(JSON.stringify({ type: 'action', action: { type: 'raise', amount: 100 }}));

// Optimistically update UI
updateLocalGameState({ myBet: myBet + 100 });

// Wait for server confirmation
ws.onmessage = (event) => {
  const serverState = JSON.parse(event.data);

  if (serverState.type === 'error') {
    // Rollback optimistic update
    resetLocalGameState();
    showError(serverState.message);
  } else {
    // Server confirmed, use server's state
    replaceLocalGameState(serverState);
  }
};
```

### Rule 3: WebSocket Disconnect = Auto Leave

**Server Guarantees**:
- If WebSocket closes, player is removed from table
- Chips returned to wallet automatically
- No manual leave needed

**Client Must**:
- Detect disconnect: `ws.onclose`
- Update UI to reflect "Left Table" state
- Refresh wallet balance from server (GET /api/auth/me or similar)

### Rule 4: Idempotency for Critical Operations

**Join/Leave via HTTP are idempotent**:
- Same idempotency key = same result
- Multiple joins with same key = only one deduction
- Multiple leaves = only first one takes effect

**Client Should**:
- Generate idempotency key per operation
- Retry on network failure with same key
- Do NOT retry on 4xx errors (client error)

---

## Integration Testing Checklist

### Test Cases to Implement

1. ✅ **Join via HTTP, then WebSocket** - Normal flow
2. ✅ **WebSocket disconnect during turn** - Auto-leave works
3. ⚠️ **Join via HTTP, HTTP timeout, WebSocket connects** - Handle partial state
4. ⚠️ **Concurrent join attempts** - Only one succeeds
5. ⚠️ **Leave via HTTP while WebSocket active** - Chips returned once
6. ⚠️ **Leave via WebSocket close (auto-leave)** - Chips returned
7. ⚠️ **Action during non-turn** - Error response
8. ⚠️ **Action after leaving** - Error response
9. ⚠️ **Attempt join via WebSocket** - Helpful error message
10. ⚠️ **Server restart mid-game** - Client reconnects gracefully

---

## Client Library Recommendations

### Suggested Client State

```typescript
interface ClientState {
  authState: 'logged_out' | 'logged_in';
  accessToken: string | null;

  tableState: 'browsing' | 'joining' | 'joined' | 'in_game' | 'leaving';
  currentTableId: number | null;

  websocketState: 'disconnected' | 'connecting' | 'connected' | 'error';
  websocket: WebSocket | null;

  gameState: GameView | null;  // From server broadcasts

  walletBalance: number;
}
```

### State Transitions

```typescript
// Join flow
setState({ tableState: 'joining' });
const response = await httpJoin(tableId, buyIn);
if (response.ok) {
  setState({ tableState: 'joined' });
  connectWebSocket(tableId, accessToken);
} else {
  setState({ tableState: 'browsing' });
  showError(response.error);
}

// WebSocket connect
setState({ websocketState: 'connecting' });
ws.onopen = () => {
  setState({ websocketState: 'connected', tableState: 'in_game' });
};

// WebSocket message
ws.onmessage = (event) => {
  const msg = JSON.parse(event.data);
  setState({ gameState: msg });  // Server is source of truth
};

// WebSocket disconnect
ws.onclose = () => {
  setState({
    websocketState: 'disconnected',
    tableState: 'browsing',
    currentTableId: null,
    gameState: null
  });
  // Server auto-left, chips already returned
};
```

---

## Common Pitfalls

### ❌ Pitfall 1: Joining via WebSocket

```javascript
// WRONG - This doesn't work!
ws.send(JSON.stringify({ type: 'join', buy_in: 1000 }));
```

**Fix**: Always use HTTP for join
```javascript
// CORRECT
await fetch('POST /api/tables/1/join', { body: JSON.stringify({ buy_in_amount: 1000 }) });
```

### ❌ Pitfall 2: Not Handling WebSocket Disconnect

```javascript
// WRONG - Missing disconnect handler
const ws = new WebSocket('ws://...');
// If connection drops, client UI shows stale state
```

**Fix**: Always handle disconnect
```javascript
// CORRECT
ws.onclose = () => {
  console.log('Disconnected from table');
  setState({ tableState: 'browsing', gameState: null });
  // Server already auto-left, update UI accordingly
};
```

### ❌ Pitfall 3: Trusting Client State Over Server

```javascript
// WRONG - Optimistic update without rollback
ws.send(raiseAction);
localGameState.myBet += 100;  // What if server rejects?
```

**Fix**: Always reconcile with server state
```javascript
// CORRECT
ws.send(raiseAction);
ws.onmessage = (event) => {
  const serverState = JSON.parse(event.data);
  if (serverState.type === 'game_view') {
    replaceLocalState(serverState);  // Server wins
  }
};
```

### ❌ Pitfall 4: Double Leave

```javascript
// WRONG - Calling leave twice
await fetch('POST /api/tables/1/leave');
ws.close();  // Server auto-leaves on close
// Results in two leave operations (harmless but inefficient)
```

**Fix**: Leave via HTTP OR WebSocket, not both
```javascript
// CORRECT - Explicit leave
await fetch('POST /api/tables/1/leave');
ws.close();  // Second leave is no-op

// OR - Implicit leave
ws.close();  // Server auto-leaves
// Don't call HTTP leave
```

---

## Summary

### Key Principles

1. **HTTP for State Changes**: Join, leave, authentication
2. **WebSocket for Real-Time**: Game state, actions
3. **Server is Authoritative**: Always trust server state
4. **Auto-Leave on Disconnect**: Server handles cleanup
5. **Idempotency**: Retry safe for HTTP operations

### Architecture Benefits

- ✅ **Separation of Concerns**: HTTP for transactions, WS for real-time
- ✅ **Atomic Operations**: Wallet transfers via HTTP
- ✅ **Error Recovery**: Auto-leave prevents stuck tables
- ✅ **Scalability**: Stateless HTTP, stateful WS per table
- ✅ **Client Simplicity**: Clear protocol boundaries

### Implementation Status

- ✅ HTTP join/leave: Fully implemented
- ✅ WebSocket actions: Fully implemented
- ✅ Auto-leave on disconnect: Implemented (Session 2)
- ✅ Error responses: Comprehensive
- ⚠️ Integration tests: Need more coverage (see checklist above)

---

**Related Files**:
- `pp_server/src/api/websocket.rs` - WebSocket handler
- `pp_server/src/api/tables.rs` - HTTP table endpoints
- `private_poker/src/table/actor.rs` - TableActor message handling
- `SESSION_2_COMPLETE.md` - WebSocket disconnect fix

**Issue**: Audit Report Issue #21 - HTTP/WebSocket State Desync
**Status**: ✅ Mitigated with this synchronization guide
