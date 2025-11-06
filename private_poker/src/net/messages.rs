use serde::{Deserialize, Serialize};
use std::fmt;

use super::super::game::{
    GameEvent, UserError,
    entities::{Action, ActionChoices, GameView, Username, Vote},
};

/// Errors due to the poker client's interaction with the poker server
/// and not from the user's particular action.
#[derive(Debug, Deserialize, Eq, thiserror::Error, PartialEq, Serialize)]
pub enum ClientError {
    #[error("already associated")]
    AlreadyAssociated,
    #[error("does not exist")]
    DoesNotExist,
    #[error("expired")]
    Expired,
    #[error("unassociated")]
    Unassociated,
}

/// Type of user state change requests.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum UserState {
    Play,
    Spectate,
}

impl fmt::Display for UserState {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let repr = match self {
            Self::Play => "waitlister",
            Self::Spectate => "spectator",
        };
        write!(f, "{repr}")
    }
}

/// A user command.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum UserCommand {
    /// The user wants to change their state (play or spectate).
    ChangeState(UserState),
    /// A new user wants to connect to the game.
    Connect,
    /// User disconnected. This is really just a
    /// friendly courtesy and doesn't need to be sent by
    /// clients.
    Disconnect,
    /// User wants to show their hand. Can only occur if they're
    /// a player and the game is in a state that allows hands to
    /// be shown.
    ShowHand,
    /// User wants to start the game. Can only start a game when
    /// there are 2+ potential players.
    StartGame,
    /// User wants to make a bet. Can only occur if they're a
    /// player and it's their turn.
    TakeAction(Action),
    /// User wants to cast a vote.
    CastVote(Vote),
}

impl fmt::Display for UserCommand {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let repr = match &self {
            Self::ChangeState(state) => &format!("requested to join the {state}s"),
            Self::Connect => "connected",
            Self::Disconnect => "disconnected",
            Self::ShowHand => "showed their hand",
            Self::StartGame => "started the game",
            Self::TakeAction(action) => &action.to_string(),
            Self::CastVote(vote) => &format!("voted to {vote}"),
        };
        write!(f, "{repr}")
    }
}

/// A message from a poker client to the poker server, indicating some
/// type of user action or command request.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ClientMessage {
    /// User the message is from.
    pub username: Username,
    /// Action the user is taking.
    pub command: UserCommand,
}

impl fmt::Display for ClientMessage {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{} {}", self.username, self.command)
    }
}

/// A message from the poker server to a poker client.
#[derive(Debug, Deserialize, Serialize)]
pub enum ServerMessage {
    /// An acknowledgement of a client message, signaling that the client's
    /// command was successfully processed by the game thread.
    Ack(ClientMessage),
    /// An indication that the poker client caused an error, resulting in
    /// the client's message not being processed correctly.
    ClientError(ClientError),
    /// An internal game event that can be shared with all clients.
    GameEvent(GameEvent),
    /// The game state as viewed from the client's perspective.
    GameView(GameView),
    /// The game state represented as a string.
    Status(String),
    /// A sginal indicating that it's the user's turn.
    TurnSignal(ActionChoices),
    /// An indication that the poker client sent a message that was read
    /// properly, but the type of action that it relayed was invalid
    /// for the game state, resulting in a user error.
    UserError(UserError),
}

impl fmt::Display for ServerMessage {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let repr = match &self {
            Self::Ack(msg) => msg.to_string(),
            Self::ClientError(error) => error.to_string(),
            Self::GameEvent(event) => event.to_string(),
            Self::GameView(_) => "game view".to_string(),
            Self::Status(status) => status.to_string(),
            Self::TurnSignal(action_choices) => action_choices.to_string(),
            Self::UserError(error) => error.to_string(),
        };
        write!(f, "{repr}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::entities::{Action, Vote};

    // === ClientError Tests ===

    #[test]
    fn test_client_error_already_associated() {
        let error = ClientError::AlreadyAssociated;
        assert_eq!(format!("{}", error), "already associated");
    }

    #[test]
    fn test_client_error_does_not_exist() {
        let error = ClientError::DoesNotExist;
        assert_eq!(format!("{}", error), "does not exist");
    }

    #[test]
    fn test_client_error_expired() {
        let error = ClientError::Expired;
        assert_eq!(format!("{}", error), "expired");
    }

    #[test]
    fn test_client_error_unassociated() {
        let error = ClientError::Unassociated;
        assert_eq!(format!("{}", error), "unassociated");
    }

    #[test]
    fn test_client_error_equality() {
        assert_eq!(ClientError::AlreadyAssociated, ClientError::AlreadyAssociated);
        assert_ne!(ClientError::AlreadyAssociated, ClientError::DoesNotExist);
    }

    #[test]
    fn test_client_error_serialization() {
        let error = ClientError::AlreadyAssociated;
        let serialized = bincode::serialize(&error).unwrap();
        let deserialized: ClientError = bincode::deserialize(&serialized).unwrap();
        assert_eq!(error, deserialized);
    }

    // === UserState Tests ===

    #[test]
    fn test_user_state_play_display() {
        let state = UserState::Play;
        assert_eq!(format!("{}", state), "waitlister");
    }

    #[test]
    fn test_user_state_spectate_display() {
        let state = UserState::Spectate;
        assert_eq!(format!("{}", state), "spectator");
    }

    #[test]
    fn test_user_state_equality() {
        assert_eq!(UserState::Play, UserState::Play);
        assert_ne!(UserState::Play, UserState::Spectate);
    }

    #[test]
    fn test_user_state_clone() {
        let state = UserState::Play;
        let cloned = state.clone();
        assert_eq!(state, cloned);
    }

    #[test]
    fn test_user_state_serialization() {
        let state = UserState::Spectate;
        let serialized = bincode::serialize(&state).unwrap();
        let deserialized: UserState = bincode::deserialize(&serialized).unwrap();
        assert_eq!(state, deserialized);
    }

    // === UserCommand Tests ===

    #[test]
    fn test_user_command_change_state_play() {
        let cmd = UserCommand::ChangeState(UserState::Play);
        assert!(format!("{}", cmd).contains("waitlister"));
    }

    #[test]
    fn test_user_command_change_state_spectate() {
        let cmd = UserCommand::ChangeState(UserState::Spectate);
        assert!(format!("{}", cmd).contains("spectator"));
    }

    #[test]
    fn test_user_command_connect() {
        let cmd = UserCommand::Connect;
        assert_eq!(format!("{}", cmd), "connected");
    }

    #[test]
    fn test_user_command_disconnect() {
        let cmd = UserCommand::Disconnect;
        assert_eq!(format!("{}", cmd), "disconnected");
    }

    #[test]
    fn test_user_command_show_hand() {
        let cmd = UserCommand::ShowHand;
        assert_eq!(format!("{}", cmd), "showed their hand");
    }

    #[test]
    fn test_user_command_start_game() {
        let cmd = UserCommand::StartGame;
        assert_eq!(format!("{}", cmd), "started the game");
    }

    #[test]
    fn test_user_command_take_action_fold() {
        let cmd = UserCommand::TakeAction(Action::Fold);
        assert!(format!("{}", cmd).contains("fold"));
    }

    #[test]
    fn test_user_command_take_action_call() {
        let cmd = UserCommand::TakeAction(Action::Call);
        assert!(format!("{}", cmd).contains("call"));
    }

    #[test]
    fn test_user_command_take_action_all_in() {
        let cmd = UserCommand::TakeAction(Action::AllIn);
        assert!(format!("{}", cmd).contains("all-in"));
    }

    #[test]
    fn test_user_command_cast_vote_kick() {
        let username = Username::new("player1");
        let cmd = UserCommand::CastVote(Vote::Kick(username));
        assert!(format!("{}", cmd).contains("voted to kick"));
    }

    #[test]
    fn test_user_command_cast_vote_reset() {
        let cmd = UserCommand::CastVote(Vote::Reset(None));
        assert!(format!("{}", cmd).contains("voted to reset"));
    }

    #[test]
    fn test_user_command_equality() {
        assert_eq!(UserCommand::Connect, UserCommand::Connect);
        assert_ne!(UserCommand::Connect, UserCommand::Disconnect);
    }

    #[test]
    fn test_user_command_clone() {
        let cmd = UserCommand::StartGame;
        let cloned = cmd.clone();
        assert_eq!(cmd, cloned);
    }

    #[test]
    fn test_user_command_serialization_connect() {
        let cmd = UserCommand::Connect;
        let serialized = bincode::serialize(&cmd).unwrap();
        let deserialized: UserCommand = bincode::deserialize(&serialized).unwrap();
        assert_eq!(cmd, deserialized);
    }

    #[test]
    fn test_user_command_serialization_take_action() {
        let cmd = UserCommand::TakeAction(Action::Raise(Some(100)));
        let serialized = bincode::serialize(&cmd).unwrap();
        let deserialized: UserCommand = bincode::deserialize(&serialized).unwrap();
        assert_eq!(cmd, deserialized);
    }

    // === ClientMessage Tests ===

    #[test]
    fn test_client_message_creation() {
        let username = Username::new("alice");
        let msg = ClientMessage {
            username: username.clone(),
            command: UserCommand::Connect,
        };
        assert_eq!(msg.username, username);
        assert_eq!(msg.command, UserCommand::Connect);
    }

    #[test]
    fn test_client_message_display() {
        let username = Username::new("bob");
        let msg = ClientMessage {
            username,
            command: UserCommand::StartGame,
        };
        let display = format!("{}", msg);
        assert!(display.contains("bob"));
        assert!(display.contains("started the game"));
    }

    #[test]
    fn test_client_message_clone() {
        let msg = ClientMessage {
            username: Username::new("charlie"),
            command: UserCommand::Disconnect,
        };
        let cloned = msg.clone();
        assert_eq!(msg.username, cloned.username);
        assert_eq!(msg.command, cloned.command);
    }

    #[test]
    fn test_client_message_serialization() {
        let msg = ClientMessage {
            username: Username::new("dave"),
            command: UserCommand::TakeAction(Action::Check),
        };
        let serialized = bincode::serialize(&msg).unwrap();
        let deserialized: ClientMessage = bincode::deserialize(&serialized).unwrap();
        assert_eq!(msg.username, deserialized.username);
        assert_eq!(msg.command, deserialized.command);
    }

    #[test]
    fn test_client_message_with_vote() {
        let msg = ClientMessage {
            username: Username::new("eve"),
            command: UserCommand::CastVote(Vote::Reset(Some(Username::new("frank")))),
        };
        let display = format!("{}", msg);
        assert!(display.contains("eve"));
        assert!(display.contains("voted to reset"));
    }

    // === ServerMessage Tests ===

    #[test]
    fn test_server_message_ack() {
        let client_msg = ClientMessage {
            username: Username::new("alice"),
            command: UserCommand::Connect,
        };
        let server_msg = ServerMessage::Ack(client_msg.clone());
        let display = format!("{}", server_msg);
        assert!(display.contains("alice"));
    }

    #[test]
    fn test_server_message_client_error() {
        let server_msg = ServerMessage::ClientError(ClientError::AlreadyAssociated);
        assert_eq!(format!("{}", server_msg), "already associated");
    }

    #[test]
    fn test_server_message_status() {
        let server_msg = ServerMessage::Status("Game in progress".to_string());
        assert_eq!(format!("{}", server_msg), "Game in progress");
    }

    #[test]
    fn test_server_message_user_error() {
        let server_msg = ServerMessage::UserError(UserError::NotEnoughPlayers);
        assert!(format!("{}", server_msg).len() > 0);
    }

    #[test]
    fn test_server_message_serialization_ack() {
        let client_msg = ClientMessage {
            username: Username::new("test"),
            command: UserCommand::Connect,
        };
        let server_msg = ServerMessage::Ack(client_msg);
        let serialized = bincode::serialize(&server_msg).unwrap();
        let _deserialized: ServerMessage = bincode::deserialize(&serialized).unwrap();
    }

    #[test]
    fn test_server_message_serialization_client_error() {
        let server_msg = ServerMessage::ClientError(ClientError::Expired);
        let serialized = bincode::serialize(&server_msg).unwrap();
        let _deserialized: ServerMessage = bincode::deserialize(&serialized).unwrap();
    }

    #[test]
    fn test_server_message_serialization_status() {
        let server_msg = ServerMessage::Status("test status".to_string());
        let serialized = bincode::serialize(&server_msg).unwrap();
        let _deserialized: ServerMessage = bincode::deserialize(&serialized).unwrap();
    }

    #[test]
    fn test_all_client_errors_unique() {
        let errors = vec![
            ClientError::AlreadyAssociated,
            ClientError::DoesNotExist,
            ClientError::Expired,
            ClientError::Unassociated,
        ];

        for i in 0..errors.len() {
            for j in (i + 1)..errors.len() {
                assert_ne!(errors[i], errors[j]);
            }
        }
    }

    #[test]
    fn test_user_command_variants() {
        let commands = vec![
            UserCommand::Connect,
            UserCommand::Disconnect,
            UserCommand::ShowHand,
            UserCommand::StartGame,
            UserCommand::ChangeState(UserState::Play),
            UserCommand::ChangeState(UserState::Spectate),
            UserCommand::TakeAction(Action::Fold),
            UserCommand::CastVote(Vote::Reset(None)),
        ];

        // Each should have different display strings
        for i in 0..commands.len() {
            for j in (i + 1)..commands.len() {
                if commands[i] != commands[j] {
                    assert_ne!(format!("{}", commands[i]), format!("{}", commands[j]));
                }
            }
        }
    }

    #[test]
    fn test_complex_client_message_serialization() {
        let msg = ClientMessage {
            username: Username::new("complex_user_123"),
            command: UserCommand::TakeAction(Action::Raise(Some(500))),
        };

        let serialized = bincode::serialize(&msg).unwrap();
        let deserialized: ClientMessage = bincode::deserialize(&serialized).unwrap();

        assert_eq!(msg.username, deserialized.username);
        assert_eq!(msg.command, deserialized.command);
    }

    // === Network Property Tests (Sprint 6 Stage 6) ===

    #[test]
    fn property_message_roundtrip_basic_commands() {
        let commands = vec![
            UserCommand::Connect,
            UserCommand::Disconnect,
            UserCommand::TakeAction(Action::Fold),
            UserCommand::TakeAction(Action::Check),
            UserCommand::TakeAction(Action::Call),
            UserCommand::TakeAction(Action::AllIn),
            UserCommand::TakeAction(Action::Raise(Some(100))),
            UserCommand::CastVote(Vote::Reset(None)),
            UserCommand::CastVote(Vote::Kick(Username::new("player"))),
        ];

        for command in commands {
            let msg = ClientMessage {
                username: Username::new("test"),
                command: command.clone(),
            };
            let serialized = bincode::serialize(&msg).unwrap();
            let deserialized: ClientMessage = bincode::deserialize(&serialized).unwrap();
            assert_eq!(msg.username, deserialized.username);
            assert_eq!(msg.command, deserialized.command);
        }
    }

    #[test]
    fn property_client_error_roundtrip() {
        let errors = vec![
            ClientError::AlreadyAssociated,
        ];

        for error in errors {
            let serialized = bincode::serialize(&error).unwrap();
            let deserialized: ClientError = bincode::deserialize(&serialized).unwrap();
            assert_eq!(error, deserialized);
        }
    }

    #[test]
    fn property_username_serialization_preserves_content() {
        let long_name = "a".repeat(100);
        let usernames = vec![
            "alice",
            "bob123",
            "player_with_underscores",
            "日本語",
            "emoji🎮",
            "",
            long_name.as_str(),
        ];

        for name in usernames {
            let username = Username::new(name);
            let serialized = bincode::serialize(&username).unwrap();
            let deserialized: Username = bincode::deserialize(&serialized).unwrap();
            assert_eq!(username, deserialized);
        }
    }

    #[test]
    fn property_action_serialization_bijective() {
        let actions = vec![
            Action::Fold,
            Action::Check,
            Action::Call,
            Action::AllIn,
            Action::Raise(Some(0)),
            Action::Raise(Some(u32::MAX)),
            Action::Raise(None),
        ];

        for action in actions {
            let ser1 = bincode::serialize(&action).unwrap();
            let deser: Action = bincode::deserialize(&ser1).unwrap();
            let ser2 = bincode::serialize(&deser).unwrap();
            assert_eq!(ser1, ser2, "Serialization should be bijective");
        }
    }

    #[test]
    fn property_vote_serialization_deterministic() {
        let votes = vec![
            Vote::Reset(None),
            Vote::Reset(Some(Username::new("player1"))),
            Vote::Kick(Username::new("player2")),
        ];

        for vote in votes {
            let ser1 = bincode::serialize(&vote).unwrap();
            let ser2 = bincode::serialize(&vote).unwrap();
            assert_eq!(ser1, ser2, "Same vote should serialize identically");
        }
    }
}
