//! DecisionCommandInterpreter — maps `DecisionCommand` to `RuntimeCommand` effects.
//!
//! This is the bridge between the read-only decision layer and the runtime
//! effect system. It translates pure decision commands into concrete
//! `RuntimeCommand` values that the EventLoop can execute.

use agent_core::agent_runtime::AgentId;
use agent_core::runtime_command::RuntimeCommand;
use agent_decision::DecisionCommand;

/// Interpreter for decision commands.
///
/// Stateless and pure — given a `DecisionCommand`, produces the corresponding
/// `RuntimeCommand` sequence (if any).
#[derive(Debug, Clone, Default)]
pub struct DecisionCommandInterpreter;

impl DecisionCommandInterpreter {
    /// Create a new interpreter.
    pub fn new() -> Self {
        Self
    }

    /// Interpret a `DecisionCommand` into zero or more `RuntimeCommand` effects.
    ///
    /// Returns `None` for commands that cannot yet be expressed as `RuntimeCommand`
    /// (e.g. `PrepareTaskStart` which requires `GitFlowExecutor` access).
    pub fn interpret(
        &self,
        agent_id: &AgentId,
        command: &DecisionCommand,
    ) -> Option<Vec<RuntimeCommand>> {
        match command {
            // Commands that map cleanly to RuntimeCommand effects
            DecisionCommand::EscalateToHuman { reason, .. } => Some(vec![
                RuntimeCommand::NotifyUser {
                    agent_id: agent_id.clone(),
                    message: reason.clone(),
                },
            ]),

            DecisionCommand::TerminateAgent { reason } => Some(vec![
                RuntimeCommand::Terminate {
                    agent_id: agent_id.clone(),
                    reason: reason.clone(),
                },
            ]),

            DecisionCommand::StopIfComplete { reason } => Some(vec![
                RuntimeCommand::Terminate {
                    agent_id: agent_id.clone(),
                    reason: reason.clone(),
                },
            ]),

            DecisionCommand::ApproveAndContinue => {
                // No-op: the agent resumes on next tick cycle
                Some(vec![])
            }

            DecisionCommand::WakeUp => {
                // No-op: wake-up is handled by state transition on next event
                Some(vec![])
            }

            // Commands that require transcript manipulation or provider thread
            // management — not yet expressible as RuntimeCommand. Fall back to
            // legacy DecisionExecutor::execute().
            DecisionCommand::SendCustomInstruction { .. }
            | DecisionCommand::Reflect { .. }
            | DecisionCommand::RetryTool { .. }
            | DecisionCommand::SwitchProvider { .. }
            | DecisionCommand::SelectOption { .. }
            | DecisionCommand::SkipDecision
            | DecisionCommand::ConfirmCompletion
            | DecisionCommand::PrepareTaskStart { .. }
            | DecisionCommand::SuggestCommit { .. }
            | DecisionCommand::PreparePr { .. }
            | DecisionCommand::CommitChanges { .. }
            | DecisionCommand::StashChanges { .. }
            | DecisionCommand::DiscardChanges { .. }
            | DecisionCommand::CreateTaskBranch { .. }
            | DecisionCommand::RebaseToMain { .. }
            | DecisionCommand::Unknown { .. } => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use agent_core::agent_runtime::AgentId;
    use agent_decision::DecisionCommand;

    #[test]
    fn interpret_escalate_to_human() {
        let interp = DecisionCommandInterpreter::new();
        let cmd = DecisionCommand::EscalateToHuman {
            reason: "stuck".to_string(),
            context: None,
        };
        let cmds = interp.interpret(&AgentId::new("ag-1"), &cmd).unwrap();
        assert_eq!(cmds.len(), 1);
        assert!(matches!(
            &cmds[0],
            RuntimeCommand::NotifyUser { agent_id, message }
            if agent_id.as_str() == "ag-1" && message == "stuck"
        ));
    }

    #[test]
    fn interpret_custom_instruction_returns_none() {
        let interp = DecisionCommandInterpreter::new();
        let cmd = DecisionCommand::SendCustomInstruction {
            prompt: "do X".to_string(),
            target_agent: "ag-1".to_string(),
        };
        assert!(interp.interpret(&AgentId::new("ag-1"), &cmd).is_none());
    }

    #[test]
    fn interpret_reflect_returns_none() {
        let interp = DecisionCommandInterpreter::new();
        let cmd = DecisionCommand::Reflect {
            prompt: "verify".to_string(),
        };
        assert!(interp.interpret(&AgentId::new("ag-1"), &cmd).is_none());
    }

    #[test]
    fn interpret_terminate_agent() {
        let interp = DecisionCommandInterpreter::new();
        let cmd = DecisionCommand::TerminateAgent {
            reason: "done".to_string(),
        };
        let cmds = interp.interpret(&AgentId::new("ag-1"), &cmd).unwrap();
        assert_eq!(cmds.len(), 1);
        assert!(matches!(
            &cmds[0],
            RuntimeCommand::Terminate { agent_id, reason }
            if agent_id.as_str() == "ag-1" && reason == "done"
        ));
    }

    #[test]
    fn interpret_approve_and_continue_is_noop() {
        let interp = DecisionCommandInterpreter::new();
        let cmd = DecisionCommand::ApproveAndContinue;
        let cmds = interp.interpret(&AgentId::new("ag-1"), &cmd).unwrap();
        assert!(cmds.is_empty());
    }

    #[test]
    fn interpret_select_option_returns_none() {
        let interp = DecisionCommandInterpreter::new();
        let cmd = DecisionCommand::SelectOption {
            option_id: "A".to_string(),
        };
        assert!(interp.interpret(&AgentId::new("ag-1"), &cmd).is_none());
    }

    #[test]
    fn interpret_prepare_task_start_returns_none() {
        let interp = DecisionCommandInterpreter::new();
        let cmd = DecisionCommand::PrepareTaskStart {
            task_id: "t1".to_string(),
            task_description: "desc".to_string(),
        };
        assert!(interp.interpret(&AgentId::new("ag-1"), &cmd).is_none());
    }

    #[test]
    fn interpret_stop_if_complete() {
        let interp = DecisionCommandInterpreter::new();
        let cmd = DecisionCommand::StopIfComplete {
            reason: "all done".to_string(),
        };
        let cmds = interp.interpret(&AgentId::new("ag-1"), &cmd).unwrap();
        assert_eq!(cmds.len(), 1);
        assert!(matches!(
            &cmds[0],
            RuntimeCommand::Terminate { reason, .. } if reason == "all done"
        ));
    }

    #[test]
    fn interpret_wakeup_is_noop() {
        let interp = DecisionCommandInterpreter::new();
        let cmd = DecisionCommand::WakeUp;
        let cmds = interp.interpret(&AgentId::new("ag-1"), &cmd).unwrap();
        assert!(cmds.is_empty());
    }
}
