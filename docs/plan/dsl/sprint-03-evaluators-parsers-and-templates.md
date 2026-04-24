# Sprint 3: Evaluators, Parsers & Templates

## Metadata

- Sprint ID: `dsl-sprint-03`
- Title: `Evaluators, Parsers & Templates`
- Duration: 2 weeks
- Priority: P0 (Critical)
- Status: `Backlog`
- Created: 2026-04-20

## Sprint Goal

Implement the expression evaluation engine, output parsers, and template rendering system. Evaluators drive Condition nodes; parsers drive Prompt nodes; templates render blackboard values into strings. All three subsystems are tested in isolation.

## Dependencies

- **Sprint 1** (`dsl-sprint-01`): Blackboard, Error types.
- **Sprint 2** (`dsl-sprint-02`): AST types (Evaluator/OutputParser are referenced in nodes).

## Non-goals

- No executor or node behavior (Sprint 4).
- No hot reload or observability (Sprint 5).

---

## Stories

### Story 3.1: Evaluator Enum & Built-ins

**Priority**: P0
**Effort**: 5 points
**Status**: Backlog

Implement the `Evaluator` enum and all 9 built-in evaluators. Evaluators are pure functions from `&Blackboard` to `Result<bool, RuntimeError>`.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T3.1.1 | Define `Evaluator` enum (9 variants) with serde attributes | Todo | - |
| T3.1.2 | Implement `OutputContains` evaluator (case-sensitive/insensitive) | Todo | - |
| T3.1.3 | Implement `SituationIs` evaluator | Todo | - |
| T3.1.4 | Implement `ReflectionRoundUnder` evaluator | Todo | - |
| T3.1.5 | Implement `VariableIs` evaluator with dot-notation support | Todo | - |
| T3.1.6 | Implement `RegexMatch` evaluator (compile regex at parse time) | Todo | - |
| T3.1.7 | Implement `Script` evaluator with grammar: `comparison (('&&' | '\|\|') comparison)*` where comparison supports `path op literal`, `is_dangerous(path)`, and `path.contains(string)` | Todo | - |
| T3.1.7a | Implement `is_dangerous(provider_output)` — checks for known dangerous keywords ("delete", "drop", "rm -rf", "truncate table", etc.) | Todo | - |
| T3.1.7b | Implement `path.contains(string)` dot-call syntax | Todo | - |
| T3.1.7c | Implement short-circuit evaluation for `&&` and `\|\|` | Todo | - |
| T3.1.8 | Implement `Or` / `And` composite evaluators with short-circuit | Todo | - |
| T3.1.9 | Implement `Not` evaluator | Todo | - |
| T3.1.10 | Define `EvaluatorRegistry` with `register` / `create` methods | Todo | - |
| T3.1.11 | Implement `with_builtins()` constructor | Todo | - |
| T3.1.12 | Implement `Custom` evaluator dispatch through registry | Todo | - |
| T3.1.12 | Write unit tests for all 9 evaluators | Todo | - |
| T3.1.13 | Write unit tests for Script evaluator comparison operators (`==`, `!=`, `<`, `<=`, `>`, `>=`) | Todo | - |
| T3.1.14 | Write unit tests for Script evaluator nested expressions (`reflection_round < 2 && provider_output.contains("error")`) | Todo | - |
| T3.1.15 | Write unit tests for Script evaluator short-circuit correctness (`&&` stops on first false, `||` stops on first true) | Todo | - |
| T3.1.16 | Write unit tests for Script evaluator path resolution with dot-notation (`last_tool_call.name`, `file_changes.0.path`) | Todo | - |

#### Acceptance Criteria

- `Evaluator::evaluate` is a pure function (no side effects).
- `Or` short-circuits on first `true`; `And` short-circuits on first `false`.
- `RegexMatch` stores pattern as `String` (validated at parse time, compiled at eval time).
- `Custom` variant allows host-registered evaluators.
- `Script` evaluator: `is_dangerous(provider_output)` detects dangerous keywords; `path.contains(string)` checks substring membership; `&&` / `||` short-circuit correctly; comparison operators (`==`, `!=`, `<`, `<=`, `>`, `>=`) work with numeric and string literals; nested expressions combine operators and function calls; dot-notation paths resolve correctly.

#### Technical Notes

**Script evaluator grammar (V1)**:
```
expr       := comparison (('&&' | '||') comparison)*
comparison := path op literal | 'is_dangerous' '(' path ')' | path '.' 'contains' '(' string ')'
path       := identifier ('.' identifier)*
op         := '==' | '!=' | '<' | '<=' | '>' | '>='
literal    := string | number | 'true' | 'false'
```

Built-in functions:
- `is_dangerous(provider_output)` → `true` if output contains dangerous keywords (delete, drop, rm -rf, truncate table, etc.)
- `path.contains(string)` → `true` if the string value at `path` contains the given substring

```rust
pub(crate) enum Evaluator {
    OutputContains { pattern: String, #[serde(default, rename = "caseSensitive")] case_sensitive: bool },
    SituationIs { situation_type: String },
    ReflectionRoundUnder { max: u8 },
    VariableIs { key: String, expected: BlackboardValue },
    RegexMatch { pattern: String },
    Script { expression: String },
    Or { conditions: Vec<Evaluator> },
    And { conditions: Vec<Evaluator> },
    Not { condition: Box<Evaluator> },
    Custom { name: String, params: HashMap<String, BlackboardValue> },
}
```

---

### Story 3.2: OutputParser Enum & Built-ins

**Priority**: P0
**Effort**: 5 points
**Status**: Backlog

Implement the `OutputParser` enum and all 4 built-in parsers. Parsers convert raw LLM text into `HashMap<String, BlackboardValue>`.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T3.2.1 | Define `OutputParser` enum (5 variants) with serde attributes | Todo | - |
| T3.2.2 | Define `StructuredField` and `FieldType` | Todo | - |
| T3.2.3 | Implement `Enum` parser (case-sensitive/insensitive match) | Todo | - |
| T3.2.4 | Implement `Structured` parser (regex capture groups + typed fields) | Todo | - |
| T3.2.5 | Implement `Json` parser (optional schema validation) | Todo | - |
| T3.2.6 | Implement `Command` parser (`__command` magic key for direct command emission) | Todo | - |
| T3.2.7 | Define `OutputParserRegistry` with `register` / `create` methods | Todo | - |
| T3.2.8 | Implement `with_builtins()` constructor | Todo | - |
| T3.2.9 | Implement `Custom` parser dispatch through registry | Todo | - |
| T3.2.9 | Write unit tests for all 4 parsers | Todo | - |
| T3.2.10 | Write unit tests for missing capture group error | Todo | - |

#### Acceptance Criteria

- `Enum` parser trims whitespace before matching.
- `Structured` parser validates all capture groups exist.
- `Json` parser handles nested objects via `BlackboardValue::Map`.
- `Command` parser emits `DecisionCommand` directly into blackboard via `__command` key.

#### Technical Notes

```rust
pub(crate) enum OutputParser {
    Enum { values: Vec<String>, #[serde(default, rename = "caseSensitive")] case_sensitive: bool },
    Structured { pattern: String, fields: Vec<StructuredField> },
    Json { schema: Option<serde_json::Value> },
    Command { mapping: HashMap<String, DecisionCommand> },
    Custom { name: String, params: HashMap<String, BlackboardValue> },
}

pub(crate) struct StructuredField {
    pub name: String,
    pub group: usize,
    #[serde(default, rename = "type")]
    pub ty: FieldType,
}
```

---

### Story 3.3: minijinja Template Engine

**Priority**: P0
**Effort**: 3 points
**Status**: Backlog

Integrate `minijinja` for Jinja2-compatible template rendering. All Prompt templates and Action command field interpolations share the same environment.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T3.3.1 | Create global `Environment` via `std::sync::OnceLock` | Todo | - |
| T3.3.2 | Register custom `slugify` filter | Todo | - |
| T3.3.3 | Register custom `truncate` filter | Todo | - |
| T3.3.4 | Implement `render_prompt_template(template_str, &Value) -> Result<String, RuntimeError>` | Todo | - |
| T3.3.5 | Implement `blackboard_value_to_minijinja` conversion | Todo | - |
| T3.3.6 | Implement `Blackboard::to_template_context()` | Todo | - |
| T3.3.7 | Write unit tests for template rendering with variables | Todo | - |
| T3.3.8 | Write unit tests for template filters | Todo | - |
| T3.3.9 | Write unit tests for template syntax validation at load time (detect invalid `{% if %}` tags, unclosed braces) | Todo | - |
| T3.3.10 | Write unit tests for template runtime error handling (missing variable with `default`, invalid filter) | Todo | - |

#### Acceptance Criteria

- `render_prompt_template` uses a lazily-initialized global `Environment`.
- `Blackboard::to_template_context()` exposes built-in fields + scoped variables.
- `last_tool_call` is exposed as a nested object; `file_changes` as a list of objects.
- Standard minijinja filters (`upper`, `lower`, `length`, `default`, `join`, etc.) are available.
- minijinja built-in features work without custom implementation: `{% if %}`, `{% for %}`, `| json`, whitespace control.
- Invalid template syntax is detected at load time with clear error messages.
- Runtime template errors (missing variables without `default`, unknown filters) are caught and reported.

#### Technical Notes

```rust
static TEMPLATE_ENV: OnceLock<Environment<'static>> = OnceLock::new();

fn get_template_env() -> &'static Environment<'static> {
    TEMPLATE_ENV.get_or_init(|| {
        let mut env = Environment::new();
        env.add_filter("slugify", |value: String| { /* ... */ });
        env.add_filter("truncate", |value: String, n: usize| { /* ... */ });
        env
    })
}
```

---

### Story 3.4: Command Template Rendering

**Priority**: P1
**Effort**: 3 points
**Status**: Backlog

Implement recursive template interpolation for all string fields in `DecisionCommand` variants.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T3.4.1 | Implement `render_command_templates(cmd, bb) -> Result<DecisionCommand, RuntimeError>` | Todo | - |
| T3.4.2 | Cover all AgentCommand string fields | Todo | - |
| T3.4.3 | Cover all GitCommand string fields | Todo | - |
| T3.4.4 | Cover all TaskCommand string fields | Todo | - |
| T3.4.5 | Cover all HumanCommand string fields | Todo | - |
| T3.4.6 | Cover all ProviderCommand string fields | Todo | - |
| T3.4.7 | Handle `Option<String>` fields with `map` + `transpose` | Todo | - |
| T3.4.8 | Pass-through commands with no string fields unchanged | Todo | - |
| T3.4.9 | Write unit tests for command template rendering | Todo | - |

#### Acceptance Criteria

- Every `String` field in every `DecisionCommand` variant is rendered.
- Commands with no string fields (e.g., `ApproveAndContinue`, `WakeUp`, `Discard`) pass through unchanged.
- Rendering errors are caught at Action execution time, not at DSL load time.

#### Technical Notes

```rust
pub(crate) fn render_command_templates(
    cmd: &DecisionCommand,
    bb: &Blackboard,
) -> Result<DecisionCommand, RuntimeError> {
    let ctx = bb.to_template_context();
    let env = get_template_env();
    let render = |s: &str| -> Result<String, RuntimeError> {
        env.template_from_str(s)?.render(&ctx)
            .map_err(|e| RuntimeError::FilterError(e.to_string()))
    };
    // exhaustive match on all 5 DecisionCommand categories
}
```

---

## Sprint Completion Criteria

- [ ] `cargo check` passes for the `decision-dsl` crate.
- [ ] `cargo test --lib` passes with 100% coverage on evaluators and output parsers.
- [ ] Template rendering tests cover variable interpolation, filters, and nested object access.
- [ ] Command template rendering tests cover at least one variant per command category.
