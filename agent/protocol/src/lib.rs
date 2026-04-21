//! Protocol contract for daemon-client communication
//!
//! This crate defines all JSON-RPC 2.0 message types, method parameters,
//! event payloads, state snapshots, and the `daemon.json` persistence format
//! exchanged between `agent-daemon` and its clients (TUI, CLI, IDE plugins).
//!
//! It also provides shared client-side utilities such as auto-link discovery.

pub mod client;
pub mod config;
pub mod events;
pub mod jsonrpc;
pub mod methods;
pub mod state;
pub mod workplace;

/// Re-export core identity types so clients don't need a direct `agent-types` dep.
pub use agent_types::WorkplaceId;

/// Protocol version negotiated during `session.initialize`.
pub const PROTOCOL_VERSION: &str = "1.0.0";
