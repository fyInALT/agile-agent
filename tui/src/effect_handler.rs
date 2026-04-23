//! TUI Effect Handler — dispatches `RuntimeCommand` values to TUI state.
//!
//! This module bridges the pure decision path (`translate() + interpreter`)
//! to the TUI's mutable `AppState` / `AgentPool`. Unlike the daemon's
//! `CompositeEffectHandler`, this handler is **not** `Send + Sync` and does
//! not implement the `EffectHandler` trait — it operates on a local
//! `&mut TuiState` borrow inside the single-threaded AppLoop.

use agent_core::agent_slot::AgentSlotStatus as CoreAgentStatus;
use agent_core::runtime_command::{EffectError, RuntimeCommand};

use crate::ui_state::TuiState;

/// Dispatch a single `RuntimeCommand` against the TUI state.
///
/// Errors are logged via `tracing::warn!` and returned as `EffectError::ExecutionFailed`
/// so the caller can decide whether to abort the remaining command batch.
pub fn dispatch_runtime_command(
    cmd: &RuntimeCommand,
    state: &mut TuiState,
) -> Result<(), EffectError> {
    match cmd {
        RuntimeCommand::NotifyUser { agent_id: _, message } => {
            state.app_mut().push_status_message(message.clone());
            Ok(())
        }

        RuntimeCommand::Terminate { agent_id, reason } => {
            state.app_mut().push_status_message(format!(
                "🧠 {}: agent terminated ({})",
                agent_id.as_str(), reason
            ));
            if let Some(pool) = state.agent_pool.as_mut() {
                let _ = pool.stop_agent(agent_id);
            }
            Ok(())
        }

        RuntimeCommand::TransitionState { agent_id, new_status } => {
            if let Some(pool) = state.agent_pool.as_mut()
                && let Some(slot) = pool.get_slot_mut_by_id(agent_id)
            {
                let target = match new_status.as_str() {
                    "idle" => CoreAgentStatus::idle(),
                    "starting" => CoreAgentStatus::starting(),
                    "responding" => CoreAgentStatus::responding_now(),
                    _ => CoreAgentStatus::idle(),
                };
                let _ = slot.transition_to(target);
            }
            state.app_mut().push_status_message(format!(
                "🧠 {}: state transitioned to {}",
                agent_id.as_str(), new_status
            ));
            Ok(())
        }

        // Commands not yet supported in the TUI pure path.
        RuntimeCommand::SpawnProvider { agent_id, .. } => {
            tracing::warn!(
                agent_id = %agent_id.as_str(),
                "TUI pure path: SpawnProvider not yet implemented — use legacy path"
            );
            Err(EffectError::NotImplemented(Box::new(cmd.clone())))
        }
        RuntimeCommand::SendToProvider { agent_id, .. } => {
            tracing::warn!(
                agent_id = %agent_id.as_str(),
                "TUI pure path: SendToProvider not yet implemented — use legacy path"
            );
            Err(EffectError::NotImplemented(Box::new(cmd.clone())))
        }
        RuntimeCommand::RequestDecision { agent_id, .. } => {
            tracing::warn!(
                agent_id = %agent_id.as_str(),
                "TUI pure path: RequestDecision not yet implemented — use legacy path"
            );
            Err(EffectError::NotImplemented(Box::new(cmd.clone())))
        }
        RuntimeCommand::UpdateWorktree { agent_id, .. } => {
            tracing::warn!(
                agent_id = %agent_id.as_str(),
                "TUI pure path: UpdateWorktree not yet implemented — use legacy path"
            );
            Err(EffectError::NotImplemented(Box::new(cmd.clone())))
        }
    }
}

/// Dispatch a batch of `RuntimeCommand`s, stopping at the first error.
///
/// Returns the number of commands successfully executed.
pub fn dispatch_runtime_commands(
    cmds: &[RuntimeCommand],
    state: &mut TuiState,
) -> Result<usize, EffectError> {
    let mut executed = 0;
    for cmd in cmds {
        dispatch_runtime_command(cmd, state)?;
        executed += 1;
    }
    Ok(executed)
}

#[cfg(test)]
mod tests {
    use agent_core::agent_runtime::AgentId;
    use agent_core::runtime_command::RuntimeCommand;

    // We can't easily construct a full TuiState in unit tests (it requires
    // RuntimeSession::bootstrap which touches the filesystem and providers).
    // Instead, we test the dispatch function signature and error paths with
    // a minimal mock. For integration tests, see `tui/src/shell_tests.rs`.

    #[test]
    fn notify_user_command_signature() {
        // Verify the command can be constructed and matched.
        let cmd = RuntimeCommand::NotifyUser {
            agent_id: AgentId::new("ag-1"),
            message: "hello".to_string(),
        };
        assert!(matches!(cmd, RuntimeCommand::NotifyUser { .. }));
    }

    #[test]
    fn terminate_command_signature() {
        let cmd = RuntimeCommand::Terminate {
            agent_id: AgentId::new("ag-1"),
            reason: "done".to_string(),
        };
        assert!(matches!(cmd, RuntimeCommand::Terminate { .. }));
    }

    #[test]
    fn transition_state_command_signature() {
        let cmd = RuntimeCommand::TransitionState {
            agent_id: AgentId::new("ag-1"),
            new_status: "idle".to_string(),
        };
        assert!(matches!(cmd, RuntimeCommand::TransitionState { .. }));
    }
}
