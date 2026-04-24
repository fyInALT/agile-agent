# Example 01: Hello Decision

> The simplest possible behavior tree. This example demonstrates the core concepts: Selector priority, Condition checking, and Action emission.

---

## What This Tree Does

The agent receives provider output from Claude/Codex. The tree decides whether the agent is waiting for a human choice, or should simply continue working.

**Decision Logic** (in plain English):

1. If the provider output contains "waiting for choice", select the first option and continue.
2. Otherwise, just approve and continue.

This is a two-branch Selector. The first branch is a guard + action sequence. The second branch is the unconditional default.

---

## The Behavior Tree

```text
[Selector "root"]
│
├── [Sequence "handle_choice"]
│   ├── [Condition "is_waiting_for_choice"]
│   │       checks: provider_output contains "waiting for choice"
│   └── [Action "select_first"]
│           emits: SelectOption { option_id: "0" }
│
└── [Action "default_continue"]
        emits: ApproveAndContinue
```

### Execution Traces

**Trace A — Provider output contains "waiting for choice"**:

```
[Selector "root"] → ticks child 0
  [Sequence "handle_choice"] → ticks child 0
    [Condition "is_waiting_for_choice"] → reads provider_output → finds pattern → SUCCESS
  [Sequence "handle_choice"] → child 0 succeeded, ticks child 1
    [Action "select_first"] → pushes SelectOption to commands → SUCCESS
  [Sequence "handle_choice"] → ALL children succeeded → SUCCESS
[Selector "root"] → first child succeeded → SUCCESS

Result: [SelectOption { option_id: "0" }]
```

**Trace B — Provider output is normal completion text**:

```
[Selector "root"] → ticks child 0
  [Sequence "handle_choice"] → ticks child 0
    [Condition "is_waiting_for_choice"] → reads provider_output → no pattern → FAILURE
  [Sequence "handle_choice"] → first child failed → FAILURE
[Selector "root"] → child 0 failed, tries child 1
  [Action "default_continue"] → pushes ApproveAndContinue → SUCCESS
[Selector "root"] → child 1 succeeded → SUCCESS

Result: [ApproveAndContinue]
```

---

## Key Concepts Demonstrated

| Concept | Node | Explanation |
|---------|------|-------------|
| **Selector priority** | `root` | Children are ordered by priority. Leftmost = highest priority. |
| **Guard + Action** | `handle_choice` | A Sequence ensures the action only runs if the guard condition passes. |
| **Default fallback** | `default_continue` | The rightmost child of a Selector is often the "default" that always succeeds. |
| **Blackboard read** | `is_waiting_for_choice` | The Condition reads `provider_output` from the Blackboard. |
| **Command emission** | `select_first`, `default_continue` | Action nodes push `DecisionCommand` values to the Blackboard's `commands` list. |

---

## Conversation Flow

Below is a realistic session between the user, the work agent (Claude/Codex), and the decision layer.

---

### Scenario A: Agent asks for a choice

```
👤 user:
  Create a REST API with Flask

🤖 worker:
  I'll create a Flask REST API for you.
  [writes app.py with route stubs]
  [writes requirements.txt]

  Which URL style would you prefer?
  A) /users/<id>     (RESTful path parameter)
  B) /users?id=<id>  (Query parameter)

  Please choose: A or B
  ───────────────────────────────
  (worker output contains: "waiting for choice")

🧠 decision:
  ┌─ Condition: is_waiting_for_choice ─┐
  │  provider_output contains pattern   │
  │  → SUCCESS                          │
  └─────────────────────────────────────┘
  ↓
  Command: SelectOption { option_id: "0" }   ← auto-select first option

🤖 worker:
  Option A selected. Using /users/<id>.
  [updates route definitions in app.py]
  Done! Flask API is ready with RESTful routes.
  ───────────────────────────────
  (no "waiting for choice" in output)

🧠 decision:
  ┌─ Condition: is_waiting_for_choice ─┐
  │  pattern NOT found                  │
  │  → FAILURE                          │
  └─────────────────────────────────────┘
  ↓
  Fallback → Command: ApproveAndContinue

🤖 worker:
  [continues with next task or idles]
```

### Scenario B: Normal completion (no choice needed)

```
👤 user:
  Write a hello world in Python

🤖 worker:
  I'll write a hello world script for you.
  [writes hello.py: print("Hello, World!")]
  Done! The script is ready.
  ───────────────────────────────
  (no "waiting for choice" in output)

🧠 decision:
  ┌─ Condition: is_waiting_for_choice ─┐
  │  pattern NOT found                  │
  │  → FAILURE                          │
  └─────────────────────────────────────┘
  ↓
  Fallback → Command: ApproveAndContinue

🤖 worker:
  [continues or idles]
```

---

## Files

- `tree.yaml` — The behavior tree definition.

---

## DecisionRules Shorthand

The same logic can be expressed more concisely using the DecisionRules format (see `decision-dsl.md` §9):

```yaml
apiVersion: decision.agile-agent.io/v1
kind: DecisionRules
metadata:
  name: hello_decision
spec:
  rules:
    - priority: 1
      name: handle_choice
      if:
        kind: outputContains
        pattern: "waiting for choice"
      then:
        command:
          SelectOption:
            option_id: "0"
    - priority: 99
      name: default_continue
      then:
        command: ApproveAndContinue
```

This desugars to the same BehaviorTree AST shown above.
