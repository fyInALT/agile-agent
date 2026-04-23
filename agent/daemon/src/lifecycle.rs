//! Daemon startup, heartbeat, and graceful shutdown sequence.

use agent_protocol::config::DaemonConfig;
use agent_protocol::state::SessionState;
use crate::server::{ShutdownHandle, WebSocketServer};
use crate::session_mgr::SessionManager;
use agent_protocol::workplace::ResolvedWorkplace;
use anyhow::{Context, Result};
use chrono::Utc;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::watch;

/// Owns the full lifecycle of a daemon instance.
pub struct DaemonLifecycle {
    /// Path to the `daemon.json` file.
    pub config_path: PathBuf,
    /// Handle to trigger server shutdown.
    server_shutdown: Option<ShutdownHandle>,
    /// Broadcast channel for shutdown notification.
    shutdown_tx: watch::Sender<bool>,
    _shutdown_rx: watch::Receiver<bool>,
}

impl DaemonLifecycle {
    /// Create a new lifecycle manager.
    pub fn new(config_path: PathBuf) -> Self {
        let (shutdown_tx, _shutdown_rx) = watch::channel(false);
        Self {
            config_path,
            server_shutdown: None,
            shutdown_tx,
            _shutdown_rx,
        }
    }

    /// Start the daemon: bind server, write config, spawn heartbeat.
    ///
    /// Returns the bound [`WebSocketServer`] and the [`DaemonConfig`] that was written.
    pub async fn start(
        &mut self,
        workplace: &ResolvedWorkplace,
        alias: Option<String>,
        bind_addr: Option<&str>,
    ) -> Result<(WebSocketServer, DaemonConfig)> {
        workplace.ensure().await.context("ensure workplace directory")?;

        let server = match bind_addr {
            Some(addr) => WebSocketServer::bind_to(addr).await,
            None => WebSocketServer::bind().await,
        }
        .context("bind WebSocket server")?;
        let addr = server.local_addr();
        let ws_url = format!("ws://{}/v1/session", addr);
        let pid = std::process::id();

        let config =
            DaemonConfig::new(pid, ws_url, workplace.workplace_id().clone(), alias);
        config
            .write(&self.config_path)
            .await
            .context("write daemon.json")?;

        tracing::info!(
            pid = pid,
            port = addr.port(),
            workplace = %workplace.workplace_id().as_str(),
            "daemon started"
        );

        self.server_shutdown = Some(server.shutdown_handle());

        // Spawn heartbeat updater.
        let heartbeat_path = self.config_path.clone();
        let mut heartbeat_rx = self.shutdown_tx.subscribe();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(30));
            loop {
                tokio::select! {
                    _ = interval.tick() => {
                        match DaemonConfig::read(&heartbeat_path).await {
                            Ok(mut cfg) => {
                                cfg.last_heartbeat = Utc::now();
                                if let Err(e) = cfg.write(&heartbeat_path).await {
                                    tracing::warn!("heartbeat update failed: {}", e);
                                }
                            }
                            Err(e) => {
                                tracing::warn!("heartbeat read failed: {}", e);
                            }
                        }
                    }
                    Ok(()) = heartbeat_rx.changed() => {
                        if *heartbeat_rx.borrow() {
                            break;
                        }
                    }
                }
            }
            tracing::debug!("heartbeat task stopped");
        });

        Ok((server, config))
    }

    /// Run the server accept loop until a shutdown signal is received.
    pub async fn run<F, Fut>(
        &self,
        server: WebSocketServer,
        on_connect: F,
    ) -> Result<()>
    where
        F: FnMut(tokio::net::TcpStream, std::net::SocketAddr) -> Fut,
        Fut: std::future::Future<Output = ()>,
    {
        let mut sig_rx = self.shutdown_tx.subscribe();

        tokio::select! {
            res = server.run(on_connect) => {
                res.context("server run")?;
            }
            Ok(()) = sig_rx.changed() => {
                if *sig_rx.borrow() {
                    tracing::info!("shutdown signal received, stopping server");
                }
            }
        }

        Ok(())
    }

    /// Trigger graceful shutdown.
    pub async fn shutdown(
        &mut self,
        snapshot_path: Option<PathBuf>,
        session_mgr: Option<Arc<SessionManager>>,
    ) -> Result<()> {
        tracing::info!("graceful shutdown initiated");

        // 1. Stop accepting new connections.
        if let Some(handle) = self.server_shutdown.take() {
            handle.shutdown();
        }

        // 2. Notify internal tasks.
        let _ = self.shutdown_tx.send(true);

        // 3. Wait briefly for existing connections to drain.
        tokio::time::sleep(Duration::from_secs(1)).await;

        // 4. Terminate all active provider subprocesses (claude/codex).
        if let Some(mgr) = &session_mgr {
            if let Err(e) = mgr.terminate_all_provider_processes().await {
                tracing::warn!("failed to terminate provider processes: {}", e);
            }
            // Wait briefly for processes to handle SIGTERM
            tokio::time::sleep(Duration::from_millis(500)).await;
        }

        // 5. Write snapshots from SessionManager if available.
        //    - ShutdownSnapshot (core format) for resume on next startup
        //    - SnapshotFile (protocol format) for external tools / monitoring
        if let Some(mgr) = session_mgr {
            if let Err(e) = mgr
                .save_shutdown_snapshot(agent_core::shutdown_snapshot::ShutdownReason::UserQuit)
                .await
            {
                tracing::warn!("failed to write shutdown snapshot: {}", e);
            } else {
                tracing::info!("shutdown snapshot written for resume");
            }
            if let Some(ref path) = snapshot_path {
                if let Err(e) = mgr.write_snapshot(&path).await {
                    tracing::warn!("failed to write session snapshot: {}", e);
                } else {
                    tracing::info!(path = %path.display(), "session snapshot written");
                }
            }
        } else {
            // Fallback: write hardcoded snapshot.
            if let Some(ref path) = snapshot_path {
                let snapshot = serde_json::json!({
                    "version": 1,
                    "sessionState": SessionState::default(),
                    "shutdownAt": Utc::now().to_rfc3339(),
                });
                let json = serde_json::to_string_pretty(&snapshot).context("serialize snapshot")?;
                tokio::fs::write(&path, json)
                    .await
                    .with_context(|| format!("write snapshot {}", path.display()))?;
                tracing::info!(path = %path.display(), "snapshot written");
            }
        }

        // 6. Create backup of snapshot + event log (retain last 3).
        if let Some(ref path) = snapshot_path {
            // Use snapshot_path's parent, fallback to config_path's parent (workplace dir)
            let backup_dir = path.parent()
                .or_else(|| self.config_path.parent())
                .unwrap_or_else(|| std::path::Path::new("."));
            if let Err(e) = Self::rotate_backups(backup_dir).await {
                tracing::warn!("failed to rotate backups: {}", e);
            }
        }

        // 7. Delete daemon.json.
        DaemonConfig::remove(&self.config_path)
            .await
            .context("remove daemon.json")?;
        tracing::info!("daemon.json removed");

        Ok(())
    }

    /// Rotate backups in `dir/.backups/`: keep last 3, delete older ones.
    async fn rotate_backups(dir: &std::path::Path) -> Result<()> {
        let backup_dir = dir.join(".backups");
        tokio::fs::create_dir_all(&backup_dir).await?;

        let timestamp = chrono::Utc::now().format("%Y%m%d_%H%M%S_%3f");
        let snapshot_src = dir.join("snapshot.json");
        let events_src = dir.join("events.jsonl");

        if snapshot_src.exists() {
            let dst = backup_dir.join(format!("snapshot_{}.json", timestamp));
            tokio::fs::copy(&snapshot_src, &dst).await?;
            tracing::info!("backup created: {}", dst.display());
        }
        if events_src.exists() {
            let dst = backup_dir.join(format!("events_{}.jsonl", timestamp));
            tokio::fs::copy(&events_src, &dst).await?;
            tracing::info!("backup created: {}", dst.display());
        }

        // Clean up old backups (keep last 3 of each type).
        let mut entries = Vec::new();
        let mut read_dir = tokio::fs::read_dir(&backup_dir).await?;
        while let Ok(Some(entry)) = read_dir.next_entry().await {
            entries.push(entry);
        }
        entries.sort_by_key(|e| e.file_name());
        entries.reverse();

        let mut snapshot_count = 0;
        let mut events_count = 0;
        for entry in entries {
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with("snapshot_") {
                snapshot_count += 1;
                if snapshot_count > 3 {
                    tokio::fs::remove_file(entry.path()).await?;
                    tracing::info!("old backup removed: {}", entry.path().display());
                }
            } else if name.starts_with("events_") {
                events_count += 1;
                if events_count > 3 {
                    tokio::fs::remove_file(entry.path()).await?;
                    tracing::info!("old backup removed: {}", entry.path().display());
                }
            }
        }

        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use agent_protocol::workplace::ResolvedWorkplace;

    #[tokio::test]
    async fn lifecycle_start_writes_config() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("workplaces");
        let cwd = tmp.path().join("my-project");
        tokio::fs::create_dir_all(&cwd).await.unwrap();

        unsafe { std::env::set_var("AGILE_AGENT_WORKPLACES_ROOT", &root) };
        let wp = ResolvedWorkplace::for_cwd(&cwd, root).unwrap();
        let config_path = wp.daemon_json_path();

        let mut lifecycle = DaemonLifecycle::new(config_path.clone());
        let (server, config) = lifecycle.start(&wp, None, None).await.unwrap();

        assert!(config_path.exists());
        assert_eq!(config.pid, std::process::id());
        assert!(config.websocket_url.contains(&server.local_addr().port().to_string()));

        lifecycle.shutdown(None, None).await.unwrap();
    }

    #[tokio::test]
    async fn lifecycle_shutdown_removes_config() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("workplaces");
        let cwd = tmp.path().join("my-project");
        tokio::fs::create_dir_all(&cwd).await.unwrap();

        unsafe { std::env::set_var("AGILE_AGENT_WORKPLACES_ROOT", &root) };
        let wp = ResolvedWorkplace::for_cwd(&cwd, root).unwrap();
        let config_path = wp.daemon_json_path();
        let snapshot_path = wp.snapshot_path();

        let mut lifecycle = DaemonLifecycle::new(config_path.clone());
        let (server, _) = lifecycle.start(&wp, Some("test".into()), None).await.unwrap();

        assert!(server.local_addr().port() > 0);

        lifecycle.shutdown(Some(snapshot_path.clone()), None).await.unwrap();

        assert!(!config_path.exists(), "daemon.json should be removed");
        assert!(snapshot_path.exists(), "snapshot.json should be written");
    }

    #[tokio::test]
    async fn shutdown_creates_backup_and_retains_three() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("wp");
        tokio::fs::create_dir_all(&dir).await.unwrap();

        // Pre-create snapshot and event log.
        tokio::fs::write(dir.join("snapshot.json"), b"{}")
            .await
            .unwrap();
        tokio::fs::write(dir.join("events.jsonl"), b"\n")
            .await
            .unwrap();

        // Run shutdown 4 times to trigger rotation.
        for _ in 0..4 {
            let mut lifecycle = DaemonLifecycle::new(dir.join("daemon.json"));
            lifecycle.shutdown(Some(dir.join("snapshot.json")), None).await.unwrap();
            // Recreate daemon.json for next iteration (shutdown removes it).
            tokio::fs::write(dir.join("daemon.json"), b"{}").await.unwrap();
        }

        let backup_dir = dir.join(".backups");
        assert!(backup_dir.exists());

        let entries: Vec<_> = std::fs::read_dir(&backup_dir).unwrap().filter_map(|e| e.ok()).collect();
        let snapshots: Vec<_> = entries.iter().filter(|e| e.file_name().to_string_lossy().starts_with("snapshot_")).collect();
        assert_eq!(snapshots.len(), 3, "should retain only 3 snapshot backups");
    }
}
