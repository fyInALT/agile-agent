//! WebSocket server — binds to localhost ephemeral port

use std::net::SocketAddr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::net::{TcpListener, TcpStream};

/// WebSocket server that binds to an ephemeral port on localhost.
pub struct WebSocketServer {
    listener: TcpListener,
    local_addr: SocketAddr,
    /// Set to `true` to stop accepting new connections.
    shutdown: Arc<AtomicBool>,
}

impl WebSocketServer {
    /// Bind to `127.0.0.1:0` and let the OS assign an ephemeral port.
    pub async fn bind() -> anyhow::Result<Self> {
        Self::bind_to("127.0.0.1:0").await
    }

    /// Bind to a specific address.
    pub async fn bind_to(addr: &str) -> anyhow::Result<Self> {
        let listener = TcpListener::bind(addr).await?;
        let local_addr = listener.local_addr()?;
        Ok(Self {
            listener,
            local_addr,
            shutdown: Arc::new(AtomicBool::new(false)),
        })
    }

    /// Return the address this server is bound to.
    pub fn local_addr(&self) -> SocketAddr {
        self.local_addr
    }

    /// Return a handle that can be used to trigger shutdown.
    pub fn shutdown_handle(&self) -> ShutdownHandle {
        ShutdownHandle {
            flag: self.shutdown.clone(),
        }
    }

    /// Accept incoming TCP connections and hand them off to the provided callback.
    ///
    /// Runs until the shutdown flag is set or the listener is closed.
    pub async fn run<F, Fut>(self, mut on_connect: F) -> anyhow::Result<()>
    where
        F: FnMut(TcpStream, SocketAddr) -> Fut,
        Fut: std::future::Future<Output = ()>,
    {
        loop {
            if self.shutdown.load(Ordering::Relaxed) {
                tracing::info!("WebSocket server shutting down (stop accepting)");
                break;
            }

            // Use a timeout so we periodically check the shutdown flag.
            match tokio::time::timeout(
                tokio::time::Duration::from_millis(100),
                self.listener.accept(),
            )
            .await
            {
                Ok(Ok((stream, addr))) => {
                    on_connect(stream, addr).await;
                }
                Ok(Err(e)) => {
                    tracing::warn!("Accept error: {}", e);
                }
                Err(_) => {
                    // Timeout — loop around and check shutdown flag
                }
            }
        }

        Ok(())
    }
}

/// Handle to trigger graceful server shutdown.
#[derive(Clone)]
pub struct ShutdownHandle {
    flag: Arc<AtomicBool>,
}

impl ShutdownHandle {
    /// Request the server to stop accepting new connections.
    pub fn shutdown(&self) {
        self.flag.store(true, Ordering::Relaxed);
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn server_binds_and_returns_valid_local_addr() {
        let server = WebSocketServer::bind().await.unwrap();
        let addr = server.local_addr();
        assert!(addr.ip().is_loopback());
        assert!(addr.port() > 0);
    }

    #[tokio::test]
    async fn two_servers_get_different_ports() {
        let s1 = WebSocketServer::bind().await.unwrap();
        let s2 = WebSocketServer::bind().await.unwrap();
        assert_ne!(s1.local_addr().port(), s2.local_addr().port());
    }

    #[tokio::test]
    async fn shutdown_stops_accept_loop() {
        let server = WebSocketServer::bind().await.unwrap();
        let handle = server.shutdown_handle();

        let run_fut = server.run(|_, _| async {});

        // Trigger shutdown immediately.
        handle.shutdown();

        // The server should exit within a short timeout.
        tokio::time::timeout(tokio::time::Duration::from_secs(2), run_fut)
            .await
            .expect("server should stop")
            .expect("run ok");
    }
}
