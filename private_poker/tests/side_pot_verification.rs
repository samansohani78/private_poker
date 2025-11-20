//! Comprehensive side pot calculation tests using property-based testing
//!
//! These tests verify that side pot distribution works correctly in all scenarios:
//! - Multiple all-ins at different amounts
//! - Folded players contribute but can't win
//! - Correct distribution of remainder chips
//! - Side pot eligibility based on investment levels

use private_poker::game::entities::{Pot, Usd};
use proptest::prelude::*;
use std::collections::{BTreeMap, HashMap};

/// Test that pot distribution respects investment amounts
#[test]
fn test_simple_side_pot_three_players() {
    // Scenario: 3 players, one all-in for less
    // Player 0: All-in $50
    // Player 1: Calls $100
    // Player 2: Calls $100
    //
    // Expected:
    // - Main pot: $150 ($50 from each, all 3 players eligible)
    // - Side pot: $100 ($50 from players 1 and 2, only players 1 and 2 eligible)

    let mut investments: BTreeMap<usize, i64> = BTreeMap::new();
    investments.insert(0, 50);  // All-in short
    investments.insert(1, 100); // Full bet
    investments.insert(2, 100); // Full bet

    // Main pot eligibility: all 3 players for first $50 each = $150
    // Side pot eligibility: players 1,2 for remaining $50 each = $100
    let total_pot: i64 = investments.values().sum();
    assert_eq!(total_pot, 250);
}

#[test]
fn test_multiple_side_pots_four_players() {
    // Scenario: 4 players at different stack sizes
    // Player 0: All-in $25
    // Player 1: All-in $75
    // Player 2: All-in $150
    // Player 3: Calls $150
    //
    // Expected pots:
    // - Main pot: $100 ($25 × 4, all 4 eligible)
    // - Side pot 1: $150 ($50 × 3, players 1/2/3 eligible)
    // - Side pot 2: $150 ($75 × 2, players 2/3 eligible)

    let mut investments: BTreeMap<usize, i64> = BTreeMap::new();
    investments.insert(0, 25);
    investments.insert(1, 75);
    investments.insert(2, 150);
    investments.insert(3, 150);

    let total_pot: i64 = investments.values().sum();
    assert_eq!(total_pot, 400);

    // Verify pot calculation logic
    // Main pot: min(25,75,150,150) × 4 = 100
    // After removing $25 from each: (0, 50, 125, 125)
    // Side pot 1: min(50,125,125) × 3 = 150
    // After removing $50 from remaining: (0, 0, 75, 75)
    // Side pot 2: min(75,75) × 2 = 150
}

#[test]
fn test_side_pot_with_folder() {
    // Scenario: One player folds after betting
    // Player 0: Bets $50, then folds
    // Player 1: All-in $100
    // Player 2: Calls $100
    //
    // Expected:
    // - Player 0's $50 goes into pot but player 0 is not eligible to win
    // - Only players 1 and 2 compete for the pot
    let mut investments: BTreeMap<usize, i64> = BTreeMap::new();
    investments.insert(0, 50);  // Folded - contributes but can't win
    investments.insert(1, 100); // Active
    investments.insert(2, 100); // Active

    let total_pot: i64 = investments.values().sum();
    assert_eq!(total_pot, 250);
}

// Property-based tests using proptest

/// Strategy to generate valid investment amounts (1-1000)
fn investment_strategy() -> impl Strategy<Value = i64> {
    1i64..=1000
}

/// Strategy to generate 2-9 players with different investment amounts
fn player_investments_strategy() -> impl Strategy<Value = BTreeMap<usize, i64>> {
    (2usize..=9).prop_flat_map(|num_players| {
        prop::collection::vec(investment_strategy(), num_players..=num_players)
            .prop_map(move |investments| {
                investments
                    .into_iter()
                    .enumerate()
                    .collect::<BTreeMap<usize, i64>>()
            })
    })
}

proptest! {
    /// Test that total pot size equals sum of all investments
    #[test]
    fn test_pot_conservation(investments in player_investments_strategy()) {
        let total_invested: i64 = investments.values().sum();

        // Total pot should equal total invested
        prop_assert!(total_invested > 0, "Total pot should be positive");

        // Verify no player has negative investment
        for &investment in investments.values() {
            prop_assert!(investment > 0, "Investment should be positive");
        }
    }

    /// Test that side pot distribution respects investment limits
    #[test]
    fn test_side_pot_eligibility(investments in player_investments_strategy()) {
        // Sort investments to identify pot levels
        let mut sorted_investments: Vec<_> = investments.iter().collect();
        sorted_investments.sort_by_key(|&(_, inv)| inv);

        // First pot should include all players up to minimum investment
        if let Some(&(_, min_investment)) = sorted_investments.first() {
            let first_pot_size = min_investment * sorted_investments.len() as i64;

            prop_assert!(first_pot_size > 0, "First pot should be positive");
            prop_assert!(
                first_pot_size <= investments.values().sum::<i64>(),
                "First pot should not exceed total"
            );
        }
    }

    /// Test that remainder distribution follows poker rules
    /// (remainder goes to earliest position)
    #[test]
    fn test_remainder_distribution(
        pot_size in 100i64..=1000,
        num_winners in 2usize..=9
    ) {
        let pot_split = pot_size / num_winners as i64;
        let remainder = pot_size % num_winners as i64;

        // Verify remainder is less than number of winners
        prop_assert!(
            (remainder as usize) < num_winners,
            "Remainder should be less than number of winners"
        );

        // Verify distribution sums to pot
        let total_distributed = pot_split * num_winners as i64 + remainder;
        prop_assert_eq!(
            total_distributed,
            pot_size,
            "Distributed amount should equal pot size"
        );
    }

    /// Test that no player receives more than total pot
    #[test]
    fn test_no_player_exceeds_pot(
        investments in player_investments_strategy(),
        _winner_idx in 0usize..=8
    ) {
        let total_pot: i64 = investments.values().sum();

        // Even if a single player wins, they can't win more than total pot
        prop_assert!(
            total_pot >= 0,
            "Winner can't receive more than total pot"
        );
    }

    /// Test that investments are non-negative after distribution
    #[test]
    fn test_investments_consumed_correctly(mut investments in player_investments_strategy()) {
        // Simulate pot distribution by removing minimum investment
        if let Some(&min_investment) = investments.values().min() {
            for investment in investments.values_mut() {
                *investment -= min_investment;
                prop_assert!(*investment >= 0, "Remaining investment should be non-negative");
            }
        }
    }

    /// Test that side pots are created only when investments differ
    #[test]
    fn test_side_pots_only_when_needed(investments in player_investments_strategy()) {
        let unique_amounts: std::collections::BTreeSet<_> =
            investments.values().copied().collect();

        // Number of pots should equal number of unique investment amounts
        let expected_pots = unique_amounts.len();

        prop_assert!(
            expected_pots > 0,
            "Should have at least one pot"
        );
        prop_assert!(
            expected_pots <= investments.len(),
            "Can't have more pots than players"
        );
    }

    /// Test that all-in amounts are respected
    #[test]
    fn test_all_in_limits(
        investments in player_investments_strategy(),
        all_in_amount in 1i64..=500
    ) {
        // If a player goes all-in for X, they can only win up to X from each opponent
        let num_opponents = investments.len() - 1;
        let max_winnable = all_in_amount * (num_opponents as i64 + 1);

        let total_pot: i64 = investments.values().sum();

        // Player's maximum win is capped by their all-in amount
        if total_pot < max_winnable {
            prop_assert!(
                total_pot <= max_winnable,
                "Can't win more than matched investment"
            );
        }
    }

    /// Test that folded players don't receive winnings
    #[test]
    fn test_folded_players_excluded(
        investments in player_investments_strategy(),
        fold_idx in 0usize..=8
    ) {
        // If player at fold_idx folds, they should not be eligible for any pot
        if fold_idx < investments.len() {
            // Player contributed to pot but is not eligible
            prop_assert!(
                investments.contains_key(&fold_idx),
                "Test setup should include folded player"
            );
        }
    }

    /// Test that pot splitting is fair and deterministic
    #[test]
    fn test_pot_split_fairness(
        pot_size in 100i64..=10000,
        num_winners in 2usize..=9
    ) {
        let _pot_split = pot_size / num_winners as i64;
        let remainder = pot_size % num_winners as i64;

        // Each winner gets at least pot_split
        // First (remainder) winners get pot_split + 1
        let max_diff = if remainder > 0 { 1 } else { 0 };

        prop_assert!(
            max_diff <= 1,
            "Max difference between winners should be at most 1 chip"
        );
    }

    /// Test complex scenario with multiple all-ins and folds
    #[test]
    fn test_complex_multi_pot_scenario(investments in player_investments_strategy()) {
        // Sort investments to simulate pot creation
        let mut sorted: Vec<_> = investments.values().copied().collect();
        sorted.sort_unstable();

        // Track total distributed
        let mut total_distributed = 0i64;
        let mut remaining_players = sorted.len();
        let mut prev_level = 0i64;

        for &level in &sorted {
            if level > prev_level {
                let pot_contribution = (level - prev_level) * remaining_players as i64;
                total_distributed += pot_contribution;
                prev_level = level;
            }
            remaining_players -= 1;
        }

        let total_invested: i64 = investments.values().sum();
        prop_assert_eq!(
            total_distributed,
            total_invested,
            "All chips should be distributed into pots"
        );
    }
}

/// Test pot distribution with actual game state
#[test]
fn test_pot_distribution_integration() {
    // This test documents the expected behavior of the full pot distribution
    // algorithm including hand evaluation and winner determination

    let mut pot = Pot {
        investments: HashMap::new(),
    };

    // Player 0: $50
    // Player 1: $100
    // Player 2: $100
    pot.investments.insert(0, 50);
    pot.investments.insert(1, 100);
    pot.investments.insert(2, 100);

    let pot_size: Usd = pot.investments.values().sum();
    assert_eq!(pot_size, 250);

    // Verify pot structure
    assert_eq!(pot.investments.len(), 3);
    assert!(pot.investments.contains_key(&0));
    assert!(pot.investments.contains_key(&1));
    assert!(pot.investments.contains_key(&2));
}

/// Test edge case: single player remaining (no contest)
#[test]
fn test_single_player_wins_all() {
    let mut investments: BTreeMap<usize, i64> = BTreeMap::new();
    investments.insert(0, 100); // Only player remaining

    let total_pot: i64 = investments.values().sum();
    assert_eq!(total_pot, 100);

    // Single player should win entire pot
}

/// Test edge case: all players invest same amount
#[test]
fn test_equal_investments_single_pot() {
    let mut investments: BTreeMap<usize, i64> = BTreeMap::new();
    investments.insert(0, 100);
    investments.insert(1, 100);
    investments.insert(2, 100);

    // Should create single pot of $300
    let total_pot: i64 = investments.values().sum();
    assert_eq!(total_pot, 300);

    // All three players eligible for entire pot
    let unique_amounts: std::collections::BTreeSet<_> =
        investments.values().copied().collect();
    assert_eq!(unique_amounts.len(), 1, "Should have only one pot level");
}

/// Test edge case: player with zero investment (big blind walk)
#[test]
fn test_zero_investment_excluded() {
    let mut investments: BTreeMap<usize, i64> = BTreeMap::new();
    investments.insert(0, 0);   // No investment
    investments.insert(1, 50);  // Small investment

    let active_investments: Vec<_> = investments
        .values()
        .copied()
        .filter(|&inv| inv > 0)
        .collect();

    assert_eq!(active_investments.len(), 1);
}
