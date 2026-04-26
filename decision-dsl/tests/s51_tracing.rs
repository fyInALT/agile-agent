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


// ═════════════════════════════════════════════════════════════════════════════
// Coverage: Tracer field verification
// ═════════════════════════════════════════════════════════════════════════════

#[test]
fn tracer_enter_fields_correct() {
    let mut tracer = Tracer::new();
    tracer.enter("my_node", 3);
    if let TraceEntry::Enter { name, child_index, depth } = &tracer.entries()[0] {
        assert_eq!(name, "my_node");
        assert_eq!(*child_index, 3);
        assert_eq!(*depth, 0);
    } else {
        panic!("expected Enter");
    }
}

#[test]
fn tracer_exit_fields_correct() {
    let mut tracer = Tracer::new();
    tracer.enter("n", 0);
    tracer.exit("n", 0, NodeStatus::Failure);
    if let TraceEntry::Exit { name, status, .. } = &tracer.entries()[1] {
        assert_eq!(name, "n");
        assert_eq!(*status, NodeStatus::Failure);
    } else {
        panic!("expected Exit");
    }
}

#[test]
fn tracer_action_fields_correct() {
    let mut tracer = Tracer::new();
    tracer.record_action("act", &DecisionCommand::Agent(AgentCommand::WakeUp));
    if let TraceEntry::Action { node_name, command } = &tracer.entries()[0] {
        assert_eq!(node_name, "act");
        assert!(command.contains("WakeUp"));
    } else {
        panic!("expected Action");
    }
}

#[test]
fn tracer_eval_fields_correct() {
    let mut tracer = Tracer::new();
    tracer.record_eval("cond", "OutputContains", false);
    if let TraceEntry::Eval { node_name, evaluator, result } = &tracer.entries()[0] {
        assert_eq!(node_name, "cond");
        assert_eq!(evaluator, "OutputContains");
        assert_eq!(*result, false);
    } else {
        panic!("expected Eval");
    }
}

#[test]
fn tracer_prompt_sent_fields_correct() {
    let mut tracer = Tracer::new();
    tracer.record_prompt_sent("p1");
    if let TraceEntry::PromptSent { node_name } = &tracer.entries()[0] {
        assert_eq!(node_name, "p1");
    } else {
        panic!("expected PromptSent");
    }
}

#[test]
fn tracer_prompt_success_fields_correct() {
    let mut tracer = Tracer::new();
    tracer.record_prompt_success("p2", "yes");
    if let TraceEntry::PromptSuccess { node_name, response } = &tracer.entries()[0] {
        assert_eq!(node_name, "p2");
        assert_eq!(response, "yes");
    } else {
        panic!("expected PromptSuccess");
    }
}

#[test]
fn tracer_prompt_failure_fields_correct() {
    let mut tracer = Tracer::new();
    tracer.record_prompt_failure("p3", "timeout");
    if let TraceEntry::PromptFailure { node_name, error } = &tracer.entries()[0] {
        assert_eq!(node_name, "p3");
        assert_eq!(error, "timeout");
    } else {
        panic!("expected PromptFailure");
    }
}

#[test]
fn tracer_rule_matched_fields_correct() {
    let mut tracer = Tracer::new();
    tracer.record_rule_matched("rule_a", 42);
    if let TraceEntry::RuleMatched { rule_name, priority } = &tracer.entries()[0] {
        assert_eq!(rule_name, "rule_a");
        assert_eq!(*priority, 42);
    } else {
        panic!("expected RuleMatched");
    }
}

#[test]
fn tracer_rule_skipped_fields_correct() {
    let mut tracer = Tracer::new();
    tracer.record_rule_skipped("rule_b", "lower priority");
    if let TraceEntry::RuleSkipped { rule_name, reason } = &tracer.entries()[0] {
        assert_eq!(rule_name, "rule_b");
        assert_eq!(reason, "lower priority");
    } else {
        panic!("expected RuleSkipped");
    }
}

#[test]
fn tracer_enter_subtree_fields_correct() {
    let mut tracer = Tracer::new();
    tracer.enter_subtree("sub", "inner_tree");
    if let TraceEntry::EnterSubTree { name, ref_name } = &tracer.entries()[0] {
        assert_eq!(name, "sub");
        assert_eq!(ref_name, "inner_tree");
    } else {
        panic!("expected EnterSubTree");
    }
}

#[test]
fn tracer_exit_subtree_fields_correct() {
    let mut tracer = Tracer::new();
    tracer.exit_subtree("sub", "inner_tree", NodeStatus::Success);
    if let TraceEntry::ExitSubTree { name, ref_name, status } = &tracer.entries()[0] {
        assert_eq!(name, "sub");
        assert_eq!(ref_name, "inner_tree");
        assert_eq!(*status, NodeStatus::Success);
    } else {
        panic!("expected ExitSubTree");
    }
}

// ═════════════════════════════════════════════════════════════════════════════
// Coverage: render_trace_ascii all branches
// ═════════════════════════════════════════════════════════════════════════════

#[test]
fn render_trace_ascii_all_entry_types() {
    let mut tracer = Tracer::new();
    tracer.enter("root", 0);
    tracer.record_eval("cond", "Script", true);
    tracer.record_action("act", &DecisionCommand::Agent(AgentCommand::WakeUp));
    tracer.record_prompt_sent("p");
    tracer.record_prompt_success("p", "ok");
    tracer.record_prompt_failure("p2", "err");
    tracer.enter_subtree("sub", "inner");
    tracer.exit_subtree("sub", "inner", NodeStatus::Success);
    tracer.record_rule_matched("r", 1);
    tracer.record_rule_skipped("r2", "low");
    tracer.exit("root", 0, NodeStatus::Success);

    let ascii = render_trace_ascii(tracer.entries());
    assert!(ascii.contains("Enter: root"));
    assert!(ascii.contains("Eval: cond"));
    assert!(ascii.contains("Action: act"));
    assert!(ascii.contains("Prompt sent: p"));
    assert!(ascii.contains("Prompt success: p"));
    assert!(ascii.contains("Prompt failure: p2"));
    assert!(ascii.contains("--> SubTree: sub (inner)"));
    assert!(ascii.contains("<-- SubTree: sub (inner)"));
    assert!(ascii.contains("Rule matched: r (priority 1)"));
    assert!(ascii.contains("Rule skipped: r2 (low)"));
    assert!(ascii.contains("Exit: root -> Success"));
}
