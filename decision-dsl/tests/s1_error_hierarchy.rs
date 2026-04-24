use decision_dsl::ext::error::{DslError, ParseError, RuntimeError, SessionErrorKind};
use std::error::Error;

// ── ParseError ──────────────────────────────────────────────────────────────

#[test]
fn parse_error_display_invalid_yaml() {
    let e = ParseError::InvalidYaml { detail: "bad indent".into() };
    assert_eq!(e.to_string(), "invalid YAML: bad indent");
}

#[test]
fn parse_error_display_missing_field() {
    let e = ParseError::MissingField { field: "name".into(), node: "selector".into() };
    assert_eq!(e.to_string(), "missing field 'name' in node 'selector'");
}

#[test]
fn parse_error_display_unknown_node_type() {
    let e = ParseError::UnknownNodeType { kind: "foo".into() };
    assert_eq!(e.to_string(), "unknown node type: foo");
}

#[test]
fn parse_error_display_duplicate_tree_id() {
    let e = ParseError::DuplicateTreeId { id: "root".into() };
    assert_eq!(e.to_string(), "duplicate tree id: root");
}

#[test]
fn parse_error_display_invalid_template() {
    let e = ParseError::InvalidTemplate { detail: "unclosed".into() };
    assert_eq!(e.to_string(), "invalid template: unclosed");
}

#[test]
fn parse_error_display_invalid_regex() {
    let e = ParseError::InvalidRegex { detail: "missing )".into() };
    assert_eq!(e.to_string(), "invalid regex: missing )");
}

#[test]
fn parse_error_display_invalid_timeout() {
    let e = ParseError::InvalidTimeout { value: "abc".into() };
    assert_eq!(e.to_string(), "invalid timeout value: abc");
}

#[test]
fn parse_error_display_invalid_evaluator() {
    let e = ParseError::InvalidEvaluator { name: "bar".into() };
    assert_eq!(e.to_string(), "invalid evaluator: bar");
}

#[test]
fn parse_error_display_missing_subtree() {
    let e = ParseError::MissingSubtree { id: "sub".into() };
    assert_eq!(e.to_string(), "missing subtree definition: sub");
}

#[test]
fn parse_error_display_cycle_detected() {
    let e = ParseError::CycleDetected { ids: vec!["a".into(), "b".into(), "a".into()] };
    assert_eq!(e.to_string(), "cycle detected in tree references: [a, b, a]");
}

#[test]
fn parse_error_display_invalid_path() {
    let e = ParseError::InvalidPath { path: "...".into() };
    assert_eq!(e.to_string(), "invalid blackboard path: ...");
}

#[test]
fn parse_error_display_invalid_case_key() {
    let e = ParseError::InvalidCaseKey { key: "".into() };
    assert_eq!(e.to_string(), "invalid case key: ''");
}

#[test]
fn parse_error_display_mixed_case_types() {
    let e = ParseError::MixedCaseTypes;
    assert_eq!(e.to_string(), "mixed case types in switch (string/int)");
}

#[test]
fn parse_error_display_invalid_enum_case() {
    let e = ParseError::InvalidEnumCase { case: "Foo".into(), allowed: vec!["A".into(), "B".into()] };
    assert_eq!(e.to_string(), "invalid enum case 'Foo', allowed: [A, B]");
}

#[test]
fn parse_error_display_missing_default_case() {
    let e = ParseError::MissingDefaultCase;
    assert_eq!(e.to_string(), "switch missing default case");
}

#[test]
fn parse_error_display_invalid_bundle_format() {
    let e = ParseError::InvalidBundleFormat { detail: "not a map".into() };
    assert_eq!(e.to_string(), "invalid bundle format: not a map");
}

#[test]
fn parse_error_display_invalid_api_version() {
    let e = ParseError::InvalidApiVersion { version: "v0".into() };
    assert_eq!(e.to_string(), "invalid API version: v0");
}

#[test]
fn parse_error_display_invalid_desugaring() {
    let e = ParseError::InvalidDesugaring { detail: "x".into() };
    assert_eq!(e.to_string(), "invalid desugaring: x");
}

#[test]
fn parse_error_display_missing_on_error_handler() {
    let e = ParseError::MissingOnErrorHandler { node: "X".into() };
    assert_eq!(e.to_string(), "missing on_error handler in node 'X'");
}

// ── RuntimeError ────────────────────────────────────────────────────────────

#[test]
fn runtime_error_display_node_not_found() {
    let e = RuntimeError::NodeNotFound { path: vec![0, 1] };
    assert_eq!(e.to_string(), "node not found at path [0, 1]");
}

#[test]
fn runtime_error_display_invalid_blackboard_access() {
    let e = RuntimeError::InvalidBlackboardAccess { path: "a.b".into() };
    assert_eq!(e.to_string(), "invalid blackboard access: a.b");
}

#[test]
fn runtime_error_display_evaluator_failure() {
    let e = RuntimeError::EvaluatorFailure { name: "eq".into(), detail: "type mismatch".into() };
    assert_eq!(e.to_string(), "evaluator 'eq' failed: type mismatch");
}

#[test]
fn runtime_error_display_template_render_failure() {
    let e = RuntimeError::TemplateRenderFailure { detail: "missing var".into() };
    assert_eq!(e.to_string(), "template render failure: missing var");
}

#[test]
fn runtime_error_display_prompt_timeout() {
    let e = RuntimeError::PromptTimeout { node: "ask".into(), timeout_ms: 5000 };
    assert_eq!(e.to_string(), "prompt node 'ask' timed out after 5000ms");
}

#[test]
fn runtime_error_display_session_error() {
    let e = RuntimeError::SessionError { kind: SessionErrorKind::SendFailed, message: "closed".into() };
    assert_eq!(e.to_string(), "session error (SendFailed): closed");
}

#[test]
fn runtime_error_display_max_reflection_exceeded() {
    let e = RuntimeError::MaxReflectionExceeded;
    assert_eq!(e.to_string(), "max reflection rounds exceeded");
}

#[test]
fn runtime_error_display_cooldown_active() {
    let e = RuntimeError::CooldownActive { node: "cool".into(), remaining_ms: 42 };
    assert_eq!(e.to_string(), "cooldown active on node 'cool', 42ms remaining");
}

// ── DslError union ──────────────────────────────────────────────────────────

#[test]
fn dsl_error_from_parse() {
    let p = ParseError::InvalidYaml { detail: "bad".into() };
    let d: DslError = p.into();
    assert_eq!(d.to_string(), "parse error: invalid YAML: bad");
}

#[test]
fn dsl_error_from_runtime() {
    let r = RuntimeError::NodeNotFound { path: vec![0] };
    let d: DslError = r.into();
    assert_eq!(d.to_string(), "runtime error: node not found at path [0]");
}

#[test]
fn dsl_error_source_parse() {
    let p = ParseError::MissingField { field: "f".into(), node: "n".into() };
    let d = DslError::Parse(p);
    assert!(d.source().is_none());
}

#[test]
fn dsl_error_source_runtime() {
    let r = RuntimeError::EvaluatorFailure { name: "e".into(), detail: "d".into() };
    let d = DslError::Runtime(r);
    assert!(d.source().is_none());
}

// ── From impls ──────────────────────────────────────────────────────────────

#[test]
fn from_serde_yaml_error() {
    let yaml_err = serde_yaml::from_str::<i32>("not a number").unwrap_err();
    let dsl: DslError = yaml_err.into();
    assert!(dsl.to_string().starts_with("parse error: invalid YAML:"));
}

#[test]
fn from_serde_json_error() {
    let json_err = serde_json::from_str::<i32>("bad").unwrap_err();
    let dsl: DslError = json_err.into();
    assert!(dsl.to_string().starts_with("parse error: invalid YAML:"));
}

// ── Error trait ─────────────────────────────────────────────────────────────

#[test]
fn parse_error_implements_std_error() {
    fn assert_std_error<T: std::error::Error>() {}
    assert_std_error::<ParseError>();
}

#[test]
fn runtime_error_implements_std_error() {
    fn assert_std_error<T: std::error::Error>() {}
    assert_std_error::<RuntimeError>();
}

#[test]
fn dsl_error_implements_std_error() {
    fn assert_std_error<T: std::error::Error>() {}
    assert_std_error::<DslError>();
}
