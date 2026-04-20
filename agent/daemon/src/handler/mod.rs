//! JSON-RPC method handlers

use agent_protocol::jsonrpc::*;

mod session;

pub use session::SessionHandler;

/// Trait for handling JSON-RPC requests.
#[async_trait::async_trait]
pub trait Handler: Send + Sync {
    async fn handle(&self, req: JsonRpcRequest) -> anyhow::Result<JsonRpcResponse>;
}
