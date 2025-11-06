# Private Poker Administrator Guide

This guide covers server deployment, configuration, monitoring, security, and maintenance for Private Poker administrators.

## Table of Contents

1. [System Requirements](#system-requirements)
2. [Installation](#installation)
3. [Configuration](#configuration)
4. [Database Setup](#database-setup)
5. [Security](#security)
6. [Monitoring](#monitoring)
7. [Maintenance](#maintenance)
8. [Troubleshooting](#troubleshooting)
9. [Scaling](#scaling)

---

## System Requirements

### Minimum Requirements

- **OS**: Linux (Ubuntu 22.04 LTS recommended)
- **CPU**: 2 cores
- **RAM**: 4GB
- **Storage**: 20GB SSD
- **Database**: PostgreSQL 14+
- **Rust**: 1.70+ (for compilation)

### Recommended Production

- **OS**: Ubuntu 22.04 LTS
- **CPU**: 4+ cores
- **RAM**: 8GB+
- **Storage**: 50GB+ SSD with daily backups
- **Database**: PostgreSQL 15 with replication
- **Network**: 1Gbps, low latency

### Estimated Resource Usage

**Per 100 Active Users:**
- CPU: ~10-15% (2 cores)
- RAM: ~500MB
- Network: ~1-2 Mbps
- Database: ~100 concurrent connections

**Per 20 Active Tables:**
- CPU: ~5-10% (2 cores)
- RAM: ~200MB

---

## Installation

### From Source

```bash
# Clone repository
git clone https://github.com/your-org/private_poker.git
cd private_poker

# Build release binaries
cargo build --release

# Binaries located in:
# target/release/pp_server
# target/release/pp_client
# target/release/pp_bots
```

### Using Docker

```bash
# Build image
docker build -t private-poker:latest .

# Run container
docker run -d \
  --name poker-server \
  -p 8080:8080 \
  -e DATABASE_URL="postgres://user:pass@host/db" \
  -e JWT_SECRET="your-secret-key" \
  private-poker:latest
```

### System Service (systemd)

Create `/etc/systemd/system/poker-server.service`:

```ini
[Unit]
Description=Private Poker Server
After=network.target postgresql.service

[Service]
Type=simple
User=poker
Group=poker
WorkingDirectory=/opt/private_poker
Environment="DATABASE_URL=postgres://poker:password@localhost/poker_db"
Environment="JWT_SECRET=your-secret-key-here"
Environment="RUST_LOG=info"
ExecStart=/opt/private_poker/pp_server --bind 0.0.0.0:8080
Restart=always
RestartSec=10

[Install]
WantedBy=multi-user.target
```

Enable and start:
```bash
sudo systemctl enable poker-server
sudo systemctl start poker-server
sudo systemctl status poker-server
```

---

## Configuration

### Environment Variables

```bash
# Required
export DATABASE_URL="postgres://user:pass@host:5432/dbname"
export JWT_SECRET="your-256-bit-secret-key"

# Optional
export RUST_LOG="info"                    # Logging level (error, warn, info, debug, trace)
export SERVER_BIND="0.0.0.0:8080"        # Bind address
export MAX_CONNECTIONS=100                # Max concurrent connections
export ACTION_TIMEOUT_SECS=30             # Player action timeout
export FAUCET_AMOUNT=1000                 # Daily faucet chips
export FAUCET_COOLDOWN_HOURS=24           # Hours between faucet claims
export DEFAULT_WALLET_BALANCE=1000        # New user starting balance
```

### Database Configuration

PostgreSQL settings in `postgresql.conf`:

```conf
# Connection Settings
max_connections = 200
shared_buffers = 256MB
effective_cache_size = 1GB

# WAL Settings
wal_level = replica
max_wal_senders = 3
wal_keep_size = 512MB

# Query Performance
random_page_cost = 1.1      # For SSD
work_mem = 4MB
maintenance_work_mem = 64MB
```

### Server Tuning

Edit `Cargo.toml` release profile:

```toml
[profile.release]
opt-level = 3              # Maximum optimization
lto = true                 # Link-time optimization
codegen-units = 1          # Better optimization
strip = true               # Strip debug symbols
panic = "abort"            # Smaller binary
```

---

## Database Setup

### Initial Setup

```bash
# Create database and user
sudo -u postgres psql

CREATE USER poker WITH PASSWORD 'secure_password';
CREATE DATABASE poker_db OWNER poker;
GRANT ALL PRIVILEGES ON DATABASE poker_db TO poker;
\q
```

### Schema Migration

```bash
# Install sqlx-cli
cargo install sqlx-cli --no-default-features --features postgres

# Run migrations
export DATABASE_URL="postgres://poker:password@localhost/poker_db"
sqlx database create
sqlx migrate run
```

### Database Schema

```sql
-- Users table
CREATE TABLE users (
    id BIGSERIAL PRIMARY KEY,
    username VARCHAR(50) UNIQUE NOT NULL,
    password_hash VARCHAR(255) NOT NULL,
    email VARCHAR(255) UNIQUE,
    created_at TIMESTAMP DEFAULT NOW(),
    updated_at TIMESTAMP DEFAULT NOW()
);

-- Sessions table
CREATE TABLE sessions (
    id BIGSERIAL PRIMARY KEY,
    user_id BIGINT REFERENCES users(id) ON DELETE CASCADE,
    refresh_token VARCHAR(255) UNIQUE NOT NULL,
    device_fingerprint VARCHAR(255),
    expires_at TIMESTAMP NOT NULL,
    created_at TIMESTAMP DEFAULT NOW()
);

-- Wallets table
CREATE TABLE wallets (
    user_id BIGINT PRIMARY KEY REFERENCES users(id) ON DELETE CASCADE,
    balance BIGINT NOT NULL DEFAULT 0 CHECK (balance >= 0),
    currency VARCHAR(10) DEFAULT 'CHIP',
    created_at TIMESTAMP DEFAULT NOW(),
    updated_at TIMESTAMP DEFAULT NOW()
);

-- Wallet entries (ledger)
CREATE TABLE wallet_entries (
    id BIGSERIAL PRIMARY KEY,
    user_id BIGINT REFERENCES users(id) ON DELETE CASCADE,
    table_id BIGINT,
    amount BIGINT NOT NULL,
    balance_after BIGINT NOT NULL,
    direction VARCHAR(10) NOT NULL,
    entry_type VARCHAR(20) NOT NULL,
    idempotency_key VARCHAR(255) UNIQUE,
    description TEXT,
    created_at TIMESTAMP DEFAULT NOW()
);

-- Faucet claims
CREATE TABLE faucet_claims (
    id BIGSERIAL PRIMARY KEY,
    user_id BIGINT REFERENCES users(id) ON DELETE CASCADE,
    amount BIGINT NOT NULL,
    claimed_at TIMESTAMP DEFAULT NOW(),
    next_claim_at TIMESTAMP NOT NULL
);

-- Rate limit attempts
CREATE TABLE rate_limit_attempts (
    endpoint VARCHAR(50) NOT NULL,
    identifier VARCHAR(255) NOT NULL,
    attempts INT NOT NULL DEFAULT 1,
    window_start TIMESTAMP NOT NULL,
    locked_until TIMESTAMP,
    PRIMARY KEY (endpoint, identifier)
);

-- Collusion flags
CREATE TABLE collusion_flags (
    id BIGSERIAL PRIMARY KEY,
    user_id BIGINT REFERENCES users(id) ON DELETE CASCADE,
    table_id BIGINT NOT NULL,
    flag_type VARCHAR(50) NOT NULL,
    severity VARCHAR(20) NOT NULL,
    details JSONB,
    created_at TIMESTAMP DEFAULT NOW(),
    reviewed BOOLEAN DEFAULT FALSE,
    reviewer_user_id BIGINT REFERENCES users(id),
    reviewed_at TIMESTAMP
);

-- Indexes
CREATE INDEX idx_sessions_user_id ON sessions(user_id);
CREATE INDEX idx_sessions_refresh_token ON sessions(refresh_token);
CREATE INDEX idx_wallet_entries_user_id ON wallet_entries(user_id);
CREATE INDEX idx_wallet_entries_created_at ON wallet_entries(created_at DESC);
CREATE INDEX idx_faucet_claims_user_id ON faucet_claims(user_id);
CREATE INDEX idx_rate_limit_attempts_identifier ON rate_limit_attempts(identifier);
CREATE INDEX idx_collusion_flags_user_id ON collusion_flags(user_id);
CREATE INDEX idx_collusion_flags_reviewed ON collusion_flags(reviewed) WHERE NOT reviewed;
```

### Backup Strategy

**Daily backups**:
```bash
#!/bin/bash
# /opt/scripts/backup-db.sh

BACKUP_DIR="/var/backups/poker"
DATE=$(date +%Y%m%d_%H%M%S)
DB_NAME="poker_db"

# Create backup
pg_dump -U poker $DB_NAME | gzip > "$BACKUP_DIR/poker_$DATE.sql.gz"

# Keep only last 30 days
find $BACKUP_DIR -name "poker_*.sql.gz" -mtime +30 -delete
```

Add to crontab:
```bash
0 2 * * * /opt/scripts/backup-db.sh
```

**Restore from backup**:
```bash
gunzip < poker_20240115_020000.sql.gz | psql -U poker poker_db
```

---

## Security

### JWT Secret Generation

Generate a secure secret:

```bash
openssl rand -base64 32
```

Store securely (never commit to git):
```bash
# Use environment variable
export JWT_SECRET="generated_secret_here"

# Or use secrets management
# - AWS Secrets Manager
# - HashiCorp Vault
# - Kubernetes Secrets
```

### SSL/TLS Setup

Use nginx as reverse proxy with Let's Encrypt:

```nginx
server {
    listen 443 ssl http2;
    server_name poker.example.com;

    ssl_certificate /etc/letsencrypt/live/poker.example.com/fullchain.pem;
    ssl_certificate_key /etc/letsencrypt/live/poker.example.com/privkey.pem;

    location / {
        proxy_pass http://127.0.0.1:8080;
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;
    }
}
```

### Firewall Configuration

```bash
# UFW (Ubuntu)
sudo ufw allow 22/tcp      # SSH
sudo ufw allow 443/tcp     # HTTPS
sudo ufw allow 8080/tcp    # Poker server (if not using nginx)
sudo ufw enable
```

### Rate Limiting

Rate limits are built-in but can be tuned:

```rust
// In auth/rate_limiter.rs
RateLimitConfig::login() {
    max_attempts: 5,         // Attempts allowed
    window_secs: 300,        // 5 minute window
    lockout_secs: 900,       // 15 minute lockout
    exponential_backoff: true,
}
```

### Anti-Collusion Monitoring

Review flagged activity daily:

```sql
-- Unreviewed collusion flags
SELECT
    f.id,
    f.user_id,
    u.username,
    f.table_id,
    f.flag_type,
    f.severity,
    f.details,
    f.created_at
FROM collusion_flags f
JOIN users u ON f.user_id = u.id
WHERE NOT f.reviewed
ORDER BY f.created_at DESC;
```

Mark as reviewed:

```sql
UPDATE collusion_flags
SET reviewed = true,
    reviewer_user_id = <admin_user_id>,
    reviewed_at = NOW()
WHERE id = <flag_id>;
```

---

## Monitoring

### Health Checks

**HTTP endpoint**:
```bash
curl http://localhost:8080/health
```

Expected response:
```json
{
  "status": "healthy",
  "database": "connected",
  "uptime_secs": 12345,
  "active_connections": 42
}
```

### Metrics Collection

Use Prometheus for metrics:

```yaml
# prometheus.yml
scrape_configs:
  - job_name: 'poker-server'
    static_configs:
      - targets: ['localhost:8080']
    metrics_path: '/metrics'
```

**Key Metrics:**
- `poker_active_connections` - Current connections
- `poker_active_tables` - Active game tables
- `poker_total_hands` - Hands played (counter)
- `poker_wallet_balance_total` - Total chips in circulation
- `poker_db_connection_pool` - Database connection usage

### Logging

Logs are written to stdout/stderr. Capture with systemd:

```bash
# View logs
sudo journalctl -u poker-server -f

# Last 100 lines
sudo journalctl -u poker-server -n 100

# Filter by priority
sudo journalctl -u poker-server -p err
```

**Log rotation** with logrotate:

```bash
# /etc/logrotate.d/poker-server
/var/log/poker-server/*.log {
    daily
    rotate 30
    compress
    delaycompress
    notifempty
    create 0644 poker poker
    sharedscripts
    postrotate
        systemctl reload poker-server
    endscript
}
```

### Alerting

Set up alerts for:
- Server down (health check fails)
- Database connection errors
- High error rate (>5% of requests)
- Disk space <10%
- Memory usage >80%
- Unreviewed collusion flags >10

**Example: Prometheus AlertManager**:

```yaml
groups:
  - name: poker_alerts
    rules:
      - alert: PokerServerDown
        expr: up{job="poker-server"} == 0
        for: 1m
        labels:
          severity: critical
        annotations:
          summary: "Poker server is down"

      - alert: HighErrorRate
        expr: rate(poker_errors_total[5m]) > 0.05
        for: 5m
        labels:
          severity: warning
        annotations:
          summary: "High error rate detected"
```

---

## Maintenance

### Routine Tasks

**Daily:**
- Check error logs
- Review collusion flags
- Monitor disk space
- Verify backups completed

**Weekly:**
- Review rate limit lockouts
- Analyze player statistics
- Check database performance
- Update dependencies

**Monthly:**
- Security audit
- Performance tuning
- Capacity planning
- User feedback review

### Database Maintenance

**Vacuum**:
```sql
VACUUM ANALYZE;
```

**Reindex**:
```sql
REINDEX DATABASE poker_db;
```

**Table statistics**:
```sql
SELECT
    schemaname,
    tablename,
    n_live_tup as rows,
    n_dead_tup as dead_rows,
    last_vacuum,
    last_autovacuum
FROM pg_stat_user_tables
ORDER BY n_live_tup DESC;
```

### Updating Server

```bash
# Pull latest code
git pull origin main

# Build new version
cargo build --release

# Stop server
sudo systemctl stop poker-server

# Backup current binary
cp /opt/private_poker/pp_server /opt/private_poker/pp_server.backup

# Deploy new binary
cp target/release/pp_server /opt/private_poker/

# Run migrations (if any)
sqlx migrate run

# Start server
sudo systemctl start poker-server

# Verify
sudo systemctl status poker-server
curl http://localhost:8080/health
```

### Rollback Procedure

```bash
# Stop server
sudo systemctl stop poker-server

# Restore previous binary
cp /opt/private_poker/pp_server.backup /opt/private_poker/pp_server

# Restore database (if needed)
gunzip < /var/backups/poker/poker_latest.sql.gz | psql -U poker poker_db

# Start server
sudo systemctl start poker-server
```

---

## Troubleshooting

### Server Won't Start

**Check logs**:
```bash
sudo journalctl -u poker-server -n 50
```

**Common issues**:
1. Database connection failed
   - Verify `DATABASE_URL`
   - Check PostgreSQL is running
   - Test connection: `psql $DATABASE_URL`

2. Port already in use
   - Check: `sudo lsof -i :8080`
   - Kill process or change port

3. Permission denied
   - Check file ownership: `ls -l /opt/private_poker/pp_server`
   - Fix: `sudo chown poker:poker /opt/private_poker/pp_server`

### High CPU Usage

**Identify cause**:
```bash
# Check server CPU
top -p $(pgrep pp_server)

# Profile with perf
sudo perf record -p $(pgrep pp_server) -g -- sleep 10
sudo perf report
```

**Common causes**:
- Too many active tables
- Inefficient queries
- Memory leaks

**Solutions**:
- Limit max tables per server
- Add database indexes
- Restart server (temporary)
- Update to latest version

### Database Performance Issues

**Slow queries**:
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

**Connection pool exhaustion**:
```sql
SELECT count(*) FROM pg_stat_activity;
```

If near `max_connections`, increase pool size or optimize queries.

### Memory Leaks

**Monitor memory**:
```bash
ps aux | grep pp_server
```

**Heap profiling** (requires recompile with `jemalloc`):
```bash
MALLOC_CONF=prof:true,lg_prof_sample:0,prof_final:true ./pp_server
```

### Network Issues

**Test connectivity**:
```bash
# From client machine
telnet poker.example.com 8080
nc -zv poker.example.com 8080
```

**Check firewall**:
```bash
sudo ufw status
sudo iptables -L -n
```

**Packet loss**:
```bash
ping -c 100 poker.example.com
```

---

## Scaling

### Vertical Scaling

**When to scale up:**
- CPU usage consistently >80%
- RAM usage >85%
- Database connections near max
- Response times degrading

**Recommendations:**
- CPU: Add cores (4 → 8)
- RAM: Double capacity (8GB → 16GB)
- Storage: Upgrade to faster SSD
- Network: Increase bandwidth

### Horizontal Scaling

**Multi-server architecture:**

```
            Load Balancer
                 |
        ┌────────┼────────┐
        │        │        │
    Server 1  Server 2  Server 3
        │        │        │
        └────────┼────────┘
                 |
         PostgreSQL Primary
                 |
            Replicas
```

**Considerations:**
- Shared PostgreSQL database
- Session affinity in load balancer
- Distributed rate limiting (Redis)
- Centralized metrics collection

### Database Scaling

**Read replicas**:
```sql
-- Setup replication
-- On primary
CREATE ROLE replicator WITH REPLICATION LOGIN ENCRYPTED PASSWORD 'password';

-- On replica
pg_basebackup -h primary -D /var/lib/postgresql/data -U replicator -P -v -R
```

**Connection pooling** with PgBouncer:
```ini
[databases]
poker_db = host=localhost port=5432 dbname=poker_db

[pgbouncer]
listen_addr = 127.0.0.1
listen_port = 6432
auth_type = md5
pool_mode = transaction
max_client_conn = 1000
default_pool_size = 25
```

---

## Best Practices

### Security
- Keep JWT secret secure and rotate regularly
- Use strong passwords for database
- Enable SSL/TLS for all connections
- Regular security audits
- Keep software updated
- Monitor for suspicious activity

### Performance
- Use connection pooling
- Add database indexes for hot queries
- Cache frequent reads (Redis)
- Monitor and optimize slow queries
- Compress network traffic

### Reliability
- Daily database backups
- Test restore procedures
- Monitor server health
- Set up alerting
- Have rollback plan
- Document procedures

### Compliance
- Log all admin actions
- Retain transaction history
- Implement audit trails
- GDPR compliance for EU users
- Regular data backups
- Secure data deletion

---

## Support

For additional help:
- GitHub Issues: https://github.com/your-org/private_poker/issues
- Documentation: `/docs` directory
- Community: Discord/Slack

---

Generated with Claude Code
