# Decision DSL: Template Engine

> Template rendering specification for the decision DSL engine. The engine uses the **`minijinja`** crate for Jinja2-compatible template rendering. All Prompt node templates and Action command field interpolations go through the same minijinja environment.

---

## 1. Why minijinja

The previous design proposed a hand-rolled 300-line template engine. This was a stub that never implemented full Jinja2 features. Replaced with `minijinja` because:

| Aspect | Hand-rolled | minijinja |
|--------|------------|-----------|
| Feature completeness | `{{ var }}`, `{% if %}`, basic filters | Full Jinja2 syntax |
| Maintenance | We own the bugs | Upstream maintained |
| `{% for %}` loops | Not implemented | ✅ |
| Whitespace control (`{%- -%}`) | Not implemented | ✅ |
| Auto-escaping | Not implemented | Configurable |
| Macro support | Not implemented | ✅ |
| Performance | Untuned | Production-tested |
| Test coverage | Minimal | Upstream + ours |

`minijinja` adds one dependency (`minijinja = "2"`) and removes ~500 lines of hand-rolled engine code.

---

## 2. Blackboard as Template Context

The Blackboard's built-in fields and scoped variables are exposed as a minijinja `Value`.
See [`decision-dsl-ast.md`](decision-dsl-ast.md) §2.2 for the full `to_template_context()` implementation.

The conversion logic (`blackboard_value_to_minijinja`) maps `BlackboardValue` variants to minijinja `Value`:

```rust
fn blackboard_value_to_minijinja(v: &BlackboardValue) -> minijinja::Value {
    use minijinja::Value;
    match v {
        BlackboardValue::String(s) => Value::from_serialize(s),
        BlackboardValue::Integer(i) => Value::from(*i),
        BlackboardValue::Float(f) => Value::from(*f),
        BlackboardValue::Boolean(b) => Value::from(*b),
        BlackboardValue::List(l) => {
            Value::from_serialize(l.iter().map(blackboard_value_to_minijinja).collect::<Vec<_>>())
        }
        BlackboardValue::Map(m) => {
            let map: HashMap<String, Value> = m.iter()
                .map(|(k, v)| (k.clone(), blackboard_value_to_minijinja(v)))
                .collect();
            Value::from_serialize(map)
        }
    }
}
```

---

## 3. Template Environment Setup

```rust
/// Create the template environment with custom filters.
pub(crate) fn create_template_env() -> Environment<'static> {
    let mut env = Environment::new();

    // Register custom filters
    env.add_filter("slugify", |value: String| {
        value.to_lowercase()
            .chars()
            .map(|c| if c.is_alphanumeric() { c } else { '-' })
            .collect::<String>()
    });

    env.add_filter("truncate", |value: String, n: usize| {
        if value.len() > n {
            format!("{}...", &value[..n])
        } else {
            value
        }
    });

    env
}
```

---

## 4. Rendering Prompt Templates

```rust
use minijinja::Environment;
use std::sync::OnceLock;

static TEMPLATE_ENV: OnceLock<Environment<'static>> = OnceLock::new();

fn get_template_env() -> &'static Environment<'static> {
    TEMPLATE_ENV.get_or_init(|| create_template_env())
}

pub(crate) fn render_prompt_template(
    template_str: &str,
    ctx: &minijinja::Value,
) -> Result<String, RuntimeError> {
    let env = get_template_env();
    let tmpl = env
        .template_from_str(template_str)
        .map_err(|e| RuntimeError::FilterError(e.to_string()))?;
    tmpl.render(ctx)
        .map_err(|e| RuntimeError::FilterError(e.to_string()))
}
```

---

## 5. Rendering Command Field Templates

String fields in `DecisionCommand` variants are rendered as inline templates:

```rust
pub(crate) fn render_command_templates(
    cmd: &DecisionCommand,
    bb: &Blackboard,
) -> Result<DecisionCommand, RuntimeError> {
    let ctx = bb.to_template_context();
    let env = get_template_env();

    // Helper: render a string template against the context
    let render = |s: &str| -> Result<String, RuntimeError> {
        let tmpl = env
            .template_from_str(s)
            .map_err(|e| RuntimeError::FilterError(e.to_string()))?;
        tmpl.render(&ctx)
            .map_err(|e| RuntimeError::FilterError(e.to_string()))
    };

    match cmd {
        // --- Agent commands ---
        DecisionCommand::Agent(AgentCommand::Reflect { prompt }) => {
            Ok(DecisionCommand::Agent(AgentCommand::Reflect { prompt: render(prompt)? }))
        }
        DecisionCommand::Agent(AgentCommand::SendInstruction { prompt, target_agent }) => {
            Ok(DecisionCommand::Agent(AgentCommand::SendInstruction {
                prompt: render(prompt)?,
                target_agent: render(target_agent)?,
            }))
        }
        DecisionCommand::Agent(AgentCommand::Terminate { reason }) => {
            Ok(DecisionCommand::Agent(AgentCommand::Terminate {
                reason: render(reason)?,
            }))
        }
        DecisionCommand::Agent(AgentCommand::ApproveAndContinue) |
        DecisionCommand::Agent(AgentCommand::WakeUp) => Ok(cmd.clone()),

        // --- Git commands ---
        DecisionCommand::Git(GitCommand::Commit { message, wip }, wt) => {
            Ok(DecisionCommand::Git(GitCommand::Commit {
                message: render(message)?,
                wip: *wip,
            }, wt.clone()))
        }
        DecisionCommand::Git(GitCommand::CreateBranch { name, base }, wt) => {
            Ok(DecisionCommand::Git(GitCommand::CreateBranch {
                name: render(name)?,
                base: render(base)?,
            }, wt.clone()))
        }
        DecisionCommand::Git(GitCommand::Stash { description, include_untracked }, wt) => {
            Ok(DecisionCommand::Git(GitCommand::Stash {
                description: render(description)?,
                include_untracked: *include_untracked,
            }, wt.clone()))
        }
        DecisionCommand::Git(GitCommand::Discard, wt) => {
            Ok(DecisionCommand::Git(GitCommand::Discard, wt.clone()))
        }
        DecisionCommand::Git(GitCommand::Rebase { base }, wt) => {
            Ok(DecisionCommand::Git(GitCommand::Rebase {
                base: render(base)?,
            }, wt.clone()))
        }

        // --- Task commands ---
        DecisionCommand::Task(TaskCommand::StopIfComplete { reason }) => {
            Ok(DecisionCommand::Task(TaskCommand::StopIfComplete {
                reason: render(reason)?,
            }))
        }
        DecisionCommand::Task(TaskCommand::PrepareStart { task_id, description }) => {
            Ok(DecisionCommand::Task(TaskCommand::PrepareStart {
                task_id: render(task_id)?,
                description: render(description)?,
            }))
        }
        DecisionCommand::Task(TaskCommand::ConfirmCompletion) => Ok(cmd.clone()),

        // --- Human commands ---
        DecisionCommand::Human(HumanCommand::Escalate { reason, context }) => {
            Ok(DecisionCommand::Human(HumanCommand::Escalate {
                reason: render(reason)?,
                context: context.as_ref().map(|c| render(c)).transpose()?,
            }))
        }
        DecisionCommand::Human(HumanCommand::SelectOption { option_id }) => {
            Ok(DecisionCommand::Human(HumanCommand::SelectOption {
                option_id: render(option_id)?,
            }))
        }
        DecisionCommand::Human(HumanCommand::SkipDecision) => Ok(cmd.clone()),

        // --- Provider commands ---
        DecisionCommand::Provider(ProviderCommand::RetryTool { tool_name, args, max_attempts }) => {
            Ok(DecisionCommand::Provider(ProviderCommand::RetryTool {
                tool_name: render(tool_name)?,
                args: args.as_ref().map(|a| render(a)).transpose()?,
                max_attempts: *max_attempts,
            }))
        }
        DecisionCommand::Provider(ProviderCommand::SwitchProvider { provider_type }) => {
            Ok(DecisionCommand::Provider(ProviderCommand::SwitchProvider {
                provider_type: render(provider_type)?,
            }))
        }
        DecisionCommand::Provider(ProviderCommand::SuggestCommit { message, mandatory, reason }) => {
            Ok(DecisionCommand::Provider(ProviderCommand::SuggestCommit {
                message: render(message)?,
                mandatory: *mandatory,
                reason: render(reason)?,
            }))
        }
        DecisionCommand::Provider(ProviderCommand::PreparePr { title, description, base, draft }) => {
            Ok(DecisionCommand::Provider(ProviderCommand::PreparePr {
                title: render(title)?,
                description: render(description)?,
                base: render(base)?,
                draft: *draft,
            }))
        }
    }
}
```

---

## 6. Supported Template Syntax

All standard minijinja features are available:

### Variable Interpolation

```text
{{ task_description }}
{{ provider_output }}
{{ reflection_round }}
{{ last_tool_call.name }}
{{ confidence_accumulator }}
```

### Filters

```text
{{ provider_output | truncate(500) }}
{{ file_changes | length }}
{{ task_description | upper }}
{{ context_summary | default("No summary available") }}
{{ current_task_id | slugify }}
```

Custom filters added by the DSL engine:

| Filter | Description |
|--------|-------------|
| `truncate(n)` | Truncate to N characters, append "...". Default n=100. |
| `slugify` | Convert to lowercase, replace non-alphanumeric with `-`. |

Standard minijinja filters (`upper`, `lower`, `length`, `default`, `join`, `replace`, `trim`, `indent`, etc.) are all available. See the [minijinja filter reference](https://docs.rs/minijinja/latest/minijinja/filters/index.html) for the full catalog.

### Conditionals

```text
{% if reflection_round > 0 %}
  This is reflection round {{ reflection_round }}.
{% elif confidence_accumulator > 0.9 %}
  High confidence completion.
{% else %}
  Initial decision.
{% endif %}
```

### Loops

```text
{% if file_changes | length > 0 %}
  Recent changes:
  {% for change in file_changes %}
    - {{ change.path }} ({{ change.change_type }})
  {% endfor %}
{% endif %}
```

### Whitespace Control

```text
{%- if true -%}
  no whitespace around this line
{%- endif -%}
```

---

## 7. Template Validation at Load Time

Templates are validated when the DSL is loaded:

```rust
impl DslLoader {
    fn validate_templates(&self, tree: &Tree) -> Result<(), ParseError> {
        tree.spec.root.validate_templates_recursive(&TEMPLATE_ENV)
    }
}

impl Node {
    fn validate_templates_recursive(&self, env: &Environment) -> Result<(), ParseError> {
        match self {
            Node::Prompt(n) => {
                // Try to compile the template; report syntax errors at load time
                env.template_from_str(&n.template)
                    .map_err(|e| ParseError::InvalidProperty {
                        key: "template".into(),
                        value: format!("<template in node {}>", n.name),
                        reason: e.to_string(),
                    })?;
            }
            Node::Action(n) => {
                // Validate command field templates
                validate_command_templates(&n.command, env)?;
            }
            _ => {
                for child in self.children() {
                    child.validate_templates_recursive(env)?;
                }
            }
        }
        Ok(())
    }
}
```

Unknown variables in templates produce **warnings** at load time (not errors), since variables like `{{ last_tool_call.name }}` may not exist until runtime.

---

## 8. Testing

```rust
#[test]
fn render_truncate_filter() {
    let mut bb = Blackboard::new();
    bb.provider_output = "a".repeat(1000);

    let result = render_prompt_template(
        "{{ provider_output | truncate(100) }}", &bb,
    ).unwrap();
    assert_eq!(result.len(), 103); // 100 chars + "..."
    assert!(result.ends_with("..."));
}

#[test]
fn render_conditional() {
    let mut bb = Blackboard::new();
    bb.reflection_round = 1;

    let result = render_prompt_template(
        "{% if reflection_round > 0 %}Round {{ reflection_round }}{% else %}Initial{% endif %}",
        &bb,
    ).unwrap();
    assert_eq!(result.trim(), "Round 1");
}

#[test]
fn render_for_loop() {
    let mut bb = Blackboard::new();
    bb.file_changes = vec![
        FileChangeRecord { path: "a.rs".into(), change_type: "modified".into() },
        FileChangeRecord { path: "b.rs".into(), change_type: "added".into() },
    ];

    let result = render_prompt_template(
        "{% for c in file_changes %}{{ c.path }}:{{ c.change_type }};{% endfor %}",
        &bb,
    ).unwrap();
    assert_eq!(result, "a.rs:modified;b.rs:added;");
}

#[test]
fn render_missing_variable_uses_default() {
    let bb = Blackboard::new();
    let result = render_prompt_template(
        "{{ nonexistent | default(\"fallback\") }}", &bb,
    ).unwrap();
    assert_eq!(result, "fallback");
}

#[test]
fn render_command_interpolation() {
    let mut bb = Blackboard::new();
    bb.current_task_id = "TASK-42".into();

    let cmd = DecisionCommand::Task(TaskCommand::PrepareStart {
        task_id: "{{ current_task_id }}".into(),
        description: "Implement {{ current_task_id }}".into(),
    });

    let rendered = render_command_templates(&cmd, &bb).unwrap();
    if let DecisionCommand::Task(TaskCommand::PrepareStart { task_id, description }) = rendered {
        assert_eq!(task_id, "TASK-42");
        assert_eq!(description, "Implement TASK-42");
    } else {
        panic!("wrong variant");
    }
}
```

---

*Document version: 2.0*
*Last updated: 2026-04-24*
