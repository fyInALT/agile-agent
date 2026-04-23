# Decision DSL Specification

> This document defines the YAML-based DSL for authoring behavior trees in the decision layer. A behavior tree is a hierarchical structure of nodes that the `BehaviorTreeExecutor` ticks to produce `DecisionCommand` values.

---

## 1. DSL Overview

The decision DSL is a **declarative YAML format** for defining behavior trees. Each file defines one tree or one reusable sub-tree.

### File Structure

```
decisions/
├── trees/
│   └── default.yaml              # The root decision tree
├── subtrees/
│   ├── rate_limit.yaml           # Reusable rate-limit handler
│   ├── reflect_loop.yaml         # Reusable reflect loop
│   └── human_escalation.yaml     # Reusable human escalation
└── schemas/
    └── action_schemas.yaml       # Action parameter schemas
```

### Top-Level Format

Every DSL file has the same top-level structure:

```yaml
apiVersion: decision.agile-agent.io/v1
kind: BehaviorTree          # or SubTree
metadata:
  name: default_decision_tree
  description: "Handles all common decision scenarios"
spec:
  root:
    kind: Selector
    name: root_handler
    children:
      - ...
```

| Field | Required | Description |
|-------|----------|-------------|
| `apiVersion` | Yes | Schema version for migration. Format: `decision.agile-agent.io/v{N}`. |
| `kind` | Yes | `BehaviorTree` (has executor) or `SubTree` (reusable unit). |
| `metadata.name` | Yes | Unique identifier for the tree. |
| `metadata.description` | No | Human-readable description. |
| `spec.root` | Yes | The root node of the tree. |

---

## 2. Node YAML Schema

### 2.1 Common Fields

Every node has these common fields:

```yaml
kind: Selector      # Node type
name: my_node       # Unique name within the tree
```

| Field | Required | Description |
|-------|----------|-------------|
| `kind` | Yes | Node type. See tables below. |
| `name` | Yes | Unique name for tracing and debugging. |

### 2.2 Composite Nodes

#### Selector

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

```yaml
kind: Sequence
name: reflect_loop
children:
  - kind: Condition
    name: is_claims_completion
    eval:
      kind: outputContains
      pattern: "claims_completion"
  - kind: Prompt
    name: ask_reflect_or_confirm
    template: |
      Should the agent reflect or confirm?
      Reply: REFLECT or CONFIRM
    parser:
      kind: enum
      values: [REFLECT, CONFIRM]
    sets:
      - key: next_action
        field: decision
  - kind: Action
    name: emit_reflect
    command:
      Reflect:
        prompt: "Review your work carefully"
    when:
      kind: variableIs
      key: next_action
      value: REFLECT
```

| Field | Required | Description |
|-------|----------|-------------|
| `children` | Yes | List of child nodes. All must succeed. |

#### Parallel

```yaml
kind: Parallel
name: safety_checks
policy: allSuccess          # or anySuccess, majority
children:
  - kind: Condition
    name: check_dangerous
    eval: { kind: script, script: "is_dangerous(provider_output)" }
  - kind: Condition
    name: check_main_branch
    eval: { kind: script, script: "branch == 'main'" }
```

| Field | Required | Description |
|-------|----------|-------------|
| `policy` | Yes | `allSuccess`, `anySuccess`, or `majority`. |
| `children` | Yes | List of child nodes. Executed concurrently. |

### 2.3 Decorator Nodes

#### Inverter

```yaml
kind: Inverter
name: not_rate_limit
child:
  kind: Condition
  name: is_rate_limit
  eval: { kind: outputContains, pattern: "429" }
```

| Field | Required | Description |
|-------|----------|-------------|
| `child` | Yes | Single child node. |

#### Repeater

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

| Field | Required | Description |
|-------|----------|-------------|
| `maxAttempts` | Yes | Maximum number of repetitions. |
| `child` | Yes | Single child node. |

#### Cooldown

```yaml
kind: Cooldown
name: rate_limit_cooldown
durationMs: 5000
child:
  kind: Action
  name: retry_tool
  command:
    RetryTool:
      tool_name: "{{last_tool_name}}"
      cooldown_ms: 5000
```

| Field | Required | Description |
|-------|----------|-------------|
| `durationMs` | Yes | Cooldown duration in milliseconds. |
| `child` | Yes | Single child node. |

#### ReflectionGuard

```yaml
kind: ReflectionGuard
name: max_2_reflections
maxRounds: 2
child:
  kind: Prompt
  name: reflect_or_confirm
  template: "..."
```

| Field | Required | Description |
|-------|----------|-------------|
| `maxRounds` | Yes | Maximum reflection rounds. |
| `child` | Yes | Single child node. |

#### ForceHuman

```yaml
kind: ForceHuman
name: force_human_on_pr
reason: "PR submission requires human approval"
child:
  kind: Prompt
  name: confirm_pr
  template: "Should we submit this PR? YES/NO"
  parser: { kind: enum, values: [YES, NO] }
```

| Field | Required | Description |
|-------|----------|-------------|
| `reason` | Yes | Reason shown to the human. |
| `child` | Yes | Single child node. |

### 2.4 Leaf Nodes

#### Condition

```yaml
kind: Condition
name: is_rate_limit
eval:
  kind: outputContains
  pattern: "429"
```

| Field | Required | Description |
|-------|----------|-------------|
| `eval` | Yes | Evaluator specification. See Condition Evaluators below. |

**Condition Evaluators**:

```yaml
# OutputContains — checks if provider_output contains a pattern
kind: outputContains
pattern: "429"
caseSensitive: false          # default: false

# SituationIs — checks if context_summary contains the situation type
kind: situationIs
type: claims_completion

# ReflectionRoundUnder — checks if reflection_round < max
kind: reflectionRoundUnder
max: 2

# VariableIs — checks a custom blackboard variable
kind: variableIs
key: next_action
value: REFLECT

# Script — evaluates a Rhai script
kind: script
script: |
  blackboard.reflection_round < 2 &&
  blackboard.provider_output.contains("claims_completion")

# Regex — matches provider_output against a regex
kind: regex
pattern: "ACTION:\\s*(\\w+)"
```

#### Action

```yaml
kind: Action
name: emit_reflect
command:
  Reflect:
    prompt: "Review your work carefully"
```

| Field | Required | Description |
|-------|----------|-------------|
| `command` | Yes | A `DecisionCommand` variant. See Command Reference below. |
| `when` | No | Optional condition. Action only executes if condition is true. |

**Command Reference** (full `DecisionCommand` enum):

```yaml
# Escalate to human
command:
  EscalateToHuman:
    reason: "Need human approval"
    context: "The agent wants to delete files"

# Retry a failed tool
command:
  RetryTool:
    tool_name: "Bash"
    args: null
    max_attempts: 3

# Send custom instruction
command:
  SendCustomInstruction:
    prompt: "Focus on fixing the tests"
    target_agent: "agent-42"

# Approve and continue
command: ApproveAndContinue

# Confirm completion
command: ConfirmCompletion

# Reflect
command:
  Reflect:
    prompt: "Review your changes for bugs"

# Stop if complete
command:
  StopIfComplete:
    reason: "All tasks finished"

# Prepare task start
command:
  PrepareTaskStart:
    task_id: "TASK-123"
    task_description: "Implement user authentication"

# Suggest commit
command:
  SuggestCommit:
    message: "Add auth module"
    mandatory: false
    reason: "Good checkpoint"

# Commit changes
command:
  CommitChanges:
    message: "WIP: auth"
    is_wip: true
    worktree_path: null

# Create task branch
command:
  CreateTaskBranch:
    branch_name: "feature/auth"
    base_branch: "main"
    worktree_path: null

# Rebase to main
command:
  RebaseToMain:
    base_branch: "main"
```

#### Prompt

```yaml
kind: Prompt
name: reflect_or_confirm
model: standard          # or "thinking" for reasoning models
timeoutMs: 30000
template: |
  You are a decision helper for a software development agent.

  ## Task
  {{ task_description }}

  ## Recent Work
  {{ context_summary }}

  ## Current Output
  {{ provider_output }}

  ## Reflection Round
  {{ reflection_round }} / {{ max_reflection_rounds }}

  Should the agent reflect on its work or confirm completion?
  Reply with exactly one word: REFLECT or CONFIRM.
parser:
  kind: enum
  values: [REFLECT, CONFIRM]
  caseSensitive: false
sets:
  - key: next_action
    field: decision
```

| Field | Required | Description |
|-------|----------|-------------|
| `model` | No | LLM model tier: `standard` (default) or `thinking`. |
| `timeoutMs` | No | Timeout in milliseconds. Default: 30000. |
| `template` | Yes | Jinja2-style template. See Template Syntax below. |
| `parser` | Yes | Output parser specification. See Output Parsers below. |
| `sets` | No | List of `(blackboard_key, parser_field)` mappings. |

#### SetVar

```yaml
kind: SetVar
name: init_counter
key: retry_count
value:
  kind: integer
  value: 0
```

| Field | Required | Description |
|-------|----------|-------------|
| `key` | Yes | Blackboard variable name. |
| `value` | Yes | Typed value. See Value Types below. |

**Value Types**:

```yaml
# String
value: { kind: string, value: "hello" }

# Integer
value: { kind: integer, value: 42 }

# Float
value: { kind: float, value: 0.85 }

# Boolean
value: { kind: boolean, value: true }

# List
value:
  kind: list
  items:
    - { kind: string, value: "a" }
    - { kind: string, value: "b" }
```

### 2.5 SubTree Reference

```yaml
kind: SubTree
name: use_reflect_loop
ref: reflect_loop              # References subtrees/reflect_loop.yaml
```

| Field | Required | Description |
|-------|----------|-------------|
| `ref` | Yes | Name of the sub-tree to execute. |

---

## 3. Blackboard Variables

### 3.1 Built-in Variables

These variables are automatically populated by the executor before each tick:

| Variable | Type | Description |
|----------|------|-------------|
| `task_description` | string | The task description given to the work agent. |
| `provider_output` | string | Raw output from the LLM provider (Claude/Codex). |
| `context_summary` | string | Condensed summary of recent tool calls and file changes. |
| `reflection_round` | integer | Current reflection round (0, 1, 2, ...). |
| `max_reflection_rounds` | integer | Maximum allowed reflection rounds. |
| `confidence_accumulator` | float | Accumulated confidence score. |
| `last_tool_call` | object | Structured record of the last tool call (name, input, output). |
| `file_changes` | list | List of recent file changes (path, change_type). |
| `project_rules` | object | Parsed project rules from CLAUDE.md / AGENTS.md. |
| `decision_history` | list | List of recent decision records. |
| `agent_id` | string | ID of the work agent being decided for. |
| `current_task_id` | string | Current task ID, if any. |
| `current_story_id` | string | Current story ID, if any. |

### 3.2 Custom Variables

Nodes can read and write custom variables via `SetVar` and `sets`:

```yaml
# Write
kind: SetVar
name: set_flag
key: needs_review
value: { kind: boolean, value: true }

# Read in template
# {{ variables.needs_review }}

# Read in condition
kind: Condition
name: check_flag
eval:
  kind: variableIs
  key: needs_review
  value: true
```

Custom variables are scoped to the current decision cycle. They do not persist between ticks.

### 3.3 Variable Interpolation in Commands

Commands can interpolate blackboard variables using `{{variable}}` syntax:

```yaml
kind: Action
name: retry_last_tool
command:
  RetryTool:
    tool_name: "{{ last_tool_call.name }}"
    args: "{{ last_tool_call.input }}"
    max_attempts: 3
```

Interpolation happens at Action node execution time, not at DSL load time.

---

## 4. Prompt Template Syntax

Prompt templates use **Jinja2-style** syntax with the Blackboard as the template context.

### 4.1 Variable Interpolation

```text
{{ task_description }}
{{ provider_output }}
{{ reflection_round }}
{{ variables.my_custom_var }}
```

### 4.2 Filters

```text
{{ provider_output | truncate(500) }}
{{ file_changes | length }}
{{ task_description | upper }}
{{ context_summary | default("No summary available") }}
```

| Filter | Description |
|--------|-------------|
| `truncate(n)` | Truncate to N characters, append "...". |
| `length` | Length of list or string. |
| `upper` / `lower` | Case conversion. |
| `default(val)` | Use default if variable is missing or empty. |
| `join(sep)` | Join list elements with separator. |

### 4.3 Conditionals

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

### 4.4 Whitespace Control

```text
{%- if true -%}  {#- strips leading/trailing whitespace -#}
no extra whitespace
{%- endif -%}
```

### 4.5 Best Practices

1. **Always include output format instructions**: Tell the LLM exactly what to return.
2. **Use `truncate` on large inputs**: `provider_output` can be very long.
3. **Use `default` for optional variables**: Prevent template errors.
4. **Keep templates under 2000 tokens**: The executor warns if a rendered prompt exceeds the budget.
5. **Use comments for complex logic**: `{# This prompt asks the LLM to choose a strategy #}`

---

## 5. Output Parsers

### 5.1 Enum Parser

Parse a single value from a constrained set:

```yaml
parser:
  kind: enum
  values: [REFLECT, CONFIRM, ESCALATE]
  caseSensitive: false
```

**Input**: `"  reflect  "` → **Parsed**: `{ decision: "REFLECT" }`
**Input**: `"maybe"` → **Failure**: Unexpected value

### 5.2 Structured Parser

Parse fields from text using regex:

```yaml
parser:
  kind: structured
  pattern: "ACTION:\\s*(\\w+)\\s*CONFIDENCE:\\s*(\\d+\\.\\d+)"
  fields:
    - name: action
      group: 1
    - name: confidence
      group: 2
      type: float
```

**Input**: `ACTION: reflect CONFIDENCE: 0.85` → **Parsed**: `{ action: "reflect", confidence: 0.85 }`

### 5.3 JSON Parser

Parse JSON responses:

```yaml
parser:
  kind: json
  schema:            # Optional JSON Schema for validation
    type: object
    properties:
      decision:
        type: string
        enum: [reflect, confirm]
      reasoning:
        type: string
      confidence:
        type: number
        minimum: 0
        maximum: 1
    required: [decision]
```

**Input**: `{"decision": "reflect", "reasoning": "Need to verify tests", "confidence": 0.82}`
→ **Parsed**: `{ decision: "reflect", reasoning: "Need to verify tests", confidence: 0.82 }`

### 5.4 Command Parser

Parse directly into a `DecisionCommand`:

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
    ESCALATE:
      command: EscalateToHuman
      params:
        reason: "LLM chose escalation"
```

This parser is special: it does not write to the Blackboard. It directly appends a `DecisionCommand` to `blackboard.commands`.

---

## 6. Complete Examples

### Example 1: Default Decision Tree

This is the root tree that handles all common scenarios. It uses a Selector to try handlers in priority order.

```yaml
# trees/default.yaml
apiVersion: decision.agile-agent.io/v1
kind: BehaviorTree
metadata:
  name: default_decision_tree
  description: "Handles rate limits, human escalation, reflect loops, errors, and defaults"
spec:
  root:
    kind: Selector
    name: root_handler
    children:
      # Priority 1: Rate limit
      - kind: Sequence
        name: handle_rate_limit
        children:
          - kind: Condition
            name: is_rate_limit
            eval:
              kind: outputContains
              pattern: "429"
          - kind: Cooldown
            name: rate_limit_cooldown
            durationMs: 5000
            child:
              kind: Action
              name: retry_with_backoff
              command:
                RetryTool:
                  tool_name: "{{ last_tool_call.name }}"
                  args: null
                  max_attempts: 3

      # Priority 2: Dangerous action → human
      - kind: Sequence
        name: handle_dangerous
        children:
          - kind: Condition
            name: is_dangerous
            eval:
              kind: script
              script: |
                let dangerous = ["delete_files", "force_push", "drop_database"];
                dangerous.any(|d| blackboard.provider_output.contains(d))
          - kind: Action
            name: escalate_human
            command:
              EscalateToHuman:
                reason: "Dangerous action detected"
                context: "{{ provider_output | truncate(200) }}"

      # Priority 3: Claims completion → reflect loop
      - kind: SubTree
        name: use_reflect_loop
        ref: reflect_loop

      # Priority 4: Error recovery
      - kind: Sequence
        name: handle_error
        children:
          - kind: Condition
            name: is_error
            eval:
              kind: outputContains
              pattern: "error"
          - kind: Prompt
            name: error_strategy
            model: standard
            template: |
              The agent encountered an error:
              {{ provider_output | truncate(500) }}

              What should we do?
              Reply with one: RETRY, ESCALATE, or CONTINUE
            parser:
              kind: enum
              values: [RETRY, ESCALATE, CONTINUE]
            sets:
              - key: error_strategy
                field: decision
          - kind: Selector
            name: branch_on_strategy
            children:
              - kind: Sequence
                name: do_retry
                children:
                  - kind: Condition
                    name: strategy_is_retry
                    eval:
                      kind: variableIs
                      key: error_strategy
                      value: RETRY
                  - kind: Action
                    name: emit_retry
                    command:
                      RetryTool:
                        tool_name: "{{ last_tool_call.name }}"
                        args: null
                        max_attempts: 3
              - kind: Sequence
                name: do_escalate
                children:
                  - kind: Condition
                    name: strategy_is_escalate
                    eval:
                      kind: variableIs
                      key: error_strategy
                      value: ESCALATE
                  - kind: Action
                    name: emit_escalate
                    command:
                      EscalateToHuman:
                        reason: "Error recovery chose escalation"
                        context: "{{ provider_output | truncate(300) }}"
              - kind: Sequence
                name: do_continue
                children:
                  - kind: Condition
                    name: strategy_is_continue
                    eval:
                      kind: variableIs
                      key: error_strategy
                      value: CONTINUE
                  - kind: Action
                    name: emit_continue
                    command: ApproveAndContinue

      # Priority 5: Default
      - kind: Action
        name: default_continue
        command: ApproveAndContinue
```

### Example 2: Reflect Loop Sub-Tree

```yaml
# subtrees/reflect_loop.yaml
apiVersion: decision.agile-agent.io/v1
kind: SubTree
metadata:
  name: reflect_loop
  description: "Handles claims_completion with up to 2 reflection rounds"
spec:
  root:
    kind: Sequence
    name: reflect_loop
    children:
      - kind: Condition
        name: is_claims_completion
        eval:
          kind: outputContains
          pattern: "claims_completion"

      - kind: ReflectionGuard
        name: max_2_reflections
        maxRounds: 2
        child:
          kind: Prompt
          name: ask_reflect_or_confirm
            model: thinking
            template: |
              You are a decision helper for a software development agent.

              ## Task
              {{ task_description }}

              ## Recent Work Summary
              {{ context_summary | truncate(800) }}

              ## Agent's Claim
              {{ provider_output | truncate(500) }}

              ## Reflection Round
              {{ reflection_round }} / {{ max_reflection_rounds }}

              The agent claims the task is complete.
              Should we:
              - REFLECT: Ask the agent to review its work more carefully
              - CONFIRM: Accept the completion and move on

              Consider: Are tests passing? Are all requirements met? Is the code clean?
              Reply with exactly one word: REFLECT or CONFIRM.
            parser:
              kind: enum
              values: [REFLECT, CONFIRM]
              caseSensitive: false
            sets:
              - key: next_action
                field: decision

      - kind: Selector
        name: branch_on_decision
        children:
          - kind: Sequence
            name: do_reflect
            children:
              - kind: Condition
                name: decision_is_reflect
                eval:
                  kind: variableIs
                  key: next_action
                  value: REFLECT
              - kind: Action
                name: emit_reflect
                command:
                  Reflect:
                    prompt: |
                      Please review your work carefully before confirming completion:
                      1. Are all tests passing?
                      2. Have you reviewed all changed files?
                      3. Does the code follow project conventions?

          - kind: Sequence
            name: do_confirm
            children:
              - kind: Condition
                name: decision_is_confirm
                eval:
                  kind: variableIs
                  key: next_action
                  value: CONFIRM
              - kind: Action
                name: emit_confirm
                command: ConfirmCompletion
```

### Example 3: Rate Limit Handler Sub-Tree

```yaml
# subtrees/rate_limit.yaml
apiVersion: decision.agile-agent.io/v1
kind: SubTree
metadata:
  name: rate_limit_handler
  description: "Handles 429/rate-limit responses with exponential backoff"
spec:
  root:
    kind: Sequence
    name: rate_limit_handler
    children:
      - kind: Condition
        name: is_rate_limit
        eval:
          kind: regex
          pattern: "(429|rate.?limit|quota.?exceeded)"

      - kind: SetVar
        name: set_backoff
        key: backoff_ms
        value:
          kind: integer
          value: 5000

      - kind: Cooldown
        name: wait_before_retry
        durationMs: "{{ variables.backoff_ms }}"
        child:
          kind: Action
          name: emit_retry
          command:
            RetryTool:
              tool_name: "{{ last_tool_call.name | default('unknown') }}"
              args: null
              max_attempts: 1
```

### Example 4: Human Escalation Sub-Tree

```yaml
# subtrees/human_escalation.yaml
apiVersion: decision.agile-agent.io/v1
kind: SubTree
metadata:
  name: human_escalation
  description: "Escalates to human with context"
spec:
  root:
    kind: Sequence
    name: human_escalation
    children:
      - kind: Prompt
        name: summarize_for_human
        model: standard
        template: |
          Summarize the current situation for a human reviewer in 2 sentences:

          Task: {{ task_description | truncate(200) }}
          Agent output: {{ provider_output | truncate(300) }}
          Recent changes: {{ context_summary | truncate(200) }}
        parser:
          kind: structured
          pattern: "(.*)"
          fields:
            - name: summary
              group: 1
        sets:
          - key: human_summary
            field: summary

      - kind: Action
        name: escalate
        command:
          EscalateToHuman:
            reason: "Decision tree escalated"
            context: "{{ variables.human_summary }}"
```

### Example 5: Task Start Sub-Tree

```yaml
# subtrees/task_start.yaml
apiVersion: decision.agile-agent.io/v1
kind: SubTree
metadata:
  name: task_start
  description: "Prepares git branch and worktree for a new task"
spec:
  root:
    kind: Sequence
    name: task_start
    children:
      - kind: Condition
        name: is_task_starting
        eval:
          kind: outputContains
          pattern: "task_starting"

      - kind: Prompt
        name: choose_git_strategy
        model: standard
        template: |
          Starting task: {{ current_task_id }}
          Description: {{ task_description | truncate(300) }}

          Choose git strategy:
          - NEW_BRANCH: Create a new feature branch
          - EXISTING: Use existing branch if available
          Reply: NEW_BRANCH or EXISTING
        parser:
          kind: enum
          values: [NEW_BRANCH, EXISTING]
        sets:
          - key: git_strategy
            field: decision

      - kind: Selector
        name: branch_on_strategy
        children:
          - kind: Sequence
            name: new_branch_flow
            children:
              - kind: Condition
                name: strategy_is_new
                eval:
                  kind: variableIs
                  key: git_strategy
                  value: NEW_BRANCH
              - kind: Action
                name: create_branch
                command:
                  CreateTaskBranch:
                    branch_name: "feature/{{ current_task_id | slugify }}"
                    base_branch: "main"
                    worktree_path: null
              - kind: Action
                name: prepare_start
                command:
                  PrepareTaskStart:
                    task_id: "{{ current_task_id }}"
                    task_description: "{{ task_description }}"

          - kind: Sequence
            name: existing_branch_flow
            children:
              - kind: Condition
                name: strategy_is_existing
                eval:
                  kind: variableIs
                  key: git_strategy
                  value: EXISTING
              - kind: Action
                name: rebase
                command:
                  RebaseToMain:
                    base_branch: "main"
              - kind: Action
                name: prepare_start
                command:
                  PrepareTaskStart:
                    task_id: "{{ current_task_id }}"
                    task_description: "{{ task_description }}"
```

### Example 6: Complex Error Recovery

```yaml
# trees/error_recovery.yaml
apiVersion: decision.agile-agent.io/v1
kind: BehaviorTree
metadata:
  name: error_recovery
  description: "Analyzes errors deeply and chooses recovery strategy"
spec:
  root:
    kind: Sequence
    name: error_recovery
    children:
      - kind: Condition
        name: is_error
        eval:
          kind: outputContains
          pattern: "error"

      - kind: Prompt
        name: classify_error
        model: thinking
        template: |
          The agent encountered an error. Classify it:

          Error output:
          {{ provider_output | truncate(600) }}

          Last tool call: {{ last_tool_call.name }}
          Tool input: {{ last_tool_call.input | default("N/A") }}
          Tool output: {{ last_tool_call.output | default("N/A") | truncate(300) }}

          Classify as one of:
          - SYNTAX: Code compilation/syntax error
          - TEST: Test failure
          - PERMISSION: File/network permission denied
          - TIMEOUT: Operation timed out
          - RATE_LIMIT: API rate limited
          - LOGIC: Logic bug in implementation
          - UNKNOWN: Cannot determine

          Then recommend: RETRY, FIX, SKIP, or ESCALATE

          Reply in this exact format:
          CLASS: <classification>
          RECOMMEND: <recommendation>
          REASON: <one sentence>
        parser:
          kind: structured
          pattern: "CLASS:\\s*(\\w+)\\s*RECOMMEND:\\s*(\\w+)\\s*REASON:\\s*(.*)"
          fields:
            - name: classification
              group: 1
            - name: recommendation
              group: 2
            - name: reason
              group: 3
        sets:
          - key: error_class
            field: classification
          - key: error_recommendation
            field: recommendation
          - key: error_reason
            field: reason

      - kind: Selector
        name: act_on_recommendation
        children:
          - kind: Sequence
            name: retry_flow
            children:
              - kind: Condition
                name: recommend_retry
                eval:
                  kind: variableIs
                  key: error_recommendation
                  value: RETRY
              - kind: Action
                name: emit_retry
                command:
                  RetryTool:
                    tool_name: "{{ last_tool_call.name }}"
                    args: null
                    max_attempts: 3

          - kind: Sequence
            name: fix_flow
            children:
              - kind: Condition
                name: recommend_fix
                eval:
                  kind: variableIs
                  key: error_recommendation
                  value: FIX
              - kind: Action
                name: emit_custom_instruction
                command:
                  SendCustomInstruction:
                    prompt: |
                      The agent encountered a {{ variables.error_class }} error.
                      Reason: {{ variables.error_reason }}
                      Please fix this issue and retry.
                    target_agent: "{{ agent_id }}"

          - kind: Sequence
            name: escalate_flow
            children:
              - kind: Condition
                name: recommend_escalate
                eval:
                  kind: or
                  conditions:
                    - kind: variableIs
                      key: error_recommendation
                      value: ESCALATE
                    - kind: variableIs
                      key: error_recommendation
                      value: SKIP
              - kind: Action
                name: emit_escalate
                command:
                  EscalateToHuman:
                    reason: "Error recovery: {{ variables.error_recommendation }}"
                    context: "{{ variables.error_reason }}"
```

---

## 7. DSL Loading & Validation

### 7.1 Loader

```rust
pub struct DslLoader {
    base_path: PathBuf,
    schema_validator: jsonschema::Validator,
}

impl DslLoader {
    pub fn load_tree(&self, name: &str) -> Result<BehaviorTree, DslError> {
        let path = self.base_path.join("trees").join(format!("{}.yaml", name));
        let raw = std::fs::read_to_string(&path)?;
        let doc: DslDocument = serde_yaml::from_str(&raw)?;

        // Validate against JSON Schema
        self.schema_validator.validate(&serde_json::to_value(&doc)?)?;

        // Check apiVersion
        if doc.api_version != "decision.agile-agent.io/v1" {
            return Err(DslError::UnsupportedVersion(doc.api_version));
        }

        // Resolve sub-tree references
        let tree = self.resolve_sub_trees(doc)?;

        // Validate node names are unique
        tree.validate_unique_names()?;

        // Validate all conditions reference known evaluators
        tree.validate_evaluators()?;

        Ok(tree)
    }
}
```

### 7.2 Validation Rules

1. **apiVersion must be supported**: The loader rejects unknown versions.
2. **All node names must be unique** within a tree.
3. **SubTree refs must resolve**: The referenced sub-tree file must exist.
4. **Condition evaluators must be registered**: Unknown `kind` values are rejected.
5. **Command variants must be valid**: Unknown `DecisionCommand` variants are rejected.
6. **Template variables must be resolvable**: The loader warns (not errors) on unknown variables.
7. **No circular SubTree references**: A → B → C → A is rejected.

### 7.3 Hot Reload

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
        match self.loader.load_tree("default") {
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

## 8. Extending the DSL

### 8.1 Custom Condition Evaluators

Register a custom evaluator in Rust:

```rust
pub struct JiraTicketExists;

impl ConditionEvaluator for JiraTicketExists {
    fn evaluate(&self, blackboard: &Blackboard) -> bool {
        // Call Jira API, check cache, etc.
        check_jira_for_task(&blackboard.current_task_id)
    }
}

// Register at startup
registry.register_condition("jiraTicketExists", || Box::new(JiraTicketExists));
```

Use in DSL:

```yaml
kind: Condition
name: check_jira
eval:
  kind: jiraTicketExists
```

### 8.2 Custom Output Parsers

```rust
pub struct XmlParser {
    pub xpath: String,
}

impl OutputParser for XmlParser {
    fn parse(&self, raw: &str) -> Result<HashMap<String, BlackboardValue>, ParseError> {
        let doc = roxmltree::Document::parse(raw)?;
        let mut result = HashMap::new();
        // XPath evaluation...
        Ok(result)
    }
}

registry.register_parser("xml", |params| Box::new(XmlParser { xpath: params["xpath"].clone() }));
```

Use in DSL:

```yaml
parser:
  kind: xml
  xpath: "/response/decision"
```

### 8.3 Custom Decorators

```rust
pub struct RateLimiter {
    pub max_per_minute: u32,
    pub counter: AtomicU32,
    pub window_start: Mutex<Instant>,
}

impl BehaviorNode for RateLimiter {
    fn tick(&mut self, blackboard: &mut Blackboard) -> NodeStatus {
        // Check rate limit, then delegate to child
        // ...
        self.child.tick(blackboard)
    }
}
```

Use in DSL:

```yaml
kind: RateLimiter
name: limit_prompt_calls
maxPerMinute: 10
child:
  kind: Prompt
  name: expensive_prompt
  template: "..."
```

### 8.4 Custom Template Filters

```rust
registry.register_filter("slugify", |input| {
    input.to_lowercase().replace(" ", "-").replace("_", "-")
});
```

Use in DSL:

```text
branch_name: "feature/{{ current_task_id | slugify }}"
```

---

## 9. DSL Design Principles

1. **Explicit over implicit**: Every node type is explicit. No hidden defaults.
2. **Fail fast at load time**: Invalid DSL is rejected when loaded, not when executed.
3. **Composability**: Sub-trees are first-class. Complex logic is built from simple blocks.
4. **Human-readable**: YAML is chosen for readability. The tree structure maps to English logic.
5. **LLM-friendly**: Prompt templates are first-class. The DSL makes it easy to write, version, and test prompts.
6. **Backward compatible**: `apiVersion` enables migration. Old trees continue to work during transition.

---

> Document version: v1.0
> Last updated: 2026-04-20
> Related document: `docs/decision-layer-design.md`
