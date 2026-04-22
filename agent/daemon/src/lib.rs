//! agent-daemon — per-workspace daemon that owns runtime state
//!
//! Serves JSON-RPC 2.0 over WebSocket to thin clients (TUI, CLI, IDE plugins).

pub mod audit;
pub mod broadcaster;
pub mod config_file;
pub mod connection;

pub mod event_log;
pub mod event_pump;
pub mod handler;
pub mod health;
pub mod lifecycle;
pub mod router;
pub mod server;
pub mod session_mgr;
