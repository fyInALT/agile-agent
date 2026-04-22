//! DecisionCommand — pure data type representing actions recommended by the decision layer.
//!
//! This enum decouples decision *reasoning* from decision *execution*.
//! The decision layer produces `DecisionCommand` values; the runtime EventLoop
//! interprets and executes them via `RuntimeCommand` effects.

use serde::{Deserialize, Serialize};

/// A command produced by the decision layer describing what the runtime should do.
///
/// All variants are pure data — no `async`, no I/O, no thread handles.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum DecisionCommand {
    /// Escalate to human (pause agent, notify TUI)
    EscalateToHuman {
        reason: String,
        context: Option<String>,
    },

    /// Retry a failed tool call or operation
    RetryTool {
        tool_name: String,
        args: Option<String>,
        max_attempts: u32,
    },

    /// Send a custom instruction / prompt to a target agent
    SendCustomInstruction {
        prompt: String,
        target_agent: String,
    },

    /// Approve and continue (resume normal agent processing)
    ApproveAndContinue,

    /// Terminate the specified agent
    TerminateAgent {
        reason: String,
    },

    /// Switch the agent to a different provider type
    SwitchProvider {
        provider_type: String,
    },

    /// Select an option from a pending human decision
    SelectOption {
        option_id: String,
    },

    /// Skip the current pending human decision
    SkipDecision,

    /// Confirm task completion and clear assignment
    ConfirmCompletion,

    /// Request the agent to reflect on its work
    Reflect {
        prompt: String,
    },

    /// Stop the agent if all tasks are complete
    StopIfComplete {
        reason: String,
    },

    /// Prepare task start (git flow: branch, rebase, etc.)
    PrepareTaskStart {
        task_id: String,
        task_description: String,
    },

    /// Suggest committing changes
    SuggestCommit {
        message: String,
        mandatory: bool,
        reason: String,
    },

    /// Prepare a pull request
    PreparePr {
        title: String,
        description: String,
        base_branch: String,
        as_draft: bool,
    },

    /// Commit changes directly
    CommitChanges {
        message: String,
        is_wip: bool,
        worktree_path: Option<String>,
    },

    /// Stash uncommitted changes
    StashChanges {
        description: String,
        include_untracked: bool,
        worktree_path: Option<String>,
    },

    /// Discard uncommitted changes
    DiscardChanges {
        worktree_path: Option<String>,
    },

    /// Create a task-specific branch
    CreateTaskBranch {
        branch_name: String,
        base_branch: String,
        worktree_path: Option<String>,
    },

    /// Rebase current branch to main/master
    RebaseToMain {
        base_branch: String,
    },

    /// Wake up agent from resting state
    WakeUp,

    /// Unknown / unsupported command (carries raw info for diagnostics)
    Unknown {
        action_type: String,
        params: String,
    },
}

impl DecisionCommand {
    /// Human-readable description of what this command does.
    pub fn description(&self) -> String {
        match self {
            Self::EscalateToHuman { reason, .. } => {
                format!("Escalate to human: {reason}")
            }
            Self::RetryTool { tool_name, .. } => {
                format!("Retry tool: {tool_name}")
            }
            Self::SendCustomInstruction { prompt, target_agent } => {
                format!("Send instruction to {target_agent}: {prompt}")
            }
            Self::ApproveAndContinue => "Approve and continue".to_string(),
            Self::TerminateAgent { reason } => {
                format!("Terminate agent: {reason}")
            }
            Self::SwitchProvider { provider_type } => {
                format!("Switch provider to {provider_type}")
            }
            Self::SelectOption { option_id } => {
                format!("Select option: {option_id}")
            }
            Self::SkipDecision => "Skip decision".to_string(),
            Self::ConfirmCompletion => "Confirm completion".to_string(),
            Self::Reflect { prompt } => {
                format!("Reflect: {prompt}")
            }
            Self::StopIfComplete { reason } => {
                format!("Stop if complete: {reason}")
            }
            Self::PrepareTaskStart { task_id, .. } => {
                format!("Prepare task start: {task_id}")
            }
            Self::SuggestCommit { message, .. } => {
                format!("Suggest commit: {message}")
            }
            Self::PreparePr { title, .. } => {
                format!("Prepare PR: {title}")
            }
            Self::CommitChanges { message, .. } => {
                format!("Commit changes: {message}")
            }
            Self::StashChanges { description, .. } => {
                format!("Stash changes: {description}")
            }
            Self::DiscardChanges { .. } => "Discard changes".to_string(),
            Self::CreateTaskBranch { branch_name, .. } => {
                format!("Create branch: {branch_name}")
            }
            Self::RebaseToMain { base_branch } => {
                format!("Rebase to {base_branch}")
            }
            Self::WakeUp => "Wake up".to_string(),
            Self::Unknown { action_type, .. } => {
                format!("Unknown action: {action_type}")
            }
        }
    }

    /// Return the target agent ID(s) for routing.
    ///
    /// Returns an empty vec for commands that don't target a specific agent.
    pub fn target_agents(&self) -> Vec<String> {
        match self {
            Self::SendCustomInstruction { target_agent, .. } => {
                vec![target_agent.clone()]
            }
            _ => vec![],
        }
    }

    /// Check if this command requires human intervention.
    pub fn needs_human(&self) -> bool {
        matches!(self, Self::EscalateToHuman { .. })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn description_escalate() {
        let cmd = DecisionCommand::EscalateToHuman {
            reason: "stuck".to_string(),
            context: None,
        };
        assert_eq!(cmd.description(), "Escalate to human: stuck");
    }

    #[test]
    fn description_approve() {
        let cmd = DecisionCommand::ApproveAndContinue;
        assert_eq!(cmd.description(), "Approve and continue");
    }

    #[test]
    fn target_agents_custom_instruction() {
        let cmd = DecisionCommand::SendCustomInstruction {
            prompt: "do X".to_string(),
            target_agent: "ag-1".to_string(),
        };
        assert_eq!(cmd.target_agents(), vec!["ag-1"]);
    }

    #[test]
    fn target_agents_terminate_is_empty() {
        let cmd = DecisionCommand::TerminateAgent {
            reason: "done".to_string(),
        };
        assert!(cmd.target_agents().is_empty());
    }

    #[test]
    fn needs_human_true() {
        let cmd = DecisionCommand::EscalateToHuman {
            reason: "r".to_string(),
            context: None,
        };
        assert!(cmd.needs_human());
    }

    #[test]
    fn needs_human_false() {
        let cmd = DecisionCommand::ApproveAndContinue;
        assert!(!cmd.needs_human());
    }

    #[test]
    fn serde_roundtrip() {
        let cmd = DecisionCommand::SendCustomInstruction {
            prompt: "hello".to_string(),
            target_agent: "a1".to_string(),
        };
        let json = serde_json::to_string(&cmd).unwrap();
        let decoded: DecisionCommand = serde_json::from_str(&json).unwrap();
        assert_eq!(cmd, decoded);
    }
}
