# Database Schema Documentation

## Overview

Private Poker uses PostgreSQL 14+ with the following design principles:
- **Integer-based chips** (BIGINT, never floats)
- **Double-entry ledger** for wallet transactions
- **Idempotency keys** for duplicate prevention
- **Audit trails** for all financial operations
- **Indexes** for query optimization

## Entity Relationship Diagram

```
users (1) ----< (N) sessions
users (1) ----< (1) wallets
users (1) ----< (N) wallet_entries
users (1) ----< (N) faucet_claims
users (1) ----< (N) collusion_flags
```

---

## Tables

### users

Stores user account information.

```sql
CREATE TABLE users (
    id BIGSERIAL PRIMARY KEY,
    username VARCHAR(50) UNIQUE NOT NULL,
    password_hash VARCHAR(255) NOT NULL,
    email VARCHAR(255) UNIQUE,
    email_verified BOOLEAN DEFAULT FALSE,
    totp_secret VARCHAR(255),                -- 2FA secret
    two_factor_enabled BOOLEAN DEFAULT FALSE,
    created_at TIMESTAMP DEFAULT NOW(),
    updated_at TIMESTAMP DEFAULT NOW()
);

CREATE INDEX idx_users_username ON users(username);
CREATE INDEX idx_users_email ON users(email) WHERE email IS NOT NULL;
```

**Columns:**
- `id`: Unique user identifier (auto-increment)
- `username`: Unique username (3-20 chars, alphanumeric + underscore)
- `password_hash`: Argon2id hashed password
- `email`: Optional email for password recovery
- `email_verified`: Whether email has been verified
- `totp_secret`: Base32-encoded TOTP secret for 2FA
- `two_factor_enabled`: Whether 2FA is enabled
- `created_at`: Account creation timestamp
- `updated_at`: Last update timestamp

**Constraints:**
- Username must be unique
- Email must be unique if provided

---

### sessions

Tracks user sessions for authentication.

```sql
CREATE TABLE sessions (
    id BIGSERIAL PRIMARY KEY,
    user_id BIGINT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    refresh_token VARCHAR(255) UNIQUE NOT NULL,
    device_fingerprint VARCHAR(255),
    ip_address INET,
    user_agent TEXT,
    expires_at TIMESTAMP NOT NULL,
    created_at TIMESTAMP DEFAULT NOW(),
    last_used_at TIMESTAMP DEFAULT NOW()
);

CREATE INDEX idx_sessions_user_id ON sessions(user_id);
CREATE INDEX idx_sessions_refresh_token ON sessions(refresh_token);
CREATE INDEX idx_sessions_expires_at ON sessions(expires_at);
```

**Columns:**
- `id`: Unique session identifier
- `user_id`: Foreign key to users table
- `refresh_token`: JWT refresh token (hashed)
- `device_fingerprint`: Unique device identifier
- `ip_address`: IP address of session
- `user_agent`: Browser/client user agent
- `expires_at`: When refresh token expires
- `created_at`: Session creation time
- `last_used_at`: Last token refresh time

**Cleanup:**
- Expired sessions deleted via cron job
- User logout deletes session immediately

---

### wallets

Stores user wallet balances.

```sql
CREATE TABLE wallets (
    user_id BIGINT PRIMARY KEY REFERENCES users(id) ON DELETE CASCADE,
    balance BIGINT NOT NULL DEFAULT 0 CHECK (balance >= 0),
    currency VARCHAR(10) DEFAULT 'CHIP',
    created_at TIMESTAMP DEFAULT NOW(),
    updated_at TIMESTAMP DEFAULT NOW()
);

CREATE INDEX idx_wallets_balance ON wallets(balance);
```

**Columns:**
- `user_id`: Foreign key to users (one wallet per user)
- `balance`: Current chip balance (must be non-negative)
- `currency`: Currency code (default: CHIP)
- `created_at`: Wallet creation timestamp
- `updated_at`: Last balance update timestamp

**Constraints:**
- Balance must be >= 0
- One wallet per user (enforced by primary key)

---

### wallet_entries

Complete ledger of all wallet transactions.

```sql
CREATE TABLE wallet_entries (
    id BIGSERIAL PRIMARY KEY,
    user_id BIGINT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    table_id BIGINT,
    amount BIGINT NOT NULL,
    balance_after BIGINT NOT NULL,
    direction VARCHAR(10) NOT NULL CHECK (direction IN ('debit', 'credit')),
    entry_type VARCHAR(20) NOT NULL CHECK (entry_type IN (
        'buyin', 'cashout', 'rake', 'bonus',
        'admin_adjust', 'transfer', 'faucet'
    )),
    idempotency_key VARCHAR(255) UNIQUE,
    description TEXT,
    created_at TIMESTAMP DEFAULT NOW()
);

CREATE INDEX idx_wallet_entries_user_id ON wallet_entries(user_id);
CREATE INDEX idx_wallet_entries_table_id ON wallet_entries(table_id) WHERE table_id IS NOT NULL;
CREATE INDEX idx_wallet_entries_created_at ON wallet_entries(created_at DESC);
CREATE INDEX idx_wallet_entries_entry_type ON wallet_entries(entry_type);
CREATE INDEX idx_wallet_entries_idempotency_key ON wallet_entries(idempotency_key) WHERE idempotency_key IS NOT NULL;
```

**Columns:**
- `id`: Unique entry identifier
- `user_id`: Foreign key to users
- `table_id`: Optional table reference
- `amount`: Amount (positive for credit, negative for debit)
- `balance_after`: Balance after this transaction
- `direction`: 'debit' (money out) or 'credit' (money in)
- `entry_type`: Transaction type
- `idempotency_key`: Prevents duplicate transactions
- `description`: Human-readable description
- `created_at`: Transaction timestamp

**Entry Types:**
- `buyin`: Chips moved from wallet to table
- `cashout`: Chips returned from table to wallet
- `rake`: House fee deduction
- `bonus`: Promotional bonus
- `admin_adjust`: Admin balance adjustment
- `transfer`: User-to-user transfer (future)
- `faucet`: Daily faucet claim

**Idempotency:**
- Same idempotency_key = duplicate ignored
- Ensures exactly-once transaction processing

---

### faucet_claims

Tracks faucet claims for cooldown enforcement.

```sql
CREATE TABLE faucet_claims (
    id BIGSERIAL PRIMARY KEY,
    user_id BIGINT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    amount BIGINT NOT NULL,
    claimed_at TIMESTAMP DEFAULT NOW(),
    next_claim_at TIMESTAMP NOT NULL
);

CREATE INDEX idx_faucet_claims_user_id ON faucet_claims(user_id);
CREATE INDEX idx_faucet_claims_next_claim_at ON faucet_claims(next_claim_at);
```

**Columns:**
- `id`: Unique claim identifier
- `user_id`: Foreign key to users
- `amount`: Chips claimed
- `claimed_at`: When faucet was claimed
- `next_claim_at`: When user can claim again

**Logic:**
- User can claim if `next_claim_at < NOW()`
- Default cooldown: 24 hours
- Only available if balance below threshold

---

### rate_limit_attempts

Tracks rate limit attempts for security endpoints.

```sql
CREATE TABLE rate_limit_attempts (
    endpoint VARCHAR(50) NOT NULL,
    identifier VARCHAR(255) NOT NULL,
    attempts INT NOT NULL DEFAULT 1,
    window_start TIMESTAMP NOT NULL,
    locked_until TIMESTAMP,
    consecutive_violations INT DEFAULT 0,
    PRIMARY KEY (endpoint, identifier)
);

CREATE INDEX idx_rate_limit_attempts_locked_until ON rate_limit_attempts(locked_until)
    WHERE locked_until IS NOT NULL;
```

**Columns:**
- `endpoint`: Endpoint name (login, register, chat, etc.)
- `identifier`: IP address or user identifier
- `attempts`: Attempts in current window
- `window_start`: When current window started
- `locked_until`: Lockout end time (NULL if not locked)
- `consecutive_violations`: For exponential backoff

**Endpoints:**
- `login`: 5 attempts/5min, 15min lockout
- `register`: 3 attempts/hour, 1hour lockout
- `password_reset`: 3 attempts/hour, 2hour lockout
- `chat`: 10 messages/minute, 5min lockout

**Cleanup:**
- Expired lockouts deleted periodically
- Window resets after time period

---

### collusion_flags

Anti-collusion detection flags requiring admin review.

```sql
CREATE TABLE collusion_flags (
    id BIGSERIAL PRIMARY KEY,
    user_id BIGINT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    table_id BIGINT NOT NULL,
    flag_type VARCHAR(50) NOT NULL CHECK (flag_type IN (
        'same_ip_table', 'win_rate_anomaly',
        'coordinated_folding', 'suspicious_transfers',
        'seat_manipulation'
    )),
    severity VARCHAR(20) NOT NULL CHECK (severity IN ('low', 'medium', 'high')),
    details JSONB,
    created_at TIMESTAMP DEFAULT NOW(),
    reviewed BOOLEAN DEFAULT FALSE,
    reviewer_user_id BIGINT REFERENCES users(id),
    reviewed_at TIMESTAMP
);

CREATE INDEX idx_collusion_flags_user_id ON collusion_flags(user_id);
CREATE INDEX idx_collusion_flags_table_id ON collusion_flags(table_id);
CREATE INDEX idx_collusion_flags_reviewed ON collusion_flags(reviewed) WHERE NOT reviewed;
CREATE INDEX idx_collusion_flags_severity ON collusion_flags(severity);
CREATE INDEX idx_collusion_flags_created_at ON collusion_flags(created_at DESC);
```

**Columns:**
- `id`: Unique flag identifier
- `user_id`: Flagged user
- `table_id`: Table where detected
- `flag_type`: Type of suspicious activity
- `severity`: low, medium, or high
- `details`: JSONB with context (IP addresses, win rates, etc.)
- `created_at`: When flag was created
- `reviewed`: Whether admin has reviewed
- `reviewer_user_id`: Admin who reviewed
- `reviewed_at`: When reviewed

**Flag Types:**
- `same_ip_table`: Multiple players from same IP (Medium)
- `win_rate_anomaly`: >80% win rate vs same-IP player (High)
- `coordinated_folding`: Always folding to same player (Low)
- `suspicious_transfers`: Unusual chip patterns (Medium)
- `seat_manipulation`: Rapid seat changes (Low)

**Shadow Flagging:**
- NO automatic bans
- ALL flags require admin review
- Details stored in JSONB for flexibility

---

## Migrations

Migrations are managed with `sqlx-cli`:

```bash
# Install
cargo install sqlx-cli --no-default-features --features postgres

# Create migration
sqlx migrate add <migration_name>

# Run migrations
sqlx migrate run

# Revert last migration
sqlx migrate revert
```

**Migration Files Location:**
`migrations/`

---

## Indexes

### Performance Indexes

```sql
-- Fast user lookups
CREATE INDEX idx_users_username ON users(username);
CREATE INDEX idx_users_email ON users(email) WHERE email IS NOT NULL;

-- Session management
CREATE INDEX idx_sessions_user_id ON sessions(user_id);
CREATE INDEX idx_sessions_refresh_token ON sessions(refresh_token);
CREATE INDEX idx_sessions_expires_at ON sessions(expires_at);

-- Wallet queries
CREATE INDEX idx_wallet_entries_user_id ON wallet_entries(user_id);
CREATE INDEX idx_wallet_entries_created_at ON wallet_entries(created_at DESC);
CREATE INDEX idx_wallet_entries_table_id ON wallet_entries(table_id) WHERE table_id IS NOT NULL;

-- Security
CREATE INDEX idx_rate_limit_attempts_locked_until ON rate_limit_attempts(locked_until)
    WHERE locked_until IS NOT NULL;
CREATE INDEX idx_collusion_flags_reviewed ON collusion_flags(reviewed) WHERE NOT reviewed;
```

### Partial Indexes

Used for filtering specific conditions efficiently:

```sql
-- Only index non-NULL emails
CREATE INDEX idx_users_email ON users(email) WHERE email IS NOT NULL;

-- Only index locked rate limits
CREATE INDEX idx_rate_limit_attempts_locked_until ON rate_limit_attempts(locked_until)
    WHERE locked_until IS NOT NULL;

-- Only index unreviewed flags
CREATE INDEX idx_collusion_flags_reviewed ON collusion_flags(reviewed) WHERE NOT reviewed;

-- Only index wallet entries with tables
CREATE INDEX idx_wallet_entries_table_id ON wallet_entries(table_id) WHERE table_id IS NOT NULL;
```

---

## Maintenance

### Vacuum

Regular vacuum to reclaim space:

```sql
-- Analyze all tables
VACUUM ANALYZE;

-- Specific table
VACUUM ANALYZE wallet_entries;

-- Full vacuum (locks table)
VACUUM FULL wallet_entries;
```

### Statistics

Update query planner statistics:

```sql
ANALYZE users;
ANALYZE wallet_entries;
```

### Reindex

Rebuild indexes if corrupted:

```sql
REINDEX TABLE wallet_entries;
REINDEX DATABASE poker_db;
```

---

## Backup & Restore

### Backup

```bash
# Full database dump
pg_dump -U poker poker_db | gzip > backup.sql.gz

# Schema only
pg_dump -U poker -s poker_db > schema.sql

# Data only
pg_dump -U poker -a poker_db > data.sql

# Specific table
pg_dump -U poker -t wallet_entries poker_db > wallet_backup.sql
```

### Restore

```bash
# Full restore
gunzip < backup.sql.gz | psql -U poker poker_db

# Schema only
psql -U poker poker_db < schema.sql

# Data only
psql -U poker poker_db < data.sql
```

---

## Monitoring Queries

### Active Connections

```sql
SELECT count(*) AS connections
FROM pg_stat_activity
WHERE datname = 'poker_db';
```

### Table Sizes

```sql
SELECT
    schemaname,
    tablename,
    pg_size_pretty(pg_total_relation_size(schemaname||'.'||tablename)) AS size
FROM pg_tables
WHERE schemaname = 'public'
ORDER BY pg_total_relation_size(schemaname||'.'||tablename) DESC;
```

### Slow Queries

```sql
SELECT
    query,
    calls,
    total_time,
    mean_time,
    max_time
FROM pg_stat_statements
ORDER BY mean_time DESC
LIMIT 10;
```

### Index Usage

```sql
SELECT
    schemaname,
    tablename,
    indexname,
    idx_scan,
    idx_tup_read,
    idx_tup_fetch
FROM pg_stat_user_indexes
ORDER BY idx_scan DESC;
```

### Unused Indexes

```sql
SELECT
    schemaname,
    tablename,
    indexname,
    idx_scan
FROM pg_stat_user_indexes
WHERE idx_scan = 0
    AND indexname NOT LIKE '%_pkey';
```

---

## Best Practices

### Data Integrity
- Use foreign keys with `ON DELETE CASCADE`
- Enforce check constraints (balance >= 0)
- Use unique constraints (idempotency_key)
- Never use floats for currency

### Performance
- Index foreign keys
- Use partial indexes for filters
- Regular VACUUM ANALYZE
- Monitor slow queries

### Security
- Hash passwords with Argon2id
- Store only hashed refresh tokens
- Use idempotency keys
- Audit trail for all transactions

### Backup
- Daily automated backups
- Test restore procedures
- Keep 30 days of backups
- Store offsite copies

---

Generated with Claude Code
