# Decision DSL Implementation Design

> Complete implementation specification for a standalone, zero-dependency behavior tree engine. The design follows embedded script-language patterns (inspired by Lua): the host program creates the engine, loads trees, ticks them, and receives pure data outputs. All external dependencies are injected via traits.

---

## 1. Overview

### 1.1 Design Goals

| Goal | How |
|------|-----|
| **Standalone** | Zero dependencies on `agent-*` crates. Only stdlib + serde + regex. |
| **Two public entrypoints** | `DslParser` (YAML → AST) and `DslRunner` (AST → commands). |
| **Trait-based injection** | LLM session, filesystem, clock, logging are all traits. |
| **Lua-inspired** | The engine is an embedded VM: host loads → ticks → receives output. |
| **Testable in isolation** | Every external dependency is mockable via trait impls. |
| **Zero-cost abstractions** | Hot paths (tick loop) use static dispatch where possible. |

### 1.2 Architecture Analogy: Lua

Lua is a minimal embedded scripting language. Our DSL engine follows the same pattern:

| Lua | Decision DSL |
|-----|--------------|
| `lua_State` — VM state | `Executor` — tree + running path + blackboard |
| `lua_pcall` — protected call | `DslRunner::tick` — returns `TickResult` |
| C function registry | `NodeRegistry` — maps `kind` string → node factory |
| Global table `_G` | `Blackboard` — shared key-value store |
| `lua_load` + `lua_pcall` | `DslParser::parse_tree` + `DslRunner::tick` |
| Custom allocator | `Blackboard` storage is pluggable |

### 1.3 What the Host Sees

```rust
// The host (e.g. agent-decision) sees ONLY this:
use decision_dsl::{DslParser, DslRunner, YamlParser, Executor};
use decision_dsl::ext::{Session, Clock, Fs, Logger};

// 1. Parse
let parser = YamlParser::new();
let tree = parser.parse_tree(&fs::read_to_string("tree.yaml")?)?;

// 2. Create runner with injected dependencies
let mut runner = Executor::new()
    .with_session(my_llm_session)
    .with_clock(SystemClock)
    .with_fs(my_fs)
    .with_logger(my_logger);

// 3. Tick
let mut ctx = TickContext::new(blackboard);
let result = runner.tick(&tree, &mut ctx)?;

// 4. Consume output
for cmd in result.commands {
    println!("{:?}", cmd);
}
```

### 1.4 Crate Dependencies

```toml
[package]
name = "decision-dsl"
version = "0.1.0"
edition = "2021"

[dependencies]
# Required: YAML parsing, serialization
serde = { version = "1.0", features = ["derive"] }
serde_yaml = "0.9"

# Required: template rendering
# tera = "1.19"  # or a minimal custom engine

# Optional: regex for structured parser
regex = { version = "1.10", optional = true }

# Optional: JSON schema validation
jsonschema = { version = "0.17", optional = true }

# Dev dependencies
[dev-dependencies]
# No agent-* crates. Only mock trait impls.
```

---

## 2. Package Structure

```
decision-dsl/
├── Cargo.toml
└── src/
    ├── lib.rs              # Public API surface
    ├── parser/
    │   ├── mod.rs          # Parser trait + YamlParser impl
    │   ├── ast.rs          # AST types (Tree, Node, Command)
    │   └── validate.rs     # Schema validation (optional)
    ├── runtime/
    │   ├── mod.rs          # Runtime exports
    │   ├── executor.rs     # Executor: tick/reset internals
    │   ├── blackboard.rs   # Blackboard: typed key-value store
    │   └── context.rs      # TickContext: per-tick injection
    ├── nodes/
    │   ├── mod.rs          # Node trait + registry
    │   ├── composite.rs    # Selector, Sequence, Parallel
    │   ├── decorator.rs    # Inverter, Repeater, Cooldown, etc.
    │   └── leaf.rs         # Condition, Action, Prompt, SetVar
    ├── template/
    │   ├── mod.rs          # Template engine
    │   └── engine.rs       # Render + filter registry
    ├── ext/                # External dependency traits
    │   ├── mod.rs
    │   ├── session.rs      # Session (LLM same-session)
    │   ├── clock.rs        # Clock (time source)
    │   ├── fs.rs           # Fs (filesystem abstraction)
    │   └── log.rs          # Logger (structured logging)
    └── error.rs            # Error types
```

### Visibility Rules

```rust
// lib.rs — ONLY these are pub
pub use parser::{DslParser, YamlParser, Tree, ParseError};
pub use runtime::{DslRunner, Executor, TickContext, TickResult};
pub use runtime::{Blackboard, BlackboardValue};
pub use ext::{Session, SessionError, Clock, Fs, FsError, Logger, LogLevel};
pub use error::{DslError, RuntimeError};

// Everything in nodes/, template/, parser/ast.rs is pub(crate)
```

---

## 3. Public API

### 3.1 DslParser

```rust
use std::path::Path;

/// Parse YAML/JSON DSL into an abstract syntax tree.
///
/// This is the ONLY way to turn DSL text into executable trees.
pub trait DslParser {
    /// Parse a single tree from YAML text.
    fn parse_tree(&self, yaml: &str) -> Result<Tree, ParseError>;

    /// Parse a bundle of trees from a directory.
    /// Requires a Fs implementation to read files.
    fn parse_bundle(&self, dir: &Path, fs: &dyn Fs) -> Result<Bundle, ParseError>;
}

/// Concrete implementation: YAML-based parser.
pub struct YamlParser {
    node_registry: NodeRegistry,
    schema_validator: Option<SchemaValidator>,
}

impl YamlParser {
    pub fn new() -> Self {
        Self {
            node_registry: NodeRegistry::with_builtins(),
            schema_validator: None,
        }
    }

    pub fn with_schema_validation(self) -> Self {
        Self {
            schema_validator: Some(SchemaValidator::default()),
            ..self
        }
    }
}

impl DslParser for YamlParser {
    fn parse_tree(&self, yaml: &str) -> Result<Tree, ParseError> {
        // 1. Parse raw YAML into serde_yaml::Value
        let raw: serde_yaml::Value = serde_yaml::from_str(yaml)?;

        // 2. Validate against JSON schema (if enabled)
        if let Some(v) = &self.schema_validator {
            v.validate(&raw)?;
        }

        // 3. Convert to AST
        let tree = self.value_to_tree(raw)?;

        // 4. Semantic validation
        tree.validate_unique_names()?;
        tree.validate_subtree_refs()?;

        Ok(tree)
    }

    fn parse_bundle(&self, dir: &Path, fs: &dyn Fs) -> Result<Bundle, ParseError> {
        let mut bundle = Bundle::default();

        // Read trees/
        for entry in fs.read_dir(&dir.join("trees"))? {
            let yaml = fs.read_to_string(&entry)?;
            let tree = self.parse_tree(&yaml)?;
            bundle.trees.insert(tree.name.clone(), tree);
        }

        // Read subtrees/
        for entry in fs.read_dir(&dir.join("subtrees"))? {
            let yaml = fs.read_to_string(&entry)?;
            let tree = self.parse_tree(&yaml)?;
            bundle.subtrees.insert(tree.name.clone(), tree);
        }

        // Resolve all subtree references
        bundle.resolve_subtrees()?;

        Ok(bundle)
    }
}
```

### 3.2 DslRunner

```rust
/// Execute a behavior tree against a Blackboard.
///
/// The runner maintains internal state (running path) across ticks.
/// Call `reset()` before starting a new decision cycle.
pub trait DslRunner {
    /// Tick the tree once.
    ///
    /// If a Prompt node returns Running, the runner stores the path
    /// and resumes from there on the next tick.
    fn tick(&mut self, tree: &Tree, ctx: &mut TickContext) -> Result<TickResult, RuntimeError>;

    /// Reset internal state for a new decision cycle.
    fn reset(&mut self);
}

/// Result of a single tick.
#[derive(Debug, Clone)]
pub struct TickResult {
    /// Final status of the root node.
    pub status: NodeStatus,
    /// Commands emitted during this tick.
    pub commands: Vec<Command>,
    /// Execution trace (for debugging / observability).
    pub trace: Vec<TraceEntry>,
}

/// Per-tick context. Constructed fresh each tick.
pub struct TickContext<'a> {
    /// The blackboard (shared mutable state).
    pub blackboard: &'a mut Blackboard,
    /// External dependencies (injected by host).
    pub session: &'a mut dyn Session,
    pub clock: &'a dyn Clock,
    pub fs: &'a dyn Fs,
    pub logger: &'a dyn Logger,
}

impl<'a> TickContext<'a> {
    pub fn new(
        blackboard: &'a mut Blackboard,
        session: &'a mut dyn Session,
        clock: &'a dyn Clock,
        fs: &'a dyn Fs,
        logger: &'a dyn Logger,
    ) -> Self {
        Self { blackboard, session, clock, fs, logger }
    }
}
```

### 3.3 Executor (Concrete Runner)

```rust
/// The default runner implementation.
pub struct Executor {
    /// Path to the currently Running node (indices from root).
    running_path: Vec<usize>,
    /// Whether we are currently mid-tick (Waiting for async prompt).
    is_running: bool,
}

impl Executor {
    pub fn new() -> Self {
        Self {
            running_path: Vec::new(),
            is_running: false,
        }
    }
}

impl DslRunner for Executor {
    fn tick(&mut self, tree: &Tree, ctx: &mut TickContext) -> Result<TickResult, RuntimeError> {
        let mut tracer = Tracer::new();

        let status = if self.is_running {
            // Resume from running node
            tree.resume(&self.running_path, ctx, &mut tracer)?
        } else {
            // Fresh tick from root
            tree.root.tick(ctx, &mut tracer)?
        };

        if status == NodeStatus::Running {
            self.is_running = true;
            self.running_path = tracer.running_path().to_vec();
        } else {
            self.is_running = false;
            self.running_path.clear();
        }

        let commands = std::mem::take(&mut ctx.blackboard.commands);

        Ok(TickResult {
            status,
            commands,
            trace: tracer.into_entries(),
        })
    }

    fn reset(&mut self) {
        self.is_running = false;
        self.running_path.clear();
    }
}
```

---

## 4. External Dependency Traits

All external integration points are traits. The host must provide implementations.

### 4.1 Session (LLM Same-Session)

```rust
/// Abstraction over the ongoing codex/claude session.
///
/// The Prompt node calls `send()` to inject a decision prompt
/// into the current conversation, then waits for the reply.
pub trait Session {
    /// Send a message to the current session and return the reply.
    ///
    /// This is a synchronous call from the runner's perspective.
    /// The actual implementation may be async internally, but the
    /// runner expects to either get a reply immediately or be told
    /// to wait (via `is_ready()`).
    fn send(&mut self, message: &str) -> Result<String, SessionError>;

    /// Check if the session has a pending reply ready.
    ///
    /// If this returns false, the Prompt node returns `Running`
    /// and the runner will re-tick later.
    fn is_ready(&self) -> bool;
}

#[derive(Debug, Clone)]
pub struct SessionError {
    pub kind: SessionErrorKind,
    pub message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionErrorKind {
    /// The session is not connected or unavailable.
    Unavailable,
    /// The message was sent but no reply was received (timeout).
    Timeout,
    /// The session returned an unexpected format.
    UnexpectedFormat,
}

// --- Host implementation example ---

use decision_dsl::Session;

pub struct AgentSession {
    // Host's internal session handle
    // (e.g. agent-provider's session object)
    tx: mpsc::Sender<String>,
    rx: mpsc::Receiver<String>,
}

impl Session for AgentSession {
    fn send(&mut self, message: &str) -> Result<String, SessionError> {
        self.tx.send(message.to_string()).map_err(|_| {
            SessionError { kind: SessionErrorKind::Unavailable, message: "channel closed".into() }
        })?;
        let reply = self.rx.recv_timeout(Duration::from_secs(30)).map_err(|_| {
            SessionError { kind: SessionErrorKind::Timeout, message: "no reply in 30s".into() }
        })?;
        Ok(reply)
    }

    fn is_ready(&self) -> bool {
        !self.rx.is_empty()
    }
}
```

### 4.2 Clock

```rust
use std::time::{Duration, Instant};

/// Time source abstraction.
///
/// Used by stateful decorators (Cooldown) to track elapsed time.
pub trait Clock {
    fn now(&self) -> Instant;
}

/// System clock (production).
pub struct SystemClock;

impl Clock for SystemClock {
    fn now(&self) -> Instant {
        Instant::now()
    }
}

/// Mock clock for testing.
pub struct MockClock {
    current: Instant,
}

impl MockClock {
    pub fn new() -> Self {
        Self { current: Instant::now() }
    }

    pub fn advance(&mut self, duration: Duration) {
        self.current += duration;
    }
}

impl Clock for MockClock {
    fn now(&self) -> Instant {
        self.current
    }
}
```

### 4.3 Fs

```rust
use std::path::{Path, PathBuf};

/// Filesystem abstraction for loading bundles and subtrees.
pub trait Fs {
    fn read_to_string(&self, path: &Path) -> Result<String, FsError>;
    fn read_dir(&self, path: &Path) -> Result<Vec<PathBuf>, FsError>;
}

#[derive(Debug, Clone)]
pub struct FsError {
    pub path: PathBuf,
    pub message: String,
}

/// Real filesystem (production).
pub struct RealFs;

impl Fs for RealFs {
    fn read_to_string(&self, path: &Path) -> Result<String, FsError> {
        std::fs::read_to_string(path).map_err(|e| FsError {
            path: path.to_path_buf(),
            message: e.to_string(),
        })
    }

    fn read_dir(&self, path: &Path) -> Result<Vec<PathBuf>, FsError> {
        std::fs::read_dir(path)
            .map_err(|e| FsError { path: path.to_path_buf(), message: e.to_string() })?
            .map(|entry| entry.map(|e| e.path()).map_err(|e| FsError {
                path: path.to_path_buf(),
                message: e.to_string(),
            }))
            .collect()
    }
}

/// In-memory filesystem (testing).
pub struct MemFs {
    files: HashMap<PathBuf, String>,
}

impl Fs for MemFs {
    fn read_to_string(&self, path: &Path) -> Result<String, FsError> {
        self.files.get(path).cloned().ok_or(FsError {
            path: path.to_path_buf(),
            message: "file not found".into(),
        })
    }

    fn read_dir(&self, path: &Path) -> Result<Vec<PathBuf>, FsError> {
        Ok(self.files.keys().filter(|p| p.parent() == Some(path)).cloned().collect())
    }
}
```

### 4.4 Logger

```rust
/// Structured logging trait.
///
/// All logging inside the DSL engine goes through this trait.
/// The host can forward to tracing, slog, or discard.
pub trait Logger {
    fn log(&self, level: LogLevel, target: &str, msg: &str);
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum LogLevel {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
}

/// Null logger (discards all logs).
pub struct NullLogger;

impl Logger for NullLogger {
    fn log(&self, _level: LogLevel, _target: &str, _msg: &str) {}
}

/// Adapter to tracing crate.
pub struct TracingLogger;

impl Logger for TracingLogger {
    fn log(&self, level: LogLevel, target: &str, msg: &str) {
        match level {
            LogLevel::Trace => tracing::trace!(target, "{}", msg),
            LogLevel::Debug => tracing::debug!(target, "{}", msg),
            LogLevel::Info => tracing::info!(target, "{}", msg),
            LogLevel::Warn => tracing::warn!(target, "{}", msg),
            LogLevel::Error => tracing::error!(target, "{}", msg),
        }
    }
}
```

---

## 5. AST Design

The AST is the internal representation of a parsed tree. It is NOT public API.

### 5.1 Tree and Bundle

```rust
pub(crate) struct Tree {
    pub api_version: String,
    pub kind: TreeKind,
    pub name: String,
    pub description: Option<String>,
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
            tree.resolve_subtrees(&self.subtrees)?;
        }
        Ok(())
    }
}
```

### 5.2 Node Enum

```rust
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

    // Leaves
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
}
```

### 5.3 Node-Specific Structs

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
    pub template: String,
    pub parser: Box<dyn Parser>,
    pub sets: Vec<SetMapping>,
    pub timeout: Duration,
    /// Internal state for async execution
    pending: bool,
}

pub(crate) struct SetVarNode {
    pub name: String,
    pub key: String,
    pub value: BlackboardValue,
}

pub(crate) struct SubTreeRefNode {
    pub name: String,
    pub ref_name: String,
    /// Resolved at parse time
    resolved: Option<Box<Node>>,
}
```

### 5.4 Command (Output Type)

```rust
/// Pure data output of the decision layer.
/// These are the ONLY values the host ever receives.
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

## 6. Node Registry

The node registry maps YAML `kind` strings to node factories. It is the C-function-equivalent registry.

### 6.1 Registry Design

```rust
/// Factory function that creates a Node from raw YAML properties.
pub(crate) type NodeFactory = fn(
    name: String,
    properties: serde_yaml::Mapping,
    children: Vec<Node>,
) -> Result<Node, ParseError>;

/// Registry of node factories.
pub(crate) struct NodeRegistry {
    factories: HashMap<String, NodeFactory>,
}

impl NodeRegistry {
    pub fn new() -> Self {
        Self { factories: HashMap::new() }
    }

    /// Register a built-in node type.
    pub fn with_builtins() -> Self {
        let mut reg = Self::new();

        // Composites
        reg.register("Selector", SelectorNode::from_yaml);
        reg.register("Sequence", SequenceNode::from_yaml);
        reg.register("Parallel", ParallelNode::from_yaml);

        // Decorators
        reg.register("Inverter", InverterNode::from_yaml);
        reg.register("Repeater", RepeaterNode::from_yaml);
        reg.register("Cooldown", CooldownNode::from_yaml);
        reg.register("ReflectionGuard", ReflectionGuardNode::from_yaml);
        reg.register("ForceHuman", ForceHumanNode::from_yaml);

        // Leaves
        reg.register("Condition", ConditionNode::from_yaml);
        reg.register("Action", ActionNode::from_yaml);
        reg.register("Prompt", PromptNode::from_yaml);
        reg.register("SetBlackboard", SetVarNode::from_yaml);
        reg.register("SubTree", SubTreeRefNode::from_yaml);

        reg
    }

    pub fn register(&mut self, kind: &str, factory: NodeFactory) {
        self.factories.insert(kind.to_string(), factory);
    }

    pub fn create(
        &self,
        kind: &str,
        name: String,
        properties: serde_yaml::Mapping,
        children: Vec<Node>,
    ) -> Result<Node, ParseError> {
        let factory = self.factories
            .get(kind)
            .ok_or_else(|| ParseError::UnknownNodeKind { kind: kind.to_string() })?;
        factory(name, properties, children)
    }
}
```

### 6.2 Extending the Registry

The host can register custom nodes:

```rust
use decision_dsl::parser::{YamlParser, NodeRegistry};

// Host provides a custom node
fn my_custom_node(
    name: String,
    properties: serde_yaml::Mapping,
    _children: Vec<Node>,
) -> Result<Node, ParseError> {
    let param = properties.get("param")
        .and_then(|v| v.as_str())
        .ok_or(ParseError::MissingProperty("param"))?;
    Ok(Node::Condition(ConditionNode {
        name,
        evaluator: Box::new(CustomEvaluator::new(param)),
    }))
}

// Register it
let parser = YamlParser::new();
// Note: register_custom is not part of DslParser trait (it would leak internals).
// The host must use the concrete type or we expose a builder.
```

To keep the public API clean, custom registration is available through a builder:

```rust
let parser = YamlParser::builder()
    .with_schema_validation(true)
    .with_custom_node("MyCondition", my_custom_node)
    .build();
```

---

## 7. Executor / Tick Loop

The executor is the heart of the runtime. It implements the behavior tree tick algorithm.

### 7.1 Tick Algorithm

```
Tick(node, ctx):
    if node is Composite:
        return TickComposite(node, ctx)
    if node is Decorator:
        return TickDecorator(node, ctx)
    if node is Leaf:
        return TickLeaf(node, ctx)
```

### 7.2 Composite Tick Implementation

```rust
impl SelectorNode {
    fn tick(&mut self, ctx: &mut TickContext, tracer: &mut Tracer) -> Result<NodeStatus, RuntimeError> {
        let start = self.active_child.unwrap_or(0);

        for i in start..self.children.len() {
            tracer.enter(self, i);
            let status = self.children[i].tick(ctx, tracer)?;
            tracer.exit(self, i, status);

            match status {
                NodeStatus::Success => {
                    self.active_child = None;
                    return Ok(NodeStatus::Success);
                }
                NodeStatus::Running => {
                    self.active_child = Some(i);
                    return Ok(NodeStatus::Running);
                }
                NodeStatus::Failure => {
                    // Try next child
                    continue;
                }
            }
        }

        self.active_child = None;
        Ok(NodeStatus::Failure)
    }

    fn reset(&mut self) {
        self.active_child = None;
        for child in &mut self.children {
            child.reset();
        }
    }
}

impl SequenceNode {
    fn tick(&mut self, ctx: &mut TickContext, tracer: &mut Tracer) -> Result<NodeStatus, RuntimeError> {
        let start = self.active_child.unwrap_or(0);

        for i in start..self.children.len() {
            tracer.enter(self, i);
            let status = self.children[i].tick(ctx, tracer)?;
            tracer.exit(self, i, status);

            match status {
                NodeStatus::Success => {
                    // Continue to next child
                    continue;
                }
                NodeStatus::Running => {
                    self.active_child = Some(i);
                    return Ok(NodeStatus::Running);
                }
                NodeStatus::Failure => {
                    self.active_child = None;
                    return Ok(NodeStatus::Failure);
                }
            }
        }

        self.active_child = None;
        Ok(NodeStatus::Success)
    }

    fn reset(&mut self) {
        self.active_child = None;
        for child in &mut self.children {
            child.reset();
        }
    }
}
```

### 7.3 Decorator Tick Implementation

```rust
impl InverterNode {
    fn tick(&mut self, ctx: &mut TickContext, tracer: &mut Tracer) -> Result<NodeStatus, RuntimeError> {
        let status = self.child.tick(ctx, tracer)?;
        Ok(match status {
            NodeStatus::Success => NodeStatus::Failure,
            NodeStatus::Failure => NodeStatus::Success,
            NodeStatus::Running => NodeStatus::Running,
        })
    }

    fn reset(&mut self) {
        self.child.reset();
    }
}

impl CooldownNode {
    fn tick(&mut self, ctx: &mut TickContext, tracer: &mut Tracer) -> Result<NodeStatus, RuntimeError> {
        if let Some(last) = self.last_success {
            if ctx.clock.now().duration_since(last) < self.duration {
                ctx.logger.log(
                    LogLevel::Debug,
                    "Cooldown",
                    &format!("{}: still on cooldown", self.name),
                );
                return Ok(NodeStatus::Failure);
            }
        }

        let status = self.child.tick(ctx, tracer)?;
        if status == NodeStatus::Success {
            self.last_success = Some(ctx.clock.now());
        }
        Ok(status)
    }

    fn reset(&mut self) {
        self.last_success = None;
        self.child.reset();
    }
}

impl ReflectionGuardNode {
    fn tick(&mut self, ctx: &mut TickContext, tracer: &mut Tracer) -> Result<NodeStatus, RuntimeError> {
        // Check reflection count from blackboard
        let count = ctx.blackboard.get_u8("reflection_count").unwrap_or(0);
        if count >= self.max_rounds {
            ctx.logger.log(
                LogLevel::Info,
                "ReflectionGuard",
                &format!("{}: max rounds ({}) reached", self.name, self.max_rounds),
            );
            return Ok(NodeStatus::Failure);
        }

        let status = self.child.tick(ctx, tracer)?;
        if status == NodeStatus::Success {
            // Increment reflection count
            let new_count = count + 1;
            ctx.blackboard.set_u8("reflection_count", new_count);
        }
        Ok(status)
    }

    fn reset(&mut self) {
        self.child.reset();
    }
}
```

### 7.4 Leaf Tick Implementation

```rust
impl ActionNode {
    fn tick(&mut self, ctx: &mut TickContext, _tracer: &mut Tracer) -> Result<NodeStatus, RuntimeError> {
        // Check precondition
        if let Some(ref evaluator) = self.when {
            if !evaluator.evaluate(&ctx.blackboard)? {
                ctx.logger.log(
                    LogLevel::Debug,
                    "Action",
                    &format!("{}: precondition failed", self.name),
                );
                return Ok(NodeStatus::Failure);
            }
        }

        // Emit command
        ctx.blackboard.push_command(self.command.clone());
        Ok(NodeStatus::Success)
    }

    fn reset(&mut self) {}
}

impl SetVarNode {
    fn tick(&mut self, ctx: &mut TickContext, _tracer: &mut Tracer) -> Result<NodeStatus, RuntimeError> {
        ctx.blackboard.set(&self.key, self.value.clone());
        Ok(NodeStatus::Success)
    }

    fn reset(&mut self) {}
}
```

### 7.5 Running / Resume Semantics

When a Prompt node returns `Running`, the executor stores the path:

```rust
impl Tree {
    /// Resume a tick from a stored path.
    fn resume(
        &self,
        path: &[usize],
        ctx: &mut TickContext,
        tracer: &mut Tracer,
    ) -> Result<NodeStatus, RuntimeError> {
        self.root.resume_at(path, 0, ctx, tracer)
    }
}

impl Node {
    fn resume_at(
        &mut self,
        path: &[usize],
        depth: usize,
        ctx: &mut TickContext,
        tracer: &mut Tracer,
    ) -> Result<NodeStatus, RuntimeError> {
        if depth >= path.len() {
            // We're at the running node, tick it
            return self.tick(ctx, tracer);
        }

        let child_idx = path[depth];
        match self {
            Node::Selector(n) => {
                n.active_child = Some(child_idx);
                let status = n.children[child_idx].resume_at(path, depth + 1, ctx, tracer)?;
                // Continue Selector logic from child_idx
                match status {
                    NodeStatus::Success | NodeStatus::Failure => {
                        n.active_child = None;
                        // Re-run Selector tick from the next child
                        // ... (simplified)
                    }
                    NodeStatus::Running => {
                        // Still running, return immediately
                        return Ok(NodeStatus::Running);
                    }
                }
                todo!("full resume logic")
            }
            // ... other composites
            _ => self.tick(ctx, tracer),
        }
    }
}
```

For simplicity, we can instead store the full `TickResult` state and simply re-tick the entire tree from the root, but skip non-running nodes:

```rust
// Simpler approach: just re-tick from root
// Prompt nodes that are not ready return Running immediately
// Composites with no active child behave normally
// Composites with active_child resume from there
```

This is the approach used in most BT libraries. It requires each composite to track its active child.

---

## 8. Blackboard Design

The Blackboard is the shared key-value store. It is typed and safe.

### 8.1 Data Model

```rust
#[derive(Debug, Clone, PartialEq)]
pub enum BlackboardValue {
    Bool(bool),
    U8(u8),
    U32(u32),
    I32(i32),
    F64(f64),
    String(String),
    List(Vec<BlackboardValue>),
    Map(HashMap<String, BlackboardValue>),
    Command(Command),
}

pub struct Blackboard {
    values: HashMap<String, BlackboardValue>,
    /// Commands emitted during the current tick (cleared after tick).
    commands: Vec<Command>,
}

impl Blackboard {
    pub fn new() -> Self {
        Self {
            values: HashMap::new(),
            commands: Vec::new(),
        }
    }

    // --- Typed getters ---

    pub fn get_bool(&self, key: &str) -> Option<bool> {
        match self.values.get(key)? {
            BlackboardValue::Bool(v) => Some(*v),
            _ => None,
        }
    }

    pub fn get_u8(&self, key: &str) -> Option<u8> {
        match self.values.get(key)? {
            BlackboardValue::U8(v) => Some(*v),
            _ => None,
        }
    }

    pub fn get_u32(&self, key: &str) -> Option<u32> {
        match self.values.get(key)? {
            BlackboardValue::U32(v) => Some(*v),
            _ => None,
        }
    }

    pub fn get_string(&self, key: &str) -> Option<&str> {
        match self.values.get(key)? {
            BlackboardValue::String(v) => Some(v.as_str()),
            _ => None,
        }
    }

    // --- Setters ---

    pub fn set(&mut self, key: &str, value: BlackboardValue) {
        self.values.insert(key.to_string(), value);
    }

    pub fn set_bool(&mut self, key: &str, value: bool) {
        self.values.insert(key.to_string(), BlackboardValue::Bool(value));
    }

    pub fn set_u8(&mut self, key: &str, value: u8) {
        self.values.insert(key.to_string(), BlackboardValue::U8(value));
    }

    pub fn set_string(&mut self, key: &str, value: String) {
        self.values.insert(key.to_string(), BlackboardValue::String(value));
    }

    // --- Commands ---

    pub(crate) fn push_command(&mut self, cmd: Command) {
        self.commands.push(cmd);
    }

    pub(crate) fn drain_commands(&mut self) -> Vec<Command> {
        std::mem::take(&mut self.commands)
    }
}
```

### 8.2 Dot Notation for Nested Access

```rust
impl Blackboard {
    /// Get a nested value using dot notation: "user.name"
    pub fn get_path(&self, path: &str) -> Option<&BlackboardValue> {
        let mut parts = path.split('.');
        let first = parts.next()?;
        let mut current = self.values.get(first)?;

        for part in parts {
            current = match current {
                BlackboardValue::Map(m) => m.get(part)?,
                BlackboardValue::List(l) => {
                    let idx: usize = part.parse().ok()?;
                    l.get(idx)?
                }
                _ => return None,
            };
        }

        Some(current)
    }
}
```

---

## 9. Prompt Node Implementation

The Prompt node is the most complex node. It implements the "same-session" invariant.

### 9.1 Lifecycle

```
Tick 1: Prompt node sends message to session, returns Running
  ↓
[Host re-ticks when session is ready]
  ↓
Tick 2: Prompt node checks session.is_ready(), receives reply
        Parses reply into Blackboard keys
        Returns Success or Failure
```

### 9.2 Implementation

```rust
impl PromptNode {
    fn tick(&mut self, ctx: &mut TickContext, tracer: &mut Tracer) -> Result<NodeStatus, RuntimeError> {
        // Step 1: If we have a pending async reply, check if it's ready
        if self.pending {
            if !ctx.session.is_ready() {
                ctx.logger.log(LogLevel::Debug, "Prompt", &format!("{}: waiting for reply", self.name));
                return Ok(NodeStatus::Running);
            }

            // Reply is ready, fetch it
            let reply = ctx.session.send("POLL")?; // or use a dedicated poll method

            // Step 2: Parse reply
            match self.parser.parse(&reply) {
                Ok(values) => {
                    // Step 3: Store parsed values into blackboard
                    for mapping in &self.sets {
                        if let Some(value) = values.get(&mapping.from) {
                            let bb_value = mapping.convert(value)?;
                            ctx.blackboard.set(&mapping.to, bb_value);
                        }
                    }

                    self.pending = false;
                    tracer.log(&format!("{}: parsed reply: {:?}", self.name, values));
                    return Ok(NodeStatus::Success);
                }
                Err(e) => {
                    ctx.logger.log(LogLevel::Warn, "Prompt", &format!("{}: parse error: {}", self.name, e));
                    self.pending = false;
                    return Ok(NodeStatus::Failure);
                }
            }
        }

        // Step 4: First tick — render template and send
        let rendered = TemplateEngine::render(&self.template, &ctx.blackboard)?;
        ctx.logger.log(LogLevel::Debug, "Prompt", &format!("{}: sending prompt", self.name));

        ctx.session.send(&rendered)?;
        self.pending = true;

        Ok(NodeStatus::Running)
    }

    fn reset(&mut self) {
        self.pending = false;
    }
}
```

### 9.3 Set Mapping

```rust
pub(crate) struct SetMapping {
    /// Key in the parsed result
    pub from: String,
    /// Key in the blackboard
    pub to: String,
    /// Expected type
    pub ty: ValueType,
}

impl SetMapping {
    fn convert(&self, raw: &str) -> Result<BlackboardValue, RuntimeError> {
        match self.ty {
            ValueType::Bool => raw.parse::<bool>()
                .map(BlackboardValue::Bool)
                .map_err(|_| RuntimeError::TypeMismatch { key: self.to.clone(), expected: "bool", got: raw.to_string() }),
            ValueType::U8 => raw.parse::<u8>()
                .map(BlackboardValue::U8)
                .map_err(|_| RuntimeError::TypeMismatch { key: self.to.clone(), expected: "u8", got: raw.to_string() }),
            ValueType::String => Ok(BlackboardValue::String(raw.to_string())),
            // ...
        }
    }
}

pub(crate) enum ValueType {
    Bool,
    U8,
    U32,
    I32,
    F64,
    String,
}
```

### 9.4 Parser Trait

```rust
/// Parses an LLM reply into key-value pairs.
pub(crate) trait Parser: std::fmt::Debug {
    fn parse(&self, reply: &str) -> Result<HashMap<String, String>, ParseError>;
}

/// Default parser: extracts markdown code blocks.
#[derive(Debug)]
pub(crate) struct MarkdownBlockParser {
    /// The language tag to look for (e.g. "json", "yaml")
    pub language: String,
}

impl Parser for MarkdownBlockParser {
    fn parse(&self, reply: &str) -> Result<HashMap<String, String>, ParseError> {
        let block = extract_markdown_block(reply, &self.language)?;
        let value: serde_yaml::Value = serde_yaml::from_str(&block)?;
        flatten_yaml(&value)
    }
}

/// Alternative: extract key: value pairs from plain text.
#[derive(Debug)]
pub(crate) struct KeyValueParser;

impl Parser for KeyValueParser {
    fn parse(&self, reply: &str) -> Result<HashMap<String, String>, ParseError> {
        let mut map = HashMap::new();
        for line in reply.lines() {
            if let Some((key, value)) = line.split_once(':') {
                map.insert(key.trim().to_string(), value.trim().to_string());
            }
        }
        Ok(map)
    }
}
```

---

## 10. Template Engine

The template engine renders prompt templates using Blackboard values.

### 10.1 Simple Template Engine

For minimal dependencies, we implement a simple engine (no Tera dependency):

```rust
/// Simple template engine: supports {{key}} and {{key | filter}}.
pub(crate) struct TemplateEngine;

impl TemplateEngine {
    pub fn render(template: &str, bb: &Blackboard) -> Result<String, RuntimeError> {
        let mut result = template.to_string();
        let re = regex!(r"\{\{\s*([a-zA-Z_][a-zA-Z0-9_.]*)\s*(?:\|\s*(\w+)\s*)?\}\}");

        // Find all placeholders and replace
        for cap in re.captures_iter(template) {
            let full_match = cap.get(0).unwrap().as_str();
            let key = cap.get(1).unwrap().as_str();
            let filter_name = cap.get(2).map(|m| m.as_str());

            let value = bb.get_path(key)
                .ok_or_else(|| RuntimeError::MissingVariable { key: key.to_string() })?;

            let rendered = match filter_name {
                Some("upper") => value.to_string().to_uppercase(),
                Some("lower") => value.to_string().to_lower_case(),
                Some("json") => serde_json::to_string(value).unwrap_or_default(),
                Some(f) => return Err(RuntimeError::UnknownFilter { filter: f.to_string() }),
                None => value.to_string(),
            };

            result = result.replace(full_match, &rendered);
        }

        Ok(result)
    }
}
```

### 10.2 Context Object

For more complex templates, we support a context object:

```rust
// In YAML:
// template: |
//   Situation: {{situation}}
//   Confidence: {{confidence}}%
//   Is it complete? {{decision.is_complete}}
```

The template engine automatically resolves dot-notation paths through the Blackboard.

### 10.3 Filters

```rust
type FilterFn = fn(&BlackboardValue) -> Result<String, RuntimeError>;

pub(crate) struct FilterRegistry {
    filters: HashMap<String, FilterFn>,
}

impl FilterRegistry {
    pub fn with_builtins() -> Self {
        let mut reg = Self { filters: HashMap::new() };
        reg.register("upper", |v| Ok(v.to_string().to_uppercase()));
        reg.register("lower", |v| Ok(v.to_string().to_lowercase()));
        reg.register("json", |v| {
            serde_json::to_string(v).map_err(|e| RuntimeError::FilterError(e.to_string()))
        });
        reg.register("yesno", |v| {
            match v {
                BlackboardValue::Bool(true) => Ok("yes".to_string()),
                BlackboardValue::Bool(false) => Ok("no".to_string()),
                _ => Err(RuntimeError::FilterError("yesno requires bool".to_string())),
            }
        });
        reg
    }

    pub fn register(&mut self, name: &str, f: FilterFn) {
        self.filters.insert(name.to_string(), f);
    }
}
```

---

## 11. Error Handling

All errors are explicit enums. No panics in library code.

### 11.1 Error Types

```rust
/// Top-level DSL error. Either parse-time or runtime.
#[derive(Debug, Clone)]
pub enum DslError {
    Parse(ParseError),
    Runtime(RuntimeError),
}

impl std::fmt::Display for DslError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DslError::Parse(e) => write!(f, "parse error: {}", e),
            DslError::Runtime(e) => write!(f, "runtime error: {}", e),
        }
    }
}

impl std::error::Error for DslError {}

// --- Parse errors (detected before any execution) ---

#[derive(Debug, Clone)]
pub enum ParseError {
    /// YAML syntax error.
    YamlSyntax(String),
    /// Unknown node `kind`.
    UnknownNodeKind { kind: String },
    /// Missing required property.
    MissingProperty(&'static str),
    /// Invalid property value.
    InvalidProperty { key: String, value: String, reason: String },
    /// Subtree reference not found.
    UnresolvedSubTree { name: String },
    /// Duplicate node name within a tree.
    DuplicateName { name: String },
    /// Schema validation failed.
    SchemaValidation(Vec<String>),
    /// Custom error from node factory.
    Custom(String),
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ParseError::YamlSyntax(e) => write!(f, "YAML syntax error: {}", e),
            ParseError::UnknownNodeKind { kind } => write!(f, "unknown node kind: {}", kind),
            ParseError::MissingProperty(p) => write!(f, "missing required property: {}", p),
            ParseError::InvalidProperty { key, value, reason } => {
                write!(f, "invalid property '{}' = '{}': {}", key, value, reason)
            }
            ParseError::UnresolvedSubTree { name } => write!(f, "unresolved subtree reference: {}", name),
            ParseError::DuplicateName { name } => write!(f, "duplicate node name: {}", name),
            ParseError::SchemaValidation(errors) => {
                write!(f, "schema validation failed: {}", errors.join(", "))
            }
            ParseError::Custom(msg) => write!(f, "{}", msg),
        }
    }
}

impl std::error::Error for ParseError {}

// --- Runtime errors (occur during tick) ---

#[derive(Debug, Clone)]
pub enum RuntimeError {
    /// Template variable not found in blackboard.
    MissingVariable { key: String },
    /// Template filter not found.
    UnknownFilter { filter: String },
    /// Template filter failed.
    FilterError(String),
    /// Type mismatch in blackboard operation.
    TypeMismatch { key: String, expected: &'static str, got: String },
    /// Session error.
    Session { kind: SessionErrorKind, message: String },
    /// Maximum recursion depth exceeded (subtree nesting).
    MaxRecursion,
    /// Custom runtime error.
    Custom(String),
}

impl std::fmt::Display for RuntimeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RuntimeError::MissingVariable { key } => write!(f, "missing variable: {}", key),
            RuntimeError::UnknownFilter { filter } => write!(f, "unknown filter: {}", filter),
            RuntimeError::FilterError(msg) => write!(f, "filter error: {}", msg),
            RuntimeError::TypeMismatch { key, expected, got } => {
                write!(f, "type mismatch for '{}': expected {}, got {}", key, expected, got)
            }
            RuntimeError::Session { kind, message } => write!(f, "session error ({:?}): {}", kind, message),
            RuntimeError::MaxRecursion => write!(f, "maximum recursion depth exceeded"),
            RuntimeError::Custom(msg) => write!(f, "{}", msg),
        }
    }
}

impl std::error::Error for RuntimeError {}
```

### 11.2 Error Conversion

```rust
impl From<serde_yaml::Error> for ParseError {
    fn from(e: serde_yaml::Error) -> Self {
        ParseError::YamlSyntax(e.to_string())
    }
}

impl From<SessionError> for RuntimeError {
    fn from(e: SessionError) -> Self {
        RuntimeError::Session { kind: e.kind, message: e.message }
    }
}
```

### 11.3 Result Aliases

```rust
pub type ParseResult<T> = Result<T, ParseError>;
pub type RuntimeResult<T> = Result<T, RuntimeError>;
```

---

## 12. Testing Strategy

Every component is tested with mock trait implementations. No I/O, no LLM calls in tests.

### 12.1 Mock Implementations

```rust
// test-support utilities (within decision-dsl/tests/)

use decision_dsl::ext::*;
use std::cell::RefCell;
use std::collections::VecDeque;

/// Mock session with pre-programmed replies.
pub struct MockSession {
    replies: RefCell<VecDeque<String>>,
    ready: RefCell<bool>,
}

impl MockSession {
    pub fn new(replies: Vec<String>) -> Self {
        Self {
            replies: RefCell::new(replies.into_iter().collect()),
            ready: RefCell::new(true),
        }
    }

    pub fn set_ready(&self, ready: bool) {
        *self.ready.borrow_mut() = ready;
    }
}

impl Session for MockSession {
    fn send(&mut self, _message: &str) -> Result<String, SessionError> {
        self.replies.borrow_mut()
            .pop_front()
            .ok_or_else(|| SessionError {
                kind: SessionErrorKind::Unavailable,
                message: "no more replies".into(),
            })
    }

    fn is_ready(&self) -> bool {
        *self.ready.borrow()
    }
}

/// Mock logger that captures all logs.
pub struct CaptureLogger {
    logs: RefCell<Vec<(LogLevel, String, String)>>,
}

impl Logger for CaptureLogger {
    fn log(&self, level: LogLevel, target: &str, msg: &str) {
        self.logs.borrow_mut().push((level, target.to_string(), msg.to_string()));
    }
}
```

### 12.2 Unit Tests: Composite Nodes

```rust
#[cfg(test)]
mod composite_tests {
    use super::*;

    #[test]
    fn selector_returns_first_success() {
        let mut selector = SelectorNode {
            name: "test".into(),
            children: vec![
                Node::Condition(ConditionNode {
                    name: "fail".into(),
                    evaluator: Box::new(|_| Ok(false)),
                }),
                Node::Action(ActionNode {
                    name: "success".into(),
                    command: Command::ApproveAndContinue,
                    when: None,
                }),
            ],
            active_child: None,
        };

        let mut bb = Blackboard::new();
        let mut ctx = tick_context(&mut bb);
        let mut tracer = Tracer::new();

        let status = selector.tick(&mut ctx, &mut tracer).unwrap();
        assert_eq!(status, NodeStatus::Success);
        assert_eq!(bb.drain_commands().len(), 1);
    }

    #[test]
    fn selector_returns_running_and_resumes() {
        let mut selector = SelectorNode {
            name: "test".into(),
            children: vec![
                Node::Condition(ConditionNode {
                    name: "fail".into(),
                    evaluator: Box::new(|_| Ok(false)),
                }),
                Node::Prompt(PromptNode {
                    name: "prompt".into(),
                    template: "test".into(),
                    parser: Box::new(KeyValueParser),
                    sets: vec![],
                    timeout: Duration::from_secs(1),
                    pending: false,
                }),
            ],
            active_child: None,
        };

        let mut bb = Blackboard::new();
        let mut session = MockSession::new(vec![]);
        session.set_ready(false);

        let mut ctx = tick_context_with_session(&mut bb, &mut session);
        let mut tracer = Tracer::new();

        // First tick: Prompt returns Running
        let status = selector.tick(&mut ctx, &mut tracer).unwrap();
        assert_eq!(status, NodeStatus::Running);
        assert_eq!(selector.active_child, Some(1));

        // Second tick: session still not ready
        let status = selector.tick(&mut ctx, &mut tracer).unwrap();
        assert_eq!(status, NodeStatus::Running);

        // Now session is ready
        session.set_ready(true);
        // But MockSession has no replies, so it will fail
        // In real test we'd provide a reply
    }
}
```

### 12.3 Integration Tests: Full Tree Tick

```rust
#[test]
fn full_tree_escalates_to_human() {
    let yaml = r#"
api_version: "1.0"
kind: BehaviorTree
name: test-tree
description: "Test tree for escalation"
root:
  type: composite
  kind: Sequence
  name: seq
  children:
    - type: leaf
      kind: Condition
      name: is_human_needed
      eval: "{{confidence}} < 0.7"
    - type: leaf
      kind: Action
      name: escalate
      command:
        action_type: EscalateToHuman
        reason: "Confidence below threshold"
        context: "Decision was ambiguous"
"#;

    let parser = YamlParser::new();
    let tree = parser.parse_tree(yaml).unwrap();

    let mut bb = Blackboard::new();
    bb.set_f64("confidence", 0.5);

    let mut executor = Executor::new();
    let mut ctx = tick_context(&mut bb);
    let result = executor.tick(&tree, &mut ctx).unwrap();

    assert_eq!(result.status, NodeStatus::Success);
    assert_eq!(result.commands.len(), 1);
    match &result.commands[0] {
        Command::EscalateToHuman { reason, .. } => {
            assert_eq!(reason, "Confidence below threshold");
        }
        other => panic!("expected EscalateToHuman, got {:?}", other),
    }
}
```

### 12.4 Property-Based Testing

```rust
#[test]
fn any_tree_with_all_actions_returns_commands() {
    // Generate random trees, ensure they never panic
    // and that Success/Failure always terminates
    proptest::proptest!(|(depth in 1u8..10)| {
        let tree = generate_random_tree(depth);
        let mut bb = Blackboard::new();
        let mut executor = Executor::new();
        let mut ctx = tick_context(&mut bb);

        let result = executor.tick(&tree, &mut ctx);
        // Must not panic; status must be terminal
        if let Ok(res) = result {
            assert!(res.status != NodeStatus::Running || tree.has_prompt_node());
        }
    });
}
```

### 12.5 Test Coverage Requirements

| Component | Coverage Target |
|-----------|----------------|
| Parser | 95% — all node kinds, error paths |
| Executor / Tick | 95% — all node types, resume paths |
| Blackboard | 100% — all types, all edge cases |
| Template Engine | 95% — all filters, missing vars |
| Prompt Node | 90% — pending, ready, parse fail |
| Error Types | 100% — Display, From impls |

---

## 13. Performance Considerations

### 13.1 Allocation Strategy

```rust
// Hot path: tick loop. Minimize allocations.

// Blackboard: use a small-string-optimized key type
pub struct SmallString([u8; 32]); // stack-allocated for keys < 32 bytes

// Commands: pre-allocate Vec capacity
impl Blackboard {
    pub fn with_capacity(n: usize) -> Self {
        Self {
            values: HashMap::with_capacity(n),
            commands: Vec::with_capacity(8),
        }
    }
}

// Template engine: compile regex once
lazy_static! {
    static ref TEMPLATE_RE: Regex = Regex::new(r"\{\{...\}\}").unwrap();
}
```

### 13.2 Tick Loop Hot Path

```rust
// The tick loop must be allocation-free for non-Prompt nodes.
// Only Prompt nodes allocate (for template rendering and session I/O).

// Benchmark target: 1M ticks/second on a single composite tree
// (measured with criterion.rs)
```

### 13.3 Memory Footprint

```rust
// Tree memory: one-time parse cost.
// Runtime memory: only Blackboard + running path.
//
// For a tree with 100 nodes:
//   - Parse: ~50KB
//   - Runtime: ~2KB (blackboard) + path Vec
//   - Per-tick: 0 allocations (no Prompt), ~1KB (with Prompt)
```

### 13.4 Parallel Execution

```rust
// Trees are Send + Sync (no internal mutability in Tree itself).
// Executor is !Sync (mutates running_path).
// Blackboard is !Sync (mutable during tick).
//
// Design decision: One Executor + Blackboard per agent thread.
// No shared mutable state across agents.
```

### 13.5 Benchmarks

```rust
// benches/tick_benchmark.rs
use criterion::{black_box, criterion_group, criterion_main, Criterion};
use decision_dsl::*;

fn tick_benchmark(c: &mut Criterion) {
    let yaml = /* 100-node tree */;
    let parser = YamlParser::new();
    let tree = parser.parse_tree(yaml).unwrap();

    let mut group = c.benchmark_group("tick");
    group.throughput(criterion::Throughput::Elements(1));

    group.bench_function("selector_sequence", |b| {
        let mut bb = Blackboard::new();
        let mut executor = Executor::new();
        b.iter(|| {
            executor.reset();
            let mut ctx = tick_context(black_box(&mut bb));
            executor.tick(black_box(&tree), black_box(&mut ctx)).unwrap()
        });
    });
}

criterion_group!(benches, tick_benchmark);
criterion_main!(benches);
```

---

## 14. Integration Example

### 14.1 Host Integration: agent-decision

```rust
// In agent-decision crate:

use decision_dsl::{DslParser, DslRunner, YamlParser, Executor, TickContext};
use decision_dsl::{Blackboard, BlackboardValue};
use decision_dsl::ext::{Session, Clock, Fs, Logger, RealFs, SystemClock, TracingLogger};

/// Bridge: adapts agent-provider's session to decision_dsl::Session.
pub struct ProviderSessionBridge {
    provider_session: Arc<Mutex<ProviderSession>>,
}

impl Session for ProviderSessionBridge {
    fn send(&mut self, message: &str) -> Result<String, SessionError> {
        let mut session = self.provider_session.lock().unwrap();
        session.send_message(message)?;
        let reply = session.await_reply(Duration::from_secs(30)).map_err(|e| {
            SessionError { kind: SessionErrorKind::Timeout, message: e.to_string() }
        })?;
        Ok(reply)
    }

    fn is_ready(&self) -> bool {
        let session = self.provider_session.lock().unwrap();
        session.has_pending_reply()
    }
}

/// Host's decision engine using the DSL.
pub struct DslDecisionEngine {
    parser: YamlParser,
    executor: Executor,
    bundle: Bundle,
    session: ProviderSessionBridge,
    clock: SystemClock,
    fs: RealFs,
    logger: TracingLogger,
}

impl DslDecisionEngine {
    pub fn new(bundle_path: &Path, session: ProviderSessionBridge) -> Result<Self, DslError> {
        let parser = YamlParser::new();
        let fs = RealFs;
        let bundle = parser.parse_bundle(bundle_path, &fs)?;

        Ok(Self {
            parser,
            executor: Executor::new(),
            bundle,
            session,
            clock: SystemClock,
            fs,
            logger: TracingLogger,
        })
    }

    pub fn decide(&mut self, situation: &str, blackboard: &mut Blackboard) -> Result<Vec<Command>, DslError> {
        let tree = self.bundle.trees.get(situation)
            .ok_or_else(|| DslError::Runtime(RuntimeError::Custom(
                format!("no tree for situation: {}", situation)
            )))?;

        self.executor.reset();

        let mut ctx = TickContext::new(
            blackboard,
            &mut self.session,
            &self.clock,
            &self.fs,
            &self.logger,
        );

        let result = self.executor.tick(tree, &mut ctx)?;

        if result.status == NodeStatus::Running {
            // Host must re-call decide() when session is ready
            // The executor retains the running path
            return Ok(vec![Command::SkipDecision]);
        }

        Ok(result.commands)
    }
}
```

### 14.2 Host Integration: agent-tui

```rust
// TUI displays decision trace from TickResult

fn render_decision_trace(trace: &[TraceEntry]) {
    for entry in trace {
        match entry {
            TraceEntry::Enter { node_name, child_index } => {
                println!("  → {}[{}]", node_name, child_index);
            }
            TraceEntry::Exit { node_name, status } => {
                println!("  ← {} → {:?}", node_name, status);
            }
            TraceEntry::Log(msg) => {
                println!("    [log] {}", msg);
            }
        }
    }
}
```

### 14.3 Complete End-to-End Example

```rust
use decision_dsl::*;

fn main() -> Result<(), DslError> {
    // 1. Parse tree
    let yaml = include_str!("tree.yaml");
    let parser = YamlParser::new();
    let tree = parser.parse_tree(yaml)?;

    // 2. Set up blackboard
    let mut bb = Blackboard::new();
    bb.set_string("situation".into(), "claims_completion".into());
    bb.set_f64("confidence".into(), 0.95);
    bb.set_u32("reflection_count".into(), 0);

    // 3. Set up mocks (in real code: real implementations)
    let mut session = MockSession::new(vec![
        "is_complete: true\nconfidence: 0.95".into(),
    ]);
    let clock = SystemClock;
    let fs = RealFs;
    let logger = TracingLogger;

    // 4. Create runner
    let mut executor = Executor::new();

    // 5. Tick
    let mut ctx = TickContext::new(&mut bb, &mut session, &clock, &fs, &logger);
    let result = executor.tick(&tree, &mut ctx)?;

    println!("Status: {:?}", result.status);
    for cmd in &result.commands {
        println!("Command: {:?}", cmd);
    }

    Ok(())
}
```

---

## 15. Appendix: Node YAML Reference

### Composites

```yaml
# Selector: returns Success on first child that succeeds
type: composite
kind: Selector
name: root
children:
  - ...

# Sequence: returns Failure on first child that fails
type: composite
kind: Sequence
name: root
children:
  - ...

# Parallel: all children ticked each tick
type: composite
kind: Parallel
name: root
policy: RequireAll  # or RequireOne
children:
  - ...
```

### Decorators

```yaml
# Inverter: inverts child result
type: decorator
kind: Inverter
name: not-complete
child:
  ...

# Repeater: retries child up to N times
type: decorator
kind: Repeater
name: retry-parse
max_attempts: 3
child:
  ...

# Cooldown: fails if child succeeded within duration
type: decorator
kind: Cooldown
duration: "1m"
child:
  ...

# ReflectionGuard: fails after max rounds
type: decorator
kind: ReflectionGuard
max_rounds: 2
child:
  ...

# ForceHuman: wraps child, human escalation on failure
type: decorator
kind: ForceHuman
reason: "Critical path failed"
child:
  ...
```

### Leaves

```yaml
# Condition: evaluates expression, returns Success/Failure
type: leaf
kind: Condition
name: is-high-confidence
eval: "{{confidence}} >= 0.8"

# Action: emits a command
type: leaf
kind: Action
name: approve
command:
  action_type: ApproveAndContinue
when: "{{is_high_confidence}} == true"  # optional

# Prompt: sends message to LLM, parses reply
type: leaf
kind: Prompt
name: ask-completion
template: |
  Is the task complete? Reply with:
  is_complete: true/false
  confidence: 0-1
parser:
  type: markdown_block
  language: yaml
sets:
  - from: is_complete
    to: is_complete
    type: bool
  - from: confidence
    to: confidence
    type: f64
timeout: "30s"

# SetBlackboard: sets a blackboard key
type: leaf
kind: SetBlackboard
name: set-default-confidence
key: confidence
value: 0.5
```

### SubTree Reference

```yaml
type: leaf
kind: SubTree
name: retry-logic
ref: retry_subtree
```

---

## 16. Migration from Current Tiered Engine

The existing `TieredDecisionEngine` will be replaced incrementally:

```
Phase 1: decision-dsl crate (new, standalone)
  - Implement all components described above
  - Unit tests + integration tests
  - Benchmarks

Phase 2: agent-decision integration
  - Create adapter layer (Session, Clock, Fs, Logger impls)
  - Add `DslDecisionEngine` alongside existing `TieredDecisionEngine`
  - Feature flag: `dsl-engine`

Phase 3: Gradual cutover
  - Port one situation at a time to YAML trees
  - A/B test: compare outputs between tiered and DSL engines
  - Remove tiered engine once all situations are ported

Phase 4: Cleanup
  - Remove old engine code
  - Update documentation
  - Archive old specs
```

---

*Document version: 1.0*
*Last updated: 2026-04-20*
