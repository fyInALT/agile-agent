use serde::{Deserialize, Serialize};

// ── DecisionCommand (grouped) ───────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum DecisionCommand {
    Agent(AgentCommand),
    Git(GitCommand, Option<String>),
    Task(TaskCommand),
    Human(HumanCommand),
    Provider(ProviderCommand),
}

// ── AgentCommand ───────────────────────────────────────────────────────────

// serde rename mapping:
//   SendInstruction → SendCustomInstruction
//   Terminate → TerminateAgent
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "payload")]
pub enum AgentCommand {
    ApproveAndContinue,
    Reflect { prompt: String },
    #[serde(rename = "SendCustomInstruction")]
    SendInstruction { prompt: String, target_agent: String },
    #[serde(rename = "TerminateAgent")]
    Terminate { reason: String },
    WakeUp,
}

// ── GitCommand ──────────────────────────────────────────────────────────────

// serde rename mapping:
//   Commit { wip } → CommitChanges { is_wip }
//   CreateBranch { name, base } → CreateTaskBranch { branch_name, base_branch }
//   Rebase { base } → RebaseToMain { base_branch }
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "payload")]
pub enum GitCommand {
    #[serde(rename = "CommitChanges")]
    Commit { message: String, #[serde(rename = "is_wip")] wip: bool },
    #[serde(rename = "StashChanges")]
    Stash { description: String, include_untracked: bool },
    #[serde(rename = "DiscardChanges")]
    Discard,
    #[serde(rename = "CreateTaskBranch")]
    CreateBranch {
        #[serde(rename = "branch_name")]
        name: String,
        #[serde(rename = "base_branch")]
        base: String,
    },
    #[serde(rename = "RebaseToMain")]
    Rebase { #[serde(rename = "base_branch")] base: String },
}

// ── TaskCommand ─────────────────────────────────────────────────────────────

// serde rename mapping:
//   PrepareStart → PrepareTaskStart
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "payload")]
pub enum TaskCommand {
    ConfirmCompletion,
    StopIfComplete { reason: String },
    #[serde(rename = "PrepareTaskStart")]
    PrepareStart { task_id: String, description: String },
}

// ── HumanCommand ───────────────────────────────────────────────────────────

// serde rename mapping:
//   Escalate → EscalateToHuman
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "payload")]
pub enum HumanCommand {
    #[serde(rename = "EscalateToHuman")]
    Escalate { reason: String, context: Option<String> },
    SelectOption { option_id: String },
    SkipDecision,
}

// ── ProviderCommand ─────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "payload")]
pub enum ProviderCommand {
    RetryTool {
        tool_name: String,
        args: Option<String>,
        max_attempts: u32,
    },
    SwitchProvider { provider_type: String },
    SuggestCommit { message: String, mandatory: bool, reason: String },
    PreparePr { title: String, description: String, base: String, draft: bool },
}
