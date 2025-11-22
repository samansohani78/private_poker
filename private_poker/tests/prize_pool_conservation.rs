//! Prize pool conservation tests for tournament payouts.
//!
//! These tests verify that tournament prize pools are correctly distributed
//! with no chips lost due to rounding errors. All payouts must sum to exactly
//! the total prize pool.

#![allow(clippy::unreadable_literal)]

use private_poker::tournament::models::PrizeStructure;

#[test]
fn test_winner_takes_all_conservation() {
    // Test with various player counts and buy-ins
    let test_cases = vec![(2, 100), (3, 50), (5, 1000), (4, 25), (5, 1)];

    for (players, buy_in) in test_cases {
        let structure = PrizeStructure::standard(players, buy_in);
        let total_pool = (players as i64) * buy_in;
        let payout_sum: i64 = structure.payouts.iter().sum();

        assert_eq!(
            total_pool, payout_sum,
            "Winner-takes-all: {} players × {} buy-in = {} pool, but payouts sum to {}",
            players, buy_in, total_pool, payout_sum
        );

        assert_eq!(
            structure.payouts.len(),
            1,
            "Winner-takes-all should have 1 payout"
        );
        assert_eq!(
            structure.payouts[0], total_pool,
            "Winner should get full pool"
        );
    }
}

#[test]
fn test_sixty_forty_split_conservation() {
    // Test 60/40 split (6-9 players)
    let test_cases = vec![
        (6, 100),  // 600 pool
        (7, 50),   // 350 pool
        (8, 1000), // 8000 pool
        (9, 25),   // 225 pool
        (6, 1),    // 6 pool (edge case)
        (9, 999),  // 8991 pool (odd number)
    ];

    for (players, buy_in) in test_cases {
        let structure = PrizeStructure::standard(players, buy_in);
        let total_pool = (players as i64) * buy_in;
        let payout_sum: i64 = structure.payouts.iter().sum();

        assert_eq!(
            total_pool, payout_sum,
            "60/40 split: {} players × {} buy-in = {} pool, but payouts sum to {}",
            players, buy_in, total_pool, payout_sum
        );

        assert_eq!(structure.payouts.len(), 2, "60/40 should have 2 payouts");

        // Verify first place gets approximately 60%
        // For very small pools (< 100 chips), allow more variance due to integer truncation
        let first_percentage = (structure.payouts[0] as f64 / total_pool as f64) * 100.0;
        let tolerance = if total_pool < 100 { 15.0 } else { 0.5 };

        assert!(
            first_percentage >= 60.0 - tolerance,
            "First place should get ~60% (±{:.1}% for small pools), got {:.2}% (payouts: {:?})",
            tolerance,
            first_percentage,
            structure.payouts
        );
        assert!(
            first_percentage <= 60.0 + tolerance,
            "First place should get ~60% (±{:.1}%), got {:.2}% (payouts: {:?})",
            tolerance,
            first_percentage,
            structure.payouts
        );
    }
}

#[test]
fn test_fifty_thirty_twenty_split_conservation() {
    // Test 50/30/20 split (10+ players)
    let test_cases = vec![
        (10, 100),  // 1000 pool
        (15, 50),   // 750 pool
        (20, 1000), // 20000 pool
        (10, 25),   // 250 pool
        (10, 1),    // 10 pool (edge case)
        (10, 333),  // 3330 pool (odd number)
    ];

    for (players, buy_in) in test_cases {
        let structure = PrizeStructure::standard(players, buy_in);
        let total_pool = (players as i64) * buy_in;
        let payout_sum: i64 = structure.payouts.iter().sum();

        assert_eq!(
            total_pool, payout_sum,
            "50/30/20 split: {} players × {} buy-in = {} pool, but payouts sum to {}",
            players, buy_in, total_pool, payout_sum
        );

        assert_eq!(structure.payouts.len(), 3, "50/30/20 should have 3 payouts");

        // Verify percentages (allowing for rounding in third place)
        let first_pct = (structure.payouts[0] as f64 / total_pool as f64) * 100.0;
        let second_pct = (structure.payouts[1] as f64 / total_pool as f64) * 100.0;
        let third_pct = (structure.payouts[2] as f64 / total_pool as f64) * 100.0;

        assert!(
            (first_pct - 50.0).abs() < 0.1,
            "First should be ~50%, got {:.2}%",
            first_pct
        );
        assert!(
            (second_pct - 30.0).abs() < 0.1,
            "Second should be ~30%, got {:.2}%",
            second_pct
        );
        assert!(
            (third_pct - 20.0).abs() < 0.1,
            "Third should be ~20%, got {:.2}%",
            third_pct
        );
    }
}

#[test]
fn test_custom_prize_structure_conservation() {
    // Test custom percentages
    let test_cases = vec![
        (1000, vec![0.70, 0.30]),                    // 70/30 split
        (5000, vec![0.50, 0.25, 0.15, 0.10]),        // 4-way split
        (100, vec![0.40, 0.30, 0.20, 0.10]),         // Even 100 chips
        (999, vec![0.60, 0.30, 0.10]),               // Odd pool
        (1, vec![1.0]),                              // Single chip winner-takes-all
        (10000, vec![0.45, 0.25, 0.15, 0.10, 0.05]), // 5-way split
    ];

    for (pool, percentages) in test_cases {
        let structure = PrizeStructure::custom(pool, percentages.clone());
        let payout_sum: i64 = structure.payouts.iter().sum();

        assert_eq!(
            pool, payout_sum,
            "Custom structure: {} pool with {:?} percentages, but payouts sum to {}",
            pool, percentages, payout_sum
        );

        // Verify total percentage is 100%
        let total_pct: f64 = percentages.iter().sum();
        assert!(
            (total_pct - 1.0).abs() < 0.0001,
            "Total percentage should be 1.0, got {}",
            total_pct
        );
    }
}

#[test]
fn test_edge_case_small_pools() {
    // Test very small pools where rounding matters most
    let test_cases = vec![
        (2, 1),  // 2-chip pool, winner-takes-all
        (6, 1),  // 6-chip pool, 60/40 split
        (10, 1), // 10-chip pool, 50/30/20 split
        (3, 1),  // 3-chip pool, winner-takes-all
    ];

    for (players, buy_in) in test_cases {
        let structure = PrizeStructure::standard(players, buy_in);
        let total_pool = (players as i64) * buy_in;
        let payout_sum: i64 = structure.payouts.iter().sum();

        assert_eq!(
            total_pool, payout_sum,
            "Small pool: {} players × {} buy-in = {} pool, payouts = {:?}",
            players, buy_in, total_pool, structure.payouts
        );

        // Verify no payout is zero (everyone in the money should get something)
        for (i, &payout) in structure.payouts.iter().enumerate() {
            assert!(
                payout > 0,
                "Position {} payout should be > 0, got {}",
                i + 1,
                payout
            );
        }
    }
}

#[test]
fn test_edge_case_large_pools() {
    // Test very large pools to ensure no overflow
    let test_cases = vec![
        (100, 10000),  // 1M pool
        (1000, 1000),  // 1M pool
        (50, 100000),  // 5M pool
        (10, 1000000), // 10M pool
    ];

    for (players, buy_in) in test_cases {
        let structure = PrizeStructure::standard(players, buy_in);
        let total_pool = (players as i64) * buy_in;
        let payout_sum: i64 = structure.payouts.iter().sum();

        assert_eq!(
            total_pool, payout_sum,
            "Large pool: {} players × {} buy-in = {} pool, but payouts sum to {}",
            players, buy_in, total_pool, payout_sum
        );
    }
}

#[test]
fn test_payout_ordering() {
    // Verify first place always gets the most
    let test_cases = vec![
        (6, 100),  // 60/40
        (10, 100), // 50/30/20
        (9, 50),   // 60/40
        (15, 25),  // 50/30/20
    ];

    for (players, buy_in) in test_cases {
        let structure = PrizeStructure::standard(players, buy_in);

        // Verify descending order
        for i in 0..structure.payouts.len() - 1 {
            assert!(
                structure.payouts[i] >= structure.payouts[i + 1],
                "Payouts should be in descending order. Position {} ({}) < Position {} ({})",
                i + 1,
                structure.payouts[i],
                i + 2,
                structure.payouts[i + 1]
            );
        }
    }
}

#[test]
fn test_payout_for_position() {
    let structure = PrizeStructure::standard(10, 100);

    // Test valid positions (1-indexed)
    assert_eq!(structure.payout_for_position(1), Some(500)); // 50% of 1000
    assert_eq!(structure.payout_for_position(2), Some(300)); // 30% of 1000
    assert_eq!(structure.payout_for_position(3), Some(200)); // 20% of 1000

    // Test out-of-money positions
    assert_eq!(structure.payout_for_position(4), None);
    assert_eq!(structure.payout_for_position(10), None);
    assert_eq!(structure.payout_for_position(0), None); // Invalid position
}

#[test]
fn test_no_negative_payouts() {
    // Ensure no payout is ever negative, even with weird inputs
    let test_cases = vec![(1, 1), (2, 1), (100, 1), (10, 999)];

    for (players, buy_in) in test_cases {
        let structure = PrizeStructure::standard(players, buy_in);

        for (i, &payout) in structure.payouts.iter().enumerate() {
            assert!(
                payout >= 0,
                "Position {} has negative payout: {}",
                i + 1,
                payout
            );
        }
    }
}

#[test]
fn test_zero_players_edge_case() {
    // Edge case: 0 or 1 player
    let structure = PrizeStructure::standard(0, 100);
    assert_eq!(structure.total_pool, 0);
    assert_eq!(structure.payouts.iter().sum::<i64>(), 0);

    let structure = PrizeStructure::standard(1, 100);
    assert_eq!(structure.total_pool, 100);
    assert_eq!(structure.payouts.iter().sum::<i64>(), 100);
}
