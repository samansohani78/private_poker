# Private Poker User Manual

Welcome to Private Poker - a secure, multi-table Texas Hold'em platform with authentication, persistent wallets, and anti-collusion features.

## Table of Contents

1. [Getting Started](#getting-started)
2. [Account Management](#account-management)
3. [Wallet & Chips](#wallet--chips)
4. [Finding & Joining Tables](#finding--joining-tables)
5. [Playing Poker](#playing-poker)
6. [Chat & Social](#chat--social)
7. [Tips & Best Practices](#tips--best-practices)
8. [Troubleshooting](#troubleshooting)

---

## Getting Started

### System Requirements

- **Server**: Linux-based system, 2GB+ RAM, PostgreSQL 14+
- **Client**: Any terminal with UTF-8 support
- **Network**: TCP connectivity to server

### Quick Start

1. **Connect to Server**:
   ```bash
   cargo run --bin pp_client --release -- <username> --connect <host>:<port>
   ```

2. **Register Account** (first time):
   ```
   /register <username> <password> [email]
   ```

3. **Login**:
   ```
   /login <username> <password>
   ```

4. **Check Balance**:
   ```
   /balance
   ```

5. **List Tables**:
   ```
   /tables
   ```

6. **Join a Table**:
   ```
   /join <table_id> <buy_in>
   ```

---

## Account Management

### Registration

Create a new account with a secure password:

```
/register player123 SecurePass123! player@example.com
```

**Password Requirements:**
- Minimum 8 characters
- At least one uppercase letter
- At least one lowercase letter
- At least one number
- Recommended: Include special characters

**Username Requirements:**
- 3-20 characters
- Alphanumeric characters and underscores only
- Must be unique

### Login

Log into your existing account:

```
/login player123 SecurePass123!
```

On successful login, you'll receive:
- Access token (for API requests)
- Refresh token (to renew access)
- Current wallet balance

### Session Management

**Logout**:
```
/logout
```

**Multiple Devices**:
You can log in from multiple devices simultaneously. Each device gets its own session.

### Security Features

**Two-Factor Authentication (2FA)**:
Enable 2FA for enhanced security:
```
/enable2fa
```

You'll receive a secret key and backup codes. Keep these secure!

**Verify 2FA**:
```
/verify2fa <6-digit-code>
```

**Password Reset**:
If you forgot your password:
```
/reset-password <email>
```

Follow the email instructions to reset.

---

## Wallet & Chips

### Understanding Your Wallet

Your wallet holds all your chips. Chips are:
- **Integer-based** (no fractions)
- **Persistent** across sessions
- **Tracked** with complete transaction history
- **Secure** with double-entry ledger

### Checking Balance

```
/balance
```

Example output:
```
Balance: 5,000 chips
```

### Daily Faucet

Claim free chips once per day (if balance is low):

```
/faucet
```

**Faucet Rules:**
- Available once per 24 hours
- Only if balance below threshold
- Amount varies based on server config
- Cooldown prevents abuse

### Transaction History

View your recent transactions:

```
/history [limit] [offset]
```

Example:
```
/history 50 0  # Last 50 transactions
```

Output shows:
- Timestamp
- Amount (+/-)
- Type (BuyIn, CashOut, Bonus, etc.)
- Description
- Balance after transaction

### Buy-Ins & Cash-Outs

**Buy-In** (when joining table):
- Chips deducted from wallet
- Moved to table escrow
- Minimum/maximum enforced by table

**Cash-Out** (when leaving table):
- Remaining chips returned to wallet
- Automatic on disconnect
- Immediate credit to balance

---

## Finding & Joining Tables

### Listing Available Tables

**All tables**:
```
/tables
```

**Filter by stakes**:
```
/tables --stakes micro   # BB ≤ 10
/tables --stakes low     # BB 10-100
/tables --stakes mid     # BB 100-1000
/tables --stakes high    # BB > 1000
```

**Filter by player count**:
```
/tables --min-players 4 --max-players 8
```

**Filter by speed**:
```
/tables --speed fast       # 5s action timer
/tables --speed standard   # 30s action timer
/tables --speed slow       # 60s action timer
```

**Show only joinable**:
```
/tables --joinable  # Has available seats
```

### Table Information

Each table listing shows:
- **ID**: Unique table identifier
- **Name**: Table name
- **Blinds**: Small blind / Big blind
- **Players**: Current / Maximum
- **Stakes**: Tier classification
- **Speed**: Action timer speed
- **Bots**: Whether bots are enabled
- **Private**: Requires passphrase/invite

Example:
```
ID: 42
Name: "High Stakes Challenge"
Blinds: 50/100
Players: 6/9
Stakes: Mid (BB=100)
Speed: Standard
Bots: Yes (5 target)
Private: No
```

### Creating Your Own Table

```
/create-table --name "My Table" \
              --blinds 10/20 \
              --max-players 9 \
              --min-buyin 20bb \
              --max-buyin 100bb \
              --speed standard \
              --bots
```

**Configuration Options:**
- `--name`: Table name (required)
- `--blinds`: Small/big blind (required)
- `--max-players`: 2-10 players (default: 9)
- `--min-buyin`: Minimum in BB (default: 20)
- `--max-buyin`: Maximum in BB (default: 100)
- `--speed`: fast/standard/slow (default: standard)
- `--bots`: Enable bot players
- `--target-bots`: Target bot count (default: 5)
- `--difficulty`: easy/standard/tag (default: standard)
- `--private`: Make private table
- `--passphrase`: Set passphrase for private table

### Joining a Table

```
/join <table_id> <buy_in>
```

Example:
```
/join 42 2000  # Join table 42 with 2000 chips
```

**Buy-In Rules:**
- Must be between table minimum and maximum
- Can't exceed your wallet balance
- Converted to BB: If blinds are 10/20, min=20BB=400 chips

**Joining Private Tables**:
```
/join 42 2000 --passphrase "secret123"
```

Or with invite token:
```
/join 42 2000 --invite "invite_token_here"
```

### Waitlisting

If a table is full, join the waitlist:

```
/waitlist <table_id>
```

You'll be notified when a seat opens.

**Leave waitlist**:
```
/leave-waitlist <table_id>
```

### Spectating

Watch a table without playing:

```
/spectate <table_id>
```

**Stop spectating**:
```
/stop-spectating <table_id>
```

---

## Playing Poker

### Game Flow

1. **Blinds Posted**: Small and big blinds auto-posted
2. **Cards Dealt**: Two hole cards dealt to each player
3. **Pre-Flop Betting**: First betting round
4. **Flop**: Three community cards revealed
5. **Flop Betting**: Second betting round
6. **Turn**: Fourth community card revealed
7. **Turn Betting**: Third betting round
8. **River**: Fifth community card revealed
9. **River Betting**: Final betting round
10. **Showdown**: Best hand wins

### Taking Actions

When it's your turn, you'll see available actions:

```
Your turn! Actions: fold, check, call 100, raise <amount>, all-in
```

**Fold**:
```
fold
```

**Check** (when no bet to call):
```
check
```

**Call**:
```
call
```

**Raise**:
```
raise 200    # Raise by 200
raise 2.5x   # Raise to 2.5x pot
raise        # Minimum raise
```

**All-In**:
```
all-in
```

### Hand Strength

From strongest to weakest:
1. **Royal Flush**: A♥ K♥ Q♥ J♥ 10♥
2. **Straight Flush**: 9♠ 8♠ 7♠ 6♠ 5♠
3. **Four of a Kind**: Q♦ Q♣ Q♥ Q♠ 7♥
4. **Full House**: J♥ J♦ J♣ 3♠ 3♥
5. **Flush**: K♦ 10♦ 8♦ 5♦ 2♦
6. **Straight**: 10♥ 9♣ 8♦ 7♠ 6♥
7. **Three of a Kind**: 8♣ 8♦ 8♥ A♠ 5♥
8. **Two Pair**: K♠ K♥ 7♦ 7♣ 2♠
9. **One Pair**: A♥ A♦ Q♠ 9♣ 4♥
10. **High Card**: A♠ K♦ 10♣ 7♥ 3♠

### Showing Your Hand

At showdown, hands are automatically revealed. To voluntarily show:

```
show
```

**Muck instead** (don't show losing hand):
Your hand is automatically mucked if you lose.

### Leaving a Table

```
/leave <table_id>
```

Your remaining chips are immediately returned to your wallet.

**Note**: Leaving during a hand forfeits your investment in that pot.

---

## Chat & Social

### Sending Messages

Chat with players at your table:

```
/chat <table_id> <message>
```

Example:
```
/chat 42 Good game everyone!
```

**Chat Rules:**
- 10 messages per minute limit
- No spam or offensive content
- Table-specific (other tables can't see)

### Viewing Chat History

Chat messages appear in real-time in your game view.

### Moderation

**Mute a player** (table owner/mod only):
```
/mute <table_id> <user_id>
```

**Kick a player** (table owner/mod only):
```
/kick <table_id> <user_id>
```

### Voting

Players can vote to kick disruptive players:

```
/vote-kick <username>
```

Majority vote required to execute kick.

**Vote to reset money**:
```
/vote-reset              # Reset all players
/vote-reset <username>   # Reset specific player
```

---

## Tips & Best Practices

### Bankroll Management

- **Never risk more than 5%** of bankroll in one session
- **Buy in for 40-60 BB** at casual tables
- **Claim faucet daily** when balance is low
- **Cash out regularly** to secure winnings

### Table Selection

- **Start at micro stakes** to learn
- **Choose standard speed** for thoughtful play
- **Avoid tables with all bots** for social experience
- **Check player count** - more players = more action

### Security

- **Use strong, unique password**
- **Enable 2FA** for account protection
- **Log out on shared computers**
- **Don't share account credentials**
- **Report suspicious activity**

### Playing Against Bots

**Bot Difficulties:**
- **Easy**: Loose-passive, plays 45% of hands, easy to beat
- **Standard**: Tight-aggressive, balanced play
- **TAG**: Very tight-aggressive, tough opponent

**Bot Tells:**
- Consistent timing
- No chat interaction
- Username often includes "Bot"

### Etiquette

- **Don't slow roll** - show your hand promptly
- **Be respectful** in chat
- **Don't angle shoot** - play honestly
- **Give action** - don't stall unnecessarily
- **Congratulate winners** - be a good sport

---

## Troubleshooting

### Can't Connect

**Check:**
- Server is running
- Correct host and port
- Firewall not blocking connection
- Network connectivity

**Try:**
```bash
telnet <host> <port>  # Test connection
```

### Login Failed

**Common causes:**
- Wrong username or password
- Account doesn't exist (register first)
- Rate limited (too many failed attempts)

**Solutions:**
- Double-check credentials
- Wait 15 minutes if rate limited
- Use password reset if forgotten

### Insufficient Balance

**If you can't join a table:**
- Check balance: `/balance`
- Claim faucet: `/faucet`
- Lower buy-in amount
- Choose table with lower blinds

### Can't Join Table

**Possible reasons:**
- Table is full (join waitlist)
- Buy-in below minimum
- Buy-in above maximum
- Buy-in exceeds wallet balance
- Same-IP player already at table

### Action Timeout

If you don't act within the time limit:
- **Standard**: 30 seconds
- **Fast**: 5 seconds
- **Slow**: 60 seconds

**Consequence**: Forced fold

**Prevention**:
- Stay attentive
- Pre-select action when possible
- Choose slower tables if needed

### Disconnected

If you disconnect:
- Your hand is auto-folded
- Session remains active briefly
- Reconnect with same credentials
- Chips remain at table until explicit leave

**Reconnect**:
```bash
pp_client player123 --connect host:port
/login player123 password
```

### Chips Missing

**Check:**
- Transaction history: `/history`
- Chips at table (didn't cash out yet)
- Ledger is authoritative

**If truly missing**:
Contact admin with transaction details.

---

## Advanced Features

### Multi-Tabling

Play at multiple tables simultaneously:

```
/join 42 2000
/join 55 1500
/join 78 3000
```

Switch between tables in UI or get turn signals for each.

### Statistics Tracking

Your play statistics are automatically tracked:
- Hands played
- VPIP (Voluntarily Put $ In Pot)
- PFR (Pre-Flop Raise)
- Win rate
- Net profit/loss

View with:
```
/stats
```

### Private Tables

Host a private game with friends:

1. Create private table:
   ```
   /create-table --name "Friends Game" \
                 --blinds 5/10 \
                 --private \
                 --passphrase "secretword"
   ```

2. Share passphrase with friends

3. Friends join:
   ```
   /join <table_id> <buy_in> --passphrase "secretword"
   ```

**Or use invite tokens**:
- More secure than passphrase
- Time-limited
- Can be revoked

---

## Support & Community

### Getting Help

- **In-game**: `/help`
- **Documentation**: Check `/docs` directory
- **GitHub Issues**: Report bugs or request features
- **Admin**: Contact server administrator

### Reporting Issues

When reporting issues, include:
- Your username
- Table ID (if applicable)
- What you were doing
- Error message (if any)
- Expected vs actual behavior

### Contributing

Private Poker is open source! Contributions welcome:
- Bug fixes
- Feature improvements
- Documentation updates
- Test coverage

See `CONTRIBUTING.md` for guidelines.

---

## Glossary

- **BB**: Big Blind - The larger forced bet
- **SB**: Small Blind - The smaller forced bet
- **VPIP**: Voluntarily Put $ In Pot - % of hands played
- **PFR**: Pre-Flop Raise - % of hands raised pre-flop
- **TAG**: Tight-Aggressive - Playing few hands aggressively
- **LAG**: Loose-Aggressive - Playing many hands aggressively
- **Buy-In**: Chips brought to table
- **Cash-Out**: Converting table chips back to wallet
- **Faucet**: Free daily chips for low-balance players
- **Escrow**: Temporary holding of chips during play

---

**Enjoy the game and play responsibly!**

Generated with Claude Code
