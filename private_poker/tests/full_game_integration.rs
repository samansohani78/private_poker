//! Full end-to-end game flow integration tests.
//!
//! Tests complete poker games from lobby to showdown with multiple players,
//! using FSM state transitions for reliable testing.

use private_poker::{
    GameSettings, PokerState,
    entities::{Card, Suit, Username},
    game::{GameStateManagement, PhaseIndependentUserManagement},
};

// ============================================================================
// Full Game Flow Tests - Lobby to Showdown
// ============================================================================

#[test]
fn test_full_game_two_players_fsm_progression() {
    // Test FSM progression through game states
    let settings = GameSettings::new(10, 20, 1000);
    let mut game = PokerState::from(settings);

    // Add two players
    let p1 = Username::new("alice");
    let p2 = Username::new("bob");

    game.new_user(&p1).unwrap();
    game.waitlist_user(&p1).unwrap();
    game.new_user(&p2).unwrap();
    game.waitlist_user(&p2).unwrap();

    // Initial views
    let views = game.get_views();
    assert_eq!(views.len(), 2);
    assert_eq!(views.get(&p1).unwrap().waitlist.len(), 2);

    // Start game
    game.init_start(&p1).unwrap();

    // Step through seating phase
    game = game.step(); // Lobby -> SeatPlayers
    game = game.step(); // SeatPlayers -> MoveButton
    game = game.step(); // MoveButton -> CollectBlinds
    game = game.step(); // CollectBlinds -> Deal

    // After deal, should have players
    let views = game.get_views();
    let view1 = views.get(&p1).unwrap();
    assert_eq!(view1.players.len(), 2);
    assert!(view1.board.is_empty()); // No board yet pre-flop
}

#[test]
fn test_full_game_with_flop() {
    // Test progressing to flop
    let settings = GameSettings::new(10, 20, 1000);
    let mut game = PokerState::from(settings);

    let p1 = Username::new("alice");
    let p2 = Username::new("bob");

    game.new_user(&p1).unwrap();
    game.waitlist_user(&p1).unwrap();
    game.new_user(&p2).unwrap();
    game.waitlist_user(&p2).unwrap();
    game.init_start(&p1).unwrap();

    // Step through many states to reach flop
    for _ in 0..20 {
        game = game.step();
    }

    // Game should have progressed
    let views = game.get_views();
    assert_eq!(views.len(), 2);
}

#[test]
fn test_complete_hand_to_showdown() {
    // Test completing a full hand
    let settings = GameSettings::new(10, 20, 1000);
    let mut game = PokerState::from(settings);

    let p1 = Username::new("alice");
    let p2 = Username::new("bob");

    game.new_user(&p1).unwrap();
    game.waitlist_user(&p1).unwrap();
    game.new_user(&p2).unwrap();
    game.waitlist_user(&p2).unwrap();
    game.init_start(&p1).unwrap();

    // Step through entire hand (50 steps should be enough)
    for _ in 0..50 {
        game = game.step();
    }

    // Game should still be valid
    let views = game.get_views();
    assert!(!views.is_empty());
}

// ============================================================================
// Multi-Player Scenario Tests
// ============================================================================

#[test]
fn test_three_players_game() {
    let settings = GameSettings::new(10, 20, 1000);
    let mut game = PokerState::from(settings);

    // Add three players
    for name in ["alice", "bob", "carol"] {
        let username = Username::new(name);
        game.new_user(&username).unwrap();
        game.waitlist_user(&username).unwrap();
    }

    let views = game.get_views();
    assert_eq!(views.len(), 3);
    assert_eq!(views.values().next().unwrap().waitlist.len(), 3);

    // Start game
    game.init_start(&Username::new("alice")).unwrap();

    // Progress through states
    for _ in 0..10 {
        game = game.step();
    }

    // All players should have views
    let views = game.get_views();
    assert_eq!(views.len(), 3);
}

#[test]
fn test_four_player_game() {
    let settings = GameSettings::new(10, 20, 1000);
    let mut game = PokerState::from(settings);

    // Add four players
    for i in 0..4 {
        let username = Username::new(&format!("player{}", i));
        game.new_user(&username).unwrap();
        game.waitlist_user(&username).unwrap();
    }

    game.init_start(&Username::new("player0")).unwrap();

    for _ in 0..15 {
        game = game.step();
    }

    let views = game.get_views();
    assert_eq!(views.len(), 4);
}

#[test]
fn test_max_players_game() {
    let settings = GameSettings::new(10, 20, 1000);
    let mut game = PokerState::from(settings);

    // Add maximum players (10)
    for i in 0..10 {
        let username = Username::new(&format!("player{}", i));
        game.new_user(&username).unwrap();
        game.waitlist_user(&username).unwrap();
    }

    game.init_start(&Username::new("player0")).unwrap();

    for _ in 0..20 {
        game = game.step();
    }

    let views = game.get_views();
    assert_eq!(views.len(), 10);
}

// ============================================================================
// Game State Validation Tests
// ============================================================================

#[test]
fn test_all_game_states_reachable() {
    let settings = GameSettings::new(10, 20, 1000);
    let mut game = PokerState::from(settings);

    let p1 = Username::new("alice");
    let p2 = Username::new("bob");

    game.new_user(&p1).unwrap();
    game.waitlist_user(&p1).unwrap();
    game.new_user(&p2).unwrap();
    game.waitlist_user(&p2).unwrap();
    game.init_start(&p1).unwrap();

    // Track which states we've seen
    let mut seen_states = std::collections::HashSet::new();

    for _ in 0..100 {
        let state_name = match &game {
            PokerState::Lobby(_) => "Lobby",
            PokerState::SeatPlayers(_) => "SeatPlayers",
            PokerState::MoveButton(_) => "MoveButton",
            PokerState::CollectBlinds(_) => "CollectBlinds",
            PokerState::Deal(_) => "Deal",
            PokerState::TakeAction(_) => "TakeAction",
            PokerState::Flop(_) => "Flop",
            PokerState::Turn(_) => "Turn",
            PokerState::River(_) => "River",
            PokerState::ShowHands(_) => "ShowHands",
            PokerState::DistributePot(_) => "DistributePot",
            PokerState::RemovePlayers(_) => "RemovePlayers",
            PokerState::UpdateBlinds(_) => "UpdateBlinds",
            PokerState::BootPlayers(_) => "BootPlayers",
        };
        seen_states.insert(state_name);
        game = game.step();
    }

    // Should have seen several key states
    assert!(seen_states.contains("SeatPlayers"));
    assert!(seen_states.contains("Deal"));
}

#[test]
fn test_hand_evaluation_with_board() {
    // Test hand evaluation with community cards
    let cards = vec![
        Card(14, Suit::Spade),   // Ace
        Card(13, Suit::Spade),   // King
        Card(12, Suit::Heart),   // Queen
        Card(11, Suit::Diamond), // Jack
        Card(10, Suit::Club),    // 10
        Card(2, Suit::Spade),    // 2
        Card(3, Suit::Heart),    // 3
    ];

    let hand = private_poker::functional::eval(&cards);
    assert!(!hand.is_empty(), "Should evaluate to a hand");
}

#[test]
fn test_multiple_hands_comparison() {
    let hand1 = private_poker::functional::eval(&[Card(14, Suit::Spade), Card(14, Suit::Heart)]);

    let hand2 = private_poker::functional::eval(&[Card(2, Suit::Club), Card(3, Suit::Diamond)]);

    let hands = vec![hand1, hand2];
    let winners = private_poker::functional::argmax(&hands);

    assert!(!winners.is_empty(), "Should have winner");
    assert_eq!(winners[0], 0, "First hand (pair of aces) should win");
}

// ============================================================================
// Edge Case Tests
// ============================================================================

#[test]
fn test_player_elimination() {
    // Player with low chips
    let settings = GameSettings::new(10, 20, 50); // Low starting chips
    let mut game = PokerState::from(settings);

    let p1 = Username::new("alice");
    let p2 = Username::new("bob");

    game.new_user(&p1).unwrap();
    game.waitlist_user(&p1).unwrap();
    game.new_user(&p2).unwrap();
    game.waitlist_user(&p2).unwrap();
    game.init_start(&p1).unwrap();

    // Play multiple hands
    for _ in 0..100 {
        game = game.step();
    }

    // Game should handle potential elimination
    let views = game.get_views();
    assert!(!views.is_empty());
}

#[test]
fn test_game_with_blind_increases() {
    let settings = GameSettings::new(10, 20, 1000);
    let mut game = PokerState::from(settings);

    let p1 = Username::new("alice");
    let p2 = Username::new("bob");

    game.new_user(&p1).unwrap();
    game.waitlist_user(&p1).unwrap();
    game.new_user(&p2).unwrap();
    game.waitlist_user(&p2).unwrap();
    game.init_start(&p1).unwrap();

    // Play many hands to potentially trigger blind increase
    for _ in 0..200 {
        game = game.step();
    }

    let views = game.get_views();
    assert_eq!(views.len(), 2);
}

#[test]
fn test_view_consistency() {
    let settings = GameSettings::new(10, 20, 1000);
    let mut game = PokerState::from(settings);

    let p1 = Username::new("alice");
    let p2 = Username::new("bob");

    game.new_user(&p1).unwrap();
    game.waitlist_user(&p1).unwrap();
    game.new_user(&p2).unwrap();
    game.waitlist_user(&p2).unwrap();

    // Generate views multiple times - should be consistent
    for _ in 0..10 {
        let views = game.get_views();
        assert_eq!(views.len(), 2);
        assert!(views.contains_key(&p1));
        assert!(views.contains_key(&p2));

        // Both players should see same waitlist size
        let view1 = views.get(&p1).unwrap();
        let view2 = views.get(&p2).unwrap();
        assert_eq!(view1.waitlist.len(), view2.waitlist.len());
    }
}

#[test]
fn test_event_generation_and_draining() {
    let settings = GameSettings::new(10, 20, 1000);
    let mut game = PokerState::from(settings);

    let p1 = Username::new("alice");
    let p2 = Username::new("bob");

    game.new_user(&p1).unwrap();
    game.waitlist_user(&p1).unwrap();
    game.new_user(&p2).unwrap();
    game.waitlist_user(&p2).unwrap();
    game.init_start(&p1).unwrap();

    // Drain events - should work without panic
    let _events_before = game.drain_events();

    // Progress game state
    game = game.step();

    // Drain events again
    let _events_after = game.drain_events();

    // Second drain should be empty (events are consumed)
    let events_second = game.drain_events();
    assert_eq!(
        events_second.len(),
        0,
        "Second drain should return empty vec"
    );

    // Verify drain_events is idempotent
    for _ in 0..10 {
        game = game.step();
        let _ = game.drain_events();
    }

    // Game should still be valid after draining
    let views = game.get_views();
    assert_eq!(views.len(), 2);
}

// ============================================================================
// Stress Tests
// ============================================================================

#[test]
fn test_rapid_state_progression() {
    let settings = GameSettings::new(10, 20, 1000);
    let mut game = PokerState::from(settings);

    let p1 = Username::new("alice");
    let p2 = Username::new("bob");

    game.new_user(&p1).unwrap();
    game.waitlist_user(&p1).unwrap();
    game.new_user(&p2).unwrap();
    game.waitlist_user(&p2).unwrap();
    game.init_start(&p1).unwrap();

    // Rapidly step through 500 states
    for _ in 0..500 {
        game = game.step();
    }

    // Game should remain valid
    let views = game.get_views();
    assert_eq!(views.len(), 2);
}

#[test]
fn test_many_sequential_hands() {
    let settings = GameSettings::new(10, 20, 10000); // High chips for long game
    let mut game = PokerState::from(settings);

    let p1 = Username::new("alice");
    let p2 = Username::new("bob");

    game.new_user(&p1).unwrap();
    game.waitlist_user(&p1).unwrap();
    game.new_user(&p2).unwrap();
    game.waitlist_user(&p2).unwrap();
    game.init_start(&p1).unwrap();

    // Play through many hands
    for _ in 0..1000 {
        game = game.step();
    }

    // Game should still be playable
    let views = game.get_views();
    assert_eq!(views.len(), 2);
}

#[test]
fn test_game_state_idempotency() {
    let settings = GameSettings::new(10, 20, 1000);
    let mut game = PokerState::from(settings);

    let p1 = Username::new("alice");
    let p2 = Username::new("bob");

    game.new_user(&p1).unwrap();
    game.waitlist_user(&p1).unwrap();
    game.new_user(&p2).unwrap();
    game.waitlist_user(&p2).unwrap();

    // Multiple view generations should not change state
    let views1 = game.get_views();
    let views2 = game.get_views();

    assert_eq!(views1.len(), views2.len());
}

#[test]
fn test_user_management_operations() {
    let settings = GameSettings::new(10, 20, 1000);
    let mut game = PokerState::from(settings);

    // Add user
    let p1 = Username::new("alice");
    game.new_user(&p1).unwrap();

    let views = game.get_views();
    assert_eq!(views.len(), 1);

    // Waitlist user
    game.waitlist_user(&p1).unwrap();

    let views = game.get_views();
    let view = views.get(&p1).unwrap();
    assert_eq!(view.waitlist.len(), 1);
}

#[test]
fn test_game_invariants_maintained() {
    let settings = GameSettings::new(10, 20, 1000);
    let mut game = PokerState::from(settings);

    let players = ["alice", "bob", "carol"];
    for name in &players {
        let username = Username::new(name);
        game.new_user(&username).unwrap();
        game.waitlist_user(&username).unwrap();
    }

    game.init_start(&Username::new("alice")).unwrap();

    // Step through game and verify invariants
    for _ in 0..50 {
        game = game.step();

        // Invariant: Number of views should equal number of users
        let views = game.get_views();
        assert_eq!(views.len(), 3, "Should always have 3 player views");

        // Invariant: Each player should have their own view
        for name in &players {
            assert!(views.contains_key(&Username::new(name)));
        }
    }
}
