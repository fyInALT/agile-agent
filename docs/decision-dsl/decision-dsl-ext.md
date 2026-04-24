# Decision DSL: External Traits & Operations

> External dependency traits and operational specification for the decision DSL engine. Covers all injectable host capabilities (Session, Clock, Fs, Logger, Watcher), error handling, testing strategy, performance considerations, validation/hot-reload, and observability.
>
> This document is a chapter of the [Decision DSL Implementation](decision-dsl-implementation.md).

## External Dependency Traits

###1 Session

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

###2 Clock

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

###3 Fs

```rust
use std::path::{Path, PathBuf};
use std::time::SystemTime;

pub trait Fs {
    fn read_to_string(&self, path: &Path) -> Result<String, FsError>;
    fn read_dir(&self, path: &Path) -> Result<Vec<PathBuf>, FsError>;
    fn modified(&self, path: &Path) -> Result<SystemTime, FsError>;
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

    fn modified(&self, path: &Path) -> Result<SystemTime, FsError> {
        std::fs::metadata(path)
            .and_then(|m| m.modified())
            .map_err(|e| FsError {
                path: path.to_path_buf(), message: e.to_string(),
            })
    }
}
```

###4 Logger

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

/// Adapter to the `tracing` crate.
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

###5 Watcher

```rust
use std::path::Path;
use std::time::SystemTime;

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

## Error Handling

All errors are explicit enums. No panics in library code.

###1 Error Types

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

###2 Error Conversion

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

## Testing Strategy

Every component is tested with mock trait implementations. No I/O, no LLM calls in unit tests.

###1 Mock Implementations

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

###2 Unit Tests: Evaluators

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

###3 Unit Tests: Output Parsers

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

###4 Integration Tests: Full Tree Tick

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

###5 Template Engine Tests

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

###6 Test Coverage Requirements

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

## Performance Considerations

###1 Allocation Strategy

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

###2 Tick Loop Hot Path

```rust
// Benchmark target: 1M ticks/second on a 100-node composite tree
// (measured with criterion.rs)
// Only Prompt nodes allocate during tick.
```

###3 Memory Footprint

```rust
// Tree memory: one-time parse cost.
// Runtime memory: Blackboard + running path.
//
// For a tree with 100 nodes:
//   - Parse: ~50KB
//   - Runtime: ~2KB (blackboard) + path Vec
//   - Per-tick: 0 allocations (no Prompt), ~1KB (with Prompt)
```

###4 Parallel Safety

```rust
// Trees are Send + Sync (no internal mutability in Tree itself).
// Executor is !Sync (mutates running_path).
// Blackboard is !Sync (mutable during tick).
//
// Design: One Executor + Blackboard per agent thread.
```

---

## Validation & Hot Reload

###1 Validation Rules

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

    /// All SubTree references must resolve to known subtrees.
    pub fn validate_subtree_refs(&self, subtrees: &HashMap<String, Tree>) -> Result<(), ParseError> {
        self.spec.root.validate_subtree_refs_recursive(subtrees)
    }
}
```

###2 Hot Reload

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

## Observability

###1 NodeTrace

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

###2 Tracer Implementation

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

    pub fn exit(&mut self, node_name: &str, status: NodeStatus, duration: std::time::Duration, child_index: usize) {
        self.current_depth -= 1;
        self.entries.push(TraceEntry::Exit {
            node_name: node_name.to_string(),
            status,
            duration,
        });
        if status == NodeStatus::Running {
            self.running_path.push(child_index);
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

###3 Tree Visualization

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

###4 Metrics

```rust
/// Optional metrics integration (host-provided).
pub trait MetricsCollector {
    fn record_tick(&self, tree_name: &str, duration: std::time::Duration);
    fn record_node(&self, node_name: &str, node_type: &str, status: NodeStatus, duration: std::time::Duration);
    fn record_prompt(&self, node_name: &str, model: &str, latency_ms: u64, prompt_tokens: u32, completion_tokens: u32);
}
```

---

