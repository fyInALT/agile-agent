# Sprint 5: Observability, Hot Reload & Integration

## Metadata

- Sprint ID: `dsl-sprint-05`
- Title: `Observability, Hot Reload & Integration`
- Duration: 2 weeks
- Priority: P1 (High)
- Status: `Backlog`
- Created: 2026-04-20

## Sprint Goal

Complete the operational layer: tracing for debugging, hot reload for zero-downtime rule updates, comprehensive test infrastructure, and host integration bridges. The engine is production-ready for integration into `agent-decision`.

## Dependencies

- **Sprint 1** (`dsl-sprint-01`): External traits, Blackboard.
- **Sprint 2** (`dsl-sprint-02`): AST, Parser, Bundle.
- **Sprint 3** (`dsl-sprint-03`): Evaluators, Parsers, Templates.
- **Sprint 4** (`dsl-sprint-04`): Executor, all node behaviors.

## Non-goals

- No changes to the AST or node behavior (completed in Sprint 4).
- No new evaluators or parsers (completed in Sprint 3).
- No DSL syntax changes (completed in Sprint 2).

---

## Stories

### Story 5.1: Tracing & Observability

**Priority**: P1
**Effort**: 3 points
**Status**: Backlog

Implement the trace system that records every node enter/exit, evaluation, action, and prompt event.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T5.1.1 | Define `TraceEntry` enum with 11 variants | Todo | - |
| T5.1.2 | Implement `Tracer` struct (`entries`, `running_path`, `current_depth`) | Todo | - |
| T5.1.3 | Implement `Tracer::enter` / `exit` for composite/decorator/leaf nodes | Todo | - |
| T5.1.4 | Implement `Tracer::enter_subtree` / `exit_subtree` | Todo | - |
| T5.1.5 | Implement `Tracer::record_eval` | Todo | - |
| T5.1.6 | Implement `Tracer::record_action` | Todo | - |
| T5.1.7 | Implement `Tracer::record_prompt_sent` / `success` / `failure` | Todo | - |
| T5.1.8 | Implement `Tracer::record_rule_matched` / `skipped` | Todo | - |
| T5.1.9 | Implement `Tracer::running_path()` (derive from enter/exit events) | Todo | - |
| T5.1.10 | Implement `render_trace_ascii` for human-readable output | Todo | - |
| T5.1.11 | Write unit tests for trace generation | Todo | - |
| T5.1.12 | Write unit tests for ASCII rendering | Todo | - |

#### Acceptance Criteria

- Every node tick produces at least one `Enter` and one `Exit` trace entry.
- `Running` status produces a partial trace that can be resumed.
- ASCII rendering shows tree structure with depth indentation.
- SubTree boundaries are visually distinct in ASCII output.

#### Technical Notes

```rust
pub enum TraceEntry {
    Enter { node_name: String, child_index: usize, depth: usize },
    Exit { node_name: String, status: NodeStatus, duration: Duration },
    EnterSubTree { name: String, ref_name: String },
    ExitSubTree { name: String, ref_name: String, status: NodeStatus },
    Eval { node_name: String, evaluator: String, result: bool },
    Action { node_name: String, command: String },
    PromptSent { node_name: String },
    PromptSuccess { node_name: String, response: String },
    PromptFailure { node_name: String, error: String },
    RuleMatched { rule_name: String, priority: u32 },
    RuleSkipped { rule_name: String, reason: String },
}
```

---

### Story 5.2: Hot Reload

**Priority**: P1
**Effort**: 3 points
**Status**: Backlog

Implement zero-downtime rule reloading with `PollWatcher` (mtime-based, zero external deps).

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T5.2.1 | Define `Watcher` trait (`has_changed`) | Todo | - |
| T5.2.2 | Define `WatcherError` enum | Todo | - |
| T5.2.3 | Implement `PollWatcher` using `Fs::modified()` | Todo | - |
| T5.2.4 | Implement `DslReloader` struct (`parser`, `fs`, `watcher`, `dir`, `current_bundle`) | Todo | - |
| T5.2.5 | Implement `DslReloader::new` (initial parse) | Todo | - |
| T5.2.6 | Implement `DslReloader::check_and_reload` | Todo | - |
| T5.2.7 | Implement `DslReloader::current` (read lock on bundle) | Todo | - |
| T5.2.8 | Handle reload failures gracefully (keep old bundle, log error) | Todo | - |
| T5.2.9 | Write unit tests for PollWatcher with MockFs | Todo | - |
| T5.2.10 | Write unit tests for DslReloader reload cycle | Todo | - |
| T5.2.11 | Write unit tests for reload failure recovery | Todo | - |

#### Acceptance Criteria

- In-flight decisions use the old tree; new ticks use the reloaded tree.
- `PollWatcher` detects changes without OS-specific file watching APIs.
- Reload failure does not crash; old bundle continues to run.
- Thread-safe: `current()` returns `RwLockReadGuard<Bundle>`.

#### Technical Notes

```rust
pub struct DslReloader {
    parser: YamlParser,
    fs: Box<dyn Fs>,
    watcher: Box<dyn Watcher>,
    dir: PathBuf,
    current_bundle: Arc<RwLock<Bundle>>,
}

impl DslReloader {
    pub fn check_and_reload(&mut self) -> Result<bool, DslError> {
        if self.watcher.has_changed()? {
            let new = self.parser.parse_bundle(&self.dir, self.fs.as_ref())?;
            *self.current_bundle.write().unwrap() = new;
            Ok(true)
        } else {
            Ok(false)
        }
    }
}
```

---

### Story 5.3: Testing Infrastructure

**Priority**: P1
**Effort**: 3 points
**Status**: Backlog

Build mock trait implementations and golden tests for the entire engine.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T5.3.1 | Implement `MockSession` with `RefCell<VecDeque>` replies | Todo | - |
| T5.3.2 | Implement `MockClock` with `advance(Duration)` | Todo | - |
| T5.3.3 | Implement `CaptureLogger` | Todo | - |
| T5.3.4 | Implement `MockFs` for in-memory file system testing | Todo | - |
| T5.3.5 | Write integration test: DecisionRules → full tick → commands | Todo | - |
| T5.3.6 | Write integration test: Switch on Prompt → 2 ticks → branch | Todo | - |
| T5.3.7 | Write integration test: SubTree scope isolation | Todo | - |
| T5.3.8 | Write integration test: Cooldown with MockClock | Todo | - |
| T5.3.9 | Write golden tests: compare expected vs actual trace output | Todo | - |
| T5.3.10 | Write property-based tests for evaluator combinations (or/and/not) | Todo | - |
| T5.3.11 | Set coverage targets: Parser 95%, Desugaring 100%, Evaluators 100%, Parsers 100%, Executor 95%, Blackboard 100% | Todo | - |

#### Acceptance Criteria

- Every external trait has a mock implementation usable in unit tests.
- At least one end-to-end integration test per high-level construct (DecisionRules, Switch, When, Pipeline).
- Golden tests produce stable trace output for regression detection.
- Coverage meets or exceeds targets defined in `decision-dsl-ext.md` §3.5.

#### Technical Notes

```rust
pub struct MockSession {
    replies: RefCell<VecDeque<String>>,
    ready: RefCell<bool>,
    sent_messages: RefCell<Vec<String>>,
}

impl Session for MockSession {
    fn send(&mut self, message: &str) -> Result<(), SessionError> {
        self.sent_messages.borrow_mut().push(message.to_string());
        Ok(())
    }
    fn is_ready(&self) -> bool { *self.ready.borrow() }
    fn receive(&mut self) -> Result<String, SessionError> {
        self.replies.borrow_mut().pop_front().ok_or(...)
    }
}
```

---

### Story 5.4: Host Integration

**Priority**: P1
**Effort**: 2 points
**Status**: Backlog

Provide bridge implementations for integration into `agent-decision` and `agent-daemon`.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T5.4.1 | Document `ProviderSessionBridge` (adapts agent-provider session to `decision_dsl::Session`) | Todo | - |
| T5.4.2 | Document `StdFs` bridge (already implemented in Sprint 1) | Todo | - |
| T5.4.3 | Document `TickContext` construction from host state | Todo | - |
| T5.4.4 | Document `Blackboard` population from work agent state | Todo | - |
| T5.4.5 | Document command consumption by `DecisionCommandInterpreter` | Todo | - |
| T5.4.6 | Document `build_blackboard(agent_state)` helper for host state → Blackboard mapping | Todo | - |
| T5.4.7 | Document `DslDecisionEngine::reload()` for hot reload integration | Todo | - |
| T5.4.8 | Provide example integration test in `agent-decision` crate | Todo | - |

#### Acceptance Criteria

- Host can parse rules, create blackboard, tick executor, and read commands without knowing AST internals.
- Integration example compiles and runs against mock providers.
- Public API surface remains minimal (only `DslParser`, `DslRunner`, `Blackboard`, `DecisionCommand`, `TickResult`, and injectable traits).

#### Technical Notes

```rust
// Host integration pattern
let parser = YamlParser::new();
let doc = parser.parse_document(&yaml)?;
let mut tree = doc.desugar(&parser.evaluator_registry)?;

let mut executor = Executor::new();
let mut bb = Blackboard::new();
bb.task_description = "Implement auth".into();
bb.provider_output = output_from_agent;

let mut ctx = TickContext::new(&mut bb, &mut session, &clock, &logger);
let result = executor.tick(&mut tree, &mut ctx)?;

for cmd in result.commands {
    match cmd {
        DecisionCommand::Agent(AgentCommand::Reflect { prompt }) => { /* ... */ }
        DecisionCommand::Human(HumanCommand::Escalate { reason, .. }) => { /* ... */ }
        _ => {}
    }
}
```

---

### Story 5.5: Performance & Metrics

**Priority**: P2
**Effort**: 2 points
**Status**: Backlog

Add performance targets and optional metrics collection.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T5.5.1 | Define `MetricsCollector` trait | Todo | - |
| T5.5.2 | Implement `Blackboard::with_capacity(n)` for pre-allocation | Todo | - |
| T5.5.3 | Add benchmark for 1M evaluator calls/second target | Todo | - |
| T5.5.4 | Add benchmark for Parser + Desugaring throughput | Todo | - |
| T5.5.5 | Document memory footprint targets (20KB parse, 10KB AST, 2KB per tick) | Todo | - |
| T5.5.6 | Document `Send + Sync` / `!Sync` executor semantics (Trees are Send+Sync; Executor is !Sync) | Todo | - |
| T5.5.7 | Document Parallel Safety: one executor + blackboard per agent thread | Todo | - |
| T5.5.8 | Write criterion benchmarks | Todo | - |
| T5.5.9 | Write memory footprint verification tests (assert parse ≤ 20KB, AST ≤ 10KB, blackboard ≤ 2KB) | Todo | - |

#### Acceptance Criteria

- Benchmark suite runs with `cargo bench`.
- 1M evaluator calls/second target is measurable.
- Memory footprint is documented and verifiable via explicit tests.
- No regressions in `cargo test --lib` from benchmark code.

#### Technical Notes

```rust
pub trait MetricsCollector {
    fn record_tick(&self, tree_name: &str, duration: Duration);
    fn record_rule_match(&self, rule_name: &str, priority: u32, duration: Duration);
    fn record_node(&self, node_name: &str, node_type: &str, status: NodeStatus, duration: Duration);
    fn record_prompt(&self, node_name: &str, model: &str, latency_ms: u64, prompt_tokens: u32, completion_tokens: u32);
    fn record_subtree(&self, ref_name: &str, duration: Duration, status: NodeStatus);
}
```

---

## Sprint Completion Criteria

- [ ] `cargo check` passes for the `decision-dsl` crate.
- [ ] `cargo test --lib` passes with 100% coverage on trace and mock modules.
- [ ] `cargo bench` runs without errors.
- [ ] Hot reload test demonstrates atomic swap semantics.
- [ ] Host integration example compiles in a separate test crate.
- [ ] All public API types are documented with rustdoc.
- [ ] README.md for `decision-dsl` crate is updated with quick-start.
