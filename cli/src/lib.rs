#[cfg(feature = "core")]
pub mod app_runner;

#[cfg(not(feature = "core"))]
pub mod protocol_client;

#[cfg(not(feature = "core"))]
pub mod app_runner {
    use anyhow::Result;
    use std::time::Duration;

    fn find_daemon_binary() -> Option<std::path::PathBuf> {
        if let Ok(exe) = std::env::current_exe() {
            if let Some(dir) = exe.parent() {
                let candidate = dir.join("agent-daemon");
                if candidate.is_file() {
                    return Some(candidate);
                }
            }
        }
        None
    }

    pub fn run() -> Result<()> {
        let workplace = agent_protocol::workplace::resolve_workplace()?;

        let result = {
            let rt = tokio::runtime::Runtime::new()?;
            rt.block_on(async {
                workplace.ensure().await?;
                agent_protocol::client::auto_link::auto_link(
                    workplace.workplace_id(),
                    &workplace.daemon_json_path(),
                    find_daemon_binary().as_deref(),
                    Duration::from_secs(10),
                )
                .await
            })?
        };

        println!("Daemon linked");
        println!("  PID:     {}", result.pid);
        println!("  URL:     {}", result.websocket_url);
        println!(
            "  Spawned: {}",
            if result.spawned {
                "yes"
            } else {
                "already running"
            }
        );

        let mut client = crate::protocol_client::ProtocolClient::connect(&result.websocket_url)?;

        let init_params = agent_protocol::methods::InitializeParams {
            client_type: agent_protocol::methods::ClientType::Cli,
            client_version: env!("CARGO_PKG_VERSION").to_string(),
            resume_snapshot_id: None,
            protocol_version: Some(agent_protocol::PROTOCOL_VERSION.to_string()),
        };

        let resp = client.request(
            "session.initialize",
            Some(serde_json::to_value(&init_params)?),
        )?;

        println!("Session initialized");
        if let Some(result) = resp.result {
            println!("  state: {}", serde_json::to_string_pretty(&result)?);
        }

        println!("Connected to daemon. Press Ctrl+C to exit.");

        loop {
            while let Some(ev) = client.try_recv_event() {
                println!("event: {:?}", ev);
            }
            std::thread::sleep(Duration::from_millis(100));
        }
    }
}
