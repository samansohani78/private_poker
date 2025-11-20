-- Migration: Add balance CHECK constraints to prevent negative balances
-- Date: November 16, 2025
-- Description: Adds database-level constraints to ensure wallet and escrow balances cannot become negative

-- Add CHECK constraint to wallets table
ALTER TABLE wallets
ADD CONSTRAINT wallets_balance_non_negative CHECK (balance >= 0);

-- Add CHECK constraint to table_escrows table
ALTER TABLE table_escrows
ADD CONSTRAINT escrows_balance_non_negative CHECK (balance >= 0);

-- Note: These constraints provide defense-in-depth alongside application-level checks
-- They prevent data corruption even if application logic has bugs
