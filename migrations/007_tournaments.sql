-- Tournament tables for Sit-n-Go and scheduled tournaments
-- Part of Phase 7 implementation

-- Tournaments table
CREATE TABLE IF NOT EXISTS tournaments (
    id BIGSERIAL PRIMARY KEY,
    name VARCHAR(255) NOT NULL,
    tournament_type VARCHAR(50) NOT NULL CHECK (tournament_type IN ('sit_and_go', 'scheduled')),
    config JSONB NOT NULL,
    state VARCHAR(50) NOT NULL CHECK (state IN ('registering', 'running', 'finished', 'cancelled')) DEFAULT 'registering',
    buy_in BIGINT NOT NULL CHECK (buy_in > 0),
    min_players INT NOT NULL CHECK (min_players >= 2),
    max_players INT NOT NULL CHECK (max_players >= min_players),
    starting_stack BIGINT NOT NULL CHECK (starting_stack > 0),
    registered_count INT NOT NULL DEFAULT 0 CHECK (registered_count >= 0),
    current_level INT NOT NULL DEFAULT 1 CHECK (current_level > 0),
    level_started_at TIMESTAMP,
    created_at TIMESTAMP NOT NULL DEFAULT NOW(),
    started_at TIMESTAMP,
    finished_at TIMESTAMP,
    CONSTRAINT valid_dates CHECK (
        (started_at IS NULL OR started_at >= created_at) AND
        (finished_at IS NULL OR (started_at IS NOT NULL AND finished_at >= started_at))
    )
);

-- Tournament registrations table
CREATE TABLE IF NOT EXISTS tournament_registrations (
    id BIGSERIAL PRIMARY KEY,
    tournament_id BIGINT NOT NULL REFERENCES tournaments(id) ON DELETE CASCADE,
    user_id BIGINT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    username VARCHAR(50) NOT NULL,
    chip_count BIGINT NOT NULL CHECK (chip_count >= 0),
    finish_position INT,
    prize_amount BIGINT,
    registered_at TIMESTAMP NOT NULL DEFAULT NOW(),
    finished_at TIMESTAMP,
    UNIQUE(tournament_id, user_id),
    CONSTRAINT valid_finish CHECK (
        (finish_position IS NULL AND finished_at IS NULL) OR
        (finish_position IS NOT NULL AND finished_at IS NOT NULL)
    )
);

-- Indexes for performance
CREATE INDEX IF NOT EXISTS idx_tournaments_state ON tournaments(state);
CREATE INDEX IF NOT EXISTS idx_tournaments_created_at ON tournaments(created_at DESC);
CREATE INDEX IF NOT EXISTS idx_tournament_registrations_tournament_id ON tournament_registrations(tournament_id);
CREATE INDEX IF NOT EXISTS idx_tournament_registrations_user_id ON tournament_registrations(user_id);
CREATE INDEX IF NOT EXISTS idx_tournament_registrations_finish_position ON tournament_registrations(tournament_id, finish_position) WHERE finish_position IS NOT NULL;

-- Comments for documentation
COMMENT ON TABLE tournaments IS 'Stores tournament configurations and state';
COMMENT ON TABLE tournament_registrations IS 'Tracks player registrations and results for tournaments';
COMMENT ON COLUMN tournaments.config IS 'JSONB containing full TournamentConfig (blind levels, etc.)';
COMMENT ON COLUMN tournaments.current_level IS 'Current blind level (1-indexed)';
COMMENT ON COLUMN tournaments.registered_count IS 'Cached count of registered players';
COMMENT ON COLUMN tournament_registrations.chip_count IS 'Current chip count (updated during play)';
COMMENT ON COLUMN tournament_registrations.finish_position IS 'Final position (1 = winner, NULL if still playing)';
COMMENT ON COLUMN tournament_registrations.prize_amount IS 'Prize won (NULL if out of the money)';
