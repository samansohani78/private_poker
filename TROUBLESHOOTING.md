# Troubleshooting - Game Not Starting

## Problem: "Game not worked" - Two users join but game doesn't start

### Possible Issues and Solutions:

## Issue 1: Users Not Actually Joining

**Symptom:** Both users connected but see "Waiting for players..." or similar

**Cause:** Users are spectating instead of joining with chips

**Solution:**
After connecting to the table, BOTH users must:
1. Type `join 1000` (or any buy-in amount)
2. Press Enter
3. Wait for confirmation

**How to verify:**
```bash
# Check if players actually joined
curl -s http://localhost:8080/api/tables | jq '.[0].player_count'
# Should show: 2 (not 0)
```

## Issue 2: Game Requires Minimum Players

**Symptom:** 2 players joined but game stuck in "Lobby" state

**Cause:** Game might need more players or specific conditions

**Solution:**
The game should start with 2+ players. Check the game state:

```bash
# Check table details
curl -s http://localhost:8080/api/tables/1
```

## Issue 3: WebSocket Not Receiving Updates

**Symptom:** Players joined but no game view updates

**Cause:** WebSocket connection issue

**Solution:**
1. Check browser console (F12) for WebSocket errors
2. Verify token is valid
3. Check server logs for WebSocket errors:
```bash
tail -f /tmp/pp_server.log | grep -i "websocket\|error"
```

## Issue 4: Game State Machine Stuck

**Symptom:** Players see each other but no cards dealt

**Cause:** Game FSM in unexpected state

**Solution:**
Check server logs for game state transitions:
```bash
tail -f /tmp/pp_server.log | grep -i "table 1\|game\|state"
```

## Step-by-Step Debugging

### Step 1: Verify Server Running
```bash
ps aux | grep pp_server
curl http://localhost:8080/api/tables
```

Expected: Server running, returns JSON with tables

### Step 2: Start User 1
```bash
# Terminal 1
cargo run --bin pp_client --release -- \
  --server http://localhost:8080 \
  --username user1 \
  --password Pass1234 \
  --tui
```

**What to do:**
1. You'll see "Available tables" list
2. Type `1` to select Table 1
3. Press Enter
4. You'll see the poker table view
5. Type `join 1000` to join with 1000 chips
6. Press Enter
7. Wait - you should see "Waiting for more players..."

### Step 3: Start User 2
```bash
# Terminal 2
cargo run --bin pp_client --release -- \
  --server http://localhost:8080 \
  --username user2 \
  --password Pass5678 \
  --tui
```

**What to do:**
1. Type `1` to select Table 1
2. Press Enter
3. Type `join 1000`
4. Press Enter
5. **Game should now start!**

### Step 4: Verify Both Players Joined

Check the API:
```bash
curl -s http://localhost:8080/api/tables | jq '.[0].player_count'
```

Should return: `2`

If it returns `0`, players didn't actually join (just spectating).

## Common Mistakes

### ❌ Mistake 1: Just Pressing Enter
**Wrong:**
- Select table
- Press Enter
- Wait... (nothing happens)

**Right:**
- Select table
- Press Enter
- Type `join 1000`
- Press Enter

### ❌ Mistake 2: Using Different Tables
**Wrong:**
- User1 joins Table 1
- User2 joins Table 2

**Right:**
- Both join the SAME table (Table 1)

### ❌ Mistake 3: Not Waiting for Game to Start
**Wrong:**
- User1 joins
- User2 joins immediately
- Expect instant action

**Right:**
- User1 joins, sees "Waiting..."
- User2 joins
- Wait 2-3 seconds for game to initialize
- Blinds are posted
- Cards are dealt
- First player's turn begins

## Expected Game Flow

### Correct Flow:
```
1. User1: Select table → join 1000
   Status: "Waiting for more players..."

2. User2: Select table → join 1000
   Status: "Game starting..."

3. Both see:
   - Blinds posted (SB: $10, BB: $20)
   - Cards dealt (2 cards each)
   - Pot: $30
   - "Your turn" or "Waiting for player X"

4. Players take turns:
   - fold, check, call, raise <amount>, all-in

5. Community cards revealed:
   - Flop (3 cards)
   - Turn (1 card)
   - River (1 card)

6. Winner determined

7. Pot awarded

8. New hand starts automatically
```

## Detailed Commands

### Join Table:
```
join <buy_in_amount>
```
Examples:
- `join 1000` - Join with $1000
- `join 500` - Join with $500
- `join 2000` - Join with $2000

### Game Actions:
```
fold         - Give up your hand
check        - Pass (when no bet to call)
call         - Match the current bet
raise 50     - Raise bet by $50
all-in       - Bet all your chips
```

### Other Commands:
```
leave        - Leave the table
spectate     - Watch without playing
help         - Show help
```

## Check Game State in Database

```bash
PGPASSWORD=7794951 psql -U postgres -h localhost -d poker_db -c "
SELECT
  t.id,
  t.name,
  COUNT(te.user_id) as player_count
FROM tables t
LEFT JOIN table_escrows te ON t.id = te.table_id
WHERE t.id = 1
GROUP BY t.id, t.name;
"
```

This shows:
- Table ID
- Table name
- Actual player count from escrows

## Full Debug Session

### Terminal 1: Server Logs
```bash
tail -f /tmp/pp_server.log
```

### Terminal 2: User1
```bash
cargo run --bin pp_client --release -- \
  --server http://localhost:8080 \
  --username debuguser1 \
  --password Pass1111 \
  --tui

# Then type:
1        # Select table
join 1000
```

### Terminal 3: User2
```bash
cargo run --bin pp_client --release -- \
  --server http://localhost:8080 \
  --username debuguser2 \
  --password Pass2222 \
  --tui

# Then type:
1        # Select table
join 1000
```

### Terminal 4: Monitor
```bash
watch -n 1 'curl -s http://localhost:8080/api/tables | jq .'
```

You should see `player_count` change from 0 → 1 → 2

## Still Not Working?

### Reset Everything:
```bash
# 1. Kill server
pkill -f pp_server

# 2. Clean database
PGPASSWORD=7794951 psql -U postgres -h localhost -d poker_db -c \
  "TRUNCATE tables, table_escrows, users, wallets CASCADE;"

# 3. Restart server
cargo run --bin pp_server --release > /tmp/pp_server.log 2>&1 &

# 4. Wait for server to be ready
sleep 3

# 5. Try again with new users
```

## Web Client Specific Issues

### Issue: Web client connects but no game updates

**Check:**
1. Open Browser DevTools (F12)
2. Go to Console tab
3. Look for errors
4. Go to Network tab
5. Filter by "WS" (WebSocket)
6. Click the WebSocket connection
7. Check Messages tab

**Expected messages:**
```json
{
  "blinds": {"small": 10, "big": 20},
  "players": [...],
  "pot": {"size": 30},
  "board": [...]
}
```

### Issue: Action buttons not working

**Verify:**
1. You clicked "Join Table" button (not just viewing)
2. You entered buy-in amount
3. WebSocket is connected (green dot or "Connected" status)
4. It's your turn (player seat should be highlighted)

## Success Indicators

Game is working if you see:
- ✅ Both players' names displayed
- ✅ Chip counts shown for each player
- ✅ Pot amount displayed (should be $30 after blinds)
- ✅ Community cards area (empty initially)
- ✅ Your 2 hole cards (if it's after deal)
- ✅ "Your turn" indicator (for active player)
- ✅ Action buttons enabled (for active player)
- ✅ Game log showing actions

## Contact Info

If still not working after all these steps, please provide:
1. Server log output: `tail -50 /tmp/pp_server.log`
2. Client terminal output (screenshot or text)
3. API response: `curl -s http://localhost:8080/api/tables | jq .`
4. Database state: run the SQL query above
5. Exact steps you followed

---

**Last Updated:** November 2025
**Version:** 1.0
