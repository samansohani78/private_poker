# Private Poker - Testing Guide

Complete guide for testing the Private Poker system with server, TUI client, and web client.

## Quick Start - Automated Test

Run the comprehensive test script:

```bash
./test_full_system.sh
```

This will:
- ✅ Clean up any running processes
- ✅ Clean the database
- ✅ Build the project
- ✅ Start the server
- ✅ Test HTTP API (register users, list tables)
- ✅ Verify TUI client binary exists
- ✅ Verify all web client files exist
- ✅ Provide instructions for manual testing

## Manual Testing

### 1. Start the Server

```bash
# Clean database first (optional but recommended)
PGPASSWORD=7794951 psql -U postgres -h localhost -d poker_db -c \
  "TRUNCATE tables, table_escrows, users, wallets, wallet_entries CASCADE;"

# Start server
cargo run --bin pp_server --release
```

Server will start on `http://localhost:8080` (configured in `.env`).

**What happens:**
- Loads configuration from `.env` file
- Connects to PostgreSQL database
- Creates 1 initial table (MAX_TABLES=1 in .env)
- Starts HTTP/WebSocket server on port 8080

### 2. Test with TUI Client

Open a new terminal:

```bash
cargo run --bin pp_client --release -- \
  --server http://localhost:8080 \
  --username alice \
  --password Pass1234 \
  --tui
```

**What happens:**
- Connects to server
- Auto-registers user if doesn't exist
- Shows table selection
- You select table and join with chips
- Rich terminal UI displays:
  - Your cards (colored)
  - Community cards
  - Other players
  - Pot and blinds
  - Action buttons

**Available Commands:**
- `join <amount>` - Join table with buy-in
- `fold` - Fold hand
- `check` - Check
- `call` - Call bet
- `raise <amount>` - Raise
- `all-in` - All in
- `leave` - Leave table
- `help` - Show help

### 3. Test with Web Client

Open another terminal:

```bash
cd web_client
python3 -m http.server 8000
```

Then open browser: `http://localhost:8000`

**Login Page:**
- Server URL: `http://localhost:8080`
- Username: `bob` (or any new name)
- Password: `Pass5678` (must meet requirements)
- Click "Register" to create new user

**Lobby:**
- See available tables
- Shows player count and blinds
- Click "Join Table" to enter game

**Game Table:**
- Visual poker table with green felt
- Graphical playing cards
- Players arranged in circle
- Your cards at bottom
- Action buttons: Fold, Check, Call, Raise, All-In
- Game log shows all actions

### 4. Multiplayer Test

Run multiple clients simultaneously:

**Terminal 1 - Alice (TUI):**
```bash
cargo run --bin pp_client --release -- \
  --server http://localhost:8080 \
  --username alice \
  --password Pass1234 \
  --tui
```

**Terminal 2 - Bob (TUI):**
```bash
cargo run --bin pp_client --release -- \
  --server http://localhost:8080 \
  --username bob \
  --password Pass5678 \
  --tui
```

**Browser - Charlie (Web):**
Open `http://localhost:8000`, login as `charlie` / `Pass9999`

All three players can join the same table and play together!

## Testing Scenarios

### Scenario 1: Basic Gameplay

1. Start server
2. Connect 2 clients (Alice and Bob)
3. Both join Table 1 with $1000 each
4. Play a hand:
   - Alice: gets cards, waits for turn
   - Bob: gets cards, waits for turn
   - Blinds are posted automatically
   - Players take turns (Check/Call/Raise/Fold)
   - Winner takes pot

### Scenario 2: Full Table

1. Start server
2. Connect 9 players (max for table)
3. All join the same table
4. Game starts with all players
5. See everyone's positions and actions

### Scenario 3: Bot Players

Configure in `.env`:
```bash
BOTS_ENABLED=true
TARGET_BOT_COUNT=6
```

1. Start server
2. Join as 1 human player
3. Bots automatically join to fill table
4. Play against smart AI opponents

### Scenario 4: Multiple Tables

Change in `.env`:
```bash
MAX_TABLES=3
```

1. Start server (creates 3 tables)
2. Players can choose which table to join
3. Each table runs independently

## Verification Checklist

### Server ✅
- [ ] Starts without errors
- [ ] Loads configuration from .env
- [ ] Connects to database
- [ ] Creates initial table(s)
- [ ] Responds to HTTP requests
- [ ] Accepts WebSocket connections
- [ ] Logs to console

### TUI Client ✅
- [ ] Connects to server
- [ ] Auto-registers new users
- [ ] Lists available tables
- [ ] Joins table with buy-in
- [ ] Displays colored cards
- [ ] Shows other players
- [ ] Action buttons work
- [ ] Real-time updates
- [ ] Leave table works

### Web Client ✅
- [ ] Login page loads
- [ ] Register new user works
- [ ] Lobby shows tables
- [ ] Join table works
- [ ] Game view displays correctly
- [ ] Cards render properly
- [ ] Players show in circle
- [ ] Action buttons work
- [ ] Real-time updates via WebSocket
- [ ] Game log shows actions

### Multiplayer ✅
- [ ] Multiple clients can connect
- [ ] Players see each other
- [ ] Turn-based gameplay works
- [ ] Pot calculations correct
- [ ] Winner determination works
- [ ] Chip transfers correct

## Troubleshooting

### Server won't start

**Check database:**
```bash
PGPASSWORD=7794951 psql -U postgres -h localhost -d poker_db -c "SELECT 1;"
```

**Check port availability:**
```bash
lsof -i :8080
```

**View logs:**
```bash
tail -f /tmp/pp_server.log
```

### Client can't connect

**Verify server is running:**
```bash
curl http://localhost:8080/api/tables
```

**Check WebSocket:**
```bash
wscat -c ws://localhost:8080/ws/1?token=test
```

### Web client issues

**Check files exist:**
```bash
ls -la web_client/
ls -la web_client/js/
ls -la web_client/css/
```

**Browser console errors:**
- Open Developer Tools (F12)
- Check Console tab for JavaScript errors
- Check Network tab for failed requests

### Database issues

**Reset database:**
```bash
PGPASSWORD=7794951 psql -U postgres -h localhost -d poker_db -c \
  "TRUNCATE tables, table_escrows, users, wallets, wallet_entries, sessions CASCADE;"
```

**Check migrations:**
```bash
sqlx migrate info --database-url "postgresql://postgres:7794951@localhost:5432/poker_db"
```

## Performance Testing

### Load Test

Test with many concurrent users:

```bash
# Terminal 1 - Start server
cargo run --bin pp_server --release

# Terminal 2 - Run load test
for i in {1..10}; do
    cargo run --bin pp_client --release -- \
      --server http://localhost:8080 \
      --username "user$i" \
      --password "Pass123$i" &
done
```

### Stress Test

Use the integration test:

```bash
cargo test --test multi_client_game -- --ignored --nocapture
```

This spawns:
- 1 server
- 5 concurrent clients
- All join same table
- Verifies game state updates

## Common Issues & Solutions

| Issue | Cause | Solution |
|-------|-------|----------|
| Server creates 100 tables | MAX_TABLES env var | Set `MAX_TABLES=1` in .env |
| Password rejected | Doesn't meet requirements | Use uppercase, lowercase, number, 8+ chars |
| Table not found | Database has old data | Clean database with TRUNCATE |
| WebSocket fails | Wrong protocol | Use `ws://` not `http://` |
| Port already in use | Server still running | `pkill -f pp_server` |
| Cards not showing | JavaScript error | Check browser console |
| TUI not colored | Terminal doesn't support | Use modern terminal (iTerm2, Windows Terminal) |

## Test Data

### Valid Test Users

| Username | Password | Notes |
|----------|----------|-------|
| alice | Pass1234 | Test user 1 |
| bob | Pass5678 | Test user 2 |
| charlie | Pass9999 | Test user 3 |
| testuser1 | Pass1234 | Created by test script |
| testuser2 | Pass1234 | Created by test script |

### Test Credentials

```bash
# Valid passwords
Pass1234  # 1 upper, 1 lower, 1 number
Alice123  # Valid
Test9876  # Valid

# Invalid passwords
password  # No uppercase, no number
Pass      # Too short
PASS1234  # No lowercase
```

## Continuous Testing

### Watch Mode

```bash
# Terminal 1 - Server
cargo watch -x 'run --bin pp_server --release'

# Terminal 2 - Tests
cargo watch -x 'test --workspace'
```

### Pre-commit Checks

```bash
# Format code
cargo fmt --all

# Check clippy
cargo clippy --workspace -- -D warnings

# Run tests
cargo test --workspace

# Run integration tests
cargo test --test multi_client_game -- --ignored
```

## Success Criteria

System is working correctly if:
- ✅ Server starts and creates 1 table
- ✅ Can register new users via HTTP API
- ✅ Can list tables via HTTP API
- ✅ TUI client can login and see tables
- ✅ Web client can login and see tables
- ✅ Multiple clients can join same table
- ✅ Players can see each other
- ✅ Game actions work (fold, call, raise)
- ✅ Real-time updates via WebSocket
- ✅ Winner determined correctly
- ✅ Chips transferred correctly

## Next Steps

After successful testing:

1. **Deploy to production**:
   - Set strong JWT_SECRET and PASSWORD_PEPPER
   - Use HTTPS/WSS in production
   - Configure firewall
   - Set up monitoring

2. **Add features**:
   - Tournament mode
   - Chat system
   - Player statistics
   - Hand history replay

3. **Scale**:
   - Add more servers
   - Load balancer
   - Redis for session storage
   - Database replication

---

**Testing Guide Version**: 1.0
**Last Updated**: November 2025
**Status**: Production Ready ✅
