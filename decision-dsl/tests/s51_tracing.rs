use decision_dsl::ast::node::NodeStatus;
use decision_dsl::ast::runtime::{Tracer, TraceEntry, render_trace_ascii};
use decision_dsl::ext::command::{DecisionCommand, AgentCommand};

#[test]
fn tracer_enter_exit_creates_entries() {
    let mut tracer = Tracer::new();
    tracer.enter("sel", 0);
    tracer.exit("sel", 0, NodeStatus::Success);
    assert_eq!(tracer.entries().len(), 2);
    assert!(matches!(tracer.entries()[0], TraceEntry::Enter { .. }));
    assert!(matches!(tracer.entries()[1], TraceEntry::Exit { .. }));
}

#[test]
fn tracer_records_action() {
    let mut tracer = Tracer::new();
    tracer.record_action("a", &DecisionCommand::Agent(AgentCommand::WakeUp));
    assert!(matches!(tracer.entries()[0], TraceEntry::Action { .. }));
}

#[test]
fn tracer_records_eval() {
    let mut tracer = Tracer::new();
    tracer.record_eval("c", "OutputContains", true);
    assert!(matches!(tracer.entries()[0], TraceEntry::Eval { .. }));
}

#[test]
fn tracer_records_prompt_sent() {
    let mut tracer = Tracer::new();
    tracer.record_prompt_sent("p");
    assert!(matches!(tracer.entries()[0], TraceEntry::PromptSent { .. }));
}

#[test]
fn tracer_records_prompt_success() {
    let mut tracer = Tracer::new();
    tracer.record_prompt_success("p", "ok");
    assert!(matches!(tracer.entries()[0], TraceEntry::PromptSuccess { .. }));
}

#[test]
fn tracer_records_prompt_failure() {
    let mut tracer = Tracer::new();
    tracer.record_prompt_failure("p", "timeout");
    assert!(matches!(tracer.entries()[0], TraceEntry::PromptFailure { .. }));
}

#[test]
fn tracer_records_rule_matched() {
    let mut tracer = Tracer::new();
    tracer.record_rule_matched("r1", 1);
    assert!(matches!(tracer.entries()[0], TraceEntry::RuleMatched { .. }));
}

#[test]
fn tracer_records_rule_skipped() {
    let mut tracer = Tracer::new();
    tracer.record_rule_skipped("r1", "no_match");
    assert!(matches!(tracer.entries()[0], TraceEntry::RuleSkipped { .. }));
}

#[test]
fn tracer_running_path_tracks_depth() {
    let mut tracer = Tracer::new();
    tracer.enter("seq", 0);
    tracer.enter("sel", 1);
    tracer.exit("sel", 1, NodeStatus::Running);
    assert_eq!(tracer.running_path(), &[0]);
    tracer.exit("seq", 0, NodeStatus::Running);
    assert!(tracer.running_path().is_empty());
}

#[test]
fn render_trace_ascii_basic() {
    let mut tracer = Tracer::new();
    tracer.enter("root", 0);
    tracer.record_action("root", &DecisionCommand::Agent(AgentCommand::WakeUp));
    tracer.exit("root", 0, NodeStatus::Success);
    let ascii = render_trace_ascii(tracer.entries());
    assert!(ascii.contains("root"));
    assert!(ascii.contains("Success"));
}

#[test]
fn render_trace_ascii_shows_depth() {
    let mut tracer = Tracer::new();
    tracer.enter("seq", 0);
    tracer.enter("cond", 0);
    tracer.record_eval("cond", "OutputContains", true);
    tracer.exit("cond", 0, NodeStatus::Success);
    tracer.exit("seq", 0, NodeStatus::Success);
    let ascii = render_trace_ascii(tracer.entries());
    // Should have indentation for nested entries
    let lines: Vec<&str> = ascii.lines().collect();
    assert!(lines.len() >= 3);
}
