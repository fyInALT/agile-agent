# Decision DSL Examples

> A collection of behavior tree examples for the decision layer, ordered from simple to complex. Each example is a self-contained folder with a `tree.yaml` and a `README.md` explaining the decision logic.
>
> **Premise**: All decisions are made within the **same codex/claude session** as the work agent. The decision layer injects formatted prompts into the ongoing conversation and interprets the LLM's reply — no separate API calls.

---

## Quick Start

Read the examples in order:

1. [01-hello-decision](01-hello-decision/) — The simplest tree: a Selector with two branches.
2. [02-rate-limit-handler](02-rate-limit-handler/) — Introduces Decorators with the `Cooldown` node.
3. [03-reflect-loop](03-reflect-loop/) — The canonical Prompt node example with `ReflectionGuard`.
4. [04-error-recovery](04-error-recovery/) — Thinking-model classification with structured parsing.
5. [05-task-lifecycle](05-task-lifecycle/) — Phase-aware routing across the full task lifecycle.

---

## Example Map

| Example | Complexity | New Concepts | Nodes Used |
|---------|-----------|--------------|------------|
| **01 Hello Decision** | ⭐ | Selector, Sequence, Condition, Action | Selector, Sequence, Condition, Action |
| **02 Rate Limit** | ⭐⭐ | Decorator (Cooldown), Regex condition, SetVar | Selector, Sequence, Condition, Cooldown, Action, SetVar |
| **03 Reflect Loop** | ⭐⭐⭐ | Prompt node, ReflectionGuard, variable branching, SubTree | Selector, Sequence, Condition, ReflectionGuard, Prompt, Action, SubTree |
| **04 Error Recovery** | ⭐⭐⭐⭐ | Structured parser, multi-field branching, OR condition | Sequence, Selector, Condition, Prompt, Action |
| **05 Task Lifecycle** | ⭐⭐⭐⭐⭐ | Phase-aware routing, cross-example SubTree reuse, stateful transitions | Selector, Sequence, Condition, Prompt, Action, SetVar, SubTree |

---

## Common Patterns

### Pattern 1: Guard + Action (Sequence)

```yaml
kind: Sequence
name: guarded_action
children:
  - kind: Condition
    name: guard
    eval: { kind: outputContains, pattern: "some_pattern" }
  - kind: Action
    name: action
    command: SomeCommand
```

The Action only executes if the Condition succeeds. This is the behavior tree equivalent of `if (guard) { action(); }`.

### Pattern 2: Priority Fallback (Selector)

```yaml
kind: Selector
name: priority_handler
children:
  - kind: Sequence
    name: high_priority
    children: [...]
  - kind: Sequence
    name: medium_priority
    children: [...]
  - kind: Action
    name: default
    command: DefaultCommand
```

The Selector tries children left-to-right. The leftmost child is the highest priority. The rightmost child is the unconditional default.

### Pattern 3: Prompt + Branch (the core decision pattern)

```yaml
kind: Sequence
name: prompt_then_branch
children:
  - kind: Prompt
    name: ask_llm
    template: "...Should we do A or B?"
    parser: { kind: enum, values: [A, B] }
    sets:
      - key: decision
        field: decision
  - kind: Selector
    name: branch
    children:
      - kind: Sequence
        name: do_a
        children:
          - kind: Condition
            name: is_a
            eval: { kind: variableIs, key: decision, value: A }
          - kind: Action
            name: emit_a
            command: CommandA
      - kind: Sequence
        name: do_b
        children:
          - kind: Condition
            name: is_b
            eval: { kind: variableIs, key: decision, value: B }
          - kind: Action
            name: emit_b
            command: CommandB
```

Send a prompt to the **same** codex/claude session, parse the reply, then branch based on the parsed value. This is the core pattern for LLM-driven decision making.

### Pattern 4: Stateful Limit (Decorator)

```yaml
kind: ReflectionGuard
name: max_3_attempts
maxRounds: 3
child:
  kind: Prompt
  name: ask_question
    template: "..."
```

The ReflectionGuard tracks state across ticks and blocks the child after N executions. This prevents infinite loops without any global state management.

---

## Running the Examples

These YAML files are **declarative specifications**. They are not executable Rust code. To use them:

1. Load the YAML with `DslLoader::load_tree("example_name")`.
2. The loader validates the tree against the JSON Schema.
3. The executor ticks the tree's root node against a populated `Blackboard`.
4. The resulting `DecisionCommand` values are returned to the runtime.

See `docs/decision-dsl.md` for the full DSL specification and `docs/decision-layer-design.md` for the behavior tree architecture.

---

## File Layout

```
docs/decision-dsl-examples/
├── README.md                           # This file
├── 01-hello-decision/
│   ├── README.md                       # Explanation and traces
│   └── tree.yaml                       # The behavior tree
├── 02-rate-limit-handler/
│   ├── README.md
│   └── tree.yaml
├── 03-reflect-loop/
│   ├── README.md
│   ├── tree.yaml                       # Root tree (uses SubTree)
│   └── subtrees/
│       └── reflect.yaml                # Reusable reflect sub-tree
├── 04-error-recovery/
│   ├── README.md
│   └── tree.yaml
└── 05-task-lifecycle/
    ├── README.md
    ├── tree.yaml                       # Root lifecycle orchestrator
    └── subtrees/
        └── task-start.yaml             # Git branch selection
```

---

> These examples are designed to be read, modified, and extended. Start with 01, modify the Condition pattern or Action command, and observe how the execution trace changes. Then move to 03 and experiment with the Prompt template — the LLM's reply (within the same session) determines the entire branch of the tree.
