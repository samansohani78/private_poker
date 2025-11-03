//! Hand Evaluation Example
//!
//! Demonstrates how to use the hand evaluation functions to compare poker hands.

use private_poker::functional::{argmax, eval};
use private_poker::entities::{Card, Rank, Suit};

fn main() {
    println!("=== Poker Hand Evaluation Example ===\n");

    // Example 1: Evaluate a single hand
    println!("Example 1: Evaluating a 7-card hand");
    let hand1 = vec![
        Card(14, Suit::Heart),   // Ace of Hearts
        Card(13, Suit::Heart),   // King of Hearts
        Card(12, Suit::Heart),   // Queen of Hearts
        Card(11, Suit::Heart),   // Jack of Hearts
        Card(10, Suit::Heart),   // Ten of Hearts
        Card(9, Suit::Spade),    // Nine of Spades
        Card(2, Suit::Club),     // Two of Clubs
    ];

    let result = eval(&hand1);
    println!("Hand: {:?}", hand1);
    println!("Best 5-card hand: {:?}", result);
    println!("Rank: {:?}\n", result.first().map(|h| h.rank));

    // Example 2: Compare two hands
    println!("Example 2: Comparing two hands");

    let hand_a = vec![
        Card(14, Suit::Spade),   // Pair of Aces
        Card(14, Suit::Heart),
        Card(10, Suit::Club),
        Card(9, Suit::Diamond),
        Card(2, Suit::Spade),
    ];

    let hand_b = vec![
        Card(13, Suit::Spade),   // Pair of Kings
        Card(13, Suit::Heart),
        Card(10, Suit::Club),
        Card(9, Suit::Diamond),
        Card(2, Suit::Spade),
    ];

    let eval_a = eval(&hand_a);
    let eval_b = eval(&hand_b);

    println!("Hand A: {:?}", hand_a);
    println!("Evaluated: {:?}", eval_a);
    println!("\nHand B: {:?}", hand_b);
    println!("Evaluated: {:?}", eval_b);

    let winner_indices = argmax(&[eval_a.clone(), eval_b.clone()]);
    match winner_indices.as_slice() {
        [0] => println!("\nWinner: Hand A (Pair of Aces)"),
        [1] => println!("\nWinner: Hand B (Pair of Kings)"),
        _ => println!("\nTie!"),
    }

    // Example 3: Multiple hands with a tie
    println!("\n\nExample 3: Three-way comparison with a tie");

    let hands = vec![
        vec![
            Card(10, Suit::Heart),
            Card(10, Suit::Diamond),
            Card(5, Suit::Club),
            Card(3, Suit::Spade),
            Card(2, Suit::Heart),
        ],
        vec![
            Card(10, Suit::Spade),
            Card(10, Suit::Club),
            Card(5, Suit::Heart),
            Card(3, Suit::Diamond),
            Card(2, Suit::Club),
        ],
        vec![
            Card(9, Suit::Heart),
            Card(9, Suit::Diamond),
            Card(5, Suit::Club),
            Card(3, Suit::Spade),
            Card(2, Suit::Heart),
        ],
    ];

    let evaluations: Vec<_> = hands.iter().map(|h| eval(h)).collect();

    for (i, (hand, eval)) in hands.iter().zip(&evaluations).enumerate() {
        println!("Hand {}: {:?}", i + 1, hand);
        println!("  Rank: {:?}", eval.first().map(|h| h.rank));
    }

    let winners = argmax(&evaluations);
    println!("\nWinner(s): Hands {:?}", winners.iter().map(|&i| i + 1).collect::<Vec<_>>());

    // Example 4: All hand types
    println!("\n\nExample 4: Examples of each hand rank");

    let examples = vec![
        ("Royal Flush", vec![
            Card(14, Suit::Spade),
            Card(13, Suit::Spade),
            Card(12, Suit::Spade),
            Card(11, Suit::Spade),
            Card(10, Suit::Spade),
        ]),
        ("Straight Flush", vec![
            Card(9, Suit::Heart),
            Card(8, Suit::Heart),
            Card(7, Suit::Heart),
            Card(6, Suit::Heart),
            Card(5, Suit::Heart),
        ]),
        ("Four of a Kind", vec![
            Card(8, Suit::Spade),
            Card(8, Suit::Heart),
            Card(8, Suit::Diamond),
            Card(8, Suit::Club),
            Card(2, Suit::Spade),
        ]),
        ("Full House", vec![
            Card(10, Suit::Spade),
            Card(10, Suit::Heart),
            Card(10, Suit::Diamond),
            Card(6, Suit::Club),
            Card(6, Suit::Spade),
        ]),
        ("Flush", vec![
            Card(13, Suit::Club),
            Card(11, Suit::Club),
            Card(8, Suit::Club),
            Card(5, Suit::Club),
            Card(3, Suit::Club),
        ]),
        ("Straight", vec![
            Card(10, Suit::Spade),
            Card(9, Suit::Heart),
            Card(8, Suit::Diamond),
            Card(7, Suit::Club),
            Card(6, Suit::Spade),
        ]),
        ("Three of a Kind", vec![
            Card(7, Suit::Spade),
            Card(7, Suit::Heart),
            Card(7, Suit::Diamond),
            Card(12, Suit::Club),
            Card(3, Suit::Spade),
        ]),
        ("Two Pair", vec![
            Card(12, Suit::Spade),
            Card(12, Suit::Heart),
            Card(5, Suit::Diamond),
            Card(5, Suit::Club),
            Card(2, Suit::Spade),
        ]),
        ("One Pair", vec![
            Card(9, Suit::Spade),
            Card(9, Suit::Heart),
            Card(13, Suit::Diamond),
            Card(7, Suit::Club),
            Card(4, Suit::Spade),
        ]),
        ("High Card", vec![
            Card(14, Suit::Spade),
            Card(12, Suit::Heart),
            Card(10, Suit::Diamond),
            Card(7, Suit::Club),
            Card(3, Suit::Spade),
        ]),
    ];

    for (name, hand) in examples {
        let evaluation = eval(&hand);
        println!("{}: {:?}", name, evaluation.first().map(|h| h.rank).unwrap_or(Rank::HighCard));
    }

    println!("\n=== End of Hand Evaluation Example ===");
}
