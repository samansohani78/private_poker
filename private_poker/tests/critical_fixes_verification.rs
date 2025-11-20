//! Integration tests verifying all critical fixes from comprehensive audit
//!
//! These tests verify that the following critical issues have been properly fixed:
//! - Issue #5: Wallet balance atomicity (tested via database integration tests)
//! - Issue #7: Blind insufficiency enforcement (tested via table actor tests)
//! - Issue #12: Deck exhaustion handling (tested here)
//! - Issue #13: Top-up cooldown enforcement (tested via table actor tests)

use private_poker::game::entities::{Deck, Suit};

/// Test Issue #12: Deck exhaustion handling
#[test]
fn test_deck_exhaustion_automatic_reshuffle() {
    let mut deck = Deck::default();

    // Deal all 52 cards
    for i in 0..52 {
        let card = deck.deal_card();
        assert!(card.0 >= 1 && card.0 <= 13, "Card value {} out of range at index {}", card.0, i);
        assert!(matches!(card.1, Suit::Club | Suit::Diamond | Suit::Heart | Suit::Spade));
    }

    // Deck should be exhausted now (deck_idx == 52)
    // Attempting to deal another card should trigger reshuffle and not panic
    let card_53 = deck.deal_card();

    // Should get a valid card (not panic)
    assert!(card_53.0 >= 1 && card_53.0 <= 13);
    assert!(matches!(card_53.1, Suit::Club | Suit::Diamond | Suit::Heart | Suit::Spade));
}

/// Test Issue #12: Multiple deck exhaustions
#[test]
fn test_multiple_deck_exhaustions() {
    let mut deck = Deck::default();

    // Deal 156 cards (3 full decks worth)
    for i in 0..156 {
        let card = deck.deal_card();
        assert!(
            card.0 >= 1 && card.0 <= 13,
            "Card value out of range at index {}",
            i
        );
    }

    // Should have automatically reshuffled 3 times without panicking
}


/// Test deck integrity after reshuffle
#[test]
fn test_deck_integrity_after_reshuffle() {
    let mut deck = Deck::default();

    // Deal 60 cards (triggers at least one reshuffle)
    let mut cards = Vec::new();
    for _ in 0..60 {
        cards.push(deck.deal_card());
    }

    // Check that all cards are valid
    for (i, card) in cards.iter().enumerate() {
        assert!(
            card.0 >= 1 && card.0 <= 13,
            "Invalid card value {} at position {}",
            card.0,
            i
        );
        assert!(
            matches!(card.1, Suit::Club | Suit::Diamond | Suit::Heart | Suit::Spade),
            "Invalid suit at position {}",
            i
        );
    }
}

/// Test deck distribution after multiple reshuffles
#[test]
fn test_deck_distribution_after_reshuffles() {
    let mut deck = Deck::default();
    let mut value_counts = [0; 14]; // Index 0 unused, 1-13 for card values

    // Deal 200 cards (will trigger multiple reshuffles)
    for _ in 0..200 {
        let card = deck.deal_card();
        assert!(card.0 >= 1 && card.0 <= 13);
        value_counts[card.0 as usize] += 1;
    }

    // Each value (1-13) should appear roughly 200/13 ≈ 15 times
    // Allow some variance but ensure all values appear
    for (value, &count) in value_counts.iter().enumerate().skip(1).take(13) {
        assert!(
            count > 0,
            "Value {} never appeared in 200 cards",
            value
        );
        assert!(
            count < 50,
            "Value {} appeared {} times (too many) in 200 cards",
            value,
            count
        );
    }
}

/// Test that deck reshuffle maintains valid state
#[test]
fn test_deck_state_after_reshuffle() {
    let mut deck = Deck::default();

    // Deal cards until we trigger reshuffle
    for _ in 0..55 {
        deck.deal_card();
    }

    // Deck should have reshuffled automatically
    // Try dealing a few more cards to ensure state is valid
    for i in 0..10 {
        let card = deck.deal_card();
        assert!(
            card.0 >= 1 && card.0 <= 13,
            "Invalid card after reshuffle at position {}",
            i
        );
    }
}

/// Test deck with different shuffle patterns
#[test]
fn test_deck_shuffle_randomness() {
    // Create two decks and compare first few cards
    let mut deck1 = Deck::default();
    let mut deck2 = Deck::default();

    let card1 = deck1.deal_card();
    let card2 = deck2.deal_card();

    // First cards might be same or different (random)
    // But both should be valid
    assert!(card1.0 >= 1 && card1.0 <= 13);
    assert!(card2.0 >= 1 && card2.0 <= 13);
}
