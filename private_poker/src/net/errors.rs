//! Network error types for serialization and protocol operations.

use thiserror::Error;

/// Errors that can occur during network message serialization/deserialization
#[derive(Debug, Error)]
pub enum SerializationError {
    /// Failed to encode a message
    #[error("Failed to encode message: {0}")]
    Encode(#[from] bincode::error::EncodeError),

    /// Failed to decode a message
    #[error("Failed to decode message: {0}")]
    Decode(#[from] bincode::error::DecodeError),

    /// Message size exceeded maximum allowed
    #[error("Message size {actual} exceeds maximum {max}")]
    MessageTooLarge { actual: usize, max: usize },

    /// Invalid message format
    #[error("Invalid message format: {0}")]
    InvalidFormat(String),
}

/// Result type for serialization operations
pub type Result<T> = std::result::Result<T, SerializationError>;
