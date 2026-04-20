//! Minimal logging facade for agent-provider
//!
//! Provides debug/warn event logging. When agent-provider is integrated with
//! agent-core, the core logging module handles file persistence.

/// Log a debug event (stub implementation)
pub fn debug_event(event: &str, message: &str, fields: serde_json::Value) {
    // In standalone mode, log to stderr
    eprintln!(
        "[DEBUG] {} - {} | {}",
        event,
        message,
        serde_json::to_string(&fields).unwrap_or_default()
    );
}

/// Log a warning event (stub implementation)
pub fn warn_event(event: &str, message: &str, fields: serde_json::Value) {
    eprintln!(
        "[WARN] {} - {} | {}",
        event,
        message,
        serde_json::to_string(&fields).unwrap_or_default()
    );
}

/// Log an error event (stub implementation)
pub fn error_event(event: &str, message: &str, fields: serde_json::Value) {
    eprintln!(
        "[ERROR] {} - {} | {}",
        event,
        message,
        serde_json::to_string(&fields).unwrap_or_default()
    );
}