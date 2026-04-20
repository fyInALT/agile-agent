//! agent-daemon — per-workspace daemon that owns runtime state
//!
//! Serves JSON-RPC 2.0 over WebSocket to thin clients (TUI, CLI, IDE plugins).

pub mod connection;
pub mod handler;
pub mod lifecycle;
pub mod router;
pub mod server;
pub mod workplace;
