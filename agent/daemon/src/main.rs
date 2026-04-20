//! agent-daemon entry point

use agent_daemon::broadcaster::EventBroadcaster;
use agent_daemon::event_log::EventLog;
use agent_daemon::handler::{AgentHandler, HealthHandler, HeartbeatHandler, SessionHandler};
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
}

fn parse_args() -> CliArgs {
    let mut args = CliArgs {
        workplace_id: None,
        alias: None,
        log_file: None,
    };
    let mut iter = std::env::args().skip(1);
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--workplace-id" => args.workplace_id = iter.next(),
            "--alias" => args.alias = iter.next(),
            "--log-file" => args.log_file = iter.next(),
            _ => {}
        }
    }
    args
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    let args = parse_args();

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

    // Bootstrap session manager (owns runtime state).
    let session_mgr = Arc::new(
        SessionManager::bootstrap(workplace.cwd().to_path_buf(), workplace.workplace_id().clone())
            .await?,
    );

    // Event infrastructure.
    let broadcaster = EventBroadcaster::new();
    let _event_log = Arc::new(EventLog::open(&event_log_path).await?);
    let metrics = Arc::new(DaemonMetrics::default());

    let mut lifecycle = DaemonLifecycle::new(config_path.clone());
    let (server, _config) = lifecycle.start(&workplace, args.alias).await?;

    let addr = server.local_addr();
    tracing::info!("agent-daemon listening on ws://{}/v1/session", addr);

    let mut router = Router::new();
    router.register("session.initialize", Arc::new(SessionHandler::new(session_mgr.clone())));
    router.register("session.heartbeat", Arc::new(HeartbeatHandler));
    router.register("session.health", Arc::new(HealthHandler::new(metrics.clone())));
    router.register("agent.spawn", Arc::new(AgentHandler::new(session_mgr.clone())));
    router.register("agent.stop", Arc::new(AgentHandler::new(session_mgr.clone())));
    router.register("agent.list", Arc::new(AgentHandler::new(session_mgr.clone())));
    let router_handle = router.handle();

    // Spawn the server run loop in a separate task so we can wait for signals.
    let server_handle = tokio::spawn({
        let broadcaster = broadcaster.clone();
        async move {
            let res = lifecycle
                .run(server, |stream, peer_addr| {
                    let router = router_handle.clone();
                    let broadcaster = broadcaster.clone();
                    async move {
                        let event_rx = broadcaster.register("conn".to_string()).await;
                        agent_daemon::connection::Connection::spawn(
                            stream, peer_addr, router, Some(event_rx),
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
