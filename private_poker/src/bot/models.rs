//! Bot player models and configuration.

use crate::table::config::BotDifficulty;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Bot player identifier
pub type BotId = i32;

/// Bot player configuration
#[derive(Debug, Clone)]
pub struct BotConfig {
    /// Bot unique ID
    pub id: BotId,

    /// Bot display name
    pub name: String,

    /// Difficulty preset
    pub difficulty: BotDifficulty,

    /// Table ID where bot is playing
    pub table_id: i64,

    /// Initial chip count
    pub starting_chips: i64,
}

/// Bot difficulty parameters
#[derive(Debug, Clone)]
pub struct DifficultyParams {
    /// VPIP (Voluntarily Put $ In Pot) percentage
    pub vpip: f32,

    /// PFR (Pre-Flop Raise) percentage
    pub pfr: f32,

    /// Aggression factor (ratio of bets/raises to calls)
    pub aggression_factor: f32,

    /// Fold to 3-bet percentage
    pub fold_to_3bet: f32,

    /// Continuation bet frequency on flop
    pub cbet_frequency: f32,

    /// Average thinking time in milliseconds (base)
    pub base_think_time_ms: u64,

    /// Random variance in thinking time (±milliseconds)
    pub think_time_variance_ms: u64,

    /// Whether bot bluffs
    pub bluffs: bool,

    /// Bluff frequency (0.0 to 1.0)
    pub bluff_frequency: f32,
}

impl DifficultyParams {
    /// Get parameters for Easy difficulty
    /// Loose-passive: plays many hands, rarely aggressive
    pub fn easy() -> Self {
        Self {
            vpip: 0.45,                   // Plays 45% of hands
            pfr: 0.10,                    // Raises pre-flop 10%
            aggression_factor: 0.5,       // Passive (more calls than raises)
            fold_to_3bet: 0.70,           // Folds to re-raises 70%
            cbet_frequency: 0.40,         // Continuation bets 40%
            base_think_time_ms: 1500,     // Thinks ~1.5s base
            think_time_variance_ms: 1000, // ±1s variance
            bluffs: false,                // Never bluffs
            bluff_frequency: 0.0,
        }
    }

    /// Get parameters for Standard difficulty
    /// Balanced TAG (Tight-Aggressive) style
    pub fn standard() -> Self {
        Self {
            vpip: 0.30,                   // Plays 30% of hands
            pfr: 0.20,                    // Raises pre-flop 20%
            aggression_factor: 1.5,       // Moderately aggressive
            fold_to_3bet: 0.50,           // Folds to re-raises 50%
            cbet_frequency: 0.65,         // Continuation bets 65%
            base_think_time_ms: 2000,     // Thinks ~2s base
            think_time_variance_ms: 1500, // ±1.5s variance
            bluffs: true,                 // Bluffs occasionally
            bluff_frequency: 0.15,        // Bluffs 15% of time
        }
    }

    /// Get parameters for TAG (Tight-Aggressive) difficulty
    /// Very tight, very aggressive when playing
    pub fn tag() -> Self {
        Self {
            vpip: 0.20,                   // Plays only 20% of hands
            pfr: 0.18,                    // Raises pre-flop 18%
            aggression_factor: 2.5,       // Very aggressive
            fold_to_3bet: 0.35,           // Only folds to re-raises 35%
            cbet_frequency: 0.75,         // Continuation bets 75%
            base_think_time_ms: 2500,     // Thinks ~2.5s base
            think_time_variance_ms: 2000, // ±2s variance
            bluffs: true,                 // Bluffs strategically
            bluff_frequency: 0.25,        // Bluffs 25% of time
        }
    }

    /// Get parameters for a given difficulty
    pub fn from_difficulty(difficulty: BotDifficulty) -> Self {
        match difficulty {
            BotDifficulty::Easy => Self::easy(),
            BotDifficulty::Standard => Self::standard(),
            BotDifficulty::Tag => Self::tag(),
        }
    }
}

/// Bot telemetry data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BotTelemetry {
    pub bot_id: BotId,
    pub table_id: i64,
    pub stakes_tier: String,
    pub difficulty: String,
    pub hands_played: i32,
    pub win_rate: f32,
    pub vpip: f32,
    pub pfr: f32,
    pub aggression_factor: f32,
    pub showdown_rate: f32,
    pub updated_at: DateTime<Utc>,
}

/// Bot statistics tracker
#[derive(Debug, Clone, Default)]
pub struct BotStats {
    /// Total hands played
    pub hands_played: u32,

    /// Total hands won
    pub hands_won: u32,

    /// Times voluntarily put money in pot
    pub vpip_count: u32,

    /// Times raised pre-flop
    pub pfr_count: u32,

    /// Times went to showdown
    pub showdown_count: u32,

    /// Total bets/raises made
    pub aggressive_actions: u32,

    /// Total calls made
    pub passive_actions: u32,

    /// Starting chips
    pub starting_chips: i64,

    /// Current chips
    pub current_chips: i64,
}

impl BotStats {
    /// Calculate VPIP percentage
    pub fn vpip(&self) -> f32 {
        if self.hands_played == 0 {
            0.0
        } else {
            self.vpip_count as f32 / self.hands_played as f32
        }
    }

    /// Calculate PFR percentage
    pub fn pfr(&self) -> f32 {
        if self.hands_played == 0 {
            0.0
        } else {
            self.pfr_count as f32 / self.hands_played as f32
        }
    }

    /// Calculate aggression factor
    pub fn aggression_factor(&self) -> f32 {
        if self.passive_actions == 0 {
            self.aggressive_actions as f32
        } else {
            self.aggressive_actions as f32 / self.passive_actions as f32
        }
    }

    /// Calculate showdown rate
    pub fn showdown_rate(&self) -> f32 {
        if self.hands_played == 0 {
            0.0
        } else {
            self.showdown_count as f32 / self.hands_played as f32
        }
    }

    /// Calculate win rate (chips won per hand)
    pub fn win_rate(&self) -> f32 {
        if self.hands_played == 0 {
            0.0
        } else {
            let net_chips = self.current_chips - self.starting_chips;
            net_chips as f32 / self.hands_played as f32
        }
    }
}

/// Bot player state
#[derive(Debug, Clone)]
pub struct BotPlayer {
    /// Bot configuration
    pub config: BotConfig,

    /// Difficulty parameters
    pub params: DifficultyParams,

    /// Statistics tracker
    pub stats: BotStats,

    /// Last action timestamp (for pacing)
    pub last_action_time: Option<DateTime<Utc>>,
}

impl BotPlayer {
    /// Create a new bot player
    pub fn new(config: BotConfig) -> Self {
        let params = DifficultyParams::from_difficulty(config.difficulty);
        let stats = BotStats {
            starting_chips: config.starting_chips,
            current_chips: config.starting_chips,
            ..Default::default()
        };

        Self {
            config,
            params,
            stats,
            last_action_time: None,
        }
    }

    /// Get thinking delay in milliseconds (with randomization)
    pub fn get_think_delay_ms(&self) -> u64 {
        use rand::Rng;
        let mut rng = rand::thread_rng();
        let variance = rng.gen_range(0..=self.params.think_time_variance_ms);
        let sign = if rng.gen_bool(0.5) { 1 } else { -1 };

        let delay = self.params.base_think_time_ms as i64 + (variance as i64 * sign);
        delay.max(500) as u64 // Minimum 500ms
    }

    /// Record a hand played
    pub fn record_hand(&mut self, won: bool, chips: i64) {
        self.stats.hands_played += 1;
        if won {
            self.stats.hands_won += 1;
        }
        self.stats.current_chips = chips;
    }

    /// Record VPIP (voluntarily put money in pot)
    pub fn record_vpip(&mut self) {
        self.stats.vpip_count += 1;
    }

    /// Record PFR (pre-flop raise)
    pub fn record_pfr(&mut self) {
        self.stats.pfr_count += 1;
    }

    /// Record aggressive action (bet/raise)
    pub fn record_aggressive_action(&mut self) {
        self.stats.aggressive_actions += 1;
    }

    /// Record passive action (call)
    pub fn record_passive_action(&mut self) {
        self.stats.passive_actions += 1;
    }

    /// Record showdown
    pub fn record_showdown(&mut self) {
        self.stats.showdown_count += 1;
    }
}
