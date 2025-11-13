//! Protocol versioning for backward compatibility.

use serde::{Deserialize, Serialize};

/// Protocol version for multi-table poker
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProtocolVersion {
    /// V1: Original single-table, no authentication
    V1,
    /// V2: Multi-table with authentication, wallet, and chat
    V2,
}

impl ProtocolVersion {
    /// Get the current protocol version
    pub fn current() -> Self {
        ProtocolVersion::V2
    }

    /// Check if this version is compatible with another
    pub fn is_compatible_with(&self, other: &ProtocolVersion) -> bool {
        // V1 and V2 are compatible via legacy mode in server
        matches!(
            (self, other),
            (ProtocolVersion::V1, ProtocolVersion::V1)
                | (ProtocolVersion::V2, ProtocolVersion::V2)
                | (ProtocolVersion::V1, ProtocolVersion::V2)
                | (ProtocolVersion::V2, ProtocolVersion::V1)
        )
    }
}

impl Default for ProtocolVersion {
    fn default() -> Self {
        Self::current()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bincode::config;
    use bincode::serde::{decode_from_slice, encode_to_vec};
    use serde::{Serialize, de::DeserializeOwned};

    // Small helpers to keep tests readable and consistent with bincode 2
    fn serialize_value<T: Serialize>(value: &T) -> Vec<u8> {
        encode_to_vec(value, config::standard()).unwrap()
    }

    fn deserialize_value<T: DeserializeOwned>(bytes: &[u8]) -> T {
        decode_from_slice(bytes, config::standard()).unwrap().0
    }

    #[test]
    fn test_current_version() {
        assert_eq!(ProtocolVersion::current(), ProtocolVersion::V2);
    }

    #[test]
    fn test_compatibility() {
        assert!(ProtocolVersion::V1.is_compatible_with(&ProtocolVersion::V1));
        assert!(ProtocolVersion::V2.is_compatible_with(&ProtocolVersion::V2));
        assert!(ProtocolVersion::V1.is_compatible_with(&ProtocolVersion::V2));
        assert!(ProtocolVersion::V2.is_compatible_with(&ProtocolVersion::V1));
    }

    #[test]
    fn test_serialization() {
        let v1 = ProtocolVersion::V1;
        let serialized = serialize_value(&v1);
        let deserialized: ProtocolVersion = deserialize_value(&serialized);
        assert_eq!(v1, deserialized);
    }
}
