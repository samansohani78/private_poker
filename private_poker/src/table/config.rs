//! Table configuration models.

use serde::{Deserialize, Serialize};

/// Table speed variants
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TableSpeed {
    Normal,
    Turbo,
    Hyper,
}

impl std::fmt::Display for TableSpeed {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TableSpeed::Normal => write!(f, "normal"),
            TableSpeed::Turbo => write!(f, "turbo"),
            TableSpeed::Hyper => write!(f, "hyper"),
        }
    }
}

/// Bot difficulty presets
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum BotDifficulty {
    Easy,     // Loose-passive, high VPIP (45%), low aggression
    Standard, // Balanced, moderate VPIP (30%), TAG-style
    Tag,      // Tight-aggressive, low VPIP (20%), high aggression
}

impl std::fmt::Display for BotDifficulty {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BotDifficulty::Easy => write!(f, "easy"),
            BotDifficulty::Standard => write!(f, "standard"),
            BotDifficulty::Tag => write!(f, "tag"),
        }
    }
}

/// Table configuration
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TableConfig {
    /// Table name
    pub name: String,

    /// Maximum number of players (default: 10)
    pub max_players: usize,

    /// Small blind amount
    pub small_blind: i64,

    /// Big blind amount
    pub big_blind: i64,

    /// Minimum buy-in in big blinds (e.g., 20 BB)
    pub min_buy_in_bb: u8,

    /// Maximum buy-in in big blinds (e.g., 100 BB)
    pub max_buy_in_bb: u8,

    /// Absolute chip cap (hard limit: 100,000)
    pub absolute_chip_cap: i64,

    /// Top-up cooldown in hands (e.g., 20 hands between top-ups)
    pub top_up_cooldown_hands: u8,

    /// Table speed
    pub speed: TableSpeed,

    /// Whether bots are enabled
    pub bots_enabled: bool,

    /// Target number of players including bots (default: 5)
    pub target_bot_count: u8,

    /// Bot difficulty preset
    pub bot_difficulty: BotDifficulty,

    /// Whether table is private (requires passphrase or invite)
    pub is_private: bool,

    /// Argon2id-hashed passphrase (if private)
    pub passphrase_hash: Option<String>,

    /// Expiring invite token (if private)
    pub invite_token: Option<String>,

    /// Invite token expiration timestamp
    pub invite_expires_at: Option<chrono::DateTime<chrono::Utc>>,
}

impl Default for TableConfig {
    fn default() -> Self {
        Self {
            name: "Default Table".to_string(),
            max_players: 10,
            small_blind: 50,
            big_blind: 100,
            min_buy_in_bb: 20,
            max_buy_in_bb: 100,
            absolute_chip_cap: 100_000,
            top_up_cooldown_hands: 20,
            speed: TableSpeed::Normal,
            bots_enabled: true,
            target_bot_count: 5,
            bot_difficulty: BotDifficulty::Standard,
            is_private: false,
            passphrase_hash: None,
            invite_token: None,
            invite_expires_at: None,
        }
    }
}

impl TableConfig {
    /// Validate configuration
    pub fn validate(&self) -> Result<(), String> {
        if self.big_blind <= self.small_blind {
            return Err("Big blind must be greater than small blind".to_string());
        }

        if self.max_buy_in_bb <= self.min_buy_in_bb {
            return Err("Max buy-in must be greater than min buy-in".to_string());
        }

        if self.max_players == 0 || self.max_players > 23 {
            return Err("Max players must be between 1 and 23".to_string());
        }

        if self.absolute_chip_cap <= 0 || self.absolute_chip_cap > 100_000 {
            return Err("Absolute chip cap must be between 1 and 100,000".to_string());
        }

        Ok(())
    }

    /// Get minimum buy-in in chips
    pub fn min_buy_in_chips(&self) -> i64 {
        self.big_blind * self.min_buy_in_bb as i64
    }

    /// Get maximum buy-in in chips
    pub fn max_buy_in_chips(&self) -> i64 {
        let bb_max = self.big_blind * self.max_buy_in_bb as i64;
        bb_max.min(self.absolute_chip_cap)
    }

    /// Get action timeout based on table speed
    pub fn action_timeout_secs(&self) -> u64 {
        match self.speed {
            TableSpeed::Normal => 30,
            TableSpeed::Turbo => 15,
            TableSpeed::Hyper => 5,
        }
    }
}
