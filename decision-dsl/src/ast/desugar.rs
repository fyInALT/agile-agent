
use crate::ext::blackboard::BlackboardValue;
use crate::ext::command::{AgentCommand, DecisionCommand, HumanCommand};
use crate::ext::error::ParseError;

use super::document::{
    DslDocument, OnError, PipelineSpec, PipelineStep, RuleSpec, Spec, SwitchOn, SwitchSpec,
    ThenSpec, Tree, TreeKind, WhenSpec,
};
use super::eval::{Evaluator, EvaluatorRegistry};
use super::node::{
    ActionNode, ConditionNode, CooldownNode, Node, PromptNode, ReflectionGuardNode, RepeaterNode,
    SelectorNode, SequenceNode, SetMapping, SubTreeNode, WhenNode,
};

impl DslDocument {
    pub fn desugar(self, registry: &EvaluatorRegistry) -> Result<Tree, ParseError> {
        match self {
            DslDocument::DecisionRules {
                api_version,
                metadata,
                rules,
            } => {
                let mut children = Vec::new();
                for rule in rules {
                    children.push(desugar_rule(rule, registry)?);
                }
                // NoMatch fallback
                children.push(Node::Action(ActionNode {
                    name: "no_match".into(),
                    command: DecisionCommand::Agent(AgentCommand::ApproveAndContinue),
                    when: None,
                }));
                let root_name = format!("{}_root", metadata.name);
                Ok(Tree {
                    api_version,
                    kind: TreeKind::BehaviorTree,
                    metadata,
                    spec: Spec {
                        root: Node::Selector(SelectorNode {
                            name: root_name,
                            children,
                            active_child: None,
                        }),
                    },
                })
            }
            DslDocument::BehaviorTree {
                api_version,
                metadata,
                root,
            } => Ok(Tree {
                api_version,
                kind: TreeKind::BehaviorTree,
                metadata,
                spec: Spec { root },
            }),
            DslDocument::SubTree {
                api_version,
                metadata,
                root,
            } => Ok(Tree {
                api_version,
                kind: TreeKind::SubTree,
                metadata,
                spec: Spec { root },
            }),
        }
    }
}

fn desugar_rule(rule: RuleSpec, registry: &EvaluatorRegistry) -> Result<Node, ParseError> {
    let inner = desugar_then(rule.action, registry)?;

    // Wrap in Sequence + Condition if `if` is present
    let mut node = if let Some(evaluator) = rule.condition {
        Node::Sequence(SequenceNode {
            name: format!("{}_guard", rule.name),
            children: vec![
                Node::Condition(ConditionNode {
                    name: format!("{}_cond", rule.name),
                    evaluator,
                }),
                inner,
            ],
            active_child: None,
        })
    } else {
        inner
    };

    // on_error (innermost — closest to the action)
    node = match rule.on_error {
        Some(OnError::Escalate) => Node::Selector(SelectorNode {
            name: format!("{}_error_handler", rule.name),
            children: vec![
                node,
                Node::Action(ActionNode {
                    name: format!("{}_escalate_fallback", rule.name),
                    command: DecisionCommand::Human(HumanCommand::Escalate {
                        reason: format!("Rule '{}' failed — escalating to human", rule.name),
                        context: None,
                    }),
                    when: None,
                }),
            ],
            active_child: None,
        }),
        Some(OnError::Retry) => Node::Repeater(RepeaterNode {
            name: format!("{}_retry", rule.name),
            max_attempts: 2,
            child: Box::new(node),
            current: 0,
        }),
        Some(OnError::Skip) | None => node,
    };

    // ReflectionGuard (inside Cooldown)
    if let Some(max_rounds) = rule.reflection_max_rounds {
        node = Node::ReflectionGuard(ReflectionGuardNode {
            name: format!("{}_reflection", rule.name),
            max_rounds,
            child: Box::new(node),
        });
    }

    // Cooldown (outermost)
    if let Some(ms) = rule.cooldown_ms {
        node = Node::Cooldown(CooldownNode {
            name: format!("{}_cooldown", rule.name),
            duration_ms: ms,
            child: Box::new(node),
            last_success: None,
        });
    }

    Ok(node)
}

fn desugar_then(then: ThenSpec, registry: &EvaluatorRegistry) -> Result<Node, ParseError> {
    match then {
        ThenSpec::InlineCommand { command } => Ok(Node::Action(ActionNode {
            name: "emit".into(),
            command,
            when: None,
        })),
        ThenSpec::Switch(switch) => desugar_switch(switch, registry),
        ThenSpec::When(when) => desugar_when(*when, registry),
        ThenSpec::Pipeline(pipeline) => desugar_pipeline(pipeline, registry),
        ThenSpec::SubTree { ref_name } => Ok(Node::SubTree(SubTreeNode {
            name: ref_name.clone(),
            ref_name,
            resolved_root: None,
        })),
    }
}

fn desugar_switch(switch: SwitchSpec, registry: &EvaluatorRegistry) -> Result<Node, ParseError> {
    match switch.on {
        SwitchOn::Prompt {
            model,
            timeout_ms,
            template,
            parser,
            result_key,
        } => {
            let result_key = result_key.unwrap_or_else(|| "decision".into());

            let prompt_node = Node::Prompt(PromptNode {
                name: format!("{}_prompt", switch.name),
                model,
                template,
                parser,
                sets: vec![SetMapping {
                    key: result_key.clone(),
                    field: "decision".into(),
                }],
                timeout_ms: timeout_ms.unwrap_or(30000),
                pending: false,
                sent_at: None,
            });

            let mut case_nodes = Vec::new();
            for (value, action) in switch.cases {
                let when = Node::When(WhenNode {
                    name: format!("{}_{}", switch.name, value.to_lowercase()),
                    condition: Evaluator::VariableIs {
                        key: result_key.clone(),
                        expected: BlackboardValue::String(value),
                    },
                    action: Box::new(desugar_then(*action, registry)?),
                });
                case_nodes.push(when);
            }

            if let Some(default_action) = switch.default {
                case_nodes.push(desugar_then(*default_action, registry)?);
            }

            Ok(Node::Sequence(SequenceNode {
                name: switch.name.clone(),
                children: vec![
                    prompt_node,
                    Node::Selector(SelectorNode {
                        name: format!("{}_branch", switch.name),
                        children: case_nodes,
                        active_child: None,
                    }),
                ],
                active_child: None,
            }))
        }
        SwitchOn::Variable { key } => {
            let mut case_nodes = Vec::new();
            for (value, action) in switch.cases {
                let when = Node::When(WhenNode {
                    name: format!("{}_{}", switch.name, value.to_lowercase()),
                    condition: Evaluator::VariableIs {
                        key: key.clone(),
                        expected: BlackboardValue::String(value),
                    },
                    action: Box::new(desugar_then(*action, registry)?),
                });
                case_nodes.push(when);
            }
            if let Some(default_action) = switch.default {
                case_nodes.push(desugar_then(*default_action, registry)?);
            }
            Ok(Node::Selector(SelectorNode {
                name: format!("{}_branch", switch.name),
                children: case_nodes,
                active_child: None,
            }))
        }
    }
}

fn desugar_when(when: WhenSpec, registry: &EvaluatorRegistry) -> Result<Node, ParseError> {
    let mut node = Node::When(WhenNode {
        name: when.name.clone(),
        condition: when.condition,
        action: Box::new(desugar_then(when.then, registry)?),
    });

    node = match when.on_error {
        Some(OnError::Escalate) => Node::Selector(SelectorNode {
            name: format!("{}_error_handler", when.name),
            children: vec![
                node,
                Node::Action(ActionNode {
                    name: format!("{}_escalate_fallback", when.name),
                    command: DecisionCommand::Human(HumanCommand::Escalate {
                        reason: format!("When '{}' failed — escalating to human", when.name),
                        context: None,
                    }),
                    when: None,
                }),
            ],
            active_child: None,
        }),
        Some(OnError::Retry) => Node::Repeater(RepeaterNode {
            name: format!("{}_retry", when.name),
            max_attempts: 2,
            child: Box::new(node),
            current: 0,
        }),
        Some(OnError::Skip) | None => node,
    };

    Ok(node)
}

fn desugar_pipeline(pipeline: PipelineSpec, _registry: &EvaluatorRegistry) -> Result<Node, ParseError> {
    let mut children = Vec::new();
    for step in pipeline.steps {
        match step {
            PipelineStep::Guard { condition } => {
                children.push(Node::Condition(ConditionNode {
                    name: format!("{}_step", pipeline.name),
                    evaluator: condition,
                }));
            }
            PipelineStep::Action { command } => {
                children.push(Node::Action(ActionNode {
                    name: format!("{}_emit", pipeline.name),
                    command,
                    when: None,
                }));
            }
        }
    }
    Ok(Node::Sequence(SequenceNode {
        name: pipeline.name,
        children,
        active_child: None,
    }))
}
