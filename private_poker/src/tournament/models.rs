//! Tournament data models for Sit-n-Go tournaments.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Tournament ID type
pub type TournamentId = i64;

/// Tournament state
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TournamentState {
    /// Accepting registrations
    Registering,
    /// Tournament in progress
    Running,
    /// Tournament finished
    Finished,
    /// Tournament cancelled
    Cancelled,
}

/// Tournament type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TournamentType {
    /// Sit-n-Go (starts when full)
    SitAndGo,
    /// Scheduled tournament (starts at specific time)
    Scheduled,
}

/// Blind structure for tournament
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BlindLevel {
    /// Level number (1-indexed)
    pub level: u32,
    /// Small blind amount
    pub small_blind: i64,
    /// Big blind amount
    pub big_blind: i64,
    /// Ante amount (optional)
    pub ante: Option<i64>,
    /// Duration of this level in seconds
    pub duration_secs: u32,
}

impl BlindLevel {
    /// Create a new blind level
    pub fn new(level: u32, small_blind: i64, big_blind: i64, duration_secs: u32) -> Self {
        Self {
            level,
            small_blind,
            big_blind,
            ante: None,
            duration_secs,
        }
    }

    /// Create a blind level with ante
    pub fn with_ante(mut self, ante: i64) -> Self {
        self.ante = Some(ante);
        self
    }
}

/// Prize structure for tournament
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrizeStructure {
    /// Total prize pool
    pub total_pool: i64,
    /// Payouts by position (1st, 2nd, 3rd, etc.)
    pub payouts: Vec<i64>,
}

impl PrizeStructure {
    /// Create standard prize structure for given number of players
    ///
    /// Standard structures:
    /// - 2-5 players: Winner takes all
    /// - 6-9 players: 60/40 split
    /// - 10+ players: 50/30/20 split
    pub fn standard(total_players: usize, buy_in: i64) -> Self {
        let total_pool = (total_players as i64) * buy_in;

        let payouts = match total_players {
            0..=1 => vec![total_pool],
            2..=5 => vec![total_pool], // Winner takes all
            6..=9 => {
                // 60/40 split
                vec![
                    (total_pool as f64 * 0.60) as i64,
                    (total_pool as f64 * 0.40) as i64,
                ]
            }
            _ => {
                // 50/30/20 split
                vec![
                    (total_pool as f64 * 0.50) as i64,
                    (total_pool as f64 * 0.30) as i64,
                    (total_pool as f64 * 0.20) as i64,
                ]
            }
        };

        Self {
            total_pool,
            payouts,
        }
    }

    /// Create custom prize structure
    pub fn custom(total_pool: i64, percentages: Vec<f64>) -> Self {
        let payouts = percentages
            .iter()
            .map(|pct| (total_pool as f64 * pct) as i64)
            .collect();

        Self {
            total_pool,
            payouts,
        }
    }

    /// Get payout for a specific position (1-indexed)
    pub fn payout_for_position(&self, position: usize) -> Option<i64> {
        if position == 0 || position > self.payouts.len() {
            None
        } else {
            Some(self.payouts[position - 1])
        }
    }
}

/// Tournament configuration
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TournamentConfig {
    /// Tournament name
    pub name: String,
    /// Tournament type
    pub tournament_type: TournamentType,
    /// Buy-in amount
    pub buy_in: i64,
    /// Minimum players required
    pub min_players: usize,
    /// Maximum players allowed
    pub max_players: usize,
    /// Starting chip stack for each player
    pub starting_stack: i64,
    /// Blind level structure
    pub blind_levels: Vec<BlindLevel>,
    /// Starting blind level (usually 1)
    pub starting_level: u32,
    /// Scheduled start time (for Scheduled tournaments)
    pub scheduled_start: Option<DateTime<Utc>>,
    /// Late registration period in seconds
    pub late_registration_secs: Option<u32>,
}

impl TournamentConfig {
    /// Create a standard Sit-n-Go configuration
    pub fn sit_and_go(name: String, max_players: usize, buy_in: i64) -> Self {
        let starting_stack = buy_in * 50; // 50x buy-in starting stack

        // Standard blind structure (doubles every 2 levels)
        let blind_levels = vec![
            BlindLevel::new(1, 10, 20, 300),    // 5 min
            BlindLevel::new(2, 15, 30, 300),    // 5 min
            BlindLevel::new(3, 20, 40, 300),    // 5 min
            BlindLevel::new(4, 30, 60, 300),    // 5 min
            BlindLevel::new(5, 40, 80, 300),    // 5 min
            BlindLevel::new(6, 60, 120, 300),   // 5 min
            BlindLevel::new(7, 80, 160, 300),   // 5 min
            BlindLevel::new(8, 120, 240, 300),  // 5 min
            BlindLevel::new(9, 160, 320, 300),  // 5 min
            BlindLevel::new(10, 240, 480, 300), // 5 min
        ];

        Self {
            name,
            tournament_type: TournamentType::SitAndGo,
            buy_in,
            min_players: 2,
            max_players,
            starting_stack,
            blind_levels,
            starting_level: 1,
            scheduled_start: None,
            late_registration_secs: None,
        }
    }

    /// Create a turbo Sit-n-Go (faster blind increases)
    pub fn turbo_sit_and_go(name: String, max_players: usize, buy_in: i64) -> Self {
        let mut config = Self::sit_and_go(name, max_players, buy_in);
        // Turbo: 3-minute levels
        for level in &mut config.blind_levels {
            level.duration_secs = 180;
        }
        config
    }

    /// Get blind level by number
    pub fn get_blind_level(&self, level: u32) -> Option<&BlindLevel> {
        self.blind_levels.iter().find(|bl| bl.level == level)
    }
}

/// Tournament registration entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TournamentRegistration {
    /// User ID
    pub user_id: i64,
    /// Username
    pub username: String,
    /// Registration timestamp
    pub registered_at: DateTime<Utc>,
    /// Current chip count (updated during tournament)
    pub chip_count: i64,
    /// Finishing position (None if still in tournament)
    pub finish_position: Option<usize>,
    /// Prize amount (None if not in the money)
    pub prize_amount: Option<i64>,
}

/// Tournament information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TournamentInfo {
    /// Tournament ID
    pub id: TournamentId,
    /// Tournament configuration
    pub config: TournamentConfig,
    /// Current state
    pub state: TournamentState,
    /// Registered players
    pub registered_count: usize,
    /// Current blind level
    pub current_level: u32,
    /// Time until next level (seconds)
    pub time_to_next_level: Option<u32>,
    /// Prize structure
    pub prize_structure: PrizeStructure,
    /// Created at timestamp
    pub created_at: DateTime<Utc>,
    /// Started at timestamp
    pub started_at: Option<DateTime<Utc>>,
    /// Finished at timestamp
    pub finished_at: Option<DateTime<Utc>>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_standard_prize_structure_winner_takes_all() {
        let prize = PrizeStructure::standard(5, 100);
        assert_eq!(prize.total_pool, 500);
        assert_eq!(prize.payouts.len(), 1);
        assert_eq!(prize.payouts[0], 500);
    }

    #[test]
    fn test_standard_prize_structure_two_payouts() {
        let prize = PrizeStructure::standard(8, 100);
        assert_eq!(prize.total_pool, 800);
        assert_eq!(prize.payouts.len(), 2);
        assert_eq!(prize.payouts[0], 480); // 60%
        assert_eq!(prize.payouts[1], 320); // 40%
    }

    #[test]
    fn test_standard_prize_structure_three_payouts() {
        let prize = PrizeStructure::standard(10, 100);
        assert_eq!(prize.total_pool, 1000);
        assert_eq!(prize.payouts.len(), 3);
        assert_eq!(prize.payouts[0], 500); // 50%
        assert_eq!(prize.payouts[1], 300); // 30%
        assert_eq!(prize.payouts[2], 200); // 20%
    }

    #[test]
    fn test_custom_prize_structure() {
        let prize = PrizeStructure::custom(1000, vec![0.5, 0.3, 0.2]);
        assert_eq!(prize.total_pool, 1000);
        assert_eq!(prize.payouts, vec![500, 300, 200]);
    }

    #[test]
    fn test_payout_for_position() {
        let prize = PrizeStructure::standard(10, 100);
        assert_eq!(prize.payout_for_position(1), Some(500));
        assert_eq!(prize.payout_for_position(2), Some(300));
        assert_eq!(prize.payout_for_position(3), Some(200));
        assert_eq!(prize.payout_for_position(4), None);
        assert_eq!(prize.payout_for_position(0), None);
    }

    #[test]
    fn test_sit_and_go_config() {
        let config = TournamentConfig::sit_and_go("Test SNG".to_string(), 9, 100);
        assert_eq!(config.tournament_type, TournamentType::SitAndGo);
        assert_eq!(config.buy_in, 100);
        assert_eq!(config.max_players, 9);
        assert_eq!(config.starting_stack, 5000); // 50x buy-in
        assert_eq!(config.blind_levels.len(), 10);
        assert_eq!(config.starting_level, 1);
    }

    #[test]
    fn test_turbo_sit_and_go_config() {
        let config = TournamentConfig::turbo_sit_and_go("Turbo SNG".to_string(), 6, 50);
        assert_eq!(config.buy_in, 50);
        assert!(config.blind_levels.iter().all(|bl| bl.duration_secs == 180));
    }

    #[test]
    fn test_get_blind_level() {
        let config = TournamentConfig::sit_and_go("Test".to_string(), 9, 100);
        let level_1 = config.get_blind_level(1);
        assert!(level_1.is_some());
        assert_eq!(level_1.unwrap().small_blind, 10);
        assert_eq!(level_1.unwrap().big_blind, 20);

        let level_99 = config.get_blind_level(99);
        assert!(level_99.is_none());
    }

    #[test]
    fn test_blind_level_with_ante() {
        let level = BlindLevel::new(5, 100, 200, 300).with_ante(25);
        assert_eq!(level.ante, Some(25));
    }
}
