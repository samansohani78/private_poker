//! Bot decision-making logic with difficulty-based behavior.

use super::models::{BotPlayer, DifficultyParams};
use crate::game::entities::{Action, Card};
use rand::Rng;

// === Hand Strength Base Values ===
// These represent the baseline strength for each poker hand rank

/// Hand strength for high card (weakest hand: 10%)
const STRENGTH_HIGH_CARD: f32 = 0.1;

/// Hand strength for one pair (25% = beats high card)
const STRENGTH_ONE_PAIR: f32 = 0.25;

/// Hand strength for two pair (40%)
const STRENGTH_TWO_PAIR: f32 = 0.40;

/// Hand strength for three of a kind (55%)
const STRENGTH_THREE_OF_A_KIND: f32 = 0.55;

/// Hand strength for straight (70%)
const STRENGTH_STRAIGHT: f32 = 0.70;

/// Hand strength for flush (75%)
const STRENGTH_FLUSH: f32 = 0.75;

/// Hand strength for full house (85%)
const STRENGTH_FULL_HOUSE: f32 = 0.85;

/// Hand strength for four of a kind (95%)
const STRENGTH_FOUR_OF_A_KIND: f32 = 0.95;

/// Hand strength for straight flush (99% = nearly unbeatable)
const STRENGTH_STRAIGHT_FLUSH: f32 = 0.99;

/// Configuration for bot decision-making thresholds and multipliers.
///
/// All threshold values are hand strength floats in range [0.0, 1.0].
/// Higher threshold = more conservative (tighter play).
///
/// # Pre-flop Thresholds
///
/// These determine when bots act based on raw hand strength:
/// - Easy bots play loose (fold < 8%, raise > 20%)
/// - Standard bots play balanced (fold < 12%, raise > 28%)
/// - TAG bots play tight (fold < 18%, raise > 33%)
///
/// # Examples
///
/// ```
/// use private_poker::bot::decision::BotDecisionConfig;
///
/// let config = BotDecisionConfig::default();
/// assert_eq!(config.easy_fold_threshold, 0.08); // Folds bottom 8%
/// assert_eq!(config.easy_raise_threshold, 0.20); // Raises top 20%
/// ```
#[derive(Debug, Clone)]
pub struct BotDecisionConfig {
    /// Hand strength below this = fold (Easy bot, pre-flop).
    ///
    /// **Range**: 0.05-0.15 (typical: 0.08)
    /// **Effect**: 0.08 = folds 92% of hands, plays very loose
    /// **Lower** = looser play (calls more weak hands)
    pub easy_fold_threshold: f32,

    /// Hand strength above this = raise (Easy bot, pre-flop).
    ///
    /// **Range**: 0.15-0.30 (typical: 0.20)
    /// **Effect**: 0.20 = raises top 20% of hands
    /// **Higher** = tighter play (only raises premium hands)
    pub easy_raise_threshold: f32,

    /// Hand strength below this = fold (Standard bot, pre-flop).
    ///
    /// **Range**: 0.10-0.18 (typical: 0.12)
    /// **Effect**: 0.12 = folds 88% of hands, balanced TAG play
    pub standard_fold_threshold: f32,

    /// Hand strength above this = raise (Standard bot, pre-flop).
    ///
    /// **Range**: 0.25-0.35 (typical: 0.28)
    /// **Effect**: 0.28 = raises top 28% of hands
    pub standard_raise_threshold: f32,

    /// Hand strength below this = fold (TAG bot, pre-flop).
    ///
    /// **Range**: 0.15-0.25 (typical: 0.18)
    /// **Effect**: 0.18 = folds 82% of hands, very tight play
    pub tag_fold_threshold: f32,

    /// Hand strength above this = raise (TAG bot, pre-flop).
    ///
    /// **Range**: 0.30-0.40 (typical: 0.33)
    /// **Effect**: 0.33 = raises top 33% of hands
    pub tag_raise_threshold: f32,

    /// Bluff size as a multiplier of the current pot.
    ///
    /// **Range**: 1.0-3.0 (typical: 1.5)
    /// **Effect**: 1.5 = bets 150% of pot when bluffing
    /// **Higher** = more intimidating bluffs (expensive for opponents)
    pub bluff_size_multiplier: f32,

    /// Pot odds threshold for receiving bonus to hand strength.
    ///
    /// **Range**: 0.2-0.4 (typical: 0.25)
    /// **Effect**: If pot odds > 0.25 (4:1), add bonus to hand strength
    /// **Lower** = more willing to chase draws
    pub pot_odds_bonus_threshold: f32,

    /// Bonus value added to hand strength when pot odds are good.
    ///
    /// **Range**: 0.1-0.3 (typical: 0.2)
    /// **Effect**: Adds 20% to hand strength when pot odds favorable
    /// **Higher** = more aggressive draw chasing
    pub pot_odds_bonus_value: f32,

    /// Base probability of calling with medium-strength hands.
    ///
    /// **Range**: 0.2-0.5 (typical: 0.3)
    /// **Effect**: 30% base chance to call (modified by aggression)
    /// **Higher** = more calling stations (passive play)
    pub base_call_probability: f32,

    /// Divisor for aggression factor when calculating call probability.
    ///
    /// **Range**: 3.0-7.0 (typical: 5.0)
    /// **Effect**: call_prob = base + (aggression / divisor)
    /// **Lower** = aggression has more impact on calling
    pub call_aggression_divisor: f32,

    /// Base probability of raising with strong hands.
    ///
    /// **Range**: 0.3-0.6 (typical: 0.4)
    /// **Effect**: 40% base chance to raise (modified by aggression)
    /// **Higher** = more aggressive betting
    pub base_raise_probability: f32,

    /// Divisor for aggression factor when calculating raise probability.
    ///
    /// **Range**: 2.0-6.0 (typical: 4.0)
    /// **Effect**: raise_prob = base + (aggression / divisor)
    /// **Lower** = aggression has more impact on raising
    pub raise_aggression_divisor: f32,

    /// Raise multiplier for passive bots (aggression < 1.0).
    ///
    /// **Range**: 1.5-3.0 (typical: 2.0)
    /// **Effect**: Passive bots raise 2x the big blind
    /// **Higher** = larger raises even with low aggression
    pub passive_raise_multiplier: f32,

    /// Raise multiplier for moderate bots (aggression 1.0-2.0).
    ///
    /// **Range**: 2.0-3.5 (typical: 2.5)
    /// **Effect**: Moderate bots raise 2.5x the big blind
    pub moderate_raise_multiplier: f32,

    /// Raise multiplier for aggressive bots (aggression > 2.0).
    ///
    /// **Range**: 2.5-4.0 (typical: 3.0)
    /// **Effect**: Aggressive bots raise 3x the big blind
    /// **Higher** = larger raises, more intimidating
    pub aggressive_raise_multiplier: f32,

    /// Variance range for raise sizing (±percentage).
    ///
    /// **Range**: 0.1-0.3 (typical: 0.2)
    /// **Effect**: Raise amount varies by ±20% randomly
    /// **Higher** = more unpredictable bet sizing
    pub raise_variance: f32,

    /// Bonus to hand strength when in late position (button/cutoff).
    ///
    /// **Range**: 0.05-0.12 (typical: 0.08)
    /// **Effect**: Adds 8% to hand strength in late position
    /// **Higher** = plays looser on the button
    pub late_position_bonus: f32,

    /// Bonus to hand strength when in middle position.
    ///
    /// **Range**: 0.02-0.06 (typical: 0.04)
    /// **Effect**: Adds 4% to hand strength in middle position
    pub middle_position_bonus: f32,

    /// Penalty to hand strength when in early-middle position.
    ///
    /// **Range**: -0.05 to -0.01 (typical: -0.03)
    /// **Effect**: Reduces hand strength by 3% in early-middle position
    /// **More negative** = plays tighter from early position
    pub early_middle_position_penalty: f32,

    /// Penalty to hand strength when in UTG (under the gun) position.
    ///
    /// **Range**: -0.08 to -0.03 (typical: -0.05)
    /// **Effect**: Reduces hand strength by 5% from UTG
    /// **More negative** = plays very tight from earliest position
    pub utg_position_penalty: f32,
}

impl Default for BotDecisionConfig {
    fn default() -> Self {
        Self {
            // Fold/raise thresholds by difficulty
            easy_fold_threshold: 0.08,
            easy_raise_threshold: 0.20,
            standard_fold_threshold: 0.12,
            standard_raise_threshold: 0.28,
            tag_fold_threshold: 0.18,
            tag_raise_threshold: 0.33,
            // Bluffing
            bluff_size_multiplier: 1.5,
            // Pot odds
            pot_odds_bonus_threshold: 0.25,
            pot_odds_bonus_value: 0.2,
            // Calling
            base_call_probability: 0.3,
            call_aggression_divisor: 5.0,
            // Raising
            base_raise_probability: 0.4,
            raise_aggression_divisor: 4.0,
            // Raise sizing
            passive_raise_multiplier: 2.0,
            moderate_raise_multiplier: 2.5,
            aggressive_raise_multiplier: 3.0,
            raise_variance: 0.2,
            // Position adjustments
            late_position_bonus: 0.08,
            middle_position_bonus: 0.04,
            early_middle_position_penalty: -0.03,
            utg_position_penalty: -0.05,
        }
    }
}

/// Context for bot decision making
///
/// Encapsulates all information needed to make a poker decision,
/// reducing parameter count and improving testability.
#[derive(Debug, Clone)]
pub struct BotDecisionContext<'a> {
    /// Bot's hole cards
    pub hole_cards: &'a [Card],

    /// Community board cards
    pub board_cards: &'a [Card],

    /// Current pot size
    pub pot_size: u32,

    /// Current bet amount to call
    pub current_bet: u32,

    /// Bot's remaining chips
    pub bot_chips: u32,

    /// Whether the bot can check (no bet to call)
    pub can_check: bool,

    /// Bot's position (0=button, 1=SB, 2=BB, etc.)
    pub position: Option<usize>,

    /// Number of players still in the hand
    pub players_remaining: usize,
}

/// Bot decision maker
pub struct BotDecisionMaker {
    /// Random number generator
    rng: rand::rngs::ThreadRng,
    /// Configuration for decision-making
    config: BotDecisionConfig,
}

impl BotDecisionMaker {
    /// Create a new decision maker with default config
    pub fn new() -> Self {
        Self {
            rng: rand::rng(),
            config: BotDecisionConfig::default(),
        }
    }

    /// Create a new decision maker with custom config
    pub fn with_config(config: BotDecisionConfig) -> Self {
        Self {
            rng: rand::rng(),
            config,
        }
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
    /// * `ctx` - Decision context with game state
    ///
    /// # Returns
    ///
    /// * `Action` - Bot's chosen action
    pub fn decide_action(&mut self, bot: &BotPlayer, ctx: &BotDecisionContext) -> Action {
        let params = &bot.params;

        // Estimate hand strength
        let mut hand_strength = self.estimate_hand_strength(ctx.hole_cards, ctx.board_cards);

        // Apply position modifier (late position can play slightly weaker hands)
        let position_modifier =
            self.calculate_position_modifier(ctx.position, ctx.players_remaining);
        hand_strength = (hand_strength + position_modifier).clamp(0.0, 1.0);

        // All-in if critically short-stacked
        if ctx.bot_chips <= ctx.current_bet {
            return Action::AllIn;
        }

        // Calculate pot odds if there's a bet to call
        let pot_odds = if ctx.current_bet > 0 && !ctx.can_check {
            self.calculate_pot_odds(ctx.pot_size, ctx.current_bet)
        } else {
            0.0 // No odds calculation needed if we can check for free
        };

        // Decision logic based on hand strength and difficulty
        // Easy bots: Play weak hands, passive
        // Standard bots: Balanced, semi-aggressive
        // TAG bots: Tight (only strong hands), very aggressive

        // Adjust thresholds based on difficulty (VPIP determines how selective)
        let (fold_threshold, raise_threshold) = match params.vpip {
            v if v > 0.40 => (
                self.config.easy_fold_threshold,
                self.config.easy_raise_threshold,
            ),
            v if v > 0.25 => (
                self.config.standard_fold_threshold,
                self.config.standard_raise_threshold,
            ),
            _ => (
                self.config.tag_fold_threshold,
                self.config.tag_raise_threshold,
            ),
        };

        // Fold weak hands unless can check for free
        if hand_strength < fold_threshold {
            if ctx.can_check {
                return Action::Check;
            }
            // Sometimes bluff with weak hands
            if params.bluffs && self.rng.random_bool(params.bluff_frequency as f64) {
                let bluff_size = (ctx.pot_size as f32 * self.config.bluff_size_multiplier) as u32;
                return if ctx.bot_chips <= bluff_size {
                    Action::AllIn
                } else {
                    Action::Raise(Some(bluff_size))
                };
            }
            return Action::Fold;
        }

        // Medium strength hands: play based on aggression and pot odds
        if hand_strength < raise_threshold {
            if ctx.can_check {
                return Action::Check;
            }

            // Use pot odds to inform decision
            // If pot odds are good (high odds), more likely to call even with medium hands
            let pot_odds_bonus = if pot_odds > self.config.pot_odds_bonus_threshold {
                self.config.pot_odds_bonus_value
            } else {
                0.0
            };

            // Call or fold based on aggression (aggressive bots call more)
            let call_probability = self.config.base_call_probability
                + (params.aggression_factor / self.config.call_aggression_divisor)
                + pot_odds_bonus;
            if self.rng.random_bool(call_probability.min(1.0) as f64) {
                return Action::Call;
            }
            return Action::Fold;
        }

        // Strong hands (>= raise_threshold): raise aggressively
        let raise_probability = self.config.base_raise_probability
            + (params.aggression_factor / self.config.raise_aggression_divisor);
        if self.rng.random_bool(raise_probability.min(1.0) as f64) {
            let raise_amount =
                self.calculate_raise_amount(params, ctx.pot_size, ctx.current_bet, ctx.bot_chips);
            if ctx.bot_chips <= raise_amount {
                Action::AllIn
            } else {
                Action::Raise(Some(raise_amount))
            }
        } else if ctx.can_check {
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
            x if x < 1.0 => self.config.passive_raise_multiplier,
            x if x < 2.0 => self.config.moderate_raise_multiplier,
            _ => self.config.aggressive_raise_multiplier,
        };

        // Add randomness (±variance%)
        let variance = self
            .rng
            .random_range(-self.config.raise_variance..=self.config.raise_variance);
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

        // Defensive check: prevent division by zero
        if players_remaining == 0 {
            log::warn!("players_remaining is 0, returning neutral position adjustment");
            return 0.0; // Neutral adjustment
        }

        // Button (0) gets max bonus, progresses toward early position
        let relative_pos = pos as f32 / players_remaining as f32;

        match relative_pos {
            x if x < 0.2 => self.config.late_position_bonus,
            x if x < 0.4 => self.config.middle_position_bonus,
            x if x < 0.6 => 0.0, // Early-middle: neutral
            x if x < 0.8 => self.config.early_middle_position_penalty,
            _ => self.config.utg_position_penalty,
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
            Rank::HighCard => STRENGTH_HIGH_CARD,
            Rank::OnePair => STRENGTH_ONE_PAIR,
            Rank::TwoPair => STRENGTH_TWO_PAIR,
            Rank::ThreeOfAKind => STRENGTH_THREE_OF_A_KIND,
            Rank::Straight => STRENGTH_STRAIGHT,
            Rank::Flush => STRENGTH_FLUSH,
            Rank::FullHouse => STRENGTH_FULL_HOUSE,
            Rank::FourOfAKind => STRENGTH_FOUR_OF_A_KIND,
            Rank::StraightFlush => STRENGTH_STRAIGHT_FLUSH,
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

    // Helper to create a test context
    #[allow(clippy::too_many_arguments)]
    fn make_ctx<'a>(
        hole_cards: &'a [Card],
        board_cards: &'a [Card],
        pot_size: u32,
        current_bet: u32,
        bot_chips: u32,
        can_check: bool,
        position: Option<usize>,
        players_remaining: usize,
    ) -> BotDecisionContext<'a> {
        BotDecisionContext {
            hole_cards,
            board_cards,
            pot_size,
            current_bet,
            bot_chips,
            can_check,
            position,
            players_remaining,
        }
    }

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
            let ctx = make_ctx(&hole_cards, &board_cards, 100, 10, 1000, false, Some(0), 6);
            let action = decision_maker.decide_action(&create_test_bot(BotDifficulty::Easy), &ctx);
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
            let ctx = make_ctx(&hole_cards, &board_cards, 100, 10, 1000, false, Some(0), 6);
            let action = decision_maker.decide_action(&create_test_bot(BotDifficulty::Tag), &ctx);
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
            let ctx = make_ctx(&hole_cards, &board_cards, 100, 10, 1000, false, Some(5), 6);
            let action = decision_maker.decide_action(&create_test_bot(BotDifficulty::Tag), &ctx);
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
            let ctx = make_ctx(&hole_cards, &board_cards, 100, 10, 1000, false, Some(0), 6);
            let action = decision_maker.decide_action(&create_test_bot(BotDifficulty::Easy), &ctx);
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
            let ctx = make_ctx(&hole_cards, &board_cards, 100, 20, 1000, false, Some(0), 6);
            let action =
                decision_maker.decide_action(&create_test_bot(BotDifficulty::Standard), &ctx);
            if !matches!(action, Action::Fold) {
                plays_button += 1;
            }
        }

        // From UTG (early position): should fold almost always
        let mut plays_utg = 0;
        for _ in 0..1000 {
            let ctx = make_ctx(&hole_cards, &board_cards, 100, 20, 1000, false, Some(5), 6);
            let action =
                decision_maker.decide_action(&create_test_bot(BotDifficulty::Standard), &ctx);
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
        assert!(
            (odds1 - 0.833).abs() < 0.01,
            "Should calculate 0.833 for 100/20"
        );

        // Medium pot odds: $100 pot, $50 to call = 100/150 = 0.667 (2:1)
        let odds2 = decision_maker.calculate_pot_odds(100, 50);
        assert!(
            (odds2 - 0.667).abs() < 0.01,
            "Should calculate 0.667 for 100/50"
        );

        // Bad pot odds: $50 pot, $100 to call = 50/150 = 0.333 (0.5:1)
        let odds3 = decision_maker.calculate_pot_odds(50, 100);
        assert!(
            (odds3 - 0.333).abs() < 0.01,
            "Should calculate 0.333 for 50/100"
        );

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
