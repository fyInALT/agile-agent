use std::collections::HashMap;
use std::sync::OnceLock;

use minijinja::{Environment, UndefinedBehavior, Value};

use crate::ext::blackboard::{Blackboard, BlackboardValue};
use crate::ext::command::{
    AgentCommand, DecisionCommand, GitCommand, HumanCommand, ProviderCommand, TaskCommand,
};
use crate::ext::error::RuntimeError;

// ── Global Template Environment ─────────────────────────────────────────────

static TEMPLATE_ENV: OnceLock<Environment<'static>> = OnceLock::new();

fn get_template_env() -> &'static Environment<'static> {
    TEMPLATE_ENV.get_or_init(|| {
        let mut env = Environment::new();
        env.set_undefined_behavior(UndefinedBehavior::Strict);
        env.add_filter("slugify", |value: String| {
            value
                .to_lowercase()
                .replace(|c: char| !c.is_alphanumeric() && c != '-', "-")
                .replace("--", "-")
                .trim_matches('-')
                .to_string()
        });
        env.add_filter("truncate", |value: String, n: usize| {
            if value.len() <= n {
                value
            } else {
                format!("{}...", &value[..n])
            }
        });
        env
    })
}

// ── Blackboard to minijinja Value conversion ────────────────────────────────

fn blackboard_value_to_minijinja(value: &BlackboardValue) -> Value {
    match value {
        BlackboardValue::String(s) => Value::from(s.as_str()),
        BlackboardValue::Integer(i) => Value::from(*i),
        BlackboardValue::Float(f) => Value::from(*f),
        BlackboardValue::Boolean(b) => Value::from(*b),
        BlackboardValue::List(l) => {
            Value::from(l.iter().map(blackboard_value_to_minijinja).collect::<Vec<_>>())
        }
        BlackboardValue::Map(m) => {
            let mut map = HashMap::new();
            for (k, v) in m {
                map.insert(k.clone(), blackboard_value_to_minijinja(v));
            }
            Value::from(map)
        }
        BlackboardValue::Command(_) => Value::from("<command>"),
        BlackboardValue::Null => Value::from(()),
    }
}

// ── BlackboardExt trait ─────────────────────────────────────────────────────

pub trait BlackboardExt {
    fn to_template_context(&self) -> Value;
}

impl BlackboardExt for Blackboard {
    fn to_template_context(&self) -> Value {
        let mut ctx = HashMap::new();

        // --- Work Agent context ---
        ctx.insert(
            "task_description".into(),
            Value::from(self.task_description.as_str()),
        );
        ctx.insert(
            "provider_output".into(),
            Value::from(self.provider_output.as_str()),
        );
        ctx.insert(
            "context_summary".into(),
            Value::from(self.context_summary.as_str()),
        );

        // last_tool_call as nested object
        if let Some(ref tool) = self.last_tool_call {
            let mut tool_map: HashMap<&str, Value> = HashMap::new();
            tool_map.insert("name", Value::from(tool.name.as_str()));
            tool_map.insert("input", Value::from(tool.input.as_str()));
            tool_map.insert("output", Value::from(tool.output.as_str()));
            ctx.insert("last_tool_call".into(), Value::from(tool_map));
        } else {
            ctx.insert("last_tool_call".into(), Value::from(()));
        }

        // file_changes as list of objects
        let file_changes: Vec<Value> = self
            .file_changes
            .iter()
            .map(|fc| {
                let mut map: HashMap<&str, Value> = HashMap::new();
                map.insert("path", Value::from(fc.path.as_str()));
                map.insert("change_type", Value::from(fc.change_type.as_str()));
                Value::from(map)
            })
            .collect();
        ctx.insert("file_changes".into(), Value::from(file_changes));

        // --- Reflection tracking ---
        ctx.insert("reflection_round".into(), Value::from(self.reflection_round as i64));
        ctx.insert(
            "max_reflection_rounds".into(),
            Value::from(self.max_reflection_rounds as i64),
        );
        ctx.insert(
            "confidence_accumulator".into(),
            Value::from(self.confidence_accumulator),
        );

        // --- Agent identification ---
        ctx.insert("agent_id".into(), Value::from(self.agent_id.as_str()));
        ctx.insert("work_agent_id".into(), Value::from(self.work_agent_id.as_str()));
        ctx.insert(
            "current_task_id".into(),
            Value::from(self.current_task_id.as_str()),
        );
        ctx.insert(
            "current_story_id".into(),
            Value::from(self.current_story_id.as_str()),
        );

        // --- Sprint tracking ---
        ctx.insert("current_sprint".into(), Value::from(self.current_sprint as i64));
        ctx.insert("total_sprints".into(), Value::from(self.total_sprints as i64));

        // Current sprint goal
        let sprint_goal = self.current_sprint_goal()
            .map(|s| s.to_string())
            .unwrap_or_default();
        ctx.insert("sprint_goal".into(), Value::from(sprint_goal.as_str()));

        // Sprint goals list
        let sprint_goals: Vec<Value> = self
            .sprint_goals
            .iter()
            .map(|sg| {
                let mut map: HashMap<&str, Value> = HashMap::new();
                map.insert("sprint_number", Value::from(sg.sprint_number as i64));
                map.insert("description", Value::from(sg.description.as_str()));
                Value::from(map)
            })
            .collect();
        ctx.insert("sprint_goals".into(), Value::from(sprint_goals));

        // --- Decision flow tracking ---
        // Reflection chain
        let reflection_chain: Vec<Value> = self
            .reflection_chain
            .iter()
            .map(|r| {
                let mut map: HashMap<&str, Value> = HashMap::new();
                map.insert("sprint", Value::from(r.sprint as i64));
                map.insert("result", Value::from(r.result.as_str()));
                map.insert("reasoning", Value::from(r.reasoning.as_str()));
                Value::from(map)
            })
            .collect();
        ctx.insert("reflection_chain".into(), Value::from(reflection_chain));

        // Decision chain
        let decision_chain: Vec<Value> = self
            .decision_chain
            .iter()
            .map(|d| {
                let mut map: HashMap<&str, Value> = HashMap::new();
                map.insert("node_name", Value::from(d.node_name.as_str()));
                map.insert("decision", Value::from(d.decision.as_str()));
                map.insert("outcome", Value::from(d.outcome.as_str()));
                Value::from(map)
            })
            .collect();
        ctx.insert("decision_chain".into(), Value::from(decision_chain));

        // --- LLM responses ---
        let mut llm_map = HashMap::new();
        for (k, v) in &self.llm_responses {
            llm_map.insert(k.clone(), Value::from(v.as_str()));
        }
        ctx.insert("llm_responses".into(), Value::from(llm_map));

        // --- Scoped variables ---
        for (k, v) in self.iter_variables() {
            ctx.insert(k.clone(), blackboard_value_to_minijinja(v));
        }

        Value::from(ctx)
    }
}

// ── Template rendering ──────────────────────────────────────────────────────

pub fn render_prompt_template(template_str: &str, context: &Value) -> Result<String, RuntimeError> {
    let env = get_template_env();
    let template = env
        .template_from_str(template_str)
        .map_err(|e| {
            // Include template snippet for easier debugging
            let snippet = if template_str.len() > 100 {
                format!("{}...", &template_str[..100])
            } else {
                template_str.to_string()
            };
            RuntimeError::FilterError(format!("template syntax error in '{}': {}", snippet, e))
        })?;
    template
        .render(context)
        .map_err(|e| RuntimeError::FilterError(format!("template render error: {}", e)))
}

// ── Command template rendering ──────────────────────────────────────────────

pub fn render_command_templates(
    cmd: &DecisionCommand,
    bb: &Blackboard,
) -> Result<DecisionCommand, RuntimeError> {
    let ctx = bb.to_template_context();
    let render = |s: &str| -> Result<String, RuntimeError> {
        render_prompt_template(s, &ctx)
    };

    match cmd {
        DecisionCommand::Agent(ac) => match ac {
            AgentCommand::ApproveAndContinue => Ok(cmd.clone()),
            AgentCommand::Reflect { prompt } => Ok(DecisionCommand::Agent(
                AgentCommand::Reflect {
                    prompt: render(prompt)?,
                },
            )),
            AgentCommand::SendInstruction { prompt, target_agent } => {
                Ok(DecisionCommand::Agent(AgentCommand::SendInstruction {
                    prompt: render(prompt)?,
                    target_agent: render(target_agent)?,
                }))
            }
            AgentCommand::Terminate { reason } => Ok(DecisionCommand::Agent(
                AgentCommand::Terminate {
                    reason: render(reason)?,
                },
            )),
            AgentCommand::WakeUp => Ok(cmd.clone()),
        },
        DecisionCommand::Git(gc, extra) => {
            let extra = match extra {
                Some(s) => Some(render(s)?),
                None => None,
            };
            match gc {
                GitCommand::Commit { message, wip } => Ok(DecisionCommand::Git(
                    GitCommand::Commit {
                        message: render(message)?,
                        wip: *wip,
                    },
                    extra,
                )),
                GitCommand::Stash {
                    description,
                    include_untracked,
                } => Ok(DecisionCommand::Git(
                    GitCommand::Stash {
                        description: render(description)?,
                        include_untracked: *include_untracked,
                    },
                    extra,
                )),
                GitCommand::Discard => Ok(DecisionCommand::Git(GitCommand::Discard, extra)),
                GitCommand::CreateBranch { name, base } => Ok(DecisionCommand::Git(
                    GitCommand::CreateBranch {
                        name: render(name)?,
                        base: render(base)?,
                    },
                    extra,
                )),
                GitCommand::Rebase { base } => Ok(DecisionCommand::Git(
                    GitCommand::Rebase {
                        base: render(base)?,
                    },
                    extra,
                )),
            }
        }
        DecisionCommand::Task(tc) => match tc {
            TaskCommand::ConfirmCompletion => Ok(cmd.clone()),
            TaskCommand::StopIfComplete { reason } => Ok(DecisionCommand::Task(
                TaskCommand::StopIfComplete {
                    reason: render(reason)?,
                },
            )),
            TaskCommand::PrepareStart {
                task_id,
                description,
            } => Ok(DecisionCommand::Task(TaskCommand::PrepareStart {
                task_id: render(task_id)?,
                description: render(description)?,
            })),
        },
        DecisionCommand::Human(hc) => match hc {
            HumanCommand::Escalate { reason, context } => {
                let context = match context {
                    Some(s) => Some(render(s)?),
                    None => None,
                };
                Ok(DecisionCommand::Human(HumanCommand::Escalate {
                    reason: render(reason)?,
                    context,
                }))
            }
            HumanCommand::SelectOption { option_id } => Ok(DecisionCommand::Human(
                HumanCommand::SelectOption {
                    option_id: render(option_id)?,
                },
            )),
            HumanCommand::SkipDecision => Ok(cmd.clone()),
        },
        DecisionCommand::Provider(pc) => match pc {
            ProviderCommand::RetryTool {
                tool_name,
                args,
                max_attempts,
            } => {
                let args = match args {
                    Some(s) => Some(render(s)?),
                    None => None,
                };
                Ok(DecisionCommand::Provider(ProviderCommand::RetryTool {
                    tool_name: render(tool_name)?,
                    args,
                    max_attempts: *max_attempts,
                }))
            }
            ProviderCommand::SwitchProvider { provider_type } => {
                Ok(DecisionCommand::Provider(ProviderCommand::SwitchProvider {
                    provider_type: render(provider_type)?,
                }))
            }
            ProviderCommand::SuggestCommit {
                message,
                mandatory,
                reason,
            } => Ok(DecisionCommand::Provider(ProviderCommand::SuggestCommit {
                message: render(message)?,
                mandatory: *mandatory,
                reason: render(reason)?,
            })),
            ProviderCommand::PreparePr {
                title,
                description,
                base,
                draft,
            } => Ok(DecisionCommand::Provider(ProviderCommand::PreparePr {
                title: render(title)?,
                description: render(description)?,
                base: render(base)?,
                draft: *draft,
            })),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ext::blackboard::{SprintGoal, ReflectionEntry, DecisionEntry};

    #[test]
    fn template_context_includes_sprint_fields() {
        let mut bb = Blackboard::new();
        bb.current_sprint = 2;
        bb.total_sprints = 4;
        bb.sprint_goals = vec![
            SprintGoal::new(1, "first goal"),
            SprintGoal::new(2, "second goal"),
        ];

        let ctx = bb.to_template_context();

        // Check sprint fields are accessible
        let rendered = render_prompt_template(
            "Sprint {{ current_sprint }} of {{ total_sprints }}",
            &ctx,
        ).unwrap();
        assert_eq!(rendered, "Sprint 2 of 4");

        let rendered_goal = render_prompt_template(
            "Goal: {{ sprint_goal }}",
            &ctx,
        ).unwrap();
        assert_eq!(rendered_goal, "Goal: second goal");
    }

    #[test]
    fn template_context_includes_work_agent_id() {
        let mut bb = Blackboard::new();
        bb.work_agent_id = "agent-123".to_string();

        let ctx = bb.to_template_context();

        let rendered = render_prompt_template(
            "Target: {{ work_agent_id }}",
            &ctx,
        ).unwrap();
        assert_eq!(rendered, "Target: agent-123");
    }

    #[test]
    fn template_context_includes_reflection_chain() {
        let mut bb = Blackboard::new();
        bb.push_reflection(ReflectionEntry::new(1, "proceed", "done"));
        bb.push_reflection(ReflectionEntry::new(2, "retry", "incomplete"));

        let ctx = bb.to_template_context();

        // Access reflection chain
        let rendered = render_prompt_template(
            "{% for r in reflection_chain %}{{ r.sprint }}: {{ r.result }}{% endfor %}",
            &ctx,
        ).unwrap();
        assert_eq!(rendered, "1: proceed2: retry");
    }

    #[test]
    fn template_context_includes_decision_chain() {
        let mut bb = Blackboard::new();
        bb.push_decision(DecisionEntry::new("node1", "proceed", "success"));
        bb.push_decision(DecisionEntry::new("node2", "retry", "running"));

        let ctx = bb.to_template_context();

        let rendered = render_prompt_template(
            "{% for d in decision_chain %}{{ d.node_name }}={{ d.decision }}{% endfor %}",
            &ctx,
        ).unwrap();
        assert_eq!(rendered, "node1=proceednode2=retry");
    }

    #[test]
    fn template_context_includes_sprint_goals_list() {
        let mut bb = Blackboard::new();
        bb.sprint_goals = vec![
            SprintGoal::new(1, "auth"),
            SprintGoal::new(2, "tests"),
        ];

        let ctx = bb.to_template_context();

        let rendered = render_prompt_template(
            "{% for sg in sprint_goals %}{{ sg.sprint_number }}: {{ sg.description }}{% endfor %}",
            &ctx,
        ).unwrap();
        assert_eq!(rendered, "1: auth2: tests");
    }
}
