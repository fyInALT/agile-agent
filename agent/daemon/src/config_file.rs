//! Daemon configuration file support (`daemon.toml`).

use serde::{Deserialize, Serialize};
use std::path::Path;

/// Daemon configuration loaded from `daemon.toml` in the workplace directory.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DaemonConfigFile {
    /// WebSocket bind address (default: 127.0.0.1:0 for ephemeral port).
    #[serde(default = "default_bind")]
    pub bind: String,
    /// Heartbeat timeout in seconds (default: 120).
    #[serde(default = "default_heartbeat_timeout")]
    pub heartbeat_timeout: u64,
    /// Maximum number of concurrent WebSocket clients (default: 10).
    #[serde(default = "default_max_clients")]
    pub max_clients: usize,
    /// Event log retention: max file size in MB before rotation (default: 100).
    #[serde(default = "default_max_event_log_mb")]
    pub max_event_log_mb: u64,
}

fn default_bind() -> String {
    "127.0.0.1:0".to_string()
}

fn default_heartbeat_timeout() -> u64 {
    120
}

fn default_max_clients() -> usize {
    10
}

fn default_max_event_log_mb() -> u64 {
    100
}

impl Default for DaemonConfigFile {
    fn default() -> Self {
        Self {
            bind: default_bind(),
            heartbeat_timeout: default_heartbeat_timeout(),
            max_clients: default_max_clients(),
            max_event_log_mb: default_max_event_log_mb(),
        }
    }
}

impl DaemonConfigFile {
    /// Load from a TOML file, returning defaults if the file is missing.
    pub fn load(path: &Path) -> anyhow::Result<Self> {
        if !path.exists() {
            return Ok(Self::default());
        }
        let text = std::fs::read_to_string(path)?;
        let config: Self = toml::from_str(&text)?;
        Ok(config)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn load_missing_returns_defaults() {
        let cfg = DaemonConfigFile::load(Path::new("/nonexistent/daemon.toml")).unwrap();
        assert_eq!(cfg.bind, "127.0.0.1:0");
        assert_eq!(cfg.heartbeat_timeout, 120);
        assert_eq!(cfg.max_clients, 10);
    }
}
