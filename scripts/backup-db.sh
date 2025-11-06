#!/bin/bash
# Database backup script for Private Poker
# Usage: ./backup-db.sh [backup_name]
# Add to crontab for automated backups:
#   0 2 * * * /opt/scripts/backup-db.sh >> /var/log/poker-backup.log 2>&1

set -e

# Configuration
BACKUP_DIR="${BACKUP_DIR:-/var/backups/poker}"
RETENTION_DAYS="${RETENTION_DAYS:-30}"
DB_NAME="${POSTGRES_DB:-poker_db}"
DB_USER="${POSTGRES_USER:-poker}"
DB_HOST="${DB_HOST:-localhost}"
DB_PORT="${DB_PORT:-5432}"

# Generate timestamp
TIMESTAMP=$(date +%Y%m%d_%H%M%S)
BACKUP_NAME="${1:-poker_$TIMESTAMP}"
BACKUP_FILE="$BACKUP_DIR/${BACKUP_NAME}.sql.gz"

# Create backup directory if it doesn't exist
mkdir -p "$BACKUP_DIR"

echo "[$(date)] Starting database backup: $BACKUP_NAME"

# Perform backup
echo "[$(date)] Dumping database..."
if pg_dump -h "$DB_HOST" -p "$DB_PORT" -U "$DB_USER" "$DB_NAME" | gzip > "$BACKUP_FILE"; then
    echo "[$(date)] Backup completed: $BACKUP_FILE"

    # Get file size
    SIZE=$(du -h "$BACKUP_FILE" | cut -f1)
    echo "[$(date)] Backup size: $SIZE"
else
    echo "[$(date)] ERROR: Backup failed!" >&2
    exit 1
fi

# Clean up old backups
echo "[$(date)] Cleaning up backups older than $RETENTION_DAYS days..."
find "$BACKUP_DIR" -name "poker_*.sql.gz" -mtime +$RETENTION_DAYS -delete
REMAINING=$(find "$BACKUP_DIR" -name "poker_*.sql.gz" | wc -l)
echo "[$(date)] Backups remaining: $REMAINING"

# Optional: Upload to cloud storage
if [ -n "$AWS_S3_BUCKET" ]; then
    echo "[$(date)] Uploading to S3: $AWS_S3_BUCKET"
    aws s3 cp "$BACKUP_FILE" "s3://$AWS_S3_BUCKET/backups/" || echo "[$(date)] WARNING: S3 upload failed"
fi

echo "[$(date)] Backup process completed successfully"
exit 0
