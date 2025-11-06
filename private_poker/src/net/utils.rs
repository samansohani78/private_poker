use bincode::{ErrorKind, deserialize, serialize};
use serde::{Serialize, de::DeserializeOwned};
use std::io::{self, Read, Write};

/// Maximum allowed message size (1MB) to prevent DoS attacks via unbounded allocation
const MAX_MESSAGE_SIZE: usize = 1024 * 1024;

pub fn read_prefixed<T: DeserializeOwned, R: Read>(reader: &mut R) -> io::Result<T> {
    // Read the size as a u32
    let mut len_bytes = [0; 4];
    reader.read_exact(&mut len_bytes)?;
    let len = u32::from_le_bytes(len_bytes) as usize;

    // Validate message size to prevent DoS attacks
    if len > MAX_MESSAGE_SIZE {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("message size {} exceeds maximum allowed size of {} bytes", len, MAX_MESSAGE_SIZE)
        ));
    }

    // Read the remaining data. If we get a would block error,
    // then it's very likely that the sender doesn't follow the
    // prefix protocol. Return an invalid data error to let
    // the readers determine how to handle such senders. It is
    // possible for the would block error to be something that
    // isn't as sketchy, but that should be pretty rare.
    let mut buf = vec![0; len];
    if let Err(error) = reader.read_exact(&mut buf) {
        let kind = match error.kind() {
            io::ErrorKind::WouldBlock => io::ErrorKind::InvalidData,
            error => error,
        };
        return Err(kind.into());
    }

    match deserialize(&buf) {
        Ok(value) => Ok(value),
        Err(error) => match *error {
            ErrorKind::Io(error) => Err(error),
            _ => Err(io::ErrorKind::InvalidData.into()),
        },
    }
}

pub fn write_prefixed<T: Serialize, W: Write>(writer: &mut W, value: &T) -> io::Result<()> {
    match serialize(&value) {
        Ok(serialized) => {
            // Validate message size before sending
            if serialized.len() > MAX_MESSAGE_SIZE {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("serialized message size {} exceeds maximum allowed size of {} bytes", serialized.len(), MAX_MESSAGE_SIZE)
                ));
            }

            // Write the size of the serialized data and the serialized data
            // all in one chunk to prevent read-side EOF race conditions.
            let size = serialized.len() as u32;
            let mut buf = Vec::from(size.to_le_bytes());
            buf.extend(serialized);
            writer.write_all(&buf)?;
            Ok(())
        }
        Err(error) => match *error {
            ErrorKind::Io(error) => Err(error),
            _ => Err(io::ErrorKind::InvalidData.into()),
        },
    }
}

#[cfg(test)]
mod tests {
    use std::io::{self, Write};

    use mio::net::{TcpListener, TcpStream};

    use super::{read_prefixed, write_prefixed};

    fn setup() -> (TcpStream, TcpStream) {
        let random_port_addr = "127.0.0.1:0".parse().unwrap();
        let server = TcpListener::bind(random_port_addr).unwrap();
        let addr = server.local_addr().unwrap();
        let client = TcpStream::connect(addr).unwrap();
        let (stream, _) = server.accept().unwrap();
        (client, stream)
    }

    #[test]
    fn write_and_read() {
        let (mut client, mut stream) = setup();
        let value = "Hello, World!".to_string();
        assert!(write_prefixed(&mut stream, &value).is_ok());
        assert!(read_prefixed::<String, TcpStream>(&mut client).is_ok_and(|v| v == value));
    }

    #[test]
    fn write_and_read_invalid_data() {
        let (mut client, mut stream) = setup();

        // Writing a size but not having the data to follow it up
        // results in invalid data.
        assert!(stream.write_all(&1u32.to_le_bytes()).is_ok());
        assert_eq!(
            read_prefixed::<String, TcpStream>(&mut client).map_err(|e| e.kind()),
            Err(io::ErrorKind::InvalidData)
        );
    }

    #[test]
    fn write_and_read_unexpected_eof() {
        let (mut client, mut stream) = setup();
        let value = "Hello, World!".to_string();
        let buf = value.as_bytes();
        let incorrect_size = buf.len() as u32 - 2;
        assert!(stream.write_all(&incorrect_size.to_le_bytes()).is_ok());
        assert!(stream.write_all(buf).is_ok());
        assert_eq!(
            read_prefixed::<String, TcpStream>(&mut client).map_err(|e| e.kind()),
            Err(io::ErrorKind::UnexpectedEof)
        );
    }

    #[test]
    fn reject_oversized_message() {
        let (mut client, mut stream) = setup();

        // Send a size prefix claiming 2GB of data (DoS attack attempt)
        let malicious_size = 2_000_000_000u32;
        assert!(stream.write_all(&malicious_size.to_le_bytes()).is_ok());

        // Should reject with InvalidData, not attempt allocation
        assert_eq!(
            read_prefixed::<String, TcpStream>(&mut client).map_err(|e| e.kind()),
            Err(io::ErrorKind::InvalidData)
        );
    }

    #[test]
    fn write_and_read_multiple_messages() {
        let (mut client, mut stream) = setup();

        // Send multiple messages in sequence
        let msgs = vec!["first", "second", "third"];
        for msg in &msgs {
            assert!(write_prefixed(&mut stream, &msg.to_string()).is_ok());
        }

        // Read them back in order
        for msg in &msgs {
            let received: String = read_prefixed(&mut client).unwrap();
            assert_eq!(&received, msg);
        }
    }

    #[test]
    fn write_prefixed_empty_string() {
        let (mut client, mut stream) = setup();
        let empty = String::new();
        assert!(write_prefixed(&mut stream, &empty).is_ok());
        let received: String = read_prefixed(&mut client).unwrap();
        assert_eq!(received, empty);
    }

    #[test]
    fn write_prefixed_large_valid_message() {
        let (mut client, mut stream) = setup();
        // Create a message just under the limit (1MB - 100 bytes for safety)
        let large_msg = "x".repeat(1024 * 1024 - 100);
        assert!(write_prefixed(&mut stream, &large_msg).is_ok());
        let received: String = read_prefixed(&mut client).unwrap();
        assert_eq!(received, large_msg);
    }

    #[test]
    fn read_prefixed_with_partial_length() {
        let (mut client, mut stream) = setup();

        // Write only 2 bytes of a 4-byte length prefix
        assert!(stream.write_all(&[0, 0]).is_ok());
        drop(stream); // Close connection

        // Should fail with UnexpectedEof
        assert_eq!(
            read_prefixed::<String, TcpStream>(&mut client).map_err(|e| e.kind()),
            Err(io::ErrorKind::UnexpectedEof)
        );
    }

    #[test]
    fn write_and_read_complex_struct() {
        use crate::net::messages::{UserCommand, ClientMessage};
        use crate::game::entities::Username;

        let (mut client, mut stream) = setup();

        let msg = ClientMessage {
            username: Username::new("test_user"),
            command: UserCommand::Connect,
        };

        assert!(write_prefixed(&mut stream, &msg).is_ok());
        let received: ClientMessage = read_prefixed(&mut client).unwrap();
        assert_eq!(received.username, msg.username);
        assert_eq!(received.command, msg.command);
    }

    #[test]
    fn write_and_read_numbers() {
        let (mut client, mut stream) = setup();

        let numbers = vec![0u64, 1, 42, 12345, u64::MAX];
        for num in &numbers {
            assert!(write_prefixed(&mut stream, num).is_ok());
        }

        for num in &numbers {
            let received: u64 = read_prefixed(&mut client).unwrap();
            assert_eq!(received, *num);
        }
    }

    #[test]
    fn write_and_read_boolean() {
        let (mut client, mut stream) = setup();

        assert!(write_prefixed(&mut stream, &true).is_ok());
        assert!(write_prefixed(&mut stream, &false).is_ok());

        assert_eq!(read_prefixed::<bool, TcpStream>(&mut client).unwrap(), true);
        assert_eq!(read_prefixed::<bool, TcpStream>(&mut client).unwrap(), false);
    }

    #[test]
    fn write_and_read_vec() {
        let (mut client, mut stream) = setup();

        let vec_data = vec![1, 2, 3, 4, 5];
        assert!(write_prefixed(&mut stream, &vec_data).is_ok());

        let received: Vec<i32> = read_prefixed(&mut client).unwrap();
        assert_eq!(received, vec_data);
    }

    // === Stress Tests ===

    #[test]
    fn stress_test_many_sequential_messages() {
        let (mut client, mut stream) = setup();

        // Send and receive 500 messages sequentially
        for i in 0..500 {
            let msg = format!("message_{}", i);
            assert!(write_prefixed(&mut stream, &msg).is_ok());
            let received: String = read_prefixed(&mut client).unwrap();
            assert_eq!(received, msg);
        }
    }

    #[test]
    fn stress_test_large_string_serialization() {
        let (mut client, mut stream) = setup();

        // Test with strings of increasing size
        for size in [1000, 10000, 100000, 500000] {
            let large_string = "x".repeat(size);
            assert!(write_prefixed(&mut stream, &large_string).is_ok());
            let received: String = read_prefixed(&mut client).unwrap();
            assert_eq!(received.len(), size);
        }
    }

    #[test]
    fn stress_test_rapid_write_operations() {
        let (mut client, mut stream) = setup();

        // Rapidly write 1000 messages
        for i in 0..1000 {
            let msg = i.to_string();
            assert!(write_prefixed(&mut stream, &msg).is_ok());
        }

        // Read them all back
        for i in 0..1000 {
            let received: String = read_prefixed(&mut client).unwrap();
            assert_eq!(received, i.to_string());
        }
    }

    #[test]
    fn stress_test_alternating_types() {
        let (mut client, mut stream) = setup();

        // Alternate between different types rapidly
        for i in 0..100 {
            // String
            assert!(write_prefixed(&mut stream, &format!("str{}", i)).is_ok());
            // Number
            assert!(write_prefixed(&mut stream, &(i as u64)).is_ok());
            // Boolean
            assert!(write_prefixed(&mut stream, &(i % 2 == 0)).is_ok());
        }

        // Read them back
        for _ in 0..100 {
            let _s: String = read_prefixed(&mut client).unwrap();
            let _n: u64 = read_prefixed(&mut client).unwrap();
            let _b: bool = read_prefixed(&mut client).unwrap();
        }
    }

    #[test]
    fn stress_test_large_vector_serialization() {
        let (mut client, mut stream) = setup();

        // Test with vectors of increasing size
        for size in [100, 1000, 10000] {
            let large_vec: Vec<i32> = (0..size).collect();
            assert!(write_prefixed(&mut stream, &large_vec).is_ok());
            let received: Vec<i32> = read_prefixed(&mut client).unwrap();
            assert_eq!(received.len(), size as usize);
        }
    }

    #[test]
    fn stress_test_complex_message_structures() {
        use crate::net::messages::{ClientMessage, UserCommand};
        use crate::game::entities::{Username, Action, Vote};

        let (mut client, mut stream) = setup();

        // Send many complex messages
        for i in 0..200 {
            let msg = ClientMessage {
                username: Username::new(&format!("user{}", i)),
                command: if i % 3 == 0 {
                    UserCommand::Connect
                } else if i % 3 == 1 {
                    UserCommand::TakeAction(Action::Raise(Some(i as u32 * 10)))
                } else {
                    UserCommand::CastVote(Vote::Reset(None))
                },
            };

            assert!(write_prefixed(&mut stream, &msg).is_ok());
            let _received: ClientMessage = read_prefixed(&mut client).unwrap();
        }
    }
}
