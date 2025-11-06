//! Networking layer for client-server communication.
//!
//! This module provides TCP-based networking with a custom binary protocol
//! using bincode serialization. The server uses `mio` for non-blocking I/O.

/// TCP client for connecting to a poker server.
pub mod client;

/// Message types for client-server communication protocol.
pub mod messages;

/// Protocol versioning for backward compatibility.
pub mod protocol_version;

/// Multi-threaded TCP server with async I/O event loop.
pub mod server;

/// Utilities for binary message serialization and framing.
pub mod utils;
