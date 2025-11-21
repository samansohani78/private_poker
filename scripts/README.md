# Scripts Directory

This directory contains operational scripts for database management and testing.

## Database Scripts

### backup-db.sh
Backs up PostgreSQL database to a timestamped SQL file.

**Usage**:
```bash
./scripts/backup-db.sh
```

**Output**: Creates backup file in `backups/backup_YYYYMMDD_HHMMSS.sql`

---

### restore-db.sh
Restores PostgreSQL database from a backup file.

**Usage**:
```bash
./scripts/restore-db.sh <backup-file.sql>
```

**Example**:
```bash
./scripts/restore-db.sh backups/backup_20251121_143000.sql
```

---

## Test Scripts

All test scripts are located in the `tests/` subdirectory.

### tests/test_full_system.sh
Full system integration test including:
- Database cleanup and reset
- Server startup
- User registration via API
- Table listing and verification
- TUI/Web client connectivity test

**Usage**:
```bash
./scripts/tests/test_full_system.sh
```

**Requirements**:
- PostgreSQL running on localhost:5432
- DATABASE_URL set in .env or environment
- Server port 8080 available

---

### tests/test_complete_flow.sh
Complete poker game flow test.

**Usage**:
```bash
./scripts/tests/test_complete_flow.sh
```

---

### tests/test_game_flow.sh
Game flow integration test focusing on betting rounds and state transitions.

**Usage**:
```bash
./scripts/tests/test_game_flow.sh
```

---

### tests/test_join_fix.sh
Test for table join functionality and player management.

**Usage**:
```bash
./scripts/tests/test_join_fix.sh
```

---

### tests/debug_game.sh
Debug script for troubleshooting game issues.

**Usage**:
```bash
./scripts/tests/debug_game.sh
```

**Features**:
- Verbose logging
- Step-by-step execution
- Database state inspection
- Game state dumps

---

## Configuration

All scripts use environment variables from `.env` or fallback to defaults:

- `DATABASE_URL` - PostgreSQL connection string (default: postgresql://postgres:7794951@localhost:5432/poker_db)
- `SERVER_URL` - Server endpoint (default: http://localhost:8080)
- `POSTGRES_USER` - Database user (default: postgres)
- `POSTGRES_PASSWORD` - Database password
- `POSTGRES_DB` - Database name (default: poker_db)

---

## Development Workflow

### 1. Backup Before Testing
```bash
./scripts/backup-db.sh
```

### 2. Run Full System Test
```bash
./scripts/tests/test_full_system.sh
```

### 3. Restore After Failed Test (if needed)
```bash
./scripts/restore-db.sh backups/backup_YYYYMMDD_HHMMSS.sql
```

---

## Notes

- All test scripts require PostgreSQL running
- Test scripts create temporary test users (alice, bob, carol)
- Server logs are written to /tmp/pp_server.log
- Kill any existing server processes before testing: `pkill pp_server`
- Scripts are safe to run multiple times (idempotent where possible)

---

**Last Updated**: November 2025
