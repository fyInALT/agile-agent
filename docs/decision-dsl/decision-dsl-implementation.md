# Decision DSL Implementation Design

> Implementation specification for a standalone, zero-dependency behavior tree engine. This is the **entry-point document**. Detailed chapters are split into focused sub-documents linked below.
>
> This specification corresponds 1:1 with `decision-dsl.md` (DSL specification), `decision-layer-design.md` (architecture), and the five `decision-dsl-examples/`.

---

## Document Map

| Document | Lines | Contents |
|----------|-------|----------|
| **This document** | ~600 | Overview, package structure, public API, integration, migration |
| [`decision-dsl-ast.md`](decision-dsl-ast.md) | ~530 | AST Design, Blackboard data model |
| [`decision-dsl-runtime.md`](decision-dsl-runtime.md) | ~350 | Executor tick loop, node implementations, Prompt node |
| [`decision-dsl-evaluators.md`](decision-dsl-evaluators.md) | ~470 | Condition evaluators, output parsers |
| [`decision-dsl-template.md`](decision-dsl-template.md) | ~310 | Template engine, action command rendering |
| [`decision-dsl-ext.md`](decision-dsl-ext.md) | ~970 | External traits, error handling, testing, performance, validation, observability |

---

## 1. Overview

### 1.1 Design Goals

| Goal | How |
|------|-----|
| **Standalone** | Zero dependencies on `agent-*` crates. Only stdlib + serde + regex. |
| **Two public entrypoints** | `DslParser` (YAML → AST) and `DslRunner` (AST → commands). |
| **Trait-based injection** | LLM session, filesystem, clock, logging, file-watching are all traits. |
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
pub use ext::{Session, SessionError, Clock, Fs, FsError, Logger, LogLevel, TracingLogger, Watcher, PollWatcher, WatcherError};
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

    /// Convert raw serde_yaml::Value into a typed AST Tree.
    fn value_to_tree(&self, raw: serde_yaml::Value) -> Result<Tree, ParseError> {
        let mapping = raw.as_mapping().ok_or_else(|| ParseError::Custom("expected YAML mapping".into()))?;

        let api_version = get_string(mapping, "apiVersion")?;
        let kind_str = get_string(mapping, "kind")?;
        let kind = match kind_str.as_str() {
            "BehaviorTree" => TreeKind::BehaviorTree,
            "SubTree" => TreeKind::SubTree,
            _ => return Err(ParseError::InvalidProperty {
                key: "kind".into(), value: kind_str, reason: "expected BehaviorTree or SubTree".into(),
            }),
        };

        let metadata = mapping.get("metadata")
            .and_then(|v| v.as_mapping())
            .ok_or_else(|| ParseError::MissingProperty("metadata"))?;
        let name = get_string(metadata, "name")?;
        let description = get_string(metadata, "description").ok();

        let spec = mapping.get("spec")
            .and_then(|v| v.as_mapping())
            .ok_or_else(|| ParseError::MissingProperty("spec"))?;
        let root = spec.get("root")
            .ok_or_else(|| ParseError::MissingProperty("spec.root"))?;
        let root_node = self.value_to_node(root)?;

        Ok(Tree { api_version, kind, metadata: Metadata { name, description }, spec: Spec { root: root_node } })
    }

    /// Recursively convert a serde_yaml::Value into a Node.
    fn value_to_node(&self, raw: &serde_yaml::Value) -> Result<Node, ParseError> {
        let mapping = raw.as_mapping().ok_or_else(|| ParseError::Custom("expected node mapping".into()))?;
        let kind = get_string(mapping, "kind")?;
        let name = get_string(mapping, "name")?;

        // Extract properties (everything except kind, name, children, child)
        let mut properties = mapping.clone();
        properties.remove("kind");
        properties.remove("name");
        properties.remove("children");
        properties.remove("child");

        // Recurse into children
        let children = if let Some(children_val) = mapping.get("children") {
            children_val.as_sequence()
                .ok_or_else(|| ParseError::Custom("expected children sequence".into()))?
                .iter()
                .map(|c| self.value_to_node(c))
                .collect::<Result<Vec<_>, _>>()?
        } else {
            Vec::new()
        };

        // Handle single-child decorators
        let mut final_children = children;
        if let Some(child_val) = mapping.get("child") {
            let child_node = self.value_to_node(child_val)?;
            final_children.push(child_node);
        }

        self.node_registry.create(&kind, name, properties, final_children)
    }
}

impl DslParser for YamlParser {
    fn parse_tree(&self, yaml: &str) -> Result<Tree, ParseError> {
        let raw: serde_yaml::Value = serde_yaml::from_str(yaml)?;
        let tree = self.value_to_tree(raw)?;
        tree.validate_api_version()?;
        tree.validate_unique_names()?;
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

See [`decision-dsl-ast.md`](decision-dsl-ast.md) for the full AST, Blackboard, and Bundle design.

### 3.1a YAML-to-Rust Naming Conventions

The parser automatically converts YAML camelCase field names to Rust snake_case:

| YAML (camelCase) | Rust (snake_case) | Type conversion |
|------------------|-------------------|-----------------|
| `apiVersion` | `api_version` | `String` |
| `durationMs` | `duration` | `u64` → `Duration` |
| `timeoutMs` | `timeout` | `u64` → `Duration` |
| `maxAttempts` | `max_attempts` | `u32` |
| `maxRounds` | `max_rounds` | `u8` |
| `caseSensitive` | `case_sensitive` | `bool` |
| `ref` (SubTree) | `ref_name` | `String` |

The helper `get_string(props, "case_sensitive")` first looks for `"case_sensitive"`; if not found, it falls back to `"caseSensitive"` via `to_camel_case()`. This allows both conventions in YAML while normalizing to Rust style internally.

```rust
fn get_string(mapping: &serde_yaml::Mapping, key: &str) -> Result<String, ParseError> {
    mapping.get(serde_yaml::Value::String(key.to_string()))
        .or_else(|| mapping.get(&serde_yaml::Value::String(to_camel_case(key))))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| ParseError::MissingProperty(key))
}

fn to_camel_case(s: &str) -> String {
    let mut result = String::new();
    let mut capitalize = false;
    for ch in s.chars() {
        if ch == '_' {
            capitalize = true;
        } else if capitalize {
            result.push(ch.to_ascii_uppercase());
            capitalize = false;
        } else {
            result.push(ch);
        }
    }
    result
}
```

### 3.2 DslRunner

```rust
/// Execute a behavior tree against a Blackboard.
pub trait DslRunner {
    fn tick(&mut self, tree: &Tree, ctx: &mut TickContext) -> Result<TickResult, RuntimeError>;
    fn reset(&mut self);
}

#[derive(Debug, Clone)]
pub struct TickResult {
    pub status: NodeStatus,
    pub commands: Vec<Command>,
    pub trace: Vec<TraceEntry>,
}

pub struct TickContext<'a> {
    pub blackboard: &'a mut Blackboard,
    pub session: &'a mut dyn Session,
    pub clock: &'a dyn Clock,
    pub fs: &'a dyn Fs,
    pub logger: &'a dyn Logger,
}
```

See [`decision-dsl-runtime.md`](decision-dsl-runtime.md) for the Executor tick loop and node implementations.

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

## 4. Integration Example

### 4.1 Host Integration: agent-decision

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
            parser, executor: Executor::new(), bundle,
            session, clock: SystemClock, fs, logger: TracingLogger,
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
            blackboard, &mut self.session, &self.clock, &self.fs, &self.logger,
        );
        let result = self.executor.tick(tree, &mut ctx)?;
        if result.status == NodeStatus::Running {
            return Ok(vec![Command::SkipDecision]);
        }
        Ok(result.commands)
    }
}
```

### 4.2 Building the Blackboard from Agent State

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

### 4.3 Complete End-to-End Example

```rust
use decision_dsl::*;

fn main() -> Result<(), DslError> {
    let yaml = include_str!("tree.yaml");
    let parser = YamlParser::new();
    let tree = parser.parse_tree(yaml)?;

    let mut bb = Blackboard::new();
    bb.task_description = "Implement auth".into();
    bb.provider_output = "Task complete! All tests passing.".into();
    bb.reflection_round = 0;
    bb.max_reflection_rounds = 2;

    let mut session = MockSession::new(vec!["is_complete: true\nconfidence: 0.95".into()]);
    let clock = SystemClock;
    let fs = RealFs;
    let logger = NullLogger;

    let mut executor = Executor::new();
    let mut ctx = TickContext::new(&mut bb, &mut session, &clock, &fs, &logger);
    let result = executor.tick(&tree, &mut ctx)?;

    println!("Status: {:?}", result.status);
    for cmd in &result.commands { println!("Command: {:?}", cmd); }
    for entry in &result.trace { println!("Trace: {:?}", entry); }

    Ok(())
}
```

---

## 5. Migration from Current Tiered Engine

### 5.1 Mapping Existing Components

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

### 5.2 Migration Path

```
Phase 1 — decision-dsl crate (standalone)
  - Implement all components described in the sub-documents
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

## Appendix: Node YAML Quick Reference

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

*Document version: 3.0 (split edition)*
*Last updated: 2026-04-20*
