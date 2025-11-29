-- Migration: Add missing indexes for query optimization
-- Date: November 23, 2025
-- Description: Adds indexes for hand history, wallet queries, and faucet cooldowns
--              to improve query performance on frequently accessed tables

-- Add index for hand history by game (for hand replays)
-- This enables fast retrieval of all actions for a specific game
CREATE INDEX IF NOT EXISTS idx_hand_history_game_id
    ON hand_history(game_id);

-- Add composite index for user wallet history with date filtering
-- This optimizes queries like "show me this user's recent transactions"
CREATE INDEX IF NOT EXISTS idx_wallet_entries_user_created
    ON wallet_entries(user_id, created_at DESC);

-- Add index for faucet cooldown checks
-- This speeds up the check for "when can this user claim again?"
CREATE INDEX IF NOT EXISTS idx_faucet_claims_user_claimed
    ON faucet_claims(user_id, claimed_at DESC);

-- Add index for collusion flag reviews
-- This helps admins quickly find unreviewed flags
CREATE INDEX IF NOT EXISTS idx_collusion_flags_reviewed_created
    ON collusion_flags(reviewed, created_at DESC);

-- Add index for session lookup by token
-- This speeds up token validation during authentication
-- Note: token already has a primary key index, but we add expires_at for filtering
CREATE INDEX IF NOT EXISTS idx_sessions_token_expires
    ON sessions(token, expires_at);
