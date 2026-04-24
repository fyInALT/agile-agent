# Decision DSL Specification

> This document defines the YAML-based DSL for authoring decision rules in the decision layer. Users write high-level **decision rules** that desugar to a behavior tree AST executed by the `BehaviorTreeExecutor`.

---

## 1. DSL Overview

The decision DSL is a **declarative YAML format** for defining what the decision layer should do when an agent produces output. Two authoring styles are supported:

| Style | Kind | Use case |
|-------|------|----------|
| **DecisionRules** (recommended) | `kind: DecisionRules` | Priority-ordered rule list. Covers 90% of decisions in ~30% of the YAML. |
| **BehaviorTree** (advanced) | `kind: BehaviorTree` | Full behavior tree. For complex scenarios that don't fit the rules model. |

Both styles compile to the same internal AST and produce identical `DecisionCommand` output.

### File Structure

```
decisions/
├── rules.d/                        # DecisionRules files
│   ├── default.yaml                # The root rule set
│   └── task-lifecycle.yaml
├── trees/                          # BehaviorTree files (advanced)
│   └── complex-recovery.yaml
├── subtrees/                       # Reusable sub-trees
│   ├── reflect_loop.yaml
│   └── human_escalation.yaml
└── schemas/
    └── action_schemas.yaml
```

### DecisionRules Format (Recommended)

```yaml
apiVersion: decision.agile-agent.io/v1
kind: DecisionRules
metadata:
  name: default_decisions
  description: "Handles all common decision scenarios"
spec:
  rules:
    - priority: 1
      name: rate_limit
      if:
        kind: regex
        pattern: "(429|rate.?limit|quota.?exceeded)"
      then:
        command:
          RetryTool:
            tool_name: "{{ last_tool_call.name }}"
            max_attempts: 3
      cooldownMs: 5000

    - priority: 2
      name: dangerous_action
      if:
        kind: outputContains
        pattern: "delete_files"
      then:
        command:
          EscalateToHuman:
            reason: "Dangerous action detected"
            context: "{{ provider_output | truncate(200) }}"

    - priority: 3
      name: claims_completion
      if:
        kind: outputContains
        pattern: "claims_completion"
      then:
        kind: Switch
        name: reflect_or_confirm
        on:
          kind: prompt
          model: thinking
          template: |
            ## Task
            {{ task_description }}

            ## Agent's Claim
            {{ provider_output | truncate(500) }}

            Should we REFLECT or CONFIRM?
            Reply with exactly one word: REFLECT or CONFIRM.
          parser:
            kind: enum
            values: [REFLECT, CONFIRM]
        cases:
          REFLECT:
            command:
              Reflect:
                prompt: "Review your work carefully"
          CONFIRM:
            command: ConfirmCompletion
      reflectionMaxRounds: 2

    - priority: 4
      name: error_recovery
      if:
        kind: outputContains
        pattern: "error"
      then:
        kind: Switch
        name: classify_and_act
        on:
          kind: prompt
          model: thinking
          template: |
            Classify this error and recommend action.
            Error: {{ provider_output | truncate(600) }}
            Reply format: CLASS: <type> RECOMMEND: <action>
          parser:
            kind: structured
            pattern: "CLASS:\\s*(\\w+)\\s*RECOMMEND:\\s*(\\w+)"
            fields:
              - { name: classification, group: 1 }
              - { name: recommendation, group: 2 }
        cases:
          RETRY:
            command:
              RetryTool:
                tool_name: "{{ last_tool_call.name }}"
                max_attempts: 3
          FIX:
            command:
              SendCustomInstruction:
                prompt: "Fix the {{ recommendation }} and retry"
                target_agent: "{{ agent_id }}"
          ESCALATE:
            command:
              EscalateToHuman:
                reason: "Error recovery chose escalation"

    - priority: 99
      name: default_continue
      then:
        command: ApproveAndContinue
```

### BehaviorTree Format (Advanced)

```yaml
apiVersion: decision.agile-agent.io/v1
kind: BehaviorTree
metadata:
  name: complex_recovery
  description: "Complex error recovery with custom tree structure"
spec:
  root:
    kind: Selector
    name: root_handler
    children:
      - kind: SubTree
        name: use_rate_limit
        ref: rate_limit_handler
      - kind: When
        name: handle_dangerous
        if:
          kind: script
          expression: "is_dangerous(provider_output)"
        then:
          command:
            EscalateToHuman:
              reason: "Dangerous action detected"
      - kind: Action
        name: default_continue
        command: ApproveAndContinue
```

| Field | Required | Description |
|-------|----------|-------------|
| `apiVersion` | Yes | Schema version for migration. Format: `decision.agile-agent.io/v{N}`. |
| `kind` | Yes | `DecisionRules`, `BehaviorTree`, or `SubTree`. |
| `metadata.name` | Yes | Unique identifier for the tree. |
| `metadata.description` | No | Human-readable description. |
| `spec.rules` | Yes (DecisionRules) | Priority-ordered list of decision rules. |
| `spec.root` | Yes (BehaviorTree) | The root node of the behavior tree. |

---

## 2. DecisionRules Spec

### 2.1 Rule Structure

Each rule in `spec.rules` is tried in priority order (lowest number first). The first rule whose `if` condition matches is executed; subsequent rules are skipped.

```yaml
rules:
  - priority: 1           # Integer. Lower = higher priority. Must be unique.
    name: my_rule         # Unique name for tracing.
    if:                   # Optional. If omitted, rule always matches.
      kind: outputContains
      pattern: "429"
    then:                 # Required. Action, Switch, When, Pipeline, or command inline.
      command: ApproveAndContinue
    cooldownMs: 5000      # Optional. Cooldown after this rule fires.
    reflectionMaxRounds: 2 # Optional. Max reflection rounds (only for Switch with prompt).
    on_error: skip        # Optional. "skip" | "escalate" | "retry". Default: "skip".
```

| Field | Required | Description |
|-------|----------|-------------|
| `priority` | Yes | Integer priority. Lower = higher priority. Must be unique within the rule set. |
| `name` | Yes | Unique name for tracing and debugging. |
| `if` | No | Condition evaluator. If omitted, rule always matches. Same schema as Condition node `eval`. |
| `then` | Yes | Action to take. Can be an inline command, a `Switch`, a `When`, or a `Pipeline`. |
| `cooldownMs` | No | Cooldown duration in milliseconds after this rule fires successfully. |
| `reflectionMaxRounds` | No | Maximum reflection rounds (only meaningful when `then` is a `Switch` with `on: prompt`). |
| `on_error` | No | Error handling behavior: `skip` (default, try next rule), `escalate` (escalate to human), `retry` (retry the rule once). |

### 2.2 Inline Command Shorthand

When `then` is a simple command, you can inline it directly:

```yaml
# Full form
then:
  kind: Action
  name: emit_retry
  command:
    RetryTool:
      tool_name: "{{ last_tool_call.name }}"
      max_attempts: 3

# Inline shorthand (equivalent)
then:
  command:
    RetryTool:
      tool_name: "{{ last_tool_call.name }}"
      max_attempts: 3
```

### 2.3 Rule Desugaring

A `DecisionRules` spec desugars to a BehaviorTree:

```
DecisionRules { rules: [R1, R2, ..., Rn] }
  ↓
BehaviorTree { root: Selector(children: [R1.desugar(), R2.desugar(), ..., Rn.desugar()]) }

Rule { if: C, then: T, cooldownMs: D, reflectionMaxRounds: R, on_error: E }
  ↓
if cooldownMs:  Cooldown(durationMs: D, child: Sequence(Condition(C), T.desugar()))
if reflectionMaxRounds:  ReflectionGuard(maxRounds: R, child: Sequence(Condition(C), T.desugar()))
otherwise:  Sequence(Condition(C), T.desugar())
```

---

## 3. High-Level Node Types

These node types are shorthands that desugar to low-level behavior tree nodes. They cover the most common decision patterns with minimal YAML.

### 3.1 Switch

The `Switch` node replaces the verbose Prompt+Selector+Condition+Action pattern. It evaluates a condition (a prompt or a blackboard variable) and dispatches to the matching case.

#### Switch on Prompt

```yaml
kind: Switch
name: completion_decision
on:
  kind: prompt
  model: thinking            # Optional. "standard" (default) or "thinking".
  timeoutMs: 30000           # Optional. Default: 30000.
  template: |
    Should we REFLECT or CONFIRM?
    Reply with exactly one word.
  parser:
    kind: enum
    values: [REFLECT, CONFIRM]
    caseSensitive: false
cases:
  REFLECT:
    command:
      Reflect:
        prompt: "Review your work carefully"
  CONFIRM:
    command: ConfirmCompletion
  _default:                  # Optional. Fires if no case matches.
    command: ApproveAndContinue
```

| Field | Required | Description |
|-------|----------|-------------|
| `on` | Yes | Switch condition. Use `on: { kind: prompt, ... }` or `on: { kind: variable, key: ... }`. |
| `cases` | Yes | Map of match value → action. Values come from the parser's `decision` field. |
| `_default` | No | Default case. If omitted, Switch returns Failure on unrecognized values. |

**Desugaring**: `Switch(on: Prompt) → Sequence(Prompt(on), Selector(for each case: When(if: var==case, then: case.command), DefaultCase))`

#### Switch on Variable

```yaml
kind: Switch
name: route_by_strategy
on:
  kind: variable
  key: error_strategy       # Reads from blackboard
cases:
  RETRY:
    command:
      RetryTool:
        tool_name: "{{ last_tool_call.name }}"
        max_attempts: 3
  FIX:
    command:
      SendCustomInstruction:
        prompt: "Fix the error and retry"
        target_agent: "{{ agent_id }}"
  ESCALATE:
    command:
      EscalateToHuman:
        reason: "Error recovery required"
```

No LLM call is made. The variable is read directly from the blackboard.

### 3.2 When

The `When` node is a guarded action: "if condition is true, execute this command."

```yaml
kind: When
name: handle_rate_limit
if:
  kind: regex
  pattern: "(429|rate.?limit)"
then:
  command:
    RetryTool:
      tool_name: "{{ last_tool_call.name }}"
      max_attempts: 3
```

| Field | Required | Description |
|-------|----------|-------------|
| `if` | Yes | Condition evaluator. Same schema as the Condition node `eval` field. |
| `then` | Yes | The action to take. Can be an inline command, a Switch, or a Pipeline. |
| `on_error` | No | Error handling: `skip` (default), `escalate`, or `retry`. |

**Desugaring**: `When → Sequence(Condition(if), Action(then.command))`

### 3.3 Pipeline

The `Pipeline` node chains multiple guarded steps sequentially. All steps must succeed for the pipeline to succeed.

```yaml
kind: Pipeline
name: safe_commit_check
steps:
  - if:
      kind: outputContains
      pattern: "all tests passing"
  - if:
      kind: script
      expression: "file_changes.length() < 10"
  - then:
      command:
        SuggestCommit:
          message: "Safe checkpoint"
          mandatory: false
          reason: "Tests pass, small change set"
```

| Field | Required | Description |
|-------|----------|-------------|
| `steps` | Yes | Ordered list of steps. Each step has `if` (guard) or `then` (unguarded action). |

**Desugaring**: `Pipeline → Sequence(for each step: if → Condition; then → Action)`

---

## 4. Low-Level Node Types

The following node types are the compilation target for high-level constructs. Use them directly only for complex scenarios that don't fit the rules model.

### 4.1 Common Fields

Every node has these common fields:

```yaml
kind: Selector      # Node type
name: my_node       # Unique name within the tree
```

| Field | Required | Description |
|-------|----------|-------------|
| `kind` | Yes | Node type. |
| `name` | Yes | Unique name for tracing and debugging. |

### 4.2 Composite Nodes

#### Selector

Tries children left-to-right. Returns Success on the first child that succeeds. Returns Failure if all children fail. Used for priority-ordered fallback.

```yaml
kind: Selector
name: root_handler
children:
  - kind: Sequence
    name: handle_rate_limit
    children: [...]
  - kind: Sequence
    name: handle_claims
    children: [...]
  - kind: Action
    name: default_continue
    command: ApproveAndContinue
```

| Field | Required | Description |
|-------|----------|-------------|
| `children` | Yes | List of child nodes. Executed left-to-right. |

#### Sequence

Executes children left-to-right. Returns Failure on the first child that fails. Returns Success if all children succeed. Used for sequential guard-then-act patterns.

```yaml
kind: Sequence
name: guarded_action
children:
  - kind: Condition
    name: guard
    eval: { kind: outputContains, pattern: "pattern" }
  - kind: Action
    name: act
    command: SomeCommand
```

| Field | Required | Description |
|-------|----------|-------------|
| `children` | Yes | List of child nodes. All must succeed. |

#### Parallel

Executes all children concurrently. Result depends on the policy.

```yaml
kind: Parallel
name: safety_checks
policy: allSuccess          # or anySuccess, majority
children:
  - kind: Condition
    name: check_a
    eval: { kind: script, expression: "check_a()" }
  - kind: Condition
    name: check_b
    eval: { kind: script, expression: "check_b()" }
```

| Field | Required | Description |
|-------|----------|-------------|
| `policy` | Yes | `allSuccess`, `anySuccess`, or `majority`. |
| `children` | Yes | List of child nodes. Executed concurrently. |

### 4.3 Decorator Nodes

Decorators wrap a single child node and modify its behavior.

#### Inverter

Inverts the child's status: Success → Failure, Failure → Success. Running passes through.

```yaml
kind: Inverter
name: not_rate_limit
child:
  kind: Condition
  name: is_rate_limit
  eval: { kind: outputContains, pattern: "429" }
```

#### Repeater

Repeats the child up to `maxAttempts` times. Returns Success if the child succeeds within the limit; Failure if it fails.

```yaml
kind: Repeater
name: retry_up_to_3
maxAttempts: 3
child:
  kind: Prompt
  name: llm_call
  template: "..."
  parser: { kind: enum, values: [SUCCESS, FAIL] }
```

#### Cooldown

Enforces a minimum interval between successful executions of the child. Returns Failure if still on cooldown.

```yaml
kind: Cooldown
name: rate_limit_cooldown
durationMs: 5000
child:
  kind: Action
  name: retry_tool
  command:
    RetryTool:
      tool_name: "{{ last_tool_call.name }}"
      max_attempts: 3
```

#### ReflectionGuard

Limits the number of times a child can succeed. Tracks `reflection_round` on the blackboard. Returns Failure when the max is reached.

```yaml
kind: ReflectionGuard
name: max_2_reflections
maxRounds: 2
child:
  kind: Prompt
  name: reflect_or_confirm
  template: "..."
```

#### ForceHuman

Wraps a child and forces an `EscalateToHuman` command after it succeeds.

```yaml
kind: ForceHuman
name: force_human_on_pr
reason: "PR submission requires human approval"
child:
  kind: Prompt
  name: confirm_pr
  template: "Submit PR? YES/NO"
  parser: { kind: enum, values: [YES, NO] }
```

### 4.4 Leaf Nodes

#### Condition

Evaluates a condition against the blackboard. Returns Success if true, Failure if false.

```yaml
kind: Condition
name: is_rate_limit
eval:
  kind: outputContains
  pattern: "429"
  caseSensitive: false
```

See [decision-dsl-evaluators.md](decision-dsl-evaluators.md) for the full evaluator catalog.

#### Action

Emits a `DecisionCommand`. Optionally guarded by a `when` condition.

```yaml
kind: Action
name: emit_reflect
command:
  Reflect:
    prompt: "Review your work carefully"
when:                        # Optional
  kind: variableIs
  key: next_action
  value: REFLECT
```

See [Command Reference](#command-reference) below for all command variants.

#### Prompt

Sends a template to the LLM within the same agent session. Parses the reply and stores values on the blackboard.

```yaml
kind: Prompt
name: classify_error
model: thinking              # Optional. "standard" (default) or "thinking".
timeoutMs: 45000             # Optional. Default: 30000.
template: |
  Classify this error:
  {{ provider_output | truncate(600) }}
  Reply: CLASS: <type> RECOMMEND: <action>
parser:
  kind: structured
  pattern: "CLASS:\\s*(\\w+)\\s*RECOMMEND:\\s*(\\w+)"
  fields:
    - { name: classification, group: 1 }
    - { name: recommendation, group: 2 }
sets:
  - key: error_class
    field: classification
  - key: error_recommendation
    field: recommendation
```

| Field | Required | Description |
|-------|----------|-------------|
| `model` | No | LLM model tier: `standard` (default) or `thinking`. |
| `timeoutMs` | No | Timeout in milliseconds. Default: 30000. |
| `template` | Yes | minijinja template. See [Template Syntax](#template-syntax). |
| `parser` | Yes | Output parser specification. |
| `sets` | No | List of `(blackboard_key, parser_field)` mappings. |

**Lifecycle**: On first tick, Prompt sends the rendered template to the session and returns `Running`. On the next tick, it receives the reply, parses it, stores values, and returns `Success` or `Failure`.

#### SetVar

Writes a value to the blackboard.

```yaml
kind: SetVar
name: init_counter
key: retry_count
value:
  kind: integer
  value: 0
```

**Value Types**:

```yaml
value: { kind: string, value: "hello" }
value: { kind: integer, value: 42 }
value: { kind: float, value: 0.85 }
value: { kind: boolean, value: true }
value:
  kind: list
  items:
    - { kind: string, value: "a" }
    - { kind: string, value: "b" }
```

### 4.5 SubTree Reference

References a reusable sub-tree. SubTrees preserve their identity in traces (unlike the previous design which inlined them).

```yaml
kind: SubTree
name: reflect_handler
ref: reflect_loop              # References subtrees/reflect_loop.yaml
```

| Field | Required | Description |
|-------|----------|-------------|
| `ref` | Yes | Name of the sub-tree to execute. |

---

## 5. Command Reference

Commands are grouped into five categories for clarity.

### Agent Commands

```yaml
command: ApproveAndContinue

command:
  Reflect:
    prompt: "Review your work carefully"

command:
  SendCustomInstruction:
    prompt: "Focus on fixing the tests"
    target_agent: "agent-42"

command:
  TerminateAgent:
    reason: "Task complete"
```

### Git Commands

```yaml
command:
  CommitChanges:
    message: "Add auth module"
    is_wip: true
    worktree_path: null

command:
  CreateTaskBranch:
    branch_name: "feature/auth"
    base_branch: "main"
    worktree_path: null

command:
  RebaseToMain:
    base_branch: "main"

command:
  StashChanges:
    description: "WIP before context switch"
    include_untracked: true

command:
  DiscardChanges:
    worktree_path: null
```

### Task Commands

```yaml
command: ConfirmCompletion

command:
  StopIfComplete:
    reason: "All tasks finished"

command:
  PrepareTaskStart:
    task_id: "TASK-123"
    task_description: "Implement user authentication"
```

### Human Commands

```yaml
command:
  EscalateToHuman:
    reason: "Need human approval"
    context: "The agent wants to delete files"

command:
  SelectOption:
    option_id: "0"
```

### Provider Commands

```yaml
command:
  RetryTool:
    tool_name: "Bash"
    max_attempts: 3

command:
  SwitchProvider:
    provider_type: "claude"

command:
  SuggestCommit:
    message: "Good checkpoint"
    mandatory: false
    reason: "Tests passing, small diff"

command:
  PreparePr:
    title: "Add auth module"
    description: "Implements JWT-based authentication"
    base_branch: "main"
    as_draft: false
```

### Command Interpolation

String fields in commands support template interpolation:

```yaml
command:
  RetryTool:
    tool_name: "{{ last_tool_call.name }}"
    args: "{{ last_tool_call.input }}"
    max_attempts: 3
```

Interpolation happens at Action execution time, not at DSL load time.

---

## 6. Blackboard Variables

### 6.1 Built-in Variables

These variables are automatically populated before each decision cycle:

| Variable | Type | Description |
|----------|------|-------------|
| `task_description` | string | The task description given to the work agent. |
| `provider_output` | string | Raw output from the LLM provider. |
| `context_summary` | string | Condensed summary of recent tool calls and file changes. |
| `reflection_round` | integer | Current reflection round (0, 1, 2, ...). |
| `max_reflection_rounds` | integer | Maximum allowed reflection rounds. |
| `confidence_accumulator` | float | Accumulated confidence score. |
| `last_tool_call` | object | Structured record of the last tool call (name, input, output). |
| `file_changes` | list | List of recent file changes (path, change_type). |
| `agent_id` | string | ID of the work agent being decided for. |
| `current_task_id` | string | Current task ID, if any. |
| `current_story_id` | string | Current story ID, if any. |
| `decision_history` | list | List of recent decision records. |
| `project_rules` | object | Parsed project rules from CLAUDE.md / AGENTS.md. |

### 6.2 Custom Variables

Nodes can read and write custom variables via `SetVar` and `sets`:

```yaml
# Write
kind: SetVar
name: set_flag
key: needs_review
value: { kind: boolean, value: true }

# Read in template: {{ needs_review }}

# Read in condition
kind: Condition
name: check_flag
eval:
  kind: variableIs
  key: needs_review
  value: true
```

### 6.3 Variable Scoping

SubTree execution creates a new scope. Variables written inside a sub-tree do not leak to the parent. Variables from parent scopes are visible (read-only) to child scopes.

```yaml
# Parent tree sets:       variables.phase = "coding"
# SubTree reads:          {{ phase }} → "coding"        (visible)
# SubTree writes:         variables.local_decision = "X" (scoped)
# Parent after SubTree:   variables.local_decision → undefined
```

This prevents accidental cross-tree variable pollution.

### 6.4 Variable Interpolation in Commands

Commands can interpolate blackboard variables using `{{ variable }}` syntax:

```yaml
kind: Action
name: retry_last_tool
command:
  RetryTool:
    tool_name: "{{ last_tool_call.name }}"
    args: "{{ last_tool_call.input }}"
    max_attempts: 3
```

---

## 7. Template Syntax

Templates use **minijinja** syntax with the Blackboard as the template context.

### 7.1 Variable Interpolation

```text
{{ task_description }}
{{ provider_output }}
{{ reflection_round }}
{{ last_tool_call.name }}
```

### 7.2 Filters

```text
{{ provider_output | truncate(500) }}
{{ file_changes | length }}
{{ task_description | upper }}
{{ context_summary | default("No summary available") }}
{{ current_task_id | slugify }}
{{ file_changes | json }}
```

| Filter | Description |
|--------|-------------|
| `truncate(n)` | Truncate to N characters, append "...". |
| `length` | Length of list or string. |
| `upper` / `lower` | Case conversion. |
| `default(val)` | Use default if variable is missing or empty. |
| `join(sep)` | Join list elements with separator. |
| `json` | Serialize value to JSON string. |
| `slugify` | Convert to lowercase-hyphenated slug. |

Additional filters are provided by minijinja's built-in set. See [minijinja documentation](https://docs.rs/minijinja) for the full catalog.

### 7.3 Conditionals and Loops

```text
{% if reflection_round > 0 %}
  This is reflection round {{ reflection_round }}.
{% else %}
  This is the initial decision.
{% endif %}

{% if file_changes | length > 0 %}
  Recent changes:
  {% for change in file_changes %}
    - {{ change.path }} ({{ change.change_type }})
  {% endfor %}
{% endif %}
```

### 7.4 Best Practices

1. **Always include output format instructions**: Tell the LLM exactly what to return.
2. **Use `truncate` on large inputs**: `provider_output` can be very long.
3. **Use `default` for optional variables**: Prevent template errors.
4. **Keep templates under 2000 tokens**: The executor warns if a rendered prompt exceeds the budget.

---

## 8. Output Parsers

### 8.1 Enum Parser

Parse a single value from a constrained set:

```yaml
parser:
  kind: enum
  values: [REFLECT, CONFIRM, ESCALATE]
  caseSensitive: false
```

**Input**: `"  reflect  "` → **Parsed**: `{ decision: "REFLECT" }`

### 8.2 Structured Parser

Parse fields from text using regex:

```yaml
parser:
  kind: structured
  pattern: "CLASS:\\s*(\\w+)\\s*RECOMMEND:\\s*(\\w+)"
  fields:
    - { name: classification, group: 1 }
    - { name: recommendation, group: 2, type: string }
```

Supported field types: `string` (default), `integer`, `float`, `boolean`.

### 8.3 JSON Parser

Parse JSON responses with optional schema validation:

```yaml
parser:
  kind: json
  schema:            # Optional JSON Schema
    type: object
    properties:
      decision:
        type: string
        enum: [reflect, confirm]
      confidence:
        type: number
    required: [decision]
```

### 8.4 Command Parser

Parse LLM output directly into a DecisionCommand (for use with low-level Prompt nodes):

```yaml
parser:
  kind: command
  mapping:
    REFLECT:
      command: Reflect
      params:
        prompt: "Review your work"
    CONFIRM:
      command: ConfirmCompletion
```

---

## 9. Complete Examples

### Example 1: Default Decision Rules (Recommended)

```yaml
# rules.d/default.yaml
apiVersion: decision.agile-agent.io/v1
kind: DecisionRules
metadata:
  name: default_decisions
  description: "Handles rate limits, dangerous actions, claims completion, errors, and defaults"
spec:
  rules:
    - priority: 1
      name: rate_limit
      if:
        kind: regex
        pattern: "(429|rate.?limit|quota.?exceeded)"
      then:
        command:
          RetryTool:
            tool_name: "{{ last_tool_call.name }}"
            max_attempts: 3
      cooldownMs: 5000

    - priority: 2
      name: dangerous_action
      if:
        kind: script
        expression: "is_dangerous(provider_output)"
      then:
        command:
          EscalateToHuman:
            reason: "Dangerous action detected"
            context: "{{ provider_output | truncate(200) }}"

    - priority: 3
      name: claims_completion
      if:
        kind: outputContains
        pattern: "claims_completion"
      then:
        kind: Switch
        name: reflect_or_confirm
        on:
          kind: prompt
          model: thinking
          template: |
            ## Task: {{ task_description }}

            ## Agent Output
            {{ provider_output | truncate(500) }}

            ## Reflection Round
            {{ reflection_round }} / {{ max_reflection_rounds }}

            Should we REFLECT or CONFIRM?
            Reply with exactly one word.
          parser:
            kind: enum
            values: [REFLECT, CONFIRM]
        cases:
          REFLECT:
            command:
              Reflect:
                prompt: |
                  Please review your work carefully:
                  1. Are all tests passing?
                  2. Have you reviewed all changed files?
                  3. Does the code follow project conventions?
          CONFIRM:
            command: ConfirmCompletion
      reflectionMaxRounds: 2

    - priority: 4
      name: error_recovery
      if:
        kind: outputContains
        pattern: "error"
      then:
        kind: Switch
        name: classify_and_recover
        on:
          kind: prompt
          model: thinking
          timeoutMs: 45000
          template: |
            Classify this error and recommend recovery.

            Error output:
            {{ provider_output | truncate(600) }}

            Last tool: {{ last_tool_call.name }}
            Tool output: {{ last_tool_call.output | default("N/A") | truncate(300) }}

            Classify: SYNTAX, TEST, PERMISSION, TIMEOUT, RATE_LIMIT, LOGIC, UNKNOWN
            Recommend: RETRY, FIX, SKIP, or ESCALATE

            Reply format:
            CLASS: <classification>
            RECOMMEND: <recommendation>
            REASON: <one sentence>
          parser:
            kind: structured
            pattern: "CLASS:\\s*(\\w+)\\s*RECOMMEND:\\s*(\\w+)\\s*REASON:\\s*(.*)"
            fields:
              - { name: classification, group: 1 }
              - { name: recommendation, group: 2 }
              - { name: reason, group: 3 }
        cases:
          RETRY:
            command:
              RetryTool:
                tool_name: "{{ last_tool_call.name }}"
                max_attempts: 3
          FIX:
            command:
              SendCustomInstruction:
                prompt: |
                  Error type: {{ recommendation }}
                  Reason: {{ reason }}
                  Please fix this issue and retry.
                target_agent: "{{ agent_id }}"
          ESCALATE:
            command:
              EscalateToHuman:
                reason: "Error recovery: {{ recommendation }}"
                context: "{{ reason }}"
          SKIP:
            command:
              EscalateToHuman:
                reason: "Cannot recover from error"
                context: "{{ reason }}"

    - priority: 99
      name: default_continue
      then:
        command: ApproveAndContinue
```

### Example 2: Using BehaviorTree for Complex Logic

```yaml
# trees/complex-recovery.yaml
apiVersion: decision.agile-agent.io/v1
kind: BehaviorTree
metadata:
  name: complex_recovery
  description: "Complex error recovery with custom parallel checks"
spec:
  root:
    kind: Selector
    name: root
    children:
      - kind: SubTree
        name: rate_limit_handler
        ref: rate_limit_handler

      - kind: Switch
        name: decide_recovery
        on:
          kind: prompt
          model: thinking
          template: |
            Error: {{ provider_output | truncate(500) }}
            Choose: RETRY, FIX, or ESCALATE
          parser:
            kind: enum
            values: [RETRY, FIX, ESCALATE]
        cases:
          RETRY:
            command:
              RetryTool:
                tool_name: "{{ last_tool_call.name }}"
                max_attempts: 3
          FIX:
            command:
              SendCustomInstruction:
                prompt: "Fix the error"
                target_agent: "{{ agent_id }}"
          ESCALATE:
            command:
              EscalateToHuman:
                reason: "LLM chose escalation"

      - kind: Action
        name: default_continue
        command: ApproveAndContinue
```

### Example 3: Task Lifecycle with Pipeline

```yaml
# rules.d/task-lifecycle.yaml
apiVersion: decision.agile-agent.io/v1
kind: DecisionRules
metadata:
  name: task_lifecycle
  description: "Task start and completion flows"
spec:
  rules:
    - priority: 1
      name: task_starting
      if:
        kind: outputContains
        pattern: "task_starting"
      then:
        kind: Switch
        name: git_strategy
        on:
          kind: prompt
          model: standard
          template: |
            Starting task: {{ current_task_id }}
            Description: {{ task_description | truncate(300) }}

            Choose git strategy:
            - NEW_BRANCH: Create a new feature branch
            - EXISTING: Use existing branch
            Reply: NEW_BRANCH or EXISTING
          parser:
            kind: enum
            values: [NEW_BRANCH, EXISTING]
        cases:
          NEW_BRANCH:
            kind: Pipeline
            name: create_branch_flow
            steps:
              - then:
                  command:
                    CreateTaskBranch:
                      branch_name: "feature/{{ current_task_id | slugify }}"
                      base_branch: "main"
              - then:
                  command:
                    PrepareTaskStart:
                      task_id: "{{ current_task_id }}"
                      task_description: "{{ task_description }}"
          EXISTING:
            kind: Pipeline
            name: existing_branch_flow
            steps:
              - then:
                  command:
                    RebaseToMain:
                      base_branch: "main"
              - then:
                  command:
                    PrepareTaskStart:
                      task_id: "{{ current_task_id }}"
                      task_description: "{{ task_description }}"

    - priority: 99
      name: default_continue
      then:
        command: ApproveAndContinue
```

---

## 10. DSL Loading & Validation

### 10.1 Loader

```rust
pub struct DslLoader {
    base_path: PathBuf,
    evaluator_registry: EvaluatorRegistry,
    parser_registry: OutputParserRegistry,
}

impl DslLoader {
    pub fn load_rules(&self, name: &str) -> Result<BehaviorTree, DslError> {
        let path = self.base_path.join("rules.d").join(format!("{}.yaml", name));
        let raw = std::fs::read_to_string(&path)?;
        let doc: DslDocument = serde_yaml::from_str(&raw)?;

        // Validate apiVersion
        if doc.api_version != "decision.agile-agent.io/v1" {
            return Err(DslError::UnsupportedVersion(doc.api_version));
        }

        // Desugar DecisionRules → BehaviorTree AST
        let tree = self.desugar_rules(doc)?;

        // Resolve sub-tree references
        let tree = self.resolve_sub_trees(tree)?;

        // Validate node names are unique
        tree.validate_unique_names()?;

        // Validate all evaluators and parsers reference known kinds
        tree.validate_all(&self.evaluator_registry, &self.parser_registry)?;

        Ok(tree)
    }

    fn desugar_rules(&self, doc: DslDocument) -> Result<BehaviorTree, DslError> {
        match doc.kind {
            DslKind::DecisionRules => {
                let rules = doc.spec.rules.ok_or(DslError::MissingRules)?;
                let mut children = Vec::new();
                for rule in rules {
                    children.push(self.desugar_rule(rule)?);
                }
                Ok(BehaviorTree {
                    api_version: doc.api_version,
                    kind: TreeKind::BehaviorTree,
                    metadata: doc.metadata,
                    spec: Spec {
                        root: Node::Selector(SelectorNode {
                            name: format!("{}_rules", doc.metadata.name),
                            children,
                            active_child: None,
                        }),
                    },
                })
            }
            DslKind::BehaviorTree => {
                // Parse directly as BT
                self.parse_behavior_tree(doc)
            }
            DslKind::SubTree => {
                self.parse_sub_tree(doc)
            }
        }
    }
}
```

### 10.2 Desugaring Rules

```rust
impl DslLoader {
    fn desugar_rule(&self, rule: Rule) -> Result<Node, DslError> {
        // Build the inner action from `then`
        let inner = self.desugar_then(rule.then)?;

        // Build the condition from `if` (if present)
        let guarded = if let Some(condition) = rule.if {
            Node::Sequence(SequenceNode {
                name: format!("{}_guard", rule.name),
                children: vec![
                    Node::Condition(ConditionNode {
                        name: format!("{}_cond", rule.name),
                        evaluator: self.evaluator_registry.create(&condition.kind, &condition.props)?,
                    }),
                    inner,
                ],
                active_child: None,
            })
        } else {
            inner
        };

        // Wrap in Cooldown if specified
        let with_cooldown = if let Some(ms) = rule.cooldown_ms {
            Node::Cooldown(CooldownNode {
                name: format!("{}_cooldown", rule.name),
                duration: Duration::from_millis(ms),
                child: Box::new(guarded),
                last_success: None,
            })
        } else {
            guarded
        };

        // Wrap in ReflectionGuard if specified
        let with_reflection = if let Some(max_rounds) = rule.reflection_max_rounds {
            Node::ReflectionGuard(ReflectionGuardNode {
                name: format!("{}_reflection", rule.name),
                max_rounds,
                child: Box::new(with_cooldown),
            })
        } else {
            with_cooldown
        };

        Ok(with_reflection)
    }

    fn desugar_then(&self, then: ThenSpec) -> Result<Node, DslError> {
        match then {
            ThenSpec::InlineCommand { command } => {
                Ok(Node::Action(ActionNode {
                    name: "emit".into(),
                    command,
                    when: None,
                }))
            }
            ThenSpec::Switch(switch) => self.desugar_switch(switch),
            ThenSpec::When(when) => self.desugar_when(when),
            ThenSpec::Pipeline(pipeline) => self.desugar_pipeline(pipeline),
        }
    }
}
```

### 10.3 Validation Rules

1. **apiVersion must be supported**: The loader rejects unknown versions.
2. **All names must be unique** within a tree or rule set.
3. **Rule priorities must be unique** within a DecisionRules spec.
4. **SubTree refs must resolve**: The referenced sub-tree file must exist.
5. **All evaluator kinds must be registered**: Unknown `kind` values are rejected at load time.
6. **All parser kinds must be registered**: Unknown `kind` values are rejected at load time.
7. **Command variants must be valid**: Unknown command variants are rejected.
8. **No circular SubTree references**: A → B → C → A is rejected.

### 10.4 Hot Reload

```rust
pub struct DslWatcher {
    loader: DslLoader,
    current_tree: Arc<RwLock<BehaviorTree>>,
}

impl DslWatcher {
    pub fn start_watching(&self) -> Result<(), notify::Error> {
        let mut watcher = notify::recommended_watcher(|res| {
            match res {
                Ok(event) => {
                    if event.kind.is_modify() {
                        self.reload();
                    }
                }
                Err(e) => tracing::error!("Watch error: {}", e),
            }
        })?;
        watcher.watch(&self.loader.base_path, RecursiveMode::Recursive)?;
        Ok(())
    }

    fn reload(&self) {
        match self.loader.load_rules("default") {
            Ok(new_tree) => {
                *self.current_tree.write() = new_tree;
                tracing::info!("DSL hot-reloaded successfully");
            }
            Err(e) => {
                tracing::error!("DSL reload failed: {}", e);
                // Keep old tree running
            }
        }
    }
}
```

**Hot reload semantics**:
- In-flight decisions use the old tree.
- New decisions use the reloaded tree.
- If reload fails, the old tree continues to run. An error is logged.

---

## 11. Extending the DSL

### 11.1 Custom Condition Evaluators

Register a custom evaluator:

```rust
pub struct JiraTicketExists;

impl Evaluator for JiraTicketExists {
    fn evaluate(&self, bb: &Blackboard) -> Result<bool, RuntimeError> {
        check_jira_for_task(&bb.current_task_id)
    }
}

// Register at startup
registry.register("jiraTicketExists", JiraTicketExists::new);
```

Use in DSL:

```yaml
if:
  kind: jiraTicketExists
```

### 11.2 Custom Output Parsers

```rust
pub struct XmlParser {
    pub xpath: String,
}

impl OutputParser for XmlParser {
    fn parse(&self, raw: &str) -> Result<HashMap<String, BlackboardValue>, ParseError> {
        let doc = roxmltree::Document::parse(raw)?;
        // XPath evaluation...
        Ok(result)
    }
}

registry.register("xml", XmlParser::new);
```

Use in DSL:

```yaml
parser:
  kind: xml
  xpath: "/response/decision"
```

### 11.3 Custom Template Filters

```rust
registry.register_filter("kebab_case", |input: &str| {
    input.to_lowercase().replace(' ', "-")
});
```

---

## 12. DSL Design Principles

1. **Simple things simple**: Common decisions (if X then Y) are one rule. No ceremony.
2. **Progressive disclosure**: Start with DecisionRules. Use BehaviorTree nodes only when needed.
3. **Explicit over implicit**: Every node type and rule field is explicit. No hidden defaults.
4. **Fail fast at load time**: Invalid DSL is rejected when loaded, not when executed.
5. **Composability**: Sub-trees are first-class. Complex logic is built from simple blocks with scoped state.
6. **Human-readable**: YAML is chosen for readability. Rule lists map to how humans think about decision logic.
7. **LLM-friendly**: Prompt templates are first-class. The DSL makes it easy to write, version, and test prompts.
8. **Backward compatible**: `apiVersion` enables migration. The AST compilation target remains stable.
9. **Observable**: Every rule and node produces traces. SubTree identity is preserved for debugging.
10. **Portable**: The DSL engine is zero-dependency. It compiles to the same AST regardless of authoring style.

---

## Appendix: Desugaring Reference

| High-Level Construct | Desugars To |
|---------------------|-------------|
| `DecisionRules { rules }` | `Selector(rule[1], rule[2], ..., rule[n])` |
| `Rule { if, then, cooldownMs }` | `Cooldown(Sequence(Condition(if), then.desugar()))` (cooldown omitted if 0) |
| `Rule { if, then, reflectionMaxRounds }` | `ReflectionGuard(Sequence(Condition(if), then.desugar()))` (guard omitted if 0) |
| `Switch { on: prompt, cases }` | `Sequence(Prompt(on), Selector(for each case: When(if: var==case, then: case.command), DefaultCase))` |
| `Switch { on: variable, cases }` | `Selector(for each case: When(if: var==case, then: case.command), DefaultCase)` |
| `When { if, then }` | `Sequence(Condition(if), Action(then.command))` |
| `Pipeline { steps }` | `Sequence(for each step: if → Condition; then → Action)` |
| `then: { command: ... }` (inline) | `Action(command)` |

---

> Document version: 2.0
> Last updated: 2026-04-24
