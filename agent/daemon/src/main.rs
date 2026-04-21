//! agent-daemon entry point

use agent_daemon::broadcaster::EventBroadcaster;
use agent_daemon::config_file::DaemonConfigFile;
use agent_daemon::connection::ConnectionTracker;
use agent_daemon::event_log::EventLog;
use agent_daemon::handler::{AgentHandler, HealthHandler, HeartbeatHandler, MetricsHandler, SessionHandler};
use agent_daemon::lifecycle::DaemonLifecycle;
use agent_daemon::router::Router;
use agent_daemon::session_mgr::SessionManager;
use agent_daemon::health::DaemonMetrics;
use agent_daemon::workplace;
use std::sync::Arc;

#[derive(Debug)]
struct CliArgs {
    workplace_id: Option<String>,
    alias: Option<String>,
    log_file: Option<String>,
    json_log: bool,
}

fn parse_args<I>(iter: I) -> CliArgs
where
    I: Iterator<Item = String>,
{
    let mut args = CliArgs {
        workplace_id: None,
        alias: None,
        log_file: None,
        json_log: false,
    };
    let mut iter = iter.skip(1);
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--workplace-id" => args.workplace_id = iter.next(),
            "--alias" => args.alias = iter.next(),
            "--log-file" => args.log_file = iter.next(),
            "--json-log" => args.json_log = true,
            _ => {}
        }
    }
    args
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = parse_args(std::env::args());

    if args.json_log {
        tracing_subscriber::fmt().json().init();
    } else {
        tracing_subscriber::fmt::init();
    }

    // Resolve workplace.
    let workplace = if let Some(_id) = args.workplace_id {
        let cwd = std::env::current_dir()?;
        let root = workplace::resolve_workplaces_root()?;
        workplace::ResolvedWorkplace::for_cwd(
            &cwd,
            root,
        )?
    } else {
        workplace::resolve_workplace()?
    };

    let config_path = workplace.daemon_json_path();
    let snapshot_path = workplace.snapshot_path();
    let event_log_path = workplace.path().join("events.jsonl");
    let audit_log_path = workplace.path().join("audit.jsonl");
    let audit_log = std::sync::Arc::new(Some(agent_daemon::audit::AuditLog::new(&audit_log_path)));

    // Load daemon configuration file (optional, defaults apply if missing).
    let daemon_toml = workplace.path().join("daemon.toml");
    let mut config_file = DaemonConfigFile::load(&daemon_toml)?;
    // Auto-generate bearer token if auth is enabled but no token is configured.
    if config_file.bearer_token.is_none() && std::env::var("AGILE_AGENT_AUTO_TOKEN").is_ok() {
        let token: String = (0..32)
            .map(|_| rand::Rng::gen_range(&mut rand::thread_rng(), 0..16))
            .map(|i| format!("{:x}", i))
            .collect();
        config_file.bearer_token = Some(token);
        tracing::info!("auto-generated bearer token for daemon");
    }
    let bind_addr = config_file.bind.clone();

    // Bootstrap session manager (owns runtime state).
    let session_mgr = Arc::new(
        SessionManager::bootstrap(workplace.cwd().to_path_buf(), workplace.workplace_id().clone())
            .await?,
    );

    // Event infrastructure.
    let broadcaster = EventBroadcaster::new();
    let _event_log = Arc::new(EventLog::open_with_max_size(&event_log_path, config_file.max_event_log_mb).await?);
    let metrics = Arc::new(DaemonMetrics::default());
    let tracker = ConnectionTracker::new(config_file.max_clients);

    let mut lifecycle = DaemonLifecycle::new(config_path.clone());
    let (server, _config) = lifecycle.start(&workplace, args.alias, Some(&bind_addr)).await?;

    let addr = server.local_addr();
    tracing::info!("agent-daemon listening on ws://{}/v1/session", addr);

    agent_daemon::health::spawn_memory_monitor();

    let mut router = Router::new();
    router.register("session.initialize", Arc::new(SessionHandler::new(session_mgr.clone())));
    router.register("session.heartbeat", Arc::new(HeartbeatHandler));
    router.register("session.health", Arc::new(HealthHandler::new(metrics.clone())));
    router.register("session.metrics", Arc::new(MetricsHandler::new(metrics.clone())));
    router.register("agent.spawn", Arc::new(AgentHandler::new(session_mgr.clone())));
    router.register("agent.stop", Arc::new(AgentHandler::new(session_mgr.clone())));
    router.register("agent.list", Arc::new(AgentHandler::new(session_mgr.clone())));
    let router_handle = router.handle();

    // Spawn the server run loop in a separate task so we can wait for signals.
    let bearer_token = config_file.bearer_token.clone();
    let server_handle = tokio::spawn({
        let broadcaster = broadcaster.clone();
        let tracker = tracker.clone();
        let audit_log_clone = (*audit_log).clone();
        async move {
            let res = lifecycle
                .run(server, move |stream, peer_addr| {
                    let router = router_handle.clone();
                    let broadcaster = broadcaster.clone();
                    let tracker = tracker.clone();
                    let bearer_token = bearer_token.clone();
                    let audit_log = audit_log_clone.clone();
                    async move {
                        let event_rx = broadcaster.register("conn".to_string()).await;
                        agent_daemon::connection::Connection::spawn(
                            stream, peer_addr, router, Some(event_rx), Some(tracker), bearer_token,
                            Some(agent_daemon::connection::RateLimiter::new(100)),
                            audit_log,
                        );
                    }
                })
                .await;
            (lifecycle, res)
        }
    });

    // Wait for termination signal (Unix: SIGTERM/SIGINT, Windows: Ctrl+C).
    #[cfg(unix)]
    {
        let mut sigterm = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())?;
        let mut sigint = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::interrupt())?;
        tokio::select! {
            _ = sigterm.recv() => {
                tracing::info!("received SIGTERM");
            }
            _ = sigint.recv() => {
                tracing::info!("received SIGINT");
            }
        }
    }
    #[cfg(not(unix))]
    {
        tokio::signal::ctrl_c().await?;
        tracing::info!("received Ctrl+C");
    }

    // Trigger graceful shutdown.
    let (mut lifecycle, server_res) = server_handle.await?;
    server_res?;
    lifecycle.shutdown(Some(snapshot_path), Some(session_mgr)).await?;

    tracing::info!("daemon exited cleanly");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_args_extracts_flags() {
        let args = parse_args(
            vec!["agent-daemon", "--json-log", "--workplace-id", "wp1", "--alias", "a1"]
                .into_iter()
                .map(String::from),
        );
        assert!(args.json_log);
        assert_eq!(args.workplace_id, Some("wp1".to_string()));
        assert_eq!(args.alias, Some("a1".to_string()));
    }

    #[test]
    fn parse_args_defaults() {
        let args = parse_args(vec!["agent-daemon"].into_iter().map(String::from));
        assert!(!args.json_log);
        assert!(args.workplace_id.is_none());
        assert!(args.alias.is_none());
        assert!(args.log_file.is_none());
    }
}
