//! RuntimeCommand — effect type representing side effects requested by Worker::apply().
//!
//! This type decouples state transitions (pure, in Worker) from I/O and
//! thread spawning (effectful, in the EventLoop). Each variant is a
//! command that the EventLoop interprets and executes via effect handlers.

use std::path::PathBuf;

use agent_types::AgentId;
use agent_events::DomainEvent;

/// A command produced by `Worker::apply()` describing a side effect to execute.
#[derive(Debug, Clone, PartialEq, Eq)]
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
    NotImplemented(RuntimeCommand),
    #[error("execution failed: {0}")]
    ExecutionFailed(String),
}

/// Trait for executing `RuntimeCommand` side effects.
///
/// Implementations bridge pure `Worker::apply()` output to actual I/O,
/// thread spawning, or channel sends. The trait is object-safe and
/// `Send + Sync` so it can live in `SessionManager` state.
pub trait EffectHandler: Send + Sync {
    /// Execute a single runtime command.
    ///
    /// # Errors
    /// Returns `EffectError::NotImplemented` if the handler does not support
    /// the given command variant. Returns `EffectError::ExecutionFailed` if
    /// the handler supports the command but execution failed.
    fn handle(&self, command: &RuntimeCommand) -> Result<(), EffectError>;
}

/// No-op effect handler.
///
/// Accepts all commands and does nothing. Useful as a default or in tests
/// where side effects are not desired.
#[derive(Debug, Clone, Default)]
pub struct NoopEffectHandler;

impl EffectHandler for NoopEffectHandler {
    fn handle(&self, _command: &RuntimeCommand) -> Result<(), EffectError> {
        Ok(())
    }
}

/// Recording effect handler for testing.
///
/// Records every command it receives so tests can assert on the
/// exact set of effects produced by `Worker::apply()`.
#[derive(Debug, Default)]
pub struct RecordingEffectHandler {
    recorded: std::sync::Mutex<Vec<RuntimeCommand>>,
}

impl RecordingEffectHandler {
    /// Create a new empty recording handler.
    pub fn new() -> Self {
        Self {
            recorded: std::sync::Mutex::new(Vec::new()),
        }
    }

    /// Return a cloned snapshot of all recorded commands.
    pub fn snapshot(&self) -> Vec<RuntimeCommand> {
        self.recorded.lock().unwrap().clone()
    }

    /// Clear the recording buffer.
    pub fn clear(&self) {
        self.recorded.lock().unwrap().clear();
    }

    /// Number of recorded commands.
    pub fn len(&self) -> usize {
        self.recorded.lock().unwrap().len()
    }

    /// Check if no commands have been recorded.
    pub fn is_empty(&self) -> bool {
        self.recorded.lock().unwrap().is_empty()
    }
}

impl EffectHandler for RecordingEffectHandler {
    fn handle(&self, command: &RuntimeCommand) -> Result<(), EffectError> {
        self.recorded.lock().unwrap().push(command.clone());
        Ok(())
    }
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

    // ── EffectHandler tests ─────────────────────────────────────

    #[test]
    fn noop_handler_accepts_all_commands() {
        let handler = NoopEffectHandler;
        let commands = vec![
            RuntimeCommand::SpawnProvider { agent_id: AgentId::new("a"), prompt: "p".to_string() },
            RuntimeCommand::NotifyUser { agent_id: AgentId::new("a"), message: "m".to_string() },
            RuntimeCommand::Terminate { agent_id: AgentId::new("a"), reason: "r".to_string() },
        ];
        for cmd in &commands {
            assert!(handler.handle(cmd).is_ok());
        }
    }

    #[test]
    fn recording_handler_captures_all_commands() {
        let handler = RecordingEffectHandler::new();
        let cmd = RuntimeCommand::NotifyUser {
            agent_id: AgentId::new("a"),
            message: "hello".to_string(),
        };
        handler.handle(&cmd).unwrap();
        assert_eq!(handler.len(), 1);
        assert_eq!(handler.snapshot(), vec![cmd]);
    }

    #[test]
    fn recording_handler_preserves_order() {
        let handler = RecordingEffectHandler::new();
        let c1 = RuntimeCommand::SpawnProvider { agent_id: AgentId::new("a"), prompt: "p1".to_string() };
        let c2 = RuntimeCommand::NotifyUser { agent_id: AgentId::new("b"), message: "m2".to_string() };
        let c3 = RuntimeCommand::Terminate { agent_id: AgentId::new("c"), reason: "r3".to_string() };
        handler.handle(&c1).unwrap();
        handler.handle(&c2).unwrap();
        handler.handle(&c3).unwrap();
        assert_eq!(handler.snapshot(), vec![c1, c2, c3]);
    }

    #[test]
    fn recording_handler_clear_works() {
        let handler = RecordingEffectHandler::new();
        handler.handle(&RuntimeCommand::Terminate { agent_id: AgentId::new("a"), reason: "r".to_string() }).unwrap();
        assert_eq!(handler.len(), 1);
        handler.clear();
        assert!(handler.is_empty());
    }
}
