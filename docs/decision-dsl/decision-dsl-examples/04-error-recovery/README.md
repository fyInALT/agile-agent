# Example 04: Error Recovery

> Demonstrates **structured parsing**, **thinking-model prompts**, and **deep multi-branch decision trees**. When the agent encounters an error, the tree classifies it and chooses a targeted recovery strategy.

---

## What This Tree Does

Not all errors are equal. A syntax error needs a fix. A timeout needs a retry. A permission error needs escalation. This tree uses a reasoning model (thinking tier) to classify the error, then branches to the appropriate recovery path.

**Decision Logic**:

1. Detect that an error occurred (provider output contains "error").
2. Ask a thinking model to classify the error into: SYNTAX, TEST, PERMISSION, TIMEOUT, RATE_LIMIT, LOGIC, or UNKNOWN.
3. Ask the same model to recommend: RETRY, FIX, SKIP, or ESCALATE.
4. Branch to the recovery path matching the recommendation.

The Prompt node uses a **structured parser** with regex to extract three fields from the LLM's response: `CLASS`, `RECOMMEND`, and `REASON`. These are written to the Blackboard as `error_class`, `error_recommendation`, and `error_reason`.

---

## The Behavior Tree

```text
[Sequence "error_recovery"]
│
├── [Condition "is_error"]
│       checks: provider_output contains "error"
│
├── [Prompt "classify_error"]
│       model: thinking
│       template: "Classify the error... Reply: CLASS: <X> RECOMMEND: <Y> REASON: <Z>"
│       parser: StructuredParser
│           pattern: "CLASS:\\s*(\\w+)\\s*RECOMMEND:\\s*(\\w+)\\s*REASON:\\s*(.*)"
│           fields: [classification, recommendation, reason]
│       sets:
│           error_class ← classification
│           error_recommendation ← recommendation
│           error_reason ← reason
│
└── [Selector "act_on_recommendation"]
    │
    ├── [Sequence "retry_flow"]
    │   ├── [Condition "recommend_retry"]
    │   │       checks: variables.error_recommendation == "RETRY"
    │   └── [Action "emit_retry"]
    │           emits: RetryTool { tool_name: "...", max_attempts: 3 }
    │
    ├── [Sequence "fix_flow"]
    │   ├── [Condition "recommend_fix"]
    │   │       checks: variables.error_recommendation == "FIX"
    │   └── [Action "emit_custom_instruction"]
    │           emits: SendCustomInstruction {
    │               prompt: "Fix this {{ variables.error_class }} error: {{ variables.error_reason }}"
    │           }
    │
    └── [Sequence "escalate_flow"]
        ├── [Condition "recommend_escalate_or_skip"]
        │       checks: error_recommendation in ["ESCALATE", "SKIP"]
        └── [Action "emit_escalate"]
                emits: EscalateToHuman {
                    reason: "Error recovery: {{ variables.error_recommendation }}"
                }
```

### Execution Traces

**Trace A — Syntax error, LLM recommends FIX**:

```
[Sequence "error_recovery"] → ticks child 0
  [Condition "is_error"] → provider_output contains "error" → SUCCESS

[Sequence "error_recovery"] → ticks child 1
  [Prompt "classify_error"] → renders template with error context → calls thinking model
    LLM returns:
      "CLASS: SYNTAX
       RECOMMEND: FIX
       REASON: Missing semicolon in auth.rs line 42"
    StructuredParser regex extracts:
      classification = "SYNTAX"
      recommendation = "FIX"
      reason = "Missing semicolon in auth.rs line 42"
    Sets Blackboard variables:
      error_class = "SYNTAX"
      error_recommendation = "FIX"
      error_reason = "Missing semicolon in auth.rs line 42"
    → SUCCESS

[Sequence "error_recovery"] → ticks child 2
  [Selector "act_on_recommendation"] → ticks child 0
    [Sequence "retry_flow"] → ticks child 0
      [Condition "recommend_retry"] → error_recommendation == "FIX" → FAILURE
    [Sequence "retry_flow"] → FAILURE

  [Selector "act_on_recommendation"] → child 0 failed, tries child 1
    [Sequence "fix_flow"] → ticks child 0
      [Condition "recommend_fix"] → error_recommendation == "FIX" → SUCCESS
    [Sequence "fix_flow"] → ticks child 1
      [Action "emit_custom_instruction"] → renders prompt with variables
        → pushes SendCustomInstruction {
             prompt: "Fix this SYNTAX error: Missing semicolon in auth.rs line 42",
             target_agent: "agent-42"
           }
        → SUCCESS
    [Sequence "fix_flow"] → ALL succeeded → SUCCESS

  [Selector "act_on_recommendation"] → child 1 succeeded → SUCCESS

[Sequence "error_recovery"] → ALL succeeded → SUCCESS

Result: [SendCustomInstruction { prompt: "Fix this SYNTAX error: Missing semicolon in auth.rs line 42", target_agent: "agent-42" }]
```

**Trace B — Timeout error, LLM recommends RETRY**:

```
[Sequence "error_recovery"] → ticks child 0
  [Condition "is_error"] → SUCCESS

[Sequence "error_recovery"] → ticks child 1
  [Prompt "classify_error"] → LLM returns:
    "CLASS: TIMEOUT
     RECOMMEND: RETRY
     REASON: Network request to API took too long"
  → Sets variables → SUCCESS

[Sequence "error_recovery"] → ticks child 2
  [Selector "act_on_recommendation"] → ticks child 0
    [Sequence "retry_flow"] → ticks child 0
      [Condition "recommend_retry"] → error_recommendation == "RETRY" → SUCCESS
    [Sequence "retry_flow"] → ticks child 1
      [Action "emit_retry"] → pushes RetryTool → SUCCESS
    [Sequence "retry_flow"] → SUCCESS
  [Selector "act_on_recommendation"] → SUCCESS

Result: [RetryTool { tool_name: "Bash", args: null, max_attempts: 3 }]
```

**Trace C — Unknown error, LLM recommends ESCALATE**:

```
[Sequence "error_recovery"] → ticks child 0
  [Condition "is_error"] → SUCCESS

[Sequence "error_recovery"] → ticks child 1
  [Prompt "classify_error"] → LLM returns:
    "CLASS: UNKNOWN
     RECOMMEND: ESCALATE
     REASON: Error message is garbled binary data"
  → Sets variables → SUCCESS

[Sequence "error_recovery"] → ticks child 2
  [Selector "act_on_recommendation"] → ticks child 0
    [Sequence "retry_flow"] → FAILURE (recommendation is ESCALATE)
  [Selector "act_on_recommendation"] → tries child 1
    [Sequence "fix_flow"] → FAILURE (recommendation is ESCALATE)
  [Selector "act_on_recommendation"] → tries child 2
    [Sequence "escalate_flow"] → ticks child 0
      [Condition "recommend_escalate_or_skip"] → error_recommendation in ["ESCALATE", "SKIP"] → SUCCESS
    [Sequence "escalate_flow"] → ticks child 1
      [Action "emit_escalate"] → pushes EscalateToHuman → SUCCESS
    [Sequence "escalate_flow"] → SUCCESS
  [Selector "act_on_recommendation"] → SUCCESS

Result: [EscalateToHuman { reason: "Error recovery: ESCALATE", context: "Error message is garbled binary data" }]
```

---

## Key Concepts Demonstrated

| Concept | Node | Explanation |
|---------|------|-------------|
| **Thinking model** | `classify_error` | Uses `model: thinking` for deeper reasoning. More expensive but higher quality classification. |
| **Structured parser** | `classify_error.parser` | Regex extracts multiple fields from LLM output. One Prompt produces three Blackboard variables. |
| **Multi-field branching** | `act_on_recommendation` | A Selector with four branches, each guarded by a Condition on the parsed recommendation. |
| **Variable interpolation in commands** | `emit_custom_instruction` | The Action's command template interpolates `{{ variables.error_class }}` and `{{ variables.error_reason }}`. |
| **OR condition** | `recommend_escalate_or_skip` | Uses `kind: or` to match either ESCALATE or SKIP with the same recovery path. |
| **Context-rich prompt** | `classify_error.template` | Includes last tool call name, input, and output to give the LLM maximum context for classification. |

---

## The Prompt Template

```text
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
```

**Design rationale**:
- **Includes tool call context**: The LLM needs to know what operation failed, not just the error message.
- **Explicit category definitions**: Each classification has a clear description, reducing LLM ambiguity.
- **Structured output format**: The `CLASS: / RECOMMEND: / REASON:` format is designed for reliable regex parsing.
- **Single-sentence reason**: Forces concision. The reason is passed to the human in escalation scenarios.

---

## Why Structured Parsing Matters

If we used an Enum parser here, we would need two separate Prompt nodes — one for classification and one for recommendation. The Structured parser allows one Prompt to produce multiple fields:

```yaml
# Without structured parser (two LLM calls):
Prompt "classify" → parser: enum [SYNTAX, TEST, ...] → sets: error_class
Prompt "recommend" → parser: enum [RETRY, FIX, ...] → sets: error_recommendation

# With structured parser (one LLM call):
Prompt "classify_error" → parser: structured → sets: error_class, error_recommendation, error_reason
```

One LLM call is faster and cheaper than two. The Structured parser pays for itself in latency and token cost.

---

## Conversation Flow

Below is a realistic session showing how the decision layer classifies and recovers from different types of errors — all within the same codex/claude session.

---

### Scenario A: Syntax error (FIX path)

```
👤 user:
  Fix the database connection bug where it fails on startup

🤖 worker:
  I'll fix the database connection issue.
  [reads db_config.py]
  [edits db.py to use connection pool]

  > python -c "import db; db.connect()"
  ───────────────────────────────
  File "db.py", line 42
      conn = psycopg2.connect(
                              ^
  SyntaxError: invalid syntax
  (provider_output contains: "error")
  (last_tool_call: Bash "python -c ...")

🧠 decision:
  ┌─ Condition: is_error ───────────────┐
  │  output contains "error" → SUCCESS  │
  └─────────────────────────────────────┘
  ↓
  ┌─ Prompt: classify_error ────────────┐
  │  [sends to SAME session]            │
  │  "Classify this error...            │
  │   CLASS: <X> RECOMMEND: <Y>         │
  │   REASON: <Z>"                      │
  │                                     │
  │  LLM replies:                       │
  │  CLASS: SYNTAX                      │
  │  RECOMMEND: FIX                     │
  │  REASON: Missing closing paren on   │
  │          line 42                    │
  │                                     │
  │  StructuredParser extracts 3 fields │
  │  → error_class = "SYNTAX"           │
  │  → error_recommendation = "FIX"     │
  │  → error_reason = "Missing..."      │
  └─────────────────────────────────────┘
  ↓
  ┌─ Selector: branch_on_recommendation ┐
  │  retry_flow → FAILURE (not RETRY)   │
  │  fix_flow → SUCCESS                 │
  └─────────────────────────────────────┘
  ↓
  Command: SendCustomInstruction {
    prompt: "The agent encountered a SYNTAX error.
             Reason: Missing closing paren on line 42
             Please fix this issue and retry.",
    target_agent: "agent-42"
  }

🤖 worker:
  [receives custom instruction]
  Fixing syntax error: missing closing parenthesis on line 42.
  [adds missing ) to db.py line 42]

  > python -c "import db; db.connect()"
  ✓ Connection successful
  ───────────────────────────────
  (no "error" in output)

🧠 decision:
  ┌─ Condition: is_error ───────────────┐
  │  no "error" in output → FAILURE     │
  └─────────────────────────────────────┘
  ↓
  Fallback → Command: ApproveAndContinue

🤖 worker:
  [continues or reports completion]
```

### Scenario B: Timeout error (RETRY path)

```
👤 user:
  Fetch data from the external API

🤖 worker:
  I'll fetch data from the external API.
  [writes api_client.py]

  > python api_client.py
  ───────────────────────────────
  requests.exceptions.Timeout: HTTPConnectionPool:
  Request timed out after 30 seconds
  (provider_output contains: "error")
  (last_tool_call: Bash "python api_client.py")

🧠 decision:
  ┌─ Prompt: classify_error ────────────┐
  │  LLM replies:                       │
  │  CLASS: TIMEOUT                     │
  │  RECOMMEND: RETRY                   │
  │  REASON: Network request took too   │
  │          long, likely transient     │
  └─────────────────────────────────────┘
  ↓
  ┌─ Selector: branch_on_recommendation ┐
  │  retry_flow → SUCCESS               │
  └─────────────────────────────────────┘
  ↓
  Command: RetryTool {
    tool_name: "Bash",
    args: "python api_client.py",
    max_attempts: 3
  }

🤖 worker:
  [receives RetryTool command]
  Retrying API fetch...
  > python api_client.py
  ✓ Fetched 1,247 records in 12.3s
  Data saved to data.json

🧠 decision:
  No error → ApproveAndContinue
```

### Scenario C: Unknown error (ESCALATE path)

```
👤 user:
  Run the data migration script

🤖 worker:
  Running migration script...
  > python migrate.py
  ───────────────────────────────
  Error: �X�q���
  [garbled binary output]
  (provider_output contains: "error")

🧠 decision:
  ┌─ Prompt: classify_error ────────────┐
  │  LLM replies:                       │
  │  CLASS: UNKNOWN                     │
  │  RECOMMEND: ESCALATE                │
  │  REASON: Error message is garbled   │
  │          binary data, cannot parse  │
  └─────────────────────────────────────┘
  ↓
  ┌─ Selector: branch_on_recommendation ┐
  │  retry_flow → FAILURE               │
  │  fix_flow → FAILURE                 │
  │  escalate_flow → SUCCESS            │
  └─────────────────────────────────────┘
  ↓
  Command: EscalateToHuman {
    reason: "Error recovery: ESCALATE",
    context: "Error message is garbled binary data, cannot parse"
  }

🤖 worker:
  [receives EscalateToHuman]
  Pausing migration. Awaiting human review.
  [notifies TUI with reason and context]
```

---

## Files

- `tree.yaml` — The complete error recovery behavior tree.
