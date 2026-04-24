# Decision DSL Implementation Design

> Implementation specification for the decision DSL engine. This is the **entry-point document**. Detailed chapters are split into focused sub-documents linked below.
>
> This specification corresponds 1:1 with `decision-dsl.md` (DSL specification) and `decision-dsl-reflection.md` (design critique and proposals).

---

## Document Map

| Document | Contents |
|----------|----------|
| **This document** | Overview, package structure, public API, desugaring, integration, migration |
| [`decision-dsl.md`](decision-dsl.md) | DSL language specification: DecisionRules, Switch, When, Pipeline, BehaviorTree nodes |
| [`decision-dsl-ast.md`](decision-dsl-ast.md) | AST design, enum_dispatch Node, desugaring pass, scoped Blackboard, grouped Command |
| [`decision-dsl-runtime.md`](decision-dsl-runtime.md) | Executor tick loop, node implementations, SubTree scope isolation, Prompt node |
| [`decision-dsl-evaluators.md`](decision-dsl-evaluators.md) | Evaluator and OutputParser enums, built-in evaluators, parsers |
| [`decision-dsl-template.md`](decision-dsl-template.md) | minijinja template engine, command field rendering |
| [`decision-dsl-ext.md`](decision-dsl-ext.md) | External traits (Session, Clock, Logger), error handling, testing, performance, observability |
| [`decision-dsl-reflection.md`](decision-dsl-reflection.md) | Design critique: problems found, references to industry projects, proposed redesign rationale |

---

## 1. Overview

### 1.1 Design Goals

| Goal | How |
|------|-----|
| **Simple things simple** | DecisionRules syntax: 1 rule = ~5 lines. Switch node: 1 node = what used to be 6. |
| **Progressive disclosure** | Start with DecisionRules. Drop down to BehaviorTree nodes only for complex cases. |
| **Enum-based, no trait objects** | Evaluator, OutputParser, Node are enums. Stack-allocated, Clone, Debug, no vtable. |
| **Trait-based injection** | Session, Clock, Logger are traits. Testable with mocks. |
| **Spec-compliant** | Every YAML construct from `decision-dsl.md` is implemented. |
| **Lua-inspired** | Host loads → ticks → receives output. The engine is an embedded VM. |
| **Dependency-aware** | `minijinja` for templates (not hand-rolled). `enum_dispatch` for Node (not manual matches). |
| **Testable in isolation** | Every external dependency is mockable. |

### 1.2 Architecture Analogy: Lua

| Lua | Decision DSL |
|-----|--------------|
| `lua_State` — VM state | `Executor` — tree + running path |
| `lua_pcall` — protected call | `DslRunner::tick` — returns `TickResult` |
| C function registry | `EvaluatorRegistry` — maps `kind` string → `Evaluator` enum variant |
| Global table `_G` | `Blackboard` — scoped key-value store |
| `lua_load` + `lua_pcall` | `DslParser::parse_document` → desugar → `DslRunner::tick` |

### 1.3 Two Authoring Styles, One AST

```
┌─────────────────────┐
│  DecisionRules YAML │  ← Concise: ~120 lines covers all common scenarios
│  (recommended)      │
└────────┬────────────┘
         │ desugar
         ▼
┌─────────────────────┐
│  BehaviorTree AST   │  ← Executor runs this
│  (Selector, etc.)   │
└────────┬────────────┘
         │ tick
         ▼
┌─────────────────────┐
│  Vec<DecisionCmd>   │
└─────────────────────┘

┌─────────────────────┐
│  BehaviorTree YAML  │  ← Full control: write BT nodes directly
│  (advanced)         │
└────────┬────────────┘
         │ parse (no desugar needed)
         ▼
┌─────────────────────┐
│  BehaviorTree AST   │
└─────────────────────┘
```

### 1.4 What the Host Sees

```rust
use decision_dsl::{DslParser, YamlParser, Executor, TickContext, TickResult};
use decision_dsl::{Blackboard, DecisionCommand};
use decision_dsl::ext::{Session, Clock, Logger};

// 1. Parse
let parser = YamlParser::new();
let doc = parser.parse_document(&fs::read_to_string("rules.d/default.yaml")?)?;

// 2. Desugar DecisionRules → BehaviorTree AST
let mut tree = doc.desugar(&parser.evaluator_registry)?;

// 3. Create runner
let mut executor = Executor::new();

// 4. Set up blackboard
let mut bb = Blackboard::new();
bb.task_description = "Implement auth".into();
bb.provider_output = "Task complete! All tests passing.".into();

// 5. Tick
let mut ctx = TickContext::new(&mut bb, &mut session, &clock, &logger);
let result = executor.tick(&mut tree, &mut ctx)?;

// 6. Consume output
for cmd in result.commands {
    match cmd {
        DecisionCommand::Agent(AgentCommand::Reflect { prompt }) => { /* ... */ }
        DecisionCommand::Agent(AgentCommand::ApproveAndContinue) => { /* ... */ }
        DecisionCommand::Human(HumanCommand::Escalate { reason, .. }) => { /* ... */ }
        _ => {}
    }
}
```

### 1.5 Crate Dependencies

```toml
[package]
name = "decision-dsl"
version = "0.1.0"
edition = "2021"

[dependencies]
serde = { version = "1.0", features = ["derive"] }
serde_yaml = "0.9"
serde_json = "1.0"
regex = "1.10"
minijinja = "2"                  # Template engine (replaces hand-rolled)
enum_dispatch = "0.3"            # Auto-generate Node match arms

[dev-dependencies]
# No agent-* crates. Only mock trait impls.
```

Removed vs previous design: hand-rolled template engine (~500 lines), `dyn_clone` crate, manual visitor match arms.

---

## 2. Package Structure

```
decision-dsl/
├── Cargo.toml
└── src/
    ├── lib.rs                  # Public API surface
    ├── parser/
    │   ├── mod.rs              # Parser trait + YamlParser impl
    │   ├── document.rs         # DslDocument: DecisionRules | BehaviorTree | SubTree
    │   ├── yaml.rs             # serde_yaml → DslDocument conversion
    │   ├── desugar.rs          # DecisionRules → BehaviorTree AST desugaring
    │   └── validate.rs         # Schema + semantic validation
    ├── ast/
    │   ├── mod.rs              # AST types
    │   ├── tree.rs             # Tree, Metadata, Spec, Bundle
    │   ├── node.rs             # Node enum + enum_dispatch trait
    │   ├── command.rs          # DecisionCommand (grouped enum)
    │   └── spec.rs             # RuleSpec, ThenSpec, SwitchSpec, etc. (parse-time types)
    ├── runtime/
    │   ├── mod.rs              # Runtime exports
    │   ├── executor.rs         # Executor: tick/reset/resume
    │   ├── blackboard.rs       # Blackboard: scoped key-value store
    │   ├── context.rs          # TickContext: per-tick injection
    │   └── trace.rs            # TraceEntry + Tracer + ASCII rendering
    ├── nodes/
    │   ├── mod.rs              # NodeBehavior trait (enum_dispatch)
    │   ├── composite.rs        # Selector, Sequence, Parallel
    │   ├── decorator.rs        # Inverter, Repeater, Cooldown, ReflectionGuard, ForceHuman
    │   ├── high_level.rs       # When
    │   └── leaf.rs             # Condition, Action, Prompt, SetVar, SubTree
    ├── eval/
    │   ├── mod.rs              # Evaluator enum + EvaluatorRegistry
    │   └── builtin.rs          # All evaluator variants
    ├── parser_out/
    │   ├── mod.rs              # OutputParser enum + OutputParserRegistry
    │   └── builtin.rs          # All parser variants
    ├── template/
    │   ├── mod.rs              # Template engine (minijinja wrapper)
    │   ├── env.rs              # Environment setup + custom filters
    │   └── render.rs           # render_prompt_template, render_command_templates
    ├── ext/                    # External dependency traits
    │   ├── mod.rs
    │   ├── session.rs          # Session (send, send_with_hint, is_ready, receive)
    │   ├── clock.rs            # Clock (time source)
    │   └── log.rs              # Logger (structured logging)
    └── error.rs                # ParseError, RuntimeError, DslError
```

### Visibility Rules

```rust
// lib.rs — ONLY these are pub
pub use parser::{DslParser, YamlParser, DslDocument, RuleSpec, ThenSpec};
pub use ast::{Tree, TreeKind, Metadata, Spec, Bundle};
pub use ast::command::{DecisionCommand, AgentCommand, GitCommand, TaskCommand, HumanCommand, ProviderCommand};
pub use runtime::{DslRunner, Executor, TickContext, TickResult, Blackboard, BlackboardValue};
pub use runtime::trace::{TraceEntry, Tracer};
pub use eval::{Evaluator, EvaluatorRegistry};
pub use parser_out::{OutputParser, OutputParserRegistry, StructuredField, FieldType};
pub use ext::{Session, SessionError, SessionErrorKind, Clock, SystemClock, Logger, LogLevel, NullLogger, TracingLogger};
pub use error::{DslError, ParseError, RuntimeError};

// Everything in nodes/, template/ is pub(crate)
```

---

## 3. Public API

### 3.1 DslParser

```rust
use std::path::Path;

/// Parse YAML into a DslDocument (before desugaring).
pub trait DslParser {
    /// Parse a single YAML string (DecisionRules, BehaviorTree, or SubTree).
    fn parse_document(&self, yaml: &str) -> Result<DslDocument, ParseError>;

    /// Parse a directory into a Bundle (desugared, validated).
    fn parse_bundle(&self, dir: &Path) -> Result<Bundle, ParseError>;
}

/// Concrete implementation.
pub struct YamlParser {
    pub evaluator_registry: EvaluatorRegistry,
    pub parser_registry: OutputParserRegistry,
}

impl YamlParser {
    pub fn new() -> Self {
        Self {
            evaluator_registry: EvaluatorRegistry::new(),
            parser_registry: OutputParserRegistry::new(),
        }
    }
}
```

### 3.2 DslDocument → Tree Desugaring

```rust
impl DslDocument {
    /// Desugar DecisionRules into a BehaviorTree AST.
    /// BehaviorTree and SubTree documents pass through unchanged.
    pub fn desugar(self, registry: &EvaluatorRegistry) -> Result<Tree, ParseError> {
        // See decision-dsl-ast.md §1.3 for the full implementation
    }
}
```

### 3.3 DslRunner

```rust
/// Execute a behavior tree against a Blackboard.
pub trait DslRunner {
    fn tick(&mut self, tree: &mut Tree, ctx: &mut TickContext) -> Result<TickResult, RuntimeError>;
    fn reset(&mut self);
}

#[derive(Debug, Clone)]
pub struct TickResult {
    pub status: NodeStatus,
    pub commands: Vec<DecisionCommand>,
    pub trace: Vec<TraceEntry>,
}

pub struct TickContext<'a> {
    pub blackboard: &'a mut Blackboard,
    pub session: &'a mut dyn Session,
    pub clock: &'a dyn Clock,
    pub logger: &'a dyn Logger,
}
```

### 3.4 Executor

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
    fn tick(&mut self, tree: &mut Tree, ctx: &mut TickContext) -> Result<TickResult, RuntimeError> {
        let mut tracer = Tracer::new();
        let status = if self.is_running {
            tree.spec.root.resume_at(&self.running_path, 0, ctx, &mut tracer)?
        } else {
            tree.spec.root.tick(ctx, &mut tracer)?
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

## 4. Desugaring Reference

| High-Level Construct | Desugars To |
|---------------------|-------------|
| `DecisionRules { rules }` | `BehaviorTree { root: Selector(rule[1], ..., rule[n], no_match_fallback) }` |
| `Rule { if, then }` | `Sequence(Condition(if), then.desugar())` |
| `Rule { if, then, cooldownMs }` | `Cooldown(durationMs, Sequence(Condition(if), then.desugar()))` |
| `Rule { if, then, reflectionMaxRounds }` | `ReflectionGuard(maxRounds, Sequence(Condition(if), then.desugar()))` |
| `Switch { on: prompt, cases }` | `Sequence(Prompt, Selector(for each case: When(var==case, then), DefaultCase))` |
| `Switch { on: variable, cases }` | `Selector(for each case: When(var==case, then), DefaultCase)` |
| `When { if, then }` | `When(condition, action)` — runtime: `if cond.eval() { action.tick() }` |
| `Pipeline { steps }` | `Sequence(for each step: if → Condition; then → Action)` |
| `then: { command: ... }` (inline) | `Action(command)` |

### Line Count Comparison

| Scenario | Old (BT nodes) | New (DecisionRules) | Reduction |
|----------|---------------|---------------------|-----------|
| Simple condition → action | 8 lines, 2 nodes | 5 lines, 1 rule | 38% |
| LLM prompt → 2-way branch | 35 lines, 6 nodes | 12 lines, 1 Switch | 66% |
| Full decision file (all scenarios) | ~800 lines, 10 files | ~120 lines, 1 file | 85% |

---

## 5. Integration Example

### 5.1 Host Integration: agent-decision

```rust
use decision_dsl::{DslParser, YamlParser, Executor, TickContext};
use decision_dsl::{Blackboard, DecisionCommand, AgentCommand, HumanCommand};
use decision_dsl::ext::{Session, Clock, Logger, SystemClock};

/// Bridge: adapts agent-provider's session to decision_dsl::Session.
pub struct ProviderSessionBridge {
    provider_session: Arc<Mutex<ProviderSession>>,
}

impl Session for ProviderSessionBridge {
    fn send(&mut self, message: &str) -> Result<(), SessionError> {
        let mut session = self.provider_session.lock().unwrap();
        session.send_message(message).map_err(|e| SessionError {
            kind: SessionErrorKind::SendFailed,
            message: e.to_string(),
        })
    }

    fn send_with_hint(&mut self, message: &str, model: &str) -> Result<(), SessionError> {
        let mut session = self.provider_session.lock().unwrap();
        session.send_message_with_model(message, model).map_err(|e| SessionError {
            kind: SessionErrorKind::SendFailed,
            message: e.to_string(),
        })
    }

    fn is_ready(&self) -> bool {
        self.provider_session.lock().unwrap().has_pending_reply()
    }

    fn receive(&mut self) -> Result<String, SessionError> {
        let mut session = self.provider_session.lock().unwrap();
        session.await_reply(Duration::from_secs(30)).map_err(|e| SessionError {
            kind: SessionErrorKind::Timeout,
            message: e.to_string(),
        })
    }
}

/// Host's decision engine using the DSL.
pub struct DslDecisionEngine {
    parser: YamlParser,
    executor: Executor,
    tree: Tree,     // Desugared AST (mutable for tick)
    session: ProviderSessionBridge,
    clock: SystemClock,
    logger: TracingLogger,
}

impl DslDecisionEngine {
    pub fn new(rules_path: &Path, session: ProviderSessionBridge) -> Result<Self, DslError> {
        let parser = YamlParser::new();
        let yaml = std::fs::read_to_string(rules_path)?;
        let doc = parser.parse_document(&yaml)?;
        let tree = doc.desugar(&parser.evaluator_registry)?;

        Ok(Self {
            parser, executor: Executor::new(), tree,
            session, clock: SystemClock, logger: TracingLogger,
        })
    }

    pub fn decide(&mut self, blackboard: &mut Blackboard) -> Result<Vec<DecisionCommand>, DslError> {
        self.executor.reset();
        let mut ctx = TickContext::new(
            blackboard, &mut self.session, &self.clock, &self.logger,
        );
        let result = self.executor.tick(&mut self.tree, &mut ctx)?;
        if result.status == NodeStatus::Running {
            return Ok(vec![DecisionCommand::Agent(AgentCommand::ApproveAndContinue)]);
        }
        Ok(result.commands)
    }

    /// Hot reload: re-parse and re-desugar the rules file.
    pub fn reload(&mut self, rules_path: &Path) -> Result<(), DslError> {
        let yaml = std::fs::read_to_string(rules_path)?;
        let doc = self.parser.parse_document(&yaml)?;
        self.tree = doc.desugar(&self.parser.evaluator_registry)?;
        Ok(())
    }
}
```

### 5.2 Building the Blackboard from Agent State

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

---

## 6. Migration from Current Tiered Engine

### 6.1 Mapping Existing Components

| Current Component | DSL Equivalent |
|-------------------|----------------|
| `TieredDecisionEngine` | `DecisionRules` with priority ordering |
| `DecisionTier::from_situation` | Rule `if` conditions (priority-ordered) |
| `RuleBasedDecisionEngine` | `When { if, then }` rule |
| `LLMDecisionEngine` | `Switch { on: prompt }` |
| `CLIDecisionEngine` | `then: { command: EscalateToHuman }` |
| `ConditionExpr` | `Evaluator` enum variants |
| `DecisionAction` | `DecisionCommand` enum |
| `DecisionContext.metadata` | Blackboard custom variables |
| `reflection_round` | `reflectionMaxRounds` on rule |
| `DecisionPreProcessor` | `Pipeline { steps: [if, if, ...] }` |
| `DecisionPostProcessor` | Pipeline suffix steps |

### 6.2 Migration Path

```
Phase 1 — decision-dsl crate (standalone)
  - Implement enum-based AST, evaluators, parsers
  - Implement desugaring pass (DecisionRules → BT AST)
  - Implement executor with enum_dispatch
  - Unit tests + integration tests for all constructs

Phase 2 — agent-decision integration
  - Create Session bridge (ProviderSessionBridge)
  - Add DslDecisionEngine alongside existing TieredDecisionEngine
  - Feature flag: dsl-engine

Phase 3 — Rule adoption
  - Port situations to DecisionRules YAML in decisions/rules.d/
  - Golden tests: compare old vs new engine outputs
  - Delete hand-written tiered engine code

Phase 4 — Deprecation & Removal
  - Mark old engines deprecated
  - Remove after one release cycle
```

---

## Appendix: Node YAML Quick Reference

### DecisionRules (Recommended)
```yaml
apiVersion: decision.agile-agent.io/v1
kind: DecisionRules
metadata:
  name: my_rules
spec:
  rules:
    - priority: 1
      name: my_rule
      if:
        kind: outputContains
        pattern: "pattern"
      then:
        command: SomeCommand
      cooldownMs: 5000        # Optional
      reflectionMaxRounds: 2  # Optional
      on_error: skip          # Optional: skip | escalate | retry
```

### High-Level Nodes
```yaml
kind: Switch
name: decide
on:
  kind: prompt                    # or: kind: variable, key: error_strategy
  template: "REFLECT or CONFIRM?"
  parser:
    kind: enum
    values: [REFLECT, CONFIRM]
cases:
  REFLECT:
    command: { Reflect: { prompt: "Review" } }
  CONFIRM:
    command: ConfirmCompletion
  _default:
    command: ApproveAndContinue

kind: When
name: guarded_action
if:
  kind: regex
  pattern: "(429|rate.?limit)"
then:
  command: { RetryTool: { tool_name: "...", max_attempts: 3 } }

kind: Pipeline
name: safe_commit
steps:
  - if: { kind: outputContains, pattern: "tests passing" }
  - if: { kind: script, expression: "file_changes.length() < 10" }
  - then: { command: { SuggestCommit: { message: "Safe", mandatory: false } } }
```

### Composites (Low-Level)
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

### Decorators (Low-Level)
```yaml
kind: Inverter
name: not-x
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
reason: "Human approval required"
child: ...
```

### Leaves (Low-Level)
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
    tool_name: "{{ last_tool_call.name }}"
    max_attempts: 3

kind: Prompt
name: ask-decision
model: thinking
template: "REFLECT or CONFIRM?"
parser:
  kind: enum
  values: [REFLECT, CONFIRM]
sets:
  - { key: next_action, field: decision }

kind: SetVar
name: set-phase
key: task_phase
value: { kind: string, value: "coding" }

kind: SubTree
name: use_handler
ref: reflect_loop
```

---

*Document version: 2.0*
*Last updated: 2026-04-24*
