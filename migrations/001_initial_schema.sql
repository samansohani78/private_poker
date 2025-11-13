-- ======================
-- USERS & AUTHENTICATION
-- ======================

CREATE TABLE users (
    id BIGSERIAL PRIMARY KEY,
    username VARCHAR(20) UNIQUE NOT NULL,
    password_hash VARCHAR(255) NOT NULL,

    -- Profile
    display_name VARCHAR(50) NOT NULL,
    avatar_url VARCHAR(500),
    email VARCHAR(255),
    country CHAR(2),              -- ISO country code
    timezone VARCHAR(50),          -- IANA timezone

    -- Consent
    tos_version SMALLINT NOT NULL DEFAULT 1,
    privacy_version SMALLINT NOT NULL DEFAULT 1,

    -- Status
    is_active BOOLEAN NOT NULL DEFAULT TRUE,
    is_admin BOOLEAN NOT NULL DEFAULT FALSE,

    -- Timestamps
    created_at TIMESTAMP NOT NULL DEFAULT NOW(),
    last_login TIMESTAMP,

    CONSTRAINT username_length CHECK (char_length(username) BETWEEN 3 AND 20),
    CONSTRAINT username_format CHECK (username ~ '^[a-zA-Z0-9_]+$')
);

CREATE INDEX idx_users_username ON users(username);
CREATE INDEX idx_users_email ON users(email) WHERE email IS NOT NULL;
CREATE INDEX idx_users_created_at ON users(created_at);

-- ======================
-- SESSIONS
-- ======================

CREATE TABLE sessions (
    token VARCHAR(255) PRIMARY KEY,
    user_id BIGINT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    device_fingerprint VARCHAR(255) NOT NULL,
    created_at TIMESTAMP NOT NULL DEFAULT NOW(),
    expires_at TIMESTAMP NOT NULL,
    last_used TIMESTAMP NOT NULL DEFAULT NOW(),

    CONSTRAINT valid_expiry CHECK (expires_at > created_at)
);

CREATE INDEX idx_sessions_user_id ON sessions(user_id);
CREATE INDEX idx_sessions_expires_at ON sessions(expires_at);

-- Auto-delete expired sessions
CREATE OR REPLACE FUNCTION delete_expired_sessions()
RETURNS void AS $$
BEGIN
    DELETE FROM sessions WHERE expires_at < NOW();
END;
$$ LANGUAGE plpgsql;

-- ======================
-- TWO-FACTOR AUTH
-- ======================

CREATE TABLE two_factor_auth (
    user_id BIGINT PRIMARY KEY REFERENCES users(id) ON DELETE CASCADE,
    secret VARCHAR(255) NOT NULL,
    enabled BOOLEAN NOT NULL DEFAULT FALSE,
    backup_codes TEXT[],  -- Array of hashed backup codes
    created_at TIMESTAMP NOT NULL DEFAULT NOW(),
    enabled_at TIMESTAMP
);

-- ======================
-- PASSWORD RESET
-- ======================

CREATE TABLE password_reset_requests (
    id BIGSERIAL PRIMARY KEY,
    user_id BIGINT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    email VARCHAR(255) NOT NULL,
    code VARCHAR(6) NOT NULL,
    created_at TIMESTAMP NOT NULL DEFAULT NOW(),
    expires_at TIMESTAMP NOT NULL,
    used BOOLEAN NOT NULL DEFAULT FALSE,

    CONSTRAINT valid_expiry CHECK (expires_at > created_at)
);

CREATE INDEX idx_password_reset_user_id ON password_reset_requests(user_id);
CREATE INDEX idx_password_reset_expires_at ON password_reset_requests(expires_at);

-- ======================
-- WALLETS
-- ======================

CREATE TABLE wallets (
    user_id BIGINT PRIMARY KEY REFERENCES users(id) ON DELETE CASCADE,
    balance BIGINT NOT NULL DEFAULT 10000,
    currency VARCHAR(10) NOT NULL DEFAULT 'CHIP',
    created_at TIMESTAMP NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMP NOT NULL DEFAULT NOW(),

    CONSTRAINT positive_balance CHECK (balance >= 0)
);

CREATE INDEX idx_wallets_currency ON wallets(currency);

-- ======================
-- TABLE ESCROWS
-- ======================

CREATE TABLE table_escrows (
    table_id BIGINT PRIMARY KEY,
    balance BIGINT NOT NULL DEFAULT 0,
    created_at TIMESTAMP NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMP NOT NULL DEFAULT NOW(),

    CONSTRAINT positive_balance CHECK (balance >= 0)
);

-- ======================
-- WALLET LEDGER (Double-Entry)
-- ======================

CREATE TABLE wallet_entries (
    id BIGSERIAL PRIMARY KEY,
    user_id BIGINT NOT NULL,  -- Can be negative for escrow accounts
    table_id BIGINT,
    amount BIGINT NOT NULL,   -- Positive = credit, negative = debit
    balance_after BIGINT NOT NULL,
    direction VARCHAR(10) NOT NULL CHECK (direction IN ('debit', 'credit')),
    entry_type VARCHAR(20) NOT NULL CHECK (entry_type IN ('buy_in', 'cash_out', 'rake', 'bonus', 'admin_adjust', 'transfer')),
    idempotency_key VARCHAR(255) UNIQUE NOT NULL,
    description TEXT,
    created_at TIMESTAMP NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_wallet_entries_user_id ON wallet_entries(user_id);
CREATE INDEX idx_wallet_entries_table_id ON wallet_entries(table_id);
CREATE INDEX idx_wallet_entries_created_at ON wallet_entries(created_at);
CREATE INDEX idx_wallet_entries_idempotency_key ON wallet_entries(idempotency_key);

-- ======================
-- FAUCET CLAIMS
-- ======================

CREATE TABLE faucet_claims (
    id BIGSERIAL PRIMARY KEY,
    user_id BIGINT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    amount BIGINT NOT NULL,
    claimed_at TIMESTAMP NOT NULL DEFAULT NOW(),
    next_claim_at TIMESTAMP NOT NULL,

    CONSTRAINT positive_amount CHECK (amount > 0)
);

CREATE INDEX idx_faucet_claims_user_id ON faucet_claims(user_id);
CREATE INDEX idx_faucet_claims_next_claim ON faucet_claims(next_claim_at);

-- ======================
-- TABLES
-- ======================

CREATE TABLE tables (
    id BIGSERIAL PRIMARY KEY,
    name VARCHAR(255) NOT NULL,

    -- Config
    max_players INT NOT NULL DEFAULT 10,
    small_blind BIGINT NOT NULL,
    big_blind BIGINT NOT NULL,
    min_buy_in_bb SMALLINT NOT NULL DEFAULT 20,
    max_buy_in_bb SMALLINT NOT NULL DEFAULT 100,
    absolute_chip_cap BIGINT NOT NULL DEFAULT 100000,
    top_up_cooldown_hands SMALLINT NOT NULL DEFAULT 20,

    -- Features
    speed VARCHAR(10) NOT NULL DEFAULT 'normal' CHECK (speed IN ('normal', 'turbo', 'hyper')),
    bots_enabled BOOLEAN NOT NULL DEFAULT TRUE,
    target_bot_count SMALLINT NOT NULL DEFAULT 5,
    bot_difficulty VARCHAR(10) NOT NULL DEFAULT 'standard' CHECK (bot_difficulty IN ('easy', 'standard', 'tag')),

    -- Privacy
    is_private BOOLEAN NOT NULL DEFAULT FALSE,
    passphrase_hash VARCHAR(255),
    invite_token VARCHAR(255),
    invite_expires_at TIMESTAMP,

    -- Status
    is_active BOOLEAN NOT NULL DEFAULT TRUE,
    created_at TIMESTAMP NOT NULL DEFAULT NOW(),
    creator_user_id BIGINT REFERENCES users(id),

    CONSTRAINT valid_blinds CHECK (big_blind > small_blind),
    CONSTRAINT valid_buy_in CHECK (max_buy_in_bb > min_buy_in_bb)
);

CREATE INDEX idx_tables_is_active ON tables(is_active);
CREATE INDEX idx_tables_speed ON tables(speed);
CREATE INDEX idx_tables_bots_enabled ON tables(bots_enabled);
CREATE INDEX idx_tables_is_private ON tables(is_private);
CREATE INDEX idx_tables_invite_token ON tables(invite_token) WHERE invite_token IS NOT NULL;

-- ======================
-- USER STATISTICS
-- ======================

CREATE TABLE user_stats (
    user_id BIGINT PRIMARY KEY REFERENCES users(id) ON DELETE CASCADE,
    hands_played INT NOT NULL DEFAULT 0,
    vpip REAL NOT NULL DEFAULT 0.0,
    pfr REAL NOT NULL DEFAULT 0.0,
    wtsd REAL NOT NULL DEFAULT 0.0,
    w_usd REAL NOT NULL DEFAULT 0.0,
    net_chips BIGINT NOT NULL DEFAULT 0,
    last_updated TIMESTAMP NOT NULL DEFAULT NOW()
);

-- ======================
-- GAME HISTORY
-- ======================

CREATE TABLE game_history (
    id BIGSERIAL PRIMARY KEY,
    table_id BIGINT NOT NULL,
    game_number BIGINT NOT NULL,
    small_blind BIGINT NOT NULL,
    big_blind BIGINT NOT NULL,
    started_at TIMESTAMP NOT NULL,
    ended_at TIMESTAMP,
    winner_user_id BIGINT REFERENCES users(id),
    pot_size BIGINT,
    num_players INT,

    UNIQUE (table_id, game_number)
);

CREATE INDEX idx_game_history_table_id ON game_history(table_id);
CREATE INDEX idx_game_history_started_at ON game_history(started_at);
CREATE INDEX idx_game_history_winner ON game_history(winner_user_id);

-- ======================
-- HAND HISTORY
-- ======================

CREATE TABLE hand_history (
    id BIGSERIAL PRIMARY KEY,
    game_id BIGINT NOT NULL REFERENCES game_history(id) ON DELETE CASCADE,
    user_id BIGINT NOT NULL REFERENCES users(id),
    hole_cards JSONB,
    position INT NOT NULL,
    actions JSONB,
    final_chips BIGINT NOT NULL,
    showed_hand BOOLEAN NOT NULL DEFAULT FALSE
);

CREATE INDEX idx_hand_history_game_id ON hand_history(game_id);
CREATE INDEX idx_hand_history_user_id ON hand_history(user_id);

-- ======================
-- CHAT MESSAGES
-- ======================

CREATE TABLE chat_messages (
    id BIGSERIAL PRIMARY KEY,
    table_id BIGINT NOT NULL,
    user_id BIGINT NOT NULL REFERENCES users(id),
    message TEXT NOT NULL,
    created_at TIMESTAMP NOT NULL DEFAULT NOW(),

    CONSTRAINT message_length CHECK (char_length(message) <= 500)
);

CREATE INDEX idx_chat_messages_table_id ON chat_messages(table_id);
CREATE INDEX idx_chat_messages_created_at ON chat_messages(created_at);

-- ======================
-- BOT TELEMETRY
-- ======================

CREATE TABLE bot_telemetry (
    id BIGSERIAL PRIMARY KEY,
    bot_id INT NOT NULL,
    table_id BIGINT NOT NULL,
    stakes_tier VARCHAR(10) NOT NULL,
    difficulty VARCHAR(10) NOT NULL,
    hands_played INT NOT NULL,
    win_rate REAL NOT NULL,
    vpip REAL NOT NULL,
    pfr REAL NOT NULL,
    aggression_factor REAL NOT NULL,
    showdown_rate REAL NOT NULL,
    updated_at TIMESTAMP NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_bot_telemetry_table_id ON bot_telemetry(table_id);
CREATE INDEX idx_bot_telemetry_stakes ON bot_telemetry(stakes_tier);

-- ======================
-- RATE LIMITING
-- ======================

CREATE TABLE rate_limit_attempts (
    id BIGSERIAL PRIMARY KEY,
    endpoint VARCHAR(50) NOT NULL,
    identifier VARCHAR(255) NOT NULL,  -- IP or username
    attempts INT NOT NULL DEFAULT 1,
    window_start TIMESTAMP NOT NULL DEFAULT NOW(),
    locked_until TIMESTAMP
);

CREATE INDEX idx_rate_limit_endpoint_identifier ON rate_limit_attempts(endpoint, identifier);
CREATE INDEX idx_rate_limit_window ON rate_limit_attempts(window_start);

-- ======================
-- ANTI-COLLUSION FLAGS
-- ======================

CREATE TABLE collusion_flags (
    id BIGSERIAL PRIMARY KEY,
    user_id BIGINT NOT NULL REFERENCES users(id),
    table_id BIGINT NOT NULL,
    flag_type VARCHAR(50) NOT NULL,
    severity VARCHAR(10) NOT NULL CHECK (severity IN ('low', 'medium', 'high')),
    details JSONB,
    created_at TIMESTAMP NOT NULL DEFAULT NOW(),
    reviewed BOOLEAN NOT NULL DEFAULT FALSE,
    reviewer_user_id BIGINT REFERENCES users(id),
    reviewed_at TIMESTAMP
);

CREATE INDEX idx_collusion_flags_user_id ON collusion_flags(user_id);
CREATE INDEX idx_collusion_flags_table_id ON collusion_flags(table_id);
CREATE INDEX idx_collusion_flags_reviewed ON collusion_flags(reviewed) WHERE NOT reviewed;

-- ======================
-- INITIAL DATA
-- ======================

-- NOTE: Admin user creation deferred until AuthManager is implemented
-- Default admin credentials will be created via migration script

-- NOTE: Tournament tables moved to migration 007_tournaments.sql
