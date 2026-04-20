//! WebSocket server — binds to localhost ephemeral port

use std::net::SocketAddr;
use tokio::net::{TcpListener, TcpStream};

/// WebSocket server that binds to an ephemeral port on localhost.
pub struct WebSocketServer {
    listener: TcpListener,
    local_addr: SocketAddr,
}

impl WebSocketServer {
    /// Bind to `127.0.0.1:0` and let the OS assign an ephemeral port.
    pub async fn bind() -> anyhow::Result<Self> {
        let listener = TcpListener::bind("127.0.0.1:0").await?;
        let local_addr = listener.local_addr()?;
        Ok(Self {
            listener,
            local_addr,
        })
    }

    /// Return the address this server is bound to.
    pub fn local_addr(&self) -> SocketAddr {
        self.local_addr
    }

    /// Accept incoming TCP connections and hand them off to the provided callback.
    ///
    /// Runs until the callback returns an error or the listener is closed.
    pub async fn run<F, Fut>(self, mut on_connect: F) -> anyhow::Result<()>
    where
        F: FnMut(TcpStream, SocketAddr) -> Fut,
        Fut: std::future::Future<Output = ()>,
    {
        loop {
            let (stream, addr) = self.listener.accept().await?;
            on_connect(stream, addr).await;
        }
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
}
