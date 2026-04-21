//! Minimal logging facade for agent-worktree
//!
//! Provides debug/warn event logging via tracing so output can be
//! captured by a subscriber instead of going straight to stderr.

/// Log a debug event.
pub fn debug_event(event: &str, message: &str, fields: serde_json::Value) {
    tracing::debug!(target: "agile-agent", event = %event, fields = %fields, "{}", message);
}

/// Log a warning event.
pub fn warn_event(event: &str, message: &str, fields: serde_json::Value) {
    tracing::warn!(target: "agile-agent", event = %event, fields = %fields, "{}", message);
}

/// Log an error event.
pub fn error_event(event: &str, message: &str, fields: serde_json::Value) {
    tracing::error!(target: "agile-agent", event = %event, fields = %fields, "{}", message);
}
