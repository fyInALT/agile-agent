# Rate Limit Resting State Design

## Overview

When HTTP 429 (rate limit) occurs, the agent enters a "resting" state (💤) that persists until the rate limit clears. Rather than repeatedly failing and deadlocking, the agent waits patiently and retries at fixed intervals. Recovery requires both the decision layer and the work agent to succeed before the agent can resume normal operation.

## Background

Current problem: When an agent hits HTTP 429, the system attempts to make a decision via the LLM. Since the work agent and decision agent share the same API quota, the decision call also hits 429, causing a deadlock where the TUI freezes.

Previous fix (commit 4bcab15): Route 429 errors to Simple tier to avoid LLM call during initial detection. However, this only handles the immediate decision — the agent still needs to wait for quota to recover.

## Design

### 1. Decision Layer

#### RateLimitBlockedReason

New `BlockingReason` implementation in `decision/src/blocking.rs`:

```rust
pub struct RateLimitBlockedReason {
    started_at: DateTime<Utc>,
    last_retry_at: Option<DateTime<Utc>>,
    retry_count: u32,
    interval_secs: u64,
}
```

- `started_at`: When the first 429 occurred (absolute timestamp, survives process restart)
- `last_retry_at`: Last recovery attempt timestamp
- `retry_count`: Number of recovery attempts made
- `interval_secs`: Configurable retry interval (default: 30 minutes)

Methods:
- `can_auto_resolve()` → `false` (must actually try LLM call to verify)
- `auto_resolve_action()` → `None`
- `urgency()` → `UrgencyLevel::Low` (non-critical, patient wait)
- `expires_at()` → `None` (no expiration, wait indefinitely)
- `description()` → `"💤 Rate limited (X min)"` where X is minutes since `started_at`

#### RateLimitRecoverySituation

New `DecisionSituation` in `decision/src/builtin_situations.rs`:

```rust
pub struct RateLimitRecoverySituation {
    started_at: DateTime<Utc>,
    retry_count: u32,
    last_error: Option<String>,
}
```

- `situation_type()` → `SituationType::new("rate_limit_recovery")`
- `available_actions()` → `[retry, request_human]`
- `to_prompt_text()` → Shows elapsed time and retry count
- `requires_human()` → `false`
- `human_urgency()` → `UrgencyLevel::Low`

#### TieredDecisionEngine

When `BlockedState.reason()` is `RateLimitBlockedReason`:
- Route to `DecisionTier::Simple` (rule engine)
- Rule engine decides `retry` action
- On `retry` action, trigger actual LLM call to test recovery

### 2. Core Layer

#### AgentSlotStatus::Resting

New status variant in `core/src/agent_slot.rs`:

```rust
Resting {
    started_at: DateTime<Utc>,
    blocked_state: BlockedState,
    on_resume: bool,
}
```

- `started_at`: When first 429 hit (for display)
- `blocked_state`: Reference to decision layer's BlockedState
- `on_resume`: If true, attempt recovery immediately on snapshot restore

Display format: `"💤 Resting (X min)"`

#### State Transitions

```
BlockedForDecision { RateLimitBlockedReason } → Resting
Resting → Idle (on successful recovery)
Resting → Error (on unrecoverable failure)
Resting → Resting (on failed recovery attempt, reset timer)
```

#### Recovery Timer

On entering `Resting`:
1. Start 30-minute timer (configurable)
2. When timer fires OR user triggers wake_up OR `on_resume=true`:
   - Attempt recovery via decision layer
3. If LLM call succeeds → `Resting → Idle`
4. If LLM call fails with 429 → extend another 30 min, increment retry_count
5. If LLM call fails with other error → `Resting → Error { message }`

### 3. New Action: wake_up

Add `wake_up` action available in `Resting` state. Allows user to force immediate recovery attempt rather than waiting for next scheduled retry.

### 4. Configuration

New fields in `AgentLaunchBundle` or launch config:

```rust
rate_limit_retry_interval_secs: u64 = 1800  // 30 minutes
```

### 5. Recovery on Resume

When CLI restarts with `--resume`:
1. Load snapshot with agent in `Resting` state
2. Set `on_resume = true`
3. On first tick, attempt immediate recovery
4. If fails, fall back to normal 30-min interval schedule

### 6. TUI Display

Status line format for resting agent:
```
[💤 Resting (45 min)] agent-1 | Developer
```

The elapsed time updates every minute.

## Data Flow

```
1. Work agent gets HTTP 429
2. Decision layer creates ErrorSituation with 429 info
3. TieredDecisionEngine routes to Simple tier (commit 4bcab15)
4. Rule engine decides to enter Resting state
5. AgentSlotStatus → Resting { started_at: now, blocked_state, on_resume: false }
6. Decision layer creates RateLimitBlockedReason with started_at
7. 30-min timer starts
8. Timer fires → create RateLimitRecoverySituation → Simple tier → retry
9. LLM call succeeds → Resting → Idle
10. LLM call fails (429) → stay Resting, reset timer, increment retry_count
```

## Persistence

Using `DateTime<Utc>` for `started_at` ensures the rest duration can be accurately displayed after CLI restart. The timestamp is serialized in the agent snapshot.

## Edge Cases

| Case | Behavior |
|------|----------|
| 429 during recovery attempt | Extend timer 30 min, retry_count++, stay Resting |
| Non-429 error during recovery | Resting → Error |
| User types command while resting | Can trigger wake_up or wait |
| CLI closes while resting | Snapshot saved, resume continues timer |
| All intervals exhausted | Continue retrying indefinitely (rate limits clear eventually) |
| Multiple agents share same API key | Each has independent Resting state and timer |

## Implementation Tasks

1. Add `RateLimitBlockedReason` to `decision/src/blocking.rs`
2. Add `RateLimitRecoverySituation` to `decision/src/builtin_situations.rs`
3. Add `Resting` status variant to `core/src/agent_slot.rs`
4. Add `wake_up` action
5. Add configurable `rate_limit_retry_interval_secs`
6. Implement recovery timer logic
7. Update TUI rendering for Resting state
8. Handle recovery-on-resume logic
9. Add tests for all new components
