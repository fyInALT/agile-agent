# Decision DSL: AST & Blackboard

> Data model specification for the decision DSL engine. Covers the abstract syntax tree (AST), the desugaring pass that compiles high-level constructs to low-level nodes, and the Blackboard — the scoped typed state store that all nodes read from and write to during execution.

---

## 1. AST Design

### 1.1 Tree, Metadata, Bundle

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

/// A Bundle holds all parsed trees and subtrees.
/// SubTree references are resolved at load time but identity is preserved.
#[derive(Default)]
pub(crate) struct Bundle {
    pub trees: HashMap<String, Tree>,
    pub subtrees: HashMap<String, Tree>,
}
```

### 1.2 DSL Document (Parse-Time Representation)

Before desugaring, the YAML parser produces a `DslDocument` that mirrors the raw YAML:

```rust
pub(crate) enum DslDocument {
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

pub(crate) struct RuleSpec {
    pub priority: u32,
    pub name: String,
    pub condition: Option<Evaluator>,       // `if` field
    pub action: ThenSpec,                   // `then` field
    #[serde(rename = "cooldownMs")]
    pub cooldown_ms: Option<u64>,
    #[serde(rename = "reflectionMaxRounds")]
    pub reflection_max_rounds: Option<u8>,
    pub on_error: Option<OnError>,
}

pub(crate) enum ThenSpec {
    InlineCommand { command: DecisionCommand },
    Switch(SwitchSpec),
    When(WhenSpec),
    Pipeline(PipelineSpec),
    SubTree { ref_name: String },
}

pub(crate) enum OnError {
    Skip,
    Escalate,
    Retry,
}
```

### 1.3 Desugaring: DecisionRules → BehaviorTree AST

The desugaring pass converts `DslDocument::DecisionRules` into a `Tree` with a `Selector` root:

```rust
impl DslDocument {
    pub fn desugar(self, registry: &EvaluatorRegistry) -> Result<Tree, ParseError> {
        match self {
            DslDocument::DecisionRules { api_version, metadata, rules } => {
                let mut children = Vec::new();
                for rule in rules {
                    children.push(desugar_rule(rule, registry)?);
                }
                // Default fallback: if no rule matches, succeed with no command
                children.push(Node::Action(ActionNode {
                    name: "no_match".into(),
                    command: DecisionCommand::Agent(AgentCommand::ApproveAndContinue),
                    when: None,
                }));
                Ok(Tree {
                    api_version,
                    kind: TreeKind::BehaviorTree,
                    metadata,
                    spec: Spec {
                        root: Node::Selector(SelectorNode {
                            name: format!("{}_root", metadata.name),
                            children,
                            active_child: None,
                        }),
                    },
                })
            }
            DslDocument::BehaviorTree { api_version, metadata, root } => {
                Ok(Tree { api_version, kind: TreeKind::BehaviorTree, metadata, spec: Spec { root } })
            }
            DslDocument::SubTree { api_version, metadata, root } => {
                Ok(Tree { api_version, kind: TreeKind::SubTree, metadata, spec: Spec { root } })
            }
        }
    }
}

fn desugar_rule(rule: RuleSpec, registry: &EvaluatorRegistry) -> Result<Node, ParseError> {
    let inner = desugar_then(rule.action, registry)?;

    // Wrap in Sequence + Condition if `if` is present
    let mut node = if let Some(evaluator) = rule.condition {
        Node::Sequence(SequenceNode {
            name: format!("{}_guard", rule.name),
            children: vec![
                Node::Condition(ConditionNode {
                    name: format!("{}_cond", rule.name),
                    evaluator,
                }),
                inner,
            ],
            active_child: None,
        })
    } else {
        inner
    };

    // Wrap in Cooldown if specified (outermost — applies after all other decorators)
    if let Some(ms) = rule.cooldown_ms {
        node = Node::Cooldown(CooldownNode {
            name: format!("{}_cooldown", rule.name),
            duration_ms: ms,
            child: Box::new(node),
            last_success: None,
        });
    }

    // Wrap in ReflectionGuard if specified (inside Cooldown, so reflection counting
    // happens even during cooldown — prevents infinite reflection loops)
    if let Some(max_rounds) = rule.reflection_max_rounds {
        node = Node::ReflectionGuard(ReflectionGuardNode {
            name: format!("{}_reflection", rule.name),
            max_rounds,
            child: Box::new(node),
        });
    }

    // Apply on_error wrapping (innermost — closest to the action)
    node = match rule.on_error {
        Some(OnError::Escalate) => {
            Node::Selector(SelectorNode {
                name: format!("{}_error_handler", rule.name),
                children: vec![
                    node,
                    Node::Action(ActionNode {
                        name: format!("{}_escalate_fallback", rule.name),
                        command: DecisionCommand::Human(HumanCommand::Escalate {
                            reason: format!("Rule '{}' failed — escalating to human", rule.name),
                            context: None,
                        }),
                        when: None,
                    }),
                ],
                active_child: None,
            })
        }
        Some(OnError::Retry) => {
            Node::Repeater(RepeaterNode {
                name: format!("{}_retry", rule.name),
                max_attempts: 2, // original attempt + 1 retry
                child: Box::new(node),
                current: 0,
            })
        }
        Some(OnError::Skip) | None => node, // Selector naturally falls through on Failure
    };

    Ok(node)
}

fn desugar_then(then: ThenSpec, registry: &EvaluatorRegistry) -> Result<Node, ParseError> {
    match then {
        ThenSpec::InlineCommand { command } => {
            Ok(Node::Action(ActionNode {
                name: "emit".into(),
                command,
                when: None,
            }))
        }
        ThenSpec::Switch(switch) => desugar_switch(switch, registry),
        ThenSpec::When(when) => desugar_when(when, registry),
        ThenSpec::Pipeline(pipeline) => desugar_pipeline(pipeline, registry),
        ThenSpec::SubTree { ref_name } => {
            Ok(Node::SubTree(SubTreeNode {
                name: ref_name.clone(),
                ref_name,
            }))
        }
    }
}

fn desugar_switch(switch: SwitchSpec, registry: &EvaluatorRegistry) -> Result<Node, ParseError> {
    match switch.on {
        SwitchOn::Prompt { model, timeout_ms, template, parser, result_key } => {
            let result_key = result_key.unwrap_or_else(|| "decision".into());

            // Sequence(Prompt, Selector(case_when_1, case_when_2, ..., DefaultCase))
            let prompt_node = Node::Prompt(PromptNode {
                name: format!("{}_prompt", switch.name),
                model,
                template,
                parser,
                sets: vec![SetMapping {
                    key: result_key.clone(),
                    field: "decision".into(), // enum/structured parser writes to this field name
                }],
                timeout_ms: timeout_ms.unwrap_or(30000),
                pending: false,
            });

            let mut case_nodes = Vec::new();
            for (value, action) in &switch.cases {
                let when = Node::When(WhenNode {
                    name: format!("{}_{}", switch.name, value.to_lowercase()),
                    condition: Evaluator::VariableIs {
                        key: result_key.clone(),
                        expected: BlackboardValue::String(value.clone()),
                    },
                    action: Box::new(desugar_then(action.clone(), registry)?),
                });
                case_nodes.push(when);
            }

            // Default case (supports any ThenSpec, not just inline commands)
            if let Some(default_action) = switch.default {
                case_nodes.push(desugar_then(default_action, registry)?);
            }

            Ok(Node::Sequence(SequenceNode {
                name: switch.name,
                children: vec![
                    prompt_node,
                    Node::Selector(SelectorNode {
                        name: format!("{}_branch", switch.name),
                        children: case_nodes,
                        active_child: None,
                    }),
                ],
                active_child: None,
            }))
        }
        SwitchOn::Variable { key } => {
            let mut case_nodes = Vec::new();
            for (value, action) in &switch.cases {
                let when = Node::When(WhenNode {
                    name: format!("{}_{}", switch.name, value.to_lowercase()),
                    condition: Evaluator::VariableIs {
                        key: key.clone(),
                        expected: BlackboardValue::String(value.clone()),
                    },
                    action: Box::new(desugar_then(action.clone(), registry)?),
                });
                case_nodes.push(when);
            }
            if let Some(default_action) = switch.default {
                case_nodes.push(desugar_then(default_action, registry)?);
            }
            Ok(Node::Selector(SelectorNode {
                name: format!("{}_branch", switch.name),
                children: case_nodes,
                active_child: None,
            }))
        }
    }
}

fn desugar_when(when: WhenSpec, _registry: &EvaluatorRegistry) -> Result<Node, ParseError> {
    Ok(Node::When(WhenNode {
        name: when.name.clone(),
        condition: when.condition,
        action: Box::new(desugar_then(when.then, _registry)?),
    }))
}

fn desugar_pipeline(pipeline: PipelineSpec, _registry: &EvaluatorRegistry) -> Result<Node, ParseError> {
    let mut children = Vec::new();
    for step in pipeline.steps {
        match step {
            PipelineStep::Guard { condition } => {
                children.push(Node::Condition(ConditionNode {
                    name: format!("{}_step", pipeline.name),
                    evaluator: condition,
                }));
            }
            PipelineStep::Action { command } => {
                children.push(Node::Action(ActionNode {
                    name: format!("{}_emit", pipeline.name),
                    command,
                    when: None,
                }));
            }
        }
    }
    Ok(Node::Sequence(SequenceNode {
        name: pipeline.name,
        children,
        active_child: None,
    }))
}
```

### 1.4 Node Enum (Low-Level AST)

The `Node` enum uses a **manual `impl NodeBehavior for Node`** rather than `enum_dispatch`.
`enum_dispatch` 0.3.x is incompatible with `#[derive(Serialize, Deserialize)]` on the same enum,
so the match arms are written by hand. Adding a new node type requires updating this impl,
but the set of node types is fixed by the DSL spec, so this is a one-time cost.

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", content = "payload")]
pub(crate) enum Node {
    // Composites
    Selector(SelectorNode),
    Sequence(SequenceNode),
    Parallel(ParallelNode),

    // Decorators
    Inverter(InverterNode),
    Repeater(RepeaterNode),
    Cooldown(CooldownNode),
    ReflectionGuard(ReflectionGuardNode),
    ForceHuman(ForceHumanNode),

    // High-level (desugared, but kept as distinct types for tracing)
    When(WhenNode),

    // Leaves
    Condition(ConditionNode),
    Action(ActionNode),
    Prompt(PromptNode),
    SetVar(SetVarNode),

    // Sub-tree reference (identity preserved)
    SubTree(SubTreeNode),
}

// Manual impl (same behavior as enum_dispatch-generated impl):
impl NodeBehavior for Node {
    fn reset(&mut self) { /* delegates to each variant's reset() */ }
    fn name(&self) -> &str { /* delegates to each variant's name() */ }
    fn children(&self) -> Vec<&Node> { /* delegates to each variant's children() */ }
    fn children_mut(&mut self) -> Vec<&mut Node> { /* delegates to each variant's children_mut() */ }
}
```

### 1.5 Node-Specific Structs

```rust
// --- Composites ---

pub(crate) struct SelectorNode {
    pub name: String,
    pub children: Vec<Node>,
    #[serde(skip)]
    pub active_child: Option<usize>,
}

pub(crate) struct SequenceNode {
    pub name: String,
    pub children: Vec<Node>,
    #[serde(skip)]
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
}

// --- Decorators ---

pub(crate) struct InverterNode {
    pub name: String,
    pub child: Box<Node>,
}

pub(crate) struct RepeaterNode {
    pub name: String,
    #[serde(rename = "maxAttempts")]
    pub max_attempts: u32,
    pub child: Box<Node>,
    #[serde(skip)]
    pub current: u32,
}

pub(crate) struct CooldownNode {
    pub name: String,
    #[serde(rename = "durationMs")]
    pub duration_ms: u64,
    pub child: Box<Node>,
    #[serde(skip)]
    pub last_success: Option<Instant>,
}

pub(crate) struct ReflectionGuardNode {
    pub name: String,
    #[serde(rename = "maxRounds")]
    pub max_rounds: u8,
    pub child: Box<Node>,
}

pub(crate) struct ForceHumanNode {
    pub name: String,
    pub reason: String,
    pub child: Box<Node>,
}

// --- High-Level (desugared) ---

pub(crate) struct WhenNode {
    pub name: String,
    pub condition: Evaluator,
    pub action: Box<Node>,
}

// --- Leaves ---

pub(crate) struct ConditionNode {
    pub name: String,
    pub evaluator: Evaluator,
}

pub(crate) struct ActionNode {
    pub name: String,
    pub command: DecisionCommand,
    pub when: Option<Evaluator>,
}

pub(crate) struct PromptNode {
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
    pub sent_at: Option<std::time::Instant>,
}

pub(crate) struct SetVarNode {
    pub name: String,
    pub key: String,
    pub value: BlackboardValue,
}

pub(crate) struct SubTreeNode {
    pub name: String,
    #[serde(rename = "ref")]
    pub ref_name: String,
    #[serde(skip)]
    pub resolved_root: Option<Box<Node>>,  // Resolved at tick time
}

pub(crate) struct SetMapping {
    pub key: String,
    pub field: String,
}
```

### 1.6 Switch / When / Pipeline Spec Types (Parse-Time)

```rust
pub(crate) struct SwitchSpec {
    pub name: String,
    pub on: SwitchOn,
    pub cases: HashMap<String, ThenSpec>,
    #[serde(rename = "_default")]
    pub default: Option<ThenSpec>,
}

pub(crate) enum SwitchOn {
    Prompt {
        model: Option<String>,
        #[serde(rename = "timeoutMs")]
        timeout_ms: Option<u64>,
        template: String,
        parser: OutputParser,
        /// Blackboard key where the parser result is stored. Default: "decision".
        /// Cases match against this key.
        #[serde(rename = "resultKey")]
        result_key: Option<String>,
    },
    Variable {
        key: String,
    },
}

pub(crate) struct WhenSpec {
    pub name: String,
    pub condition: Evaluator,
    pub then: ThenSpec,
    pub on_error: Option<OnError>,
}

pub(crate) struct PipelineSpec {
    pub name: String,
    pub steps: Vec<PipelineStep>,
}

pub(crate) enum PipelineStep {
    Guard { condition: Evaluator },
    Action { command: DecisionCommand },
}
```

### 1.7 Grouped Command Enum

```rust
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum DecisionCommand {
    Agent(AgentCommand),
    Git(GitCommand, Option<String>),     // worktree_path
    Task(TaskCommand),
    Human(HumanCommand),
    Provider(ProviderCommand),
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum AgentCommand {
    ApproveAndContinue,
    Reflect { prompt: String },
    #[serde(rename = "SendCustomInstruction")]
    SendInstruction { prompt: String, target_agent: String },
    #[serde(rename = "TerminateAgent")]
    Terminate { reason: String },
    WakeUp,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
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

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum TaskCommand {
    ConfirmCompletion,
    StopIfComplete { reason: String },
    #[serde(rename = "PrepareTaskStart")]
    PrepareStart { task_id: String, description: String },
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum HumanCommand {
    #[serde(rename = "EscalateToHuman")]
    Escalate { reason: String, context: Option<String> },
    SelectOption { option_id: String },
    SkipDecision,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum ProviderCommand {
    RetryTool { tool_name: String, args: Option<String>, max_attempts: u32 },
    SwitchProvider { provider_type: String },
    SuggestCommit { message: String, mandatory: bool, reason: String },
    PreparePr { title: String, description: String, base: String, draft: bool },
}
```

### 1.8 Serde Rename Mapping (Rust ↔ YAML)

Rust enum variants use idiomatic short names. YAML serialization maps to the full DSL command names via `#[serde(rename)]`:

| Rust Variant | YAML Name (in DSL) | Field Renames |
|-------------|---------------------|---------------|
| `AgentCommand::SendInstruction` | `SendCustomInstruction` | — |
| `AgentCommand::Terminate` | `TerminateAgent` | — |
| `GitCommand::Commit` | `CommitChanges` | `wip` → `is_wip` |
| `GitCommand::Stash` | `StashChanges` | — |
| `GitCommand::Discard` | `DiscardChanges` | — |
| `GitCommand::CreateBranch` | `CreateTaskBranch` | `name` → `branch_name`, `base` → `base_branch` |
| `GitCommand::Rebase` | `RebaseToMain` | `base` → `base_branch` |
| `HumanCommand::Escalate` | `EscalateToHuman` | — |
| `TaskCommand::PrepareStart` | `PrepareTaskStart` | — |

All other variants use the same name in both Rust and YAML (e.g., `ApproveAndContinue`, `Reflect`, `RetryTool`, `SwitchProvider`, `SelectOption`, `ConfirmCompletion`, etc.).

Git commands carry an optional `worktree_path` in the outer `DecisionCommand::Git(_, Option<String>)` tuple. When serialized to YAML, `worktree_path` appears as a field within each Git command object.

---

## 2. Blackboard Design

The Blackboard is the shared memory of the behavior tree. It uses **scoped layers** so that SubTree execution does not pollute the parent's variable namespace.

### 2.1 Data Model

```rust
#[derive(Default)]
pub struct Blackboard {
    // --- Built-in input variables (strongly typed) ---
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

    // --- Scoped custom variables ---
    scopes: Vec<HashMap<String, BlackboardValue>>,

    // --- Outputs ---
    pub commands: Vec<DecisionCommand>,
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
    pub command: DecisionCommand,
    pub timestamp: String,
}
```

### 2.2 Scoped Access

```rust
impl Blackboard {
    pub fn new() -> Self {
        Self {
            task_description: String::new(),
            provider_output: String::new(),
            context_summary: String::new(),
            reflection_round: 0,
            max_reflection_rounds: 2,
            confidence_accumulator: 0.0,
            agent_id: String::new(),
            current_task_id: String::new(),
            current_story_id: String::new(),
            last_tool_call: None,
            file_changes: Vec::new(),
            project_rules: ProjectRules::default(),
            decision_history: Vec::new(),
            scopes: vec![HashMap::new()],  // Root scope
            commands: Vec::new(),
            llm_responses: HashMap::new(),
        }
    }

    /// Start a new scope (e.g., when entering a SubTree).
    pub fn push_scope(&mut self) {
        self.scopes.push(HashMap::new());
    }

    /// End current scope, discarding all variables set within it.
    pub fn pop_scope(&mut self) {
        if self.scopes.len() > 1 {
            self.scopes.pop();
        }
    }

    /// Set a variable in the innermost scope.
    pub fn set(&mut self, key: &str, value: BlackboardValue) {
        if let Some(scope) = self.scopes.last_mut() {
            scope.insert(key.to_string(), value);
        }
    }

    /// Get a variable, searching scopes from innermost to outermost.
    pub fn get(&self, key: &str) -> Option<&BlackboardValue> {
        for scope in self.scopes.iter().rev() {
            if let Some(v) = scope.get(key) {
                return Some(v);
            }
        }
        None
    }

    /// Get a value by dot-notation path.
    /// Built-in fields take priority over scoped variables.
    pub fn get_path(&self, path: &str) -> Option<BlackboardValue> {
        let mut parts = path.split('.');
        let first = parts.next()?;

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
            "llm_responses" => Some(BlackboardValue::Map(
                self.llm_responses.iter().map(|(k, v)| {
                    (k.clone(), BlackboardValue::String(v.clone()))
                }).collect()
            )),
            // Scoped variable lookup
            _ => self.get(first).cloned(),
        };

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

    // --- Typed convenience getters ---

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

    // --- Typed setters ---

    pub fn set_string(&mut self, key: &str, value: String) {
        self.set(key, BlackboardValue::String(value));
    }

    pub fn set_u8(&mut self, key: &str, value: u8) {
        self.set(key, BlackboardValue::Integer(value as i64));
    }

    pub fn set_f64(&mut self, key: &str, value: f64) {
        self.set(key, BlackboardValue::Float(value));
    }

    pub fn set_bool(&mut self, key: &str, value: bool) {
        self.set(key, BlackboardValue::Boolean(value));
    }

    // --- Command management ---

    pub(crate) fn push_command(&mut self, cmd: DecisionCommand) {
        self.commands.push(cmd);
    }

    pub(crate) fn drain_commands(&mut self) -> Vec<DecisionCommand> {
        std::mem::take(&mut self.commands)
    }

    pub(crate) fn store_llm_response(&mut self, node_name: &str, response: String) {
        self.llm_responses.insert(node_name.to_string(), response);
    }
}
```

---

## 3. Template Rendering

Template rendering uses the `minijinja` crate. The Blackboard's built-in fields and scoped variables are exposed as template context.

```rust
use minijinja::Value;
use std::collections::HashMap;

impl Blackboard {
    /// Build a minijinja context from the current Blackboard state.
    pub fn to_template_context(&self) -> Value {
        let mut ctx: HashMap<String, Value> = HashMap::new();
        ctx.insert("task_description".into(), Value::from_serialize(&self.task_description));
        ctx.insert("provider_output".into(), Value::from_serialize(&self.provider_output));
        ctx.insert("context_summary".into(), Value::from_serialize(&self.context_summary));
        ctx.insert("reflection_round".into(), Value::from(self.reflection_round));
        ctx.insert("max_reflection_rounds".into(), Value::from(self.max_reflection_rounds));
        ctx.insert("confidence_accumulator".into(), Value::from(self.confidence_accumulator));
        ctx.insert("agent_id".into(), Value::from_serialize(&self.agent_id));
        ctx.insert("current_task_id".into(), Value::from_serialize(&self.current_task_id));
        ctx.insert("current_story_id".into(), Value::from_serialize(&self.current_story_id));

        // Expose last_tool_call as a nested object
        if let Some(ref tc) = self.last_tool_call {
            let mut tc_map = HashMap::new();
            tc_map.insert("name".into(), Value::from_serialize(&tc.name));
            tc_map.insert("input".into(), Value::from_serialize(&tc.input));
            tc_map.insert("output".into(), Value::from_serialize(&tc.output));
            ctx.insert("last_tool_call".into(), Value::from_serialize(tc_map));
        }

        // Expose file_changes as a list
        let file_changes: Vec<HashMap<String, Value>> = self.file_changes.iter().map(|fc| {
            let mut m = HashMap::new();
            m.insert("path".into(), Value::from_serialize(&fc.path));
            m.insert("change_type".into(), Value::from_serialize(&fc.change_type));
            m
        }).collect();
        ctx.insert("file_changes".into(), Value::from_serialize(file_changes));

        // Expose scoped variables at root level (innermost scope wins)
        for scope in &self.scopes {
            for (k, v) in scope {
                ctx.insert(k.clone(), blackboard_value_to_minijinja(v));
            }
        }

        Value::from_serialize(ctx)
    }
}

fn blackboard_value_to_minijinja(v: &BlackboardValue) -> Value {
    match v {
        BlackboardValue::String(s) => Value::from_serialize(s),
        BlackboardValue::Integer(i) => Value::from(*i),
        BlackboardValue::Float(f) => Value::from(*f),
        BlackboardValue::Boolean(b) => Value::from(*b),
        BlackboardValue::List(l) => Value::from_serialize(
            l.iter().map(blackboard_value_to_minijinja).collect::<Vec<_>>()
        ),
        BlackboardValue::Map(m) => Value::from_serialize(
            m.iter().map(|(k, v)| (k.clone(), blackboard_value_to_minijinja(v))).collect::<HashMap<_, _>>()
        ),
    }
}
```

---

*Document version: 2.0*
*Last updated: 2026-04-24*
