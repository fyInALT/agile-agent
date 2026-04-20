//! agent-daemon entry point

use agent_daemon::handler::SessionHandler;
use agent_daemon::router::Router;
use agent_daemon::server::WebSocketServer;
use std::sync::Arc;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    let server = WebSocketServer::bind().await?;
    let addr = server.local_addr();
    tracing::info!("agent-daemon listening on ws://{}/v1/session", addr);

    let mut router = Router::new();
    router.register("session.initialize", Arc::new(SessionHandler));
    let router_handle = router.handle();

    server
        .run(|stream, peer_addr| {
            let router = router_handle.clone();
            async move {
                agent_daemon::connection::Connection::spawn(stream, peer_addr, router);
            }
        })
        .await?;

    Ok(())
}
