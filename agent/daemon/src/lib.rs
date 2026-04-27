//! agent-daemon — per-workspace daemon that owns runtime state
//!
//! Serves JSON-RPC 2.0 over WebSocket to thin clients (TUI, CLI, IDE plugins).

pub mod audit;
pub mod broadcaster;
pub mod config_file;
pub mod connection;
pub mod decision_agent_slot;
pub mod decision_integration;
pub mod event_log;
pub mod event_pump;
pub mod handler;
pub mod health;
pub mod lifecycle;
pub mod real_provider;
pub mod router;
pub mod server;
pub mod session_interpreter;
pub mod session_mgr;

// Re-export key types for external use
pub use decision_agent_slot::{
    DecisionAgentSlot, DecisionCommandInterpreter, DecisionSlotConfig, MockCommandInterpreter,
};
pub use real_provider::{RealLLMProvider, create_real_provider};
pub use session_interpreter::{SessionManagerInterpreter, EscalationHandler, LogEscalationHandler};
