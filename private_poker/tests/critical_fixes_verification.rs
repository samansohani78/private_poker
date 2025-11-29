//! Integration tests verifying all critical fixes from comprehensive audit
//!
//! These tests verify that the following critical issues have been properly fixed:
//! - Issue #5: Wallet balance atomicity (tested via database integration tests)
//! - Issue #7: Blind insufficiency enforcement (tested via table actor tests)
//! - Issue #12: Deck exhaustion handling - now correctly panics (deck should never exhaust)
//! - Issue #13: Top-up cooldown enforcement (tested via table actor tests)

use private_poker::game::entities::{Deck, Suit};

/// Test deck normal operation (dealing less than 52 cards)
#[test]
fn test_deck_normal_operation() {
    let mut deck = Deck::default();

    // Deal 28 cards (max realistic usage: 9 players × 2 + 5 board + 3 burn)
    for i in 0..28 {
        let card = deck.deal_card();
        assert!(
            card.0 >= 1 && card.0 <= 13,
            "Card value {} out of range at index {}",
            card.0,
            i
        );
        assert!(matches!(
            card.1,
            Suit::Club | Suit::Diamond | Suit::Heart | Suit::Spade
        ));
    }

    // Should still have 24 cards remaining
    assert_eq!(deck.deck_idx, 28, "Deck index should be 28 after dealing 28 cards");
}

/// Test that deck exhaustion correctly panics (deck should NEVER exhaust in valid gameplay)
#[test]
#[should_panic(expected = "Deck exhausted - this indicates a critical bug!")]
fn test_deck_exhaustion_panics() {
    let mut deck = Deck::default();

    // Deal all 52 cards
    for _ in 0..52 {
        deck.deal_card();
    }

    // Attempting to deal 53rd card should panic
    deck.deal_card();
}

/// Test deck integrity - all 52 cards are unique
#[test]
fn test_deck_integrity() {
    use std::collections::HashSet;

    let mut deck = Deck::default();
    let mut seen_cards = HashSet::new();

    // Deal all 52 cards
    for _ in 0..52 {
        let card = deck.deal_card();

        // Verify card is valid
        assert!(card.0 >= 1 && card.0 <= 13, "Invalid card value: {}", card.0);
        assert!(
            matches!(
                card.1,
                Suit::Club | Suit::Diamond | Suit::Heart | Suit::Spade
            ),
            "Invalid suit"
        );

        // Verify card is unique
        let card_repr = (card.0, card.1 as u8);
        assert!(
            seen_cards.insert(card_repr),
            "Duplicate card found: {:?}",
            card
        );
    }

    // Verify we got exactly 52 unique cards
    assert_eq!(seen_cards.len(), 52, "Should have 52 unique cards");
}

/// Test deck distribution - each value appears 4 times (once per suit)
#[test]
fn test_deck_distribution() {
    use std::collections::HashMap;

    let mut deck = Deck::default();
    let mut value_counts: HashMap<u8, usize> = HashMap::new();

    // Deal all 52 cards
    for _ in 0..52 {
        let card = deck.deal_card();
        *value_counts.entry(card.0).or_insert(0) += 1;
    }

    // Each value (1-13) should appear exactly 4 times
    for value in 1..=13 {
        assert_eq!(
            value_counts.get(&value).copied().unwrap_or(0),
            4,
            "Value {} should appear exactly 4 times",
            value
        );
    }
}

/// Test that deck produces valid cards
#[test]
fn test_deck_produces_valid_cards() {
    let mut deck = Deck::default();

    // Deal 10 cards and verify each is valid
    for _ in 0..10 {
        let card = deck.deal_card();

        // All cards should be valid
        assert!(card.0 >= 1 && card.0 <= 13, "Invalid card value: {}", card.0);
        assert!(
            matches!(card.1, Suit::Club | Suit::Diamond | Suit::Heart | Suit::Spade),
            "Invalid suit"
        );
    }
}

/// Test deck state management
#[test]
fn test_deck_state_management() {
    let mut deck = Deck::default();

    assert_eq!(deck.deck_idx, 0, "New deck should start at index 0");

    // Deal some cards
    for i in 1..=10 {
        deck.deal_card();
        assert_eq!(deck.deck_idx, i, "Deck index should increment");
    }

    // Shuffle should reset index
    deck.shuffle();
    assert_eq!(deck.deck_idx, 0, "Shuffle should reset deck_idx to 0");

    // Should be able to deal again
    deck.deal_card();
    assert_eq!(deck.deck_idx, 1, "Should be able to deal after shuffle");
}
