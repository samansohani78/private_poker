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
        let mut rng = rand::rng();
        let variance = rng.random_range(0..=self.params.think_time_variance_ms);
        let sign = if rng.random_bool(0.5) { 1 } else { -1 };

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_difficulty_params_easy() {
        let params = DifficultyParams::easy();
        assert_eq!(params.vpip, 0.45);
        assert_eq!(params.pfr, 0.10);
        assert_eq!(params.aggression_factor, 0.5);
        assert!(!params.bluffs);
        assert_eq!(params.bluff_frequency, 0.0);
    }

    #[test]
    fn test_difficulty_params_standard() {
        let params = DifficultyParams::standard();
        assert_eq!(params.vpip, 0.30);
        assert_eq!(params.pfr, 0.20);
        assert_eq!(params.aggression_factor, 1.5);
        assert!(params.bluffs);
        assert_eq!(params.bluff_frequency, 0.15);
    }

    #[test]
    fn test_difficulty_params_tag() {
        let params = DifficultyParams::tag();
        assert_eq!(params.vpip, 0.20);
        assert_eq!(params.pfr, 0.18);
        assert_eq!(params.aggression_factor, 2.5);
        assert!(params.bluffs);
        assert_eq!(params.bluff_frequency, 0.25);
    }

    #[test]
    fn test_difficulty_params_from_difficulty() {
        let easy = DifficultyParams::from_difficulty(BotDifficulty::Easy);
        assert_eq!(easy.vpip, 0.45);

        let standard = DifficultyParams::from_difficulty(BotDifficulty::Standard);
        assert_eq!(standard.vpip, 0.30);

        let tag = DifficultyParams::from_difficulty(BotDifficulty::Tag);
        assert_eq!(tag.vpip, 0.20);
    }

    #[test]
    fn test_bot_stats_vpip_zero_hands() {
        let stats = BotStats::default();
        assert_eq!(stats.vpip(), 0.0);
    }

    #[test]
    fn test_bot_stats_vpip_calculation() {
        let stats = BotStats {
            hands_played: 100,
            vpip_count: 30,
            ..Default::default()
        };
        assert_eq!(stats.vpip(), 0.30);
    }

    #[test]
    fn test_bot_stats_pfr_zero_hands() {
        let stats = BotStats::default();
        assert_eq!(stats.pfr(), 0.0);
    }

    #[test]
    fn test_bot_stats_pfr_calculation() {
        let stats = BotStats {
            hands_played: 100,
            pfr_count: 20,
            ..Default::default()
        };
        assert_eq!(stats.pfr(), 0.20);
    }

    #[test]
    fn test_bot_stats_aggression_factor_zero_passive() {
        let stats = BotStats {
            aggressive_actions: 50,
            passive_actions: 0,
            ..Default::default()
        };
        assert_eq!(stats.aggression_factor(), 50.0);
    }

    #[test]
    fn test_bot_stats_aggression_factor_calculation() {
        let stats = BotStats {
            aggressive_actions: 60,
            passive_actions: 40,
            ..Default::default()
        };
        assert_eq!(stats.aggression_factor(), 1.5);
    }

    #[test]
    fn test_bot_stats_showdown_rate_zero_hands() {
        let stats = BotStats::default();
        assert_eq!(stats.showdown_rate(), 0.0);
    }

    #[test]
    fn test_bot_stats_showdown_rate_calculation() {
        let stats = BotStats {
            hands_played: 100,
            showdown_count: 25,
            ..Default::default()
        };
        assert_eq!(stats.showdown_rate(), 0.25);
    }

    #[test]
    fn test_bot_stats_win_rate_zero_hands() {
        let stats = BotStats::default();
        assert_eq!(stats.win_rate(), 0.0);
    }

    #[test]
    fn test_bot_stats_win_rate_positive() {
        let stats = BotStats {
            starting_chips: 1000,
            current_chips: 1500,
            hands_played: 100,
            ..Default::default()
        };
        assert_eq!(stats.win_rate(), 5.0); // +500 chips / 100 hands = 5.0
    }

    #[test]
    fn test_bot_stats_win_rate_negative() {
        let stats = BotStats {
            starting_chips: 1000,
            current_chips: 800,
            hands_played: 50,
            ..Default::default()
        };
        assert_eq!(stats.win_rate(), -4.0); // -200 chips / 50 hands = -4.0
    }

    #[test]
    fn test_bot_player_new() {
        let config = BotConfig {
            id: 1,
            name: "TestBot".to_string(),
            difficulty: BotDifficulty::Standard,
            table_id: 1,
            starting_chips: 1000,
        };

        let bot = BotPlayer::new(config.clone());
        assert_eq!(bot.config.id, 1);
        assert_eq!(bot.config.name, "TestBot");
        assert_eq!(bot.stats.starting_chips, 1000);
        assert_eq!(bot.stats.current_chips, 1000);
        assert!(bot.last_action_time.is_none());
    }

    #[test]
    fn test_bot_player_get_think_delay_ms() {
        let config = BotConfig {
            id: 1,
            name: "TestBot".to_string(),
            difficulty: BotDifficulty::Standard,
            table_id: 1,
            starting_chips: 1000,
        };

        let bot = BotPlayer::new(config);
        let delay = bot.get_think_delay_ms();

        // Should be at least 500ms (minimum)
        assert!(delay >= 500);

        // Should be roughly around base time ± variance
        // Standard has 2000ms base and 1500ms variance, so 500-3500ms range
        assert!(delay <= 4000);
    }

    #[test]
    fn test_bot_player_record_hand_won() {
        let config = BotConfig {
            id: 1,
            name: "TestBot".to_string(),
            difficulty: BotDifficulty::Easy,
            table_id: 1,
            starting_chips: 1000,
        };

        let mut bot = BotPlayer::new(config);
        bot.record_hand(true, 1200);

        assert_eq!(bot.stats.hands_played, 1);
        assert_eq!(bot.stats.hands_won, 1);
        assert_eq!(bot.stats.current_chips, 1200);
    }

    #[test]
    fn test_bot_player_record_hand_lost() {
        let config = BotConfig {
            id: 1,
            name: "TestBot".to_string(),
            difficulty: BotDifficulty::Easy,
            table_id: 1,
            starting_chips: 1000,
        };

        let mut bot = BotPlayer::new(config);
        bot.record_hand(false, 900);

        assert_eq!(bot.stats.hands_played, 1);
        assert_eq!(bot.stats.hands_won, 0);
        assert_eq!(bot.stats.current_chips, 900);
    }

    #[test]
    fn test_bot_player_record_vpip() {
        let config = BotConfig {
            id: 1,
            name: "TestBot".to_string(),
            difficulty: BotDifficulty::Tag,
            table_id: 1,
            starting_chips: 1000,
        };

        let mut bot = BotPlayer::new(config);
        bot.record_vpip();
        bot.record_vpip();

        assert_eq!(bot.stats.vpip_count, 2);
    }

    #[test]
    fn test_bot_player_record_pfr() {
        let config = BotConfig {
            id: 1,
            name: "TestBot".to_string(),
            difficulty: BotDifficulty::Tag,
            table_id: 1,
            starting_chips: 1000,
        };

        let mut bot = BotPlayer::new(config);
        bot.record_pfr();

        assert_eq!(bot.stats.pfr_count, 1);
    }

    #[test]
    fn test_bot_player_record_aggressive_action() {
        let config = BotConfig {
            id: 1,
            name: "TestBot".to_string(),
            difficulty: BotDifficulty::Standard,
            table_id: 1,
            starting_chips: 1000,
        };

        let mut bot = BotPlayer::new(config);
        bot.record_aggressive_action();
        bot.record_aggressive_action();
        bot.record_aggressive_action();

        assert_eq!(bot.stats.aggressive_actions, 3);
    }

    #[test]
    fn test_bot_player_record_passive_action() {
        let config = BotConfig {
            id: 1,
            name: "TestBot".to_string(),
            difficulty: BotDifficulty::Easy,
            table_id: 1,
            starting_chips: 1000,
        };

        let mut bot = BotPlayer::new(config);
        bot.record_passive_action();
        bot.record_passive_action();

        assert_eq!(bot.stats.passive_actions, 2);
    }

    #[test]
    fn test_bot_player_record_showdown() {
        let config = BotConfig {
            id: 1,
            name: "TestBot".to_string(),
            difficulty: BotDifficulty::Standard,
            table_id: 1,
            starting_chips: 1000,
        };

        let mut bot = BotPlayer::new(config);
        bot.record_showdown();

        assert_eq!(bot.stats.showdown_count, 1);
    }

    #[test]
    fn test_bot_player_full_session() {
        let config = BotConfig {
            id: 1,
            name: "TestBot".to_string(),
            difficulty: BotDifficulty::Standard,
            table_id: 1,
            starting_chips: 1000,
        };

        let mut bot = BotPlayer::new(config);

        // Play 10 hands
        for i in 0..10 {
            // VPIP in 3 out of 10 hands (30%)
            if i < 3 {
                bot.record_vpip();
                bot.record_pfr(); // Also raise pre-flop
                bot.record_aggressive_action();
            }

            // Win 4 hands
            let won = i < 4;
            let chips = if won { 1000 + (i + 1) * 50 } else { 1000 };
            bot.record_hand(won, chips);
        }

        assert_eq!(bot.stats.hands_played, 10);
        assert_eq!(bot.stats.hands_won, 4);
        assert_eq!(bot.stats.vpip_count, 3);
        assert_eq!(bot.stats.pfr_count, 3);
        assert_eq!(bot.stats.vpip(), 0.30);
        assert_eq!(bot.stats.pfr(), 0.30);
    }

    #[test]
    fn test_bot_config_clone() {
        let config = BotConfig {
            id: 1,
            name: "TestBot".to_string(),
            difficulty: BotDifficulty::Easy,
            table_id: 1,
            starting_chips: 1000,
        };

        let cloned = config.clone();
        assert_eq!(config.id, cloned.id);
        assert_eq!(config.name, cloned.name);
    }

    #[test]
    fn test_difficulty_params_clone() {
        let params = DifficultyParams::standard();
        let cloned = params.clone();
        assert_eq!(params.vpip, cloned.vpip);
        assert_eq!(params.pfr, cloned.pfr);
    }

    #[test]
    fn test_bot_stats_default() {
        let stats = BotStats::default();
        assert_eq!(stats.hands_played, 0);
        assert_eq!(stats.hands_won, 0);
        assert_eq!(stats.vpip_count, 0);
        assert_eq!(stats.pfr_count, 0);
        assert_eq!(stats.showdown_count, 0);
        assert_eq!(stats.aggressive_actions, 0);
        assert_eq!(stats.passive_actions, 0);
    }
}
