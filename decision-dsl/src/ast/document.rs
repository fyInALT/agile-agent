use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use super::eval::Evaluator;
use super::node::Node;
use super::parser_out::OutputParser;

// ── Tree, Metadata, Spec, Bundle ────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct Tree {
    pub api_version: String,
    pub kind: TreeKind,
    pub metadata: Metadata,
    pub spec: Spec,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Metadata {
    pub name: String,
    pub description: Option<String>,
}

#[derive(Debug, Clone)]
pub struct Spec {
    pub root: Node,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TreeKind {
    BehaviorTree,
    SubTree,
}

#[derive(Default)]
pub struct Bundle {
    pub trees: HashMap<String, Tree>,
    pub subtrees: HashMap<String, Tree>,
}

// ── DslDocument ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub enum DslDocument {
    DecisionRules {
        api_version: String,
        metadata: Metadata,
        rules: Vec<RuleSpec>,
    },
    BehaviorTree {
        api_version: String,
        metadata: Metadata,
        root: Node,
    },
    SubTree {
        api_version: String,
        metadata: Metadata,
        root: Node,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuleSpec {
    pub priority: u32,
    pub name: String,
    #[serde(rename = "if")]
    pub condition: Option<Evaluator>,
    #[serde(rename = "then")]
    pub action: ThenSpec,
    #[serde(rename = "cooldownMs")]
    pub cooldown_ms: Option<u64>,
    #[serde(rename = "reflectionMaxRounds")]
    pub reflection_max_rounds: Option<u8>,
    pub on_error: Option<OnError>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ThenSpec {
    InlineCommand { command: crate::ext::command::DecisionCommand },
    Switch(SwitchSpec),
    When(Box<WhenSpec>),
    Pipeline(PipelineSpec),
    SubTree {
        #[serde(rename = "ref")]
        ref_name: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum OnError {
    Skip,
    Escalate,
    Retry,
}

// ── Switch / When / Pipeline ────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SwitchSpec {
    pub name: String,
    pub on: SwitchOn,
    pub cases: HashMap<String, Box<ThenSpec>>,
    #[serde(rename = "_default")]
    pub default: Option<Box<ThenSpec>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SwitchOn {
    Prompt {
        model: Option<String>,
        #[serde(rename = "timeoutMs")]
        timeout_ms: Option<u64>,
        template: String,
        parser: OutputParser,
        #[serde(rename = "resultKey")]
        result_key: Option<String>,
    },
    Variable {
        key: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WhenSpec {
    pub name: String,
    pub condition: Evaluator,
    pub then: ThenSpec,
    pub on_error: Option<OnError>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipelineSpec {
    pub name: String,
    pub steps: Vec<PipelineStep>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PipelineStep {
    Guard { condition: Evaluator },
    Action { command: crate::ext::command::DecisionCommand },
}
