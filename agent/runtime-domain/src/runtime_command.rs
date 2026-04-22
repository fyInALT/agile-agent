//! RuntimeCommand — effect type representing side effects requested by Worker::apply().
//!
//! This type decouples state transitions (pure, in Worker) from I/O and
//! thread spawning (effectful, in the EventLoop). Each variant is a
//! command that the EventLoop interprets and executes via effect handlers.

use std::path::PathBuf;

use agent_events::DomainEvent;
use agent_types::AgentId;
use serde::{Deserialize, Serialize};

/// A command produced by `Worker::apply()` describing a side effect to execute.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum RuntimeCommand {
    /// Spawn a provider thread for the given agent
    SpawnProvider {
        agent_id: AgentId,
        prompt: String,
    },

    /// Send an event to the provider thread's input channel
    SendToProvider {
        agent_id: AgentId,
        event: DomainEvent,
    },

    /// Request decision layer intervention for a situation
    RequestDecision {
        agent_id: AgentId,
        situation_type: String,
    },

    /// Notify the user (via TUI event bus)
    NotifyUser {
        agent_id: AgentId,
        message: String,
    },

    /// Update worktree path/branch for the agent
    UpdateWorktree {
        agent_id: AgentId,
        path: PathBuf,
        branch: String,
    },

    /// Gracefully terminate the agent
    Terminate {
        agent_id: AgentId,
        reason: String,
    },

    /// Transition agent to a new operational status.
    /// Used by ApproveAndContinue, WakeUp, and other state-transition decisions.
    TransitionState {
        agent_id: AgentId,
        new_status: String,
    },
}

/// Ordered queue of runtime commands.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RuntimeCommandQueue {
    commands: Vec<RuntimeCommand>,
}

impl RuntimeCommandQueue {
    /// Create an empty queue.
    pub fn new() -> Self {
        Self {
            commands: Vec::new(),
        }
    }

    /// Push a command onto the queue.
    pub fn push(&mut self, cmd: RuntimeCommand) {
        self.commands.push(cmd);
    }

    /// Extend with multiple commands.
    pub fn extend(&mut self, cmds: Vec<RuntimeCommand>) {
        self.commands.extend(cmds);
    }

    /// Drain all commands (returns iterator and clears queue).
    pub fn drain(&mut self) -> impl Iterator<Item = RuntimeCommand> + '_ {
        self.commands.drain(..)
    }

    /// Check if queue is empty.
    pub fn is_empty(&self) -> bool {
        self.commands.is_empty()
    }

    /// Get the number of pending commands.
    pub fn len(&self) -> usize {
        self.commands.len()
    }

    /// Peek at the next command without removing it.
    pub fn peek(&self) -> Option<&RuntimeCommand> {
        self.commands.first()
    }
}

/// Error type for effect handler execution failures.
#[derive(Debug, Clone, thiserror::Error, PartialEq, Eq)]
pub enum EffectError {
    #[error("handler not implemented for command: {0:?}")]
    NotImplemented(Box<RuntimeCommand>),
    #[error("execution failed: {0}")]
    ExecutionFailed(String),
}

#[cfg(test)]
mod tests {
    use super::*;
    use agent_types::AgentId;

    #[test]
    fn queue_push_and_drain() {
        let mut q = RuntimeCommandQueue::new();
        q.push(RuntimeCommand::NotifyUser {
            agent_id: AgentId::new("a"),
            message: "hello".to_string(),
        });
        assert_eq!(q.len(), 1);

        let drained: Vec<_> = q.drain().collect();
        assert_eq!(drained.len(), 1);
        assert!(q.is_empty());
    }

    #[test]
    fn queue_extend() {
        let mut q = RuntimeCommandQueue::new();
        q.extend(vec![
            RuntimeCommand::Terminate {
                agent_id: AgentId::new("a"),
                reason: "done".to_string(),
            },
            RuntimeCommand::Terminate {
                agent_id: AgentId::new("b"),
                reason: "done".to_string(),
            },
        ]);
        assert_eq!(q.len(), 2);
    }

    #[test]
    fn queue_peek() {
        let mut q = RuntimeCommandQueue::new();
        assert!(q.peek().is_none());
        q.push(RuntimeCommand::SpawnProvider {
            agent_id: AgentId::new("a"),
            prompt: "hi".to_string(),
        });
        assert!(matches!(q.peek(), Some(RuntimeCommand::SpawnProvider { .. })));
        assert_eq!(q.len(), 1); // peek does not remove
    }

    #[test]
    fn transition_state_command_construction() {
        let cmd = RuntimeCommand::TransitionState {
            agent_id: AgentId::new("ag-1"),
            new_status: "idle".to_string(),
        };
        assert!(matches!(cmd, RuntimeCommand::TransitionState { .. }));
    }

    #[test]
    fn transition_state_serde_roundtrip() {
        let cmd = RuntimeCommand::TransitionState {
            agent_id: AgentId::new("ag-1"),
            new_status: "idle".to_string(),
        };
        let json = serde_json::to_string(&cmd).unwrap();
        let decoded: RuntimeCommand = serde_json::from_str(&json).unwrap();
        assert_eq!(cmd, decoded);
    }
}
