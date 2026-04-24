# Decision DSL: External Traits & Operations

> External dependency traits and operational specification for the decision DSL engine. Covers all injectable host capabilities (Session, Clock, Logger, Watcher), error handling, testing strategy, performance considerations, and observability.

---

## 1. External Dependency Traits

### 1.1 Session

```rust
/// Abstraction over the ongoing codex/claude session.
pub trait Session {
    /// Send a message to the LLM session. Returns immediately.
    fn send(&mut self, message: &str) -> Result<(), SessionError>;

    /// Send with a model hint (e.g., "thinking" or "standard").
    /// Default implementation delegates to send().
    fn send_with_hint(&mut self, message: &str, model: &str) -> Result<(), SessionError> {
        let _ = model;
        self.send(message)
    }

    /// Check if a reply is available.
    fn is_ready(&self) -> bool;

    /// Receive the reply. Call only after is_ready() returns true.
    fn receive(&mut self) -> Result<String, SessionError>;
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
    SendFailed,
}
```

### 1.2 Clock

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

### 1.3 Logger

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

## 2. Error Handling

All errors are explicit enums. No panics in library code.

### 2.1 Error Types

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

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ParseError::YamlSyntax(e) => write!(f, "YAML syntax error: {}", e),
            ParseError::UnknownNodeKind { kind } => write!(f, "unknown node kind: {}", kind),
            ParseError::UnknownEvaluatorKind { kind } => write!(f, "unknown evaluator kind: {}", kind),
            ParseError::UnknownParserKind { kind } => write!(f, "unknown parser kind: {}", kind),
            ParseError::MissingProperty(p) => write!(f, "missing required property: {}", p),
            ParseError::MissingRules => write!(f, "DecisionRules must have at least one rule"),
            ParseError::DuplicatePriority { priority } => write!(f, "duplicate rule priority: {}", priority),
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
    SubTreeNotResolved { name: String },
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
            RuntimeError::SubTreeNotResolved { name } => write!(f, "subtree '{}' not resolved", name),
            RuntimeError::Custom(msg) => write!(f, "{}", msg),
        }
    }
}

impl std::error::Error for RuntimeError {}
```

### 2.2 Error Conversion

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

## 3. Testing Strategy

Every component is tested with mock trait implementations. No I/O, no LLM calls in unit tests.

### 3.1 Mock Implementations

```rust
use std::cell::RefCell;
use std::collections::VecDeque;

pub struct MockSession {
    replies: RefCell<VecDeque<String>>,
    ready: RefCell<bool>,
    sent_messages: RefCell<Vec<String>>,
}

impl MockSession {
    pub fn new(replies: Vec<String>) -> Self {
        Self {
            replies: RefCell::new(replies.into_iter().collect()),
            ready: RefCell::new(true),
            sent_messages: RefCell::new(Vec::new()),
        }
    }
    pub fn set_ready(&self, ready: bool) { *self.ready.borrow_mut() = ready; }
    pub fn sent_messages(&self) -> Vec<String> { self.sent_messages.borrow().clone() }
}

impl Session for MockSession {
    fn send(&mut self, message: &str) -> Result<(), SessionError> {
        self.sent_messages.borrow_mut().push(message.to_string());
        Ok(())
    }

    fn send_with_hint(&mut self, message: &str, model: &str) -> Result<(), SessionError> {
        self.sent_messages.borrow_mut().push(format!("[{}] {}", model, message));
        Ok(())
    }

    fn is_ready(&self) -> bool { *self.ready.borrow() }

    fn receive(&mut self) -> Result<String, SessionError> {
        self.replies.borrow_mut().pop_front().ok_or_else(|| SessionError {
            kind: SessionErrorKind::Unavailable,
            message: "no more replies".into(),
        })
    }
}

pub struct CaptureLogger {
    pub logs: RefCell<Vec<(LogLevel, String, String)>>,
}

impl CaptureLogger {
    pub fn new() -> Self { Self { logs: RefCell::new(Vec::new()) } }
}

impl Logger for CaptureLogger {
    fn log(&self, level: LogLevel, target: &str, msg: &str) {
        self.logs.borrow_mut().push((level, target.to_string(), msg.to_string()));
    }
}
```

### 3.2 Unit Tests: Evaluators (Enum-Based)

```rust
#[test]
fn output_contains_case_insensitive() {
    let mut bb = Blackboard::new();
    bb.provider_output = "Error 429: Rate Limit".into();

    let eval = Evaluator::OutputContains { pattern: "429".into(), case_sensitive: false };
    assert!(eval.evaluate(&bb).unwrap());

    let eval = Evaluator::OutputContains { pattern: "rate limit".into(), case_sensitive: false };
    assert!(eval.evaluate(&bb).unwrap());

    let eval = Evaluator::OutputContains { pattern: "quota".into(), case_sensitive: false };
    assert!(!eval.evaluate(&bb).unwrap());
}

#[test]
fn or_evaluator_short_circuits() {
    let mut bb = Blackboard::new();
    bb.set("a", BlackboardValue::Boolean(false));
    bb.set("b", BlackboardValue::Boolean(true));

    let eval = Evaluator::Or {
        conditions: vec![
            Evaluator::VariableIs { key: "a".into(), expected: BlackboardValue::Boolean(true) },
            Evaluator::VariableIs { key: "b".into(), expected: BlackboardValue::Boolean(true) },
        ],
    };
    assert!(eval.evaluate(&bb).unwrap());
}

#[test]
fn not_inverts() {
    let mut bb = Blackboard::new();
    bb.provider_output = "success".into();

    let eval = Evaluator::Not {
        condition: Box::new(Evaluator::OutputContains {
            pattern: "error".into(), case_sensitive: false,
        }),
    };
    assert!(eval.evaluate(&bb).unwrap());
}
```

### 3.3 Unit Tests: Output Parsers (Enum-Based)

```rust
#[test]
fn enum_parser_case_insensitive() {
    let parser = OutputParser::Enum {
        values: vec!["REFLECT".into(), "CONFIRM".into()],
        case_sensitive: false,
    };
    let result = parser.parse("  reflect  ").unwrap();
    assert_eq!(result.get("decision"), Some(&BlackboardValue::String("REFLECT".into())));
}

#[test]
fn structured_parser_with_types() {
    let parser = OutputParser::Structured {
        pattern: r"CLASS:\s*(\w+)\s*RECOMMEND:\s*(\w+)\s*CONFIDENCE:\s*(\d+\.\d+)".into(),
        fields: vec![
            StructuredField { name: "classification".into(), group: 1, ty: FieldType::String },
            StructuredField { name: "recommendation".into(), group: 2, ty: FieldType::String },
            StructuredField { name: "confidence".into(), group: 3, ty: FieldType::Float },
        ],
    };
    let result = parser.parse("CLASS: SYNTAX RECOMMEND: FIX CONFIDENCE: 0.85").unwrap();
    assert_eq!(result.get("classification"), Some(&BlackboardValue::String("SYNTAX".into())));
    assert_eq!(result.get("confidence"), Some(&BlackboardValue::Float(0.85)));
}
```

### 3.4 Integration Tests: DecisionRules Desugaring

```rust
#[test]
fn decision_rules_desugars_to_selector() {
    let yaml = r#"
apiVersion: decision.agile-agent.io/v1
kind: DecisionRules
metadata:
  name: test-rules
spec:
  rules:
    - priority: 1
      name: rate_limit
      if:
        kind: outputContains
        pattern: "429"
      then:
        command: ApproveAndContinue
    - priority: 99
      name: default
      then:
        command: ApproveAndContinue
"#;

    let parser = YamlParser::new();
    let doc = parser.parse_document(yaml).unwrap();
    let tree = doc.desugar(&EvaluatorRegistry::new()).unwrap();

    // The desugared tree should be a Selector with 3 children
    // (2 rules + 1 automatic no_match fallback)
    match &tree.spec.root {
        Node::Selector(sel) => {
            assert_eq!(sel.children.len(), 3);
        }
        _ => panic!("expected Selector root"),
    }
}

#[test]
fn switch_on_prompt_desugars_to_sequence() {
    let yaml = r#"
apiVersion: decision.agile-agent.io/v1
kind: DecisionRules
metadata:
  name: test
spec:
  rules:
    - priority: 1
      name: decide
      if:
        kind: outputContains
        pattern: "claims_completion"
      then:
        kind: Switch
        name: reflect_or_confirm
        on:
          kind: prompt
          template: "REFLECT or CONFIRM?"
          parser:
            kind: enum
            values: [REFLECT, CONFIRM]
        cases:
          REFLECT:
            command:
              Reflect:
                prompt: "Review"
          CONFIRM:
            command: ConfirmCompletion
      reflectionMaxRounds: 2
"#;

    let parser = YamlParser::new();
    let doc = parser.parse_document(yaml).unwrap();
    let tree = doc.desugar(&EvaluatorRegistry::new()).unwrap();

    // Verify the structure (should contain ReflectionGuard)
    if let Node::Selector(sel) = &tree.spec.root {
        if let Node::ReflectionGuard(rg) = &sel.children[0] {
            assert_eq!(rg.max_rounds, 2);
        } else {
            panic!("expected ReflectionGuard, got {:?}", sel.children[0].name());
        }
    } else {
        panic!("expected Selector root");
    }
}

#[test]
fn sub_tree_preserves_identity() {
    let mut bb = Blackboard::new();
    bb.provider_output = "test".into();

    let mut sub_node = SubTreeNode {
        name: "handler".into(),
        ref_name: "reflect_loop".into(),
        resolved_root: Some(Box::new(Node::Action(ActionNode {
            name: "inner".into(),
            command: DecisionCommand::Agent(AgentCommand::ApproveAndContinue),
            when: None,
        }))),
    };

    let mut session = MockSession::new(vec![]);
    let clock = SystemClock;
    let logger = NullLogger;
    let mut tracer = Tracer::new();
    let mut ctx = TickContext::new(&mut bb, &mut session, &clock, &logger);

    let status = sub_node.tick(&mut ctx, &mut tracer).unwrap();
    assert_eq!(status, NodeStatus::Success);

    // Should have enter_subtree and exit_subtree trace entries
    let entries = tracer.into_entries();
    let has_enter = entries.iter().any(|e| matches!(e, TraceEntry::EnterSubTree { .. }));
    let has_exit = entries.iter().any(|e| matches!(e, TraceEntry::ExitSubTree { .. }));
    assert!(has_enter, "trace should contain EnterSubTree");
    assert!(has_exit, "trace should contain ExitSubTree");
}
```

### 3.5 Test Coverage Requirements

| Component | Coverage Target |
|-----------|----------------|
| Parser (YAML → DslDocument) | 95% — all node kinds, error paths, DecisionRules and BehaviorTree formats |
| Desugaring (DslDocument → Tree) | 100% — all high-level constructs, edge cases |
| Evaluators | 100% — all builtin evaluators |
| Output Parsers | 100% — enum, structured, json, command |
| Executor / Tick | 95% — all node types, resume paths |
| Blackboard | 100% — all types, dot notation, scoped access, push/pop scope |
| Template Engine | 90% — variable interpolation, filters, conditionals, loops (delegated to minijinja) |
| Prompt Node | 90% — pending, ready, parse fail, command parser |
| SubTree Node | 90% — scope isolation, identity in traces |
| Error Types | 100% — Display, From impls |

---

## 4. Performance Considerations

### 4.1 Allocation Strategy

```rust
impl Blackboard {
    pub fn with_capacity(n: usize) -> Self {
        Self {
            commands: Vec::with_capacity(8),
            scopes: vec![HashMap::with_capacity(n)],
            ..Default::default()
        }
    }
}
```

### 4.2 Enum vs Trait Object

Switching from `Box<dyn Evaluator>` to `Evaluator` enum eliminates:
- One heap allocation per Condition/Action node
- Vtable dispatch overhead per evaluation
- `dyn_clone` dependency

Benchmark target: 1M evaluator calls/second on a single core.

### 4.3 Memory Footprint

```
For a rule set with 10 rules (typical):
  - Parse: ~20KB (YAML + AST)
  - Desugared AST: ~10KB
  - Runtime per tick: ~2KB (blackboard) + trace entries
  - Per-tick allocations: 0 (no Prompt) or ~1KB (with Prompt)
```

### 4.4 Parallel Safety

Trees are `Send + Sync`. The executor is `!Sync` (mutates running_path). One executor + blackboard per agent thread.

---

## 5. Observability

### 5.1 TraceEntry (Updated)

```rust
#[derive(Debug, Clone)]
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

### 5.2 Tree Visualization (ASCII)

```rust
pub fn render_trace_ascii(trace: &[TraceEntry]) -> String {
    let mut output = String::new();
    for entry in trace {
        match entry {
            TraceEntry::EnterSubTree { name, ref_name } => {
                output.push_str(&format!("┌─ SubTree: {} (→ {})\n", name, ref_name));
            }
            TraceEntry::ExitSubTree { name, status, .. } => {
                let symbol = match status {
                    NodeStatus::Success => "✓",
                    NodeStatus::Failure => "✗",
                    NodeStatus::Running => "…",
                };
                output.push_str(&format!("└─ {} — {}\n", name, symbol));
            }
            TraceEntry::Enter { node_name, depth, .. } => {
                let indent = "  ".repeat(*depth);
                output.push_str(&format!("{}├─ {}\n", indent, node_name));
            }
            TraceEntry::Action { command, .. } => {
                output.push_str(&format!("  → {}\n", command));
            }
            TraceEntry::RuleMatched { rule_name, priority } => {
                output.push_str(&format!("▶ Rule: {} (priority {})\n", rule_name, priority));
            }
            _ => {}
        }
    }
    output
}
```

### 5.3 Metrics (Optional)

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

*Document version: 2.0*
*Last updated: 2026-04-24*
