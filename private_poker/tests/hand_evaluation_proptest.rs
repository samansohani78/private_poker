/// Property-based tests for hand evaluation using proptest
///
/// These tests verify that the hand evaluation logic is correct
/// across a wide range of randomly generated card combinations.

use private_poker::game::{
    entities::{Card, Suit},
    functional::{argmax, eval, prepare_hand},
};
use proptest::prelude::*;
use std::collections::BTreeSet;

// Strategy to generate a valid card (values 1-13, aces are value 1)
fn card_strategy() -> impl Strategy<Value = Card> {
    (1u8..=13, 0u8..=3).prop_map(|(value, suit_idx)| {
        let suit = match suit_idx {
            0 => Suit::Club,
            1 => Suit::Diamond,
            2 => Suit::Heart,
            _ => Suit::Spade,
        };
        Card(value, suit)
    })
}

// Strategy to generate a vec of unique cards (no duplicates)
fn unique_cards_strategy(min: usize, max: usize) -> impl Strategy<Value = Vec<Card>> {
    prop::collection::vec(card_strategy(), min..=max).prop_filter(
        "Cards must be unique",
        |cards| {
            let set: BTreeSet<_> = cards.iter().collect();
            set.len() == cards.len()
        },
    )
}

// Strategy to generate exactly 5 unique cards
fn five_card_hand_strategy() -> impl Strategy<Value = Vec<Card>> {
    unique_cards_strategy(5, 5)
}

// Strategy to generate 7 unique cards (like Texas Hold'em: 2 hole + 5 board)
fn seven_card_hand_strategy() -> impl Strategy<Value = Vec<Card>> {
    unique_cards_strategy(7, 7)
}

// Helper function to prepare and evaluate a hand
fn eval_hand(cards: &[Card]) -> Vec<private_poker::game::entities::SubHand> {
    let mut hand = cards.to_vec();
    prepare_hand(&mut hand);
    eval(&hand)
}

proptest! {
    #[test]
    fn test_eval_always_returns_valid_hand(cards in seven_card_hand_strategy()) {
        let hand = eval_hand(&cards);

        // Should return at least one subhand
        prop_assert!(!hand.is_empty(), "eval() should return at least one subhand");

        // Total cards in all subhands should be <= 5 (best 5-card hand)
        let total_cards: usize = hand.iter().map(|sh| sh.values.len()).sum();
        prop_assert!(total_cards <= 5, "eval() should return at most 5 cards total");
    }

    #[test]
    fn test_eval_handles_minimum_cards(cards in unique_cards_strategy(2, 2)) {
        let hand = eval_hand(&cards);
        // Should be able to evaluate even with just 2 cards
        prop_assert!(!hand.is_empty(), "eval() should handle 2 cards");
    }

    #[test]
    fn test_eval_deterministic(cards in seven_card_hand_strategy()) {
        let hand1 = eval_hand(&cards);
        let hand2 = eval_hand(&cards);

        // Same input should produce same output
        prop_assert_eq!(hand1, hand2, "eval() should be deterministic");
    }

    #[test]
    fn test_argmax_single_hand_returns_zero(cards in five_card_hand_strategy()) {
        let hand = eval_hand(&cards);
        let winners = argmax(&[hand]);

        // Single hand should always be the winner
        prop_assert_eq!(winners, vec![0], "Single hand should always win");
    }

    #[test]
    fn test_argmax_identical_hands_all_win(cards in five_card_hand_strategy()) {
        let hand = eval_hand(&cards);
        let winners = argmax(&[hand.clone(), hand.clone(), hand]);

        // All identical hands should tie
        prop_assert_eq!(winners, vec![0, 1, 2], "Identical hands should all win");
    }

    #[test]
    fn test_argmax_returns_valid_indices(
        hands in prop::collection::vec(five_card_hand_strategy(), 2..=10)
    ) {
        let evaluated: Vec<_> = hands.iter().map(|h| eval(h)).collect();
        let winners = argmax(&evaluated);

        // Winners should not be empty
        prop_assert!(!winners.is_empty(), "argmax should return at least one winner");

        // All indices should be valid
        for &winner_idx in &winners {
            prop_assert!(winner_idx < evaluated.len(), "Winner index should be valid");
        }

        // Indices should be sorted and unique
        let mut sorted_winners = winners.clone();
        sorted_winners.sort();
        sorted_winners.dedup();
        prop_assert_eq!(winners, sorted_winners, "Winners should be sorted and unique");
    }
}

// Additional specific property tests for hand rankings

proptest! {
    /// Test that a royal flush (A-K-Q-J-10 of same suit) beats any other hand
    #[test]
    fn test_royal_flush_beats_all(suit_idx in 0u8..=3) {
        let suit = match suit_idx {
            0 => Suit::Club,
            1 => Suit::Diamond,
            2 => Suit::Heart,
            _ => Suit::Spade,
        };

        // Get the other suits (not used in royal flush)
        let other_suits: Vec<Suit> = vec![Suit::Club, Suit::Diamond, Suit::Heart, Suit::Spade]
            .into_iter()
            .filter(|&s| s != suit)
            .collect();

        // Create a royal flush (Ace as value 1, prepare_hand will add value 14)
        let royal_flush = vec![
            Card(1, suit),  // Ace (will become 1 and 14 after prepare_hand)
            Card(10, suit), // 10
            Card(11, suit), // Jack
            Card(12, suit), // Queen
            Card(13, suit), // King
        ];

        // Create four of a kind (using different suit to avoid card overlap)
        let four_kind = vec![
            Card(9, other_suits[0]),
            Card(9, other_suits[1]),
            Card(9, other_suits[2]),
            Card(9, suit),
            Card(8, suit),
        ];

        let royal_hand = eval_hand(&royal_flush);
        let four_kind_hand = eval_hand(&four_kind);

        // Royal flush should always beat four of a kind
        let winners = argmax(&[royal_hand, four_kind_hand]);
        prop_assert_eq!(winners, vec![0], "Royal flush should beat four of a kind");
    }

    /// Test that a straight flush beats four of a kind
    #[test]
    fn test_straight_flush_beats_four_kind(suit_idx in 0u8..=3) {
        let suit = match suit_idx {
            0 => Suit::Club,
            1 => Suit::Diamond,
            2 => Suit::Heart,
            _ => Suit::Spade,
        };

        // Create a 5-6-7-8-9 straight flush
        let straight_flush = vec![
            Card(5, suit),
            Card(6, suit),
            Card(7, suit),
            Card(8, suit),
            Card(9, suit),
        ];

        // Create four kings with a queen kicker
        let four_kind = vec![
            Card(13, Suit::Club),
            Card(13, Suit::Diamond),
            Card(13, Suit::Heart),
            Card(13, Suit::Spade),
            Card(12, Suit::Club),
        ];

        let sf_hand = eval_hand(&straight_flush);
        let fk_hand = eval_hand(&four_kind);

        let winners = argmax(&[sf_hand, fk_hand]);
        prop_assert_eq!(winners, vec![0], "Straight flush should beat four of a kind");
    }

    /// Test that four of a kind beats a full house
    #[test]
    fn test_four_kind_beats_full_house(quad_value in 2u8..=13, trip_value in 2u8..=13) {
        // Ensure different values
        prop_assume!(quad_value != trip_value);

        // Create four of a kind
        let four_kind = vec![
            Card(quad_value, Suit::Club),
            Card(quad_value, Suit::Diamond),
            Card(quad_value, Suit::Heart),
            Card(quad_value, Suit::Spade),
            Card(trip_value, Suit::Club), // Kicker
        ];

        // Create full house
        let full_house = vec![
            Card(trip_value, Suit::Club),
            Card(trip_value, Suit::Diamond),
            Card(trip_value, Suit::Heart),
            Card(quad_value, Suit::Club),
            Card(quad_value, Suit::Diamond),
        ];

        let fk_hand = eval_hand(&four_kind);
        let fh_hand = eval_hand(&full_house);

        let winners = argmax(&[fk_hand, fh_hand]);
        prop_assert_eq!(winners, vec![0], "Four of a kind should beat full house");
    }

    /// Test that a full house beats a flush
    #[test]
    fn test_full_house_beats_flush(suit_idx in 0u8..=3) {
        let suit = match suit_idx {
            0 => Suit::Club,
            1 => Suit::Diamond,
            2 => Suit::Heart,
            _ => Suit::Spade,
        };

        // Create full house: three 8s and two 5s
        let full_house = vec![
            Card(8, Suit::Club),
            Card(8, Suit::Diamond),
            Card(8, Suit::Heart),
            Card(5, Suit::Club),
            Card(5, Suit::Diamond),
        ];

        // Create flush: 2-4-7-10-13 all same suit (not a straight)
        let flush = vec![
            Card(2, suit),
            Card(4, suit),
            Card(7, suit),
            Card(10, suit),
            Card(13, suit),
        ];

        let fh_hand = eval_hand(&full_house);
        let fl_hand = eval_hand(&flush);

        let winners = argmax(&[fh_hand, fl_hand]);
        prop_assert_eq!(winners, vec![0], "Full house should beat flush");
    }

    /// Test that a flush beats a straight
    #[test]
    fn test_flush_beats_straight(suit_idx in 0u8..=3) {
        let suit = match suit_idx {
            0 => Suit::Club,
            1 => Suit::Diamond,
            2 => Suit::Heart,
            _ => Suit::Spade,
        };

        // Create flush: 2-5-8-10-K all same suit (not a straight)
        let flush = vec![
            Card(2, suit),
            Card(5, suit),
            Card(8, suit),
            Card(10, suit),
            Card(13, suit),
        ];

        // Create straight (mixed suits): 7-8-9-10-J
        let straight = vec![
            Card(7, Suit::Club),
            Card(8, Suit::Diamond),
            Card(9, Suit::Heart),
            Card(10, Suit::Spade),
            Card(11, Suit::Club),
        ];

        let fl_hand = eval_hand(&flush);
        let st_hand = eval_hand(&straight);

        let winners = argmax(&[fl_hand, st_hand]);
        prop_assert_eq!(winners, vec![0], "Flush should beat straight");
    }

    /// Test that three of a kind beats two pair
    #[test]
    fn test_three_kind_beats_two_pair(trip_value in 2u8..=13, pair1 in 2u8..=13, pair2 in 2u8..=13) {
        prop_assume!(trip_value != pair1 && trip_value != pair2 && pair1 != pair2);

        // Create three of a kind
        let three_kind = vec![
            Card(trip_value, Suit::Club),
            Card(trip_value, Suit::Diamond),
            Card(trip_value, Suit::Heart),
            Card(pair1, Suit::Club),
            Card(pair2, Suit::Diamond),
        ];

        // Create two pair
        let two_pair = vec![
            Card(pair1, Suit::Club),
            Card(pair1, Suit::Diamond),
            Card(pair2, Suit::Heart),
            Card(pair2, Suit::Spade),
            Card(trip_value, Suit::Club),
        ];

        let tk_hand = eval_hand(&three_kind);
        let tp_hand = eval_hand(&two_pair);

        let winners = argmax(&[tk_hand, tp_hand]);
        prop_assert_eq!(winners, vec![0], "Three of a kind should beat two pair");
    }

    /// Test that a pair beats high card
    #[test]
    fn test_pair_beats_high_card(suit_idx in 0u8..=3) {
        let suit = match suit_idx {
            0 => Suit::Club,
            1 => Suit::Diamond,
            2 => Suit::Heart,
            _ => Suit::Spade,
        };

        // Create pair of 7s with kickers 2, 5, 10
        let pair = vec![
            Card(7, Suit::Club),
            Card(7, Suit::Diamond),
            Card(2, suit),
            Card(5, suit),
            Card(10, suit),
        ];

        // Create high card: A-K-Q-J-9 (no pairs)
        let high_card = vec![
            Card(1, Suit::Club),  // Ace
            Card(9, suit),
            Card(11, Suit::Spade),
            Card(12, Suit::Heart),
            Card(13, Suit::Diamond),
        ];

        let pair_hand = eval_hand(&pair);
        let hc_hand = eval_hand(&high_card);

        let winners = argmax(&[pair_hand, hc_hand]);
        prop_assert_eq!(winners, vec![0], "Pair should beat high card");
    }
}
