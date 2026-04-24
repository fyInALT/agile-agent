# Sprint 1: Core Types & Parser Foundation

## Metadata

- Sprint ID: `dsl-sprint-01`
- Title: `Core Types & Parser Foundation`
- Duration: 2 weeks
- Priority: P0 (Critical)
- Status: `Backlog`
- Created: 2026-04-20

## Sprint Goal

Establish the foundational type system for the decision DSL engine: error handling, external trait boundaries, the blackboard state store, and the grouped command enum. All components compile, are unit-tested with mocks, and form a stable base for AST construction in Sprint 2.

## Dependencies

- None (foundational sprint).

## Non-goals

- No AST nodes or parser logic (Sprint 2).
- No evaluators or output parsers (Sprint 3).
- No executor or tick loop (Sprint 4).
- No hot reload or observability (Sprint 5).

---

## Stories

### Story 1.1: Error Type Hierarchy

**Priority**: P0
**Effort**: 3 points
**Status**: Backlog

Define the complete error hierarchy used across the DSL engine. Every error path must be representable without panics.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T1.1.1 | Create `ParseError` enum with all 18 variants | Todo | - |
| T1.1.2 | Create `RuntimeError` enum with all 8 variants | Todo | - |
| T1.1.3 | Create `DslError` union enum (`Parse` \| `Runtime`) | Todo | - |
| T1.1.4 | Implement `Display` for all error types | Todo | - |
| T1.1.5 | Implement `std::error::Error` for all error types | Todo | - |
| T1.1.6 | Add `From<serde_yaml::Error>` for `ParseError` | Todo | - |
| T1.1.7 | Add `From<SessionError>` for `RuntimeError` | Todo | - |
| T1.1.8 | Write unit tests for error Display messages | Todo | - |

#### Acceptance Criteria

- All error variants can be constructed and formatted.
- `?` operator works across parser ↔ runtime boundaries via `DslError`.
- No panics in library code for any error path.

#### Technical Notes

```rust
pub enum ParseError {
    YamlSyntax(String),
    UnknownNodeKind { kind: String },
    UnknownEvaluatorKind { kind: String },
    UnknownParserKind { kind: String },
    MissingProperty(&'static str),
    MissingRules,
    DuplicatePriority { priority: u32 },
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

pub enum RuntimeError {
    MissingVariable { key: String },
    UnknownFilter { filter: String },
    FilterError(String),
    TypeMismatch { key: String, expected: &'static str, got: String },
    Session { kind: SessionErrorKind, message: String },
    MaxRecursion,
    SubTreeNotResolved { name: String },
    Custom(String),
}
```

---

### Story 1.2: External Dependency Traits

**Priority**: P0
**Effort**: 5 points
**Status**: Backlog

Implement all injectable host-capability traits. The engine must not depend on any `agent-*` crate; all IO comes through these traits.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T1.2.1 | Define `Session` trait (`send`, `send_with_hint`, `is_ready`, `receive`) | Todo | - |
| T1.2.2 | Define `SessionError` + `SessionErrorKind` | Todo | - |
| T1.2.3 | Define `Clock` trait + `SystemClock` + `MockClock` | Todo | - |
| T1.2.4 | Define `Logger` trait + `LogLevel` enum | Todo | - |
| T1.2.5 | Implement `NullLogger` and `StderrLogger` | Todo | - |
| T1.2.6 | Define `Fs` trait (`read_to_string`, `read_dir`, `modified`) | Todo | - |
| T1.2.7 | Define `FsError` enum | Todo | - |
| T1.2.8 | Implement `StdFs` using `std::fs` | Todo | - |
| T1.2.9 | Write unit tests for `MockClock` | Todo | - |
| T1.2.10 | Write unit tests for `NullLogger` / `StderrLogger` | Todo | - |

#### Acceptance Criteria

- All traits are object-safe (`dyn Trait` usable).
- `MockClock::advance()` works deterministically.
- `StdFs` delegates correctly to `std::fs`.
- No `agent-*` types appear in trait signatures.

#### Technical Notes

```rust
pub trait Session {
    fn send(&mut self, message: &str) -> Result<(), SessionError>;
    fn send_with_hint(&mut self, message: &str, model: &str) -> Result<(), SessionError>;
    fn is_ready(&self) -> bool;
    fn receive(&mut self) -> Result<String, SessionError>;
}

pub trait Clock {
    fn now(&self) -> Instant;
}

pub trait Logger {
    fn log(&self, level: LogLevel, target: &str, msg: &str);
}

pub trait Fs {
    fn read_to_string(&self, path: &Path) -> Result<String, FsError>;
    fn read_dir(&self, path: &Path) -> Result<Vec<PathBuf>, FsError>;
    fn modified(&self, path: &Path) -> Result<SystemTime, FsError>;
}
```

---

### Story 1.3: Blackboard & BlackboardValue

**Priority**: P0
**Effort**: 5 points
**Status**: Backlog

Implement the scoped typed state store. All nodes read from and write to the blackboard during execution.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T1.3.1 | Define `BlackboardValue` enum (String, Integer, Float, Boolean, List, Map) | Todo | - |
| T1.3.2 | Define `Blackboard` struct with built-in typed fields | Todo | - |
| T1.3.3 | Implement `Blackboard::new()` and `Default` | Todo | - |
| T1.3.4 | Implement `Blackboard::with_capacity(n)` | Todo | - |
| T1.3.5 | Implement `push_scope()` / `pop_scope()` for variable scoping | Todo | - |
| T1.3.6 | Implement `set()` / `get()` for scoped variable access | Todo | - |
| T1.3.7 | Implement `get_path()` with dot-notation (built-in fields + scoped vars) | Todo | - |
| T1.3.8 | Implement typed getters (`get_string`, `get_bool`, `get_u8`, `get_f64`) | Todo | - |
| T1.3.9 | Implement `push_command()` / `drain_commands()` | Todo | - |
| T1.3.10 | Implement `store_llm_response()` | Todo | - |
| T1.3.11 | Write unit tests for scoped variable read/write | Todo | - |
| T1.3.12 | Write unit tests for `get_path()` dot-notation | Todo | - |

#### Acceptance Criteria

- `Blackboard::default()` compiles and produces empty state.
- Scope push/pop correctly isolates variables (inner writes invisible to outer).
- `get_path("last_tool_call.name")` resolves nested Map fields.
- `get_path("file_changes.0.path")` resolves List indexing.

#### Technical Notes

```rust
#[derive(Default)]
pub struct Blackboard {
    pub task_description: String,
    pub provider_output: String,
    pub context_summary: String,
    pub reflection_round: u8,
    pub max_reflection_rounds: u8,
    pub confidence_accumulator: f64,
    pub agent_id: String,
    pub current_task_id: String,
    pub current_story_id: String,
    pub last_tool_call: Option<ToolCallRecord>,
    pub file_changes: Vec<FileChangeRecord>,
    pub project_rules: ProjectRules,
    pub decision_history: Vec<DecisionRecord>,
    scopes: Vec<HashMap<String, BlackboardValue>>,
    pub commands: Vec<DecisionCommand>,
    pub llm_responses: HashMap<String, String>,
}
```

---

### Story 1.4: Grouped Command Enum

**Priority**: P0
**Effort**: 3 points
**Status**: Backlog

Implement the `DecisionCommand` hierarchy. Commands are the only output of the DSL engine.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T1.4.1 | Define `DecisionCommand` grouped enum | Todo | - |
| T1.4.2 | Define `AgentCommand` (5 variants) with serde renames | Todo | - |
| T1.4.3 | Define `GitCommand` (5 variants) with serde renames | Todo | - |
| T1.4.4 | Define `TaskCommand` (3 variants) with serde renames | Todo | - |
| T1.4.5 | Define `HumanCommand` (3 variants) with serde renames | Todo | - |
| T1.4.6 | Define `ProviderCommand` (4 variants) | Todo | - |
| T1.4.7 | Derive `Debug`, `Clone`, `PartialEq`, `Eq`, `Serialize`, `Deserialize` | Todo | - |
| T1.4.8 | Write unit tests for YAML round-trip serialization | Todo | - |

#### Acceptance Criteria

- All 22 flat variants from the old design are grouped into 5 categories.
- YAML serialization produces full DSL names (`CommitChanges`, `EscalateToHuman`, etc.).
- `worktree_path` is carried in `DecisionCommand::Git(_, Option<String>)` tuple.

#### Technical Notes

```rust
pub enum DecisionCommand {
    Agent(AgentCommand),
    Git(GitCommand, Option<String>),
    Task(TaskCommand),
    Human(HumanCommand),
    Provider(ProviderCommand),
}
```

Serde rename mapping:
- `SendInstruction` → `SendCustomInstruction`
- `Terminate` → `TerminateAgent`
- `Commit { wip }` → field `is_wip`
- `CreateBranch { name, base }` → fields `branch_name`, `base_branch`
- `Rebase { base }` → field `base_branch`
- `PrepareStart` → `PrepareTaskStart`
- `Escalate` → `EscalateToHuman`

---

## Sprint Completion Criteria

- [ ] `cargo check` passes for the `decision-dsl` crate.
- [ ] `cargo test --lib` passes with ≥90% coverage on error, blackboard, and command modules.
- [ ] All traits have mock implementations usable in downstream tests.
- [ ] No dependency on any `agent-*` crate.
