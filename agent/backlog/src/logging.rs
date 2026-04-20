//! Stub logging module for standalone crate compilation
//!
//! This module provides no-op logging functions when the backlog crate
//! is compiled standalone. When used within the full workspace, the
//! actual logging implementation from agent-core is used.

/// No-op debug event logging
pub fn debug_event(_event_type: &str, _message: &str, _payload: serde_json::Value) {
    // No-op in standalone compilation
}

/// No-op warning event logging
pub fn warn_event(_event_type: &str, _message: &str, _payload: serde_json::Value) {
    // No-op in standalone compilation
}

/// No-op info event logging
pub fn info_event(_event_type: &str, _message: &str, _payload: serde_json::Value) {
    // No-op in standalone compilation
}