use decision_dsl::ast::{
    ActionNode, Bundle, ConditionNode, CooldownNode, DslDocument, ForceHumanNode, InverterNode,
    Metadata, Node, NodeBehavior, NodeStatus, ParallelNode, ParallelPolicy, PromptNode,
    ReflectionGuardNode, RepeaterNode, SelectorNode, SequenceNode, SetMapping, SetVarNode,
    SubTreeNode, TreeKind, WhenNode,
};
use decision_dsl::ext::blackboard::BlackboardValue;
use decision_dsl::ext::command::{AgentCommand, DecisionCommand};

// ── Tree & Bundle ───────────────────────────────────────────────────────────

#[test]
fn tree_kind_variants() {
    let _ = TreeKind::BehaviorTree;
    let _ = TreeKind::SubTree;
}

#[test]
fn bundle_default_empty() {
    let b = Bundle::default();
    assert!(b.trees.is_empty());
    assert!(b.subtrees.is_empty());
}

// ── NodeStatus ──────────────────────────────────────────────────────────────

#[test]
fn node_status_variants() {
    let _ = NodeStatus::Success;
    let _ = NodeStatus::Failure;
    let _ = NodeStatus::Running;
}

// ── Node variants ───────────────────────────────────────────────────────────

#[test]
fn node_selector() {
    let n = Node::Selector(SelectorNode {
        name: "sel".into(),
        children: vec![],
        active_child: None,
    });
    assert_eq!(n.name(), "sel");
}

#[test]
fn node_sequence() {
    let n = Node::Sequence(SequenceNode {
        name: "seq".into(),
        children: vec![],
        active_child: None,
    });
    assert_eq!(n.name(), "seq");
}

#[test]
fn node_parallel() {
    let n = Node::Parallel(ParallelNode {
        name: "par".into(),
        policy: ParallelPolicy::AllSuccess,
        children: vec![],
    });
    assert_eq!(n.name(), "par");
}

#[test]
fn node_inverter() {
    let n = Node::Inverter(InverterNode {
        name: "inv".into(),
        child: Box::new(Node::Action(ActionNode {
            name: "act".into(),
            command: DecisionCommand::Agent(AgentCommand::WakeUp),
            when: None,
        })),
    });
    assert_eq!(n.name(), "inv");
}

#[test]
fn node_repeater() {
    let n = Node::Repeater(RepeaterNode {
        name: "rep".into(),
        max_attempts: 3,
        child: Box::new(Node::Action(ActionNode {
            name: "act".into(),
            command: DecisionCommand::Agent(AgentCommand::WakeUp),
            when: None,
        })),
        current: 0,
    });
    assert_eq!(n.name(), "rep");
}

#[test]
fn node_cooldown() {
    let n = Node::Cooldown(CooldownNode {
        name: "cool".into(),
        duration_ms: 1000,
        child: Box::new(Node::Action(ActionNode {
            name: "act".into(),
            command: DecisionCommand::Agent(AgentCommand::WakeUp),
            when: None,
        })),
        last_success: None,
    });
    assert_eq!(n.name(), "cool");
}

#[test]
fn node_reflection_guard() {
    let n = Node::ReflectionGuard(ReflectionGuardNode {
        name: "rg".into(),
        max_rounds: 2,
        child: Box::new(Node::Action(ActionNode {
            name: "act".into(),
            command: DecisionCommand::Agent(AgentCommand::WakeUp),
            when: None,
        })),
    });
    assert_eq!(n.name(), "rg");
}

#[test]
fn node_force_human() {
    let n = Node::ForceHuman(ForceHumanNode {
        name: "fh".into(),
        reason: "test".into(),
        child: Box::new(Node::Action(ActionNode {
            name: "act".into(),
            command: DecisionCommand::Agent(AgentCommand::WakeUp),
            when: None,
        })),
    });
    assert_eq!(n.name(), "fh");
}

#[test]
fn node_when() {
    use decision_dsl::ast::Evaluator;
    let n = Node::When(WhenNode {
        name: "when".into(),
        condition: Evaluator::VariableIs {
            key: "x".into(),
            expected: BlackboardValue::Integer(1),
        },
        action: Box::new(Node::Action(ActionNode {
            name: "act".into(),
            command: DecisionCommand::Agent(AgentCommand::WakeUp),
            when: None,
        })),
    });
    assert_eq!(n.name(), "when");
}

#[test]
fn node_condition() {
    use decision_dsl::ast::Evaluator;
    let n = Node::Condition(ConditionNode {
        name: "cond".into(),
        evaluator: Evaluator::VariableIs {
            key: "x".into(),
            expected: BlackboardValue::Boolean(true),
        },
    });
    assert_eq!(n.name(), "cond");
}

#[test]
fn node_action() {
    let n = Node::Action(ActionNode {
        name: "act".into(),
        command: DecisionCommand::Agent(AgentCommand::WakeUp),
        when: None,
    });
    assert_eq!(n.name(), "act");
}

#[test]
fn node_prompt() {
    use decision_dsl::ast::OutputParser;
    let n = Node::Prompt(PromptNode {
        name: "prompt".into(),
        model: None,
        template: "hello".into(),
        parser: OutputParser::Enum {
            values: vec!["A".into(), "B".into()],
            case_sensitive: true,
        },
        sets: vec![],
        timeout_ms: 30000,
        pending: false,
        sent_at: None,
    });
    assert_eq!(n.name(), "prompt");
}

#[test]
fn node_set_var() {
    let n = Node::SetVar(SetVarNode {
        name: "set".into(),
        key: "x".into(),
        value: BlackboardValue::Integer(42),
    });
    assert_eq!(n.name(), "set");
}

#[test]
fn node_subtree() {
    let n = Node::SubTree(SubTreeNode {
        name: "sub".into(),
        ref_name: "other".into(),
        resolved_root: None,
    });
    assert_eq!(n.name(), "sub");
}

// ── Node::children ──────────────────────────────────────────────────────────

#[test]
fn selector_children() {
    let child = Node::Action(ActionNode {
        name: "c1".into(),
        command: DecisionCommand::Agent(AgentCommand::WakeUp),
        when: None,
    });
    let n = Node::Selector(SelectorNode {
        name: "sel".into(),
        children: vec![child.clone()],
        active_child: None,
    });
    let kids = n.children();
    assert_eq!(kids.len(), 1);
    assert_eq!(kids[0].name(), "c1");
}

#[test]
fn inverter_children() {
    let child = Node::Action(ActionNode {
        name: "c1".into(),
        command: DecisionCommand::Agent(AgentCommand::WakeUp),
        when: None,
    });
    let n = Node::Inverter(InverterNode {
        name: "inv".into(),
        child: Box::new(child),
    });
    let kids = n.children();
    assert_eq!(kids.len(), 1);
    assert_eq!(kids[0].name(), "c1");
}

#[test]
fn action_children_empty() {
    let n = Node::Action(ActionNode {
        name: "act".into(),
        command: DecisionCommand::Agent(AgentCommand::WakeUp),
        when: None,
    });
    assert!(n.children().is_empty());
}

// ── Node::reset ─────────────────────────────────────────────────────────────

#[test]
fn selector_reset_clears_active_child() {
    let mut n = Node::Selector(SelectorNode {
        name: "sel".into(),
        children: vec![],
        active_child: Some(0),
    });
    n.reset();
    if let Node::Selector(ref s) = n {
        assert_eq!(s.active_child, None);
    }
}

#[test]
fn repeater_reset_clears_current() {
    let mut n = Node::Repeater(RepeaterNode {
        name: "rep".into(),
        max_attempts: 3,
        child: Box::new(Node::Action(ActionNode {
            name: "act".into(),
            command: DecisionCommand::Agent(AgentCommand::WakeUp),
            when: None,
        })),
        current: 2,
    });
    n.reset();
    if let Node::Repeater(ref r) = n {
        assert_eq!(r.current, 0);
    }
}

// ── SetMapping ──────────────────────────────────────────────────────────────

#[test]
fn set_mapping_creation() {
    let sm = SetMapping {
        key: "result".into(),
        field: "decision".into(),
    };
    assert_eq!(sm.key, "result");
    assert_eq!(sm.field, "decision");
}

// ── ParallelPolicy ──────────────────────────────────────────────────────────

#[test]
fn parallel_policy_variants() {
    let _ = ParallelPolicy::AllSuccess;
    let _ = ParallelPolicy::AnySuccess;
    let _ = ParallelPolicy::Majority;
}

// ── DslDocument ─────────────────────────────────────────────────────────────

#[test]
fn dsl_document_decision_rules() {
    let doc = DslDocument::DecisionRules {
        api_version: "v1".into(),
        metadata: Metadata {
            name: "rules".into(),
            description: None,
        },
        rules: vec![],
    };
    assert!(matches!(doc, DslDocument::DecisionRules { .. }));
}

#[test]
fn dsl_document_behavior_tree() {
    let doc = DslDocument::BehaviorTree {
        api_version: "v1".into(),
        metadata: Metadata {
            name: "tree".into(),
            description: None,
        },
        root: Node::Action(ActionNode {
            name: "act".into(),
            command: DecisionCommand::Agent(AgentCommand::WakeUp),
            when: None,
        }),
    };
    assert!(matches!(doc, DslDocument::BehaviorTree { .. }));
}

// ── Clone on Node ───────────────────────────────────────────────────────────

#[test]
fn node_can_clone() {
    let n = Node::Action(ActionNode {
        name: "act".into(),
        command: DecisionCommand::Agent(AgentCommand::WakeUp),
        when: None,
    });
    let _ = n.clone();
}

// ── YAML serde rename round-trip ───────────────────────────────────────────

#[test]
fn repeater_serializes_max_attempts_camel() {
    let node = Node::Repeater(RepeaterNode {
        name: "rep".into(),
        max_attempts: 3,
        child: Box::new(Node::Action(ActionNode {
            name: "act".into(),
            command: DecisionCommand::Agent(AgentCommand::WakeUp),
            when: None,
        })),
        current: 0,
    });
    let yaml = serde_yaml::to_string(&node).unwrap();
    assert!(yaml.contains("maxAttempts"), "expected maxAttempts in YAML, got: {}", yaml);
    assert!(!yaml.contains("max_attempts"), "should not contain snake_case");
}

#[test]
fn cooldown_serializes_duration_ms_camel() {
    let node = Node::Cooldown(CooldownNode {
        name: "cool".into(),
        duration_ms: 5000,
        child: Box::new(Node::Action(ActionNode {
            name: "act".into(),
            command: DecisionCommand::Agent(AgentCommand::WakeUp),
            when: None,
        })),
        last_success: None,
    });
    let yaml = serde_yaml::to_string(&node).unwrap();
    assert!(yaml.contains("durationMs"), "expected durationMs in YAML, got: {}", yaml);
    assert!(!yaml.contains("duration_ms"), "should not contain snake_case");
}

#[test]
fn reflection_guard_serializes_max_rounds_camel() {
    let node = Node::ReflectionGuard(ReflectionGuardNode {
        name: "rg".into(),
        max_rounds: 3,
        child: Box::new(Node::Action(ActionNode {
            name: "act".into(),
            command: DecisionCommand::Agent(AgentCommand::WakeUp),
            when: None,
        })),
    });
    let yaml = serde_yaml::to_string(&node).unwrap();
    assert!(yaml.contains("maxRounds"), "expected maxRounds in YAML, got: {}", yaml);
    assert!(!yaml.contains("max_rounds"), "should not contain snake_case");
}

#[test]
fn prompt_serializes_timeout_ms_camel() {
    let node = Node::Prompt(PromptNode {
        name: "p".into(),
        model: None,
        template: "test".into(),
        parser: decision_dsl::ast::OutputParser::Enum {
            values: vec!["a".into(), "b".into()],
            case_sensitive: false,
        },
        sets: vec![],
        timeout_ms: 30000,
        pending: false,
        sent_at: None,
    });
    let yaml = serde_yaml::to_string(&node).unwrap();
    assert!(yaml.contains("timeoutMs"), "expected timeoutMs in YAML, got: {}", yaml);
    assert!(!yaml.contains("timeout_ms"), "should not contain snake_case");
}

#[test]
fn rename_roundtrip_repeater_preserves_max_attempts() {
    let node = Node::Repeater(RepeaterNode {
        name: "rep".into(),
        max_attempts: 5,
        child: Box::new(Node::Action(ActionNode {
            name: "act".into(),
            command: DecisionCommand::Agent(AgentCommand::WakeUp),
            when: None,
        })),
        current: 0,
    });
    let yaml = serde_yaml::to_string(&node).unwrap();
    let back: Node = serde_yaml::from_str(&yaml).unwrap();
    assert!(matches!(back, Node::Repeater(r) if r.max_attempts == 5));
}

#[test]
fn rename_roundtrip_cooldown_preserves_duration_ms() {
    let node = Node::Cooldown(CooldownNode {
        name: "cool".into(),
        duration_ms: 10000,
        child: Box::new(Node::Action(ActionNode {
            name: "act".into(),
            command: DecisionCommand::Agent(AgentCommand::WakeUp),
            when: None,
        })),
        last_success: None,
    });
    let yaml = serde_yaml::to_string(&node).unwrap();
    let back: Node = serde_yaml::from_str(&yaml).unwrap();
    assert!(matches!(back, Node::Cooldown(c) if c.duration_ms == 10000));
}

#[test]
fn rename_roundtrip_reflection_guard_preserves_max_rounds() {
    let node = Node::ReflectionGuard(ReflectionGuardNode {
        name: "rg".into(),
        max_rounds: 4,
        child: Box::new(Node::Action(ActionNode {
            name: "act".into(),
            command: DecisionCommand::Agent(AgentCommand::WakeUp),
            when: None,
        })),
    });
    let yaml = serde_yaml::to_string(&node).unwrap();
    let back: Node = serde_yaml::from_str(&yaml).unwrap();
    assert!(matches!(back, Node::ReflectionGuard(rg) if rg.max_rounds == 4));
}

#[test]
fn rename_roundtrip_prompt_preserves_timeout_ms() {
    let node = Node::Prompt(PromptNode {
        name: "p".into(),
        model: Some("claude".into()),
        template: "test".into(),
        parser: decision_dsl::ast::OutputParser::Enum {
            values: vec!["a".into()],
            case_sensitive: true,
        },
        sets: vec![],
        timeout_ms: 60000,
        pending: false,
        sent_at: None,
    });
    let yaml = serde_yaml::to_string(&node).unwrap();
    let back: Node = serde_yaml::from_str(&yaml).unwrap();
    assert!(matches!(back, Node::Prompt(p) if p.timeout_ms == 60000));
}
