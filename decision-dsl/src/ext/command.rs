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

// ── AgentCommand ────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "payload")]
pub enum AgentCommand {
    WakeUp,
    #[serde(rename = "SendCustomInstruction")]
    SendCustomInstruction { content: String },
    #[serde(rename = "TerminateAgent")]
    TerminateAgent { status: String },
    RequestSkill { skill: String },
    Transfer { target_agent: String },
}

// ── GitCommand ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "payload")]
pub enum GitCommand {
    CommitChanges { is_wip: bool },
    CreateBranch {
        branch_name: String,
        base_branch: String,
    },
    Rebase { base_branch: String },
    Push { force: bool },
    StageAll,
}

// ── TaskCommand ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "payload")]
pub enum TaskCommand {
    #[serde(rename = "PrepareTaskStart")]
    PrepareTaskStart,
    StartTask { task_id: String },
    FinishTask { task_id: String },
}

// ── HumanCommand ────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "payload")]
pub enum HumanCommand {
    #[serde(rename = "EscalateToHuman")]
    EscalateToHuman { reason: String },
    SelectOption {
        options: Vec<String>,
        default: Option<usize>,
    },
    SkipDecision,
}

// ── ProviderCommand ─────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "payload")]
pub enum ProviderCommand {
    RetryTool {
        tool_name: String,
        #[serde(default)]
        args: Vec<String>,
    },
    SwitchProvider { provider: String },
    SuggestCommit,
    PreparePr {
        title: String,
        description: String,
    },
}
