//! Task Preparation Pipeline
//!
//! Orchestrates the complete task preparation flow:
//! 1. Extract task metadata (branch name, type, summary)
//! 2. Analyze current git state
//! 3. Handle uncommitted changes if present
//! 4. Sync/create branch for task

use crate::git_state::{GitState, GitStateAnalyzer};
use crate::task_metadata::TaskMetadata;
use crate::uncommitted_handler::{
    UncommittedAction, UncommittedAnalysis, UncommittedAnalyzer,
};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Request for task preparation pipeline
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskPreparationRequest {
    /// Task description from backlog
    pub task_description: String,

    /// Task ID from backlog (if available)
    pub task_id: Option<String>,

    /// Worktree path for the agent
    pub worktree_path: PathBuf,

    /// Agent ID
    pub agent_id: String,
}

/// Result of task preparation pipeline
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TaskPreparationResult {
    /// All preparation steps succeeded
    Ready {
        /// Extracted task metadata
        task_meta: TaskMetadata,
        /// Branch is ready for work
        branch_ready: bool,
        /// Working tree is clean
        clean_state: bool,
    },

    /// Needs uncommitted changes handling first
    NeedsUncommittedHandling {
        /// Analysis of uncommitted changes
        analysis: UncommittedAnalysis,
        /// Task metadata for the branch name
        task_meta: TaskMetadata,
    },

    /// Needs rebase/conflict resolution
    NeedsSync {
        /// Current git state
        git_state: GitState,
        /// Task metadata
        task_meta: TaskMetadata,
    },

    /// Needs human intervention
    NeedsHuman {
        /// Reason for human intervention
        reason: String,
    },

    /// Preparation failed
    Failed {
        /// Error message
        error: String,
        /// Step that failed
        step: PreparationStep,
    },
}

/// Steps in the preparation pipeline
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PreparationStep {
    /// Task metadata extraction
    MetaExtraction,
    /// Git state analysis
    GitStateAnalysis,
    /// Uncommitted changes handling
    UncommittedHandling,
    /// Branch setup/sync
    BranchSetup,
    /// Final verification
    FinalVerification,
}

impl std::fmt::Display for PreparationStep {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PreparationStep::MetaExtraction => write!(f, "meta extraction"),
            PreparationStep::GitStateAnalysis => write!(f, "git state analysis"),
            PreparationStep::UncommittedHandling => write!(f, "uncommitted handling"),
            PreparationStep::BranchSetup => write!(f, "branch setup"),
            PreparationStep::FinalVerification => write!(f, "final verification"),
        }
    }
}

/// Pre-action to execute before starting task
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PreAction {
    /// Handle uncommitted changes
    HandleUncommitted {
        action: UncommittedAction,
        commit_message: Option<String>,
        stash_description: Option<String>,
    },
    /// Create a new branch
    CreateBranch {
        branch_name: String,
        base_branch: String,
    },
    /// Rebase to main/master
    RebaseToMain {
        base_branch: String,
    },
}

impl PreAction {
    /// Get a summary of the action for display
    pub fn summary(&self) -> String {
        match self {
            PreAction::HandleUncommitted { action, .. } => {
                format!("Handle uncommitted: {}", action)
            }
            PreAction::CreateBranch {
                branch_name,
                base_branch,
            } => {
                format!("Create branch '{}' from '{}'", branch_name, base_branch)
            }
            PreAction::RebaseToMain { base_branch } => {
                format!("Rebase to {}", base_branch)
            }
        }
    }
}

/// Task preparation pipeline orchestrator
pub struct TaskPreparationPipeline {
    state_analyzer: GitStateAnalyzer,
}

impl TaskPreparationPipeline {
    /// Create a new task preparation pipeline
    pub fn new() -> Self {
        Self {
            state_analyzer: GitStateAnalyzer::default(),
        }
    }

    /// Prepare for task execution
    pub fn prepare(&self, request: &TaskPreparationRequest) -> TaskPreparationResult {
        // Step 1: Extract task metadata
        let task_meta = self.extract_task_meta(&request.task_description, request.task_id.as_deref());

        // Step 2: Analyze current git state
        let git_state = match self.state_analyzer.analyze(&request.worktree_path) {
            Ok(state) => state,
            Err(e) => {
                return TaskPreparationResult::Failed {
                    error: format!("Git state analysis failed: {}", e),
                    step: PreparationStep::GitStateAnalysis,
                };
            }
        };

        // Step 3: Check for uncommitted changes
        if git_state.has_uncommitted {
            let analysis = UncommittedAnalyzer::analyze(&git_state, request.task_id.as_deref());

            // If needs human decision, return early
            if analysis.suggested_action == UncommittedAction::RequestHuman {
                return TaskPreparationResult::NeedsHuman {
                    reason: format!(
                        "Uncommitted changes require human decision: {}",
                        analysis.reason
                    ),
                };
            }

            // If needs handling, return that result
            if matches!(
                analysis.suggested_action,
                UncommittedAction::Commit | UncommittedAction::Stash | UncommittedAction::Discard
            ) {
                return TaskPreparationResult::NeedsUncommittedHandling {
                    analysis,
                    task_meta,
                };
            }
        }

        // Step 4: Check if branch needs sync (behind base)
        let base_branch = self.determine_base_branch(&git_state);
        if self.needs_rebase(&git_state, &base_branch) {
            return TaskPreparationResult::NeedsSync { git_state, task_meta };
        }

        // Step 5: Determine if branch exists or needs creation
        let branch_name = &task_meta.branch_name;
        match self.state_analyzer.branch_exists(&request.worktree_path, branch_name) {
            Ok(true) => {
                // Branch exists, check if it's checked out
                // For now, assume it's ready
                TaskPreparationResult::Ready {
                    task_meta,
                    branch_ready: true,
                    clean_state: !git_state.has_uncommitted,
                }
            }
            Ok(false) => {
                // Branch doesn't exist - needs creation
                // Return NeedsSync to trigger branch creation
                TaskPreparationResult::NeedsSync { git_state, task_meta }
            }
            Err(e) => TaskPreparationResult::Failed {
                error: format!("Failed to check branch existence: {}", e),
                step: PreparationStep::BranchSetup,
            },
        }
    }

    /// Extract task metadata from description
    fn extract_task_meta(&self, description: &str, task_id: Option<&str>) -> TaskMetadata {
        let id = task_id.unwrap_or("task-001");
        TaskMetadata::new(id, description)
    }

    /// Determine base branch (main or master)
    fn determine_base_branch(&self, git_state: &GitState) -> String {
        // If current branch is main or master, use it
        if git_state.current_branch == "main" || git_state.current_branch == "master" {
            return git_state.current_branch.clone();
        }
        // Default to main
        "main".to_string()
    }

    /// Check if branch needs rebase
    fn needs_rebase(&self, git_state: &GitState, base_branch: &str) -> bool {
        // If behind base branch, needs rebase
        git_state.commits_behind > 0
        // Or if not on main/master and has diverged
        || (git_state.current_branch != base_branch
            && git_state.commits_ahead > 0
            && git_state.commits_behind > 0)
    }

    /// Generate pre-actions needed for the preparation
    pub fn generate_pre_actions(&self, result: &TaskPreparationResult) -> Vec<PreAction> {
        let mut actions = Vec::new();

        match result {
            TaskPreparationResult::NeedsUncommittedHandling { analysis, .. } => {
                actions.push(PreAction::HandleUncommitted {
                    action: analysis.suggested_action,
                    commit_message: Some(self.generate_commit_message(analysis)),
                    stash_description: Some(format!("WIP: {}", analysis.reason)),
                });
            }
            TaskPreparationResult::NeedsSync { git_state, task_meta } => {
                let base_branch = self.determine_base_branch(git_state);

                // Check if branch exists
                if let Ok(false) = self
                    .state_analyzer
                    .branch_exists(&PathBuf::from(&task_meta.task_id), &task_meta.branch_name)
                {
                    actions.push(PreAction::CreateBranch {
                        branch_name: task_meta.branch_name.clone(),
                        base_branch: base_branch.clone(),
                    });
                } else if self.needs_rebase(git_state, &base_branch) {
                    actions.push(PreAction::RebaseToMain { base_branch });
                }
            }
            _ => {}
        }

        actions
    }

    /// Generate a commit message for uncommitted changes
    fn generate_commit_message(&self, analysis: &UncommittedAnalysis) -> String {
        format!(
            "{}: {}",
            match analysis.changes_context {
                crate::uncommitted_handler::ChangesContext::CurrentTask => "wip",
                crate::uncommitted_handler::ChangesContext::PreviousTask => "chore",
                crate::uncommitted_handler::ChangesContext::Unknown => "wip",
                crate::uncommitted_handler::ChangesContext::Temporary => "cleanup",
            },
            analysis.reason
        )
    }
}

impl Default for TaskPreparationPipeline {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_preparation_step_display() {
        assert_eq!(format!("{}", PreparationStep::MetaExtraction), "meta extraction");
        assert_eq!(
            format!("{}", PreparationStep::GitStateAnalysis),
            "git state analysis"
        );
    }

    #[test]
    fn test_pre_action_summary() {
        let action = PreAction::HandleUncommitted {
            action: UncommittedAction::Commit,
            commit_message: None,
            stash_description: None,
        };
        assert!(action.summary().contains("Handle uncommitted"));

        let create = PreAction::CreateBranch {
            branch_name: "feature/test".to_string(),
            base_branch: "main".to_string(),
        };
        assert!(create.summary().contains("Create branch"));

        let rebase = PreAction::RebaseToMain {
            base_branch: "main".to_string(),
        };
        assert!(rebase.summary().contains("Rebase"));
    }

    #[test]
    fn test_task_preparation_pipeline_new() {
        let _pipeline = TaskPreparationPipeline::new();
        // Basic creation test
        assert!(true);
    }
}
