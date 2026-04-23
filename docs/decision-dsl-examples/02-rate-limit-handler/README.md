# Example 02: Rate Limit Handler

> Demonstrates Decorator nodes — specifically the `Cooldown` decorator — and how to build a robust retry mechanism for 429/rate-limit errors.

---

## What This Tree Does

When the LLM provider returns a 429 (rate limit) or quota-exceeded error, the agent should wait before retrying. Hitting the API again immediately would worsen the rate limit.

**Decision Logic**:

1. Check if the provider output or last tool call indicates a rate-limit error (429, "rate limit", "quota").
2. If yes, enforce a 5-second cooldown before allowing retry.
3. Emit a `RetryTool` command with exponential backoff metadata.
4. If no rate limit detected, fall through to the default action.

The `Cooldown` decorator is the key innovation here: it tracks the last successful execution time and returns `Failure` if the cooldown period has not elapsed. This prevents retry spam.

---

## The Behavior Tree

```text
[Selector "root"]
│
├── [Sequence "rate_limit_handler"]
│   ├── [Condition "is_rate_limit"]
│   │       checks: provider_output matches /(429|rate.?limit|quota)/
│   ├── [Cooldown "rate_limit_cooldown" duration=5s]
│   │   └── [Action "emit_retry"]
│   │           emits: RetryTool { tool_name: "...", cooldown_ms: 5000 }
│   └── [SetVar "set_retry_flag"]
│           sets: retry_due_to_rate_limit = true
│
└── [Action "default_continue"]
        emits: ApproveAndContinue
```

### Execution Traces

**Trace A — Rate limit detected, cooldown elapsed**:

```
[Selector "root"] → ticks child 0
  [Sequence "rate_limit_handler"] → ticks child 0
    [Condition "is_rate_limit"] → regex matches "429" in provider_output → SUCCESS
  [Sequence "rate_limit_handler"] → ticks child 1
    [Cooldown "rate_limit_cooldown"] → checks last_success: None (never succeeded) → cooldown elapsed
      [Action "emit_retry"] → pushes RetryTool → SUCCESS
    [Cooldown "rate_limit_cooldown"] → child succeeded → updates last_success → SUCCESS
  [Sequence "rate_limit_handler"] → ticks child 2
    [SetVar "set_retry_flag"] → sets blackboard variable → SUCCESS
  [Sequence "rate_limit_handler"] → ALL children succeeded → SUCCESS
[Selector "root"] → child 0 succeeded → SUCCESS

Result: [RetryTool { ... }, retry_due_to_rate_limit = true]
```

**Trace B — Rate limit detected, cooldown NOT elapsed**:

```
[Selector "root"] → ticks child 0
  [Sequence "rate_limit_handler"] → ticks child 0
    [Condition "is_rate_limit"] → regex matches → SUCCESS
  [Sequence "rate_limit_handler"] → ticks child 1
    [Cooldown "rate_limit_cooldown"] → checks last_success: 2 seconds ago → cooldown NOT elapsed → FAILURE
  [Sequence "rate_limit_handler"] → child 1 failed → FAILURE
[Selector "root"] → child 0 failed, tries child 1
  [Action "default_continue"] → pushes ApproveAndContinue → SUCCESS
[Selector "root"] → child 1 succeeded → SUCCESS

Result: [ApproveAndContinue]
```

**Trace C — No rate limit**:

```
[Selector "root"] → ticks child 0
  [Sequence "rate_limit_handler"] → ticks child 0
    [Condition "is_rate_limit"] → no match → FAILURE
  [Sequence "rate_limit_handler"] → FAILURE
[Selector "root"] → child 0 failed, tries child 1
  [Action "default_continue"] → SUCCESS
[Selector "root"] → SUCCESS

Result: [ApproveAndContinue]
```

---

## Key Concepts Demonstrated

| Concept | Node | Explanation |
|---------|------|-------------|
| **Decorator** | `rate_limit_cooldown` | A `Cooldown` wraps an Action and enforces a time gate. |
| **Regex condition** | `is_rate_limit` | Uses a regex pattern instead of a simple substring match. |
| **Stateful decorator** | `Cooldown` | Tracks `last_success` internally across ticks. |
| **Variable setting** | `set_retry_flag` | `SetVar` writes to the Blackboard for downstream nodes or debugging. |
| **Guarded retry** | The entire Sequence | Rate limit handling only triggers when both the condition AND the cooldown pass. |

---

## Why Cooldown Matters

Without the `Cooldown` decorator, a fast-polling decision loop would retry immediately, making the rate limit worse:

```
Tick 1: 429 detected → RetryTool emitted → API call → 429
Tick 2: 429 detected → RetryTool emitted → API call → 429
Tick 3: 429 detected → RetryTool emitted → API call → 429
# Infinite retry loop, burning quota and time
```

With `Cooldown`, the tree fails gracefully and lets the default action run until the cooldown elapses:

```
Tick 1: 429 detected → Cooldown ok → RetryTool emitted → API call → 429
Tick 2: 429 detected → Cooldown NOT ok → FAILURE → ApproveAndContinue
Tick 3: 429 detected → Cooldown NOT ok → FAILURE → ApproveAndContinue
# ... 5 seconds later ...
Tick N: 429 detected → Cooldown ok → RetryTool emitted → API call → Success
```

---

## Conversation Flow

Below is a realistic session showing how the decision layer handles a rate-limit error within the same codex/claude session.

---

### Scenario: Rate limit during test execution

```
👤 user:
  Sort this array in O(n log n) time

🤖 worker:
  I'll implement quicksort for you.
  [writes quicksort.py with partition logic]
  [writes test_sort.py with 5 test cases]

  Now running tests...
  > python -m pytest test_sort.py -v
  ───────────────────────────────
  Error: Request rejected (429) — rate limit exceeded
  (provider_output contains: "429")

🧠 decision (Tick 1 — immediately after error):
  ┌─ Condition: is_rate_limit ──────────┐
  │  regex matches "429" in output      │
  │  → SUCCESS                          │
  └─────────────────────────────────────┘
  ↓
  ┌─ Cooldown: rate_limit_cooldown ─────┐
  │  last_success: None (never fired)   │
  │  → cooldown ELAPSED → SUCCESS       │
  └─────────────────────────────────────┘
  ↓
  Command: RetryTool { tool_name: "Bash", cooldown_ms: 5000 }
  Variable: retry_due_to_rate_limit = true

🤖 worker:
  [receives RetryTool command]
  Retrying test execution after 5s cooldown...
  > sleep 5 && python -m pytest test_sort.py -v
  test_sort.py::test_empty PASSED
  test_sort.py::test_single PASSED
  test_sort.py::test_sorted PASSED
  test_sort.py::test_reverse PASSED
  test_sort.py::test_random PASSED
  ✓ 5 tests passed
  ───────────────────────────────
  (no "429" in output)

🧠 decision (Tick 2 — tests passed):
  ┌─ Condition: is_rate_limit ──────────┐
  │  regex does NOT match               │
  │  → FAILURE                          │
  └─────────────────────────────────────┘
  ↓
  Fallback → Command: ApproveAndContinue

🤖 worker:
  [continues with next task or reports completion]
```

### Scenario: Cooldown blocks rapid retry

```
👤 user:
  (same as above)

🤖 worker:
  > python -m pytest test_sort.py -v
  Error: Request rejected (429) — rate limit exceeded

🧠 decision (Tick 1 — first 429):
  Cooldown elapsed → RetryTool emitted
  [worker waits 5s and retries → success]

🤖 worker:
  Now let me run a performance benchmark...
  > python benchmark.py
  Error: Request rejected (429) — rate limit exceeded
  [only 2 seconds have passed since last retry]

🧠 decision (Tick 2 — second 429, too soon):
  ┌─ Condition: is_rate_limit ──────────┐
  │  regex matches "429"                │
  │  → SUCCESS                          │
  └─────────────────────────────────────┘
  ↓
  ┌─ Cooldown: rate_limit_cooldown ─────┐
  │  last_success: 2 seconds ago        │
  │  → cooldown NOT elapsed → FAILURE   │
  └─────────────────────────────────────┘
  ↓
  Sequence fails → Selector tries fallback
  ↓
  Command: ApproveAndContinue

🤖 worker:
  [receives ApproveAndContinue]
  Rate limit still active. I'll pause benchmarking
  and move on to documentation instead.
  [switches to non-API work]
  ───────────────────────────────
  [3 seconds later...]

🧠 decision (Tick 3 — cooldown now elapsed):
  Cooldown elapsed → RetryTool emitted
  [worker retries benchmark → success]
```

---

## Files

- `tree.yaml` — The behavior tree definition.
