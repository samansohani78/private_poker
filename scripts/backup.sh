#!/bin/bash
# Automated backup script for Private Poker database
# 
# Usage:
#   ./scripts/backup.sh
#
# Environment variables:
#   BACKUP_DIR - Directory to store backups (default: ./backups)
#   DATABASE_URL - PostgreSQL connection string
#   RETENTION_DAYS - Number of days to keep backups (default: 30)

set -e

# Configuration
BACKUP_DIR="${BACKUP_DIR:-./backups}"
RETENTION_DAYS="${RETENTION_DAYS:-30}"
TIMESTAMP=$(date +%Y%m%d_%H%M%S)
BACKUP_FILE="${BACKUP_DIR}/poker_db_${TIMESTAMP}.sql.gz"

# Colors
GREEN='\033[0;32m'
YELLOW='\033[0;33m'
RED='\033[0;31m'
NC='\033[0m' # No Color

echo -e "${GREEN}Starting database backup...${NC}"

# Create backup directory if it doesn't exist
mkdir -p "${BACKUP_DIR}"

# Extract database connection info from DATABASE_URL
if [ -z "$DATABASE_URL" ]; then
    echo -e "${RED}ERROR: DATABASE_URL environment variable not set${NC}"
    exit 1
fi

# Perform backup
echo -e "${YELLOW}Backing up to ${BACKUP_FILE}...${NC}"
pg_dump "$DATABASE_URL" | gzip > "$BACKUP_FILE"

# Verify backup was created
if [ -f "$BACKUP_FILE" ]; then
    SIZE=$(du -h "$BACKUP_FILE" | cut -f1)
    echo -e "${GREEN}Backup completed successfully: ${BACKUP_FILE} (${SIZE})${NC}"
else
    echo -e "${RED}ERROR: Backup file was not created${NC}"
    exit 1
fi

# Clean up old backups
echo -e "${YELLOW}Cleaning up backups older than ${RETENTION_DAYS} days...${NC}"
find "${BACKUP_DIR}" -name "poker_db_*.sql.gz" -type f -mtime +${RETENTION_DAYS} -delete

REMAINING=$(find "${BACKUP_DIR}" -name "poker_db_*.sql.gz" -type f | wc -l)
echo -e "${GREEN}Cleanup complete. ${REMAINING} backup(s) remaining.${NC}"

echo -e "${GREEN}Backup process finished successfully!${NC}"
