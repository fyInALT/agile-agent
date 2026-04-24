use decision_dsl::ext::error::{DslError, ParseError, RuntimeError, SessionErrorKind};
use std::error::Error;

// ── ParseError ──────────────────────────────────────────────────────────────

#[test]
fn parse_error_yaml_syntax() {
    let e = ParseError::YamlSyntax("bad indent".into());
    assert_eq!(e.to_string(), "YAML syntax error: bad indent");
}

#[test]
fn parse_error_unknown_node_kind() {
    let e = ParseError::UnknownNodeKind { kind: "foo".into() };
    assert_eq!(e.to_string(), "unknown node kind: foo");
}

#[test]
fn parse_error_unknown_evaluator_kind() {
    let e = ParseError::UnknownEvaluatorKind { kind: "bar".into() };
    assert_eq!(e.to_string(), "unknown evaluator kind: bar");
}

#[test]
fn parse_error_unknown_parser_kind() {
    let e = ParseError::UnknownParserKind { kind: "baz".into() };
    assert_eq!(e.to_string(), "unknown parser kind: baz");
}

#[test]
fn parse_error_missing_property() {
    let e = ParseError::MissingProperty("name");
    assert_eq!(e.to_string(), "missing required property: name");
}

#[test]
fn parse_error_missing_rules() {
    let e = ParseError::MissingRules;
    assert_eq!(e.to_string(), "DecisionRules must have at least one rule");
}

#[test]
fn parse_error_duplicate_priority() {
    let e = ParseError::DuplicatePriority { priority: 5 };
    assert_eq!(e.to_string(), "duplicate rule priority: 5");
}

#[test]
fn parse_error_invalid_property() {
    let e = ParseError::InvalidProperty {
        key: "timeout".into(),
        value: "abc".into(),
        reason: "not a number".into(),
    };
    assert_eq!(
        e.to_string(),
        "invalid property 'timeout' = 'abc': not a number"
    );
}

#[test]
fn parse_error_unresolved_subtree() {
    let e = ParseError::UnresolvedSubTree { name: "reflect".into() };
    assert_eq!(e.to_string(), "unresolved subtree reference: reflect");
}

#[test]
fn parse_error_circular_subtree_ref() {
    let e = ParseError::CircularSubTreeRef { name: "loop".into() };
    assert_eq!(e.to_string(), "circular subtree reference: loop");
}

#[test]
fn parse_error_duplicate_name() {
    let e = ParseError::DuplicateName { name: "root".into() };
    assert_eq!(e.to_string(), "duplicate node name: root");
}

#[test]
fn parse_error_unexpected_value() {
    let e = ParseError::UnexpectedValue {
        got: "MAYBE".into(),
        expected: vec!["YES".into(), "NO".into()],
    };
    assert_eq!(
        e.to_string(),
        "unexpected value 'MAYBE', expected one of: [\"YES\", \"NO\"]"
    );
}

#[test]
fn parse_error_no_match() {
    let e = ParseError::NoMatch { pattern: "CLASS:".into() };
    assert_eq!(e.to_string(), "no match for pattern: CLASS:");
}

#[test]
fn parse_error_missing_capture_group() {
    let e = ParseError::MissingCaptureGroup {
        group: 2,
        pattern: "(\\w+)".into(),
    };
    assert_eq!(
        e.to_string(),
        "missing capture group 2 in pattern: (\\w+)"
    );
}

#[test]
fn parse_error_type_mismatch() {
    let e = ParseError::TypeMismatch {
        field: "max_attempts".into(),
        expected: "integer",
        got: "foo".into(),
    };
    assert_eq!(
        e.to_string(),
        "type mismatch for field 'max_attempts': expected integer, got foo"
    );
}

#[test]
fn parse_error_json_syntax() {
    let e = ParseError::JsonSyntax("unexpected token".into());
    assert_eq!(e.to_string(), "JSON syntax error: unexpected token");
}

#[test]
fn parse_error_unsupported_version() {
    let e = ParseError::UnsupportedVersion("v99".into());
    assert_eq!(e.to_string(), "unsupported api version: v99");
}

#[test]
fn parse_error_custom() {
    let e = ParseError::Custom("something went wrong".into());
    assert_eq!(e.to_string(), "something went wrong");
}

// ── RuntimeError ────────────────────────────────────────────────────────────

#[test]
fn runtime_error_missing_variable() {
    let e = RuntimeError::MissingVariable { key: "foo".into() };
    assert_eq!(e.to_string(), "missing variable: foo");
}

#[test]
fn runtime_error_unknown_filter() {
    let e = RuntimeError::UnknownFilter { filter: "bad".into() };
    assert_eq!(e.to_string(), "unknown filter: bad");
}

#[test]
fn runtime_error_filter_error() {
    let e = RuntimeError::FilterError("template error".into());
    assert_eq!(e.to_string(), "filter error: template error");
}

#[test]
fn runtime_error_type_mismatch() {
    let e = RuntimeError::TypeMismatch {
        key: "count".into(),
        expected: "integer",
        got: "string".into(),
    };
    assert_eq!(
        e.to_string(),
        "type mismatch for 'count': expected integer, got string"
    );
}

#[test]
fn runtime_error_session() {
    let e = RuntimeError::Session {
        kind: SessionErrorKind::Timeout,
        message: "timed out".into(),
    };
    assert_eq!(
        e.to_string(),
        "session error (Timeout): timed out"
    );
}

#[test]
fn runtime_error_max_recursion() {
    let e = RuntimeError::MaxRecursion;
    assert_eq!(e.to_string(), "maximum recursion depth exceeded");
}

#[test]
fn runtime_error_subtree_not_resolved() {
    let e = RuntimeError::SubTreeNotResolved { name: "nested".into() };
    assert_eq!(e.to_string(), "subtree 'nested' not resolved");
}

#[test]
fn runtime_error_custom() {
    let e = RuntimeError::Custom("oops".into());
    assert_eq!(e.to_string(), "oops");
}

// ── DslError union ──────────────────────────────────────────────────────────

#[test]
fn dsl_error_from_parse() {
    let p = ParseError::MissingRules;
    let d: DslError = p.into();
    assert_eq!(
        d.to_string(),
        "parse error: DecisionRules must have at least one rule"
    );
}

#[test]
fn dsl_error_from_runtime() {
    let r = RuntimeError::MissingVariable { key: "x".into() };
    let d: DslError = r.into();
    assert_eq!(d.to_string(), "runtime error: missing variable: x");
}

#[test]
fn dsl_error_source() {
    let p = ParseError::YamlSyntax("bad".into());
    let d = DslError::Parse(p);
    assert!(d.source().is_none());
}

// ── From impls ──────────────────────────────────────────────────────────────

#[test]
fn from_serde_yaml_error() {
    let yaml_err = serde_yaml::from_str::<i32>("not a number").unwrap_err();
    let dsl: DslError = yaml_err.into();
    assert!(dsl
        .to_string()
        .starts_with("parse error: YAML syntax error:"));
}

#[test]
fn from_serde_json_error() {
    let json_err = serde_json::from_str::<i32>("bad").unwrap_err();
    let dsl: DslError = json_err.into();
    assert!(dsl.to_string().starts_with("parse error: JSON syntax error:"));
}

#[test]
fn from_session_error_to_runtime() {
    use decision_dsl::ext::error::SessionError;
    let se = SessionError {
        kind: SessionErrorKind::SendFailed,
        message: "closed".into(),
    };
    let re: RuntimeError = se.into();
    assert_eq!(
        re.to_string(),
        "session error (SendFailed): closed"
    );
}

#[test]
fn from_session_error_to_dsl() {
    use decision_dsl::ext::error::SessionError;
    let se = SessionError {
        kind: SessionErrorKind::Unavailable,
        message: "no reply".into(),
    };
    let de: DslError = se.into();
    assert!(de
        .to_string()
        .starts_with("runtime error: session error (Unavailable)"));
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
