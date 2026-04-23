# Decision Layer Architecture Design

> This document describes the decision-layer architecture based on **Behavior Trees** — a hierarchical, composable decision-making pattern adopted from game AI. The decision layer treats LLM prompts as first-class nodes in the tree, enabling complex, debuggable, and reusable decision logic.

---

## 1. Design Philosophy

The decision layer is an **embedded AI** inside the agent runtime. Its job is to turn ambiguous LLM outputs into explicit, auditable, and reversible commands — just as a game AI decides what an NPC should do next based on the game state.

Guiding principles:

1. **Decision as data** — A decision is a value (`DecisionCommand`), not a side effect. The layer is read-only; execution is delegated to the runtime.
2. **Fail closed** — When uncertain, escalate to human. Never guess silently.
3. **Tree-driven logic** — All decision logic is expressed as a behavior tree. No hard-coded `if-else` chains, no monolithic engines.
4. **Prompt as a node** — Calling an LLM is not a special engine; it is a `Prompt` node in the tree, indistinguishable from `Condition` or `Action` nodes in structure.
5. **Observability by design** — Every node execution leaves a trace: which node ran, what it read from the Blackboard, what it wrote, what status it returned.

---

## 2. Why Behavior Trees?

### 2.1 The Problem with the Current Design

The current architecture uses a **TieredDecisionEngine** that internally branches:

```text
TieredDecisionEngine::decide()
  ├── type_name == "error" ?
  │     ├── rate limit? → Simple tier → RuleBasedEngine
  │     └── else → Complex tier → LLMEngine
  ├── type_name == "claims_completion" ? → Medium tier → LLMEngine
  ├── type_name == "waiting_for_choice" ? → Simple tier → RuleBasedEngine
  └── ...
```

Problems:
- **Hard-coded priorities**: The order of `if` statements *is* the priority order. Changing it requires editing the engine.
- **No composability**: You cannot reuse the "rate limit check" logic in another decision path without copy-paste.
- **Opaque execution**: When a decision is wrong, you cannot tell which `if` branch was taken without adding ad-hoc logging.
- **LLM is special**: The LLM engine is a monolithic black box. You cannot interleave LLM calls with condition checks or action emissions.

### 2.2 Behavior Trees Solve These Problems

A behavior tree is a **directed acyclic graph** where:
- **Internal nodes** control execution flow (Selector, Sequence, Parallel, Decorator).
- **Leaf nodes** perform work (Condition check, Action execution, Prompt rendering).
- All nodes share a **Blackboard** — a key-value store for state.

**Advantages over the current design**:

| Aspect | Tiered Engine | Behavior Tree |
|--------|---------------|---------------|
| **Priority** | Hard-coded `if` order | Selector node orders children explicitly |
| **Reusability** | Copy-paste logic | Sub-trees are composable units |
| **Debugging** | Ad-hoc logging | Every tick traces the exact path through the tree |
| **LLM integration** | Monolithic engine | Prompt nodes are first-class leaves |
| **Visualization** | None | Trees map directly to diagrams |
| **Extensibility** | Edit engine code | Add nodes without changing the executor |

### 2.3 Why Not Finite State Machines?

Finite State Machines (FSM) are the other common AI pattern. FSMs model behavior as states and transitions:

```text
[Idle] --task_received--> [Working] --claims_completion--> [Reflecting]
                                            |
                                            v
                                    [Confirming] --done--> [Idle]
```

FSMs suffer from **state explosion**: as you add more situations (error, rate limit, partial completion, human escalation), the number of states and transitions grows quadratically. Adding a new state requires reconsidering *all* transitions.

Behavior trees avoid this by being **hierarchical**: a sub-tree handles a concern locally. The parent tree does not need to know the internal structure of the sub-tree.

### 2.4 Why Not Just a Rule Engine?

Rule engines (like the current `RuleBasedDecisionEngine`) are good for flat condition→action mappings:

```text
IF situation == "claims_completion" AND reflection_round < 2 THEN reflect
```

They do not handle:
- **Sequential logic**: "Check budget, then check confidence, then ask LLM."
- **Priority naturally**: Rules have salience/priority, but the engine must resolve conflicts.
- **Stateful loops**: "Reflect up to 2 times, then confirm." Requires external state tracking.

Behavior trees subsume rule engines: a `Sequence` of `Condition` + `Action` nodes is exactly a rule.

---

## 3. Core Concepts

### 3.1 The Big Picture

```text
┌─────────────────────────────────────────────────────────────────────────────┐
│                    Behavior Tree Decision Layer                              │
│                                                                              │
│  ┌─────────────┐    ┌─────────────┐    ┌─────────────┐                     │
│  │ Task Desc   │    │ LLM Summary │    │ LLM Output  │                     │
│  └──────┬──────┘    └──────┬──────┘    └──────┬──────┘                     │
│         │                  │                  │                             │
│         └──────────────────┼──────────────────┘                             │
│                            ▼                                                 │
│                   ┌─────────────────┐                                       │
│                   │   BLACKBOARD    │  ← Shared state all nodes read/write │
│                   └────────┬────────┘                                       │
│                            │                                                 │
│                            ▼                                                 │
│                   ┌─────────────────┐                                       │
│                   │  ROOT NODE      │  ← Selector / Sequence / Parallel      │
│                   │  (BehaviorTree) │                                       │
│                   └────────┬────────┘                                       │
│                            │ tick()                                         │
│                            ▼                                                 │
│                   ┌─────────────────┐                                       │
│                   │  NODE STATUS    │  ← Success / Failure / Running         │
│                   └────────┬────────┘                                       │
│                            │                                                 │
│                            ▼                                                 │
│                   ┌─────────────────┐                                       │
│                   │  COMMANDS[]     │  ← Vec<DecisionCommand>                │
│                   └─────────────────┘                                       │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

### 3.2 Blackboard

The Blackboard is the **shared memory** of the behavior tree. All nodes read from it and write to it. It is the single source of truth for the current decision cycle.

```rust
/// The Blackboard is the shared state for a single decision cycle.
pub struct Blackboard {
    /// Input: the task description given to the work agent
    pub task_description: String,

    /// Input: the raw output from the LLM provider (Claude/Codex)
    pub provider_output: String,

    /// Input: a condensed summary of the LLM's recent work (tool calls, file changes)
    pub context_summary: String,

    /// State: current reflection round (for claims_completion loops)
    pub reflection_round: u8,

    /// State: maximum allowed reflection rounds
    pub max_reflection_rounds: u8,

    /// State: accumulated confidence across decisions
    pub confidence_accumulator: f64,

    /// State: the last tool call made by the work agent
    pub last_tool_call: Option<ToolCallRecord>,

    /// State: recent file changes
    pub file_changes: Vec<FileChangeRecord>,

    /// State: project rules from CLAUDE.md / AGENTS.md
    pub project_rules: ProjectRules,

    /// State: decision history for this session
    pub decision_history: Vec<DecisionRecord>,

    /// Output: raw LLM responses from Prompt nodes (node_name → response)
    pub llm_responses: HashMap<String, String>,

    /// Output: accumulated DecisionCommands
    pub commands: Vec<DecisionCommand>,

    /// Custom: ad-hoc variables set by nodes
    pub variables: HashMap<String, BlackboardValue>,
}

pub enum BlackboardValue {
    String(String),
    Integer(i64),
    Float(f64),
    Boolean(bool),
    List(Vec<BlackboardValue>),
}
```

**Key design point**: The Blackboard is **rebuilt every decision cycle** from the agent's current state. It is not persisted. Persistent state (like `reflection_round`) lives in the agent's state store and is copied into the Blackboard at the start of each tick.

### 3.3 Node Status

Every node returns a status when ticked:

```rust
pub enum NodeStatus {
    /// The node succeeded in its goal.
    Success,
    /// The node failed to achieve its goal.
    Failure,
    /// The node is still working (async operation in progress).
    /// The executor will re-tick this node on the next cycle.
    Running,
}
```

**Running** is critical for the decision layer: a `Prompt` node that calls an LLM returns `Running` until the LLM response arrives. The executor does not block; it returns `Running` up the tree, and the caller re-ticks on the next decision poll interval.

### 3.4 The Tick

```rust
pub trait BehaviorNode: Send + Sync {
    /// Execute one step of this node.
    fn tick(&mut self, blackboard: &mut Blackboard) -> NodeStatus;

    /// Reset the node to its initial state.
    /// Called when the tree is re-entered (new decision cycle).
    fn reset(&mut self);

    /// Human-readable name for debugging.
    fn name(&self) -> &str;
}
```

The **tick** is the atomic unit of execution:
1. The executor calls `tick()` on the root node.
2. The root node propagates the tick to its children according to its logic.
3. Leaf nodes perform their work and return a status.
4. The status propagates back up the tree.
5. If any node returns `Running`, the executor stores the "running node path" and resumes from there on the next tick.

**Reset semantics**: When a new decision cycle starts (new provider output arrives), the entire tree is `reset()`. This clears internal state (like "how many times have I retried") but does not clear the Blackboard — the Blackboard is rebuilt fresh by the caller.

---

## 4. Node Type Reference

### 4.1 Composite Nodes

Composite nodes have children and control how they are executed.

#### Selector

```rust
pub struct Selector {
    pub name: String,
    pub children: Vec<Box<dyn BehaviorNode>>,
    pub active_child: Option<usize>, // for Running resume
}

impl BehaviorNode for Selector {
    fn tick(&mut self, blackboard: &mut Blackboard) -> NodeStatus {
        let start = self.active_child.unwrap_or(0);
        for i in start..self.children.len() {
            let status = self.children[i].tick(blackboard);
            match status {
                NodeStatus::Success => {
                    self.active_child = None;
                    return NodeStatus::Success;
                }
                NodeStatus::Running => {
                    self.active_child = Some(i);
                    return NodeStatus::Running;
                }
                NodeStatus::Failure => {
                    // Try next child
                    continue;
                }
            }
        }
        self.active_child = None;
        NodeStatus::Failure
    }

    fn reset(&mut self) {
        self.active_child = None;
        for child in &mut self.children {
            child.reset();
        }
    }
}
```

**Behavior**: Execute children left-to-right. Return `Success` on the first child that succeeds. Return `Failure` only if **all** children fail.

**Use case**: Priority decision. The leftmost child is the highest priority. Example: try rate-limit handler → try human escalation → try reflect → default to continue.

#### Sequence

```rust
pub struct Sequence {
    pub name: String,
    pub children: Vec<Box<dyn BehaviorNode>>,
    pub active_child: Option<usize>,
}

impl BehaviorNode for Sequence {
    fn tick(&mut self, blackboard: &mut Blackboard) -> NodeStatus {
        let start = self.active_child.unwrap_or(0);
        for i in start..self.children.len() {
            let status = self.children[i].tick(blackboard);
            match status {
                NodeStatus::Success => {
                    // Continue to next child
                    continue;
                }
                NodeStatus::Running => {
                    self.active_child = Some(i);
                    return NodeStatus::Running;
                }
                NodeStatus::Failure => {
                    self.active_child = None;
                    return NodeStatus::Failure;
                }
            }
        }
        self.active_child = None;
        NodeStatus::Success
    }
}
```

**Behavior**: Execute children left-to-right. Return `Failure` on the first child that fails. Return `Success` only if **all** children succeed.

**Use case**: Guard + action chains. Example: check situation → check budget → execute prompt → emit command.

#### Parallel

```rust
pub enum ParallelPolicy {
    /// Return Success if ALL children succeed.
    AllSuccess,
    /// Return Success if ANY child succeeds.
    AnySuccess,
    /// Return Success if majority succeeds.
    Majority,
}

pub struct Parallel {
    pub name: String,
    pub children: Vec<Box<dyn BehaviorNode>>,
    pub policy: ParallelPolicy,
}
```

**Behavior**: Execute all children. Aggregate results based on policy.

**Use case**: Concurrent safety checks. Example: check "is dangerous action" AND "is main branch" in parallel.

### 4.2 Decorator Nodes

Decorators have exactly one child and modify its behavior.

#### Inverter

```rust
pub struct Inverter {
    pub name: String,
    pub child: Box<dyn BehaviorNode>,
}

impl BehaviorNode for Inverter {
    fn tick(&mut self, blackboard: &mut Blackboard) -> NodeStatus {
        match self.child.tick(blackboard) {
            NodeStatus::Success => NodeStatus::Failure,
            NodeStatus::Failure => NodeStatus::Success,
            NodeStatus::Running => NodeStatus::Running,
        }
    }
}
```

**Use case**: "If NOT rate limit, then ..."

#### Repeater

```rust
pub struct Repeater {
    pub name: String,
    pub child: Box<dyn BehaviorNode>,
    pub max_attempts: u32,
    pub current: u32,
}

impl BehaviorNode for Repeater {
    fn tick(&mut self, blackboard: &mut Blackboard) -> NodeStatus {
        while self.current < self.max_attempts {
            match self.child.tick(blackboard) {
                NodeStatus::Success => {
                    self.current += 1;
                    if self.current >= self.max_attempts {
                        return NodeStatus::Success;
                    }
                    // Continue looping
                }
                NodeStatus::Failure => return NodeStatus::Failure,
                NodeStatus::Running => return NodeStatus::Running,
            }
        }
        NodeStatus::Success
    }
}
```

**Use case**: Retry logic. "Retry the LLM call up to 3 times."

#### Cooldown

```rust
pub struct Cooldown {
    pub name: String,
    pub child: Box<dyn BehaviorNode>,
    pub duration: Duration,
    pub last_success: Option<Instant>,
}

impl BehaviorNode for Cooldown {
    fn tick(&mut self, blackboard: &mut Blackboard) -> NodeStatus {
        if let Some(last) = self.last_success {
            if last.elapsed() < self.duration {
                return NodeStatus::Failure; // Still cooling down
            }
        }
        let status = self.child.tick(blackboard);
        if status == NodeStatus::Success {
            self.last_success = Some(Instant::now());
        }
        status
    }
}
```

**Use case**: Prevent spam. "Do not retry more than once every 5 seconds."

#### ReflectionGuard

```rust
pub struct ReflectionGuard {
    pub name: String,
    pub child: Box<dyn BehaviorNode>,
    pub max_rounds: u8,
}

impl BehaviorNode for ReflectionGuard {
    fn tick(&mut self, blackboard: &mut Blackboard) -> NodeStatus {
        if blackboard.reflection_round >= self.max_rounds {
            return NodeStatus::Failure;
        }
        let status = self.child.tick(blackboard);
        if status == NodeStatus::Success {
            blackboard.reflection_round += 1;
        }
        status
    }
}
```

**Use case**: The reflection round limit. "Allow reflection only up to N times."

#### ForceHuman

```rust
pub struct ForceHuman {
    pub name: String,
    pub child: Box<dyn BehaviorNode>,
    pub reason: String,
}

impl BehaviorNode for ForceHuman {
    fn tick(&mut self, blackboard: &mut Blackboard) -> NodeStatus {
        let status = self.child.tick(blackboard);
        if status == NodeStatus::Success {
            blackboard.commands.push(DecisionCommand::EscalateToHuman {
                reason: self.reason.clone(),
                context: Some(format!("Forced by decorator after {} succeeded", self.child.name())),
            });
        }
        status
    }
}
```

**Use case**: Override any sub-tree to require human confirmation.

### 4.3 Leaf Nodes

Leaf nodes perform actual work. They have no children.

#### Condition

```rust
pub struct Condition {
    pub name: String,
    pub evaluator: Box<dyn ConditionEvaluator>,
}

pub trait ConditionEvaluator: Send + Sync {
    fn evaluate(&self, blackboard: &Blackboard) -> bool;
}

impl BehaviorNode for Condition {
    fn tick(&mut self, blackboard: &mut Blackboard) -> NodeStatus {
        if self.evaluator.evaluate(blackboard) {
            NodeStatus::Success
        } else {
            NodeStatus::Failure
        }
    }
}
```

**Use case**: Any boolean check against the Blackboard.

**Built-in evaluators**:

```rust
/// Check if the situation matches a type.
pub struct SituationIs {
    pub situation_type: String,
}
impl ConditionEvaluator for SituationIs {
    fn evaluate(&self, blackboard: &Blackboard) -> bool {
        blackboard.provider_output.contains(&self.situation_type)
            || blackboard.context_summary.contains(&self.situation_type)
    }
}

/// Check if provider output contains a pattern.
pub struct OutputContains {
    pub pattern: String,
}
impl ConditionEvaluator for OutputContains {
    fn evaluate(&self, blackboard: &Blackboard) -> bool {
        blackboard.provider_output.to_lowercase().contains(&self.pattern.to_lowercase())
    }
}

/// Check reflection round against a limit.
pub struct ReflectionRoundUnder {
    pub max: u8,
}
impl ConditionEvaluator for ReflectionRoundUnder {
    fn evaluate(&self, blackboard: &Blackboard) -> bool {
        blackboard.reflection_round < self.max
    }
}

/// Scripted condition (rhai or custom expression).
pub struct ScriptCondition {
    pub script: String,
}
impl ConditionEvaluator for ScriptCondition {
    fn evaluate(&self, blackboard: &Blackboard) -> bool {
        // Evaluate script against blackboard variables
        // ...
        true
    }
}
```

#### Action

```rust
pub struct Action {
    pub name: String,
    pub command: DecisionCommand,
}

impl BehaviorNode for Action {
    fn tick(&mut self, blackboard: &mut Blackboard) -> NodeStatus {
        blackboard.commands.push(self.command.clone());
        NodeStatus::Success
    }
}
```

**Use case**: Emit a `DecisionCommand` unconditionally.

#### SetVar

```rust
pub struct SetVar {
    pub name: String,
    pub key: String,
    pub value: BlackboardValue,
}

impl BehaviorNode for SetVar {
    fn tick(&mut self, blackboard: &mut Blackboard) -> NodeStatus {
        blackboard.variables.insert(self.key.clone(), self.value.clone());
        NodeStatus::Success
    }
}
```

**Use case**: Initialize variables before a Prompt node runs.

---

## 5. The Prompt Node

The `Prompt` node is the **most important node type** in the decision layer. It is what makes the behavior tree approach powerful for LLM-based agents.

### 5.1 What is a Prompt Node?

A Prompt node is a Leaf node that:
1. **Reads** variables from the Blackboard.
2. **Renders** a prompt template (Jinja2/Tera style).
3. **Sends** the rendered template as the next message in the **ongoing codex/claude session** (not a separate API call).
4. **Waits** for the LLM's reply within that same session.
5. **Parses** the LLM's reply.
6. **Writes** parsed values back to the Blackboard.
7. **Returns** `Success` or `Failure` based on whether parsing succeeded.

```rust
pub struct Prompt {
    pub name: String,
    pub template: String,
    pub model: ModelTier,
    pub parser: Box<dyn OutputParser>,
    pub sets: Vec<SetMapping>,
    pub timeout: Duration,
}

pub struct SetMapping {
    /// Blackboard key to write to
    pub key: String,
    /// Parser field to read from
    pub field: String,
}

impl BehaviorNode for Prompt {
    fn tick(&mut self, blackboard: &mut Blackboard) -> NodeStatus {
        // 1. Render template
        let prompt = render_template(&self.template, blackboard);

        // 2. Send prompt as next message in current session (async → returns Running on first tick)
        match self.send_to_session(&prompt, blackboard) {
            LlmCallState::Pending => return NodeStatus::Running,
            LlmCallState::Completed(response) => {
                // 3. Parse response
                match self.parser.parse(&response) {
                    Ok(parsed) => {
                        // 4. Write to blackboard
                        for mapping in &self.sets {
                            if let Some(value) = parsed.get(&mapping.field) {
                                blackboard.variables.insert(
                                    mapping.key.clone(),
                                    value.clone(),
                                );
                            }
                        }
                        // Store raw response for debugging
                        blackboard.llm_responses.insert(self.name.clone(), response);
                        NodeStatus::Success
                    }
                    Err(e) => {
                        tracing::warn!(node = %self.name, error = %e, "Prompt parse failed");
                        NodeStatus::Failure
                    }
                }
            }
            LlmCallState::Failed(e) => {
                tracing::warn!(node = %self.name, error = %e, "LLM call failed");
                NodeStatus::Failure
            }
        }
    }
}
```

### 5.2 Template Rendering

Templates use Jinja2-style syntax with the Blackboard as the context:

```text
You are a decision helper for a software development agent.

## Task
{{ task_description }}

## Recent Work Summary
{{ context_summary }}

## Current LLM Output
{{ provider_output }}

## Reflection Round
{{ reflection_round }} / {{ max_reflection_rounds }}

## Question
Should the agent reflect on its work, confirm completion, or escalate to human?
Reply with exactly one word: REFLECT, CONFIRM, or ESCALATE.
```

**Available variables** (all Blackboard fields):
- `task_description`
- `provider_output`
- `context_summary`
- `reflection_round`, `max_reflection_rounds`
- `confidence_accumulator`
- `last_tool_call` (as structured object)
- `file_changes` (as list)
- `project_rules` (as structured object)
- `decision_history` (as list)
- `variables.*` (custom variables)

### 5.3 Output Parsers

The parser converts raw LLM text into structured Blackboard values.

```rust
pub trait OutputParser: Send + Sync {
    fn parse(&self, raw: &str) -> Result<HashMap<String, BlackboardValue>, ParseError>;
}
```

#### Enum Parser

```rust
pub struct EnumParser {
    pub allowed_values: Vec<String>,
    pub case_sensitive: bool,
}

impl OutputParser for EnumParser {
    fn parse(&self, raw: &str) -> Result<HashMap<String, BlackboardValue>, ParseError> {
        let trimmed = raw.trim();
        for value in &self.allowed_values {
            let matches = if self.case_sensitive {
                trimmed == value
            } else {
                trimmed.eq_ignore_ascii_case(value)
            };
            if matches {
                let mut result = HashMap::new();
                result.insert("decision".to_string(), BlackboardValue::String(value.clone()));
                return Ok(result);
            }
        }
        Err(ParseError::UnexpectedValue {
            got: trimmed.to_string(),
            expected: self.allowed_values.clone(),
        })
    }
}
```

**Example**: Parse `REFLECT`, `CONFIRM`, or `ESCALATE` from LLM output.

#### Structured Parser

```rust
pub struct StructuredParser {
    pub pattern: regex::Regex,
    pub fields: Vec<(String, usize)>, // (field_name, capture_group_index)
}

impl OutputParser for StructuredParser {
    fn parse(&self, raw: &str) -> Result<HashMap<String, BlackboardValue>, ParseError> {
        let caps = self.pattern.captures(raw)
            .ok_or(ParseError::NoMatch)?;
        let mut result = HashMap::new();
        for (field_name, group_index) in &self.fields {
            if let Some(m) = caps.get(*group_index) {
                result.insert(field_name.clone(), BlackboardValue::String(m.as_str().to_string()));
            }
        }
        Ok(result)
    }
}
```

**Example**: Parse `ACTION: reflect` and `CONFIDENCE: 0.85` using regex.

#### JSON Parser

```rust
pub struct JsonParser {
    pub schema: JsonSchema, // Optional validation
}

impl OutputParser for JsonParser {
    fn parse(&self, raw: &str) -> Result<HashMap<String, BlackboardValue>, ParseError> {
        let json: serde_json::Value = serde_json::from_str(raw)?;
        // Convert JSON object to BlackboardValue map
        let mut result = HashMap::new();
        if let serde_json::Value::Object(map) = json {
            for (k, v) in map {
                result.insert(k, json_to_blackboard(v));
            }
        }
        Ok(result)
    }
}
```

**Example**: Parse JSON responses from reasoning models.

### 5.4 Prompt Node Best Practices

1. **Be explicit about output format**: The template should tell the LLM exactly what to return. One word? JSON? Markdown?
2. **Use Enum parser when possible**: Constrained outputs are easier to parse and less error-prone.
3. **Handle parse failures**: If parsing fails, the Prompt node returns `Failure`. The parent Selector will try the next branch.
4. **Store raw responses**: Always store the raw LLM response in `blackboard.llm_responses` for debugging.
5. **Set timeouts**: Prompt nodes should have configurable timeouts. On timeout, return `Failure`.
6. **Model selection**: Use cheaper/faster models for simple prompts, reasoning models for complex analysis.

---

## 6. Execution Model

### 6.1 The Executor

```rust
pub struct BehaviorTreeExecutor {
    pub root: Box<dyn BehaviorNode>,
    pub running_path: Vec<usize>, // Path to the currently Running node
}

impl BehaviorTreeExecutor {
    pub fn new(root: Box<dyn BehaviorNode>) -> Self {
        Self { root, running_path: Vec::new() }
    }

    /// Tick the tree. Returns the final status and any commands produced.
    pub fn tick(&mut self, blackboard: &mut Blackboard) -> (NodeStatus, Vec<DecisionCommand>) {
        let status = self.root.tick(blackboard);
        let commands = std::mem::take(&mut blackboard.commands);
        (status, commands)
    }

    /// Reset the tree for a new decision cycle.
    pub fn reset(&mut self) {
        self.root.reset();
        self.running_path.clear();
    }
}
```

### 6.2 Synchronous vs Asynchronous Execution

**Synchronous** (for rule-based decisions):
```rust
// All nodes return Success or Failure immediately.
let (status, commands) = executor.tick(&mut blackboard);
assert!(status != NodeStatus::Running);
```

**Asynchronous** (for LLM-based decisions):
```rust
// First tick starts the LLM call and returns Running.
let (status, commands) = executor.tick(&mut blackboard);
assert_eq!(status, NodeStatus::Running);
assert!(commands.is_empty()); // No commands yet

// ... poll on next decision cycle ...

// Second tick resumes the Prompt node. LLM has responded.
let (status, commands) = executor.tick(&mut blackboard);
assert_eq!(status, NodeStatus::Success);
assert!(!commands.is_empty());
```

**The async model**: The executor does not block. When a `Prompt` node returns `Running`, the executor stores the running path. The caller (e.g. `decision_agent_slot.rs`) polls the executor periodically. On each poll, the executor re-ticks from the running node.

### 6.3 Decision Cycle Lifecycle

```text
1. OBSERVE
   └── Collect task_description, provider_output, context_summary
       from the work agent's state.

2. BUILD BLACKBOARD
   └── Populate Blackboard with inputs and persistent state
       (reflection_round, decision_history, etc.).

3. RESET
   └── Call executor.reset() to clear internal node state.

4. TICK
   └── Call executor.tick(&mut blackboard).
       └── If Running: store executor, return empty commands.
           Poll again later.
       └── If Success/Failure: collect commands from blackboard.

5. PERSIST
   └── Write changed state (reflection_round, etc.) back to
       the agent's persistent state store.

6. RETURN
   └── Return Vec<DecisionCommand> to the runtime.
```

### 6.4 Sub-Tree Reuse

Sub-trees are first-class units. They can be defined once and referenced multiple times:

```rust
/// A named sub-tree that can be referenced by name.
pub struct SubTree {
    pub name: String,
    pub root: Box<dyn BehaviorNode>,
}

pub struct SubTreeNode {
    pub name: String,
    pub sub_tree_name: String,
    pub registry: Arc<SubTreeRegistry>,
}

impl BehaviorNode for SubTreeNode {
    fn tick(&mut self, blackboard: &mut Blackboard) -> NodeStatus {
        let sub_tree = self.registry.get(&self.sub_tree_name)?;
        sub_tree.root.tick(blackboard)
    }
}
```

**Example sub-trees**:
- `check_budget`: Sequence of condition checks for token budget.
- `human_escalation`: Action node that emits `EscalateToHuman`.
- `reflect_prompt`: Prompt node that asks the LLM whether to reflect.

---

## 7. Building Decision Trees: Complete Examples

### 7.1 Example 1: The Default Decision Tree

This is the "root" tree that handles all common situations. It uses a Selector to try handlers in priority order.

```text
[ROOT: Selector "root_handler"]
│
├── [Sequence "rate_limit_handler"]
│   ├── [Condition "is_rate_limit"]
│   │       OutputContains { pattern: "429" }
│   └── [Action "retry_with_backoff"]
│           RetryTool { tool_name: "{{last_tool}}", cooldown_ms: 5000 }
│
├── [Sequence "human_escalation"]
│   ├── [Condition "is_dangerous_action"]
│   │       ScriptCondition { script: "is_dangerous(provider_output)" }
│   └── [Action "escalate"]
│           EscalateToHuman { reason: "Dangerous action detected" }
│
├── [Sequence "reflect_loop"]
│   ├── [Condition "is_claims_completion"]
│   │       OutputContains { pattern: "claims_completion" }
│   ├── [ReflectionGuard "guard" max=2]
│   │   └── [Prompt "reflect_or_confirm"]
│   │           template: "..."
│   │           parser: EnumParser ["REFLECT", "CONFIRM"]
│   │           sets: [("next_action", "decision")]
│   └── [Selector "branch_on_decision"]
│       ├── [Sequence "do_reflect"]
│       │   ├── [Condition "decision_is_reflect"]
│       │   │       VariableIs { key: "next_action", value: "REFLECT" }
│       │   └── [Action "emit_reflect"]
│       │           Reflect { prompt: "Review your work carefully" }
│       └── [Sequence "do_confirm"]
│           ├── [Condition "decision_is_confirm"]
│           │       VariableIs { key: "next_action", value: "CONFIRM" }
│           └── [Action "emit_confirm"]
│                   ConfirmCompletion
│
├── [Sequence "error_recovery"]
│   ├── [Condition "is_error"]
│   │       OutputContains { pattern: "error" }
│   └── [Prompt "error_strategy"]
│           template: "..."
│           parser: EnumParser ["RETRY", "ESCALATE", "CONTINUE"]
│           sets: [("error_strategy", "decision")]
│   └── [Selector "branch_on_strategy"]
│       ├── [Sequence "retry"]
│       │   ├── [VariableIs { key: "error_strategy", value: "RETRY" }]
│       │   └── [Action "emit_retry"]
│       │           RetryTool { ... }
│       ├── [Sequence "escalate"]
│       │   ├── [VariableIs { key: "error_strategy", value: "ESCALATE" }]
│       │   └── [Action "emit_escalate"]
│       │           EscalateToHuman { ... }
│       └── [Sequence "continue"]
│           ├── [VariableIs { key: "error_strategy", value: "CONTINUE" }]
│           └── [Action "emit_continue"]
│                   ApproveAndContinue
│
└── [Action "default_continue"]
        ApproveAndContinue
```

**Trace for a claims_completion scenario**:
1. `root_handler` (Selector) ticks child 0: `rate_limit_handler`.
   - `is_rate_limit` checks output → does not contain "429" → returns Failure.
   - Sequence returns Failure. Selector tries child 1.
2. `human_escalation`: `is_dangerous_action` → false → Failure. Selector tries child 2.
3. `reflect_loop`:
   - `is_claims_completion` → true → Success.
   - `ReflectionGuard` checks `reflection_round` (0 < 2) → Success.
   - `reflect_or_confirm` (Prompt) renders template, calls LLM.
     - LLM returns "REFLECT". EnumParser parses successfully.
     - Sets `blackboard.variables["next_action"] = "REFLECT"`.
     - Returns Success.
   - `branch_on_decision` (Selector) ticks child 0: `do_reflect`.
     - `decision_is_reflect` checks variable → true → Success.
     - `emit_reflect` pushes `DecisionCommand::Reflect` → Success.
     - Selector returns Success.
   - Sequence returns Success.
4. `root_handler` (Selector) received Success from child 2 → returns Success.
5. Executor collects commands: `[Reflect { prompt: "..." }]`.
6. `reflection_round` incremented to 1 by ReflectionGuard.

### 7.2 Example 2: Reflect Loop (Third Round)

Same tree, but `reflection_round` is now 2 (max reached).

1. `root_handler` → `reflect_loop`.
2. `is_claims_completion` → Success.
3. `ReflectionGuard` checks `reflection_round` (2 >= 2) → returns **Failure**.
4. Sequence `reflect_loop` returns Failure.
5. Selector tries child 3: `error_recovery` → `is_error` → Failure.
6. Selector tries child 4: `default_continue` → pushes `ApproveAndContinue` → Success.
7. Root returns Success.

**Result**: When max reflections are reached, the tree falls through to the default action. No infinite loop.

### 7.3 Example 3: Rate Limit Handling

```text
[ROOT: Selector]
├── [Sequence "rate_limit_handler"]
│   ├── [Condition "is_rate_limit"]
│   │       OutputContains { pattern: "429" }
│   ├── [Cooldown "cooldown" duration=5s]
│   │   └── [Action "retry"]
│   │           RetryTool { cooldown_ms: 5000 }
│   └── [Action "emit_retry"]
│           RetryTool { ... }
│
└── ...
```

**Trace**:
1. `is_rate_limit` detects "429" in output → Success.
2. `Cooldown` checks if 5 seconds have passed since last success.
   - If yes: `retry` Action returns Success. Cooldown updates last_success. Sequence returns Success.
   - If no: Cooldown returns Failure. Sequence returns Failure. Root tries next child.

### 7.4 Example 4: Human Escalation with ForceHuman Decorator

```text
[Sequence "submit_pr_guard"]
├── [Condition "is_submit_pr"]
│       OutputContains { pattern: "submit_pr" }
├── [ForceHuman "force_human" reason="PR submission requires approval"]
│   └── [Prompt "confirm_pr"]
│           template: "The agent wants to submit a PR. Approve? YES/NO"
│           parser: EnumParser ["YES", "NO"]
│           sets: [("pr_approved", "decision")]
└── [Sequence "if_approved"]
    ├── [VariableIs { key: "pr_approved", value: "YES" }]
    └── [Action "emit_prepare_pr"]
            PreparePr { ... }
```

**Trace**:
1. `is_submit_pr` → Success.
2. `ForceHuman` ticks `confirm_pr`.
   - Prompt asks LLM for approval. LLM says "YES".
   - Prompt returns Success.
   - ForceHuman pushes `EscalateToHuman` to commands (human must confirm).
   - ForceHuman returns Success.
3. `if_approved`: checks variable → Success → emits `PreparePr`.

**Result**: Two commands are emitted: `[EscalateToHuman, PreparePr]`. The runtime presents the human with the escalation, and if approved, executes the PR preparation.

### 7.5 Example 5: Task Start Preparation

```text
[Sequence "task_start"]
├── [Condition "is_task_starting"]
│       OutputContains { pattern: "task_starting" }
├── [Prompt "choose_strategy"]
│       template: "..."
│       parser: EnumParser ["NEW_BRANCH", "EXISTING_BRANCH"]
│       sets: [("branch_strategy", "decision")]
└── [Selector "branch_on_strategy"]
    ├── [Sequence "new_branch"]
    │   ├── [VariableIs { key: "branch_strategy", value: "NEW_BRANCH" }]
    │   └── [Action "create_branch"]
    │           CreateTaskBranch { ... }
    └── [Sequence "existing_branch"]
        ├── [VariableIs { key: "branch_strategy", value: "EXISTING_BRANCH" }]
        └── [Action "rebase"]
                RebaseToMain { ... }
```

---

## 8. Observability in Behavior Trees

### 8.1 Node-Level Tracing

Every tick produces a trace entry:

```rust
pub struct NodeTrace {
    pub node_name: String,
    pub node_type: String, // "Selector", "Prompt", "Condition", ...
    pub depth: usize,
    pub status: NodeStatus,
    pub duration_us: u64,
    pub blackboard_reads: Vec<String>,
    pub blackboard_writes: Vec<(String, BlackboardValue)>,
    pub commands_emitted: Vec<DecisionCommand>,
    pub llm_latency_ms: Option<u64>,
    pub llm_tokens: Option<(u32, u32)>, // prompt, completion
}
```

**Trace output for Example 1**:
```json
[
  { "node_name": "root_handler", "node_type": "Selector", "depth": 0, "status": "Success", "duration_us": 2450000 },
  { "node_name": "rate_limit_handler", "node_type": "Sequence", "depth": 1, "status": "Failure", "duration_us": 150 },
  { "node_name": "is_rate_limit", "node_type": "Condition", "depth": 2, "status": "Failure", "duration_us": 50, "blackboard_reads": ["provider_output"] },
  { "node_name": "human_escalation", "node_type": "Sequence", "depth": 1, "status": "Failure", "duration_us": 200 },
  { "node_name": "reflect_loop", "node_type": "Sequence", "depth": 1, "status": "Success", "duration_us": 2450000 },
  { "node_name": "is_claims_completion", "node_type": "Condition", "depth": 2, "status": "Success", "duration_us": 100, "blackboard_reads": ["provider_output"] },
  { "node_name": "guard", "node_type": "ReflectionGuard", "depth": 2, "status": "Success", "duration_us": 50, "blackboard_reads": ["reflection_round"], "blackboard_writes": [["reflection_round", 1]] },
  { "node_name": "reflect_or_confirm", "node_type": "Prompt", "depth": 3, "status": "Success", "duration_us": 2400000, "llm_latency_ms": 2400, "llm_tokens": [1240, 15], "blackboard_writes": [["next_action", "REFLECT"]] },
  { "node_name": "branch_on_decision", "node_type": "Selector", "depth": 2, "status": "Success", "duration_us": 50000 },
  { "node_name": "emit_reflect", "node_type": "Action", "depth": 4, "status": "Success", "commands_emitted": [{"Reflect": {"prompt": "Review your work carefully"}}] }
]
```

### 8.2 Tree Visualization

Behavior trees can be visualized as ASCII art or rendered in the TUI:

```text
[✓] root_handler (Selector) — 2.45ms
├── [✗] rate_limit_handler (Sequence) — 0.15ms
│   └── [✗] is_rate_limit (Condition) — 0.05ms
├── [✗] human_escalation (Sequence) — 0.2ms
├── [✓] reflect_loop (Sequence) — 2.45ms
│   ├── [✓] is_claims_completion (Condition) — 0.1ms
│   ├── [✓] guard (ReflectionGuard) — 0.05ms
│   ├── [✓] reflect_or_confirm (Prompt) — 2.4ms  [LLM: 1240→15 tok, 2.4s]
│   └── [✓] branch_on_decision (Selector) — 0.05ms
│       └── [✓] emit_reflect (Action) — 0.01ms
└── (skipped) error_recovery
    └── (skipped) default_continue
```

### 8.3 Metrics

| Metric | Labels | Description |
|--------|--------|-------------|
| `bt_tick_total` | `tree_name` | Total ticks executed |
| `bt_node_duration_us` | `node_name`, `node_type` | Per-node execution time |
| `bt_node_status` | `node_name`, `status` | Status histogram per node |
| `bt_prompt_latency_ms` | `node_name`, `model` | LLM latency per Prompt node |
| `bt_prompt_tokens` | `node_name` | Prompt/completion tokens per Prompt node |
| `bt_blackboard_reads` | `key` | How often each Blackboard key is read |
| `bt_blackboard_writes` | `key` | How often each Blackboard key is written |

---

## 9. Migration from Current Design

### 9.1 Mapping Existing Components

| Current Component | Behavior Tree Equivalent |
|-------------------|--------------------------|
| `TieredDecisionEngine` | A `Selector` root node with children for each tier/path |
| `DecisionTier::from_situation` | A `Selector` child ordering (priority encoded in tree structure) |
| `RuleBasedDecisionEngine` | A `Sequence` of `Condition` + `Action` nodes |
| `LLMDecisionEngine` | A `Prompt` node |
| `CLIDecisionEngine` | An `Action` node emitting `EscalateToHuman` |
| `ConditionExpr` | `Condition` nodes with `ConditionEvaluator` |
| `DecisionAction` | `Action` nodes (or `Prompt` nodes that parse to actions) |
| `DecisionContext.metadata` | Blackboard `variables` |
| `reflection_round` | Blackboard field, managed by `ReflectionGuard` decorator |
| `DecisionPreProcessor` | `Sequence` prefix nodes (context enrichment) |
| `DecisionPostProcessor` | `Sequence` suffix nodes (validation) |

### 9.2 Migration Path

**Phase 1 — Coexistence**: Build `BehaviorTreeExecutor` alongside existing engines. Add a `BehaviorTreeEngine` that implements `DecisionEngine` and delegates to the executor. Existing code continues to work.

**Phase 2 — Tree adoption**: Port the most common decision paths (claims_completion, rate_limit, error) to behavior trees. Compare outputs between old and new engines using golden tests.

**Phase 3 — Deprecation**: Mark `TieredDecisionEngine`, `RuleBasedDecisionEngine`, and `LLMDecisionEngine` as deprecated. All new decision logic is added as behavior trees.

**Phase 4 — Removal**: After one release cycle, remove the old engine hierarchy. The behavior tree executor becomes the sole decision mechanism.

---

## 10. Current Codebase Reality Check

### What Exists

| Component | Status | Notes |
|-----------|--------|-------|
| `DecisionCommand` | ✅ Stable | Pure data enum — fits perfectly as Action node output |
| Session integration | ✅ Stable | Prompt nodes send messages within the same codex/claude session |
| `DecisionContext` | ⚠️ Needs refactor | Replace with `Blackboard` struct |
| `ConditionExpr` | ⚠️ Partially reusable | Can be ported to `ConditionEvaluator` trait |
| `ActionRegistry` | ⚠️ Obsolete | Replaced by Action nodes |
| `TieredDecisionEngine` | ❌ To be replaced | Root Selector node |
| `RuleBasedDecisionEngine` | ❌ To be replaced | Sequence of Condition + Action nodes |
| `LLMDecisionEngine` | ❌ To be replaced | Prompt node |
| `pipeline.rs` | ❌ To be replaced | Replaced by BehaviorTreeExecutor |

### What Needs to Be Built

1. `Blackboard` struct — typed shared state.
2. `BehaviorNode` trait — node interface.
3. Composite nodes — `Selector`, `Sequence`, `Parallel`.
4. Decorator nodes — `Inverter`, `Repeater`, `Cooldown`, `ReflectionGuard`, `ForceHuman`.
5. Leaf nodes — `Condition`, `Action`, `Prompt`, `SetVar`.
6. `BehaviorTreeExecutor` — tick/reset logic.
7. Template renderer — Jinja2-style for Prompt nodes.
8. Output parsers — `EnumParser`, `StructuredParser`, `JsonParser`.
9. Sub-tree registry — named reusable sub-trees.
10. Node tracer — per-node execution tracing.

---

> Document version: v3.0-behavior-tree
> Last updated: 2026-04-20
> Related modules: `agent-decision`, `agent-core/decision_agent_slot`
