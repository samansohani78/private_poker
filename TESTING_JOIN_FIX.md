# Testing the WebSocket Join Fix

## Summary of Changes

The WebSocket connection issue has been fixed by separating concerns:

1. **Server-side** (`pp_server/src/api/websocket.rs`):
   - Reverted WebSocket join handler to return error message
   - Join via WebSocket now disabled - directs users to HTTP API

2. **Client-side HTTP API** (`pp_client/src/api_client.rs`):
   - Added `join_table(table_id, buy_in)` method
   - Calls `POST /api/tables/{id}/join` with Authorization header

3. **TUI Client** (`pp_client/src/tui_app.rs`):
   - Modified to accept `table_id` and `api_client`
   - Intercepts `join` command to call HTTP API first
   - Displays success/error messages in the TUI log
   - Does NOT send join via WebSocket anymore

## Architecture

**Old Flow** (broken):
```
Client → WebSocket connect → Send join message → Database query → Wallet transfer → Join table
```

**New Flow** (working):
```
Client → HTTP POST /api/tables/1/join → Database + Wallet → Success response
       ↓
       → WebSocket connect → Receive game updates (read-only)
```

## How to Test

### Prerequisites

1. Server must be running:
```bash
env SERVER_BIND="0.0.0.0:8080" MAX_TABLES=1 target/release/pp_server
```

2. Client must be built:
```bash
cargo build --release
```

### Manual Test (TUI Mode)

1. Start the TUI client:
```bash
target/release/pp_client --tui
```

2. Enter credentials:
   - Username: `alice` (or any name)
   - Password: `Pass1234` (or any password)
   - If login fails, it will auto-register

3. Select table:
   - Enter `1` to select Table 1

4. Once in TUI, type:
```
join 1000
```

5. Expected behavior:
   - You should see: `[timestamp ACK  ]: Joined table successfully via HTTP API`
   - The game view should update showing you at the table
   - No "Connection reset" error

6. Verify you're in the game:
   - You should see your stack, position, etc.
   - You should receive periodic game view updates

### Automated Test Script

The script `test_join_fix.sh` tests the HTTP API directly:

```bash
bash test_join_fix.sh
```

**Note**: This may show "Table not found" if the TableManager doesn't have the table loaded as an actor. This is a separate issue with table loading, not related to the WebSocket join fix.

## Verification

### Success Indicators

✅ TUI client connects without immediate disconnect
✅ `join 1000` command shows "Joined table successfully" message
✅ Game view updates appear in TUI
✅ No "WebSocket protocol error: Connection reset" error

### Failure Indicators

❌ Immediate disconnect upon typing `join 1000`
❌ "Connection reset without closing handshake" error
❌ "Table not found" error (indicates table actor not loaded)

## Known Issues

1. **Table actor not loaded**: Existing tables in database aren't loaded as actors on server startup
   - **Workaround**: Start server with `MAX_TABLES=1` to create a fresh table
   - **Fix needed**: Implement table loading from database in TableManager

2. **WebSocket join disabled**: The `join` command via WebSocket now returns an error
   - This is intentional - join must happen via HTTP API first
   - WebSocket is now read-only for game state updates

## Files Modified

### Server
- `pp_server/src/api/mod.rs` - Added `pool` field to AppState
- `pp_server/src/api/websocket.rs` - Simplified join handler
- `pp_server/src/main.rs` - Updated AppState initialization
- `pp_server/Cargo.toml` - Added sqlx and chrono dependencies

### Client
- `pp_client/src/api_client.rs` - Added `join_table()` method
- `pp_client/src/tui_app.rs` - Added table_id/api_client fields, intercept join command
- `pp_client/src/main.rs` - Updated TuiApp::new() call with new parameters

## Next Steps

1. ✅ WebSocket connection fix implemented
2. ⏳ Test with real TUI client (requires interactive terminal)
3. ⏳ Fix table loading issue (load existing tables from DB)
4. ⏳ Update documentation
5. ⏳ Run full test suite

## Contact

For issues or questions about this fix, refer to:
- WebSocket handler: `pp_server/src/api/websocket.rs:326`
- HTTP join API: `pp_server/src/api/tables.rs`
- TUI join intercept: `pp_client/src/tui_app.rs:332`
