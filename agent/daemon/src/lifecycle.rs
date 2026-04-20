//! Daemon startup, heartbeat, and graceful shutdown sequence.

use agent_protocol::config::DaemonConfig;
use crate::server::{ShutdownHandle, WebSocketServer};
use crate::workplace::ResolvedWorkplace;
use anyhow::{Context, Result};
use chrono::Utc;
use std::path::PathBuf;
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
    ) -> Result<(WebSocketServer, DaemonConfig)> {
        workplace.ensure().await.context("ensure workplace directory")?;

        let server = WebSocketServer::bind().await.context("bind WebSocket server")?;
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
                        // Read, update heartbeat, write atomically.
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
    pub async fn shutdown(&mut self, snapshot_path: Option<PathBuf>) -> Result<()> {
        tracing::info!("graceful shutdown initiated");

        // 1. Stop accepting new connections.
        if let Some(handle) = self.server_shutdown.take() {
            handle.shutdown();
        }

        // 2. Notify internal tasks.
        let _ = self.shutdown_tx.send(true);

        // 3. Wait briefly for existing connections to drain.
        tokio::time::sleep(Duration::from_secs(1)).await;

        // 4. Write snapshot (hardcoded schema version 1 for now).
        if let Some(path) = snapshot_path {
            let snapshot = serde_json::json!({
                "version": 1,
                "sessionState": serde_json::Value::Null,
                "shutdownAt": Utc::now().to_rfc3339(),
            });
            let json = serde_json::to_string_pretty(&snapshot).context("serialize snapshot")?;
            tokio::fs::write(&path, json)
                .await
                .with_context(|| format!("write snapshot {}", path.display()))?;
            tracing::info!(path = %path.display(), "snapshot written");
        }

        // 5. Delete daemon.json.
        DaemonConfig::remove(&self.config_path)
            .await
            .context("remove daemon.json")?;
        tracing::info!("daemon.json removed");

        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workplace::ResolvedWorkplace;

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
        let (server, config) = lifecycle.start(&wp, None).await.unwrap();

        assert!(config_path.exists());
        assert_eq!(config.pid, std::process::id());
        assert!(config.websocket_url.contains(&server.local_addr().port().to_string()));

        // Clean up
        lifecycle.shutdown(None).await.unwrap();
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
        let (server, _) = lifecycle.start(&wp, Some("test".into())).await.unwrap();

        // Sanity: server is bound
        assert!(server.local_addr().port() > 0);

        lifecycle.shutdown(Some(snapshot_path.clone())).await.unwrap();

        assert!(!config_path.exists(), "daemon.json should be removed");
        assert!(snapshot_path.exists(), "snapshot.json should be written");
    }
}
