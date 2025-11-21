//! Bot decision-making logic with difficulty-based behavior.

use super::models::{BotPlayer, DifficultyParams};
use crate::game::entities::Action;
use rand::Rng;

/// Bot decision maker
pub struct BotDecisionMaker {
    /// Random number generator
    rng: rand::rngs::ThreadRng,
}

impl BotDecisionMaker {
    /// Create a new decision maker
    pub fn new() -> Self {
        Self { rng: rand::rng() }
    }

    /// Decide bot action based on difficulty and game state
    ///
    /// # Arguments
    ///
    /// * `bot` - Bot player
    /// * `hole_cards` - Bot's hole cards
    /// * `board_cards` - Community cards
    /// * `pot_size` - Current pot size
    /// * `current_bet` - Current bet to call
    /// * `bot_chips` - Bot's current chip count
    /// * `can_check` - Whether bot can check
    /// * `position` - Optional position (0=button, 1=SB, 2=BB, etc.). None defaults to middle position.
    /// * `players_remaining` - Number of players still in the hand (for pot odds calculation)
    ///
    /// # Returns
    ///
    /// * `Action` - Bot's chosen action
    #[allow(clippy::too_many_arguments)]
    pub fn decide_action(
        &mut self,
        bot: &BotPlayer,
        hole_cards: &[crate::game::entities::Card],
        board_cards: &[crate::game::entities::Card],
        pot_size: u32,
        current_bet: u32,
        bot_chips: u32,
        can_check: bool,
        position: Option<usize>,
        players_remaining: usize,
    ) -> Action {
        let params = &bot.params;

        // Estimate hand strength
        let mut hand_strength = self.estimate_hand_strength(hole_cards, board_cards);

        // Apply position modifier (late position can play slightly weaker hands)
        let position_modifier = self.calculate_position_modifier(position, players_remaining);
        hand_strength = (hand_strength + position_modifier).clamp(0.0, 1.0);

        // All-in if critically short-stacked
        if bot_chips <= current_bet {
            return Action::AllIn;
        }

        // Calculate pot odds if there's a bet to call
        let pot_odds = if current_bet > 0 && !can_check {
            self.calculate_pot_odds(pot_size, current_bet)
        } else {
            0.0 // No odds calculation needed if we can check for free
        };

        // Decision logic based on hand strength and difficulty
        // Easy bots: Play weak hands, passive
        // Standard bots: Balanced, semi-aggressive
        // TAG bots: Tight (only strong hands), very aggressive

        // Adjust thresholds based on difficulty (VPIP determines how selective)
        let (fold_threshold, raise_threshold) = match params.vpip {
            v if v > 0.40 => (0.08, 0.20), // Easy: plays almost anything, raises at 0.20+
            v if v > 0.25 => (0.12, 0.28), // Standard: selective, raises at 0.28+
            _ => (0.18, 0.33),             // TAG: very selective, raises at 0.33+ (high pairs)
        };

        // Fold weak hands unless can check for free
        if hand_strength < fold_threshold {
            if can_check {
                return Action::Check;
            }
            // Sometimes bluff with weak hands
            if params.bluffs && self.rng.random_bool(params.bluff_frequency as f64) {
                let bluff_size = (pot_size as f32 * 1.5) as u32;
                return if bot_chips <= bluff_size {
                    Action::AllIn
                } else {
                    Action::Raise(Some(bluff_size))
                };
            }
            return Action::Fold;
        }

        // Medium strength hands: play based on aggression and pot odds
        if hand_strength < raise_threshold {
            if can_check {
                return Action::Check;
            }

            // Use pot odds to inform decision
            // If pot odds are good (high odds), more likely to call even with medium hands
            let pot_odds_bonus = if pot_odds > 0.25 { 0.2 } else { 0.0 };

            // Call or fold based on aggression (aggressive bots call more)
            let call_probability = 0.3 + (params.aggression_factor / 5.0) + pot_odds_bonus; // 0.4 to 1.1
            if self.rng.random_bool(call_probability.min(1.0) as f64) {
                return Action::Call;
            }
            return Action::Fold;
        }

        // Strong hands (>= raise_threshold): raise aggressively
        let raise_probability = 0.4 + (params.aggression_factor / 4.0); // 0.525 to 1.15 (clamped to 1.0)
        if self.rng.random_bool(raise_probability.min(1.0) as f64) {
            let raise_amount =
                self.calculate_raise_amount(params, pot_size, current_bet, bot_chips);
            if bot_chips <= raise_amount {
                Action::AllIn
            } else {
                Action::Raise(Some(raise_amount))
            }
        } else if can_check {
            // Slow-play strong hands occasionally
            Action::Check
        } else {
            Action::Call
        }
    }

    /// Decide whether to play a hand pre-flop based on VPIP
    ///
    /// # Arguments
    ///
    /// * `params` - Difficulty parameters
    /// * `hand_strength` - Estimated hand strength (0.0 to 1.0)
    ///
    /// # Returns
    ///
    /// * `bool` - Whether to play the hand
    pub fn should_play_hand(&mut self, params: &DifficultyParams, hand_strength: f32) -> bool {
        // Higher VPIP = plays weaker hands
        let threshold = 1.0 - params.vpip;
        hand_strength >= threshold
    }

    /// Decide whether to raise pre-flop based on PFR
    ///
    /// # Arguments
    ///
    /// * `params` - Difficulty parameters
    /// * `hand_strength` - Estimated hand strength (0.0 to 1.0)
    ///
    /// # Returns
    ///
    /// * `bool` - Whether to raise
    pub fn should_raise_preflop(&mut self, params: &DifficultyParams, hand_strength: f32) -> bool {
        // PFR threshold is higher than VPIP threshold
        let threshold = 1.0 - params.pfr;
        hand_strength >= threshold && self.rng.random_bool(params.pfr as f64)
    }

    /// Decide whether to make continuation bet
    ///
    /// # Arguments
    ///
    /// * `params` - Difficulty parameters
    ///
    /// # Returns
    ///
    /// * `bool` - Whether to c-bet
    pub fn should_cbet(&mut self, params: &DifficultyParams) -> bool {
        self.rng.random_bool(params.cbet_frequency as f64)
    }

    /// Calculate raise amount based on pot size and difficulty
    fn calculate_raise_amount(
        &mut self,
        params: &DifficultyParams,
        pot_size: u32,
        current_bet: u32,
        bot_chips: u32,
    ) -> u32 {
        // Raise size varies by aggression
        let base_multiplier = match params.aggression_factor {
            x if x < 1.0 => 2.0, // Passive: small raises
            x if x < 2.0 => 2.5, // Moderate: standard raises
            _ => 3.0,            // Aggressive: large raises
        };

        // Add randomness (±20%)
        let variance = self.rng.random_range(-0.2..=0.2);
        let multiplier = base_multiplier * (1.0 + variance);

        let raise_amount = ((pot_size + current_bet) as f32 * multiplier) as u32;

        // Cap at bot's chips
        raise_amount.min(bot_chips)
    }

    /// Calculate pot odds (ratio of pot size to call amount)
    ///
    /// # Arguments
    ///
    /// * `pot_size` - Current pot size
    /// * `call_amount` - Amount needed to call
    ///
    /// # Returns
    ///
    /// * `f32` - Pot odds as a ratio (e.g., 0.33 = 3:1 odds)
    fn calculate_pot_odds(&self, pot_size: u32, call_amount: u32) -> f32 {
        if call_amount == 0 {
            return 1.0; // Free to see next card
        }

        // Pot odds = pot / (pot + call)
        // Example: $100 pot, $50 to call = 100 / (100 + 50) = 0.67 (1.5:1)
        let total_pot = pot_size + call_amount;
        pot_size as f32 / total_pot as f32
    }

    /// Calculate position modifier for hand strength
    ///
    /// Players in late position can play slightly weaker hands because they have
    /// more information (seeing how other players acted).
    ///
    /// # Arguments
    ///
    /// * `position` - Button=0, SB=1, BB=2, UTG=3, MP=4, CO=5, etc.
    /// * `players_remaining` - Number of players still in hand
    ///
    /// # Returns
    ///
    /// * `f32` - Position bonus to add to hand strength (0.0 to 0.08)
    fn calculate_position_modifier(
        &self,
        position: Option<usize>,
        players_remaining: usize,
    ) -> f32 {
        let pos = position.unwrap_or(players_remaining / 2); // Default to middle position

        // Late position (button, cutoff) gets bonus
        // Early position (UTG, UTG+1) gets penalty
        // Relative position matters more in larger games
        if players_remaining <= 2 {
            return 0.0; // Heads-up, position less critical
        }

        // Button (0) gets max bonus, progresses toward early position
        let relative_pos = pos as f32 / players_remaining as f32;

        match relative_pos {
            x if x < 0.2 => 0.08,  // Button, cutoff: +0.08 (can play J-9s, A-5s)
            x if x < 0.4 => 0.04,  // Middle position: +0.04
            x if x < 0.6 => 0.0,   // Early-middle: neutral
            x if x < 0.8 => -0.03, // Early position: -0.03 (fold marginal hands)
            _ => -0.05,            // UTG (under the gun): -0.05 (very tight)
        }
    }

    /// Estimate hand strength based on hole cards and board
    ///
    /// # Arguments
    ///
    /// * `hole_cards` - Bot's hole cards
    /// * `board_cards` - Community cards
    ///
    /// # Returns
    ///
    /// * `f32` - Estimated strength (0.0 to 1.0)
    pub fn estimate_hand_strength(
        &self,
        hole_cards: &[crate::game::entities::Card],
        board_cards: &[crate::game::entities::Card],
    ) -> f32 {
        use crate::game::{entities::Rank, functional::eval};

        // Combine hole cards and board cards
        let mut all_cards = Vec::with_capacity(hole_cards.len() + board_cards.len());
        all_cards.extend_from_slice(hole_cards);
        all_cards.extend_from_slice(board_cards);

        // Need at least 2 cards to evaluate
        if all_cards.len() < 2 {
            return 0.0;
        }

        // Evaluate hand
        let hand = eval(&all_cards);

        if hand.is_empty() {
            return 0.0;
        }

        // Base strength on hand rank
        let base_strength = match hand[0].rank {
            Rank::HighCard => 0.1,
            Rank::OnePair => 0.25,
            Rank::TwoPair => 0.40,
            Rank::ThreeOfAKind => 0.55,
            Rank::Straight => 0.70,
            Rank::Flush => 0.75,
            Rank::FullHouse => 0.85,
            Rank::FourOfAKind => 0.95,
            Rank::StraightFlush => 0.99,
        };

        // Adjust for kickers (higher values = stronger hand within same rank)
        let kicker_bonus = if !hand[0].values.is_empty() {
            let max_value = *hand[0].values.iter().max().unwrap_or(&0);
            // Normalize value (1-14) to 0.0-0.1 range for kicker bonus
            (max_value as f32 / 14.0) * 0.1
        } else {
            0.0
        };

        // Clamp final strength to 0.0-1.0
        (base_strength + kicker_bonus).min(1.0)
    }
}

impl Default for BotDecisionMaker {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bot::models::BotConfig;
    use crate::game::entities::{Card, Suit};
    use crate::table::config::BotDifficulty;

    #[test]
    fn test_easy_bot_is_passive() {
        let mut decision_maker = BotDecisionMaker::new();

        // Strong hand: pocket aces
        let hole_cards = vec![Card(14, Suit::Spade), Card(14, Suit::Heart)];
        let board_cards = vec![];

        // Easy bot should be less aggressive even with strong hands
        let mut raise_count = 0;
        let trials = 100;
        for _ in 0..trials {
            let action = decision_maker.decide_action(
                &create_test_bot(BotDifficulty::Easy),
                &hole_cards,
                &board_cards,
                100,
                10,
                1000,
                false,
                Some(0), // Button position
                6,       // 6 players
            );
            if matches!(action, Action::Raise(_) | Action::AllIn) {
                raise_count += 1;
            }
        }

        // Easy bots are passive, so even with AA they should raise < 70% of the time
        let max_raises = (trials as f32 * 0.70) as usize;
        assert!(
            raise_count < max_raises,
            "Easy bot raised {} times out of {} (should be < {})",
            raise_count,
            trials,
            max_raises
        );
    }

    #[test]
    fn test_tag_bot_is_aggressive() {
        let mut decision_maker = BotDecisionMaker::new();

        // Strong hand: pocket kings
        let hole_cards = vec![Card(13, Suit::Spade), Card(13, Suit::Heart)];
        let board_cards = vec![];

        // TAG bot should raise frequently with strong hands
        let mut raise_count = 0;
        let trials = 100;
        for _ in 0..trials {
            let action = decision_maker.decide_action(
                &create_test_bot(BotDifficulty::Tag),
                &hole_cards,
                &board_cards,
                100,
                10,
                1000,
                false,
                Some(0), // Button position
                6,       // 6 players
            );
            if matches!(action, Action::Raise(_) | Action::AllIn) {
                raise_count += 1;
            }
        }

        // TAG bots are aggressive with strong hands - should raise > 70% of the time
        assert!(
            raise_count > 70,
            "TAG bot raised {} times out of {} (should be > 70)",
            raise_count,
            trials
        );
    }

    #[test]
    fn test_tag_bot_folds_weak_hands() {
        let mut decision_maker = BotDecisionMaker::new();

        // Weak hand: 7-2 offsuit
        let hole_cards = vec![Card(7, Suit::Club), Card(2, Suit::Diamond)];
        let board_cards = vec![];

        // TAG bot should fold weak hands most of the time
        let mut fold_count = 0;
        let trials = 500; // Increased for statistical reliability
        for _ in 0..trials {
            let action = decision_maker.decide_action(
                &create_test_bot(BotDifficulty::Tag),
                &hole_cards,
                &board_cards,
                100,
                10,
                1000,
                false,   // can't check
                Some(5), // UTG (early position) - no bonus for weak hands
                6,       // 6 players
            );
            if matches!(action, Action::Fold) {
                fold_count += 1;
            }
        }

        // TAG is tight - should fold weak hands > 60% from early position
        // With larger sample size (500), expecting at least 60% fold rate for 7-2o from UTG
        assert!(
            fold_count > 300,
            "TAG bot folded {} times out of {} (should be > 300/500 = 60% from UTG with 7-2o)",
            fold_count,
            trials
        );
    }

    #[test]
    fn test_easy_bot_plays_weak_hands() {
        let mut decision_maker = BotDecisionMaker::new();

        // Weak hand: 9-3 offsuit
        let hole_cards = vec![Card(9, Suit::Heart), Card(3, Suit::Club)];
        let board_cards = vec![];

        // Easy bot should NOT fold weak hands as often (loose play)
        let mut fold_count = 0;
        let trials = 100;
        for _ in 0..trials {
            let action = decision_maker.decide_action(
                &create_test_bot(BotDifficulty::Easy),
                &hole_cards,
                &board_cards,
                100,
                10,
                1000,
                false,   // can't check
                Some(0), // Button position
                6,       // 6 players
            );
            if matches!(action, Action::Fold) {
                fold_count += 1;
            }
        }

        // Easy bots are loose - should fold < 70% even with weak hands
        assert!(
            fold_count < 70,
            "Easy bot folded {} times out of {} (should be < 70)",
            fold_count,
            trials
        );
    }

    // Note: This test uses large sample sizes to verify position awareness statistically.
    // The position awareness logic is implemented and working, verified through
    // statistical aggregation over many trials.
    #[test]
    fn test_position_awareness() {
        let mut decision_maker = BotDecisionMaker::new();

        // Marginal hand: 8-6 suited (borderline playable from button, fold from UTG)
        let hole_cards = vec![Card(8, Suit::Spade), Card(6, Suit::Spade)];
        let board_cards = vec![];

        // From button (late position): should play sometimes
        let mut plays_button = 0;
        for _ in 0..1000 {
            let action = decision_maker.decide_action(
                &create_test_bot(BotDifficulty::Standard),
                &hole_cards,
                &board_cards,
                100,
                20,
                1000,
                false,
                Some(0), // Button
                6,
            );
            if !matches!(action, Action::Fold) {
                plays_button += 1;
            }
        }

        // From UTG (early position): should fold almost always
        let mut plays_utg = 0;
        for _ in 0..1000 {
            let action = decision_maker.decide_action(
                &create_test_bot(BotDifficulty::Standard),
                &hole_cards,
                &board_cards,
                100,
                20,
                1000,
                false,
                Some(5), // UTG (early position)
                6,
            );
            if !matches!(action, Action::Fold) {
                plays_utg += 1;
            }
        }

        // Should play at least 5% more from button than UTG (50 out of 1000 trials)
        // With larger sample size, this is statistically reliable
        assert!(
            plays_button > plays_utg + 50,
            "Button played {} times vs UTG {} times (button should play significantly more with marginal hands)",
            plays_button,
            plays_utg
        );
    }

    // Deterministic test for pot odds calculation
    #[test]
    fn test_pot_odds_calculation() {
        let decision_maker = BotDecisionMaker::new();

        // Test various pot odds scenarios
        // Good pot odds: $100 pot, $20 to call = 100/120 = 0.833 (5:1)
        let odds1 = decision_maker.calculate_pot_odds(100, 20);
        assert!((odds1 - 0.833).abs() < 0.01, "Should calculate 0.833 for 100/20");

        // Medium pot odds: $100 pot, $50 to call = 100/150 = 0.667 (2:1)
        let odds2 = decision_maker.calculate_pot_odds(100, 50);
        assert!((odds2 - 0.667).abs() < 0.01, "Should calculate 0.667 for 100/50");

        // Bad pot odds: $50 pot, $100 to call = 50/150 = 0.333 (0.5:1)
        let odds3 = decision_maker.calculate_pot_odds(50, 100);
        assert!((odds3 - 0.333).abs() < 0.01, "Should calculate 0.333 for 50/100");

        // Free to call: $100 pot, $0 to call = 1.0 (infinite odds)
        let odds4 = decision_maker.calculate_pot_odds(100, 0);
        assert_eq!(odds4, 1.0, "Should return 1.0 for free call");

        // Verify ordering: good odds > medium odds > bad odds
        assert!(odds1 > odds2, "Good odds should be > medium odds");
        assert!(odds2 > odds3, "Medium odds should be > bad odds");
    }

    fn create_test_bot(difficulty: BotDifficulty) -> BotPlayer {
        let config = BotConfig {
            id: 1,
            name: "TestBot".to_string(),
            difficulty,
            table_id: 1,
            starting_chips: 1000,
        };
        BotPlayer::new(config)
    }
}
