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
    /// Optional bearer token for WebSocket authentication.
    #[serde(default)]
    pub bearer_token: Option<String>,
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
            bearer_token: None,
        }
    }
}

impl DaemonConfigFile {
    /// Load from a TOML file, returning defaults if the file is missing.
    /// Environment variables with prefix `AGILE_AGENT_` override file values.
    pub fn load(path: &Path) -> anyhow::Result<Self> {
        let mut config = if !path.exists() {
            Self::default()
        } else {
            let text = std::fs::read_to_string(path)?;
            toml::from_str(&text)?
        };

        // Apply env var overrides: AGILE_AGENT_BIND, AGILE_AGENT_HEARTBEAT_TIMEOUT, etc.
        if let Ok(val) = std::env::var("AGILE_AGENT_BIND") {
            config.bind = val;
        }
        if let Ok(val) = std::env::var("AGILE_AGENT_HEARTBEAT_TIMEOUT") {
            if let Ok(v) = val.parse() {
                config.heartbeat_timeout = v;
            }
        }
        if let Ok(val) = std::env::var("AGILE_AGENT_MAX_CLIENTS") {
            if let Ok(v) = val.parse() {
                config.max_clients = v;
            }
        }
        if let Ok(val) = std::env::var("AGILE_AGENT_MAX_EVENT_LOG_MB") {
            if let Ok(v) = val.parse() {
                config.max_event_log_mb = v;
            }
        }
        if let Ok(val) = std::env::var("AGILE_AGENT_BEARER_TOKEN") {
            config.bearer_token = Some(val);
        }

        Ok(config)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;

    #[test]
    #[serial(env_vars)]
    fn load_missing_returns_defaults() {
        // Clear any env vars that might override defaults (parallel test safety)
        unsafe {
            std::env::remove_var("AGILE_AGENT_BIND");
            std::env::remove_var("AGILE_AGENT_HEARTBEAT_TIMEOUT");
            std::env::remove_var("AGILE_AGENT_MAX_CLIENTS");
            std::env::remove_var("AGILE_AGENT_MAX_EVENT_LOG_MB");
            std::env::remove_var("AGILE_AGENT_BEARER_TOKEN");
        }
        let cfg = DaemonConfigFile::load(Path::new("/nonexistent/daemon.toml")).unwrap();
        assert_eq!(cfg.bind, "127.0.0.1:0");
        assert_eq!(cfg.heartbeat_timeout, 120);
        assert_eq!(cfg.max_clients, 10);
        assert_eq!(cfg.bearer_token, None);
    }

    #[test]
    #[serial(env_vars)]
    fn env_var_overrides_defaults() {
        unsafe { std::env::set_var("AGILE_AGENT_BIND", "127.0.0.1:9999") };
        unsafe { std::env::set_var("AGILE_AGENT_MAX_CLIENTS", "42") };
        let cfg = DaemonConfigFile::load(Path::new("/nonexistent/daemon.toml")).unwrap();
        assert_eq!(cfg.bind, "127.0.0.1:9999");
        assert_eq!(cfg.max_clients, 42);
        unsafe { std::env::remove_var("AGILE_AGENT_BIND") };
        unsafe { std::env::remove_var("AGILE_AGENT_MAX_CLIENTS") };
    }
}
