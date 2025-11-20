-- Migration: Add unique constraint to rate_limit_attempts table
-- Date: November 17, 2025
-- Description: Adds UNIQUE constraint on (endpoint, identifier) to support ON CONFLICT clause
--
-- Issue: The rate_limiter.rs code uses "ON CONFLICT (endpoint, identifier)" but the
-- table only had an INDEX, not a UNIQUE constraint. This caused errors:
-- "there is no unique or exclusion constraint matching the ON CONFLICT specification"

-- Drop the existing non-unique index
DROP INDEX IF EXISTS idx_rate_limit_endpoint_identifier;

-- Add unique constraint on (endpoint, identifier) combination
-- This allows ON CONFLICT clauses to work properly
ALTER TABLE rate_limit_attempts
ADD CONSTRAINT rate_limit_attempts_endpoint_identifier_unique
UNIQUE (endpoint, identifier);

-- Note: This constraint ensures each (endpoint, identifier) pair appears only once,
-- which is the intended behavior for rate limiting per endpoint per user/IP
