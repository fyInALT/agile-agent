//! EffectHandler trait and implementations for executing RuntimeCommand side effects.
//!
//! The command types (`RuntimeCommand`, `RuntimeCommandQueue`, `EffectError`) live in
//! `agent-runtime-domain`. This crate provides the trait and concrete handlers that
//! bridge pure domain logic to actual I/O and thread spawning.

pub use agent_runtime_domain::{EffectError, RuntimeCommand, RuntimeCommandQueue};

/// Trait for executing `RuntimeCommand` side effects.
///
/// Implementations bridge pure `Worker::apply()` output to actual I/O,
/// thread spawning, or channel sends. The trait is object-safe and
/// `Send + Sync` so it can live in `EventLoop` state.
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
    use agent_runtime_domain::RuntimeCommand;
    use agent_types::AgentId;

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
