//! Client-side auto-link logic: discover an existing daemon or spawn a new one.

use agent_types::WorkplaceId;
use anyhow::{Context, Result};
use std::path::{Path, PathBuf};
use std::time::Duration;
use tokio::process::Command;

use crate::config::DaemonConfig;

/// Result of a successful auto-link.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AutoLinkResult {
    /// WebSocket URL to connect to.
    pub websocket_url: String,
    /// PID of the daemon process.
    pub pid: u32,
    /// `true` if the daemon was spawned by this call.
    pub spawned: bool,
}

/// Attempt to auto-link to a daemon for the given workplace.
///
/// 1. Reads `daemon.json` at the provided path.
/// 2. Validates the PID is still alive (Unix: `kill -0`).
/// 3. If alive, returns the existing daemon's URL.
/// 4. If stale or missing, spawns a new daemon via `daemon_bin` and waits
///    up to `wait_timeout` for `daemon.json` to appear.
pub async fn auto_link(
    workplace_id: &WorkplaceId,
    daemon_json_path: &Path,
    daemon_bin_path: Option<&Path>,
    wait_timeout: Duration,
) -> Result<AutoLinkResult> {
    // Try existing daemon first.
    if let Some(result) = try_existing(daemon_json_path).await? {
        return Ok(result);
    }

    // No valid daemon — need to spawn.
    let bin = daemon_bin_path
        .map(PathBuf::from)
        .or_else(find_daemon_binary)
        .context("daemon binary not found")?;

    tracing::info!(bin = %bin.display(), "spawning daemon");

    let child = Command::new(&bin)
        .arg("--workplace-id")
        .arg(workplace_id.as_str())
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .with_context(|| format!("spawn daemon {}", bin.display()))?;

    let spawned_pid = child.id();
    tracing::debug!(pid = ?spawned_pid, "daemon spawned");

    // Wait for daemon.json to appear with exponential backoff polling.
    let result = wait_for_daemon_json(daemon_json_path, wait_timeout).await?;

    Ok(AutoLinkResult {
        websocket_url: result.websocket_url,
        pid: result.pid,
        spawned: true,
    })
}

/// Try to connect to an existing daemon by reading `daemon.json` and validating PID.
async fn try_existing(daemon_json_path: &Path) -> Result<Option<AutoLinkResult>> {
    let config = match DaemonConfig::read(daemon_json_path).await {
        Ok(c) => c,
        Err(e) => {
            tracing::debug!("no daemon.json or unreadable: {}", e);
            return Ok(None);
        }
    };

    if !is_pid_alive(config.pid) {
        tracing::warn!(pid = config.pid, "stale daemon.json detected (dead PID)");
        // Best-effort removal of stale config.
        let _ = tokio::fs::remove_file(daemon_json_path).await;
        return Ok(None);
    }

    tracing::info!(pid = config.pid, url = %config.websocket_url, "linked to existing daemon");
    Ok(Some(AutoLinkResult {
        websocket_url: config.websocket_url,
        pid: config.pid,
        spawned: false,
    }))
}

/// Check whether a process with the given PID is alive.
#[cfg(unix)]
fn is_pid_alive(pid: u32) -> bool {
    // `kill -0` sends no signal but still validates the PID.
    unsafe { libc::kill(pid as i32, 0) == 0 }
}

#[cfg(not(unix))]
fn is_pid_alive(pid: u32) -> bool {
    // Fallback for non-Unix: always assume dead to force respawn.
    false
}

/// Poll for `daemon.json` to appear, with exponential backoff.
async fn wait_for_daemon_json(
    path: &Path,
    max_wait: Duration,
) -> Result<DaemonConfig> {
    let start = tokio::time::Instant::now();
    let mut delay_ms = 100u64;
    const MAX_DELAY_MS: u64 = 5000;

    while start.elapsed() < max_wait {
        match DaemonConfig::read(path).await {
            Ok(config) => return Ok(config),
            Err(_) => {
                tokio::time::sleep(Duration::from_millis(delay_ms)).await;
                delay_ms = (delay_ms * 2).min(MAX_DELAY_MS);
            }
        }
    }

    anyhow::bail!(
        "daemon.json did not appear within {:?} at {}",
        max_wait,
        path.display()
    );
}

/// Attempt to locate the `agent-daemon` binary.
///
/// 1. `CARGO_BIN_EXE_agent-daemon` env var (set by cargo during tests).
/// 2. `agent-daemon` in `$PATH`.
fn find_daemon_binary() -> Option<PathBuf> {
    if let Ok(path) = std::env::var("CARGO_BIN_EXE_agent-daemon") {
        return Some(PathBuf::from(path));
    }

    // Simple PATH search.
    if let Ok(path_var) = std::env::var("PATH") {
        for dir in path_var.split(std::path::MAIN_SEPARATOR) {
            let candidate = Path::new(dir).join("agent-daemon");
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }

    None
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use agent_types::WorkplaceId;

    #[tokio::test]
    async fn autolink_stale_config_triggers_respawn_logic() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("daemon.json");

        // Write a config with a PID that is extremely unlikely to exist.
        let stale = DaemonConfig::new(
            999_999,
            "ws://127.0.0.1:1/v1/session",
            WorkplaceId::new("wp-stale"),
            None,
        );
        stale.write(&path).await.unwrap();
        assert!(path.exists());

        // try_existing should detect the stale PID and remove the file.
        let result = try_existing(&path).await.unwrap();
        assert!(result.is_none());
        assert!(!path.exists(), "stale daemon.json should be removed");
    }

    #[tokio::test]
    async fn autolink_existing_returns_valid_daemon() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("daemon.json");

        let config = DaemonConfig::new(
            std::process::id(),
            "ws://127.0.0.1:12345/v1/session",
            WorkplaceId::new("wp-live"),
            None,
        );
        config.write(&path).await.unwrap();

        let result = try_existing(&path).await.unwrap().expect("should link");
        assert!(!result.spawned);
        assert_eq!(result.pid, std::process::id());
        assert_eq!(result.websocket_url, "ws://127.0.0.1:12345/v1/session");
    }

    #[tokio::test]
    async fn wait_for_daemon_json_times_out() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("daemon.json");

        let err = wait_for_daemon_json(&path, Duration::from_millis(200))
            .await
            .unwrap_err();
        assert!(format!("{err}").contains("did not appear"));
    }

    #[tokio::test]
    async fn wait_for_daemon_json_succeeds_when_file_appears() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("daemon.json");
        let path_clone = path.clone();

        let config = DaemonConfig::new(
            42,
            "ws://127.0.0.1:4242/v1/session",
            WorkplaceId::new("wp-wait"),
            None,
        );

        // Write the file after a short delay.
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(150)).await;
            config.write(&path_clone).await.unwrap();
        });

        let found = wait_for_daemon_json(&path, Duration::from_secs(5))
            .await
            .unwrap();
        assert_eq!(found.pid, 42);
    }

    #[test]
    fn find_daemon_binary_falls_back_to_path() {
        // We can't guarantee PATH has agent-daemon, but we can at least
        // exercise the code path when CARGO_BIN_EXE is missing.
        unsafe { std::env::remove_var("CARGO_BIN_EXE_agent-daemon") };
        let result = find_daemon_binary();
        // Result depends on environment; just ensure it doesn't panic.
        let _ = result;
    }
}
