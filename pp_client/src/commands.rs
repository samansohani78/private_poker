use private_poker::{
    entities::{Action, Usd, Username, Vote},
    messages::{UserCommand, UserState},
};
use std::fmt;

/// Errors that can occur during command parsing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseError {
    /// Invalid raise amount (not a valid number).
    InvalidRaiseAmount(String),
    /// Vote kick command missing username.
    VoteKickMissingUsername,
    /// Invalid vote command format.
    InvalidVoteCommand,
    /// Unrecognized command.
    UnrecognizedCommand(String),
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidRaiseAmount(value) => write!(
                f,
                "Invalid raise amount '{}'. Must be a positive number (e.g., 'raise 100')",
                value
            ),
            Self::VoteKickMissingUsername => {
                write!(f, "Vote kick requires a username (e.g., 'vote kick alice')")
            }
            Self::InvalidVoteCommand => write!(
                f,
                "Invalid vote command. Use 'vote kick USERNAME' or 'vote reset [USERNAME]'"
            ),
            Self::UnrecognizedCommand(cmd) => write!(
                f,
                "Unrecognized command '{}'. Type 'help' to see available commands",
                cmd
            ),
        }
    }
}

impl std::error::Error for ParseError {}

/// Parse a command string into a UserCommand.
///
/// # Arguments
///
/// * `input` - The raw command string from user input
///
/// # Returns
///
/// * `Ok(UserCommand)` - Successfully parsed command
/// * `Err(ParseError)` - Parse error with descriptive message
///
/// # Examples
///
/// ```
/// use pp_client::commands::parse_command;
/// use private_poker::messages::{UserCommand, UserState};
/// use private_poker::entities::Action;
///
/// // Single-word commands
/// assert!(matches!(parse_command("call"), Ok(UserCommand::TakeAction(Action::Call))));
/// assert!(matches!(parse_command("fold"), Ok(UserCommand::TakeAction(Action::Fold))));
/// assert!(matches!(parse_command("play"), Ok(UserCommand::ChangeState(UserState::Play))));
///
/// // Multi-word commands
/// assert!(matches!(parse_command("raise 100"), Ok(UserCommand::TakeAction(Action::Raise(Some(100))))));
/// ```
pub fn parse_command(input: &str) -> Result<UserCommand, ParseError> {
    let trimmed = input.trim();

    // Try single-word commands first
    match trimmed {
        "all-in" => return Ok(UserCommand::TakeAction(Action::AllIn)),
        "call" => return Ok(UserCommand::TakeAction(Action::Call)),
        "check" => return Ok(UserCommand::TakeAction(Action::Check)),
        "fold" => return Ok(UserCommand::TakeAction(Action::Fold)),
        "play" => return Ok(UserCommand::ChangeState(UserState::Play)),
        "show" => return Ok(UserCommand::ShowHand),
        "spectate" => return Ok(UserCommand::ChangeState(UserState::Spectate)),
        "start" => return Ok(UserCommand::StartGame),
        _ => {}
    }

    // Parse multi-word commands
    let parts: Vec<&str> = trimmed.split_ascii_whitespace().collect();
    match parts.first() {
        Some(&"raise") => parse_raise_command(&parts),
        Some(&"vote") => parse_vote_command(&parts),
        _ => Err(ParseError::UnrecognizedCommand(trimmed.to_string())),
    }
}

/// Parse a raise command: "raise [amount]"
fn parse_raise_command(parts: &[&str]) -> Result<UserCommand, ParseError> {
    match parts.get(1) {
        Some(value) => {
            // Parse the raise amount
            let amount = value
                .parse::<Usd>()
                .map_err(|_| ParseError::InvalidRaiseAmount(value.to_string()))?;
            Ok(UserCommand::TakeAction(Action::Raise(Some(amount))))
        }
        None => {
            // Raise with no amount specified (let server determine minimum)
            Ok(UserCommand::TakeAction(Action::Raise(None)))
        }
    }
}

/// Parse a vote command: "vote kick USERNAME" or "vote reset [USERNAME]"
fn parse_vote_command(parts: &[&str]) -> Result<UserCommand, ParseError> {
    match (parts.get(1), parts.get(2)) {
        (Some(&"kick"), Some(username)) => {
            Ok(UserCommand::CastVote(Vote::Kick(Username::new(username))))
        }
        (Some(&"reset"), Some(username)) => Ok(UserCommand::CastVote(Vote::Reset(Some(
            Username::new(username),
        )))),
        (Some(&"reset"), None) => Ok(UserCommand::CastVote(Vote::Reset(None))),
        (Some(&"kick"), None) => Err(ParseError::VoteKickMissingUsername),
        _ => Err(ParseError::InvalidVoteCommand),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // === Single-word command tests ===

    #[test]
    fn test_parse_all_in() {
        let result = parse_command("all-in");
        assert!(matches!(result, Ok(UserCommand::TakeAction(Action::AllIn))));
    }

    #[test]
    fn test_parse_call() {
        let result = parse_command("call");
        assert!(matches!(result, Ok(UserCommand::TakeAction(Action::Call))));
    }

    #[test]
    fn test_parse_check() {
        let result = parse_command("check");
        assert!(matches!(result, Ok(UserCommand::TakeAction(Action::Check))));
    }

    #[test]
    fn test_parse_fold() {
        let result = parse_command("fold");
        assert!(matches!(result, Ok(UserCommand::TakeAction(Action::Fold))));
    }

    #[test]
    fn test_parse_play() {
        let result = parse_command("play");
        assert!(matches!(
            result,
            Ok(UserCommand::ChangeState(UserState::Play))
        ));
    }

    #[test]
    fn test_parse_show() {
        let result = parse_command("show");
        assert!(matches!(result, Ok(UserCommand::ShowHand)));
    }

    #[test]
    fn test_parse_spectate() {
        let result = parse_command("spectate");
        assert!(matches!(
            result,
            Ok(UserCommand::ChangeState(UserState::Spectate))
        ));
    }

    #[test]
    fn test_parse_start() {
        let result = parse_command("start");
        assert!(matches!(result, Ok(UserCommand::StartGame)));
    }

    // === Whitespace handling ===

    #[test]
    fn test_parse_with_leading_whitespace() {
        let result = parse_command("  call");
        assert!(matches!(result, Ok(UserCommand::TakeAction(Action::Call))));
    }

    #[test]
    fn test_parse_with_trailing_whitespace() {
        let result = parse_command("fold  ");
        assert!(matches!(result, Ok(UserCommand::TakeAction(Action::Fold))));
    }

    #[test]
    fn test_parse_with_surrounding_whitespace() {
        let result = parse_command("  check  ");
        assert!(matches!(result, Ok(UserCommand::TakeAction(Action::Check))));
    }

    // === Raise command tests ===

    #[test]
    fn test_parse_raise_with_amount() {
        let result = parse_command("raise 100");
        assert!(matches!(
            result,
            Ok(UserCommand::TakeAction(Action::Raise(Some(100))))
        ));
    }

    #[test]
    fn test_parse_raise_without_amount() {
        let result = parse_command("raise");
        assert!(matches!(
            result,
            Ok(UserCommand::TakeAction(Action::Raise(None)))
        ));
    }

    #[test]
    fn test_parse_raise_with_large_amount() {
        let result = parse_command("raise 999999");
        assert!(matches!(
            result,
            Ok(UserCommand::TakeAction(Action::Raise(Some(999_999))))
        ));
    }

    #[test]
    fn test_parse_raise_with_invalid_amount() {
        let result = parse_command("raise abc");
        assert!(matches!(result, Err(ParseError::InvalidRaiseAmount(_))));
    }

    #[test]
    fn test_parse_raise_with_negative_amount() {
        let result = parse_command("raise -50");
        assert!(matches!(result, Err(ParseError::InvalidRaiseAmount(_))));
    }

    #[test]
    fn test_parse_raise_with_float() {
        let result = parse_command("raise 10.5");
        assert!(matches!(result, Err(ParseError::InvalidRaiseAmount(_))));
    }

    // === Vote command tests ===

    #[test]
    fn test_parse_vote_kick_with_username() {
        let result = parse_command("vote kick alice");
        match result {
            Ok(UserCommand::CastVote(Vote::Kick(username))) => {
                assert_eq!(username.to_string(), "alice");
            }
            _ => panic!("Expected Vote::Kick"),
        }
    }

    #[test]
    fn test_parse_vote_kick_without_username() {
        let result = parse_command("vote kick");
        assert!(matches!(result, Err(ParseError::VoteKickMissingUsername)));
    }

    #[test]
    fn test_parse_vote_reset_with_username() {
        let result = parse_command("vote reset bob");
        match result {
            Ok(UserCommand::CastVote(Vote::Reset(Some(username)))) => {
                assert_eq!(username.to_string(), "bob");
            }
            _ => panic!("Expected Vote::Reset with username"),
        }
    }

    #[test]
    fn test_parse_vote_reset_without_username() {
        let result = parse_command("vote reset");
        match result {
            Ok(UserCommand::CastVote(Vote::Reset(None))) => {}
            _ => panic!("Expected Vote::Reset without username"),
        }
    }

    #[test]
    fn test_parse_vote_invalid_type() {
        let result = parse_command("vote start");
        assert!(matches!(result, Err(ParseError::InvalidVoteCommand)));
    }

    #[test]
    fn test_parse_vote_no_type() {
        let result = parse_command("vote");
        assert!(matches!(result, Err(ParseError::InvalidVoteCommand)));
    }

    // === Error cases ===

    #[test]
    fn test_parse_unrecognized_command() {
        let result = parse_command("invalid");
        assert!(matches!(result, Err(ParseError::UnrecognizedCommand(_))));
    }

    #[test]
    fn test_parse_empty_string() {
        let result = parse_command("");
        assert!(matches!(result, Err(ParseError::UnrecognizedCommand(_))));
    }

    #[test]
    fn test_parse_whitespace_only() {
        let result = parse_command("   ");
        assert!(matches!(result, Err(ParseError::UnrecognizedCommand(_))));
    }

    // === Error message tests ===

    #[test]
    fn test_error_message_invalid_raise_amount() {
        let error = ParseError::InvalidRaiseAmount("abc".to_string());
        let msg = error.to_string();
        assert!(msg.contains("Invalid raise amount"));
        assert!(msg.contains("abc"));
        assert!(msg.contains("positive number"));
    }

    #[test]
    fn test_error_message_vote_kick_missing_username() {
        let error = ParseError::VoteKickMissingUsername;
        let msg = error.to_string();
        assert!(msg.contains("Vote kick requires a username"));
    }

    #[test]
    fn test_error_message_invalid_vote_command() {
        let error = ParseError::InvalidVoteCommand;
        let msg = error.to_string();
        assert!(msg.contains("Invalid vote command"));
        assert!(msg.contains("vote kick USERNAME"));
        assert!(msg.contains("vote reset"));
    }

    #[test]
    fn test_error_message_unrecognized_command() {
        let error = ParseError::UnrecognizedCommand("xyz".to_string());
        let msg = error.to_string();
        assert!(msg.contains("Unrecognized command"));
        assert!(msg.contains("xyz"));
        assert!(msg.contains("help"));
    }
}
