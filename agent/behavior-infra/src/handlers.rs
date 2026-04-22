//! Per-variant effect handler traits.
//!
//! Each trait corresponds to a single `RuntimeCommand` variant.
//! Implementations are provided in `agent-daemon` (where `SessionInner` lives).
//!
//! The `CompositeEffectHandler` in this crate combines all trait implementations
//! into a single `EffectHandler` for use by the `EventLoop`.

use agent_events::DomainEvent;
use agent_types::AgentId;

use crate::EffectError;

/// Handler for `RuntimeCommand::SpawnProvider`.
pub trait SpawnProviderHandler: Send + Sync {
    /// Spawn a provider thread for the given agent.
    fn execute(&self, agent_id: &AgentId, prompt: &str) -> Result<(), EffectError>;
}

/// Handler for `RuntimeCommand::SendToProvider`.
pub trait SendToProviderHandler: Send + Sync {
    /// Send an event to the provider thread's input channel.
    fn execute(&self, agent_id: &AgentId, event: &DomainEvent) -> Result<(), EffectError>;
}

/// Handler for `RuntimeCommand::RequestDecision`.
pub trait RequestDecisionHandler: Send + Sync {
    /// Create a decision request and route it to the decision agent.
    fn execute(&self, agent_id: &AgentId, situation_type: &str) -> Result<(), EffectError>;
}

/// Handler for `RuntimeCommand::NotifyUser`.
pub trait NotifyUserHandler: Send + Sync {
    /// Emit a user-facing notification via the TUI event bus.
    fn execute(&self, agent_id: &AgentId, message: &str) -> Result<(), EffectError>;
}

/// Handler for `RuntimeCommand::UpdateWorktree`.
pub trait UpdateWorktreeHandler: Send + Sync {
    /// Update worktree path/branch for the agent.
    fn execute(&self, agent_id: &AgentId, path: &std::path::Path, branch: &str) -> Result<(), EffectError>;
}

/// Handler for `RuntimeCommand::Terminate`.
pub trait TerminateHandler: Send + Sync {
    /// Gracefully terminate the agent.
    fn execute(&self, agent_id: &AgentId, reason: &str) -> Result<(), EffectError>;
}

/// Handler for `RuntimeCommand::TransitionState`.
pub trait TransitionStateHandler: Send + Sync {
    /// Transition agent to a new operational status.
    fn execute(&self, agent_id: &AgentId, new_status: &str) -> Result<(), EffectError>;
}

/// Composite effect handler that delegates each `RuntimeCommand` variant
/// to its corresponding per-variant handler.
///
/// This struct is intentionally generic so the daemon can inject its own
/// handler implementations that have access to `SessionInner`.
pub struct CompositeEffectHandler {
    pub spawn_provider: Box<dyn SpawnProviderHandler>,
    pub send_to_provider: Box<dyn SendToProviderHandler>,
    pub request_decision: Box<dyn RequestDecisionHandler>,
    pub notify_user: Box<dyn NotifyUserHandler>,
    pub update_worktree: Box<dyn UpdateWorktreeHandler>,
    pub terminate: Box<dyn TerminateHandler>,
    pub transition_state: Box<dyn TransitionStateHandler>,
}

impl CompositeEffectHandler {
    /// Create a composite handler from individual handlers.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        spawn_provider: Box<dyn SpawnProviderHandler>,
        send_to_provider: Box<dyn SendToProviderHandler>,
        request_decision: Box<dyn RequestDecisionHandler>,
        notify_user: Box<dyn NotifyUserHandler>,
        update_worktree: Box<dyn UpdateWorktreeHandler>,
        terminate: Box<dyn TerminateHandler>,
        transition_state: Box<dyn TransitionStateHandler>,
    ) -> Self {
        Self {
            spawn_provider,
            send_to_provider,
            request_decision,
            notify_user,
            update_worktree,
            terminate,
            transition_state,
        }
    }
}

impl crate::EffectHandler for CompositeEffectHandler {
    fn handle(&self, command: &crate::RuntimeCommand) -> Result<(), EffectError> {
        match command {
            crate::RuntimeCommand::SpawnProvider { agent_id, prompt } => {
                self.spawn_provider.execute(agent_id, prompt)
            }
            crate::RuntimeCommand::SendToProvider { agent_id, event } => {
                self.send_to_provider.execute(agent_id, event)
            }
            crate::RuntimeCommand::RequestDecision { agent_id, situation_type } => {
                self.request_decision.execute(agent_id, situation_type)
            }
            crate::RuntimeCommand::NotifyUser { agent_id, message } => {
                self.notify_user.execute(agent_id, message)
            }
            crate::RuntimeCommand::UpdateWorktree { agent_id, path, branch } => {
                self.update_worktree.execute(agent_id, path, branch)
            }
            crate::RuntimeCommand::Terminate { agent_id, reason } => {
                self.terminate.execute(agent_id, reason)
            }
            crate::RuntimeCommand::TransitionState { agent_id, new_status } => {
                self.transition_state.execute(agent_id, new_status)
            }
        }
    }
}
