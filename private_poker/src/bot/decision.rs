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
        Self {
            rng: rand::thread_rng(),
        }
    }

    /// Decide bot action based on difficulty and game state
    ///
    /// # Arguments
    ///
    /// * `bot` - Bot player
    /// * `pot_size` - Current pot size
    /// * `current_bet` - Current bet to call
    /// * `bot_chips` - Bot's current chip count
    /// * `can_check` - Whether bot can check
    ///
    /// # Returns
    ///
    /// * `Action` - Bot's chosen action
    pub fn decide_action(
        &mut self,
        bot: &BotPlayer,
        pot_size: u32,
        current_bet: u32,
        bot_chips: u32,
        can_check: bool,
    ) -> Action {
        let params = &bot.params;

        // If can check, sometimes take free card based on passiveness
        if can_check && self.should_check(params) {
            return Action::Check;
        }

        // All-in if critically short-stacked
        if bot_chips <= current_bet {
            return Action::AllIn;
        }

        // Decide based on difficulty parameters
        let action_type = self.decide_action_type(params, pot_size, current_bet);

        match action_type {
            ActionType::Fold => Action::Fold,
            ActionType::Call => {
                if current_bet == 0 {
                    Action::Check
                } else if bot_chips <= current_bet {
                    Action::AllIn
                } else {
                    Action::Call
                }
            }
            ActionType::Raise => {
                let raise_amount = self.calculate_raise_amount(params, pot_size, current_bet, bot_chips);
                if bot_chips <= raise_amount {
                    Action::AllIn
                } else {
                    Action::Raise(Some(raise_amount))
                }
            }
            ActionType::Bluff => {
                // Aggressive bluff sizing
                let bluff_size = (pot_size as f32 * 1.5) as u32;
                if bot_chips <= bluff_size {
                    Action::AllIn
                } else {
                    Action::Raise(Some(bluff_size))
                }
            }
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
        hand_strength >= threshold && self.rng.gen_bool(params.pfr as f64)
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
        self.rng.gen_bool(params.cbet_frequency as f64)
    }

    /// Decide whether to check when possible
    fn should_check(&mut self, params: &DifficultyParams) -> bool {
        // More passive = more likely to check
        let check_probability = 1.0 / (params.aggression_factor + 1.0);
        self.rng.gen_bool(check_probability as f64)
    }

    /// Decide action type (fold/call/raise/bluff)
    fn decide_action_type(
        &mut self,
        params: &DifficultyParams,
        pot_size: u32,
        current_bet: u32,
    ) -> ActionType {
        // Check if bot should bluff
        if params.bluffs && self.rng.gen_bool(params.bluff_frequency as f64) {
            return ActionType::Bluff;
        }

        // Decision based on pot odds and aggression
        let pot_odds = if pot_size > 0 {
            current_bet as f32 / pot_size as f32
        } else {
            0.0
        };

        // Aggressive bots more likely to raise
        let raise_threshold = 1.0 / (params.aggression_factor + 1.0);

        if self.rng.gen_bool(raise_threshold as f64) {
            ActionType::Raise
        } else if pot_odds < 0.3 || self.rng.gen_bool(0.6) {
            ActionType::Call
        } else {
            ActionType::Fold
        }
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
            x if x < 1.0 => 2.0,   // Passive: small raises
            x if x < 2.0 => 2.5,   // Moderate: standard raises
            _ => 3.0,              // Aggressive: large raises
        };

        // Add randomness (±20%)
        let variance = self.rng.gen_range(-0.2..=0.2);
        let multiplier = base_multiplier * (1.0 + variance);

        let raise_amount = ((pot_size + current_bet) as f32 * multiplier) as u32;

        // Cap at bot's chips
        raise_amount.min(bot_chips)
    }

    /// Estimate hand strength (placeholder - would integrate with hand evaluation)
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
        _hole_cards: &[crate::game::entities::Card],
        _board_cards: &[crate::game::entities::Card],
    ) -> f32 {
        // TODO: Integrate with actual hand evaluation
        // For now, return random value
        use rand::Rng;
        rand::thread_rng().gen_range(0.0..=1.0)
    }
}

impl Default for BotDecisionMaker {
    fn default() -> Self {
        Self::new()
    }
}

/// Internal action type for decision making
enum ActionType {
    Fold,
    Call,
    Raise,
    Bluff,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::table::config::BotDifficulty;
    use crate::bot::models::BotConfig;

    #[test]
    fn test_easy_bot_is_passive() {
        let mut decision_maker = BotDecisionMaker::new();
        let params = DifficultyParams::easy();

        // Easy bot should rarely raise
        let mut raise_count = 0;
        for _ in 0..100 {
            let action = decision_maker.decide_action(
                &create_test_bot(BotDifficulty::Easy),
                100,
                10,
                1000,
                false,
            );
            if matches!(action, Action::Raise(_)) {
                raise_count += 1;
            }
        }

        // Should raise less than 30% of the time (passive)
        assert!(raise_count < 30);
    }

    #[test]
    fn test_tag_bot_is_aggressive() {
        let mut decision_maker = BotDecisionMaker::new();
        let _params = DifficultyParams::tag();

        // TAG bot should raise more frequently
        let mut raise_count = 0;
        for _ in 0..100 {
            let action = decision_maker.decide_action(
                &create_test_bot(BotDifficulty::Tag),
                100,
                10,
                1000,
                false,
            );
            if matches!(action, Action::Raise(_) | Action::AllIn) {
                raise_count += 1;
            }
        }

        // Should raise more than 40% of the time (aggressive)
        assert!(raise_count > 40);
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
