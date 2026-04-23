# Decision DSL Implementation Design

> Complete implementation specification for a standalone, zero-dependency behavior tree engine. This document is the engineering blueprint that corresponds 1:1 with `decision-dsl.md` (DSL specification), `decision-layer-design.md` (architecture), and the five `decision-dsl-examples/`. Every feature described in those documents is assigned a concrete implementation here.

---

## 1. Overview

### 1.1 Design Goals

| Goal | How |
|------|-----|
| **Standalone** | Zero dependencies on `agent-*` crates. Only stdlib + serde + regex. |
| **Two public entrypoints** | `DslParser` (YAML → AST) and `DslRunner` (AST → commands). |
| **Trait-based injection** | LLM session, filesystem, clock, logging are all traits. |
| **Spec-compliant** | Every YAML construct, node type, parser, evaluator, and filter from `decision-dsl.md` is implemented. |
| **Lua-inspired** | The engine is an embedded VM: host loads → ticks → receives output. |
| **Testable in isolation** | Every external dependency is mockable via trait impls. |

### 1.2 Architecture Analogy: Lua

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
use decision_dsl::{DslParser, DslRunner, YamlParser, Executor, TickContext};
use decision_dsl::{Blackboard, BlackboardValue};
use decision_dsl::ext::{Session, Clock, Fs, Logger};

// 1. Parse
let parser = YamlParser::new();
let tree = parser.parse_tree(&fs::read_to_string("tree.yaml")?)?;

// 2. Create runner with injected dependencies
let mut runner = Executor::new();

// 3. Set up blackboard (built-in variables auto-populated by host)
let mut bb = Blackboard::new();
bb.set_string("task_description", "Implement auth".into());
bb.set_string("provider_output", output.into());

// 4. Tick
let mut ctx = TickContext::new(&mut bb, &mut session, &clock, &fs, &logger);
let result = runner.tick(&tree, &mut ctx)?;

// 5. Consume output
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
serde = { version = "1.0", features = ["derive"] }
serde_yaml = "0.9"
regex = "1.10"

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
    │   ├── yaml.rs         # serde_yaml → AST conversion
    │   └── validate.rs     # Schema + semantic validation
    ├── runtime/
    │   ├── mod.rs          # Runtime exports
    │   ├── executor.rs     # Executor: tick/reset/resume
    │   ├── blackboard.rs   # Blackboard: typed key-value store
    │   ├── context.rs      # TickContext: per-tick injection
    │   └── trace.rs        # NodeTrace + Tracer + metrics
    ├── nodes/
    │   ├── mod.rs          # Node trait + registry
    │   ├── composite.rs    # Selector, Sequence, Parallel
    │   ├── decorator.rs    # Inverter, Repeater, Cooldown, ReflectionGuard, ForceHuman
    │   └── leaf.rs         # Condition, Action, Prompt, SetVar
    ├── eval/
    │   ├── mod.rs          # Evaluator trait + registry
    │   └── builtin.rs      # OutputContains, VariableIs, Regex, Or, etc.
    ├── parser_out/
    │   ├── mod.rs          # OutputParser trait + registry
    │   └── builtin.rs      # EnumParser, StructuredParser, JsonParser, CommandParser
    ├── template/
    │   ├── mod.rs          # Template engine
    │   └── engine.rs       # Lexer + render + filter registry
    ├── ext/                # External dependency traits
    │   ├── mod.rs
    │   ├── session.rs      # Session (LLM same-session)
    │   ├── clock.rs        # Clock (time source)
    │   ├── fs.rs           # Fs (filesystem abstraction)
    │   ├── log.rs          # Logger (structured logging)
    │   └── watcher.rs      # Watcher (hot-reload change detection)
    └── error.rs            # Error types
```

### Visibility Rules

```rust
// lib.rs — ONLY these are pub
pub use parser::{DslParser, YamlParser, Tree, ParseError};
pub use runtime::{DslRunner, Executor, TickContext, TickResult, Blackboard, BlackboardValue};
pub use runtime::trace::{NodeTrace, TraceEntry};
pub use ext::{Session, SessionError, Clock, Fs, FsError, Logger, LogLevel, Watcher, PollWatcher, WatcherError};
pub use error::{DslError, RuntimeError};

// Everything in nodes/, eval/, parser_out/, template/ is pub(crate)
```

---

## 3. Public API

### 3.1 DslParser

```rust
use std::path::Path;

/// Parse YAML DSL into an abstract syntax tree.
pub trait DslParser {
    fn parse_tree(&self, yaml: &str) -> Result<Tree, ParseError>;
    fn parse_bundle(&self, dir: &Path, fs: &dyn Fs) -> Result<Bundle, ParseError>;
}

/// Concrete implementation: YAML-based parser.
pub struct YamlParser {
    node_registry: NodeRegistry,
    evaluator_registry: EvaluatorRegistry,
    parser_registry: OutputParserRegistry,
    filter_registry: FilterRegistry,
}

impl YamlParser {
    pub fn new() -> Self {
        Self {
            node_registry: NodeRegistry::with_builtins(),
            evaluator_registry: EvaluatorRegistry::with_builtins(),
            parser_registry: OutputParserRegistry::with_builtins(),
            filter_registry: FilterRegistry::with_builtins(),
        }
    }
}

impl DslParser for YamlParser {
    fn parse_tree(&self, yaml: &str) -> Result<Tree, ParseError> {
        // 1. Parse raw YAML into serde_yaml::Value
        let raw: serde_yaml::Value = serde_yaml::from_str(yaml)?;

        // 2. Convert to AST (respects metadata/spec nesting)
        let tree = self.value_to_tree(raw)?;

        // 3. Semantic validation
        tree.validate_unique_names()?;
        tree.validate_subtree_refs()?;
        tree.validate_evaluators(&self.evaluator_registry)?;
        tree.validate_parsers(&self.parser_registry)?;

        Ok(tree)
    }

    fn parse_bundle(&self, dir: &Path, fs: &dyn Fs) -> Result<Bundle, ParseError> {
        let mut bundle = Bundle::default();

        for entry in fs.read_dir(&dir.join("trees"))? {
            let yaml = fs.read_to_string(&entry)?;
            let tree = self.parse_tree(&yaml)?;
            bundle.trees.insert(tree.metadata.name.clone(), tree);
        }

        for entry in fs.read_dir(&dir.join("subtrees"))? {
            let yaml = fs.read_to_string(&entry)?;
            let tree = self.parse_tree(&yaml)?;
            bundle.subtrees.insert(tree.metadata.name.clone(), tree);
        }

        bundle.resolve_subtrees()?;
        bundle.detect_circular_refs()?;

        Ok(bundle)
    }
}
```

### 3.2 DslRunner

```rust
/// Execute a behavior tree against a Blackboard.
pub trait DslRunner {
    fn tick(&mut self, tree: &Tree, ctx: &mut TickContext) -> Result<TickResult, RuntimeError>;
    fn reset(&mut self);
}

/// Result of a single tick.
#[derive(Debug, Clone)]
pub struct TickResult {
    pub status: NodeStatus,
    pub commands: Vec<Command>,
    pub trace: Vec<TraceEntry>,
}

/// Per-tick context.
pub struct TickContext<'a> {
    pub blackboard: &'a mut Blackboard,
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

### 3.3 Executor

```rust
pub struct Executor {
    running_path: Vec<usize>,
    is_running: bool,
}

impl Executor {
    pub fn new() -> Self {
        Self { running_path: Vec::new(), is_running: false }
    }
}

impl DslRunner for Executor {
    fn tick(&mut self, tree: &Tree, ctx: &mut TickContext) -> Result<TickResult, RuntimeError> {
        let mut tracer = Tracer::new();

        let status = if self.is_running {
            tree.resume(&self.running_path, ctx, &mut tracer)?
        } else {
            tree.root.tick(ctx, &mut tracer)?
        };

        if status == NodeStatus::Running {
            self.is_running = true;
            self.running_path = tracer.running_path().to_vec();
        } else {
            self.is_running = false;
            self.running_path.clear();
        }

        let commands = ctx.blackboard.drain_commands();

        Ok(TickResult { status, commands, trace: tracer.into_entries() })
    }

    fn reset(&mut self) {
        self.is_running = false;
        self.running_path.clear();
    }
}
```

---

## 4. External Dependency Traits

### 4.1 Session

```rust
/// Abstraction over the ongoing codex/claude session.
pub trait Session {
    fn send(&mut self, message: &str) -> Result<String, SessionError>;
    fn is_ready(&self) -> bool;
}

#[derive(Debug, Clone)]
pub struct SessionError {
    pub kind: SessionErrorKind,
    pub message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionErrorKind {
    Unavailable,
    Timeout,
    UnexpectedFormat,
}
```

### 4.2 Clock

```rust
use std::time::{Duration, Instant};

pub trait Clock {
    fn now(&self) -> Instant;
}

pub struct SystemClock;
impl Clock for SystemClock {
    fn now(&self) -> Instant { Instant::now() }
}

pub struct MockClock {
    current: Instant,
}
impl MockClock {
    pub fn new() -> Self { Self { current: Instant::now() } }
    pub fn advance(&mut self, d: Duration) { self.current += d; }
}
impl Clock for MockClock {
    fn now(&self) -> Instant { self.current }
}
```

### 4.3 Fs

```rust
use std::path::{Path, PathBuf};

pub trait Fs {
    fn read_to_string(&self, path: &Path) -> Result<String, FsError>;
    fn read_dir(&self, path: &Path) -> Result<Vec<PathBuf>, FsError>;
}

#[derive(Debug, Clone)]
pub struct FsError {
    pub path: PathBuf,
    pub message: String,
}

pub struct RealFs;
impl Fs for RealFs {
    fn read_to_string(&self, path: &Path) -> Result<String, FsError> {
        std::fs::read_to_string(path).map_err(|e| FsError {
            path: path.to_path_buf(), message: e.to_string(),
        })
    }
    fn read_dir(&self, path: &Path) -> Result<Vec<PathBuf>, FsError> {
        std::fs::read_dir(path).map_err(|e| FsError {
            path: path.to_path_buf(), message: e.to_string(),
        })?.map(|e| e.map(|entry| entry.path()).map_err(|e| FsError {
            path: path.to_path_buf(), message: e.to_string(),
        })).collect()
    }
}
```

### 4.4 Logger

```rust
pub trait Logger {
    fn log(&self, level: LogLevel, target: &str, msg: &str);
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum LogLevel { Trace, Debug, Info, Warn, Error }

pub struct NullLogger;
impl Logger for NullLogger {
    fn log(&self, _level: LogLevel, _target: &str, _msg: &str) {}
}
```

### 4.5 Watcher

```rust
use std::path::Path;

/// Abstraction over file-system change detection for hot reload.
///
/// The host provides a Watcher implementation. The engine queries it
/// before each tick (or on a background interval) to decide whether
/// to re-parse the DSL bundle.
pub trait Watcher {
    /// Check if any DSL source files have changed since the last call.
    fn has_changed(&mut self) -> bool;

    /// Return the list of changed files (for incremental reload).
    fn changed_files(&mut self) -> Vec<PathBuf>;
}

#[derive(Debug, Clone)]
pub struct WatcherError {
    pub message: String,
}

/// Poll-based watcher using file modification times.
/// Zero-dependency: does not use inotify/fsevents.
pub struct PollWatcher {
    base_path: PathBuf,
    fs: Box<dyn Fs>,
    last_mtrees: HashMap<PathBuf, SystemTime>,
}

impl PollWatcher {
    pub fn new(base_path: PathBuf, fs: Box<dyn Fs>) -> Self {
        Self { base_path, fs, last_mtrees: HashMap::new() }
    }

    fn scan_mtrees(&self) -> Result<HashMap<PathBuf, SystemTime>, FsError> {
        let mut mtrees = HashMap::new();
        for dir in &[self.base_path.join("trees"), self.base_path.join("subtrees")] {
            for path in self.fs.read_dir(dir)? {
                // RealFs would use std::fs::metadata; MockFs would return pre-set values
                let mtime = self.fs.modified(&path)?;
                mtrees.insert(path, mtime);
            }
        }
        Ok(mtrees)
    }
}

impl Watcher for PollWatcher {
    fn has_changed(&mut self) -> bool {
        match self.scan_mtrees() {
            Ok(current) => {
                let changed = current != self.last_mtrees;
                self.last_mtrees = current;
                changed
            }
            Err(_) => false, // On error, assume no change
        }
    }

    fn changed_files(&mut self) -> Vec<PathBuf> {
        // Compare current mtimes with last_mtrees, return deltas
        vec![]
    }
}
```

---

## 5. AST Design

The AST exactly mirrors the YAML spec: `apiVersion`, `kind`, `metadata`, `spec`.

### 5.1 Tree, Metadata, Bundle

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

### 5.2 Node Enum

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

### 5.4 Command (Output Type)

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

## 6. Evaluator System

Condition nodes and Action `when` guards both use the same `Evaluator` trait.

### 6.1 Trait Definition

```rust
pub(crate) trait Evaluator: std::fmt::Debug + Send + Sync {
    fn evaluate(&self, blackboard: &Blackboard) -> Result<bool, RuntimeError>;
}

/// Registry of evaluator factories.
pub(crate) struct EvaluatorRegistry {
    factories: HashMap<String, EvaluatorFactory>,
}

type EvaluatorFactory = fn(properties: &serde_yaml::Mapping) -> Result<Box<dyn Evaluator>, ParseError>;

impl EvaluatorRegistry {
    pub fn with_builtins() -> Self {
        let mut reg = Self { factories: HashMap::new() };
        reg.register("outputContains", OutputContains::from_yaml);
        reg.register("situationIs", SituationIs::from_yaml);
        reg.register("reflectionRoundUnder", ReflectionRoundUnder::from_yaml);
        reg.register("variableIs", VariableIs::from_yaml);
        reg.register("regex", RegexMatch::from_yaml);
        reg.register("script", ScriptEvaluator::from_yaml);
        reg.register("or", OrEvaluator::from_yaml);
        reg.register("and", AndEvaluator::from_yaml);
        reg
    }

    pub fn register(&mut self, kind: &str, factory: EvaluatorFactory) {
        self.factories.insert(kind.to_string(), factory);
    }

    pub fn create(&self, kind: &str, properties: &serde_yaml::Mapping) -> Result<Box<dyn Evaluator>, ParseError> {
        let factory = self.factories.get(kind)
            .ok_or_else(|| ParseError::UnknownEvaluatorKind { kind: kind.to_string() })?;
        factory(properties)
    }
}
```

### 6.2 Built-in Evaluators

```rust
// --- outputContains ---
#[derive(Debug)]
pub struct OutputContains {
    pub pattern: String,
    pub case_sensitive: bool,
}

impl OutputContains {
    fn from_yaml(props: &serde_yaml::Mapping) -> Result<Box<dyn Evaluator>, ParseError> {
        let pattern = get_string(props, "pattern")?;
        let case_sensitive = get_bool(props, "caseSensitive").unwrap_or(false);
        Ok(Box::new(Self { pattern, case_sensitive }))
    }
}

impl Evaluator for OutputContains {
    fn evaluate(&self, bb: &Blackboard) -> Result<bool, RuntimeError> {
        let output = bb.get_string("provider_output").unwrap_or("");
        Ok(if self.case_sensitive {
            output.contains(&self.pattern)
        } else {
            output.to_lowercase().contains(&self.pattern.to_lowercase())
        })
    }
}

// --- situationIs ---
#[derive(Debug)]
pub struct SituationIs {
    pub situation_type: String,
}

impl Evaluator for SituationIs {
    fn evaluate(&self, bb: &Blackboard) -> Result<bool, RuntimeError> {
        let output = bb.get_string("provider_output").unwrap_or("");
        let summary = bb.get_string("context_summary").unwrap_or("");
        Ok(output.contains(&self.situation_type) || summary.contains(&self.situation_type))
    }
}

// --- reflectionRoundUnder ---
#[derive(Debug)]
pub struct ReflectionRoundUnder {
    pub max: u8,
}

impl Evaluator for ReflectionRoundUnder {
    fn evaluate(&self, bb: &Blackboard) -> Result<bool, RuntimeError> {
        let round = bb.get_u8("reflection_round").unwrap_or(0);
        Ok(round < self.max)
    }
}

// --- variableIs ---
#[derive(Debug)]
pub struct VariableIs {
    pub key: String,
    pub expected: BlackboardValue,
}

impl Evaluator for VariableIs {
    fn evaluate(&self, bb: &Blackboard) -> Result<bool, RuntimeError> {
        match bb.get_path(&self.key) {
            Some(value) => Ok(value == &self.expected),
            None => Ok(false),
        }
    }
}

// --- regex ---
#[derive(Debug)]
pub struct RegexMatch {
    pub re: regex::Regex,
}

impl Evaluator for RegexMatch {
    fn evaluate(&self, bb: &Blackboard) -> Result<bool, RuntimeError> {
        let output = bb.get_string("provider_output").unwrap_or("");
        Ok(self.re.is_match(output))
    }
}

// --- script (Rhai) ---
// Note: If Rhai is too heavy for a zero-dependency crate, we implement
// a minimal expression language instead.
#[derive(Debug)]
pub struct ScriptEvaluator {
    pub script: String,
}

impl Evaluator for ScriptEvaluator {
    fn evaluate(&self, bb: &Blackboard) -> Result<bool, RuntimeError> {
        // V1: Evaluate a minimal expression against blackboard variables.
        // Full Rhai integration can be added as an optional feature.
        evaluate_minimal_script(&self.script, bb)
    }
}

// --- or ---
#[derive(Debug)]
pub struct OrEvaluator {
    pub conditions: Vec<Box<dyn Evaluator>>,
}

impl Evaluator for OrEvaluator {
    fn evaluate(&self, bb: &Blackboard) -> Result<bool, RuntimeError> {
        for cond in &self.conditions {
            if cond.evaluate(bb)? {
                return Ok(true);
            }
        }
        Ok(false)
    }
}

// --- and ---
#[derive(Debug)]
pub struct AndEvaluator {
    pub conditions: Vec<Box<dyn Evaluator>>,
}

impl Evaluator for AndEvaluator {
    fn evaluate(&self, bb: &Blackboard) -> Result<bool, RuntimeError> {
        for cond in &self.conditions {
            if !cond.evaluate(bb)? {
                return Ok(false);
            }
        }
        Ok(true)
    }
}
```

### 6.3 Minimal Script Engine (V1)

For zero-dependency compliance, the `script` evaluator in V1 supports a tiny expression language:

```rust
/// Supported syntax:
///   blackboard.key < 2
///   blackboard.provider_output.contains("claims_completion")
///   blackboard.reflection_round < 2 && blackboard.confidence > 0.8
fn evaluate_minimal_script(script: &str, bb: &Blackboard) -> Result<bool, RuntimeError> {
    // Design: recursive descent parser for a tiny expression language.
    //
    // Grammar:
    //   expr       := term (('&&' | '||') term)*
    //   term       := comparison | '(' expr ')'
    //   comparison := path op literal
    //   op         := '==' | '!=' | '<' | '<=' | '>' | '>='
    //   path       := 'blackboard.' identifier ('.' identifier)*
    //   literal    := string | number | bool
    //
    // Implementation sketch:
    //   1. Tokenize the script into tokens (identifier, operator, literal, punctuation).
    //   2. Parse into an AST using recursive descent.
    //   3. Evaluate the AST against the Blackboard:
    //      - Resolve paths via bb.get_path()
    //      - Compare values using the specified operator.
    //      - Short-circuit && and ||.
    //
    // For V1, script evaluators can also be implemented as a pre-registered
    // custom evaluator if the host provides a full scripting engine (e.g. Rhai).
    Err(RuntimeError::Custom("script evaluator not yet implemented".into()))
}
```

---

## 7. Output Parser System

Prompt nodes use `OutputParser` to turn raw LLM text into structured Blackboard values.

### 7.1 Trait Definition

```rust
pub(crate) trait OutputParser: std::fmt::Debug + Send + Sync {
    fn parse(&self, raw: &str) -> Result<HashMap<String, BlackboardValue>, ParseError>;
}

pub(crate) struct OutputParserRegistry {
    factories: HashMap<String, ParserFactory>,
}

type ParserFactory = fn(properties: &serde_yaml::Mapping) -> Result<Box<dyn OutputParser>, ParseError>;

impl OutputParserRegistry {
    pub fn with_builtins() -> Self {
        let mut reg = Self { factories: HashMap::new() };
        reg.register("enum", EnumParser::from_yaml);
        reg.register("structured", StructuredParser::from_yaml);
        reg.register("json", JsonParser::from_yaml);
        reg.register("command", CommandParser::from_yaml);
        reg
    }

    pub fn register(&mut self, kind: &str, factory: ParserFactory) {
        self.factories.insert(kind.to_string(), factory);
    }

    pub fn create(&self, kind: &str, props: &serde_yaml::Mapping) -> Result<Box<dyn OutputParser>, ParseError> {
        let factory = self.factories.get(kind)
            .ok_or_else(|| ParseError::UnknownParserKind { kind: kind.to_string() })?;
        factory(props)
    }
}
```

### 7.2 Enum Parser

```rust
#[derive(Debug)]
pub struct EnumParser {
    pub allowed_values: Vec<String>,
    pub case_sensitive: bool,
}

impl EnumParser {
    fn from_yaml(props: &serde_yaml::Mapping) -> Result<Box<dyn OutputParser>, ParseError> {
        let values = get_string_array(props, "values")?;
        let case_sensitive = get_bool(props, "caseSensitive").unwrap_or(false);
        Ok(Box::new(Self { allowed_values: values, case_sensitive }))
    }
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

### 7.3 Structured Parser (Regex with Typed Groups)

```rust
#[derive(Debug)]
pub struct StructuredField {
    pub name: String,
    pub group: usize,
    pub ty: FieldType,   // optional type conversion
}

#[derive(Debug, Clone, Copy)]
pub enum FieldType {
    String,
    Integer,
    Float,
    Boolean,
}

#[derive(Debug)]
pub struct StructuredParser {
    pub pattern: regex::Regex,
    pub fields: Vec<StructuredField>,
}

impl StructuredParser {
    fn from_yaml(props: &serde_yaml::Mapping) -> Result<Box<dyn OutputParser>, ParseError> {
        let pattern_str = get_string(props, "pattern")?;
        let pattern = regex::Regex::new(&pattern_str)
            .map_err(|e| ParseError::InvalidProperty {
                key: "pattern".into(),
                value: pattern_str,
                reason: e.to_string(),
            })?;

        let fields = get_structured_fields(props)?;
        Ok(Box::new(Self { pattern, fields }))
    }
}

impl OutputParser for StructuredParser {
    fn parse(&self, raw: &str) -> Result<HashMap<String, BlackboardValue>, ParseError> {
        let caps = self.pattern.captures(raw)
            .ok_or_else(|| ParseError::NoMatch { pattern: self.pattern.as_str().to_string() })?;

        let mut result = HashMap::new();
        for field in &self.fields {
            let m = caps.get(field.group)
                .ok_or_else(|| ParseError::MissingCaptureGroup {
                    group: field.group,
                    pattern: self.pattern.as_str().to_string(),
                })?;
            let value = match field.ty {
                FieldType::String => BlackboardValue::String(m.as_str().to_string()),
                FieldType::Integer => m.as_str().parse::<i64>()
                    .map(BlackboardValue::Integer)
                    .map_err(|_| ParseError::TypeMismatch {
                        field: field.name.clone(), expected: "integer", got: m.as_str().to_string(),
                    })?,
                FieldType::Float => m.as_str().parse::<f64>()
                    .map(BlackboardValue::Float)
                    .map_err(|_| ParseError::TypeMismatch {
                        field: field.name.clone(), expected: "float", got: m.as_str().to_string(),
                    })?,
                FieldType::Boolean => match m.as_str().to_lowercase().as_str() {
                    "true" | "yes" | "1" => BlackboardValue::Boolean(true),
                    "false" | "no" | "0" => BlackboardValue::Boolean(false),
                    _ => return Err(ParseError::TypeMismatch {
                        field: field.name.clone(), expected: "boolean", got: m.as_str().to_string(),
                    }),
                },
            };
            result.insert(field.name.clone(), value);
        }
        Ok(result)
    }
}
```

### 7.4 JSON Parser

```rust
#[derive(Debug)]
pub struct JsonParser {
    pub schema: Option<serde_json::Value>, // Optional JSON Schema
}

impl OutputParser for JsonParser {
    fn parse(&self, raw: &str) -> Result<HashMap<String, BlackboardValue>, ParseError> {
        let json: serde_json::Value = serde_json::from_str(raw)
            .map_err(|e| ParseError::JsonSyntax(e.to_string()))?;

        // Optional schema validation
        if let Some(schema) = &self.schema {
            // jsonschema validation (optional feature)
            // validate_json_schema(&json, schema)?;
        }

        let mut result = HashMap::new();
        if let serde_json::Value::Object(map) = json {
            for (k, v) in map {
                result.insert(k, json_to_blackboard(v)?);
            }
        }
        Ok(result)
    }
}

fn json_to_blackboard(v: serde_json::Value) -> Result<BlackboardValue, ParseError> {
    match v {
        serde_json::Value::String(s) => Ok(BlackboardValue::String(s)),
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                Ok(BlackboardValue::Integer(i))
            } else {
                Ok(BlackboardValue::Float(n.as_f64().unwrap_or(0.0)))
            }
        }
        serde_json::Value::Bool(b) => Ok(BlackboardValue::Boolean(b)),
        serde_json::Value::Array(arr) => {
            let items: Result<Vec<_>, _> = arr.into_iter().map(json_to_blackboard).collect();
            Ok(BlackboardValue::List(items?))
        }
        serde_json::Value::Object(map) => {
            let mut result = HashMap::new();
            for (k, v) in map {
                result.insert(k, json_to_blackboard(v)?);
            }
            Ok(BlackboardValue::Map(result))
        }
        serde_json::Value::Null => Ok(BlackboardValue::String("".to_string())),
    }
}
```

### 7.5 Command Parser

```rust
/// Parses LLM output directly into a Command, bypassing the Blackboard.
#[derive(Debug)]
pub struct CommandParser {
    pub mapping: HashMap<String, CommandMapping>,
}

#[derive(Debug, Clone)]
pub struct CommandMapping {
    pub command: Command,
}

impl OutputParser for CommandParser {
    fn parse(&self, raw: &str) -> Result<HashMap<String, BlackboardValue>, ParseError> {
        let trimmed = raw.trim();
        for (key, mapping) in &self.mapping {
            if trimmed.eq_ignore_ascii_case(key) {
                // Command parsers do NOT return Blackboard values.
                // Instead, they store the command in a special marker.
                // The Prompt node detects this marker and pushes the command directly.
                let mut result = HashMap::new();
                result.insert("__command".to_string(), BlackboardValue::String(
                    serde_json::to_string(&mapping.command).unwrap()
                ));
                return Ok(result);
            }
        }
        Err(ParseError::UnexpectedValue {
            got: trimmed.to_string(),
            expected: self.mapping.keys().cloned().collect(),
        })
    }
}
```

---

## 8. Executor / Tick Loop

### 8.1 Composite Tick Implementation

```rust
impl SelectorNode {
    fn tick(&mut self, ctx: &mut TickContext, tracer: &mut Tracer) -> Result<NodeStatus, RuntimeError> {
        let start = self.active_child.unwrap_or(0);

        for i in start..self.children.len() {
            tracer.enter(self.name(), i);
            let t0 = ctx.clock.now();
            let status = self.children[i].tick(ctx, tracer)?;
            let duration = t0.elapsed();
            tracer.exit(self.name(), i, status, duration);

            match status {
                NodeStatus::Success => {
                    self.active_child = None;
                    return Ok(NodeStatus::Success);
                }
                NodeStatus::Running => {
                    self.active_child = Some(i);
                    return Ok(NodeStatus::Running);
                }
                NodeStatus::Failure => continue,
            }
        }

        self.active_child = None;
        Ok(NodeStatus::Failure)
    }

    fn reset(&mut self) {
        self.active_child = None;
        for child in &mut self.children { child.reset(); }
    }
}

impl SequenceNode {
    fn tick(&mut self, ctx: &mut TickContext, tracer: &mut Tracer) -> Result<NodeStatus, RuntimeError> {
        let start = self.active_child.unwrap_or(0);

        for i in start..self.children.len() {
            tracer.enter(self.name(), i);
            let t0 = ctx.clock.now();
            let status = self.children[i].tick(ctx, tracer)?;
            let duration = t0.elapsed();
            tracer.exit(self.name(), i, status, duration);

            match status {
                NodeStatus::Success => continue,
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
        for child in &mut self.children { child.reset(); }
    }
}

impl ParallelNode {
    fn tick(&mut self, ctx: &mut TickContext, tracer: &mut Tracer) -> Result<NodeStatus, RuntimeError> {
        let mut successes = 0;
        let mut failures = 0;

        for (i, child) in self.children.iter_mut().enumerate() {
            tracer.enter(self.name(), i);
            let t0 = ctx.clock.now();
            let status = child.tick(ctx, tracer)?;
            let duration = t0.elapsed();
            tracer.exit(self.name(), i, status, duration);

            match status {
                NodeStatus::Success => successes += 1,
                NodeStatus::Failure => failures += 1,
                NodeStatus::Running => return Ok(NodeStatus::Running),
            }
        }

        let total = self.children.len();
        let result = match self.policy {
            ParallelPolicy::AllSuccess => {
                if successes == total { NodeStatus::Success } else { NodeStatus::Failure }
            }
            ParallelPolicy::AnySuccess => {
                if successes > 0 { NodeStatus::Success } else { NodeStatus::Failure }
            }
            ParallelPolicy::Majority => {
                if successes > total / 2 { NodeStatus::Success } else { NodeStatus::Failure }
            }
        };
        Ok(result)
    }

    fn reset(&mut self) {
        for child in &mut self.children { child.reset(); }
    }
}
```

### 8.2 Decorator Tick Implementation

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
    fn reset(&mut self) { self.child.reset(); }
}

impl RepeaterNode {
    fn tick(&mut self, ctx: &mut TickContext, tracer: &mut Tracer) -> Result<NodeStatus, RuntimeError> {
        while self.current < self.max_attempts {
            match self.child.tick(ctx, tracer)? {
                NodeStatus::Success => {
                    self.current += 1;
                    if self.current >= self.max_attempts {
                        return Ok(NodeStatus::Success);
                    }
                }
                NodeStatus::Failure => return Ok(NodeStatus::Failure),
                NodeStatus::Running => return Ok(NodeStatus::Running),
            }
        }
        Ok(NodeStatus::Success)
    }
    fn reset(&mut self) { self.current = 0; self.child.reset(); }
}

impl CooldownNode {
    fn tick(&mut self, ctx: &mut TickContext, tracer: &mut Tracer) -> Result<NodeStatus, RuntimeError> {
        if let Some(last) = self.last_success {
            if ctx.clock.now().duration_since(last) < self.duration {
                ctx.logger.log(LogLevel::Debug, "Cooldown",
                    &format!("{}: still on cooldown", self.name));
                return Ok(NodeStatus::Failure);
            }
        }
        let status = self.child.tick(ctx, tracer)?;
        if status == NodeStatus::Success {
            self.last_success = Some(ctx.clock.now());
        }
        Ok(status)
    }
    fn reset(&mut self) { self.last_success = None; self.child.reset(); }
}

impl ReflectionGuardNode {
    fn tick(&mut self, ctx: &mut TickContext, tracer: &mut Tracer) -> Result<NodeStatus, RuntimeError> {
        let count = ctx.blackboard.get_u8("reflection_round").unwrap_or(0);
        if count >= self.max_rounds {
            ctx.logger.log(LogLevel::Info, "ReflectionGuard",
                &format!("{}: max rounds ({}) reached", self.name, self.max_rounds));
            return Ok(NodeStatus::Failure);
        }
        let status = self.child.tick(ctx, tracer)?;
        if status == NodeStatus::Success {
            ctx.blackboard.set_u8("reflection_round", count + 1);
        }
        Ok(status)
    }
    fn reset(&mut self) { self.child.reset(); }
}

impl ForceHumanNode {
    fn tick(&mut self, ctx: &mut TickContext, tracer: &mut Tracer) -> Result<NodeStatus, RuntimeError> {
        let status = self.child.tick(ctx, tracer)?;
        if status == NodeStatus::Success {
            ctx.blackboard.push_command(Command::EscalateToHuman {
                reason: self.reason.clone(),
                context: Some(format!("Forced by decorator after {} succeeded", self.child.name())),
            });
        }
        Ok(status)
    }
    fn reset(&mut self) { self.child.reset(); }
}
```

### 8.3 Leaf Tick Implementation

```rust
impl ConditionNode {
    fn tick(&mut self, ctx: &mut TickContext, tracer: &mut Tracer) -> Result<NodeStatus, RuntimeError> {
        let result = self.evaluator.evaluate(&ctx.blackboard)?;
        tracer.record_eval(self.name(), &self.evaluator, result);
        Ok(if result { NodeStatus::Success } else { NodeStatus::Failure })
    }
    fn reset(&mut self) {}
}

impl ActionNode {
    fn tick(&mut self, ctx: &mut TickContext, tracer: &mut Tracer) -> Result<NodeStatus, RuntimeError> {
        // Check precondition
        if let Some(ref evaluator) = self.when {
            if !evaluator.evaluate(&ctx.blackboard)? {
                ctx.logger.log(LogLevel::Debug, "Action",
                    &format!("{}: precondition failed", self.name));
                return Ok(NodeStatus::Failure);
            }
        }

        // Render command fields that contain templates
        let rendered_cmd = render_command_templates(&self.command, &ctx.blackboard)?;

        ctx.blackboard.push_command(rendered_cmd.clone());
        tracer.record_action(self.name(), &rendered_cmd);
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

impl SubTreeRefNode {
    fn tick(&mut self, _ctx: &mut TickContext, _tracer: &mut Tracer) -> Result<NodeStatus, RuntimeError> {
        // SubTreeRef is resolved at parse time; this should never be called.
        Err(RuntimeError::Custom("SubTreeRef not resolved".into()))
    }
    fn reset(&mut self) {}
}
```

---

## 9. Blackboard Design

The Blackboard is the shared memory of the behavior tree. It matches the spec exactly: built-in variables, custom variables, commands, and LLM responses.

### 9.1 Data Model

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

### 9.2 Unified Access Interface

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

## 10. Prompt Node Implementation

The Prompt node is the most complex leaf. It implements the same-session invariant.

### 10.1 Lifecycle

```
Tick 1: Prompt node renders template → sends to session → returns Running
  ↓
[Host polls; session receives reply]
  ↓
Tick 2: Prompt node checks session.is_ready() → receives reply
        Parses reply into Blackboard values
        Stores raw response in blackboard.llm_responses
        Returns Success or Failure
```

### 10.2 Implementation

```rust
impl PromptNode {
    fn tick(&mut self, ctx: &mut TickContext, tracer: &mut Tracer) -> Result<NodeStatus, RuntimeError> {
        if self.pending {
            // Async continuation
            if !ctx.session.is_ready() {
                ctx.logger.log(LogLevel::Debug, "Prompt",
                    &format!("{}: waiting for reply", self.name));
                return Ok(NodeStatus::Running);
            }

            let reply = ctx.session.send("POLL")?;
            ctx.blackboard.store_llm_response(&self.name, reply.clone());

            match self.parser.parse(&reply) {
                Ok(values) => {
                    // Handle CommandParser special case
                    if let Some(BlackboardValue::String(cmd_json)) = values.get("__command") {
                        let cmd: Command = serde_json::from_str(cmd_json)
                            .map_err(|e| RuntimeError::FilterError(e.to_string()))?;
                        ctx.blackboard.push_command(cmd);
                    } else {
                        // Normal set mapping
                        for mapping in &self.sets {
                            if let Some(value) = values.get(&mapping.field) {
                                ctx.blackboard.set(&mapping.key, value.clone());
                            }
                        }
                    }

                    self.pending = false;
                    tracer.record_prompt_success(&self.name, &reply);
                    return Ok(NodeStatus::Success);
                }
                Err(e) => {
                    ctx.logger.log(LogLevel::Warn, "Prompt",
                        &format!("{}: parse error: {}", self.name, e));
                    self.pending = false;
                    tracer.record_prompt_failure(&self.name, &e.to_string());
                    return Ok(NodeStatus::Failure);
                }
            }
        }

        // First tick — render and send
        let rendered = TemplateEngine::render(&self.template, &ctx.blackboard)?;
        ctx.logger.log(LogLevel::Debug, "Prompt",
            &format!("{}: sending prompt ({} chars)", self.name, rendered.len()));

        ctx.session.send(&rendered)?;
        self.pending = true;
        tracer.record_prompt_sent(&self.name);

        Ok(NodeStatus::Running)
    }

    fn reset(&mut self) {
        self.pending = false;
    }
}
```

### 10.3 Model Selection

The `model` field is stored but not directly used by the DSL engine. The engine passes it through to the host via the `Session` trait if needed:

```rust
// Alternative: extend Session trait with model hint
pub trait Session {
    fn send(&mut self, message: &str) -> Result<String, SessionError>;
    fn send_with_model(&mut self, message: &str, model: &str) -> Result<String, SessionError>;
    fn is_ready(&self) -> bool;
}
```

For V1, we keep `model` as metadata on the node and let the host's `Session` implementation decide whether to honor it.

---

## 11. Template Engine

The template engine supports Jinja2-style syntax with the Blackboard as context.

### 11.1 Supported Syntax

| Feature | Syntax | Status |
|---------|--------|--------|
| Variable interpolation | `{{ variable }}` | ✅ V1 |
| Dot notation | `{{ last_tool_call.name }}` | ✅ V1 |
| Filters | `{{ var \| filter }}` | ✅ V1 |
| Conditionals | `{% if %}`, `{% else %}`, `{% endif %}` | ✅ V1 |
| Loops | `{% for item in list %}` | ✅ V1 |
| Whitespace control | `{%- -%}` | ✅ V1 |
| Comments | `{# comment #}` | ✅ V1 |

### 11.2 Engine Design

The engine is a two-pass system: **lexer** → **render**.

```rust
pub(crate) struct TemplateEngine;

impl TemplateEngine {
    pub fn render(template: &str, bb: &Blackboard) -> Result<String, RuntimeError> {
        let tokens = Self::lex(template)?;
        Self::render_tokens(&tokens, bb)
    }

    fn lex(template: &str) -> Result<Vec<Token>, RuntimeError> {
        let mut tokens = Vec::new();
        let mut chars = template.char_indices().peekable();

        while let Some((i, c)) = chars.next() {
            if c == '{' && chars.peek().map(|(_, n)| *n) == Some('{') {
                chars.next(); // consume second '{'
                // Parse expression until }}
                let (expr, filters) = Self::parse_expression(&mut chars)?;
                tokens.push(Token::Expr { expr, filters });
            } else if c == '{' && chars.peek().map(|(_, n)| *n) == Some('%') {
                chars.next(); // consume '%'
                // Parse statement until %}
                let stmt = Self::parse_statement(&mut chars)?;
                tokens.push(Token::Statement(stmt));
            } else if c == '{' && chars.peek().map(|(_, n)| *n) == Some('#') {
                chars.next(); // consume '#'
                // Skip comment until #}
                Self::skip_comment(&mut chars)?;
            } else {
                // Collect literal text
                let start = i;
                let mut end = i + c.len_utf8();
                while let Some((j, ch)) = chars.peek() {
                    if *ch == '{' || *ch == '%' || *ch == '#' {
                        break;
                    }
                    end = *j + ch.len_utf8();
                    chars.next();
                }
                tokens.push(Token::Literal(template[start..end].to_string()));
            }
        }

        Ok(tokens)
    }
}
```

### 11.3 Token Types

```rust
enum Token {
    Literal(String),
    Expr { expr: String, filters: Vec<FilterCall> },
    Statement(Stmt),
}

struct FilterCall {
    name: String,
    args: Vec<String>,
}

enum Stmt {
    If { condition: String, then_body: Vec<Token>, else_body: Option<Vec<Token>> },
    For { var: String, iter: String, body: Vec<Token> },
}
```

### 11.4 Filter Registry

```rust
pub(crate) struct FilterRegistry {
    filters: HashMap<String, FilterFn>,
}

type FilterFn = fn(value: &BlackboardValue, args: &[String]) -> Result<BlackboardValue, RuntimeError>;

impl FilterRegistry {
    pub fn with_builtins() -> Self {
        let mut reg = Self { filters: HashMap::new() };
        reg.register("upper", |v, _| Ok(BlackboardValue::String(v.to_string().to_uppercase())));
        reg.register("lower", |v, _| Ok(BlackboardValue::String(v.to_string().to_lowercase())));
        reg.register("truncate", |v, args| {
            let n: usize = args.get(0).and_then(|s| s.parse().ok()).unwrap_or(100);
            let s = v.to_string();
            if s.len() > n {
                Ok(BlackboardValue::String(format!("{}...", &s[..n])))
            } else {
                Ok(BlackboardValue::String(s))
            }
        });
        reg.register("length", |v, _| {
            let len = match v {
                BlackboardValue::String(s) => s.len(),
                BlackboardValue::List(l) => l.len(),
                BlackboardValue::Map(m) => m.len(),
                _ => 0,
            };
            Ok(BlackboardValue::Integer(len as i64))
        });
        reg.register("default", |v, args| {
            match v {
                BlackboardValue::String(s) if s.is_empty() => {
                    Ok(BlackboardValue::String(args.get(0).cloned().unwrap_or_default()))
                }
                _ => Ok(v.clone()),
            }
        });
        reg.register("join", |v, args| {
            match v {
                BlackboardValue::List(l) => {
                    let sep = args.get(0).cloned().unwrap_or(", ".to_string());
                    let joined = l.iter().map(|item| item.to_string()).collect::<Vec<_>>().join(&sep);
                    Ok(BlackboardValue::String(joined))
                }
                _ => Ok(BlackboardValue::String(v.to_string())),
            }
        });
        reg.register("json", |v, _| {
            serde_json::to_string(v).map(BlackboardValue::String)
                .map_err(|e| RuntimeError::FilterError(e.to_string()))
        });
        reg.register("slugify", |v, _| {
            let s = v.to_string().to_lowercase()
                .replace(" ", "-")
                .replace("_", "-")
                .replace(|c: char| !c.is_alphanumeric() && c != '-', "");
            Ok(BlackboardValue::String(s))
        });
        reg
    }

    pub fn register(&mut self, name: &str, f: FilterFn) {
        self.filters.insert(name.to_string(), f);
    }
}
```

### 11.5 Expression Evaluation

```rust
impl TemplateEngine {
    fn eval_expr(expr: &str, bb: &Blackboard) -> Result<BlackboardValue, RuntimeError> {
        let trimmed = expr.trim();
        // Simple path lookup: "task_description", "variables.next_action", "last_tool_call.name"
        bb.get_path(trimmed)
            .ok_or_else(|| RuntimeError::MissingVariable { key: trimmed.to_string() })
    }

    fn apply_filters(
        value: BlackboardValue,
        filters: &[FilterCall],
        registry: &FilterRegistry,
    ) -> Result<BlackboardValue, RuntimeError> {
        let mut current = value;
        for filter in filters {
            let f = registry.filters.get(&filter.name)
                .ok_or_else(|| RuntimeError::UnknownFilter { filter: filter.name.clone() })?;
            current = f(&current, &filter.args)?;
        }
        Ok(current)
    }
}
```

### 11.6 Render Implementation

```rust
impl TemplateEngine {
    fn render_tokens(tokens: &[Token], bb: &Blackboard) -> Result<String, RuntimeError> {
        let mut output = String::new();
        let mut i = 0;
        while i < tokens.len() {
            match &tokens[i] {
                Token::Literal(text) => output.push_str(text),
                Token::Expr { expr, filters } => {
                    let value = Self::eval_expr(expr, bb)?;
                    let value = Self::apply_filters(value, filters, &FilterRegistry::with_builtins())?;
                    output.push_str(&value.to_string());
                }
                Token::Statement(Stmt::If { condition, then_body, else_body }) => {
                    let cond_value = Self::eval_condition(condition, bb)?;
                    let body = if cond_value { then_body } else { else_body.as_ref().unwrap_or(&vec![]) };
                    output.push_str(&Self::render_tokens(body, bb)?);
                }
                Token::Statement(Stmt::For { var, iter, body }) => {
                    let iter_value = Self::eval_expr(iter, bb)?;
                    match iter_value {
                        BlackboardValue::List(list) => {
                            for item in list {
                                // Create a temporary scope with the loop variable
                                let mut scoped_bb = bb.clone();
                                scoped_bb.set(var, item);
                                output.push_str(&Self::render_tokens(body, &scoped_bb)?);
                            }
                        }
                        _ => return Err(RuntimeError::FilterError(
                            format!("cannot iterate over {}", iter)
                        )),
                    }
                }
            }
            i += 1;
        }
        Ok(output)
    }

    fn eval_condition(condition: &str, bb: &Blackboard) -> Result<bool, RuntimeError> {
        // Simple condition evaluation: "reflection_round > 0", "file_changes | length > 0"
        // V1: Support simple comparisons and boolean variable lookups
        let trimmed = condition.trim();
        if let Ok(val) = Self::eval_expr(trimmed, bb) {
            return Ok(match val {
                BlackboardValue::Boolean(b) => b,
                BlackboardValue::Integer(0) | BlackboardValue::Float(0.0) => false,
                BlackboardValue::String(s) if s.is_empty() => false,
                _ => true,
            });
        }
        // Parse comparison: "left op right"
        Self::eval_comparison(trimmed, bb)
    }
}
```

---

## 12. Action Command Template Rendering

Action nodes support template interpolation inside command fields. This is a second render pass that runs during Action tick.

### 12.1 Command Rendering

```rust
/// Recursively render all String fields in a Command using the Blackboard.
fn render_command_templates(cmd: &Command, bb: &Blackboard) -> Result<Command, RuntimeError> {
    match cmd {
        Command::RetryTool { tool_name, args, max_attempts } => Ok(Command::RetryTool {
            tool_name: TemplateEngine::render(tool_name, bb)?,
            args: args.as_ref().map(|a| TemplateEngine::render(a, bb)).transpose()?,
            max_attempts: *max_attempts,
        }),
        Command::SendCustomInstruction { prompt, target_agent } => Ok(Command::SendCustomInstruction {
            prompt: TemplateEngine::render(prompt, bb)?,
            target_agent: TemplateEngine::render(target_agent, bb)?,
        }),
        Command::EscalateToHuman { reason, context } => Ok(Command::EscalateToHuman {
            reason: TemplateEngine::render(reason, bb)?,
            context: context.as_ref().map(|c| TemplateEngine::render(c, bb)).transpose()?,
        }),
        Command::Reflect { prompt } => Ok(Command::Reflect {
            prompt: TemplateEngine::render(prompt, bb)?,
        }),
        Command::StopIfComplete { reason } => Ok(Command::StopIfComplete {
            reason: TemplateEngine::render(reason, bb)?,
        }),
        Command::PrepareTaskStart { task_id, task_description } => Ok(Command::PrepareTaskStart {
            task_id: TemplateEngine::render(task_id, bb)?,
            task_description: TemplateEngine::render(task_description, bb)?,
        }),
        Command::SuggestCommit { message, mandatory, reason } => Ok(Command::SuggestCommit {
            message: TemplateEngine::render(message, bb)?,
            mandatory: *mandatory,
            reason: TemplateEngine::render(reason, bb)?,
        }),
        Command::CommitChanges { message, is_wip, worktree_path } => Ok(Command::CommitChanges {
            message: TemplateEngine::render(message, bb)?,
            is_wip: *is_wip,
            worktree_path: worktree_path.as_ref().map(|p| TemplateEngine::render(p, bb)).transpose()?,
        }),
        Command::CreateTaskBranch { branch_name, base_branch, worktree_path } => {
            Ok(Command::CreateTaskBranch {
                branch_name: TemplateEngine::render(branch_name, bb)?,
                base_branch: TemplateEngine::render(base_branch, bb)?,
                worktree_path: worktree_path.as_ref().map(|p| TemplateEngine::render(p, bb)).transpose()?,
            })
        }
        Command::RebaseToMain { base_branch } => Ok(Command::RebaseToMain {
            base_branch: TemplateEngine::render(base_branch, bb)?,
        }),
        // Commands with no string fields pass through unchanged
        other => Ok(other.clone()),
    }
}
```

---

## 13. Error Handling

All errors are explicit enums. No panics in library code.

### 13.1 Error Types

```rust
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

#[derive(Debug, Clone)]
pub enum ParseError {
    YamlSyntax(String),
    UnknownNodeKind { kind: String },
    UnknownEvaluatorKind { kind: String },
    UnknownParserKind { kind: String },
    MissingProperty(&'static str),
    InvalidProperty { key: String, value: String, reason: String },
    UnresolvedSubTree { name: String },
    CircularSubTreeRef { name: String },
    DuplicateName { name: String },
    UnexpectedValue { got: String, expected: Vec<String> },
    NoMatch { pattern: String },
    MissingCaptureGroup { group: usize, pattern: String },
    TypeMismatch { field: String, expected: &'static str, got: String },
    JsonSyntax(String),
    UnsupportedVersion(String),
    Custom(String),
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ParseError::YamlSyntax(e) => write!(f, "YAML syntax error: {}", e),
            ParseError::UnknownNodeKind { kind } => write!(f, "unknown node kind: {}", kind),
            ParseError::UnknownEvaluatorKind { kind } => write!(f, "unknown evaluator kind: {}", kind),
            ParseError::UnknownParserKind { kind } => write!(f, "unknown parser kind: {}", kind),
            ParseError::MissingProperty(p) => write!(f, "missing required property: {}", p),
            ParseError::InvalidProperty { key, value, reason } => {
                write!(f, "invalid property '{}' = '{}': {}", key, value, reason)
            }
            ParseError::UnresolvedSubTree { name } => write!(f, "unresolved subtree reference: {}", name),
            ParseError::CircularSubTreeRef { name } => write!(f, "circular subtree reference: {}", name),
            ParseError::DuplicateName { name } => write!(f, "duplicate node name: {}", name),
            ParseError::UnexpectedValue { got, expected } => {
                write!(f, "unexpected value '{}', expected one of: {:?}", got, expected)
            }
            ParseError::NoMatch { pattern } => write!(f, "no match for pattern: {}", pattern),
            ParseError::MissingCaptureGroup { group, pattern } => {
                write!(f, "missing capture group {} in pattern: {}", group, pattern)
            }
            ParseError::TypeMismatch { field, expected, got } => {
                write!(f, "type mismatch for field '{}': expected {}, got {}", field, expected, got)
            }
            ParseError::JsonSyntax(e) => write!(f, "JSON syntax error: {}", e),
            ParseError::UnsupportedVersion(v) => write!(f, "unsupported api version: {}", v),
            ParseError::Custom(msg) => write!(f, "{}", msg),
        }
    }
}

impl std::error::Error for ParseError {}

#[derive(Debug, Clone)]
pub enum RuntimeError {
    MissingVariable { key: String },
    UnknownFilter { filter: String },
    FilterError(String),
    TypeMismatch { key: String, expected: &'static str, got: String },
    Session { kind: SessionErrorKind, message: String },
    MaxRecursion,
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

### 13.2 Error Conversion

```rust
impl From<serde_yaml::Error> for ParseError {
    fn from(e: serde_yaml::Error) -> Self { ParseError::YamlSyntax(e.to_string()) }
}

impl From<SessionError> for RuntimeError {
    fn from(e: SessionError) -> Self {
        RuntimeError::Session { kind: e.kind, message: e.message }
    }
}
```

---

## 14. Testing Strategy

Every component is tested with mock trait implementations. No I/O, no LLM calls in unit tests.

### 14.1 Mock Implementations

```rust
// test-support utilities (within decision-dsl/tests/ or test-support crate)

use decision_dsl::ext::*;
use std::cell::RefCell;
use std::collections::VecDeque;

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
    pub fn set_ready(&self, ready: bool) { *self.ready.borrow_mut() = ready; }
}

impl Session for MockSession {
    fn send(&mut self, _message: &str) -> Result<String, SessionError> {
        self.replies.borrow_mut().pop_front().ok_or_else(|| SessionError {
            kind: SessionErrorKind::Unavailable,
            message: "no more replies".into(),
        })
    }
    fn is_ready(&self) -> bool { *self.ready.borrow() }
}

pub struct CaptureLogger {
    logs: RefCell<Vec<(LogLevel, String, String)>>,
}

impl Logger for CaptureLogger {
    fn log(&self, level: LogLevel, target: &str, msg: &str) {
        self.logs.borrow_mut().push((level, target.to_string(), msg.to_string()));
    }
}
```

### 14.2 Unit Tests: Evaluators

```rust
#[test]
fn output_contains_case_insensitive() {
    let mut bb = Blackboard::new();
    bb.provider_output = "Error 429: Rate Limit".into();

    let eval = OutputContains { pattern: "429".into(), case_sensitive: false };
    assert!(eval.evaluate(&bb).unwrap());

    let eval = OutputContains { pattern: "rate limit".into(), case_sensitive: false };
    assert!(eval.evaluate(&bb).unwrap());

    let eval = OutputContains { pattern: "quota".into(), case_sensitive: false };
    assert!(!eval.evaluate(&bb).unwrap());
}

#[test]
fn variable_is_matches() {
    let mut bb = Blackboard::new();
    bb.set("next_action", BlackboardValue::String("REFLECT".into()));

    let eval = VariableIs {
        key: "variables.next_action".into(),
        expected: BlackboardValue::String("REFLECT".into()),
    };
    assert!(eval.evaluate(&bb).unwrap());
}

#[test]
fn or_evaluator_short_circuits() {
    let mut bb = Blackboard::new();
    bb.set("a", BlackboardValue::Boolean(false));
    bb.set("b", BlackboardValue::Boolean(true));

    let eval = OrEvaluator {
        conditions: vec![
            Box::new(VariableIs { key: "variables.a".into(), expected: BlackboardValue::Boolean(true) }),
            Box::new(VariableIs { key: "variables.b".into(), expected: BlackboardValue::Boolean(true) }),
        ],
    };
    assert!(eval.evaluate(&bb).unwrap());
}
```

### 14.3 Unit Tests: Output Parsers

```rust
#[test]
fn enum_parser_case_insensitive() {
    let parser = EnumParser {
        allowed_values: vec!["REFLECT".into(), "CONFIRM".into()],
        case_sensitive: false,
    };
    let result = parser.parse("  reflect  ").unwrap();
    assert_eq!(result.get("decision"), Some(&BlackboardValue::String("REFLECT".into())));
}

#[test]
fn structured_parser_with_types() {
    let parser = StructuredParser {
        pattern: regex::Regex::new(r"CLASS:\s*(\w+)\s*RECOMMEND:\s*(\w+)\s*REASON:\s*(.*)").unwrap(),
        fields: vec![
            StructuredField { name: "classification".into(), group: 1, ty: FieldType::String },
            StructuredField { name: "recommendation".into(), group: 2, ty: FieldType::String },
            StructuredField { name: "reason".into(), group: 3, ty: FieldType::String },
        ],
    };
    let input = "CLASS: SYNTAX RECOMMEND: FIX REASON: Missing semicolon";
    let result = parser.parse(input).unwrap();
    assert_eq!(result.get("classification"), Some(&BlackboardValue::String("SYNTAX".into())));
    assert_eq!(result.get("recommendation"), Some(&BlackboardValue::String("FIX".into())));
}

#[test]
fn json_parser_parses_nested() {
    let parser = JsonParser { schema: None };
    let input = r#"{"decision": "reflect", "confidence": 0.82}"#;
    let result = parser.parse(input).unwrap();
    assert_eq!(result.get("decision"), Some(&BlackboardValue::String("reflect".into())));
    assert_eq!(result.get("confidence"), Some(&BlackboardValue::Float(0.82)));
}
```

### 14.4 Integration Tests: Full Tree Tick

```rust
#[test]
fn full_tree_escalates_to_human() {
    let yaml = r#"
apiVersion: decision.agile-agent.io/v1
kind: BehaviorTree
metadata:
  name: test-tree
spec:
  root:
    kind: Selector
    name: root
    children:
      - kind: Sequence
        name: handle_choice
        children:
          - kind: Condition
            name: is_waiting
            eval:
              kind: outputContains
              pattern: "waiting for choice"
          - kind: Action
            name: select_first
            command:
              SelectOption:
                option_id: "0"
      - kind: Action
        name: default_continue
        command: ApproveAndContinue
"#;

    let parser = YamlParser::new();
    let tree = parser.parse_tree(yaml).unwrap();

    let mut bb = Blackboard::new();
    bb.provider_output = "Please choose: A or B (waiting for choice)".into();

    let mut executor = Executor::new();
    let mut session = MockSession::new(vec![]);
    let clock = SystemClock;
    let fs = RealFs;
    let logger = CaptureLogger { logs: RefCell::new(vec![]) };

    let mut ctx = TickContext::new(&mut bb, &mut session, &clock, &fs, &logger);
    let result = executor.tick(&tree, &mut ctx).unwrap();

    assert_eq!(result.status, NodeStatus::Success);
    assert_eq!(result.commands.len(), 1);
    match &result.commands[0] {
        Command::SelectOption { option_id } => assert_eq!(option_id, "0"),
        other => panic!("expected SelectOption, got {:?}", other),
    }
}

#[test]
fn reflect_loop_with_prompt() {
    let yaml = r#"
apiVersion: decision.agile-agent.io/v1
kind: BehaviorTree
metadata:
  name: reflect-test
spec:
  root:
    kind: Sequence
    name: reflect_flow
    children:
      - kind: ReflectionGuard
        name: max_2
        maxRounds: 2
        child:
          kind: Prompt
          name: ask
            template: "REFLECT or CONFIRM?"
            parser:
              kind: enum
              values: [REFLECT, CONFIRM]
            sets:
              - key: next_action
                field: decision
      - kind: Selector
        name: branch
        children:
          - kind: Sequence
            name: do_reflect
            children:
              - kind: Condition
                name: is_reflect
                eval:
                  kind: variableIs
                  key: next_action
                  value: REFLECT
              - kind: Action
                name: emit_reflect
                command:
                  Reflect:
                    prompt: "Review your work"
          - kind: Sequence
            name: do_confirm
            children:
              - kind: Condition
                name: is_confirm
                eval:
                  kind: variableIs
                  key: next_action
                  value: CONFIRM
              - kind: Action
                name: emit_confirm
                command: ConfirmCompletion
"#;

    let parser = YamlParser::new();
    let tree = parser.parse_tree(yaml).unwrap();

    let mut bb = Blackboard::new();
    bb.reflection_round = 0;

    let mut executor = Executor::new();
    let mut session = MockSession::new(vec!["REFLECT".into()]);
    let clock = SystemClock;
    let fs = RealFs;
    let logger = CaptureLogger { logs: RefCell::new(vec![]) };

    // Tick 1: Prompt sends, returns Running
    let mut ctx = TickContext::new(&mut bb, &mut session, &clock, &fs, &logger);
    let result = executor.tick(&tree, &mut ctx).unwrap();
    assert_eq!(result.status, NodeStatus::Running);

    // Tick 2: Session ready, parse succeeds
    let mut ctx = TickContext::new(&mut bb, &mut session, &clock, &fs, &logger);
    let result = executor.tick(&tree, &mut ctx).unwrap();
    assert_eq!(result.status, NodeStatus::Success);
    assert_eq!(bb.reflection_round, 1);
    assert_eq!(result.commands.len(), 1);
    match &result.commands[0] {
        Command::Reflect { prompt } => assert_eq!(prompt, "Review your work"),
        other => panic!("expected Reflect, got {:?}", other),
    }
}
```

### 14.5 Template Engine Tests

```rust
#[test]
fn template_truncate_filter() {
    let mut bb = Blackboard::new();
    bb.provider_output = "a".repeat(1000);

    let template = "{{ provider_output | truncate(100) }}";
    let result = TemplateEngine::render(template, &bb).unwrap();
    assert_eq!(result.len(), 103); // 100 chars + "..."
    assert!(result.ends_with("..."));
}

#[test]
fn template_conditional() {
    let mut bb = Blackboard::new();
    bb.reflection_round = 1;

    let template = r#"
{% if reflection_round > 0 %}
This is reflection round {{ reflection_round }}.
{% else %}
Initial decision.
{% endif %}
"#;
    let result = TemplateEngine::render(template, &bb).unwrap();
    assert!(result.contains("reflection round 1"));
    assert!(!result.contains("Initial decision"));
}

#[test]
fn template_for_loop() {
    let mut bb = Blackboard::new();
    bb.file_changes = vec![
        FileChangeRecord { path: "a.py".into(), change_type: "modified".into() },
        FileChangeRecord { path: "b.py".into(), change_type: "added".into() },
    ];

    let template = r#"
{% if file_changes | length > 0 %}
Changes:
{% for change in file_changes %}
- {{ change.path }} ({{ change.change_type }})
{% endfor %}
{% endif %}
"#;
    let result = TemplateEngine::render(template, &bb).unwrap();
    assert!(result.contains("a.py (modified)"));
    assert!(result.contains("b.py (added)"));
}

#[test]
fn template_default_filter() {
    let mut bb = Blackboard::new();
    // last_tool_call is None

    let template = r#"{{ last_tool_call.name | default("unknown") }}"#;
    let result = TemplateEngine::render(template, &bb).unwrap();
    assert_eq!(result, "unknown");
}

#[test]
fn template_slugify_filter() {
    let mut bb = Blackboard::new();
    bb.current_task_id = "Implement user auth".into();

    let template = r#"feature/{{ current_task_id | slugify }}"#;
    let result = TemplateEngine::render(template, &bb).unwrap();
    assert_eq!(result, "feature/implement-user-auth");
}
```

### 14.6 Test Coverage Requirements

| Component | Coverage Target |
|-----------|----------------|
| Parser | 95% — all node kinds, error paths, YAML structures |
| Evaluators | 100% — all builtin evaluators |
| Output Parsers | 100% — enum, structured, json, command |
| Executor / Tick | 95% — all node types, resume paths |
| Blackboard | 100% — all types, dot notation, built-in fields |
| Template Engine | 95% — all filters, conditionals, loops, missing vars |
| Prompt Node | 90% — pending, ready, parse fail, command parser |
| Error Types | 100% — Display, From impls |

---

## 15. Performance Considerations

### 15.1 Allocation Strategy

```rust
// Hot path: tick loop. Minimize allocations.

// Commands: pre-allocate Vec capacity
impl Blackboard {
    pub fn with_capacity(n: usize) -> Self {
        Self {
            commands: Vec::with_capacity(8),
            variables: HashMap::with_capacity(n),
            ..Default::default()
        }
    }
}

// Template engine: compile regex once per parser
// StructuredParser compiles regex at parse time, not tick time.
```

### 15.2 Tick Loop Hot Path

```rust
// Benchmark target: 1M ticks/second on a 100-node composite tree
// (measured with criterion.rs)
// Only Prompt nodes allocate during tick.
```

### 15.3 Memory Footprint

```rust
// Tree memory: one-time parse cost.
// Runtime memory: Blackboard + running path.
//
// For a tree with 100 nodes:
//   - Parse: ~50KB
//   - Runtime: ~2KB (blackboard) + path Vec
//   - Per-tick: 0 allocations (no Prompt), ~1KB (with Prompt)
```

### 15.4 Parallel Safety

```rust
// Trees are Send + Sync (no internal mutability in Tree itself).
// Executor is !Sync (mutates running_path).
// Blackboard is !Sync (mutable during tick).
//
// Design: One Executor + Blackboard per agent thread.
```

---

## 16. Validation & Hot Reload

### 16.1 Validation Rules

```rust
impl Tree {
    /// Check apiVersion is supported.
    pub fn validate_api_version(&self) -> Result<(), ParseError> {
        if self.api_version != "decision.agile-agent.io/v1" {
            return Err(ParseError::UnsupportedVersion(self.api_version.clone()));
        }
        Ok(())
    }

    /// All node names must be unique within the tree.
    pub fn validate_unique_names(&self) -> Result<(), ParseError> {
        let mut seen = HashSet::new();
        self.spec.root.validate_unique_names_recursive(&mut seen)
    }

    /// All evaluator kinds must be registered.
    pub fn validate_evaluators(&self, registry: &EvaluatorRegistry) -> Result<(), ParseError> {
        self.spec.root.validate_evaluators_recursive(registry)
    }

    /// All parser kinds must be registered.
    pub fn validate_parsers(&self, registry: &OutputParserRegistry) -> Result<(), ParseError> {
        self.spec.root.validate_parsers_recursive(registry)
    }
}
```

### 16.2 Hot Reload

Hot reload uses the `Watcher` trait (see §4.5). The engine queries the watcher before each tick; if files changed, it re-parses the bundle.

```rust
use std::sync::{Arc, RwLock};
use decision_dsl::ext::Watcher;

pub struct DslReloader {
    parser: YamlParser,
    fs: Box<dyn Fs>,
    watcher: Box<dyn Watcher>,
    current_bundle: Arc<RwLock<Bundle>>,
}

impl DslReloader {
    pub fn new(
        parser: YamlParser,
        fs: Box<dyn Fs>,
        watcher: Box<dyn Watcher>,
    ) -> Self {
        Self {
            parser,
            fs,
            watcher,
            current_bundle: Arc::new(RwLock::new(Bundle::default())),
        }
    }

    pub fn load(&mut self, base_path: &Path) -> Result<(), DslError> {
        let bundle = self.parser.parse_bundle(base_path, self.fs.as_ref())?;
        *self.current_bundle.write().unwrap() = bundle;
        Ok(())
    }

    /// Call before each tick. Returns true if bundle was reloaded.
    pub fn reload_if_changed(&mut self, base_path: &Path) -> Result<bool, DslError> {
        if !self.watcher.has_changed() {
            return Ok(false);
        }

        match self.load(base_path) {
            Ok(()) => Ok(true),
            Err(e) => {
                // Log error, keep old bundle running
                // Host decides whether to surface the error
                Err(e)
            }
        }
    }

    pub fn current_bundle(&self) -> Arc<RwLock<Bundle>> {
        self.current_bundle.clone()
    }
}
```

**Hot reload semantics**:
- In-flight decisions use the old tree.
- New decisions use the reloaded tree.
- If reload fails, the old tree continues to run. An error is returned to the host.

---

## 17. Observability

### 17.1 NodeTrace

```rust
#[derive(Debug, Clone)]
pub struct NodeTrace {
    pub node_name: String,
    pub node_type: String,
    pub depth: usize,
    pub status: NodeStatus,
    pub duration: std::time::Duration,
    pub blackboard_reads: Vec<String>,
    pub blackboard_writes: Vec<(String, BlackboardValue)>,
    pub commands_emitted: Vec<Command>,
    pub llm_latency_ms: Option<u64>,
    pub llm_tokens: Option<(u32, u32)>, // (prompt, completion)
}

#[derive(Debug, Clone)]
pub enum TraceEntry {
    Enter { node_name: String, child_index: usize, depth: usize },
    Exit { node_name: String, status: NodeStatus, duration: std::time::Duration },
    Eval { node_name: String, evaluator: String, result: bool },
    Action { node_name: String, command: Command },
    PromptSent { node_name: String },
    PromptSuccess { node_name: String, response: String },
    PromptFailure { node_name: String, error: String },
    Log(String),
}
```

### 17.2 Tracer Implementation

```rust
pub(crate) struct Tracer {
    entries: Vec<TraceEntry>,
    running_path: Vec<usize>,
    current_depth: usize,
}

impl Tracer {
    pub fn new() -> Self {
        Self { entries: Vec::new(), running_path: Vec::new(), current_depth: 0 }
    }

    pub fn enter(&mut self, node_name: &str, child_index: usize) {
        self.entries.push(TraceEntry::Enter {
            node_name: node_name.to_string(),
            child_index,
            depth: self.current_depth,
        });
        self.current_depth += 1;
    }

    pub fn exit(&mut self, node_name: &str, status: NodeStatus, duration: std::time::Duration) {
        self.current_depth -= 1;
        self.entries.push(TraceEntry::Exit {
            node_name: node_name.to_string(),
            status,
            duration,
        });
        if status == NodeStatus::Running {
            self.running_path.push(child_index); // tracked externally
        }
    }

    pub fn record_eval(&mut self, node_name: &str, evaluator: &dyn Evaluator, result: bool) {
        self.entries.push(TraceEntry::Eval {
            node_name: node_name.to_string(),
            evaluator: format!("{:?}", evaluator),
            result,
        });
    }

    pub fn record_action(&mut self, node_name: &str, command: &Command) {
        self.entries.push(TraceEntry::Action {
            node_name: node_name.to_string(),
            command: command.clone(),
        });
    }

    pub fn record_prompt_sent(&mut self, node_name: &str) {
        self.entries.push(TraceEntry::PromptSent { node_name: node_name.to_string() });
    }

    pub fn record_prompt_success(&mut self, node_name: &str, response: &str) {
        self.entries.push(TraceEntry::PromptSuccess {
            node_name: node_name.to_string(),
            response: response.to_string(),
        });
    }

    pub fn record_prompt_failure(&mut self, node_name: &str, error: &str) {
        self.entries.push(TraceEntry::PromptFailure {
            node_name: node_name.to_string(),
            error: error.to_string(),
        });
    }

    pub fn log(&mut self, msg: String) {
        self.entries.push(TraceEntry::Log(msg));
    }

    pub fn running_path(&self) -> &[usize] { &self.running_path }
    pub fn into_entries(self) -> Vec<TraceEntry> { self.entries }
}
```

### 17.3 Tree Visualization

```rust
/// Render trace entries as ASCII art for TUI display.
pub fn render_trace_as_ascii(trace: &[TraceEntry]) -> String {
    let mut output = String::new();
    let mut stack: Vec<(String, NodeStatus, std::time::Duration)> = Vec::new();

    for entry in trace {
        match entry {
            TraceEntry::Enter { node_name, depth, .. } => {
                let indent = "  ".repeat(*depth);
                output.push_str(&format!("{}[{}] ", indent, node_name));
            }
            TraceEntry::Exit { node_name, status, duration } => {
                let symbol = match status {
                    NodeStatus::Success => "✓",
                    NodeStatus::Failure => "✗",
                    NodeStatus::Running => "…",
                };
                output.push_str(&format!("{} — {} ({:?})\n", symbol, node_name, duration));
            }
            TraceEntry::PromptSuccess { node_name, .. } => {
                output.push_str(&format!("  [LLM] {} → parsed\n", node_name));
            }
            TraceEntry::Action { command, .. } => {
                output.push_str(&format!("  [CMD] {:?}\n", command));
            }
            _ => {}
        }
    }
    output
}
```

### 17.4 Metrics

```rust
/// Optional metrics integration (host-provided).
pub trait MetricsCollector {
    fn record_tick(&self, tree_name: &str, duration: std::time::Duration);
    fn record_node(&self, node_name: &str, node_type: &str, status: NodeStatus, duration: std::time::Duration);
    fn record_prompt(&self, node_name: &str, model: &str, latency_ms: u64, prompt_tokens: u32, completion_tokens: u32);
}
```

---

## 18. Integration Example

### 18.1 Host Integration: agent-decision

```rust
use decision_dsl::{DslParser, DslRunner, YamlParser, Executor, TickContext};
use decision_dsl::{Blackboard, BlackboardValue};
use decision_dsl::ext::{Session, Clock, Fs, Logger, RealFs, SystemClock};

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
            .or_else(|| self.bundle.trees.get("default"))
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
            return Ok(vec![Command::SkipDecision]);
        }

        Ok(result.commands)
    }
}
```

### 18.2 Building the Blackboard from Agent State

```rust
fn build_blackboard(agent_state: &AgentState) -> Blackboard {
    let mut bb = Blackboard::new();
    bb.task_description = agent_state.task.description.clone();
    bb.provider_output = agent_state.last_provider_output.clone();
    bb.context_summary = agent_state.context_summary();
    bb.reflection_round = agent_state.decision_state.reflection_round;
    bb.max_reflection_rounds = agent_state.decision_state.max_reflection_rounds;
    bb.confidence_accumulator = agent_state.decision_state.confidence_accumulator;
    bb.agent_id = agent_state.id.to_string();
    bb.current_task_id = agent_state.current_task_id.clone().unwrap_or_default();
    bb.current_story_id = agent_state.current_story_id.clone().unwrap_or_default();
    bb.last_tool_call = agent_state.last_tool_call.clone();
    bb.file_changes = agent_state.recent_file_changes();
    bb.decision_history = agent_state.decision_history();
    bb
}
```

### 18.3 Complete End-to-End Example

```rust
use decision_dsl::*;

fn main() -> Result<(), DslError> {
    // 1. Parse tree
    let yaml = include_str!("tree.yaml");
    let parser = YamlParser::new();
    let tree = parser.parse_tree(yaml)?;

    // 2. Set up blackboard
    let mut bb = Blackboard::new();
    bb.task_description = "Implement auth".into();
    bb.provider_output = "Task complete! All tests passing.".into();
    bb.reflection_round = 0;
    bb.max_reflection_rounds = 2;

    // 3. Set up mocks
    let mut session = MockSession::new(vec![
        "is_complete: true\nconfidence: 0.95".into(),
    ]);
    let clock = SystemClock;
    let fs = RealFs;
    let logger = NullLogger;

    // 4. Create runner
    let mut executor = Executor::new();

    // 5. Tick
    let mut ctx = TickContext::new(&mut bb, &mut session, &clock, &fs, &logger);
    let result = executor.tick(&tree, &mut ctx)?;

    println!("Status: {:?}", result.status);
    for cmd in &result.commands {
        println!("Command: {:?}", cmd);
    }
    for entry in &result.trace {
        println!("Trace: {:?}", entry);
    }

    Ok(())
}
```

---

## 19. Migration from Current Tiered Engine

### 19.1 Mapping Existing Components

| Current Component | Behavior Tree Equivalent |
|-------------------|--------------------------|
| `TieredDecisionEngine` | Root `Selector` with children for each tier |
| `DecisionTier::from_situation` | Selector child ordering |
| `RuleBasedDecisionEngine` | `Sequence` of `Condition` + `Action` |
| `LLMDecisionEngine` | `Prompt` node |
| `CLIDecisionEngine` | `Action` node emitting `EscalateToHuman` |
| `ConditionExpr` | `Condition` nodes with `Evaluator` |
| `DecisionAction` | `Action` nodes |
| `DecisionContext.metadata` | Blackboard `variables` |
| `reflection_round` | Blackboard field, managed by `ReflectionGuard` |
| `DecisionPreProcessor` | `Sequence` prefix nodes |
| `DecisionPostProcessor` | `Sequence` suffix nodes |

### 19.2 Migration Path

```
Phase 1 — decision-dsl crate (standalone)
  - Implement all components described in this document
  - Unit tests + integration tests
  - Benchmarks

Phase 2 — agent-decision integration
  - Create adapter layer (Session, Clock, Fs, Logger impls)
  - Add DslDecisionEngine alongside existing TieredDecisionEngine
  - Feature flag: dsl-engine

Phase 3 — Tree adoption
  - Port situations to YAML trees in decisions/trees/ and decisions/subtrees/
  - Golden tests: compare old vs new engine outputs

Phase 4 — Deprecation & Removal
  - Mark old engines deprecated
  - Remove after one release cycle
```

---

## Appendix A: Node YAML Quick Reference

### Composites
```yaml
kind: Selector
name: root
children:
  - ...

kind: Sequence
name: flow
children:
  - ...

kind: Parallel
name: checks
policy: allSuccess  # or anySuccess, majority
children:
  - ...
```

### Decorators
```yaml
kind: Inverter
name: not-complete
child: ...

kind: Repeater
name: retry-3
maxAttempts: 3
child: ...

kind: Cooldown
name: cooldown-5s
durationMs: 5000
child: ...

kind: ReflectionGuard
name: max-2
maxRounds: 2
child: ...

kind: ForceHuman
name: force-approval
reason: "PR submission requires human approval"
child: ...
```

### Leaves
```yaml
kind: Condition
name: is-rate-limit
eval:
  kind: regex
  pattern: "(429|rate.?limit)"

kind: Action
name: emit-retry
command:
  RetryTool:
    tool_name: "{{ last_tool_call.name | default('unknown') }}"
    args: null
    max_attempts: 3
when:
  kind: variableIs
  key: retry_needed
  value: true

kind: Prompt
name: ask-decision
model: thinking
timeoutMs: 30000
template: |
  Should we REFLECT or CONFIRM?
parser:
  kind: enum
  values: [REFLECT, CONFIRM]
  caseSensitive: false
sets:
  - key: next_action
    field: decision

kind: SetVar
name: set-phase
key: task_phase
value:
  kind: string
  value: "coding"
```

---

*Document version: 2.0*
*Last updated: 2026-04-20*
