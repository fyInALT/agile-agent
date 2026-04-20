//! JSON-RPC method handlers

use agent_protocol::jsonrpc::*;

mod agent;
mod heartbeat;
mod session;

pub use agent::AgentHandler;
pub use heartbeat::HeartbeatHandler;
pub use session::SessionHandler;

/// Trait for handling JSON-RPC requests.
#[async_trait::async_trait]
pub trait Handler: Send + Sync {
    /// Handle a request and return either a [`JsonRpcMessage::Response`] or
    /// [`JsonRpcMessage::Error`].
    async fn handle(&self, req: JsonRpcRequest) -> anyhow::Result<JsonRpcMessage>;
}
