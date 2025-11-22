use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use private_poker::{
    GameSettings, PokerState,
    entities::{Card, Suit, Username},
    functional::{argmax, eval},
    game::{GameStateManagement, PhaseIndependentUserManagement},
};

/// Helper to create a game state with N players ready to play
fn setup_game_with_players(n_players: usize) -> PokerState {
    let settings = GameSettings::new(1000, 10, 20, 10, 9);
    let mut game = PokerState::from(settings);

    // Add players as spectators first, then move them to waitlist
    for i in 0..n_players {
        let username = Username::new(&format!("player{}", i));
        game.new_user(&username).unwrap();
        game.waitlist_user(&username).unwrap();
    }

    // Get first player username to start game
    let first_player = Username::new("player0");
    game.init_start(&first_player).unwrap();

    // Step through states until we're in an active game phase
    // Lobby -> SeatPlayers -> MoveButton -> CollectBlinds -> Deal
    for _ in 0..10 {
        let new_game = game.step();
        game = new_game;
    }

    game
}

/// Benchmark hand evaluation with 2 cards (pocket cards)
fn bench_hand_eval_2_cards(c: &mut Criterion) {
    let cards = vec![
        Card(14, Suit::Spade), // Ace
        Card(13, Suit::Spade), // King
    ];

    c.bench_function("hand_eval_2_cards", |b| {
        b.iter(|| eval(&cards));
    });
}

/// Benchmark hand evaluation with 7 cards (full hand + board)
fn bench_hand_eval_7_cards(c: &mut Criterion) {
    let cards = vec![
        Card(14, Suit::Spade),  // Pocket: Ace of Spades
        Card(13, Suit::Spade),  // Pocket: King of Spades
        Card(12, Suit::Spade),  // Board: Queen of Spades
        Card(11, Suit::Spade),  // Board: Jack of Spades
        Card(10, Suit::Spade),  // Board: 10 of Spades (royal flush)
        Card(2, Suit::Heart),   // Board: 2 of Hearts
        Card(3, Suit::Diamond), // Board: 3 of Diamonds
    ];

    c.bench_function("hand_eval_7_cards", |b| {
        b.iter(|| eval(&cards));
    });
}

/// Benchmark hand evaluation 100 times with random-ish hands
fn bench_hand_eval_100_iterations(c: &mut Criterion) {
    // Create 100 different 7-card hands
    let mut all_hands = Vec::new();
    for i in 0..100 {
        let base_value = (i % 13) as u8 + 1;
        let cards = vec![
            Card(base_value, Suit::Spade),
            Card((base_value + 1).min(13), Suit::Heart),
            Card((base_value + 2).min(13), Suit::Diamond),
            Card((base_value + 3).min(13), Suit::Club),
            Card((base_value + 4).min(13), Suit::Spade),
            Card((base_value + 5).min(13), Suit::Heart),
            Card((base_value + 6).min(13), Suit::Diamond),
        ];
        all_hands.push(cards);
    }

    c.bench_function("hand_eval_100_iterations", |b| {
        b.iter(|| {
            all_hands
                .iter()
                .map(|cards| eval(cards))
                .collect::<Vec<_>>()
        });
    });
}

/// Benchmark hand comparison (argmax) with multiple hands
fn bench_hand_comparison(c: &mut Criterion) {
    let hands = vec![
        // High card
        eval(&[
            Card(2, Suit::Club),
            Card(5, Suit::Heart),
            Card(9, Suit::Diamond),
        ]),
        // Pair
        eval(&[
            Card(2, Suit::Club),
            Card(2, Suit::Heart),
            Card(9, Suit::Diamond),
        ]),
        // Two pair
        eval(&[
            Card(2, Suit::Club),
            Card(2, Suit::Heart),
            Card(9, Suit::Diamond),
            Card(9, Suit::Club),
        ]),
        // Three of a kind
        eval(&[
            Card(2, Suit::Club),
            Card(2, Suit::Heart),
            Card(2, Suit::Diamond),
        ]),
    ];

    c.bench_function("hand_comparison_4_hands", |b| {
        b.iter(|| argmax(&hands));
    });
}

/// Benchmark view generation with different player counts
fn bench_view_generation(c: &mut Criterion) {
    let mut group = c.benchmark_group("view_generation");

    for n_players in [2, 4, 6, 8, 10].iter() {
        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{}_players", n_players)),
            n_players,
            |b, &n| {
                let game = setup_game_with_players(n);
                b.iter(|| game.get_views());
            },
        );
    }

    group.finish();
}

/// Benchmark full game state step with different player counts
fn bench_game_step(c: &mut Criterion) {
    let mut group = c.benchmark_group("game_step");

    for n_players in [2, 10].iter() {
        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{}_players", n_players)),
            n_players,
            |b, &n| {
                b.iter_batched(
                    || setup_game_with_players(n),
                    |game| {
                        // Take one step in the game (consumes game, returns new game)
                        game.step()
                    },
                    criterion::BatchSize::SmallInput,
                );
            },
        );
    }

    group.finish();
}

/// Benchmark event draining (common operation)
fn bench_drain_events(c: &mut Criterion) {
    c.bench_function("drain_events", |b| {
        b.iter_batched(
            || setup_game_with_players(5),
            |mut g| {
                g.drain_events();
                g
            },
            criterion::BatchSize::SmallInput,
        );
    });
}

criterion_group!(
    hand_evaluation,
    bench_hand_eval_2_cards,
    bench_hand_eval_7_cards,
    bench_hand_eval_100_iterations,
    bench_hand_comparison,
);

criterion_group!(
    game_operations,
    bench_view_generation,
    bench_game_step,
    bench_drain_events,
);

criterion_main!(hand_evaluation, game_operations);
