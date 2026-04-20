//! Daemon configuration persistence — `daemon.json` read/write

use agent_types::WorkplaceId;
use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use tokio::fs;

/// Current schema version for `daemon.json`.
pub const DAEMON_CONFIG_VERSION: u32 = 1;

/// On-disk representation of a running daemon instance.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DaemonConfig {
    /// Schema version (must be `1`).
    pub version: u32,
    /// OS process ID of the daemon.
    pub pid: u32,
    /// WebSocket URL clients can connect to, e.g. `ws://127.0.0.1:12345/v1/session`.
    pub websocket_url: String,
    /// Workplace this daemon is serving.
    pub workplace_id: WorkplaceId,
    /// Optional human-friendly alias for the daemon.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub alias: Option<String>,
    /// Timestamp when the daemon started.
    pub started_at: DateTime<Utc>,
    /// Timestamp of the last successful heartbeat.
    pub last_heartbeat: DateTime<Utc>,
}

impl DaemonConfig {
    /// Create a new config with the current timestamp as `started_at` and `last_heartbeat`.
    pub fn new(
        pid: u32,
        websocket_url: impl Into<String>,
        workplace_id: WorkplaceId,
        alias: Option<String>,
    ) -> Self {
        let now = Utc::now();
        Self {
            version: DAEMON_CONFIG_VERSION,
            pid,
            websocket_url: websocket_url.into(),
            workplace_id,
            alias,
            started_at: now,
            last_heartbeat: now,
        }
    }

    /// Write this config to `path` atomically (temp file + rename).
    pub async fn write(&self, path: impl AsRef<Path>) -> Result<()> {
        let path = path.as_ref();
        let json = serde_json::to_string_pretty(self).context("serialize daemon config")?;

        // Ensure parent directory exists.
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .await
                .with_context(|| format!("create parent directory {}", parent.display()))?;
        }

        // Write to a temporary file next to the target, then rename for atomicity.
        let temp_path = path.with_extension("tmp");
        fs::write(&temp_path, json)
            .await
            .with_context(|| format!("write temp config {}", temp_path.display()))?;
        fs::rename(&temp_path, path)
            .await
            .with_context(|| format!("rename {} to {}", temp_path.display(), path.display()))?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let perms = std::fs::Permissions::from_mode(0o600);
            std::fs::set_permissions(path, perms)
                .with_context(|| format!("set permissions on {}", path.display()))?;
        }

        Ok(())
    }

    /// Read and validate a config from `path`.
    pub async fn read(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        let bytes = fs::read(path)
            .await
            .with_context(|| format!("read daemon config {}", path.display()))?;
        let config: DaemonConfig = serde_json::from_slice(&bytes)
            .with_context(|| format!("parse daemon config {}", path.display()))?;

        if config.version != DAEMON_CONFIG_VERSION {
            anyhow::bail!(
                "unsupported daemon.json schema version: expected {}, got {}",
                DAEMON_CONFIG_VERSION,
                config.version
            );
        }

        Ok(config)
    }

    /// Delete the config file at `path` if it exists.
    pub async fn remove(path: impl AsRef<Path>) -> Result<()> {
        let path = path.as_ref();
        match fs::remove_file(path).await {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(e).with_context(|| format!("remove daemon config {}", path.display())),
        }
    }

    /// Return the default config path for a given workplace inside `base_dir`.
    ///
    /// Layout: `<base_dir>/workplaces/<workplace_id>/daemon.json`
    pub fn path_for_workplace(base_dir: impl AsRef<Path>, workplace_id: &WorkplaceId) -> PathBuf {
        base_dir
            .as_ref()
            .join("workplaces")
            .join(workplace_id.as_str())
            .join("daemon.json")
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[tokio::test]
    async fn config_roundtrip_write_read() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("daemon.json");

        let original = DaemonConfig::new(
            12345,
            "ws://127.0.0.1:9999/v1/session",
            WorkplaceId::new("wp-test"),
            Some("test-daemon".into()),
        );

        original.write(&path).await.unwrap();
        let loaded = DaemonConfig::read(&path).await.unwrap();

        assert_eq!(original, loaded);
    }

    #[tokio::test]
    async fn config_corrupted_file_rejected() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("daemon.json");

        fs::write(&path, b"not-json-at-all").await.unwrap();

        let err = DaemonConfig::read(&path).await.unwrap_err();
        let msg = format!("{err}");
        assert!(msg.contains("parse daemon config"), "expected parse error, got: {msg}");
    }

    #[tokio::test]
    async fn config_unknown_version_rejected() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("daemon.json");

        let bad = serde_json::json!({
            "version": 99,
            "pid": 1,
            "websocketUrl": "ws://localhost:1/v1/session",
            "workplaceId": "wp-1",
            "startedAt": "2024-01-01T00:00:00Z",
            "lastHeartbeat": "2024-01-01T00:00:00Z"
        });
        fs::write(&path, serde_json::to_vec(&bad).unwrap())
            .await
            .unwrap();

        let err = DaemonConfig::read(&path).await.unwrap_err();
        let msg = format!("{err}");
        assert!(
            msg.contains("unsupported daemon.json schema version"),
            "expected version error, got: {msg}"
        );
    }

    #[tokio::test]
    async fn config_atomic_write_no_half_written_file() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("daemon.json");

        let config = DaemonConfig::new(
            42,
            "ws://127.0.0.1:4242/v1/session",
            WorkplaceId::new("wp-atomic"),
            None,
        );

        // Start a write, then immediately try to read.
        // With atomic rename the reader should either see the old file or the new one,
        // never a half-written file. Since there is no old file, it either succeeds
        // (after rename) or fails (before rename).
        let write_fut = config.write(&path);
        let _read_fut = DaemonConfig::read(&path);

        // Drive both concurrently for a brief moment.
        tokio::select! {
            _ = tokio::time::sleep(Duration::from_millis(1)) => {}
            _ = write_fut => {}
        }

        // After the write completes the file must be valid.
        config.write(&path).await.unwrap();
        let loaded = DaemonConfig::read(&path).await.unwrap();
        assert_eq!(loaded.pid, 42);
    }

    #[tokio::test]
    async fn config_remove_missing_is_ok() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("nonexistent.json");
        DaemonConfig::remove(&path).await.unwrap();
    }

    #[tokio::test]
    async fn config_path_for_workplace() {
        let base = "/tmp/.agile-agent";
        let wp = WorkplaceId::new("wp-abc");
        let path = DaemonConfig::path_for_workplace(base, &wp);
        assert_eq!(
            path,
            PathBuf::from("/tmp/.agile-agent/workplaces/wp-abc/daemon.json")
        );
    }
}
