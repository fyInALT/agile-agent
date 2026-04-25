use serde::{Deserialize, Serialize};
use std::time::Instant;

use crate::ext::blackboard::BlackboardValue;
use crate::ext::command::DecisionCommand;

use super::eval::Evaluator;
use super::parser_out::OutputParser;

// ── Node enum ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeStatus {
    Success,
    Failure,
    Running,
}

// ── NodeBehavior trait ──────────────────────────────────────────────────────

pub trait NodeBehavior {
    fn reset(&mut self);
    fn name(&self) -> &str;
    fn children(&self) -> Vec<&Node>;
    fn children_mut(&mut self) -> Vec<&mut Node>;
}

// ── Node enum ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", content = "payload")]
pub enum Node {
    Selector(SelectorNode),
    Sequence(SequenceNode),
    Parallel(ParallelNode),
    Inverter(InverterNode),
    Repeater(RepeaterNode),
    Cooldown(CooldownNode),
    ReflectionGuard(ReflectionGuardNode),
    ForceHuman(ForceHumanNode),
    When(WhenNode),
    Condition(ConditionNode),
    Action(ActionNode),
    Prompt(PromptNode),
    SetVar(SetVarNode),
    SubTree(SubTreeNode),
}

// ── Composites ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SelectorNode {
    pub name: String,
    pub children: Vec<Node>,
    #[serde(skip)]
    pub active_child: Option<usize>,
    /// Rule name for DecisionRules selectors (used to emit RuleMatched/RulesSkipped trace events).
    #[serde(skip)]
    pub rule_name: Option<String>,
    /// Rule priority for DecisionRules selectors (used to emit RuleMatched trace event).
    #[serde(skip)]
    pub rule_priority: Option<u32>,
    /// Whether RuleMatched has already been emitted for the current tick cycle.
    /// Cleared on selector reset.
    #[serde(skip)]
    pub matched: bool,
}

impl NodeBehavior for SelectorNode {
    fn reset(&mut self) {
        self.active_child = None;
        self.matched = false;
        for child in &mut self.children {
            child.reset();
        }
    }
    fn name(&self) -> &str {
        &self.name
    }
    fn children(&self) -> Vec<&Node> {
        self.children.iter().collect()
    }
    fn children_mut(&mut self) -> Vec<&mut Node> {
        self.children.iter_mut().collect()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SequenceNode {
    pub name: String,
    pub children: Vec<Node>,
    #[serde(skip)]
    pub active_child: Option<usize>,
}

impl NodeBehavior for SequenceNode {
    fn reset(&mut self) {
        self.active_child = None;
        for child in &mut self.children {
            child.reset();
        }
    }
    fn name(&self) -> &str {
        &self.name
    }
    fn children(&self) -> Vec<&Node> {
        self.children.iter().collect()
    }
    fn children_mut(&mut self) -> Vec<&mut Node> {
        self.children.iter_mut().collect()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ParallelPolicy {
    AllSuccess,
    AnySuccess,
    Majority,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParallelNode {
    pub name: String,
    pub policy: ParallelPolicy,
    pub children: Vec<Node>,
    #[serde(skip)]
    pub active_child: Option<usize>,
}

impl NodeBehavior for ParallelNode {
    fn reset(&mut self) {
        self.active_child = None;
        for child in &mut self.children {
            child.reset();
        }
    }
    fn name(&self) -> &str {
        &self.name
    }
    fn children(&self) -> Vec<&Node> {
        self.children.iter().collect()
    }
    fn children_mut(&mut self) -> Vec<&mut Node> {
        self.children.iter_mut().collect()
    }
}

// ── Decorators ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InverterNode {
    pub name: String,
    pub child: Box<Node>,
}

impl NodeBehavior for InverterNode {
    fn reset(&mut self) {
        self.child.reset();
    }
    fn name(&self) -> &str {
        &self.name
    }
    fn children(&self) -> Vec<&Node> {
        vec![&self.child]
    }
    fn children_mut(&mut self) -> Vec<&mut Node> {
        vec![&mut self.child]
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepeaterNode {
    pub name: String,
    #[serde(rename = "maxAttempts")]
    pub max_attempts: u32,
    pub child: Box<Node>,
    #[serde(skip)]
    pub current: u32,
}

impl NodeBehavior for RepeaterNode {
    fn reset(&mut self) {
        self.current = 0;
        self.child.reset();
    }
    fn name(&self) -> &str {
        &self.name
    }
    fn children(&self) -> Vec<&Node> {
        vec![&self.child]
    }
    fn children_mut(&mut self) -> Vec<&mut Node> {
        vec![&mut self.child]
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CooldownNode {
    pub name: String,
    #[serde(rename = "durationMs")]
    pub duration_ms: u64,
    pub child: Box<Node>,
    #[serde(skip)]
    pub last_success: Option<Instant>,
}

impl NodeBehavior for CooldownNode {
    fn reset(&mut self) {
        self.last_success = None;
        self.child.reset();
    }
    fn name(&self) -> &str {
        &self.name
    }
    fn children(&self) -> Vec<&Node> {
        vec![&self.child]
    }
    fn children_mut(&mut self) -> Vec<&mut Node> {
        vec![&mut self.child]
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReflectionGuardNode {
    pub name: String,
    #[serde(rename = "maxRounds")]
    pub max_rounds: u8,
    pub child: Box<Node>,
}

impl NodeBehavior for ReflectionGuardNode {
    fn reset(&mut self) {
        self.child.reset();
    }
    fn name(&self) -> &str {
        &self.name
    }
    fn children(&self) -> Vec<&Node> {
        vec![&self.child]
    }
    fn children_mut(&mut self) -> Vec<&mut Node> {
        vec![&mut self.child]
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ForceHumanNode {
    pub name: String,
    pub reason: String,
    pub child: Box<Node>,
}

impl NodeBehavior for ForceHumanNode {
    fn reset(&mut self) {
        self.child.reset();
    }
    fn name(&self) -> &str {
        &self.name
    }
    fn children(&self) -> Vec<&Node> {
        vec![&self.child]
    }
    fn children_mut(&mut self) -> Vec<&mut Node> {
        vec![&mut self.child]
    }
}

// ── High-level (desugared) ──────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WhenNode {
    pub name: String,
    pub condition: Evaluator,
    pub action: Box<Node>,
}

impl NodeBehavior for WhenNode {
    fn reset(&mut self) {
        self.action.reset();
    }
    fn name(&self) -> &str {
        &self.name
    }
    fn children(&self) -> Vec<&Node> {
        vec![&self.action]
    }
    fn children_mut(&mut self) -> Vec<&mut Node> {
        vec![&mut self.action]
    }
}

// ── Leaves ──────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConditionNode {
    pub name: String,
    pub evaluator: Evaluator,
}

impl NodeBehavior for ConditionNode {
    fn reset(&mut self) {}
    fn name(&self) -> &str {
        &self.name
    }
    fn children(&self) -> Vec<&Node> {
        vec![]
    }
    fn children_mut(&mut self) -> Vec<&mut Node> {
        vec![]
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionNode {
    pub name: String,
    pub command: DecisionCommand,
    pub when: Option<Evaluator>,
}

impl NodeBehavior for ActionNode {
    fn reset(&mut self) {}
    fn name(&self) -> &str {
        &self.name
    }
    fn children(&self) -> Vec<&Node> {
        vec![]
    }
    fn children_mut(&mut self) -> Vec<&mut Node> {
        vec![]
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptNode {
    pub name: String,
    pub model: Option<String>,
    pub template: String,
    pub parser: OutputParser,
    pub sets: Vec<SetMapping>,
    #[serde(rename = "timeoutMs")]
    pub timeout_ms: u64,
    #[serde(skip)]
    pub pending: bool,
    #[serde(skip)]
    pub sent_at: Option<Instant>,
}

impl NodeBehavior for PromptNode {
    fn reset(&mut self) {
        self.pending = false;
        self.sent_at = None;
    }
    fn name(&self) -> &str {
        &self.name
    }
    fn children(&self) -> Vec<&Node> {
        vec![]
    }
    fn children_mut(&mut self) -> Vec<&mut Node> {
        vec![]
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetVarNode {
    pub name: String,
    pub key: String,
    pub value: BlackboardValue,
}

impl NodeBehavior for SetVarNode {
    fn reset(&mut self) {}
    fn name(&self) -> &str {
        &self.name
    }
    fn children(&self) -> Vec<&Node> {
        vec![]
    }
    fn children_mut(&mut self) -> Vec<&mut Node> {
        vec![]
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubTreeNode {
    pub name: String,
    #[serde(rename = "ref")]
    pub ref_name: String,
    #[serde(skip)]
    pub resolved_root: Option<Box<Node>>,
}

impl NodeBehavior for SubTreeNode {
    fn reset(&mut self) {
        if let Some(ref mut root) = self.resolved_root {
            root.reset();
        }
    }
    fn name(&self) -> &str {
        &self.name
    }
    fn children(&self) -> Vec<&Node> {
        match &self.resolved_root {
            Some(root) => vec![root],
            None => vec![],
        }
    }
    fn children_mut(&mut self) -> Vec<&mut Node> {
        match &mut self.resolved_root {
            Some(root) => vec![root],
            None => vec![],
        }
    }
}

// ── SetMapping ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SetMapping {
    pub key: String,
    pub field: String,
}

// ── Manual NodeBehavior impl for Node enum ──────────────────────────────────

impl NodeBehavior for Node {
    fn reset(&mut self) {
        match self {
            Node::Selector(n) => n.reset(),
            Node::Sequence(n) => n.reset(),
            Node::Parallel(n) => n.reset(),
            Node::Inverter(n) => n.reset(),
            Node::Repeater(n) => n.reset(),
            Node::Cooldown(n) => n.reset(),
            Node::ReflectionGuard(n) => n.reset(),
            Node::ForceHuman(n) => n.reset(),
            Node::When(n) => n.reset(),
            Node::Condition(n) => n.reset(),
            Node::Action(n) => n.reset(),
            Node::Prompt(n) => n.reset(),
            Node::SetVar(n) => n.reset(),
            Node::SubTree(n) => n.reset(),
        }
    }

    fn name(&self) -> &str {
        match self {
            Node::Selector(n) => n.name(),
            Node::Sequence(n) => n.name(),
            Node::Parallel(n) => n.name(),
            Node::Inverter(n) => n.name(),
            Node::Repeater(n) => n.name(),
            Node::Cooldown(n) => n.name(),
            Node::ReflectionGuard(n) => n.name(),
            Node::ForceHuman(n) => n.name(),
            Node::When(n) => n.name(),
            Node::Condition(n) => n.name(),
            Node::Action(n) => n.name(),
            Node::Prompt(n) => n.name(),
            Node::SetVar(n) => n.name(),
            Node::SubTree(n) => n.name(),
        }
    }

    fn children(&self) -> Vec<&Node> {
        match self {
            Node::Selector(n) => n.children(),
            Node::Sequence(n) => n.children(),
            Node::Parallel(n) => n.children(),
            Node::Inverter(n) => n.children(),
            Node::Repeater(n) => n.children(),
            Node::Cooldown(n) => n.children(),
            Node::ReflectionGuard(n) => n.children(),
            Node::ForceHuman(n) => n.children(),
            Node::When(n) => n.children(),
            Node::Condition(n) => n.children(),
            Node::Action(n) => n.children(),
            Node::Prompt(n) => n.children(),
            Node::SetVar(n) => n.children(),
            Node::SubTree(n) => n.children(),
        }
    }

    fn children_mut(&mut self) -> Vec<&mut Node> {
        match self {
            Node::Selector(n) => n.children_mut(),
            Node::Sequence(n) => n.children_mut(),
            Node::Parallel(n) => n.children_mut(),
            Node::Inverter(n) => n.children_mut(),
            Node::Repeater(n) => n.children_mut(),
            Node::Cooldown(n) => n.children_mut(),
            Node::ReflectionGuard(n) => n.children_mut(),
            Node::ForceHuman(n) => n.children_mut(),
            Node::When(n) => n.children_mut(),
            Node::Condition(n) => n.children_mut(),
            Node::Action(n) => n.children_mut(),
            Node::Prompt(n) => n.children_mut(),
            Node::SetVar(n) => n.children_mut(),
            Node::SubTree(n) => n.children_mut(),
        }
    }
}
