//! A low-level TCP poker client.
//!
//! This client is blocking and so is primarily used as a testing utility
//! rather than an actual poker client.

use anyhow::{Error, bail};
use std::{
    net::{SocketAddr, TcpStream},
    thread,
    time::Duration,
};

use super::{
    super::{
        entities::{Username, Vote},
        game::{
            GameEvent, UserError,
            entities::{Action, GameView},
        },
    },
    messages::{ClientError, ClientMessage, ServerMessage, UserCommand, UserState},
    utils,
};

/// Default timeout for reading from the server.
pub const READ_TIMEOUT: Duration = Duration::from_secs(10);

/// Default timeout for writing to the server.
pub const WRITE_TIMEOUT: Duration = Duration::from_secs(1);

/// A blocking TCP client for connecting to a poker server.
///
/// This client provides a synchronous interface for sending commands
/// and receiving updates from the server. Primarily used for testing
/// and simple client implementations.
pub struct Client {
    /// The username associated with this client.
    pub username: Username,
    /// The underlying TCP stream.
    pub stream: TcpStream,
}

impl Client {
    /// Cast a vote (kick user or reset money).
    ///
    /// # Errors
    ///
    /// Returns an error if the message cannot be sent to the server.
    pub fn cast_vote(&mut self, vote: Vote) -> Result<(), Error> {
        let msg = ClientMessage {
            username: self.username.clone(),
            command: UserCommand::CastVote(vote),
        };
        utils::write_prefixed(&mut self.stream, &msg)?;
        Ok(())
    }

    /// Change the user's state (join waitlist or become spectator).
    ///
    /// # Errors
    ///
    /// Returns an error if the message cannot be sent to the server.
    pub fn change_state(&mut self, state: UserState) -> Result<(), Error> {
        let msg = ClientMessage {
            username: self.username.clone(),
            command: UserCommand::ChangeState(state),
        };
        utils::write_prefixed(&mut self.stream, &msg)?;
        Ok(())
    }

    /// Connect to a poker server and receive the initial game view.
    ///
    /// This method attempts to connect with exponential backoff, trying
    /// three times with decreasing timeouts (1s, 500ms, 100ms).
    ///
    /// # Arguments
    ///
    /// * `username` - The username for this client
    /// * `addr` - The server socket address
    ///
    /// # Returns
    ///
    /// Returns the connected client and initial game view on success.
    ///
    /// # Errors
    ///
    /// Returns an error if unable to connect or if the server rejects the connection.
    pub fn connect(username: Username, addr: &SocketAddr) -> Result<(Self, GameView), Error> {
        let mut connect_timeouts = vec![
            Duration::from_secs(1),
            Duration::from_millis(500),
            Duration::from_millis(100),
        ];
        while let Some(connect_timeout) = connect_timeouts.pop() {
            match TcpStream::connect_timeout(addr, connect_timeout) {
                Ok(mut stream) => {
                    stream.set_read_timeout(Some(READ_TIMEOUT))?;
                    stream.set_write_timeout(Some(WRITE_TIMEOUT))?;
                    let msg = ClientMessage {
                        username: username.clone(),
                        command: UserCommand::Connect,
                    };
                    utils::write_prefixed(&mut stream, &msg)?;
                    Self::recv_ack(&mut stream)?;
                    // Then receive the game view.
                    match Self::recv_view(&mut stream) {
                        Ok(view) => {
                            return Ok((Self { username, stream }, view));
                        }
                        Err(error) => bail!(error),
                    }
                }
                _ => thread::sleep(connect_timeout),
            }
        }
        bail!("couldn't connect to {addr} as {username}")
    }

    pub fn recv(&mut self) -> Result<ServerMessage, Error> {
        match utils::read_prefixed::<ServerMessage, TcpStream>(&mut self.stream) {
            Ok(ServerMessage::ClientError(error)) => bail!(error),
            Ok(ServerMessage::UserError(error)) => bail!(error),
            Ok(msg) => Ok(msg),
            Err(error) => bail!(error),
        }
    }

    pub fn recv_ack(stream: &mut TcpStream) -> Result<(), Error> {
        match utils::read_prefixed::<ServerMessage, TcpStream>(stream) {
            Ok(ServerMessage::Ack(_)) => Ok(()),
            Ok(ServerMessage::ClientError(error)) => bail!(error),
            Ok(ServerMessage::UserError(error)) => bail!(error),
            Ok(response) => {
                bail!("invalid server response: {response}")
            }
            Err(error) => bail!(error),
        }
    }

    pub fn recv_client_error(stream: &mut TcpStream) -> Result<ClientError, Error> {
        match utils::read_prefixed::<ServerMessage, TcpStream>(stream) {
            Ok(ServerMessage::ClientError(error)) => Ok(error),
            Ok(response) => {
                bail!("invalid server response: {response}")
            }
            Err(error) => bail!(error),
        }
    }

    pub fn recv_event(stream: &mut TcpStream) -> Result<GameEvent, Error> {
        match utils::read_prefixed::<ServerMessage, TcpStream>(stream) {
            Ok(ServerMessage::GameEvent(event)) => Ok(event),
            Ok(response) => {
                bail!("invalid server response: {response}")
            }
            Err(error) => bail!(error),
        }
    }

    pub fn recv_user_error(stream: &mut TcpStream) -> Result<UserError, Error> {
        match utils::read_prefixed::<ServerMessage, TcpStream>(stream) {
            Ok(ServerMessage::UserError(error)) => Ok(error),
            Ok(response) => {
                bail!("invalid server response: {response}")
            }
            Err(error) => bail!(error),
        }
    }

    pub fn recv_view(stream: &mut TcpStream) -> Result<GameView, Error> {
        match utils::read_prefixed::<ServerMessage, TcpStream>(stream) {
            Ok(ServerMessage::ClientError(error)) => bail!(error),
            Ok(ServerMessage::GameView(view)) => Ok(view),
            Ok(ServerMessage::UserError(error)) => bail!(error),
            Ok(response) => {
                bail!("invalid server response: {response}")
            }
            Err(error) => bail!(error),
        }
    }

    pub fn show_hand(&mut self) -> Result<(), Error> {
        let msg = ClientMessage {
            username: self.username.clone(),
            command: UserCommand::ShowHand,
        };
        utils::write_prefixed(&mut self.stream, &msg)?;
        Ok(())
    }

    pub fn start_game(&mut self) -> Result<(), Error> {
        let msg = ClientMessage {
            username: self.username.clone(),
            command: UserCommand::StartGame,
        };
        utils::write_prefixed(&mut self.stream, &msg)?;
        Ok(())
    }

    pub fn take_action(&mut self, action: Action) -> Result<(), Error> {
        let msg = ClientMessage {
            username: self.username.clone(),
            command: UserCommand::TakeAction(action),
        };
        utils::write_prefixed(&mut self.stream, &msg)?;
        Ok(())
    }
}
