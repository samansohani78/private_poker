#!/bin/bash
# Private Poker - Organization Script
# Reorganizes test scripts into better structure
# SAFE - No deletions, only moves

set -e

echo "================================================"
echo "Private Poker - Organization"
echo "================================================"
echo ""
echo "This script will:"
echo "  1. Create scripts/tests/ directory"
echo "  2. Move test_*.sh and debug_*.sh to scripts/tests/"
echo "  3. Create scripts/README.md with documentation"
echo ""
read -p "Continue? (y/N): " -n 1 -r
echo
if [[ ! $REPLY =~ ^[Yy]$ ]]; then
    echo "Cancelled."
    exit 0
fi

echo ""
echo "Starting organization..."
echo ""

# Create directories
mkdir -p scripts/tests/
echo "✓ Created scripts/tests/"

# Move test scripts
MOVED=0
for script in test_*.sh debug_*.sh; do
    if [ -f "$script" ]; then
        git mv "$script" scripts/tests/
        echo "✓ Moved $script → scripts/tests/"
        MOVED=$((MOVED + 1))
    fi
done

if [ $MOVED -eq 0 ]; then
    echo "⊘ No test scripts found to move"
fi

# Create scripts README
cat > scripts/README.md << 'EOF'
# Scripts Directory

## Database Scripts

### backup-db.sh
Backs up PostgreSQL database to a timestamped SQL file.

**Usage**:
```bash
./scripts/backup-db.sh
```

### restore-db.sh
Restores PostgreSQL database from a backup file.

**Usage**:
```bash
./scripts/restore-db.sh <backup-file.sql>
```

---

## Test Scripts

### tests/test_full_system.sh
Full system integration test including:
- Database cleanup
- Server startup
- User registration via API
- Table listing
- TUI/Web client verification

**Usage**:
```bash
./scripts/tests/test_full_system.sh
```

**Environment**:
- Requires PostgreSQL running
- Uses DATABASE_URL from .env or hardcoded credentials
- Starts server on port 8080

---

### tests/test_complete_flow.sh
Complete flow test for game mechanics.

**Usage**:
```bash
./scripts/tests/test_complete_flow.sh
```

---

### tests/test_game_flow.sh
Game flow integration test.

**Usage**:
```bash
./scripts/tests/test_game_flow.sh
```

---

### tests/test_join_fix.sh
Test for table join functionality.

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

---

## Configuration

All scripts use environment variables from `.env` or fallback to defaults:

- `DATABASE_URL` - PostgreSQL connection string
- `SERVER_URL` - Server endpoint (default: http://localhost:8080)
- `POSTGRES_USER` - Database user
- `POSTGRES_PASSWORD` - Database password
- `POSTGRES_DB` - Database name

---

## Development Workflow

### 1. Run Full System Test
```bash
./scripts/tests/test_full_system.sh
```

### 2. Backup Database Before Testing
```bash
./scripts/backup-db.sh
```

### 3. Restore After Failed Test
```bash
./scripts/restore-db.sh backups/backup_YYYYMMDD_HHMMSS.sql
```

---

## Notes

- All test scripts require PostgreSQL running
- Test scripts create temporary test users
- Server logs are written to /tmp/pp_server.log
- Kill any existing server processes before testing

---

**Last Updated**: November 2025
EOF

git add scripts/README.md
echo "✓ Created scripts/README.md"

echo ""
echo "Staging changes for commit..."
git status --short

echo ""
read -p "Commit these changes? (y/N): " -n 1 -r
echo
if [[ $REPLY =~ ^[Yy]$ ]]; then
    git commit -m "chore: Organize scripts directory

- Move test scripts to scripts/tests/
- Add scripts/README.md with comprehensive documentation
- Improve project organization
- No functional changes
"
    echo ""
    echo "✓ Changes committed"
else
    echo ""
    echo "Changes staged but not committed."
    echo "To commit later: git commit"
    echo "To undo: git reset HEAD"
fi

echo ""
echo "================================================"
echo "Organization complete!"
echo "================================================"
echo ""
echo "New structure:"
echo "  scripts/"
echo "  ├── README.md          ← Documentation"
echo "  ├── backup-db.sh       ← Database backup"
echo "  ├── restore-db.sh      ← Database restore"
echo "  └── tests/"
echo "      ├── test_full_system.sh"
echo "      ├── test_complete_flow.sh"
echo "      ├── test_game_flow.sh"
echo "      ├── test_join_fix.sh"
echo "      └── debug_game.sh"
echo ""
echo "View documentation: cat scripts/README.md"
