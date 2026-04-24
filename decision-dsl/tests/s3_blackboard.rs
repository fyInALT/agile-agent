use std::collections::HashMap;

use decision_dsl::ext::blackboard::{
    Blackboard, BlackboardValue, DecisionRecord, FileChangeRecord, ProjectRules, ToolCallRecord,
};
use decision_dsl::ext::command::{AgentCommand, DecisionCommand, HumanCommand};

// ── BlackboardValue ─────────────────────────────────────────────────────────

#[test]
fn blackboard_value_variants() {
    let _ = BlackboardValue::String("hello".into());
    let _ = BlackboardValue::Integer(42);
    let _ = BlackboardValue::Float(3.14);
    let _ = BlackboardValue::Boolean(true);
    let _ = BlackboardValue::List(vec![]);
    let _ = BlackboardValue::Map(HashMap::new());
}

#[test]
fn blackboard_value_equality() {
    assert_eq!(
        BlackboardValue::String("a".into()),
        BlackboardValue::String("a".into())
    );
    assert_ne!(
        BlackboardValue::String("a".into()),
        BlackboardValue::String("b".into())
    );
}

// ── Blackboard::default ─────────────────────────────────────────────────────

#[test]
fn blackboard_default_is_empty() {
    let bb = Blackboard::default();
    assert_eq!(bb.task_description, "");
    assert_eq!(bb.provider_output, "");
    assert_eq!(bb.reflection_round, 0);
    assert_eq!(bb.max_reflection_rounds, 0);
    assert_eq!(bb.confidence_accumulator, 0.0);
    assert_eq!(bb.agent_id, "");
    assert_eq!(bb.current_task_id, "");
    assert_eq!(bb.current_story_id, "");
    assert!(bb.last_tool_call.is_none());
    assert!(bb.file_changes.is_empty());
    assert!(bb.decision_history.is_empty());
    assert!(bb.commands.is_empty());
    assert!(bb.llm_responses.is_empty());
}

#[test]
fn blackboard_new() {
    let bb = Blackboard::new();
    assert_eq!(bb.task_description, "");
}

#[test]
fn blackboard_with_capacity() {
    let bb = Blackboard::with_capacity(16);
    assert_eq!(bb.task_description, "");
    // Just ensure it compiles and doesn't panic
}

// ── Scope push/pop ──────────────────────────────────────────────────────────

#[test]
fn scope_isolation() {
    let mut bb = Blackboard::default();
    bb.set("x", BlackboardValue::Integer(1));
    assert_eq!(bb.get("x"), Some(&BlackboardValue::Integer(1)));

    bb.push_scope();
    bb.set("x", BlackboardValue::Integer(2));
    assert_eq!(bb.get("x"), Some(&BlackboardValue::Integer(2)));

    bb.pop_scope();
    assert_eq!(bb.get("x"), Some(&BlackboardValue::Integer(1)));
}

#[test]
fn scope_inner_reads_outer() {
    let mut bb = Blackboard::default();
    bb.set("x", BlackboardValue::Integer(1));
    bb.push_scope();
    // Inner scope can read outer scope
    assert_eq!(bb.get("x"), Some(&BlackboardValue::Integer(1)));
}

#[test]
fn scope_pop_does_not_underflow() {
    let mut bb = Blackboard::default();
    bb.pop_scope(); // Should not panic even with only root scope
    bb.pop_scope();
}

#[test]
fn scope_new_variable_in_inner_scope() {
    let mut bb = Blackboard::default();
    bb.push_scope();
    bb.set("inner", BlackboardValue::String("val".into()));
    assert_eq!(bb.get("inner"), Some(&BlackboardValue::String("val".into())));
    bb.pop_scope();
    assert_eq!(bb.get("inner"), None);
}

// ── get_path built-in fields ────────────────────────────────────────────────

#[test]
fn get_path_task_description() {
    let mut bb = Blackboard::default();
    bb.task_description = "do it".into();
    assert_eq!(
        bb.get_path("task_description"),
        Some(BlackboardValue::String("do it".into()))
    );
}

#[test]
fn get_path_reflection_round() {
    let mut bb = Blackboard::default();
    bb.reflection_round = 3;
    assert_eq!(
        bb.get_path("reflection_round"),
        Some(BlackboardValue::Integer(3))
    );
}

#[test]
fn get_path_confidence_accumulator() {
    let mut bb = Blackboard::default();
    bb.confidence_accumulator = 0.95;
    assert_eq!(
        bb.get_path("confidence_accumulator"),
        Some(BlackboardValue::Float(0.95))
    );
}

#[test]
fn get_path_last_tool_call() {
    let mut bb = Blackboard::default();
    bb.last_tool_call = Some(ToolCallRecord {
        name: "search".into(),
        input: "q".into(),
        output: "r".into(),
    });
    let mut expected = HashMap::new();
    expected.insert("name".into(), BlackboardValue::String("search".into()));
    expected.insert("input".into(), BlackboardValue::String("q".into()));
    expected.insert("output".into(), BlackboardValue::String("r".into()));
    assert_eq!(
        bb.get_path("last_tool_call"),
        Some(BlackboardValue::Map(expected.clone()))
    );
    assert_eq!(
        bb.get_path("last_tool_call.name"),
        Some(BlackboardValue::String("search".into()))
    );
}

#[test]
fn get_path_file_changes() {
    let mut bb = Blackboard::default();
    bb.file_changes.push(FileChangeRecord {
        path: "/a.rs".into(),
        change_type: "modified".into(),
    });
    assert_eq!(
        bb.get_path("file_changes.0.path"),
        Some(BlackboardValue::String("/a.rs".into()))
    );
    assert_eq!(
        bb.get_path("file_changes.0.change_type"),
        Some(BlackboardValue::String("modified".into()))
    );
}

#[test]
fn get_path_llm_responses() {
    let mut bb = Blackboard::default();
    bb.llm_responses.insert("key1".into(), "val1".into());
    assert_eq!(
        bb.get_path("llm_responses.key1"),
        Some(BlackboardValue::String("val1".into()))
    );
}

#[test]
fn get_path_scoped_variable() {
    let mut bb = Blackboard::default();
    bb.set("my_var", BlackboardValue::Integer(42));
    assert_eq!(
        bb.get_path("my_var"),
        Some(BlackboardValue::Integer(42))
    );
}

#[test]
fn get_path_scoped_variable_dot_notation() {
    let mut bb = Blackboard::default();
    let mut inner = HashMap::new();
    inner.insert("a".into(), BlackboardValue::Integer(1));
    bb.set("map", BlackboardValue::Map(inner));
    assert_eq!(
        bb.get_path("map.a"),
        Some(BlackboardValue::Integer(1))
    );
}

#[test]
fn get_path_scoped_list_index() {
    let mut bb = Blackboard::default();
    bb.set("list", BlackboardValue::List(vec![
        BlackboardValue::String("x".into()),
        BlackboardValue::String("y".into()),
    ]));
    assert_eq!(
        bb.get_path("list.1"),
        Some(BlackboardValue::String("y".into()))
    );
}

#[test]
fn get_path_unknown_returns_none() {
    let bb = Blackboard::default();
    assert_eq!(bb.get_path("nonexistent"), None);
    assert_eq!(bb.get_path("nonexistent.nested"), None);
}

// ── Typed getters ───────────────────────────────────────────────────────────

#[test]
fn get_string() {
    let mut bb = Blackboard::default();
    bb.task_description = "task".into();
    assert_eq!(bb.get_string("task_description"), Some("task".into()));
}

#[test]
fn get_bool() {
    let mut bb = Blackboard::default();
    bb.set("flag", BlackboardValue::Boolean(true));
    assert_eq!(bb.get_bool("flag"), Some(true));
}

#[test]
fn get_u8() {
    let mut bb = Blackboard::default();
    bb.set("n", BlackboardValue::Integer(255));
    assert_eq!(bb.get_u8("n"), Some(255));
}

#[test]
fn get_u8_out_of_range() {
    let mut bb = Blackboard::default();
    bb.set("n", BlackboardValue::Integer(300));
    assert_eq!(bb.get_u8("n"), None);
}

#[test]
fn get_f64_from_float() {
    let mut bb = Blackboard::default();
    bb.set("f", BlackboardValue::Float(2.5));
    assert_eq!(bb.get_f64("f"), Some(2.5));
}

#[test]
fn get_f64_from_integer() {
    let mut bb = Blackboard::default();
    bb.set("i", BlackboardValue::Integer(7));
    assert_eq!(bb.get_f64("i"), Some(7.0));
}

// ── Typed setters ───────────────────────────────────────────────────────────

#[test]
fn set_string() {
    let mut bb = Blackboard::default();
    bb.set_string("key", "val".into());
    assert_eq!(bb.get("key"), Some(&BlackboardValue::String("val".into())));
}

#[test]
fn set_u8() {
    let mut bb = Blackboard::default();
    bb.set_u8("key", 42);
    assert_eq!(bb.get("key"), Some(&BlackboardValue::Integer(42)));
}

#[test]
fn set_f64() {
    let mut bb = Blackboard::default();
    bb.set_f64("key", 3.14);
    assert_eq!(bb.get("key"), Some(&BlackboardValue::Float(3.14)));
}

#[test]
fn set_bool() {
    let mut bb = Blackboard::default();
    bb.set_bool("key", true);
    assert_eq!(bb.get("key"), Some(&BlackboardValue::Boolean(true)));
}

// ── Commands ────────────────────────────────────────────────────────────────

#[test]
fn push_and_drain_commands() {
    let mut bb = Blackboard::default();
    let cmd1 = DecisionCommand::Agent(AgentCommand::WakeUp);
    let cmd2 = DecisionCommand::Human(HumanCommand::Escalate { reason: "r".into(), context: None });
    bb.push_command(cmd1.clone());
    bb.push_command(cmd2.clone());
    let drained = bb.drain_commands();
    assert_eq!(drained.len(), 2);
    assert_eq!(drained[0], cmd1);
    assert_eq!(drained[1], cmd2);
    assert!(bb.commands.is_empty());
}

#[test]
fn drain_empty_commands() {
    let mut bb = Blackboard::default();
    let drained = bb.drain_commands();
    assert!(drained.is_empty());
}

// ── LLM responses ───────────────────────────────────────────────────────────

#[test]
fn store_llm_response() {
    let mut bb = Blackboard::default();
    bb.store_llm_response("model_a", "hello");
    assert_eq!(bb.llm_responses.get("model_a"), Some(&"hello".into()));
}

// ── DecisionRecord ──────────────────────────────────────────────────────────

#[test]
fn decision_record_creation() {
    let _ = DecisionRecord {
        situation: "s".into(),
        command: DecisionCommand::Agent(AgentCommand::WakeUp),
        timestamp: "2024".into(),
    };
}

// ── ProjectRules ────────────────────────────────────────────────────────────

#[test]
fn project_rules_default() {
    let pr = ProjectRules::default();
    assert!(pr.rules.is_empty());
}
