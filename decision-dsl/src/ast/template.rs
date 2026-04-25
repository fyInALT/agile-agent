use std::collections::HashMap;
use std::sync::OnceLock;

use minijinja::{Environment, UndefinedBehavior, Value};

use crate::ext::blackboard::{Blackboard, BlackboardValue};
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
    }
}

// ── BlackboardExt trait ─────────────────────────────────────────────────────

pub trait BlackboardExt {
    fn to_template_context(&self) -> Value;
}

impl BlackboardExt for Blackboard {
    fn to_template_context(&self) -> Value {
        let mut ctx = HashMap::new();

        // Built-in fields
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
        ctx.insert("reflection_round".into(), Value::from(self.reflection_round as i64));
        ctx.insert(
            "max_reflection_rounds".into(),
            Value::from(self.max_reflection_rounds as i64),
        );
        ctx.insert(
            "confidence_accumulator".into(),
            Value::from(self.confidence_accumulator),
        );
        ctx.insert("agent_id".into(), Value::from(self.agent_id.as_str()));
        ctx.insert(
            "current_task_id".into(),
            Value::from(self.current_task_id.as_str()),
        );
        ctx.insert(
            "current_story_id".into(),
            Value::from(self.current_story_id.as_str()),
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

        // llm_responses as map
        let mut llm_map = HashMap::new();
        for (k, v) in &self.llm_responses {
            llm_map.insert(k.clone(), Value::from(v.as_str()));
        }
        ctx.insert("llm_responses".into(), Value::from(llm_map));

        // Scoped variables
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
        .map_err(|e| RuntimeError::FilterError(format!("template syntax error: {e}")))?;
    template
        .render(context)
        .map_err(|e| RuntimeError::FilterError(e.to_string()))
}
