# Decision DSL: Template Engine

> Template engine specification for the decision DSL engine. Covers Jinja2-style template rendering for Prompt nodes and Action command fields, including the full filter registry.
>
> This document is a chapter of the [Decision DSL Implementation](decision-dsl-implementation.md).

## Template Engine

The template engine supports Jinja2-style syntax with the Blackboard as context.

###1 Supported Syntax

| Feature | Syntax | Status |
|---------|--------|--------|
| Variable interpolation | `{{ variable }}` | ✅ V1 |
| Dot notation | `{{ last_tool_call.name }}` | ✅ V1 |
| Filters | `{{ var \| filter }}` | ✅ V1 |
| Conditionals | `{% if %}`, `{% else %}`, `{% endif %}` | ✅ V1 |
| Loops | `{% for item in list %}` | ✅ V1 |
| Whitespace control | `{%- -%}` | ✅ V1 |
| Comments | `{# comment #}` | ✅ V1 |

###2 Engine Design

The engine is a two-pass system: **lexer** → **render**.

```rust
pub(crate) struct TemplateEngine;

impl TemplateEngine {
    pub fn render(template: &str, bb: &Blackboard) -> Result<String, RuntimeError> {
        let tokens = Self::lex(template)?;
        Self::render_tokens(&tokens, bb)
    }

    fn lex(template: &str) -> Result<Vec<Token>, RuntimeError> {
        let mut tokens = Vec::new();
        let mut chars = template.char_indices().peekable();

        while let Some((i, c)) = chars.next() {
            if c == '{' && chars.peek().map(|(_, n)| *n) == Some('{') {
                chars.next(); // consume second '{'
                // Parse expression until }}
                let (expr, filters) = Self::parse_expression(&mut chars)?;
                tokens.push(Token::Expr { expr, filters });
            } else if c == '{' && chars.peek().map(|(_, n)| *n) == Some('%') {
                chars.next(); // consume '%'
                // Parse statement until %}
                let stmt = Self::parse_statement(&mut chars)?;
                tokens.push(Token::Statement(stmt));
            } else if c == '{' && chars.peek().map(|(_, n)| *n) == Some('#') {
                chars.next(); // consume '#'
                // Skip comment until #}
                Self::skip_comment(&mut chars)?;
            } else {
                // Collect literal text
                let start = i;
                let mut end = i + c.len_utf8();
                while let Some((j, ch)) = chars.peek() {
                    if *ch == '{' || *ch == '%' || *ch == '#' {
                        break;
                    }
                    end = *j + ch.len_utf8();
                    chars.next();
                }
                tokens.push(Token::Literal(template[start..end].to_string()));
            }
        }

        Ok(tokens)
    }
}
```

###3 Token Types

```rust
enum Token {
    Literal(String),
    Expr { expr: String, filters: Vec<FilterCall> },
    Statement(Stmt),
}

struct FilterCall {
    name: String,
    args: Vec<String>,
}

enum Stmt {
    If { condition: String, then_body: Vec<Token>, else_body: Option<Vec<Token>> },
    For { var: String, iter: String, body: Vec<Token> },
}
```

###4 Filter Registry

```rust
pub(crate) struct FilterRegistry {
    filters: HashMap<String, FilterFn>,
}

type FilterFn = fn(value: &BlackboardValue, args: &[String]) -> Result<BlackboardValue, RuntimeError>;

impl FilterRegistry {
    pub fn with_builtins() -> Self {
        let mut reg = Self { filters: HashMap::new() };
        reg.register("upper", |v, _| Ok(BlackboardValue::String(v.to_string().to_uppercase())));
        reg.register("lower", |v, _| Ok(BlackboardValue::String(v.to_string().to_lowercase())));
        reg.register("truncate", |v, args| {
            let n: usize = args.get(0).and_then(|s| s.parse().ok()).unwrap_or(100);
            let s = v.to_string();
            if s.len() > n {
                Ok(BlackboardValue::String(format!("{}...", &s[..n])))
            } else {
                Ok(BlackboardValue::String(s))
            }
        });
        reg.register("length", |v, _| {
            let len = match v {
                BlackboardValue::String(s) => s.len(),
                BlackboardValue::List(l) => l.len(),
                BlackboardValue::Map(m) => m.len(),
                _ => 0,
            };
            Ok(BlackboardValue::Integer(len as i64))
        });
        reg.register("default", |v, args| {
            match v {
                BlackboardValue::String(s) if s.is_empty() => {
                    Ok(BlackboardValue::String(args.get(0).cloned().unwrap_or_default()))
                }
                _ => Ok(v.clone()),
            }
        });
        reg.register("join", |v, args| {
            match v {
                BlackboardValue::List(l) => {
                    let sep = args.get(0).cloned().unwrap_or(", ".to_string());
                    let joined = l.iter().map(|item| item.to_string()).collect::<Vec<_>>().join(&sep);
                    Ok(BlackboardValue::String(joined))
                }
                _ => Ok(BlackboardValue::String(v.to_string())),
            }
        });
        reg.register("json", |v, _| {
            serde_json::to_string(v).map(BlackboardValue::String)
                .map_err(|e| RuntimeError::FilterError(e.to_string()))
        });
        reg.register("slugify", |v, _| {
            let s = v.to_string().to_lowercase()
                .replace(" ", "-")
                .replace("_", "-")
                .replace(|c: char| !c.is_alphanumeric() && c != '-', "");
            Ok(BlackboardValue::String(s))
        });
        reg
    }

    pub fn register(&mut self, name: &str, f: FilterFn) {
        self.filters.insert(name.to_string(), f);
    }
}
```

###5 Expression Evaluation

```rust
impl TemplateEngine {
    fn eval_expr(expr: &str, bb: &Blackboard) -> Result<BlackboardValue, RuntimeError> {
        let trimmed = expr.trim();
        // Simple path lookup: "task_description", "variables.next_action", "last_tool_call.name"
        bb.get_path(trimmed)
            .ok_or_else(|| RuntimeError::MissingVariable { key: trimmed.to_string() })
    }

    fn apply_filters(
        value: BlackboardValue,
        filters: &[FilterCall],
        registry: &FilterRegistry,
    ) -> Result<BlackboardValue, RuntimeError> {
        let mut current = value;
        for filter in filters {
            let f = registry.filters.get(&filter.name)
                .ok_or_else(|| RuntimeError::UnknownFilter { filter: filter.name.clone() })?;
            current = f(&current, &filter.args)?;
        }
        Ok(current)
    }
}
```

###6 Render Implementation

```rust
impl TemplateEngine {
    fn render_tokens(tokens: &[Token], bb: &Blackboard) -> Result<String, RuntimeError> {
        let mut output = String::new();
        let mut i = 0;
        while i < tokens.len() {
            match &tokens[i] {
                Token::Literal(text) => output.push_str(text),
                Token::Expr { expr, filters } => {
                    let value = Self::eval_expr(expr, bb)?;
                    let value = Self::apply_filters(value, filters, &FilterRegistry::with_builtins())?;
                    output.push_str(&value.to_string());
                }
                Token::Statement(Stmt::If { condition, then_body, else_body }) => {
                    let cond_value = Self::eval_condition(condition, bb)?;
                    let body = if cond_value { then_body } else { else_body.as_ref().unwrap_or(&vec![]) };
                    output.push_str(&Self::render_tokens(body, bb)?);
                }
                Token::Statement(Stmt::For { var, iter, body }) => {
                    let iter_value = Self::eval_expr(iter, bb)?;
                    match iter_value {
                        BlackboardValue::List(list) => {
                            for item in list {
                                // Create a temporary scope with the loop variable
                                let mut scoped_bb = bb.clone();
                                scoped_bb.set(var, item);
                                output.push_str(&Self::render_tokens(body, &scoped_bb)?);
                            }
                        }
                        _ => return Err(RuntimeError::FilterError(
                            format!("cannot iterate over {}", iter)
                        )),
                    }
                }
            }
            i += 1;
        }
        Ok(output)
    }

    fn eval_condition(condition: &str, bb: &Blackboard) -> Result<bool, RuntimeError> {
        // Simple condition evaluation: "reflection_round > 0", "file_changes | length > 0"
        // V1: Support simple comparisons and boolean variable lookups
        let trimmed = condition.trim();
        if let Ok(val) = Self::eval_expr(trimmed, bb) {
            return Ok(match val {
                BlackboardValue::Boolean(b) => b,
                BlackboardValue::Integer(0) | BlackboardValue::Float(0.0) => false,
                BlackboardValue::String(s) if s.is_empty() => false,
                _ => true,
            });
        }
        // Parse comparison: "left op right"
        Self::eval_comparison(trimmed, bb)
    }
}
```

---

## Action Command Template Rendering

Action nodes support template interpolation inside command fields. This is a second render pass that runs during Action tick.

###1 Command Rendering

```rust
/// Recursively render all String fields in a Command using the Blackboard.
fn render_command_templates(cmd: &Command, bb: &Blackboard) -> Result<Command, RuntimeError> {
    match cmd {
        Command::RetryTool { tool_name, args, max_attempts } => Ok(Command::RetryTool {
            tool_name: TemplateEngine::render(tool_name, bb)?,
            args: args.as_ref().map(|a| TemplateEngine::render(a, bb)).transpose()?,
            max_attempts: *max_attempts,
        }),
        Command::SendCustomInstruction { prompt, target_agent } => Ok(Command::SendCustomInstruction {
            prompt: TemplateEngine::render(prompt, bb)?,
            target_agent: TemplateEngine::render(target_agent, bb)?,
        }),
        Command::EscalateToHuman { reason, context } => Ok(Command::EscalateToHuman {
            reason: TemplateEngine::render(reason, bb)?,
            context: context.as_ref().map(|c| TemplateEngine::render(c, bb)).transpose()?,
        }),
        Command::Reflect { prompt } => Ok(Command::Reflect {
            prompt: TemplateEngine::render(prompt, bb)?,
        }),
        Command::StopIfComplete { reason } => Ok(Command::StopIfComplete {
            reason: TemplateEngine::render(reason, bb)?,
        }),
        Command::PrepareTaskStart { task_id, task_description } => Ok(Command::PrepareTaskStart {
            task_id: TemplateEngine::render(task_id, bb)?,
            task_description: TemplateEngine::render(task_description, bb)?,
        }),
        Command::SuggestCommit { message, mandatory, reason } => Ok(Command::SuggestCommit {
            message: TemplateEngine::render(message, bb)?,
            mandatory: *mandatory,
            reason: TemplateEngine::render(reason, bb)?,
        }),
        Command::CommitChanges { message, is_wip, worktree_path } => Ok(Command::CommitChanges {
            message: TemplateEngine::render(message, bb)?,
            is_wip: *is_wip,
            worktree_path: worktree_path.as_ref().map(|p| TemplateEngine::render(p, bb)).transpose()?,
        }),
        Command::CreateTaskBranch { branch_name, base_branch, worktree_path } => {
            Ok(Command::CreateTaskBranch {
                branch_name: TemplateEngine::render(branch_name, bb)?,
                base_branch: TemplateEngine::render(base_branch, bb)?,
                worktree_path: worktree_path.as_ref().map(|p| TemplateEngine::render(p, bb)).transpose()?,
            })
        }
        Command::RebaseToMain { base_branch } => Ok(Command::RebaseToMain {
            base_branch: TemplateEngine::render(base_branch, bb)?,
        }),
        // Commands with no string fields pass through unchanged
        other => Ok(other.clone()),
    }
}
```

---

