//! Protocol contract for daemon-client communication
//!
//! This crate defines all JSON-RPC 2.0 message types, method parameters,
//! event payloads, and state snapshots exchanged between `agent-daemon`
//! and its clients (TUI, CLI, IDE plugins).
//!
//! It is pure data — no runtime dependencies, no I/O, no transport logic.

pub mod events;
pub mod jsonrpc;
pub mod methods;
pub mod state;

/// Protocol version negotiated during `session.initialize`.
pub const PROTOCOL_VERSION: &str = "1.0.0";
