-- Migration: Additional performance indexes
-- Purpose: Add composite and single-column indexes for high-traffic query patterns
-- Date: 2025-11-28

-- Game history lookup by table + date range (for table history queries)
CREATE INDEX IF NOT EXISTS idx_game_history_table_started
    ON game_history(table_id, started_at DESC);

-- Session cleanup query optimization (for expired session cleanup job)
CREATE INDEX IF NOT EXISTS idx_sessions_expires_user
    ON sessions(expires_at, user_id);

-- Wallet transaction type filtering (for transaction history by type)
CREATE INDEX IF NOT EXISTS idx_wallet_entries_entry_type
    ON wallet_entries(entry_type);

-- IP-based anti-collusion lookups (for same-IP detection at tables)
CREATE INDEX IF NOT EXISTS idx_ip_table_restrictions_ip_table
    ON ip_table_restrictions(ip_address, table_id);

-- Hand history filtering by user (for player hand history queries)
CREATE INDEX IF NOT EXISTS idx_hand_history_user_game
    ON hand_history(user_id, game_id);

-- Faucet claim frequency checks (for cooldown enforcement)
CREATE INDEX IF NOT EXISTS idx_faucet_claims_user_claimed
    ON faucet_claims(user_id, claimed_at DESC);
