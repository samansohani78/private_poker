#!/bin/bash
# Database restore script for Private Poker
# Usage: ./restore-db.sh <backup_file>

set -e

# Configuration
DB_NAME="${POSTGRES_DB:-poker_db}"
DB_USER="${POSTGRES_USER:-poker}"
DB_HOST="${DB_HOST:-localhost}"
DB_PORT="${DB_PORT:-5432}"

# Check arguments
if [ $# -eq 0 ]; then
    echo "Usage: $0 <backup_file>"
    echo "Example: $0 /var/backups/poker/poker_20240115_020000.sql.gz"
    exit 1
fi

BACKUP_FILE="$1"

# Verify backup file exists
if [ ! -f "$BACKUP_FILE" ]; then
    echo "ERROR: Backup file not found: $BACKUP_FILE" >&2
    exit 1
fi

echo "[$(date)] Starting database restore from: $BACKUP_FILE"

# Confirmation prompt
echo "WARNING: This will OVERWRITE the current database: $DB_NAME"
read -p "Are you sure you want to continue? (yes/no): " CONFIRM

if [ "$CONFIRM" != "yes" ]; then
    echo "Restore cancelled."
    exit 0
fi

# Stop poker server (if running)
echo "[$(date)] Stopping poker server..."
if systemctl is-active --quiet poker-server; then
    sudo systemctl stop poker-server
    RESTART_SERVER=true
else
    RESTART_SERVER=false
fi

# Drop existing database (with confirmation)
echo "[$(date)] Dropping existing database..."
psql -h "$DB_HOST" -p "$DB_PORT" -U "$DB_USER" -d postgres -c "DROP DATABASE IF EXISTS $DB_NAME;"

# Create fresh database
echo "[$(date)] Creating fresh database..."
psql -h "$DB_HOST" -p "$DB_PORT" -U "$DB_USER" -d postgres -c "CREATE DATABASE $DB_NAME OWNER $DB_USER;"

# Restore from backup
echo "[$(date)] Restoring database from backup..."
if gunzip < "$BACKUP_FILE" | psql -h "$DB_HOST" -p "$DB_PORT" -U "$DB_USER" "$DB_NAME"; then
    echo "[$(date)] Database restored successfully"
else
    echo "[$(date)] ERROR: Restore failed!" >&2
    exit 1
fi

# Verify restoration
echo "[$(date)] Verifying restoration..."
TABLE_COUNT=$(psql -h "$DB_HOST" -p "$DB_PORT" -U "$DB_USER" -d "$DB_NAME" -t -c "SELECT COUNT(*) FROM information_schema.tables WHERE table_schema = 'public';")
echo "[$(date)] Tables restored: $TABLE_COUNT"

# Restart poker server if it was running
if [ "$RESTART_SERVER" = true ]; then
    echo "[$(date)] Restarting poker server..."
    sudo systemctl start poker-server
    sleep 2
    sudo systemctl status poker-server
fi

echo "[$(date)] Restore process completed successfully"
exit 0
