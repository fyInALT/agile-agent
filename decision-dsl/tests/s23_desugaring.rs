use decision_dsl::ast::{
    ActionNode, DslDocument, Evaluator, Metadata, Node, OnError, PipelineSpec, PipelineStep,
    SwitchOn, SwitchSpec, ThenSpec, TreeKind, WhenSpec,
};
use decision_dsl::ext::blackboard::BlackboardValue;
use decision_dsl::ext::command::{AgentCommand, DecisionCommand};

fn make_action(name: &str, cmd: DecisionCommand) -> Node {
    Node::Action(ActionNode {
        name: name.into(),
        command: cmd,
        when: None,
    })
}

fn simple_rules_doc() -> DslDocument {
    DslDocument::DecisionRules {
        api_version: "decision.agile-agent.io/v1".into(),
        metadata: Metadata {
            name: "test".into(),
            description: None,
        },
        rules: vec![
            decision_dsl::ast::RuleSpec {
                priority: 1,
                name: "rule1".into(),
                condition: None,
                action: ThenSpec::InlineCommand {
                    command: DecisionCommand::Agent(AgentCommand::WakeUp),
                },
                cooldown_ms: None,
                reflection_max_rounds: None,
                on_error: None,
            },
        ],
    }
}

// ── DecisionRules desugaring ────────────────────────────────────────────────

#[test]
fn decision_rules_desugars_to_selector() {
    let doc = simple_rules_doc();
    let tree = doc.desugar().unwrap();
    assert_eq!(tree.api_version, "decision.agile-agent.io/v1");
    assert!(matches!(tree.kind, TreeKind::BehaviorTree));
    assert!(matches!(tree.spec.root, Node::Selector(_)));
}

#[test]
fn decision_rules_adds_no_match_fallback() {
    let doc = simple_rules_doc();
    let tree = doc.desugar().unwrap();
    if let Node::Selector(sel) = tree.spec.root {
        assert_eq!(sel.children.len(), 2); // rule + fallback
        let last = &sel.children[1];
        assert!(matches!(last, Node::Action(_)));
    } else {
        panic!("expected Selector");
    }
}

// ── Rule with condition desugars to Sequence ────────────────────────────────

#[test]
fn rule_with_condition_desugars_to_sequence() {
    let doc = DslDocument::DecisionRules {
        api_version: "v1".into(),
        metadata: Metadata { name: "test".into(), description: None },
        rules: vec![
            decision_dsl::ast::RuleSpec {
                priority: 1,
                name: "rule1".into(),
                condition: Some(Evaluator::VariableIs {
                    key: "x".into(),
                    expected: BlackboardValue::Boolean(true),
                }),
                action: ThenSpec::InlineCommand {
                    command: DecisionCommand::Agent(AgentCommand::WakeUp),
                },
                cooldown_ms: None,
                reflection_max_rounds: None,
                on_error: None,
            },
        ],
    };
    let tree = doc.desugar().unwrap();
    if let Node::Selector(sel) = tree.spec.root {
        let rule_node = &sel.children[0];
        assert!(matches!(rule_node, Node::Sequence(_)));
        if let Node::Sequence(seq) = rule_node {
            assert_eq!(seq.children.len(), 2); // Condition + Action
            assert!(matches!(seq.children[0], Node::Condition(_)));
            assert!(matches!(seq.children[1], Node::Action(_)));
        }
    }
}

// ── Decorator wrapping order ────────────────────────────────────────────────

#[test]
fn rule_cooldown_wraps_outermost() {
    let doc = DslDocument::DecisionRules {
        api_version: "v1".into(),
        metadata: Metadata { name: "test".into(), description: None },
        rules: vec![
            decision_dsl::ast::RuleSpec {
                priority: 1,
                name: "rule1".into(),
                condition: None,
                action: ThenSpec::InlineCommand {
                    command: DecisionCommand::Agent(AgentCommand::WakeUp),
                },
                cooldown_ms: Some(1000),
                reflection_max_rounds: Some(2),
                on_error: Some(OnError::Escalate),
            },
        ],
    };
    let tree = doc.desugar().unwrap();
    if let Node::Selector(sel) = tree.spec.root {
        let rule_node = &sel.children[0];
        // Outermost should be Cooldown
        assert!(matches!(rule_node, Node::Cooldown(_)));
        if let Node::Cooldown(cool) = rule_node {
            assert_eq!(cool.duration_ms, 1000);
            // Inside Cooldown should be ReflectionGuard
            assert!(matches!(cool.child.as_ref(), Node::ReflectionGuard(_)));
            if let Node::ReflectionGuard(rg) = cool.child.as_ref() {
                assert_eq!(rg.max_rounds, 2);
                // Inside ReflectionGuard should be on_error (Selector for Escalate)
                assert!(matches!(rg.child.as_ref(), Node::Selector(_)));
            }
        }
    }
}

// ── on_error desugaring ─────────────────────────────────────────────────────

#[test]
fn rule_on_error_escalate_desugars_to_selector() {
    let doc = DslDocument::DecisionRules {
        api_version: "v1".into(),
        metadata: Metadata { name: "test".into(), description: None },
        rules: vec![
            decision_dsl::ast::RuleSpec {
                priority: 1,
                name: "rule1".into(),
                condition: None,
                action: ThenSpec::InlineCommand {
                    command: DecisionCommand::Agent(AgentCommand::WakeUp),
                },
                cooldown_ms: None,
                reflection_max_rounds: None,
                on_error: Some(OnError::Escalate),
            },
        ],
    };
    let tree = doc.desugar().unwrap();
    if let Node::Selector(sel) = tree.spec.root {
        let rule_node = &sel.children[0];
        assert!(matches!(rule_node, Node::Selector(_)));
        if let Node::Selector(err_sel) = rule_node {
            assert_eq!(err_sel.children.len(), 2);
            assert!(matches!(err_sel.children[0], Node::Action(_)));
            assert!(matches!(err_sel.children[1], Node::Action(_)));
        }
    }
}

#[test]
fn rule_on_error_retry_desugars_to_repeater() {
    let doc = DslDocument::DecisionRules {
        api_version: "v1".into(),
        metadata: Metadata { name: "test".into(), description: None },
        rules: vec![
            decision_dsl::ast::RuleSpec {
                priority: 1,
                name: "rule1".into(),
                condition: None,
                action: ThenSpec::InlineCommand {
                    command: DecisionCommand::Agent(AgentCommand::WakeUp),
                },
                cooldown_ms: None,
                reflection_max_rounds: None,
                on_error: Some(OnError::Retry),
            },
        ],
    };
    let tree = doc.desugar().unwrap();
    if let Node::Selector(sel) = tree.spec.root {
        let rule_node = &sel.children[0];
        assert!(matches!(rule_node, Node::Repeater(_)));
        if let Node::Repeater(rep) = rule_node {
            assert_eq!(rep.max_attempts, 2);
        }
    }
}

#[test]
fn rule_on_error_skip_is_pass_through() {
    let doc = DslDocument::DecisionRules {
        api_version: "v1".into(),
        metadata: Metadata { name: "test".into(), description: None },
        rules: vec![
            decision_dsl::ast::RuleSpec {
                priority: 1,
                name: "rule1".into(),
                condition: None,
                action: ThenSpec::InlineCommand {
                    command: DecisionCommand::Agent(AgentCommand::WakeUp),
                },
                cooldown_ms: None,
                reflection_max_rounds: None,
                on_error: Some(OnError::Skip),
            },
        ],
    };
    let tree = doc.desugar().unwrap();
    if let Node::Selector(sel) = tree.spec.root {
        let rule_node = &sel.children[0];
        // Skip should not add any wrapping
        assert!(matches!(rule_node, Node::Action(_)));
    }
}

// ── Switch on Prompt desugaring ─────────────────────────────────────────────

#[test]
fn switch_on_prompt_desugars_to_sequence_prompt_selector() {
    let mut cases = std::collections::HashMap::new();
    cases.insert(
        "yes".into(),
        Box::new(ThenSpec::InlineCommand {
            command: DecisionCommand::Agent(AgentCommand::WakeUp),
        }),
    );
    cases.insert(
        "no".into(),
        Box::new(ThenSpec::InlineCommand {
            command: DecisionCommand::Agent(AgentCommand::ApproveAndContinue),
        }),
    );

    let doc = DslDocument::DecisionRules {
        api_version: "v1".into(),
        metadata: Metadata { name: "test".into(), description: None },
        rules: vec![
            decision_dsl::ast::RuleSpec {
                priority: 1,
                name: "ask".into(),
                condition: None,
                action: ThenSpec::Switch(SwitchSpec {
                    name: "ask".into(),
                    on: SwitchOn::Prompt {
                        model: None,
                        timeout_ms: None,
                        template: "yes or no?".into(),
                        parser: decision_dsl::ast::OutputParser::Enum {
                            values: vec!["yes".into(), "no".into()],
                            case_sensitive: false,
                        },
                        result_key: None,
                    },
                    cases,
                    default: None,
                }),
                cooldown_ms: None,
                reflection_max_rounds: None,
                on_error: None,
            },
        ],
    };
    let tree = doc.desugar().unwrap();
    if let Node::Selector(sel) = tree.spec.root {
        let rule_node = &sel.children[0];
        assert!(matches!(rule_node, Node::Sequence(_)));
        if let Node::Sequence(seq) = rule_node {
            assert_eq!(seq.children.len(), 2);
            assert!(matches!(seq.children[0], Node::Prompt(_)));
            assert!(matches!(seq.children[1], Node::Selector(_)));
        }
    }
}

// ── Switch on Variable desugaring ───────────────────────────────────────────

#[test]
fn switch_on_variable_desugars_to_selector() {
    let mut cases = std::collections::HashMap::new();
    cases.insert(
        "A".into(),
        Box::new(ThenSpec::InlineCommand {
            command: DecisionCommand::Agent(AgentCommand::WakeUp),
        }),
    );

    let doc = DslDocument::DecisionRules {
        api_version: "v1".into(),
        metadata: Metadata { name: "test".into(), description: None },
        rules: vec![
            decision_dsl::ast::RuleSpec {
                priority: 1,
                name: "choose".into(),
                condition: None,
                action: ThenSpec::Switch(SwitchSpec {
                    name: "choose".into(),
                    on: SwitchOn::Variable { key: "choice".into() },
                    cases,
                    default: None,
                }),
                cooldown_ms: None,
                reflection_max_rounds: None,
                on_error: None,
            },
        ],
    };
    let tree = doc.desugar().unwrap();
    if let Node::Selector(sel) = tree.spec.root {
        let rule_node = &sel.children[0];
        assert!(matches!(rule_node, Node::Selector(_)));
    }
}

// ── Switch result_key configuration ──────────────────────────────────────────

#[test]
fn switch_result_key_defaults_to_decision() {
    let mut cases = std::collections::HashMap::new();
    cases.insert(
        "yes".into(),
        Box::new(ThenSpec::InlineCommand {
            command: DecisionCommand::Agent(AgentCommand::WakeUp),
        }),
    );

    let doc = DslDocument::DecisionRules {
        api_version: "v1".into(),
        metadata: Metadata { name: "test".into(), description: None },
        rules: vec![
            decision_dsl::ast::RuleSpec {
                priority: 1,
                name: "ask".into(),
                condition: None,
                action: ThenSpec::Switch(SwitchSpec {
                    name: "ask".into(),
                    on: SwitchOn::Prompt {
                        model: None,
                        timeout_ms: None,
                        template: "yes or no?".into(),
                        parser: decision_dsl::ast::OutputParser::Enum {
                            values: vec!["yes".into(), "no".into()],
                            case_sensitive: false,
                        },
                        result_key: None, // Should default to "decision"
                    },
                    cases,
                    default: None,
                }),
                cooldown_ms: None,
                reflection_max_rounds: None,
                on_error: None,
            },
        ],
    };
    let tree = doc.desugar().unwrap();
    if let Node::Selector(sel) = tree.spec.root {
        let rule_node = &sel.children[0];
        if let Node::Sequence(seq) = rule_node {
            // Check Prompt node sets key = "decision"
            if let Node::Prompt(prompt) = &seq.children[0] {
                assert_eq!(prompt.sets.len(), 1);
                assert_eq!(prompt.sets[0].key, "decision");
            }
            // Check When node matches against "decision" key
            if let Node::Selector(branch_sel) = &seq.children[1] {
                if let Node::When(when) = &branch_sel.children[0] {
                    match &when.condition {
                        decision_dsl::ast::Evaluator::VariableIs { key, .. } => {
                            assert_eq!(key, "decision");
                        }
                        _ => panic!("expected VariableIs"),
                    }
                }
            }
        }
    }
}

#[test]
fn switch_custom_result_key_stored_correctly() {
    let mut cases = std::collections::HashMap::new();
    cases.insert(
        "yes".into(),
        Box::new(ThenSpec::InlineCommand {
            command: DecisionCommand::Agent(AgentCommand::WakeUp),
        }),
    );

    let doc = DslDocument::DecisionRules {
        api_version: "v1".into(),
        metadata: Metadata { name: "test".into(), description: None },
        rules: vec![
            decision_dsl::ast::RuleSpec {
                priority: 1,
                name: "ask".into(),
                condition: None,
                action: ThenSpec::Switch(SwitchSpec {
                    name: "ask".into(),
                    on: SwitchOn::Prompt {
                        model: None,
                        timeout_ms: None,
                        template: "yes or no?".into(),
                        parser: decision_dsl::ast::OutputParser::Enum {
                            values: vec!["yes".into(), "no".into()],
                            case_sensitive: false,
                        },
                        result_key: Some("my_result".into()), // Custom result_key
                    },
                    cases,
                    default: None,
                }),
                cooldown_ms: None,
                reflection_max_rounds: None,
                on_error: None,
            },
        ],
    };
    let tree = doc.desugar().unwrap();
    if let Node::Selector(sel) = tree.spec.root {
        let rule_node = &sel.children[0];
        if let Node::Sequence(seq) = rule_node {
            // Check Prompt node sets key = "my_result"
            if let Node::Prompt(prompt) = &seq.children[0] {
                assert_eq!(prompt.sets.len(), 1);
                assert_eq!(prompt.sets[0].key, "my_result");
            }
            // Check When node matches against "my_result" key
            if let Node::Selector(branch_sel) = &seq.children[1] {
                if let Node::When(when) = &branch_sel.children[0] {
                    match &when.condition {
                        decision_dsl::ast::Evaluator::VariableIs { key, .. } => {
                            assert_eq!(key, "my_result");
                        }
                        _ => panic!("expected VariableIs"),
                    }
                }
            }
        }
    }
}

// ── Switch default with nested ThenSpec ───────────────────────────────────────

#[test]
fn switch_default_supports_nested_when() {
    let mut cases = std::collections::HashMap::new();
    cases.insert(
        "A".into(),
        Box::new(ThenSpec::InlineCommand {
            command: DecisionCommand::Agent(AgentCommand::WakeUp),
        }),
    );

    let default_action = ThenSpec::When(Box::new(WhenSpec {
        name: "default_when".into(),
        condition: decision_dsl::ast::Evaluator::VariableIs {
            key: "fallback".into(),
            expected: decision_dsl::ext::blackboard::BlackboardValue::Boolean(true),
        },
        then: ThenSpec::InlineCommand {
            command: DecisionCommand::Agent(AgentCommand::ApproveAndContinue),
        },
        on_error: None,
    }));

    let doc = DslDocument::DecisionRules {
        api_version: "v1".into(),
        metadata: Metadata { name: "test".into(), description: None },
        rules: vec![
            decision_dsl::ast::RuleSpec {
                priority: 1,
                name: "choose".into(),
                condition: None,
                action: ThenSpec::Switch(SwitchSpec {
                    name: "choose".into(),
                    on: SwitchOn::Variable { key: "choice".into() },
                    cases,
                    default: Some(Box::new(default_action)),
                }),
                cooldown_ms: None,
                reflection_max_rounds: None,
                on_error: None,
            },
        ],
    };
    let tree = doc.desugar().unwrap();
    if let Node::Selector(sel) = tree.spec.root {
        let rule_node = &sel.children[0];
        if let Node::Selector(branch_sel) = rule_node {
            // Last child should be When node (the default)
            let default_node = &branch_sel.children[1];
            assert!(matches!(default_node, Node::When(_)));
            if let Node::When(when) = default_node {
                assert_eq!(when.name, "default_when");
            }
        }
    }
}

#[test]
fn switch_default_supports_nested_switch() {
    let mut cases = std::collections::HashMap::new();
    cases.insert(
        "A".into(),
        Box::new(ThenSpec::InlineCommand {
            command: DecisionCommand::Agent(AgentCommand::WakeUp),
        }),
    );

    let mut inner_cases = std::collections::HashMap::new();
    inner_cases.insert(
        "X".into(),
        Box::new(ThenSpec::InlineCommand {
            command: DecisionCommand::Agent(AgentCommand::ApproveAndContinue),
        }),
    );

    let default_action = ThenSpec::Switch(SwitchSpec {
        name: "inner_switch".into(),
        on: SwitchOn::Variable { key: "inner_choice".into() },
        cases: inner_cases,
        default: None,
    });

    let doc = DslDocument::DecisionRules {
        api_version: "v1".into(),
        metadata: Metadata { name: "test".into(), description: None },
        rules: vec![
            decision_dsl::ast::RuleSpec {
                priority: 1,
                name: "choose".into(),
                condition: None,
                action: ThenSpec::Switch(SwitchSpec {
                    name: "choose".into(),
                    on: SwitchOn::Variable { key: "choice".into() },
                    cases,
                    default: Some(Box::new(default_action)),
                }),
                cooldown_ms: None,
                reflection_max_rounds: None,
                on_error: None,
            },
        ],
    };
    let tree = doc.desugar().unwrap();
    if let Node::Selector(sel) = tree.spec.root {
        let rule_node = &sel.children[0];
        if let Node::Selector(branch_sel) = rule_node {
            // Last child should be Selector (nested switch)
            let default_node = &branch_sel.children[1];
            assert!(matches!(default_node, Node::Selector(_)));
            if let Node::Selector(inner_sel) = default_node {
                assert_eq!(inner_sel.name, "inner_switch_branch");
            }
        }
    }
}

#[test]
fn switch_default_supports_nested_pipeline() {
    let mut cases = std::collections::HashMap::new();
    cases.insert(
        "A".into(),
        Box::new(ThenSpec::InlineCommand {
            command: DecisionCommand::Agent(AgentCommand::WakeUp),
        }),
    );

    let default_action = ThenSpec::Pipeline(PipelineSpec {
        name: "default_pipeline".into(),
        steps: vec![
            PipelineStep::Guard {
                condition: decision_dsl::ast::Evaluator::VariableIs {
                    key: "x".into(),
                    expected: decision_dsl::ext::blackboard::BlackboardValue::Boolean(true),
                },
            },
            PipelineStep::Action {
                command: DecisionCommand::Agent(AgentCommand::ApproveAndContinue),
            },
        ],
    });

    let doc = DslDocument::DecisionRules {
        api_version: "v1".into(),
        metadata: Metadata { name: "test".into(), description: None },
        rules: vec![
            decision_dsl::ast::RuleSpec {
                priority: 1,
                name: "choose".into(),
                condition: None,
                action: ThenSpec::Switch(SwitchSpec {
                    name: "choose".into(),
                    on: SwitchOn::Variable { key: "choice".into() },
                    cases,
                    default: Some(Box::new(default_action)),
                }),
                cooldown_ms: None,
                reflection_max_rounds: None,
                on_error: None,
            },
        ],
    };
    let tree = doc.desugar().unwrap();
    if let Node::Selector(sel) = tree.spec.root {
        let rule_node = &sel.children[0];
        if let Node::Selector(branch_sel) = rule_node {
            // Last child should be Sequence (pipeline)
            let default_node = &branch_sel.children[1];
            assert!(matches!(default_node, Node::Sequence(_)));
            if let Node::Sequence(seq) = default_node {
                assert_eq!(seq.name, "default_pipeline");
            }
        }
    }
}

// ── When desugaring ─────────────────────────────────────────────────────────

#[test]
fn when_desugars_to_when_node() {
    let doc = DslDocument::DecisionRules {
        api_version: "v1".into(),
        metadata: Metadata { name: "test".into(), description: None },
        rules: vec![
            decision_dsl::ast::RuleSpec {
                priority: 1,
                name: "rule1".into(),
                condition: None,
                action: ThenSpec::When(Box::new(WhenSpec {
                    name: "when1".into(),
                    condition: Evaluator::VariableIs {
                        key: "x".into(),
                        expected: BlackboardValue::Boolean(true),
                    },
                    then: ThenSpec::InlineCommand {
                        command: DecisionCommand::Agent(AgentCommand::WakeUp),
                    },
                    on_error: None,
                })),
                cooldown_ms: None,
                reflection_max_rounds: None,
                on_error: None,
            },
        ],
    };
    let tree = doc.desugar().unwrap();
    if let Node::Selector(sel) = tree.spec.root {
        let rule_node = &sel.children[0];
        assert!(matches!(rule_node, Node::When(_)));
        if let Node::When(w) = rule_node {
            assert_eq!(w.name, "when1");
            assert!(matches!(w.action.as_ref(), Node::Action(_)));
        }
    }
}

#[test]
fn when_on_error_escalate_desugars_to_selector() {
    let doc = DslDocument::DecisionRules {
        api_version: "v1".into(),
        metadata: Metadata { name: "test".into(), description: None },
        rules: vec![decision_dsl::ast::RuleSpec {
            priority: 1,
            name: "rule1".into(),
            condition: None,
            action: ThenSpec::When(Box::new(WhenSpec {
                name: "when1".into(),
                condition: Evaluator::VariableIs {
                    key: "x".into(),
                    expected: BlackboardValue::Boolean(true),
                },
                then: ThenSpec::InlineCommand {
                    command: DecisionCommand::Agent(AgentCommand::WakeUp),
                },
                on_error: Some(OnError::Escalate),
            })),
            cooldown_ms: None,
            reflection_max_rounds: None,
            on_error: None,
        }],
    };
    let tree = doc.desugar().unwrap();
    if let Node::Selector(sel) = tree.spec.root {
        let rule_node = &sel.children[0];
        // Should be Selector wrapping WhenNode + Escalate fallback
        assert!(matches!(rule_node, Node::Selector(_)));
        if let Node::Selector(err_sel) = rule_node {
            assert_eq!(err_sel.children.len(), 2);
            assert!(matches!(err_sel.children[0], Node::When(_)));
            assert!(matches!(err_sel.children[1], Node::Action(_)));
        }
    }
}

#[test]
fn when_on_error_retry_desugars_to_repeater() {
    let doc = DslDocument::DecisionRules {
        api_version: "v1".into(),
        metadata: Metadata { name: "test".into(), description: None },
        rules: vec![decision_dsl::ast::RuleSpec {
            priority: 1,
            name: "rule1".into(),
            condition: None,
            action: ThenSpec::When(Box::new(WhenSpec {
                name: "when1".into(),
                condition: Evaluator::VariableIs {
                    key: "x".into(),
                    expected: BlackboardValue::Boolean(true),
                },
                then: ThenSpec::InlineCommand {
                    command: DecisionCommand::Agent(AgentCommand::WakeUp),
                },
                on_error: Some(OnError::Retry),
            })),
            cooldown_ms: None,
            reflection_max_rounds: None,
            on_error: None,
        }],
    };
    let tree = doc.desugar().unwrap();
    if let Node::Selector(sel) = tree.spec.root {
        let rule_node = &sel.children[0];
        assert!(matches!(rule_node, Node::Repeater(_)));
        if let Node::Repeater(rep) = rule_node {
            assert_eq!(rep.max_attempts, 2);
            assert!(matches!(rep.child.as_ref(), Node::When(_)));
        }
    }
}

// ── Pipeline desugaring ─────────────────────────────────────────────────────

#[test]
fn pipeline_desugars_to_sequence() {
    let doc = DslDocument::DecisionRules {
        api_version: "v1".into(),
        metadata: Metadata { name: "test".into(), description: None },
        rules: vec![
            decision_dsl::ast::RuleSpec {
                priority: 1,
                name: "rule1".into(),
                condition: None,
                action: ThenSpec::Pipeline(decision_dsl::ast::PipelineSpec {
                    name: "pipe".into(),
                    steps: vec![
                        decision_dsl::ast::PipelineStep::Guard {
                            condition: Evaluator::VariableIs {
                                key: "x".into(),
                                expected: BlackboardValue::Boolean(true),
                            },
                        },
                        decision_dsl::ast::PipelineStep::Action {
                            command: DecisionCommand::Agent(AgentCommand::WakeUp),
                        },
                    ],
                }),
                cooldown_ms: None,
                reflection_max_rounds: None,
                on_error: None,
            },
        ],
    };
    let tree = doc.desugar().unwrap();
    if let Node::Selector(sel) = tree.spec.root {
        let rule_node = &sel.children[0];
        assert!(matches!(rule_node, Node::Sequence(_)));
        if let Node::Sequence(seq) = rule_node {
            assert_eq!(seq.children.len(), 2);
            assert!(matches!(seq.children[0], Node::Condition(_)));
            assert!(matches!(seq.children[1], Node::Action(_)));
        }
    }
}

// ── SubTree reference desugaring ────────────────────────────────────────────

#[test]
fn subtree_ref_desugars_to_subtree_node() {
    let doc = DslDocument::DecisionRules {
        api_version: "v1".into(),
        metadata: Metadata { name: "test".into(), description: None },
        rules: vec![
            decision_dsl::ast::RuleSpec {
                priority: 1,
                name: "rule1".into(),
                condition: None,
                action: ThenSpec::SubTree { ref_name: "helper".into() },
                cooldown_ms: None,
                reflection_max_rounds: None,
                on_error: None,
            },
        ],
    };
    let tree = doc.desugar().unwrap();
    if let Node::Selector(sel) = tree.spec.root {
        let rule_node = &sel.children[0];
        assert!(matches!(rule_node, Node::SubTree(_)));
        if let Node::SubTree(st) = rule_node {
            assert_eq!(st.ref_name, "helper");
        }
    }
}

// ── BehaviorTree passthrough ────────────────────────────────────────────────

#[test]
fn behavior_tree_desugar_passthrough() {
    let doc = DslDocument::BehaviorTree {
        api_version: "v1".into(),
        metadata: Metadata { name: "bt".into(), description: None },
        root: make_action("act", DecisionCommand::Agent(AgentCommand::WakeUp)),
    };
    let tree = doc.desugar().unwrap();
    assert_eq!(tree.api_version, "v1");
    assert!(matches!(tree.kind, TreeKind::BehaviorTree));
    assert!(matches!(tree.spec.root, Node::Action(_)));
}

// ── SubTree passthrough ─────────────────────────────────────────────────────

#[test]
fn subtree_desugar_passthrough() {
    let doc = DslDocument::SubTree {
        api_version: "v1".into(),
        metadata: Metadata { name: "st".into(), description: None },
        root: make_action("act", DecisionCommand::Agent(AgentCommand::WakeUp)),
    };
    let tree = doc.desugar().unwrap();
    assert_eq!(tree.api_version, "v1");
    assert!(matches!(tree.kind, TreeKind::SubTree));
    assert!(matches!(tree.spec.root, Node::Action(_)));
}
