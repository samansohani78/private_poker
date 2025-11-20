# Manual TUI Test Instructions

## Setup

1. Server is running on port 8080
2. Table 1 is loaded and active
3. Built client: `target/release/pp_client`

## Test Steps

1. Run: `target/release/pp_client --tui`

2. Enter credentials:
   - Username: `carol` (or any new name)
   - Password: `Pass1234`

3. Select table: Enter `1`

4. Once in TUI, type: `join 1000`

## Expected Results

✅ You should see:
```
[timestamp ACK  ]: Joined table successfully via HTTP API
```

✅ The game view should update showing:
- Your username at the table
- Your stack: 1000 chips
- 5-6 bots at the table
- Game state (blinds, positions, etc.)

✅ No errors like:
- "Connection reset without closing handshake"
- "Table not found"

## What Just Got Fixed

### Problem
- WebSocket connection was resetting immediately when trying to join
- Complex async operations (DB queries, wallet transfers) in WebSocket upgrade path were timing out

### Solution
1. **Server** - WebSocket join handler simplified to return error, directs to HTTP API
2. **Client API** - Added `join_table()` method that calls HTTP POST endpoint
3. **TUI Client** - Intercepts `join` command, calls HTTP API first, then stays connected via WebSocket for updates

### Architecture
```
Old (broken):
WebSocket → join command → timeout → reset

New (working):
HTTP POST → join succeeds → WebSocket → receive updates
```

## Server Logs to Verify

Check `/tmp/server_final.log` for:
```
[timestamp INFO ] User <id> (<username>) joined table 1 with 1000 chips
```

This confirms the HTTP join succeeded and the WebSocket is receiving updates.
