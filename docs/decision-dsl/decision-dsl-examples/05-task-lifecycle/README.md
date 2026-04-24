# Example 05: Task Lifecycle Orchestrator

> The most advanced example. Demonstrates **phase-aware routing**, **SubTree reuse across the full task lifecycle**, and **stateful phase transitions**. A single work agent progresses through phases (starting → coding → completing → idle), and the decision tree routes to the appropriate handler based on the current phase.

---

## Important Premise: Single Session

**The entire decision process runs within the same codex/claude session as the work agent.** The decision layer does not spawn a separate LLM call. Instead, it injects a formatted prompt into the **current** session and interprets the LLM's response within that same conversation context.

This means:
- The `Prompt` node renders a template and sends it as the next message in the ongoing session.
- The LLM's reply becomes the new `provider_output`, which the parser then extracts.
- There is no external `LLMCaller::call()` — there is only "send message to current session, wait for reply."

---

## What This Tree Does

A software development task has a natural lifecycle:

1. **Starting** — Set up git environment (branch, worktree).
2. **Coding** — Implement the task. Errors trigger error recovery.
3. **Completing** — The agent claims completion. Trigger reflect loop.
4. **Idle** — All tasks done. Check for new assignments.

The decision tree stores the current phase in `blackboard.variables.task_phase`. The root node is a Selector that routes to the phase-specific handler. When a phase completes, an Action node updates `task_phase` to the next phase.

**This example reuses SubTrees from previous examples**:
- `rate_limit` from Example 02 (universal safety check)
- `reflect` from Example 03 (completion verification)
- `error_recovery` from Example 04 (error classification and recovery)

---

## The Behavior Tree

```text
[Selector "root"]
│
├── [Sequence "phase_starting"]
│   ├── [Condition "is_starting_phase"]
│   │       checks: variables.task_phase == "starting"
│   └── [SubTree "task_start"]
│           ref: task_start
│
├── [Sequence "phase_coding"]
│   ├── [Condition "is_coding_phase"]
│   │       checks: variables.task_phase == "coding"
│   ├── [SubTree "rate_limit"]
│   │       ref: rate_limit (from Example 02)
│   ├── [Condition "is_error"]
│   │       checks: provider_output contains "error"
│   └── [SubTree "error_recovery"]
│           ref: error_recovery (from Example 04)
│
├── [Sequence "phase_completing"]
│   ├── [Condition "is_completing_phase"]
│   │       checks: variables.task_phase == "completing"
│   └── [SubTree "reflect"]
│           ref: reflect (from Example 03)
│
├── [Sequence "phase_idle"]
│   ├── [Condition "is_idle_phase"]
│   │       checks: variables.task_phase == "idle"
│   └── [Prompt "check_new_tasks"]
│           template: "Are there new tasks? YES/NO"
│           parser: enum [YES, NO]
│           sets: has_new_tasks = decision
│
└── [Action "default_continue"]
        emits: ApproveAndContinue
```

### Phase Transition Flow

```text
[starting] ──(task_start SubTree)──> sets task_phase = "coding"
    │
    ▼
[coding] ──(no error)──> ApproveAndContinue
    │
    ├──(error)──> error_recovery SubTree ──> may set task_phase = "completing"
    │
    ▼
[completing] ──(reflect SubTree)──>
    ├──(confirm)──> sets task_phase = "idle"
    └──(reflect)──> stays in "completing", increments reflection_round
    │
    ▼
[idle] ──(Prompt: new tasks?)──>
    ├──(YES)──> sets task_phase = "starting"
    └──(NO)──> StopIfComplete
```

---

## Execution Traces

**Trace A — Phase: coding, no error, normal progress**:

```
[Selector "root"] → ticks child 0
  [Sequence "phase_starting"] → ticks child 0
    [Condition "is_starting_phase"] → variables.task_phase == "coding" → FAILURE
  [Sequence "phase_starting"] → FAILURE

[Selector "root"] → child 0 failed, tries child 1
  [Sequence "phase_coding"] → ticks child 0
    [Condition "is_coding_phase"] → variables.task_phase == "coding" → SUCCESS
  [Sequence "phase_coding"] → ticks child 1
    [SubTree "rate_limit"] → no 429 detected → falls through to default → SUCCESS
  [Sequence "phase_coding"] → ticks child 2
    [Condition "is_error"] → provider_output does not contain "error" → FAILURE
  [Sequence "phase_coding"] → FAILURE

[Selector "root"] → tries children 2, 3 — all FAILURE (wrong phase)
[Selector "root"] → tries child 4
  [Action "default_continue"] → pushes ApproveAndContinue → SUCCESS
[Selector "root"] → SUCCESS

Result: [ApproveAndContinue]
```

**Trace B — Phase: completing, first reflection, LLM says "reflect"**:

```
[Selector "root"] → tries children 0, 1 — all FAILURE (wrong phase)
[Selector "root"] → tries child 2
  [Sequence "phase_completing"] → ticks child 0
    [Condition "is_completing_phase"] → variables.task_phase == "completing" → SUCCESS
  [Sequence "phase_completing"] → ticks child 1
    [SubTree "reflect"] → executes reflect SubTree from Example 03
      [ReflectionGuard] → reflection_round=0 < 2 → ok
        [Prompt "ask_reflect_or_confirm"] → sends message to SAME session
          LLM replies: "REFLECT"
          EnumParser parses → sets variables["next_action"] = "REFLECT"
          → SUCCESS
      [ReflectionGuard] → increments reflection_round to 1 → SUCCESS
      [Selector "branch_on_decision"] → ticks child 0
        [Sequence "do_reflect"] → condition passes → emits Reflect → SUCCESS
      [Selector "branch_on_decision"] → SUCCESS
    [SubTree "reflect"] → SUCCESS
  [Sequence "phase_completing"] → SUCCESS
[Selector "root"] → SUCCESS

Result: [Reflect { prompt: "Review your work..." }]
Side effect: reflection_round = 1
```

**Trace C — Phase: idle, check for new tasks, LLM says "YES"**:

```
[Selector "root"] → tries children 0-2 — all FAILURE (wrong phase)
[Selector "root"] → tries child 3
  [Sequence "phase_idle"] → ticks child 0
    [Condition "is_idle_phase"] → variables.task_phase == "idle" → SUCCESS
  [Sequence "phase_idle"] → ticks child 1
    [Prompt "check_new_tasks"] → sends message to SAME session
      template: "The agent has completed all assigned tasks. Are there new tasks? Reply: YES or NO"
      LLM replies: "YES"
      EnumParser parses → sets variables["has_new_tasks"] = "YES"
      → SUCCESS
  [Sequence "phase_idle"] → ticks child 2
    [Selector "branch_on_new_tasks"] → ticks child 0
      [Sequence "start_new_task"] → ticks child 0
        [Condition "has_new_tasks_yes"] → variables["has_new_tasks"] == "YES" → SUCCESS
      [Sequence "start_new_task"] → ticks child 1
        [SetVar "set_phase_starting"] → sets variables["task_phase"] = "starting"
        → SUCCESS
      [Sequence "start_new_task"] → SUCCESS
    [Selector "branch_on_new_tasks"] → SUCCESS
  [Sequence "phase_idle"] → ALL succeeded → SUCCESS
[Selector "root"] → SUCCESS

Result: [] (no commands, but task_phase = "starting" for next cycle)
```

---

## Key Concepts Demonstrated

| Concept | Node | Explanation |
|---------|------|-------------|
| **Phase-aware routing** | `root` Selector | Routes based on `variables.task_phase`. Each phase is a top-level branch. |
| **Cross-example SubTree reuse** | `rate_limit`, `reflect`, `error_recovery` | SubTrees defined in Examples 02-04 are referenced here. No copy-paste. |
| **Phase transitions** | `set_phase_coding`, `set_phase_idle` | SetVar nodes mutate the phase variable, causing the next tick to route to a different branch. |
| **Session-local Prompt** | All Prompt nodes | Every Prompt sends its template as a message within the **same** codex/claude session, not a new API call. |
| **Stateful lifecycle** | `task_phase` variable | The phase persists across decision cycles (stored in agent state, copied to Blackboard each tick). |
| **Universal safety** | `rate_limit` SubTree | Rate limit checking is included in the coding phase, regardless of what the agent is working on. |

---

## Why Phase-Based Routing?

Without phases, the decision tree would need to check every possible situation at every tick:

```yaml
# Bad: flat, unscoped tree
Selector:
  - is_task_starting? → task_start
  - is_rate_limit? → retry
  - is_error? → error_recovery
  - is_claims_completion? → reflect
  - is_idle? → check_new_tasks
  - default → continue
```

This is fragile because conditions can overlap. An error during task startup is different from an error during coding. With phases, the error recovery SubTree is only evaluated in the `coding` phase, where it makes sense.

Phases also make the tree **debuggable**: if the agent behaves strangely during completion, you only need to look at the `phase_completing` branch.

---

## The Same-Session Prompt Model

All Prompt nodes in this example (and in the entire decision layer) follow this execution model:

```
┌─────────────────────────────────────────────────────────────┐
│              Same-Session Prompt Execution                   │
│                                                              │
│  Work Agent Session (codex/claude)                          │
│      │                                                       │
│      ▼                                                       │
│  ┌─────────────────┐                                         │
│  │ Agent produces  │  "I have completed the auth module"    │
│  │ provider_output │                                         │
│  └────────┬────────┘                                         │
│           │                                                  │
│           ▼                                                  │
│  ┌─────────────────┐                                         │
│  │ Decision Layer  │  Builds Blackboard from agent state    │
│  │   (this tree)   │                                         │
│  └────────┬────────┘                                         │
│           │                                                  │
│           ▼                                                  │
│  ┌─────────────────┐                                         │
│  │ Prompt node     │  Renders template:                     │
│  │ "ask_reflect_   │  "The agent claims completion...       │
│  │  or_confirm"    │   REFLECT or CONFIRM?"                 │
│  └────────┬────────┘                                         │
│           │                                                  │
│           ▼  SAME SESSION — no new API call                  │
│  ┌─────────────────┐                                         │
│  │ Send as next    │  Message appended to current thread    │
│  │ message         │                                         │
│  └────────┬────────┘                                         │
│           │                                                  │
│           ▼                                                  │
│  ┌─────────────────┐                                         │
│  │ LLM replies     │  "REFLECT"                              │
│  │ (new provider_  │                                         │
│  │  output)        │                                         │
│  └────────┬────────┘                                         │
│           │                                                  │
│           ▼                                                  │
│  ┌─────────────────┐                                         │
│  │ EnumParser      │  Extracts "REFLECT"                    │
│  │                 │  Sets blackboard variable               │
│  └─────────────────┘                                         │
│                                                              │
└─────────────────────────────────────────────────────────────┘
```

**Critical invariant**: The Prompt node never leaves the session. The LLM sees the decision prompt as the natural next turn in the conversation. This preserves context and avoids the cost of a new API call.

---

## Conversation Flow

Below is a realistic session showing the **complete task lifecycle** — from git setup through coding, error recovery, completion verification, and idle check. All decisions happen within the same codex/claude session.

---

### Scenario: Full lifecycle of "Implement login functionality"

```
👤 user:
  Implement user login functionality with session management

═══════════════════════════════════════════════════════════════
  PHASE: starting
═══════════════════════════════════════════════════════════════

🤖 worker:
  I'll implement user login with session management.
  Let me start by setting up the git environment...
  [Context: task_phase = "starting"]

🧠 decision:
  ┌─ Condition: is_starting_phase ──────┐
  │  task_phase == "starting" → SUCCESS │
  └─────────────────────────────────────┘
  ↓
  ┌─ SubTree: task_start ───────────────┐
  │  Prompt: choose_git_strategy        │
  │  "NEW_BRANCH or EXISTING?"          │
  │  LLM replies: "NEW_BRANCH"          │
  │                                     │
  │  Selector → new_branch_flow         │
  │  Command: CreateTaskBranch {        │
  │    branch_name: "feature/login",    │
  │    base_branch: "main"              │
  │  }                                  │
  │  Command: PrepareTaskStart {        │
  │    task_id: "login",                │
  │    task_description: "..."          │
  │  }                                  │
  │  SetVar: task_phase = "coding"      │
  └─────────────────────────────────────┘

🤖 worker:
  [receives commands]
  git checkout -b feature/login main
  ✓ Branch 'feature/login' created
  Ready to implement login functionality.

═══════════════════════════════════════════════════════════════
  PHASE: coding
═══════════════════════════════════════════════════════════════

🤖 worker:
  Implementing login form and session handling...
  [writes login.py with form validation]
  [writes session.py with Flask session management]

  Now running tests...
  > python -m pytest test_login.py -v
  test_login.py::test_valid_login PASSED
  test_login.py::test_invalid_password PASSED
  test_login.py::test_session_creation PASSED
  test_login.py::test_session_expiry PASSED
  ✓ 4/4 tests passed
  ───────────────────────────────
  (no "error", no "429" in output)
  (task_phase = "coding")

🧠 decision:
  ┌─ Condition: is_coding_phase ────────┐
  │  task_phase == "coding" → SUCCESS   │
  └─────────────────────────────────────┘
  ↓
  ┌─ SubTree: rate_limit ───────────────┐
  │  Condition: no "429" → FAILURE      │
  │  Fallback → ApproveAndContinue      │
  │  (no rate limit, continue)          │
  └─────────────────────────────────────┘
  ↓
  ┌─ Condition: is_error ───────────────┐
  │  no "error" → FAILURE               │
  └─────────────────────────────────────┘
  ↓
  No error recovery needed
  → Command: ApproveAndContinue

🤖 worker:
  [continues coding]
  Adding remember-me checkbox and CSRF protection...
  [writes remember_me.py]
  [updates login.html template]

  Running final integration tests...
  > python -m pytest test_integration.py -v
  test_integration.py::test_full_login_flow PASSED
  test_integration.py::test_remember_me_cookie PASSED
  test_integration.py::test_csrf_token PASSED
  ✓ 3/3 tests passed

  Login functionality complete!
  ───────────────────────────────
  (task_phase = "coding")

🧠 decision:
  Same path → ApproveAndContinue

🤖 worker:
  All features implemented and tested.
  Ready to mark as complete.
  ───────────────────────────────
  (worker signals completion)
  [Context updated: task_phase = "completing"]

═══════════════════════════════════════════════════════════════
  PHASE: completing
═══════════════════════════════════════════════════════════════

🤖 worker:
  Login feature is complete with:
  - Form validation
  - Password hashing (bcrypt)
  - Session management
  - CSRF protection
  - Remember-me functionality
  All 7 tests passing.
  ───────────────────────────────
  (task_phase = "completing")

🧠 decision — Round 1:
  ┌─ Condition: is_completing_phase ────┐
  │  task_phase == "completing" → SUCCESS│
  └─────────────────────────────────────┘
  ↓
  ┌─ SubTree: reflect ──────────────────┐
  │  ReflectionGuard: 0 < 2 → ALLOWED   │
  │                                     │
  │  Prompt: ask_reflect_or_confirm     │
  │  "Login feature complete.           │
  │   REFLECT or CONFIRM?"              │
  │                                     │
  │  LLM replies: "REFLECT"             │
  └─────────────────────────────────────┘
  ↓
  Command: Reflect {
    prompt: "Review login implementation..."
  }
  reflection_round = 1

🤖 worker:
  [receives Reflect command]
  Let me review the login implementation...

  [reviews login.py, session.py, remember_me.py]
  Found issue: No rate limiting on login endpoint.
  Potential brute-force vulnerability.
  [adds rate limiting with flask-limiter]
  [adds test for rate limiting]

  > python -m pytest test_login.py -v
  ✓ 8/8 tests passed (including rate limit test)

  Review complete. Security issue fixed.
  ───────────────────────────────
  (still task_phase = "completing")

🧠 decision — Round 2:
  ┌─ SubTree: reflect ──────────────────┐
  │  ReflectionGuard: 1 < 2 → ALLOWED   │
  │                                     │
  │  Prompt: ask_reflect_or_confirm     │
  │  "Reflection round 1/2.             │
  │   REFLECT or CONFIRM?"              │
  │                                     │
  │  LLM replies: "CONFIRM"             │
  └─────────────────────────────────────┘
  ↓
  Command: ConfirmCompletion

🤖 worker:
  [receives ConfirmCompletion]
  Login feature confirmed complete.
  All tests passing. Security verified.
  [Context updated: task_phase = "idle"]

═══════════════════════════════════════════════════════════════
  PHASE: idle
═══════════════════════════════════════════════════════════════

🤖 worker:
  All assigned tasks completed.
  Waiting for new assignments...
  [Context: task_phase = "idle"]

🧠 decision:
  ┌─ Condition: is_idle_phase ──────────┐
  │  task_phase == "idle" → SUCCESS     │
  └─────────────────────────────────────┘
  ↓
  ┌─ Prompt: check_new_tasks ───────────┐
  │  "Are there new tasks? YES/NO"      │
  │                                     │
  │  LLM replies: "NO"                  │
  └─────────────────────────────────────┘
  ↓
  ┌─ Selector: branch_on_new_tasks ─────┐
  │  start_new_task → FAILURE (NO)      │
  │  stop_if_done → SUCCESS             │
  └─────────────────────────────────────┘
  ↓
  Command: StopIfComplete {
    reason: "All tasks completed, no new assignments"
  }

🤖 worker:
  [receives StopIfComplete]
  No new tasks. Shutting down gracefully.
  Summary: Implemented login feature with
  8 tests, CSRF protection, and rate limiting.
  ───────────────────────────────
  [Agent enters idle state]
```

---

## Files

- `tree.yaml` — The root task lifecycle orchestrator.
- `subtrees/task-start.yaml` — Task startup sub-tree (git branch selection).

---

## DecisionRules Shorthand

Phase-based routing with cross-example SubTree reuse can be expressed as DecisionRules with Switch nodes:

```yaml
apiVersion: decision.agile-agent.io/v1
kind: DecisionRules
metadata:
  name: task_lifecycle
spec:
  rules:
    - priority: 1
      name: phase_starting
      if:
        kind: variableIs
        key: task_phase
        value: "starting"
      then:
        kind: Switch
        name: git_strategy
        on:
          kind: prompt
          template: |
            Choose git strategy for {{ current_task_id }}.
            Reply: NEW_BRANCH or EXISTING
          parser:
            kind: enum
            values: [NEW_BRANCH, EXISTING]
        cases:
          NEW_BRANCH:
            command:
              CreateTaskBranch:
                branch_name: "feature/{{ current_task_id | slugify }}"
                base_branch: "main"
          EXISTING:
            command:
              RebaseToMain:
                base_branch: "main"

    - priority: 2
      name: phase_completing
      if:
        kind: variableIs
        key: task_phase
        value: "completing"
      then:
        kind: Switch
        name: reflect_or_confirm
        on:
          kind: prompt
          template: "REFLECT or CONFIRM?"
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

    - priority: 99
      name: default_continue
      then:
        command: ApproveAndContinue
```

Phase transitions are managed by `SetVar` nodes that write `task_phase` — these are emitted by the downstream commands or by explicit `SetVar` actions after each phase completes.
