use decision_dsl::ast::template::{render_prompt_template, BlackboardExt};
use decision_dsl::ext::blackboard::{Blackboard, BlackboardValue, FileChangeRecord, ToolCallRecord};
use decision_dsl::ext::error::RuntimeError;

fn simple_bb() -> Blackboard {
    let mut bb = Blackboard::default();
    bb.provider_output = "hello world".into();
    bb.task_description = "test task".into();
    bb.reflection_round = 2;
    bb.confidence_accumulator = 0.95;
    bb.agent_id = "agent-1".into();
    bb
}

// ── Basic variable interpolation ────────────────────────────────────────────

#[test]
fn template_simple_variable() {
    let bb = simple_bb();
    let result = render_prompt_template("Output: {{ provider_output }}", &bb.to_template_context()).unwrap();
    assert_eq!(result, "Output: hello world");
}

#[test]
fn template_multiple_variables() {
    let bb = simple_bb();
    let result = render_prompt_template(
        "Task: {{ task_description }}, Round: {{ reflection_round }}",
        &bb.to_template_context(),
    ).unwrap();
    assert_eq!(result, "Task: test task, Round: 2");
}

// ── Standard filters ────────────────────────────────────────────────────────

#[test]
fn template_upper_filter() {
    let bb = simple_bb();
    let result = render_prompt_template("{{ provider_output | upper }}", &bb.to_template_context()).unwrap();
    assert_eq!(result, "HELLO WORLD");
}

#[test]
fn template_lower_filter() {
    let bb = simple_bb();
    let result = render_prompt_template("{{ provider_output | lower }}", &bb.to_template_context()).unwrap();
    assert_eq!(result, "hello world");
}

#[test]
fn template_length_filter() {
    let bb = simple_bb();
    let result = render_prompt_template("{{ provider_output | length }}", &bb.to_template_context()).unwrap();
    assert_eq!(result, "11");
}

#[test]
fn template_default_filter() {
    let bb = simple_bb();
    let result = render_prompt_template(
        "{{ missing_var | default('fallback') }}",
        &bb.to_template_context(),
    ).unwrap();
    assert_eq!(result, "fallback");
}

#[test]
fn template_join_filter() {
    let mut bb = simple_bb();
    bb.set("tags", BlackboardValue::List(vec![
        BlackboardValue::String("a".into()),
        BlackboardValue::String("b".into()),
    ]));
    let result = render_prompt_template("{{ tags | join(', ') }}", &bb.to_template_context()).unwrap();
    assert_eq!(result, "a, b");
}

// ── Custom filters ──────────────────────────────────────────────────────────

#[test]
fn template_slugify_filter() {
    let bb = simple_bb();
    let result = render_prompt_template(
        "{{ 'Hello World!' | slugify }}",
        &bb.to_template_context(),
    ).unwrap();
    assert_eq!(result, "hello-world");
}

#[test]
fn template_truncate_filter() {
    let bb = simple_bb();
    let result = render_prompt_template(
        "{{ provider_output | truncate(5) }}",
        &bb.to_template_context(),
    ).unwrap();
    assert_eq!(result, "hello...");
}

// ── Nested object access ────────────────────────────────────────────────────

#[test]
fn template_nested_object() {
    let mut bb = simple_bb();
    bb.last_tool_call = Some(ToolCallRecord {
        name: "search".into(),
        input: "query".into(),
        output: "result".into(),
    });
    let result = render_prompt_template(
        "Tool: {{ last_tool_call.name }}",
        &bb.to_template_context(),
    ).unwrap();
    assert_eq!(result, "Tool: search");
}

#[test]
fn template_list_access() {
    let mut bb = simple_bb();
    bb.file_changes = vec![
        FileChangeRecord {
            path: "src/main.rs".into(),
            change_type: "modified".into(),
        },
    ];
    let result = render_prompt_template(
        "File: {{ file_changes[0].path }}",
        &bb.to_template_context(),
    ).unwrap();
    assert_eq!(result, "File: src/main.rs");
}

// ── Control flow ────────────────────────────────────────────────────────────

#[test]
fn template_if_statement() {
    let bb = simple_bb();
    let template = "{% if reflection_round < 3 %}low{% else %}high{% endif %}";
    let result = render_prompt_template(template, &bb.to_template_context()).unwrap();
    assert_eq!(result, "low");
}

#[test]
fn template_for_loop() {
    let mut bb = simple_bb();
    bb.file_changes = vec![
        FileChangeRecord {
            path: "a.rs".into(),
            change_type: "modified".into(),
        },
        FileChangeRecord {
            path: "b.rs".into(),
            change_type: "added".into(),
        },
    ];
    let template = "{% for f in file_changes %}{{ f.path }} {% endfor %}";
    let result = render_prompt_template(template, &bb.to_template_context()).unwrap();
    assert_eq!(result, "a.rs b.rs ");
}

// ── Scoped variables ────────────────────────────────────────────────────────

#[test]
fn template_scoped_variable() {
    let mut bb = simple_bb();
    bb.set("my_key", BlackboardValue::String("scoped value".into()));
    let result = render_prompt_template("{{ my_key }}", &bb.to_template_context()).unwrap();
    assert_eq!(result, "scoped value");
}

// ── Error handling ──────────────────────────────────────────────────────────

#[test]
fn template_missing_variable_without_default_fails() {
    let bb = simple_bb();
    let err = render_prompt_template("{{ missing_var }}", &bb.to_template_context()).unwrap_err();
    assert!(matches!(err, RuntimeError::FilterError(_)));
}

#[test]
fn template_invalid_syntax_fails() {
    let bb = simple_bb();
    let err = render_prompt_template("{{ unclosed", &bb.to_template_context()).unwrap_err();
    assert!(matches!(err, RuntimeError::FilterError(_)));
}

#[test]
fn template_unknown_filter_fails() {
    let bb = simple_bb();
    let err = render_prompt_template(
        "{{ provider_output | nonexistent_filter }}",
        &bb.to_template_context(),
    ).unwrap_err();
    assert!(matches!(err, RuntimeError::FilterError(_)));
}

// ── Float rendering ─────────────────────────────────────────────────────────

#[test]
fn template_float_variable() {
    let bb = simple_bb();
    let result = render_prompt_template("{{ confidence_accumulator }}", &bb.to_template_context()).unwrap();
    assert_eq!(result, "0.95");
}
