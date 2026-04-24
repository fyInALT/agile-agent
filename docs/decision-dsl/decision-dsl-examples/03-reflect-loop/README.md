# Example 03: Reflect Loop

> The canonical behavior tree for the `claims_completion` situation. Demonstrates the **Prompt node** (the decision layer's most powerful primitive), the **ReflectionGuard** decorator, and variable-based branching.

---

## What This Tree Does

When the agent claims a task is complete, the decision layer should verify this claim before accepting it. The standard verification strategy is:

1. Allow up to 2 reflection rounds where the LLM decides whether to reflect or confirm.
2. On each reflection round, ask a reasoning model to judge: should the agent review its work, or is it truly done?
3. If the LLM says "reflect", emit a `Reflect` command and increment the reflection counter.
4. If the LLM says "confirm", emit a `ConfirmCompletion` command.
5. If max reflections (2) are reached, the ReflectionGuard blocks further reflection and the tree falls through to a final confirmation Prompt.

This example also demonstrates **SubTree reuse**: the reflect logic is extracted into a reusable `reflect` sub-tree that can be referenced from multiple parent trees.

---

## The Behavior Tree

```text
[Selector "root"]
│
├── [Sequence "claims_completion_flow"]
│   ├── [Condition "is_claims_completion"]
│   │       checks: provider_output contains "claims_completion"
│   │
│   ├── [ReflectionGuard "max_2_reflections" max=2]
│   │   └── [Prompt "ask_reflect_or_confirm"]
│   │           template: "...Should the agent REFLECT or CONFIRM?"
│   │           parser: Enum [REFLECT, CONFIRM]
│   │           sets: next_action = parsed.decision
│   │
│   └── [Selector "branch_on_decision"]
│       ├── [Sequence "do_reflect"]
│       │   ├── [Condition "decision_is_reflect"]
│       │   │       checks: variables.next_action == "REFLECT"
│       │   └── [Action "emit_reflect"]
│       │           emits: Reflect { prompt: "Review your work..." }
│       │
│       └── [Sequence "do_confirm"]
│           ├── [Condition "decision_is_confirm"]
│           │       checks: variables.next_action == "CONFIRM"
│           └── [Action "emit_confirm"]
│               emits: ConfirmCompletion
│
└── [Action "default_continue"]
        emits: ApproveAndContinue
```

### Execution Traces

**Trace A — First reflection round, LLM says "reflect"**:

```
[Selector "root"] → ticks child 0
  [Sequence "claims_completion_flow"] → ticks child 0
    [Condition "is_claims_completion"] → finds pattern → SUCCESS

  [Sequence "claims_completion_flow"] → ticks child 1
    [ReflectionGuard "max_2_reflections"] → checks: reflection_round=0 < 2 → ok
      [Prompt "ask_reflect_or_confirm"] → renders template → calls LLM
        LLM returns: "REFLECT"
        EnumParser parses successfully
        Sets: variables["next_action"] = "REFLECT"
        → SUCCESS
    [ReflectionGuard] → child succeeded → increments reflection_round to 1 → SUCCESS

  [Sequence "claims_completion_flow"] → ticks child 2
    [Selector "branch_on_decision"] → ticks child 0
      [Sequence "do_reflect"] → ticks child 0
        [Condition "decision_is_reflect"] → variables["next_action"] == "REFLECT" → SUCCESS
      [Sequence "do_reflect"] → ticks child 1
        [Action "emit_reflect"] → pushes Reflect command → SUCCESS
      [Sequence "do_reflect"] → ALL succeeded → SUCCESS
    [Selector "branch_on_decision"] → first child succeeded → SUCCESS

  [Sequence "claims_completion_flow"] → ALL succeeded → SUCCESS
[Selector "root"] → SUCCESS

Result: [Reflect { prompt: "Review your work..." }]
Side effect: blackboard.reflection_round = 1 (persisted to agent state)
```

**Trace B — Second reflection round, LLM says "confirm"**:

```
[Selector "root"] → ticks child 0
  [Sequence "claims_completion_flow"] → ticks child 0
    [Condition "is_claims_completion"] → SUCCESS

  [Sequence "claims_completion_flow"] → ticks child 1
    [ReflectionGuard "max_2_reflections"] → checks: reflection_round=1 < 2 → ok
      [Prompt "ask_reflect_or_confirm"] → calls LLM → returns "CONFIRM"
        Sets: variables["next_action"] = "CONFIRM"
        → SUCCESS
    [ReflectionGuard] → increments reflection_round to 2 → SUCCESS

  [Sequence "claims_completion_flow"] → ticks child 2
    [Selector "branch_on_decision"] → ticks child 0
      [Sequence "do_reflect"] → ticks child 0
        [Condition "decision_is_reflect"] → variables["next_action"] == "CONFIRM" → FAILURE
      [Sequence "do_reflect"] → FAILURE
    [Selector "branch_on_decision"] → child 0 failed, tries child 1
      [Sequence "do_confirm"] → ticks child 0
        [Condition "decision_is_confirm"] → variables["next_action"] == "CONFIRM" → SUCCESS
      [Sequence "do_confirm"] → ticks child 1
        [Action "emit_confirm"] → pushes ConfirmCompletion → SUCCESS
      [Sequence "do_confirm"] → SUCCESS
    [Selector "branch_on_decision"] → SUCCESS

  [Sequence "claims_completion_flow"] → ALL succeeded → SUCCESS
[Selector "root"] → SUCCESS

Result: [ConfirmCompletion]
Side effect: blackboard.reflection_round = 2
```

**Trace C — Third tick, max reflections reached**:

```
[Selector "root"] → ticks child 0
  [Sequence "claims_completion_flow"] → ticks child 0
    [Condition "is_claims_completion"] → SUCCESS

  [Sequence "claims_completion_flow"] → ticks child 1
    [ReflectionGuard "max_2_reflections"] → checks: reflection_round=2 >= 2 → MAX REACHED
    → returns FAILURE immediately, does NOT tick child

  [Sequence "claims_completion_flow"] → child 1 failed → FAILURE
[Selector "root"] → child 0 failed, tries child 1
  [Action "default_continue"] → pushes ApproveAndContinue → SUCCESS
[Selector "root"] → SUCCESS

Result: [ApproveAndContinue]
```

**Notice**: On the third tick, the ReflectionGuard returns `Failure` before the Prompt node is even executed. No LLM call is made. The tree falls through to the default action. This is how the reflection hard limit is enforced — without any special-case code, just a Decorator in the tree.

---

## Key Concepts Demonstrated

| Concept | Node | Explanation |
|---------|------|-------------|
| **Prompt node** | `ask_reflect_or_confirm` | The core decision mechanism: render template → call LLM → parse → set variable. |
| **ReflectionGuard** | `max_2_reflections` | A stateful Decorator that counts and limits iterations. |
| **Variable branching** | `branch_on_decision` | The Prompt writes to `variables.next_action`; Conditions read it to route execution. |
| **Selector fallback** | `root` | When the reflect loop is exhausted, the tree falls through to `default_continue`. |
| **SubTree reuse** | `reflect` | The reflect logic is extracted to a separate file and referenced by name. |
| **Template rendering** | `ask_reflect_or_confirm` | The prompt includes `{{ task_description }}`, `{{ context_summary }}`, `{{ reflection_round }}`. |
| **Enum parser** | `ask_reflect_or_confirm.parser` | Constrains LLM output to exactly two valid values. Parse failure returns Failure. |

---

## The Prompt Template

The Prompt node's template is the "brain" of this decision. It instructs the LLM to act as a judge:

```text
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
```

**Key design choices**:
- **Context is truncated** to fit the LLM's token budget.
- **Reflection round is shown** so the LLM knows this is not the first check.
- **Explicit criteria** (tests, requirements, code quality) guide the LLM's judgment.
- **Constrained output** (`REFLECT` or `CONFIRM`) makes parsing trivial and reliable.

---

## Conversation Flow

Below is a realistic session showing the reflect loop in action. The decision layer intervenes **within the same codex/claude session** to verify the agent's completion claim.

---

### Scenario: JWT authentication implementation

```
👤 user:
  Implement user authentication with JWT tokens

🤖 worker:
  I'll implement JWT authentication with bcrypt password hashing.
  [writes auth.py with login/logout/register functions]
  [writes test_auth.py with 8 test cases]

  > python -m pytest test_auth.py -v
  test_auth.py::test_register PASSED
  test_auth.py::test_login PASSED
  test_auth.py::test_logout PASSED
  test_auth.py::test_token_refresh PASSED
  test_auth.py::test_invalid_token PASSED
  test_auth.py::test_expired_token PASSED
  test_auth.py::test_password_hashing PASSED
  test_auth.py::test_unauthorized_access PASSED
  ✓ 8/8 tests passed

  Task complete! JWT authentication is fully implemented and tested.
  ───────────────────────────────
  (provider_output contains: "claims_completion")
  (reflection_round = 0)

🧠 decision — Round 1 (reflection_round: 0/2):
  ┌─ ReflectionGuard: max_2_reflections ─┐
  │  0 < 2 → ALLOWED                    │
  └─────────────────────────────────────┘
  ↓
  ┌─ Prompt: ask_reflect_or_confirm ────┐
  │  [sends template to SAME session]   │
  │                                     │
  │  "The agent claims the task is      │
  │   complete. REFLECT or CONFIRM?"    │
  │                                     │
  │  LLM replies: "REFLECT"             │
  │  EnumParser: ✓ valid                │
  │  → sets next_action = "REFLECT"     │
  └─────────────────────────────────────┘
  ↓
  Command: Reflect {
    prompt: "Please review your work carefully..."
  }
  Side effect: reflection_round = 1

🤖 worker:
  [receives Reflect command]
  Let me review the implementation more carefully...

  [reviews auth.py line by line]
  Found issue: Token blacklist on logout is not persisted to Redis.
  Current implementation only clears token from memory.
  [adds Redis integration for token blacklist]
  [adds test for persisted logout]

  > python -m pytest test_auth.py -v
  ✓ 9/9 tests passed (including new Redis test)

  Review complete. All issues fixed. Task is complete!
  ───────────────────────────────
  (provider_output contains: "claims_completion")
  (reflection_round = 1)

🧠 decision — Round 2 (reflection_round: 1/2):
  ┌─ ReflectionGuard: max_2_reflections ─┐
  │  1 < 2 → ALLOWED                    │
  └─────────────────────────────────────┘
  ↓
  ┌─ Prompt: ask_reflect_or_confirm ────┐
  │  [sends template to SAME session]   │
  │                                     │
  │  "Reflection round 1/2.             │
  │   REFLECT or CONFIRM?"              │
  │                                     │
  │  LLM replies: "CONFIRM"             │
  │  EnumParser: ✓ valid                │
  │  → sets next_action = "CONFIRM"     │
  └─────────────────────────────────────┘
  ↓
  Command: ConfirmCompletion
  Side effect: reflection_round = 2

🤖 worker:
  [receives ConfirmCompletion]
  Task confirmed complete. JWT auth with Redis-backed
  token blacklist is ready for production.
```

### Scenario: Max reflections reached

```
👤 user:
  (same task as above)

🤖 worker:
  Task complete! All tests passing.
  ───────────────────────────────
  (reflection_round = 2, already at max)

🧠 decision — Round 3 (reflection_round: 2/2):
  ┌─ ReflectionGuard: max_2_reflections ─┐
  │  2 >= 2 → MAX REACHED               │
  │  → returns FAILURE immediately      │
  │  (Prompt node is NEVER executed)    │
  └─────────────────────────────────────┘
  ↓
  Sequence fails → Selector tries fallback
  ↓
  Command: ApproveAndContinue

🤖 worker:
  [receives ApproveAndContinue]
  Max reflection rounds reached. Accepting completion.
  [moves on to next task or idles]
```

---

## Files

- `tree.yaml` — The root behavior tree.
- `subtrees/reflect.yaml` — The reusable reflect-loop sub-tree.
