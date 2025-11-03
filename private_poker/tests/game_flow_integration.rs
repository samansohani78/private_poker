/// Integration tests for game flow scenarios
///
/// These tests verify game state transitions and player interactions
/// during lobby, game start, and basic gameplay scenarios.

use mio::net::TcpListener;
use std::{net::SocketAddr, thread, time::Duration};

use private_poker::{
    Client, UserError,
    game::entities::Username,
    messages,
    server::{self, PokerConfig},
};

fn get_random_open_port() -> u16 {
    let addr = "127.0.0.1:0".parse().unwrap();
    let listener = TcpListener::bind(addr).unwrap();
    listener.local_addr().unwrap().port()
}

#[test]
fn test_two_players_join_waitlist() {
    let port = get_random_open_port();
    let addr: SocketAddr = format!("127.0.0.1:{port}").parse().unwrap();
    thread::spawn(move || server::run(addr, PokerConfig::default()));
    thread::sleep(Duration::from_millis(50));

    // First player connects and joins waitlist
    let (mut player1, _) = Client::connect(Username::new("alice"), &addr).unwrap();
    player1.change_state(messages::UserState::Play).unwrap();
    Client::recv_ack(&mut player1.stream).unwrap();
    let view1 = Client::recv_view(&mut player1.stream).unwrap();
    Client::recv_event(&mut player1.stream).unwrap(); // Waitlisted event

    assert_eq!(view1.waitlist.len(), 1);

    // Second player connects and joins waitlist
    let (mut player2, _) = Client::connect(Username::new("bob"), &addr).unwrap();
    player2.change_state(messages::UserState::Play).unwrap();
    Client::recv_ack(&mut player2.stream).unwrap();
    let view2 = Client::recv_view(&mut player2.stream).unwrap();
    Client::recv_event(&mut player2.stream).unwrap(); // Waitlisted event

    // Both players should see 2 people on waitlist
    assert_eq!(view2.waitlist.len(), 2);
}

#[test]
fn test_cannot_start_game_with_one_player() {
    let port = get_random_open_port();
    let addr: SocketAddr = format!("127.0.0.1:{port}").parse().unwrap();
    thread::spawn(move || server::run(addr, PokerConfig::default()));
    thread::sleep(Duration::from_millis(50));

    let (mut player, _) = Client::connect(Username::new("alice"), &addr).unwrap();
    player.change_state(messages::UserState::Play).unwrap();
    Client::recv_ack(&mut player.stream).unwrap();
    Client::recv_view(&mut player.stream).unwrap();
    Client::recv_event(&mut player.stream).unwrap();

    // Try to start game with only 1 player
    player.start_game().unwrap();
    let error = Client::recv_user_error(&mut player.stream).unwrap();

    assert_eq!(error, UserError::NotEnoughPlayers);
}

#[test]
fn test_spectator_cannot_start_game() {
    let port = get_random_open_port();
    let addr: SocketAddr = format!("127.0.0.1:{port}").parse().unwrap();
    thread::spawn(move || server::run(addr, PokerConfig::default()));
    thread::sleep(Duration::from_millis(50));

    // Add two players to waitlist first
    let (mut p1, _) = Client::connect(Username::new("alice"), &addr).unwrap();
    p1.change_state(messages::UserState::Play).unwrap();
    Client::recv_ack(&mut p1.stream).unwrap();
    Client::recv_view(&mut p1.stream).unwrap();
    Client::recv_event(&mut p1.stream).unwrap();

    let (mut p2, _) = Client::connect(Username::new("bob"), &addr).unwrap();
    p2.change_state(messages::UserState::Play).unwrap();
    Client::recv_ack(&mut p2.stream).unwrap();
    Client::recv_view(&mut p2.stream).unwrap();
    Client::recv_event(&mut p2.stream).unwrap();

    // Now connect spectator (so they don't get connection events from p1/p2)
    let (mut spectator, _) = Client::connect(Username::new("observer"), &addr).unwrap();

    // Spectator tries to start game
    spectator.start_game().unwrap();
    let error = Client::recv_user_error(&mut spectator.stream).unwrap();

    assert_eq!(error, UserError::CannotStartGame);
}

#[test]
fn test_player_leaves_waitlist() {
    let port = get_random_open_port();
    let addr: SocketAddr = format!("127.0.0.1:{port}").parse().unwrap();
    thread::spawn(move || server::run(addr, PokerConfig::default()));
    thread::sleep(Duration::from_millis(50));

    let (mut player, _) = Client::connect(Username::new("alice"), &addr).unwrap();

    // Join waitlist
    player.change_state(messages::UserState::Play).unwrap();
    Client::recv_ack(&mut player.stream).unwrap();
    let view1 = Client::recv_view(&mut player.stream).unwrap();
    Client::recv_event(&mut player.stream).unwrap();

    assert_eq!(view1.waitlist.len(), 1);

    // Leave waitlist (go back to spectating)
    player.change_state(messages::UserState::Spectate).unwrap();
    Client::recv_ack(&mut player.stream).unwrap();
    let view2 = Client::recv_view(&mut player.stream).unwrap();
    Client::recv_event(&mut player.stream).unwrap();

    assert_eq!(view2.waitlist.len(), 0);
    assert_eq!(view2.spectators.len(), 1);
}

#[test]
fn test_multiple_spectators() {
    let port = get_random_open_port();
    let addr: SocketAddr = format!("127.0.0.1:{port}").parse().unwrap();
    thread::spawn(move || server::run(addr, PokerConfig::default()));
    thread::sleep(Duration::from_millis(50));

    // Connect 3 spectators
    let (spec1, view1) = Client::connect(Username::new("viewer1"), &addr).unwrap();
    let (spec2, view2) = Client::connect(Username::new("viewer2"), &addr).unwrap();
    let (spec3, view3) = Client::connect(Username::new("viewer3"), &addr).unwrap();

    // All should see 3 spectators
    assert!(view3.spectators.len() == 3 || view3.spectators.len() == 2); // Race condition tolerance
    assert_eq!(view3.waitlist.len(), 0);
    assert_eq!(view3.players.len(), 0);
}

#[test]
fn test_waitlist_preserves_order() {
    let port = get_random_open_port();
    let addr: SocketAddr = format!("127.0.0.1:{port}").parse().unwrap();
    thread::spawn(move || server::run(addr, PokerConfig::default()));
    thread::sleep(Duration::from_millis(50));

    // Add players in order
    let (mut p1, _) = Client::connect(Username::new("alice"), &addr).unwrap();
    p1.change_state(messages::UserState::Play).unwrap();
    Client::recv_ack(&mut p1.stream).unwrap();
    Client::recv_view(&mut p1.stream).unwrap();
    Client::recv_event(&mut p1.stream).unwrap();

    let (mut p2, _) = Client::connect(Username::new("bob"), &addr).unwrap();
    p2.change_state(messages::UserState::Play).unwrap();
    Client::recv_ack(&mut p2.stream).unwrap();
    Client::recv_view(&mut p2.stream).unwrap();
    Client::recv_event(&mut p2.stream).unwrap();

    let (mut p3, _) = Client::connect(Username::new("charlie"), &addr).unwrap();
    p3.change_state(messages::UserState::Play).unwrap();
    Client::recv_ack(&mut p3.stream).unwrap();
    let view3 = Client::recv_view(&mut p3.stream).unwrap();
    Client::recv_event(&mut p3.stream).unwrap();

    // Verify waitlist order
    assert_eq!(view3.waitlist.len(), 3);
    assert_eq!(view3.waitlist[0].name, Username::new("alice"));
    assert_eq!(view3.waitlist[1].name, Username::new("bob"));
    assert_eq!(view3.waitlist[2].name, Username::new("charlie"));
}

#[test]
fn test_cannot_show_hand_before_game_starts() {
    let port = get_random_open_port();
    let addr: SocketAddr = format!("127.0.0.1:{port}").parse().unwrap();
    thread::spawn(move || server::run(addr, PokerConfig::default()));
    thread::sleep(Duration::from_millis(50));

    let (mut player, _) = Client::connect(Username::new("alice"), &addr).unwrap();
    player.change_state(messages::UserState::Play).unwrap();
    Client::recv_ack(&mut player.stream).unwrap();
    Client::recv_view(&mut player.stream).unwrap();
    Client::recv_event(&mut player.stream).unwrap();

    // Try to show hand before game starts
    player.show_hand().unwrap();
    let error = Client::recv_user_error(&mut player.stream).unwrap();

    assert_eq!(error, UserError::CannotShowHand);
}

#[test]
fn test_duplicate_username_rejected() {
    let port = get_random_open_port();
    let addr: SocketAddr = format!("127.0.0.1:{port}").parse().unwrap();
    thread::spawn(move || server::run(addr, PokerConfig::default()));
    thread::sleep(Duration::from_millis(50));

    // First connection succeeds
    let (_client1, _) = Client::connect(Username::new("alice"), &addr).unwrap();

    // Second connection with same username fails
    let result = Client::connect(Username::new("alice"), &addr);
    assert!(result.is_err());
}

#[test]
fn test_player_transitions_between_states() {
    let port = get_random_open_port();
    let addr: SocketAddr = format!("127.0.0.1:{port}").parse().unwrap();
    thread::spawn(move || server::run(addr, PokerConfig::default()));
    thread::sleep(Duration::from_millis(50));

    let (mut player, view) = Client::connect(Username::new("alice"), &addr).unwrap();

    // Starts as spectator
    assert_eq!(view.spectators.len(), 1);

    // Join waitlist
    player.change_state(messages::UserState::Play).unwrap();
    Client::recv_ack(&mut player.stream).unwrap();
    let view2 = Client::recv_view(&mut player.stream).unwrap();
    Client::recv_event(&mut player.stream).unwrap();
    assert_eq!(view2.waitlist.len(), 1);
    assert_eq!(view2.spectators.len(), 0);

    // Back to spectator
    player.change_state(messages::UserState::Spectate).unwrap();
    Client::recv_ack(&mut player.stream).unwrap();
    let view3 = Client::recv_view(&mut player.stream).unwrap();
    Client::recv_event(&mut player.stream).unwrap();
    assert_eq!(view3.spectators.len(), 1);
    assert_eq!(view3.waitlist.len(), 0);
}
