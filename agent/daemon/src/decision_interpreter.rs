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

    #[test]
    fn interpret_retry_tool_returns_none() {
        let interp = DecisionCommandInterpreter::new();
        let cmd = DecisionCommand::RetryTool {
            tool_name: "test_tool".to_string(),
            args: None,
            max_attempts: 3,
        };
        assert!(interp.interpret(&AgentId::new("ag-1"), &cmd).is_none());
    }

    #[test]
    fn interpret_switch_provider_returns_none() {
        let interp = DecisionCommandInterpreter::new();
        let cmd = DecisionCommand::SwitchProvider {
            provider_type: "openai".to_string(),
        };
        assert!(interp.interpret(&AgentId::new("ag-1"), &cmd).is_none());
    }

    #[test]
    fn interpret_skip_decision_returns_none() {
        let interp = DecisionCommandInterpreter::new();
        let cmd = DecisionCommand::SkipDecision;
        assert!(interp.interpret(&AgentId::new("ag-1"), &cmd).is_none());
    }

    #[test]
    fn interpret_confirm_completion_returns_none() {
        let interp = DecisionCommandInterpreter::new();
        let cmd = DecisionCommand::ConfirmCompletion;
        assert!(interp.interpret(&AgentId::new("ag-1"), &cmd).is_none());
    }

    #[test]
    fn interpret_suggest_commit_returns_none() {
        let interp = DecisionCommandInterpreter::new();
        let cmd = DecisionCommand::SuggestCommit {
            message: "feat: add X".to_string(),
            mandatory: false,
            reason: "feature complete".to_string(),
        };
        assert!(interp.interpret(&AgentId::new("ag-1"), &cmd).is_none());
    }

    #[test]
    fn interpret_prepare_pr_returns_none() {
        let interp = DecisionCommandInterpreter::new();
        let cmd = DecisionCommand::PreparePr {
            title: "Add X".to_string(),
            description: "Adds X".to_string(),
            base_branch: "main".to_string(),
            as_draft: false,
        };
        assert!(interp.interpret(&AgentId::new("ag-1"), &cmd).is_none());
    }

    #[test]
    fn interpret_commit_changes_returns_none() {
        let interp = DecisionCommandInterpreter::new();
        let cmd = DecisionCommand::CommitChanges {
            message: "wip".to_string(),
            is_wip: true,
            worktree_path: None,
        };
        assert!(interp.interpret(&AgentId::new("ag-1"), &cmd).is_none());
    }

    #[test]
    fn interpret_stash_changes_returns_none() {
        let interp = DecisionCommandInterpreter::new();
        let cmd = DecisionCommand::StashChanges {
            description: "stash".to_string(),
            include_untracked: false,
            worktree_path: None,
        };
        assert!(interp.interpret(&AgentId::new("ag-1"), &cmd).is_none());
    }

    #[test]
    fn interpret_discard_changes_returns_none() {
        let interp = DecisionCommandInterpreter::new();
        let cmd = DecisionCommand::DiscardChanges {
            worktree_path: None,
        };
        assert!(interp.interpret(&AgentId::new("ag-1"), &cmd).is_none());
    }

    #[test]
    fn interpret_create_task_branch_returns_none() {
        let interp = DecisionCommandInterpreter::new();
        let cmd = DecisionCommand::CreateTaskBranch {
            branch_name: "feature/x".to_string(),
            base_branch: "main".to_string(),
            worktree_path: None,
        };
        assert!(interp.interpret(&AgentId::new("ag-1"), &cmd).is_none());
    }

    #[test]
    fn interpret_rebase_to_main_returns_none() {
        let interp = DecisionCommandInterpreter::new();
        let cmd = DecisionCommand::RebaseToMain {
            base_branch: "main".to_string(),
        };
        assert!(interp.interpret(&AgentId::new("ag-1"), &cmd).is_none());
    }

    #[test]
    fn interpret_unknown_returns_none() {
        let interp = DecisionCommandInterpreter::new();
        let cmd = DecisionCommand::Unknown {
            action_type: "foo".to_string(),
            params: "{}".to_string(),
        };
        assert!(interp.interpret(&AgentId::new("ag-1"), &cmd).is_none());
    }

    #[test]
    fn mixed_interpretable_and_non_interpretable_commands() {
        let interp = DecisionCommandInterpreter::new();
        let agent_id = AgentId::new("ag-1");

        let commands = vec![
            DecisionCommand::EscalateToHuman {
                reason: "stuck".to_string(),
                context: None,
            },
            DecisionCommand::SkipDecision,
            DecisionCommand::TerminateAgent {
                reason: "done".to_string(),
            },
        ];

        let mut all_interpreted = true;
        let mut runtime_cmds = Vec::new();

        for cmd in &commands {
            match interp.interpret(&agent_id, cmd) {
                Some(mut cmds) => runtime_cmds.append(&mut cmds),
                None => {
                    all_interpreted = false;
                    break;
                }
            }
        }

        assert!(!all_interpreted, "SkipDecision returns None, so all_interpreted should be false");
        // Only EscalateToHuman should have been collected before hitting SkipDecision
        assert_eq!(runtime_cmds.len(), 1);
        assert!(matches!(&runtime_cmds[0], RuntimeCommand::NotifyUser { .. }));
    }
}
