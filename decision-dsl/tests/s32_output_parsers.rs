use std::collections::HashMap;

use decision_dsl::ast::{
    FieldType, OutputParser, OutputParserRegistry, StructuredField,
};
use decision_dsl::ext::blackboard::BlackboardValue;
use decision_dsl::ext::command::{AgentCommand, DecisionCommand};

// ── Enum Parser ─────────────────────────────────────────────────────────────

#[test]
fn enum_parser_case_sensitive_match() {
    let parser = OutputParser::Enum {
        values: vec!["Approve".into(), "Reject".into()],
        case_sensitive: true,
    };
    let result = parser.parse("Approve").unwrap();
    assert_eq!(result.get("value"), Some(&BlackboardValue::String("Approve".into())));
}

#[test]
fn enum_parser_case_sensitive_no_match() {
    let parser = OutputParser::Enum {
        values: vec!["Approve".into(), "Reject".into()],
        case_sensitive: true,
    };
    let err = parser.parse("approve").unwrap_err();
    assert!(err.to_string().contains("did not match"));
}

#[test]
fn enum_parser_case_insensitive_match() {
    let parser = OutputParser::Enum {
        values: vec!["Approve".into(), "Reject".into()],
        case_sensitive: false,
    };
    let result = parser.parse("approve").unwrap();
    assert_eq!(result.get("value"), Some(&BlackboardValue::String("approve".into())));
}

#[test]
fn enum_parser_trims_whitespace() {
    let parser = OutputParser::Enum {
        values: vec!["Approve".into()],
        case_sensitive: true,
    };
    let result = parser.parse("  Approve  \n").unwrap();
    assert_eq!(result.get("value"), Some(&BlackboardValue::String("Approve".into())));
}

// ── Structured Parser ───────────────────────────────────────────────────────

#[test]
fn structured_parser_extracts_fields() {
    let parser = OutputParser::Structured {
        pattern: r"Confidence: (?<confidence>\d+), Action: (?<action>\w+)".into(),
        fields: vec![
            StructuredField {
                name: "confidence".into(),
                group: 1,
                field_type: FieldType::Integer,
            },
            StructuredField {
                name: "action".into(),
                group: 2,
                field_type: FieldType::String,
            },
        ],
    };
    let result = parser.parse("Confidence: 95, Action: Approve").unwrap();
    assert_eq!(result.get("confidence"), Some(&BlackboardValue::Integer(95)));
    assert_eq!(result.get("action"), Some(&BlackboardValue::String("Approve".into())));
}

#[test]
fn structured_parser_float_field() {
    let parser = OutputParser::Structured {
        pattern: r"Score: (?<score>[\d.]+)".into(),
        fields: vec![
            StructuredField {
                name: "score".into(),
                group: 1,
                field_type: FieldType::Float,
            },
        ],
    };
    let result = parser.parse("Score: 0.95").unwrap();
    assert_eq!(result.get("score"), Some(&BlackboardValue::Float(0.95)));
}

#[test]
fn structured_parser_boolean_field() {
    let parser = OutputParser::Structured {
        pattern: r"Safe: (?<safe>true|false)".into(),
        fields: vec![
            StructuredField {
                name: "safe".into(),
                group: 1,
                field_type: FieldType::Boolean,
            },
        ],
    };
    let result = parser.parse("Safe: true").unwrap();
    assert_eq!(result.get("safe"), Some(&BlackboardValue::Boolean(true)));
}

#[test]
fn structured_parser_missing_group_fails() {
    let parser = OutputParser::Structured {
        pattern: r"Action: (?<action>\w+)".into(),
        fields: vec![
            StructuredField {
                name: "confidence".into(),
                group: 2,
                field_type: FieldType::Integer,
            },
        ],
    };
    let err = parser.parse("Action: Approve").unwrap_err();
    assert!(err.to_string().contains("capture group"));
}

#[test]
fn structured_parser_no_match_fails() {
    let parser = OutputParser::Structured {
        pattern: r"Action: (?<action>\w+)".into(),
        fields: vec![
            StructuredField {
                name: "action".into(),
                group: 1,
                field_type: FieldType::String,
            },
        ],
    };
    let err = parser.parse("No action here").unwrap_err();
    assert!(err.to_string().contains("did not match"));
}

#[test]
fn structured_parser_invalid_integer_fails() {
    let parser = OutputParser::Structured {
        pattern: r"Num: (?<num>\w+)".into(),
        fields: vec![
            StructuredField {
                name: "num".into(),
                group: 1,
                field_type: FieldType::Integer,
            },
        ],
    };
    let err = parser.parse("Num: abc").unwrap_err();
    assert!(err.to_string().contains("parse integer"));
}

// ── Json Parser ─────────────────────────────────────────────────────────────

#[test]
fn json_parser_object() {
    let parser = OutputParser::Json { schema: None };
    let result = parser.parse(r#"{"status": "ok", "count": 42}"#).unwrap();
    let mut expected = HashMap::new();
    expected.insert("status".into(), BlackboardValue::String("ok".into()));
    expected.insert("count".into(), BlackboardValue::Integer(42));
    assert_eq!(result, expected);
}

#[test]
fn json_parser_nested_object() {
    let parser = OutputParser::Json { schema: None };
    let result = parser.parse(r#"{"user": {"name": "Alice", "age": 30}}"#).unwrap();
    let user = result.get("user").unwrap();
    if let BlackboardValue::Map(m) = user {
        assert_eq!(m.get("name"), Some(&BlackboardValue::String("Alice".into())));
        assert_eq!(m.get("age"), Some(&BlackboardValue::Integer(30)));
    } else {
        panic!("expected Map");
    }
}

#[test]
fn json_parser_array() {
    let parser = OutputParser::Json { schema: None };
    let result = parser.parse(r#"{"items": ["a", "b", "c"]}"#).unwrap();
    let items = result.get("items").unwrap();
    if let BlackboardValue::List(l) = items {
        assert_eq!(l.len(), 3);
        assert_eq!(l[0], BlackboardValue::String("a".into()));
    } else {
        panic!("expected List");
    }
}

#[test]
fn json_parser_invalid_json_fails() {
    let parser = OutputParser::Json { schema: None };
    let err = parser.parse("not json").unwrap_err();
    assert!(err.to_string().contains("JSON"));
}

// ── Command Parser ──────────────────────────────────────────────────────────

#[test]
fn command_parser_maps_to_agent_command() {
    let mut mapping = HashMap::new();
    mapping.insert(
        "reflect".into(),
        DecisionCommand::Agent(AgentCommand::Reflect {
            prompt: "Think again".into(),
        }),
    );
    let parser = OutputParser::Command { mapping };
    let result = parser.parse("reflect").unwrap();
    assert!(result.contains_key("__command"));
}

#[test]
fn command_parser_unknown_key_fails() {
    let parser = OutputParser::Command {
        mapping: HashMap::new(),
    };
    let err = parser.parse("unknown").unwrap_err();
    assert!(err.to_string().contains("unknown command key"));
}

// ── OutputParserRegistry ────────────────────────────────────────────────────

#[test]
fn registry_with_builtins() {
    let reg = OutputParserRegistry::with_builtins();
    assert!(reg.create("Enum", &[]).is_ok());
    assert!(reg.create("Structured", &[]).is_ok());
    assert!(reg.create("Json", &[]).is_ok());
    assert!(reg.create("Command", &[]).is_ok());
}

#[test]
fn registry_create_unknown() {
    let reg = OutputParserRegistry::with_builtins();
    let err = reg.create("Unknown", &[]).unwrap_err();
    assert!(err.to_string().contains("unknown parser"));
}

#[test]
fn registry_register_custom() {
    let mut reg = OutputParserRegistry::with_builtins();
    reg.register("MyParser", |_params| Ok(OutputParser::Json { schema: None }));
    assert!(reg.create("MyParser", &[]).is_ok());
}

#[test]
fn registry_create_enum_with_params() {
    let reg = OutputParserRegistry::with_builtins();
    let params = vec![
        ("values".into(), BlackboardValue::List(vec![
            BlackboardValue::String("Yes".into()),
            BlackboardValue::String("No".into()),
        ])),
        ("caseSensitive".into(), BlackboardValue::Boolean(true)),
    ];
    let parser = reg.create("Enum", &params).unwrap();
    if let OutputParser::Enum { values, case_sensitive } = parser {
        assert_eq!(values, vec!["Yes".to_string(), "No".to_string()]);
        assert!(case_sensitive);
    } else {
        panic!("expected Enum parser");
    }
}

// ═════════════════════════════════════════════════════════════════════════════
// Edge case tests: Null handling
// ═════════════════════════════════════════════════════════════════════════════

#[test]
fn json_parser_null_to_blackboard_null() {
    let parser = OutputParser::Json { schema: None };
    let result = parser.parse("{\"value\": null}").unwrap();
    assert_eq!(result.get("value"), Some(&BlackboardValue::Null));
}

#[test]
fn json_parser_nested_null() {
    let parser = OutputParser::Json { schema: None };
    let result = parser.parse("{\"data\": {\"inner\": null}}").unwrap();
    let data = result.get("data").unwrap();
    match data {
        BlackboardValue::Map(m) => {
            assert_eq!(m.get("inner"), Some(&BlackboardValue::Null));
        }
        _ => panic!("expected Map"),
    }
}


// ═════════════════════════════════════════════════════════════════════════════
// Coverage: Structured parser type conversion failures
// ═════════════════════════════════════════════════════════════════════════════

#[test]
fn structured_parser_integer_parse_failure() {
    let parser = OutputParser::Structured {
        pattern: r"(\w+)".into(),
        fields: vec![StructuredField {
            name: "num".into(),
            group: 1,
            field_type: FieldType::Integer,
        }],
    };
    let err = parser.parse("abc").unwrap_err();
    assert!(err.to_string().contains("cannot parse integer"));
}

#[test]
fn structured_parser_float_parse_failure() {
    let parser = OutputParser::Structured {
        pattern: r"(\w+)".into(),
        fields: vec![StructuredField {
            name: "num".into(),
            group: 1,
            field_type: FieldType::Float,
        }],
    };
    let err = parser.parse("abc").unwrap_err();
    assert!(err.to_string().contains("cannot parse float"));
}

#[test]
fn structured_parser_boolean_parse_failure() {
    let parser = OutputParser::Structured {
        pattern: r"(\w+)".into(),
        fields: vec![StructuredField {
            name: "flag".into(),
            group: 1,
            field_type: FieldType::Boolean,
        }],
    };
    let err = parser.parse("maybe").unwrap_err();
    assert!(err.to_string().contains("cannot parse boolean"));
}

#[test]
fn structured_parser_missing_capture_group() {
    let parser = OutputParser::Structured {
        pattern: r"(\w+)".into(),
        fields: vec![StructuredField {
            name: "extra".into(),
            group: 5, // Group 5 doesn't exist
            field_type: FieldType::String,
        }],
    };
    let err = parser.parse("hello").unwrap_err();
    assert!(err.to_string().contains("capture group 5 not found"));
}

// ═════════════════════════════════════════════════════════════════════════════
// Coverage: Command parser edge cases
// ═════════════════════════════════════════════════════════════════════════════

#[test]
fn command_parser_trims_input() {
    let mut mapping = HashMap::new();
    mapping.insert(
        "wake".into(),
        DecisionCommand::Agent(AgentCommand::WakeUp),
    );
    let parser = OutputParser::Command { mapping };
    let result = parser.parse("  wake  \n").unwrap();
    assert!(result.contains_key("__command"));
}

// ═════════════════════════════════════════════════════════════════════════════
// Coverage: Custom parser
// ═════════════════════════════════════════════════════════════════════════════

#[test]
fn custom_parser_returns_error() {
    let parser = OutputParser::Custom {
        name: "MyParser".into(),
        params: HashMap::new(),
    };
    let result = parser.parse("anything");
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("MyParser"));
}

// ═════════════════════════════════════════════════════════════════════════════
// Coverage: OutputParserRegistry create Structured
// ═════════════════════════════════════════════════════════════════════════════

#[test]
fn registry_create_structured_with_params() {
    let reg = OutputParserRegistry::with_builtins();
    let params = vec![
        ("pattern".into(), BlackboardValue::String(r"(\d+)".into())),
        ("fields".into(), BlackboardValue::List(vec![
            BlackboardValue::Map({
                let mut m = HashMap::new();
                m.insert("name".into(), BlackboardValue::String("id".into()));
                m.insert("type".into(), BlackboardValue::String("Integer".into()));
                m.insert("group".into(), BlackboardValue::Integer(1));
                m
            }),
        ])),
    ];
    let parser = reg.create("Structured", &params).unwrap();
    if let OutputParser::Structured { pattern, fields } = parser {
        assert_eq!(pattern, r"(\d+)");
        assert_eq!(fields.len(), 1);
        assert_eq!(fields[0].name, "id");
        assert_eq!(fields[0].field_type, FieldType::Integer);
        assert_eq!(fields[0].group, 1);
    } else {
        panic!("expected Structured parser");
    }
}

// ═════════════════════════════════════════════════════════════════════════════
// Coverage: Json parser top-level non-object error
// ═════════════════════════════════════════════════════════════════════════════

#[test]
fn json_parser_top_level_array_fails() {
    let parser = OutputParser::Json { schema: None };
    let err = parser.parse("[1, 2, 3]").unwrap_err();
    assert!(err.to_string().contains("expects a top-level object"));
}


