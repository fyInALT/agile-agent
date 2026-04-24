# Decision DSL: AST & Blackboard

> Data model specification for the decision DSL engine. Covers the abstract syntax tree (AST) that represents parsed YAML behavior trees, and the Blackboard — the shared typed state store that all nodes read from and write to during execution.
>
> This document is a chapter of the [Decision DSL Implementation](decision-dsl-implementation.md).

## AST Design

The AST exactly mirrors the YAML spec: `apiVersion`, `kind`, `metadata`, `spec`.

###1 Tree, Metadata, Bundle

```rust
pub(crate) struct Tree {
    pub api_version: String,
    pub kind: TreeKind,
    pub metadata: Metadata,
    pub spec: Spec,
}

pub(crate) struct Metadata {
    pub name: String,
    pub description: Option<String>,
}

pub(crate) struct Spec {
    pub root: Node,
}

pub(crate) enum TreeKind {
    BehaviorTree,
    SubTree,
}

pub(crate) struct Bundle {
    pub trees: HashMap<String, Tree>,
    pub subtrees: HashMap<String, Tree>,
}

impl Bundle {
    /// Inline all SubTree references into their parent trees.
    pub fn resolve_subtrees(&mut self) -> Result<(), ParseError> {
        for tree in self.trees.values_mut() {
            tree.spec.root.resolve_subtrees(&self.subtrees)?;
        }
        Ok(())
    }

    /// Detect circular SubTree references: A → B → C → A.
    pub fn detect_circular_refs(&self) -> Result<(), ParseError> {
        for tree in self.trees.values() {
            let mut visited = HashSet::new();
            tree.spec.root.detect_cycles(&mut visited)?;
        }
        Ok(())
    }
}
```

###2 Node Enum

```rust
pub(crate) enum Node {
    Selector(SelectorNode),
    Sequence(SequenceNode),
    Parallel(ParallelNode),
    Inverter(InverterNode),
    Repeater(RepeaterNode),
    Cooldown(CooldownNode),
    ReflectionGuard(ReflectionGuardNode),
    ForceHuman(ForceHumanNode),
    Condition(ConditionNode),
    Action(ActionNode),
    Prompt(PromptNode),
    SetVar(SetVarNode),
    SubTreeRef(SubTreeRefNode),
}

impl Node {
    pub fn tick(&mut self, ctx: &mut TickContext, tracer: &mut Tracer) -> Result<NodeStatus, RuntimeError> {
        match self {
            Node::Selector(n) => n.tick(ctx, tracer),
            Node::Sequence(n) => n.tick(ctx, tracer),
            Node::Parallel(n) => n.tick(ctx, tracer),
            Node::Inverter(n) => n.tick(ctx, tracer),
            Node::Repeater(n) => n.tick(ctx, tracer),
            Node::Cooldown(n) => n.tick(ctx, tracer),
            Node::ReflectionGuard(n) => n.tick(ctx, tracer),
            Node::ForceHuman(n) => n.tick(ctx, tracer),
            Node::Condition(n) => n.tick(ctx, tracer),
            Node::Action(n) => n.tick(ctx, tracer),
            Node::Prompt(n) => n.tick(ctx, tracer),
            Node::SetVar(n) => n.tick(ctx, tracer),
            Node::SubTreeRef(n) => n.tick(ctx, tracer),
        }
    }

    pub fn reset(&mut self) {
        match self {
            Node::Selector(n) => n.reset(),
            Node::Sequence(n) => n.reset(),
            Node::Parallel(n) => n.reset(),
            Node::Inverter(n) => n.reset(),
            Node::Repeater(n) => n.reset(),
            Node::Cooldown(n) => n.reset(),
            Node::ReflectionGuard(n) => n.reset(),
            Node::ForceHuman(n) => n.reset(),
            Node::Condition(n) => n.reset(),
            Node::Action(n) => n.reset(),
            Node::Prompt(n) => n.reset(),
            Node::SetVar(n) => n.reset(),
            Node::SubTreeRef(n) => n.reset(),
        }
    }

    pub fn name(&self) -> &str {
        match self {
            Node::Selector(n) => &n.name,
            Node::Sequence(n) => &n.name,
            Node::Parallel(n) => &n.name,
            Node::Inverter(n) => &n.name,
            Node::Repeater(n) => &n.name,
            Node::Cooldown(n) => &n.name,
            Node::ReflectionGuard(n) => &n.name,
            Node::ForceHuman(n) => &n.name,
            Node::Condition(n) => &n.name,
            Node::Action(n) => &n.name,
            Node::Prompt(n) => &n.name,
            Node::SetVar(n) => &n.name,
            Node::SubTreeRef(n) => &n.name,
        }
    }

    /// Recursively inline SubTree references.
    pub fn resolve_subtrees(&mut self, subtrees: &HashMap<String, Tree>) -> Result<(), ParseError> {
        match self {
            Node::SubTreeRef(n) => {
                let subtree = subtrees.get(&n.ref_name)
                    .ok_or_else(|| ParseError::UnresolvedSubTree { name: n.ref_name.clone() })?;
                *self = subtree.spec.root.clone();
                // Recurse into the inlined tree
                self.resolve_subtrees(subtrees)?;
            }
            Node::Selector(n) => {
                for child in &mut n.children { child.resolve_subtrees(subtrees)?; }
            }
            Node::Sequence(n) => {
                for child in &mut n.children { child.resolve_subtrees(subtrees)?; }
            }
            Node::Parallel(n) => {
                for child in &mut n.children { child.resolve_subtrees(subtrees)?; }
            }
            Node::Inverter(n) => n.child.resolve_subtrees(subtrees)?,
            Node::Repeater(n) => n.child.resolve_subtrees(subtrees)?,
            Node::Cooldown(n) => n.child.resolve_subtrees(subtrees)?,
            Node::ReflectionGuard(n) => n.child.resolve_subtrees(subtrees)?,
            Node::ForceHuman(n) => n.child.resolve_subtrees(subtrees)?,
            _ => {}
        }
        Ok(())
    }

    /// Detect circular SubTree references.
    pub fn detect_cycles(&self, visited: &mut HashSet<String>) -> Result<(), ParseError> {
        match self {
            Node::SubTreeRef(n) => {
                if !visited.insert(n.ref_name.clone()) {
                    return Err(ParseError::CircularSubTreeRef { name: n.ref_name.clone() });
                }
            }
            Node::Selector(n) => {
                for child in &n.children { child.detect_cycles(visited)?; }
            }
            Node::Sequence(n) => {
                for child in &n.children { child.detect_cycles(visited)?; }
            }
            Node::Parallel(n) => {
                for child in &n.children { child.detect_cycles(visited)?; }
            }
            Node::Inverter(n) => n.child.detect_cycles(visited)?,
            Node::Repeater(n) => n.child.detect_cycles(visited)?,
            Node::Cooldown(n) => n.child.detect_cycles(visited)?,
            Node::ReflectionGuard(n) => n.child.detect_cycles(visited)?,
            Node::ForceHuman(n) => n.child.detect_cycles(visited)?,
            _ => {}
        }
        Ok(())
    }
}
```

###3 Node-Specific Structs

```rust
// --- Composites ---

pub(crate) struct SelectorNode {
    pub name: String,
    pub children: Vec<Node>,
    pub active_child: Option<usize>,
}

pub(crate) struct SequenceNode {
    pub name: String,
    pub children: Vec<Node>,
    pub active_child: Option<usize>,
}

pub(crate) enum ParallelPolicy {
    AllSuccess,
    AnySuccess,
    Majority,
}

pub(crate) struct ParallelNode {
    pub name: String,
    pub policy: ParallelPolicy,
    pub children: Vec<Node>,
    pub child_statuses: Vec<NodeStatus>,
}

// --- Decorators ---

pub(crate) struct InverterNode {
    pub name: String,
    pub child: Box<Node>,
}

pub(crate) struct RepeaterNode {
    pub name: String,
    pub max_attempts: u32,
    pub child: Box<Node>,
    pub current: u32,
}

pub(crate) struct CooldownNode {
    pub name: String,
    pub duration: Duration,
    pub child: Box<Node>,
    pub last_success: Option<Instant>,
}

pub(crate) struct ReflectionGuardNode {
    pub name: String,
    pub max_rounds: u8,
    pub child: Box<Node>,
}

pub(crate) struct ForceHumanNode {
    pub name: String,
    pub reason: String,
    pub child: Box<Node>,
}

// --- Leaves ---

pub(crate) struct ConditionNode {
    pub name: String,
    pub evaluator: Box<dyn Evaluator>,
}

pub(crate) struct ActionNode {
    pub name: String,
    pub command: Command,
    pub when: Option<Box<dyn Evaluator>>,
}

pub(crate) struct PromptNode {
    pub name: String,
    pub model: Option<String>,       // "standard" | "thinking"
    pub template: String,
    pub parser: Box<dyn OutputParser>,
    pub sets: Vec<SetMapping>,
    pub timeout: Duration,
    pub pending: bool,
}

pub(crate) struct SetVarNode {
    pub name: String,
    pub key: String,
    pub value: BlackboardValue,
}

pub(crate) struct SubTreeRefNode {
    pub name: String,
    pub ref_name: String,
}

pub(crate) struct SetMapping {
    pub key: String,      // blackboard key
    pub field: String,    // parser field
}
```

###4 Command (Output Type)

```rust
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Command {
    EscalateToHuman { reason: String, context: Option<String> },
    RetryTool { tool_name: String, args: Option<String>, max_attempts: u32 },
    SendCustomInstruction { prompt: String, target_agent: String },
    ApproveAndContinue,
    TerminateAgent { reason: String },
    SwitchProvider { provider_type: String },
    SelectOption { option_id: String },
    SkipDecision,
    ConfirmCompletion,
    Reflect { prompt: String },
    StopIfComplete { reason: String },
    PrepareTaskStart { task_id: String, task_description: String },
    SuggestCommit { message: String, mandatory: bool, reason: String },
    CommitChanges { message: String, is_wip: bool, worktree_path: Option<String> },
    StashChanges { description: String, include_untracked: bool, worktree_path: Option<String> },
    DiscardChanges { worktree_path: Option<String> },
    CreateTaskBranch { branch_name: String, base_branch: String, worktree_path: Option<String> },
    RebaseToMain { base_branch: String },
    WakeUp,
    Unknown { action_type: String, params: String },
}
```

---

## Blackboard Design

The Blackboard is the shared memory of the behavior tree. It matches the spec exactly: built-in variables, custom variables, commands, and LLM responses.

###1 Data Model

```rust
/// The shared state for a single decision cycle.
pub struct Blackboard {
    // --- Built-in input variables ---
    pub task_description: String,
    pub provider_output: String,
    pub context_summary: String,
    pub reflection_round: u8,
    pub max_reflection_rounds: u8,
    pub confidence_accumulator: f64,
    pub agent_id: String,
    pub current_task_id: String,
    pub current_story_id: String,

    // --- Structured state ---
    pub last_tool_call: Option<ToolCallRecord>,
    pub file_changes: Vec<FileChangeRecord>,
    pub project_rules: ProjectRules,
    pub decision_history: Vec<DecisionRecord>,

    // --- Custom variables ---
    pub variables: HashMap<String, BlackboardValue>,

    // --- Outputs ---
    pub commands: Vec<Command>,
    pub llm_responses: HashMap<String, String>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum BlackboardValue {
    String(String),
    Integer(i64),
    Float(f64),
    Boolean(bool),
    List(Vec<BlackboardValue>),
    Map(HashMap<String, BlackboardValue>),
}

#[derive(Debug, Clone)]
pub struct ToolCallRecord {
    pub name: String,
    pub input: String,
    pub output: String,
}

#[derive(Debug, Clone)]
pub struct FileChangeRecord {
    pub path: String,
    pub change_type: String,
}

#[derive(Debug, Clone, Default)]
pub struct ProjectRules {
    pub rules: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct DecisionRecord {
    pub situation: String,
    pub command: Command,
    pub timestamp: String,
}
```

###2 Unified Access Interface

All template rendering, evaluator access, and dot-notation paths go through a unified interface:

```rust
impl Blackboard {
    /// Get a value by dot-notation path.
    /// Supported paths:
    ///   - "task_description"
    ///   - "reflection_round"
    ///   - "variables.next_action"
    ///   - "last_tool_call.name"
    ///   - "file_changes.0.path"
    pub fn get_path(&self, path: &str) -> Option<BlackboardValue> {
        let mut parts = path.split('.');
        let first = parts.next()?;

        // Check built-in fields first
        let mut current = match first {
            "task_description" => Some(BlackboardValue::String(self.task_description.clone())),
            "provider_output" => Some(BlackboardValue::String(self.provider_output.clone())),
            "context_summary" => Some(BlackboardValue::String(self.context_summary.clone())),
            "reflection_round" => Some(BlackboardValue::Integer(self.reflection_round as i64)),
            "max_reflection_rounds" => Some(BlackboardValue::Integer(self.max_reflection_rounds as i64)),
            "confidence_accumulator" => Some(BlackboardValue::Float(self.confidence_accumulator)),
            "agent_id" => Some(BlackboardValue::String(self.agent_id.clone())),
            "current_task_id" => Some(BlackboardValue::String(self.current_task_id.clone())),
            "current_story_id" => Some(BlackboardValue::String(self.current_story_id.clone())),
            "last_tool_call" => self.last_tool_call.as_ref().map(|t| {
                let mut m = HashMap::new();
                m.insert("name".into(), BlackboardValue::String(t.name.clone()));
                m.insert("input".into(), BlackboardValue::String(t.input.clone()));
                m.insert("output".into(), BlackboardValue::String(t.output.clone()));
                BlackboardValue::Map(m)
            }),
            "file_changes" => Some(BlackboardValue::List(
                self.file_changes.iter().map(|fc| {
                    let mut m = HashMap::new();
                    m.insert("path".into(), BlackboardValue::String(fc.path.clone()));
                    m.insert("change_type".into(), BlackboardValue::String(fc.change_type.clone()));
                    BlackboardValue::Map(m)
                }).collect()
            )),
            "variables" => {
                // Continue with variable lookup
                let key = parts.next()?;
                return self.variables.get(key).cloned();
            }
            _ => None,
        };

        // Navigate nested paths for Map/List values
        for part in parts {
            current = match current? {
                BlackboardValue::Map(m) => m.get(part).cloned(),
                BlackboardValue::List(l) => {
                    let idx: usize = part.parse().ok()?;
                    l.get(idx).cloned()
                }
                _ => None,
            };
        }

        current
    }

    /// Typed convenience getters (used by evaluators and template engine).
    pub fn get_string(&self, key: &str) -> Option<String> {
        self.get_path(key).and_then(|v| match v {
            BlackboardValue::String(s) => Some(s),
            _ => None,
        })
    }

    pub fn get_bool(&self, key: &str) -> Option<bool> {
        self.get_path(key).and_then(|v| match v {
            BlackboardValue::Boolean(b) => Some(b),
            _ => None,
        })
    }

    pub fn get_u8(&self, key: &str) -> Option<u8> {
        self.get_path(key).and_then(|v| match v {
            BlackboardValue::Integer(i) => i.try_into().ok(),
            _ => None,
        })
    }

    pub fn get_f64(&self, key: &str) -> Option<f64> {
        self.get_path(key).and_then(|v| match v {
            BlackboardValue::Float(f) => Some(f),
            BlackboardValue::Integer(i) => Some(i as f64),
            _ => None,
        })
    }

    /// Set a variable in the custom variables map.
    pub fn set(&mut self, key: &str, value: BlackboardValue) {
        self.variables.insert(key.to_string(), value);
    }

    pub fn set_string(&mut self, key: &str, value: String) {
        self.variables.insert(key.to_string(), BlackboardValue::String(value));
    }

    pub fn set_u8(&mut self, key: &str, value: u8) {
        self.variables.insert(key.to_string(), BlackboardValue::Integer(value as i64));
    }

    pub fn set_f64(&mut self, key: &str, value: f64) {
        self.variables.insert(key.to_string(), BlackboardValue::Float(value));
    }

    pub fn set_bool(&mut self, key: &str, value: bool) {
        self.variables.insert(key.to_string(), BlackboardValue::Boolean(value));
    }

    /// Push a command to the output list.
    pub(crate) fn push_command(&mut self, cmd: Command) {
        self.commands.push(cmd);
    }

    /// Drain all commands (called at end of tick).
    pub(crate) fn drain_commands(&mut self) -> Vec<Command> {
        std::mem::take(&mut self.commands)
    }

    /// Store an LLM response for debugging.
    pub(crate) fn store_llm_response(&mut self, node_name: &str, response: String) {
        self.llm_responses.insert(node_name.to_string(), response);
    }
}
```

---

