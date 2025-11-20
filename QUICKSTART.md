# Quick Start Guide - Private Poker

## Play Poker in 2 Minutes! 🎰

The server is **already running** on port 8080 with users alice and bob ready to play.

### Option 1: TUI Client (Recommended - Fully Working)

Open two terminal windows:

**Terminal 1 - Alice:**
```bash
cargo run --bin pp_client --release -- \
  --server http://localhost:8080 \
  --username alice \
  --password Pass1234 \
  --tui
```

**Terminal 2 - Bob:**
```bash
cargo run --bin pp_client --release -- \
  --server http://localhost:8080 \
  --username bob \
  --password Pass5678 \
  --tui
```

**In each terminal:**
1. Type: `join 1000` (join with 1000 chips)
2. Wait for the other player to join
3. Game starts automatically!
4. Play poker with: `check`, `call`, `raise 100`, `fold`, `allin`

### Option 2: CLI Client (Simple Text Mode)

Same commands as TUI, but remove the `--tui` flag:

```bash
cargo run --bin pp_client --release -- \
  --server http://localhost:8080 \
  --username alice \
  --password Pass1234
```

### Option 3: Web Client (Needs Minor Fix)

```bash
cd web_client
python3 -m http.server 8000
```

Open http://localhost:8000 in your browser.

**Note**: Web client is created but needs HTTP API integration for joining tables. Use TUI for now.

### Create Your Own User

```bash
# Register
curl -X POST http://localhost:8080/api/auth/register \
  -H "Content-Type: application/json" \
  -d '{"username": "yourname", "password": "YourPass123", "display_name": "Your Name"}'

# Then use in client
cargo run --bin pp_client --release -- \
  --server http://localhost:8080 \
  --username yourname \
  --password YourPass123 \
  --tui
```

### Game Commands

- `join <amount>` - Join the table with buy-in amount (e.g., `join 1000`)
- `check` - Check (no bet required)
- `call` - Call the current bet
- `raise <amount>` - Raise by amount (e.g., `raise 50`)
- `fold` - Fold your hand
- `allin` - Go all-in with all your chips
- `leave` - Leave the table
- `help` - Show help menu
- `quit` - Exit client

### What Happens When You Play

1. **Join**: You and another player join Table 1
2. **Bots**: System spawns 5 bots to fill the table
3. **Game Starts**: Automatic when 2+ players
4. **Blinds Posted**: Small blind (10) and big blind (20)
5. **Cards Dealt**: Each player gets 2 hole cards
6. **Betting Rounds**: Pre-flop → Flop → Turn → River
7. **Showdown**: Best hand wins the pot!
8. **Bot Management**: Bots despawn as real players join

### Troubleshooting

**Server not running?**
```bash
nohup env SERVER_BIND="0.0.0.0:8080" MAX_TABLES=1 target/release/pp_server \
  > /tmp/pp_server_clean.log 2>&1 &
```

**Can't join table?**
Make sure you registered/logged in first. The error will tell you what's wrong.

**Game not starting?**
Need at least 2 players. Bots will fill the rest.

**WebSocket error?**
Make sure server is on port 8080: `curl http://localhost:8080/health`

### Server Status

```bash
# Check if server is running
ps aux | grep pp_server | grep -v grep

# View logs
tail -f /tmp/pp_server_clean.log

# Check health
curl http://localhost:8080/health
```

### Ready to Play!

The system is fully configured and ready. Just run the TUI client commands above and start playing!

**Have fun!** 🎰♠️♥️♣️♦️
