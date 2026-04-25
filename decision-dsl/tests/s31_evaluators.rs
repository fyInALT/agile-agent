use decision_dsl::ast::Evaluator;
use decision_dsl::ext::blackboard::{Blackboard, BlackboardValue};

fn bb_with_output(output: &str) -> Blackboard {
    let mut bb = Blackboard::default();
    bb.provider_output = output.into();
    bb
}

fn bb_with_task(task: &str) -> Blackboard {
    let mut bb = Blackboard::default();
    bb.task_description = task.into();
    bb
}

fn bb_with_reflection(round: u8) -> Blackboard {
    let mut bb = Blackboard::default();
    bb.reflection_round = round;
    bb
}

fn bb_with_var(key: &str, value: BlackboardValue) -> Blackboard {
    let mut bb = Blackboard::default();
    bb.set(key, value);
    bb
}

// ── OutputContains ──────────────────────────────────────────────────────────

#[test]
fn output_contains_case_sensitive() {
    let bb = bb_with_output("Hello World");
    let eval = Evaluator::OutputContains {
        pattern: "World".into(),
        case_sensitive: true,
    };
    assert_eq!(eval.evaluate(&bb).unwrap(), true);
}

#[test]
fn output_contains_case_insensitive() {
    let bb = bb_with_output("Hello World");
    let eval = Evaluator::OutputContains {
        pattern: "world".into(),
        case_sensitive: false,
    };
    assert_eq!(eval.evaluate(&bb).unwrap(), true);
}

#[test]
fn output_contains_not_found() {
    let bb = bb_with_output("Hello World");
    let eval = Evaluator::OutputContains {
        pattern: "xyz".into(),
        case_sensitive: true,
    };
    assert_eq!(eval.evaluate(&bb).unwrap(), false);
}

// ── SituationIs ─────────────────────────────────────────────────────────────

#[test]
fn situation_is_matches() {
    let bb = bb_with_task("implement auth");
    let eval = Evaluator::SituationIs {
        situation_type: "implement auth".into(),
    };
    assert_eq!(eval.evaluate(&bb).unwrap(), true);
}

#[test]
fn situation_is_no_match() {
    let bb = bb_with_task("fix bug");
    let eval = Evaluator::SituationIs {
        situation_type: "implement auth".into(),
    };
    assert_eq!(eval.evaluate(&bb).unwrap(), false);
}

// ── ReflectionRoundUnder ────────────────────────────────────────────────────

#[test]
fn reflection_round_under_true() {
    let bb = bb_with_reflection(1);
    let eval = Evaluator::ReflectionRoundUnder { max: 2 };
    assert_eq!(eval.evaluate(&bb).unwrap(), true);
}

#[test]
fn reflection_round_under_false() {
    let bb = bb_with_reflection(3);
    let eval = Evaluator::ReflectionRoundUnder { max: 2 };
    assert_eq!(eval.evaluate(&bb).unwrap(), false);
}

// ── VariableIs ──────────────────────────────────────────────────────────────

#[test]
fn variable_is_string_match() {
    let bb = bb_with_var("status", BlackboardValue::String("ok".into()));
    let eval = Evaluator::VariableIs {
        key: "status".into(),
        expected: BlackboardValue::String("ok".into()),
    };
    assert_eq!(eval.evaluate(&bb).unwrap(), true);
}

#[test]
fn variable_is_int_match() {
    let bb = bb_with_var("count", BlackboardValue::Integer(42));
    let eval = Evaluator::VariableIs {
        key: "count".into(),
        expected: BlackboardValue::Integer(42),
    };
    assert_eq!(eval.evaluate(&bb).unwrap(), true);
}

#[test]
fn variable_is_dot_notation() {
    let mut bb = Blackboard::default();
    bb.task_description = "test".into();
    let eval = Evaluator::VariableIs {
        key: "task_description".into(),
        expected: BlackboardValue::String("test".into()),
    };
    assert_eq!(eval.evaluate(&bb).unwrap(), true);
}

// ── RegexMatch ──────────────────────────────────────────────────────────────

#[test]
fn regex_match_found() {
    let bb = bb_with_output("error: something went wrong");
    let eval = Evaluator::RegexMatch {
        pattern: r"error:".into(),
    };
    assert_eq!(eval.evaluate(&bb).unwrap(), true);
}

#[test]
fn regex_match_not_found() {
    let bb = bb_with_output("all good");
    let eval = Evaluator::RegexMatch {
        pattern: r"error:".into(),
    };
    assert_eq!(eval.evaluate(&bb).unwrap(), false);
}

#[test]
fn regex_invalid_pattern_fails() {
    let bb = bb_with_output("test");
    let eval = Evaluator::RegexMatch {
        pattern: r"(unclosed".into(),
    };
    let result = eval.evaluate(&bb);
    assert!(result.is_err());
}

// ── Script: basic comparisons ───────────────────────────────────────────────

#[test]
fn script_string_equality() {
    let bb = bb_with_output("hello");
    let eval = Evaluator::Script {
        expression: r#"provider_output == "hello""#.into(),
    };
    assert_eq!(eval.evaluate(&bb).unwrap(), true);
}

#[test]
fn script_string_inequality() {
    let bb = bb_with_output("hello");
    let eval = Evaluator::Script {
        expression: r#"provider_output != "world""#.into(),
    };
    assert_eq!(eval.evaluate(&bb).unwrap(), true);
}

#[test]
fn script_numeric_less_than() {
    let mut bb = Blackboard::default();
    bb.reflection_round = 1;
    let eval = Evaluator::Script {
        expression: "reflection_round < 2".into(),
    };
    assert_eq!(eval.evaluate(&bb).unwrap(), true);
}

#[test]
fn script_numeric_greater_than_or_equal() {
    let mut bb = Blackboard::default();
    bb.confidence_accumulator = 0.95;
    let eval = Evaluator::Script {
        expression: "confidence_accumulator >= 0.9".into(),
    };
    assert_eq!(eval.evaluate(&bb).unwrap(), true);
}

#[test]
fn script_boolean_literal_true() {
    let mut bb = Blackboard::default();
    bb.set_bool("flag", true);
    let eval = Evaluator::Script {
        expression: "flag == true".into(),
    };
    assert_eq!(eval.evaluate(&bb).unwrap(), true);
}

#[test]
fn script_boolean_literal_false() {
    let mut bb = Blackboard::default();
    bb.set_bool("flag", false);
    let eval = Evaluator::Script {
        expression: "flag == false".into(),
    };
    assert_eq!(eval.evaluate(&bb).unwrap(), true);
}

// ── Script: is_dangerous ────────────────────────────────────────────────────

#[test]
fn script_is_dangerous_detects_delete() {
    let bb = bb_with_output("delete from users");
    let eval = Evaluator::Script {
        expression: "is_dangerous(provider_output)".into(),
    };
    assert_eq!(eval.evaluate(&bb).unwrap(), true);
}

#[test]
fn script_is_dangerous_detects_drop() {
    let bb = bb_with_output("drop table users");
    let eval = Evaluator::Script {
        expression: "is_dangerous(provider_output)".into(),
    };
    assert_eq!(eval.evaluate(&bb).unwrap(), true);
}

#[test]
fn script_is_dangerous_safe() {
    let bb = bb_with_output("select * from users");
    let eval = Evaluator::Script {
        expression: "is_dangerous(provider_output)".into(),
    };
    assert_eq!(eval.evaluate(&bb).unwrap(), false);
}

// ── Script: contains ────────────────────────────────────────────────────────

#[test]
fn script_contains_found() {
    let bb = bb_with_output("error in line 42");
    let eval = Evaluator::Script {
        expression: "provider_output.contains(\"error\")".into(),
    };
    assert_eq!(eval.evaluate(&bb).unwrap(), true);
}

#[test]
fn script_contains_not_found() {
    let bb = bb_with_output("all good");
    let eval = Evaluator::Script {
        expression: "provider_output.contains(\"error\")".into(),
    };
    assert_eq!(eval.evaluate(&bb).unwrap(), false);
}

// ── Script: compound with && / || ───────────────────────────────────────────

#[test]
fn script_and_both_true() {
    let bb = bb_with_output("error");
    let eval = Evaluator::Script {
        expression: r#"provider_output == "error" && provider_output.contains("err")"#.into(),
    };
    assert_eq!(eval.evaluate(&bb).unwrap(), true);
}

#[test]
fn script_and_short_circuit_left_false() {
    let bb = bb_with_output("ok");
    let eval = Evaluator::Script {
        expression: r#"provider_output == "error" && provider_output.contains("xyz")"#.into(),
    };
    assert_eq!(eval.evaluate(&bb).unwrap(), false);
}

#[test]
fn script_or_left_true() {
    let bb = bb_with_output("error");
    let eval = Evaluator::Script {
        expression: r#"provider_output == "error" || provider_output.contains("xyz")"#.into(),
    };
    assert_eq!(eval.evaluate(&bb).unwrap(), true);
}

#[test]
fn script_or_short_circuit_right_true() {
    let bb = bb_with_output("ok");
    let eval = Evaluator::Script {
        expression: r#"provider_output == "error" || provider_output == "ok""#.into(),
    };
    assert_eq!(eval.evaluate(&bb).unwrap(), true);
}

#[test]
fn script_nested_expression() {
    let mut bb = Blackboard::default();
    bb.provider_output = "error in system".into();
    bb.reflection_round = 1;
    let eval = Evaluator::Script {
        expression: "reflection_round < 2 && provider_output.contains(\"error\")".into(),
    };
    assert_eq!(eval.evaluate(&bb).unwrap(), true);
}

#[test]
fn script_dot_notation_path() {
    let mut bb = Blackboard::default();
    bb.last_tool_call = Some(decision_dsl::ext::blackboard::ToolCallRecord {
        name: "search".into(),
        input: "q".into(),
        output: "r".into(),
    });
    let eval = Evaluator::Script {
        expression: r#"last_tool_call.name == "search""#.into(),
    };
    assert_eq!(eval.evaluate(&bb).unwrap(), true);
}

// ── Or / And / Not ──────────────────────────────────────────────────────────

#[test]
fn or_short_circuits_on_first_true() {
    let bb = Blackboard::default();
    let eval = Evaluator::Or {
        conditions: vec![
            Evaluator::OutputContains {
                pattern: "x".into(),
                case_sensitive: true,
            },
            Evaluator::OutputContains {
                pattern: "y".into(),
                case_sensitive: true,
            },
        ],
    };
    // Both false
    assert_eq!(eval.evaluate(&bb).unwrap(), false);
}

#[test]
fn or_returns_true_on_first_match() {
    let bb = bb_with_output("abc");
    let eval = Evaluator::Or {
        conditions: vec![
            Evaluator::OutputContains {
                pattern: "a".into(),
                case_sensitive: true,
            },
            Evaluator::OutputContains {
                pattern: "z".into(),
                case_sensitive: true,
            },
        ],
    };
    assert_eq!(eval.evaluate(&bb).unwrap(), true);
}

#[test]
fn and_short_circuits_on_first_false() {
    let bb = bb_with_output("abc");
    let eval = Evaluator::And {
        conditions: vec![
            Evaluator::OutputContains {
                pattern: "z".into(),
                case_sensitive: true,
            },
            Evaluator::OutputContains {
                pattern: "a".into(),
                case_sensitive: true,
            },
        ],
    };
    assert_eq!(eval.evaluate(&bb).unwrap(), false);
}

#[test]
fn and_returns_true_when_all_match() {
    let bb = bb_with_output("abc");
    let eval = Evaluator::And {
        conditions: vec![
            Evaluator::OutputContains {
                pattern: "a".into(),
                case_sensitive: true,
            },
            Evaluator::OutputContains {
                pattern: "b".into(),
                case_sensitive: true,
            },
        ],
    };
    assert_eq!(eval.evaluate(&bb).unwrap(), true);
}

#[test]
fn not_inverts_result() {
    let bb = bb_with_output("abc");
    let eval = Evaluator::Not {
        condition: Box::new(Evaluator::OutputContains {
            pattern: "z".into(),
            case_sensitive: true,
        }),
    };
    assert_eq!(eval.evaluate(&bb).unwrap(), true);
}

// ── Custom (placeholder, no registry) ───────────────────────────────────────

#[test]
fn custom_evaluator_fails() {
    let bb = Blackboard::default();
    let eval = Evaluator::Custom {
        name: "unknown".into(),
        params: std::collections::HashMap::new(),
    };
    let result = eval.evaluate(&bb);
    assert!(result.is_err());
}

// ── EvaluatorRegistry ───────────────────────────────────────────────────────

#[test]
fn registry_with_builtins() {
    let _reg = decision_dsl::ast::EvaluatorRegistry::with_builtins();
    // Just ensure it compiles and doesn't panic
}

#[test]
fn registry_create_builtin() {
    let reg = decision_dsl::ast::EvaluatorRegistry::with_builtins();
    let eval = reg.create("OutputContains", &[("pattern".into(), BlackboardValue::String("x".into()))]);
    assert!(eval.is_some());
}
