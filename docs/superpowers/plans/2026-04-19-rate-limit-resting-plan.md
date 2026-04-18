# Rate Limit Resting State Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement a "resting" state (💤) for agents that hit HTTP 429. Agent waits patiently, retries every 30 minutes, and only recovers when both decision layer AND work agent succeed.

**Architecture:** Decision layer adds `RateLimitBlockedReason` and `RateLimitRecoverySituation`. Core layer adds `AgentSlotStatus::Resting`. Recovery timer checks via Simple tier rule engine.

**Tech Stack:** Rust (agent-decision, agent-core crates), chrono for timestamps, existing BlockingReason/BlockingState infrastructure.

---

## File Structure

| File | Responsibility |
|------|----------------|
| `decision/src/blocking.rs` | Add `RateLimitBlockedReason` struct implementing `BlockingReason` |
| `decision/src/builtin_situations.rs` | Add `RateLimitRecoverySituation` implementing `DecisionSituation` |
| `decision/src/builtin_actions.rs` | Add `wake_up` action type |
| `decision/src/tiered_engine.rs` | Route `RateLimitBlockedReason` to Simple tier |
| `core/src/agent_slot.rs` | Add `Resting { started_at, blocked_state, on_resume }` status variant |
| `core/src/launch_config.rs` | Add `rate_limit_retry_interval_secs` config field |

---

## Task 1: Add wake_up action

**Files:**
- Modify: `decision/src/builtin_actions.rs:45-47`

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn test_wake_up_action_type() {
    let action_type = wake_up();
    assert_eq!(action_type.name, "wake_up");
}

#[test]
fn test_wake_up_action_impl() {
    let action = WakeUpAction;
    assert_eq!(action.action_type(), wake_up());
    assert_eq!(action.implementation_type(), "WakeUpAction");
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p agent-decision test_wake_up_action_type -v`
Expected: FAIL with "cannot find function `wake_up`"

- [ ] **Step 3: Add wake_up function and struct**

In `decision/src/builtin_actions.rs`, add after line 55:

```rust
pub fn wake_up() -> ActionType {
    ActionType::new("wake_up")
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WakeUpAction;

impl WakeUpAction {
    pub fn new() -> Self {
        Self
    }
}

impl Default for WakeUpAction {
    fn default() -> Self {
        Self
    }
}

impl DecisionAction for WakeUpAction {
    fn action_type(&self) -> ActionType {
        wake_up()
    }

    fn implementation_type(&self) -> &'static str {
        "WakeUpAction"
    }

    fn to_prompt_format(&self) -> String {
        "WakeUp".to_string()
    }

    fn serialize_params(&self) -> String {
        "{}".to_string()
    }

    fn clone_boxed(&self) -> Box<dyn DecisionAction> {
        Box::new(self.clone())
    }
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p agent-decision test_wake_up -v`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add decision/src/builtin_actions.rs
git commit -m "feat(decision): add wake_up action"
```

---

## Task 2: Add RateLimitBlockedReason

**Files:**
- Modify: `decision/src/blocking.rs:1-15` (imports), `decision/src/blocking.rs` (add struct impl near line 280)

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn test_rate_limit_blocked_reason_description() {
    use chrono::Utc;
    let reason = RateLimitBlockedReason::new(Utc::now());
    let desc = reason.description();
    assert!(desc.contains("Rate limited"));
    assert!(desc.contains("💤"));
}

#[test]
fn test_rate_limit_blocked_reason_properties() {
    use chrono::Utc;
    let reason = RateLimitBlockedReason::new(Utc::now());
    assert!(!reason.can_auto_resolve());
    assert!(reason.auto_resolve_action().is_none());
    assert_eq!(reason.urgency(), UrgencyLevel::Low);
    assert!(reason.expires_at().is_none());
}

#[test]
fn test_rate_limit_blocked_reason_elapsed() {
    use chrono::Utc;
    use std::time::Duration;
    let started = Utc::now() - chrono::Duration::minutes(10);
    let reason = RateLimitBlockedReason::new(started);
    let elapsed = reason.elapsed_minutes();
    assert!(elapsed >= 9 && elapsed <= 11);
}

#[test]
fn test_rate_limit_blocked_reason_retry_count() {
    use chrono::Utc;
    let reason = RateLimitBlockedReason::new(Utc::now())
        .with_retry_count(3);
    assert_eq!(reason.retry_count(), 3);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p agent-decision test_rate_limit_blocked_reason -v`
Expected: FAIL with "cannot find type `RateLimitBlockedReason`"

- [ ] **Step 3: Add RateLimitBlockedReason struct and impl**

After the existing `WaitingBlocking` impl (around line 280), add:

```rust
/// Rate limit blocked reason - when agent hits HTTP 429
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateLimitBlockedReason {
    started_at: DateTime<Utc>,
    last_retry_at: Option<DateTime<Utc>>,
    retry_count: u32,
    interval_secs: u64,
}

impl RateLimitBlockedReason {
    pub fn new(started_at: DateTime<Utc>) -> Self {
        Self {
            started_at,
            last_retry_at: None,
            retry_count: 0,
            interval_secs: 1800, // 30 minutes default
        }
    }

    pub fn with_interval_secs(self, interval_secs: u64) -> Self {
        Self { interval_secs, ..self }
    }

    pub fn with_last_retry_at(self, last_retry_at: DateTime<Utc>) -> Self {
        Self { last_retry_at: Some(last_retry_at), ..self }
    }

    pub fn with_retry_count(self, retry_count: u32) -> Self {
        Self { retry_count, ..self }
    }

    pub fn started_at(&self) -> DateTime<Utc> {
        self.started_at
    }

    pub fn last_retry_at(&self) -> Option<DateTime<Utc>> {
        self.last_retry_at
    }

    pub fn retry_count(&self) -> u32 {
        self.retry_count
    }

    pub fn interval_secs(&self) -> u64 {
        self.interval_secs
    }

    /// Minutes since first 429
    pub fn elapsed_minutes(&self) -> i64 {
        (Utc::now() - self.started_at).num_minutes()
    }

    /// Whether enough time has passed to retry
    pub fn can_retry_now(&self) -> bool {
        if let Some(last) = self.last_retry_at {
            let elapsed = (Utc::now() - last).num_seconds() as u64;
            elapsed >= self.interval_secs
        } else {
            true // Never tried, can retry now
        }
    }

    /// Record a retry attempt
    pub fn record_retry(&mut self) {
        self.last_retry_at = Some(Utc::now());
        self.retry_count += 1;
    }
}

impl BlockingReason for RateLimitBlockedReason {
    fn reason_type(&self) -> &'static str {
        "rate_limit"
    }

    fn urgency(&self) -> UrgencyLevel {
        UrgencyLevel::Low
    }

    fn expires_at(&self) -> Option<DateTime<Utc>> {
        None // No expiration, wait indefinitely
    }

    fn can_auto_resolve(&self) -> bool {
        false // Must actually try LLM call to verify recovery
    }

    fn auto_resolve_action(&self) -> Option<AutoAction> {
        None
    }

    fn description(&self) -> String {
        let mins = self.elapsed_minutes();
        format!("💤 Rate limited ({} min)", mins)
    }

    fn clone_boxed(&self) -> Box<dyn BlockingReason> {
        Box::new(self.clone())
    }
}
```

- [ ] **Step 3b: Add serde import if needed**

Verify `blocking.rs` has `use serde::{Deserialize, Serialize};` at top (should already have it from line 7).

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p agent-decision test_rate_limit_blocked_reason -v`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add decision/src/blocking.rs
git commit -m "feat(decision): add RateLimitBlockedReason for 429 handling"
```

---

## Task 3: Add RateLimitRecoverySituation

**Files:**
- Modify: `decision/src/builtin_situations.rs` (add struct, impl, tests, register function)

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn test_rate_limit_recovery_situation_type() {
    use chrono::Utc;
    let situation = RateLimitRecoverySituation::new(Utc::now(), 0);
    assert_eq!(situation.situation_type(), SituationType::new("rate_limit_recovery"));
}

#[test]
fn test_rate_limit_recovery_available_actions() {
    use chrono::Utc;
    let situation = RateLimitRecoverySituation::new(Utc::now(), 0);
    let actions = situation.available_actions();
    assert!(actions.contains(&ActionType::new("retry")));
    assert!(actions.contains(&ActionType::new("request_human")));
}

#[test]
fn test_rate_limit_recovery_requires_human_false() {
    use chrono::Utc;
    let situation = RateLimitRecoverySituation::new(Utc::now(), 0);
    assert!(!situation.requires_human());
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p agent-decision test_rate_limit_recovery -v`
Expected: FAIL with "cannot find type `RateLimitRecoverySituation`"

- [ ] **Step 3: Add RateLimitRecoverySituation struct and impl**

In `decision/src/builtin_situations.rs`, add after `AgentIdleSituation` struct (around line 475):

```rust
/// Situation: Rate Limit Recovery
///
/// Triggered when an agent is in resting state due to HTTP 429 and needs to
/// decide whether to attempt recovery (retry) or continue waiting.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateLimitRecoverySituation {
    started_at: DateTime<Utc>,
    retry_count: u32,
    last_error: Option<String>,
}

impl RateLimitRecoverySituation {
    pub fn new(started_at: DateTime<Utc>, retry_count: u32) -> Self {
        Self {
            started_at,
            retry_count,
            last_error: None,
        }
    }

    pub fn with_last_error(self, error: impl Into<String>) -> Self {
        Self {
            last_error: Some(error.into()),
            ..self
        }
    }

    /// Minutes since first 429
    pub fn elapsed_minutes(&self) -> i64 {
        (Utc::now() - self.started_at).num_minutes()
    }
}

impl Default for RateLimitRecoverySituation {
    fn default() -> Self {
        Self::new(Utc::now(), 0)
    }
}

impl DecisionSituation for RateLimitRecoverySituation {
    fn situation_type(&self) -> SituationType {
        SituationType::new("rate_limit_recovery")
    }

    fn implementation_type(&self) -> &'static str {
        "RateLimitRecoverySituation"
    }

    fn requires_human(&self) -> bool {
        false
    }

    fn human_urgency(&self) -> UrgencyLevel {
        UrgencyLevel::Low
    }

    fn to_prompt_text(&self) -> String {
        let mins = self.elapsed_minutes();
        let error_text = self.last_error.as_deref().unwrap_or("None");
        format!(
            "Rate Limit Recovery:\nFirst 429 hit: {} min ago\nRetry attempts: {}\nLast error: {}\n\n\
            Option: retry (try LLM call to check if rate limit cleared)\n\
            Option: request_human (ask user for manual intervention)",
            mins, self.retry_count, error_text
        )
    }

    fn available_actions(&self) -> Vec<ActionType> {
        vec![
            ActionType::new("retry"),
            ActionType::new("request_human"),
        ]
    }

    fn clone_boxed(&self) -> Box<dyn DecisionSituation> {
        Box::new(self.clone())
    }
}
```

- [ ] **Step 4: Register in builtin_situations module**

Update the `register_situation_builtins` function to register `RateLimitRecoverySituation` (but note it may not need default registration since it's created dynamically).

- [ ] **Step 5: Run test to verify it passes**

Run: `cargo test -p agent-decision test_rate_limit_recovery -v`
Expected: PASS

- [ ] **Step 6: Commit**

```bash
git add decision/src/builtin_situations.rs
git commit -m "feat(decision): add RateLimitRecoverySituation"
```

---

## Task 4: Route RateLimitBlockedReason to Simple tier

**Files:**
- Modify: `decision/src/tiered_engine.rs:45-75`

- [ ] **Step 1: Find the rate limit detection code**

Locate the existing 429 detection in `TieredDecisionEngine::from_situation` (around line 54-70).

- [ ] **Step 2: Add routing for RateLimitBlockedReason**

Modify the error handling section to also check for rate limit blocked state:

```rust
// Rate limit blocked (429) - use Simple tier to avoid LLM deadlock
if type_name == "rate_limit" {
    return DecisionTier::Simple;
}
```

This goes before the return to Complex in the error block. The key logic is: when the situation is a `BlockedState` wrapping `RateLimitBlockedReason`, the tier selection should route to Simple.

Actually, looking at the code more carefully, `TieredDecisionEngine::from_situation` takes a `&dyn DecisionSituation`, not a `BlockedState`. The routing for blocked states happens elsewhere. Let me verify the actual flow...

Looking at the code flow:
1. When agent gets 429, `ErrorSituation` is created with 429 error info
2. `TieredDecisionEngine::from_situation` routes 429 errors to Simple tier
3. Rule engine decides action → creates `BlockedState` with `RateLimitBlockedReason`
4. Later when checking recovery, `RateLimitRecoverySituation` is created

The current 429 detection in `from_situation` should handle the initial ErrorSituation routing. But we also need to make sure that when a blocked agent checks for recovery, the tier selection is correct.

- [ ] **Step 3: Verify existing test still passes**

Run: `cargo test -p agent-decision test_decision_tier_from_error_rate_limit -v`
Expected: PASS (existing test should still work)

- [ ] **Step 4: Commit**

```bash
git add decision/src/tiered_engine.rs
git commit -m "fix(decision): ensure rate limit scenarios use Simple tier"
```

Note: This task may be a no-op if the existing 429 detection already covers the flow. Verify with tests.

---

## Task 5: Add Resting status to AgentSlotStatus

**Files:**
- Modify: `core/src/agent_slot.rs:23-48` (enum), `core/src/agent_slot.rs:50-73` (PartialEq), `core/src/agent_slot.rs` (add state transition validity)

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn test_agent_slot_status_resting_partial_eq() {
    use chrono::Utc;
    use agent_decision::BlockedState;
    let blocked_state = BlockedState::new(Box::new(
        agent_decision::RateLimitBlockedReason::new(Utc::now())
    ));
    let status1 = AgentSlotStatus::Resting {
        started_at: Utc::now(),
        blocked_state: blocked_state.clone(),
        on_resume: false,
    };
    let status2 = AgentSlotStatus::Resting {
        started_at: Utc::now(),
        blocked_state: blocked_state.clone(),
        on_resume: false,
    };
    assert_eq!(status1, status2);
}

#[test]
fn test_agent_slot_status_resting_display() {
    let status = AgentSlotStatus::Resting {
        started_at: Utc::now(),
        blocked_state: BlockedState::new(Box::new(
            agent_decision::RateLimitBlockedReason::new(Utc::now())
        )),
        on_resume: false,
    };
    let display = format!("{:?}", status);
    assert!(display.contains("Resting"));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p agent-core test_agent_slot_status_resting -v`
Expected: FAIL with "variant `Resting` does not exist"

- [ ] **Step 3: Add Resting variant to AgentSlotStatus**

In `core/src/agent_slot.rs`, add to enum after line 45:

```rust
/// Agent is resting due to rate limit (💤), waiting for quota to recover
Resting {
    /// When first 429 occurred
    started_at: DateTime<Utc>,
    /// Reference to decision layer's blocked state
    blocked_state: BlockedState,
    /// If true, attempt recovery immediately on snapshot restore
    on_resume: bool,
},
```

Add the import at top of file:
```rust
use chrono::{DateTime, Utc};
```

- [ ] **Step 4: Update PartialEq implementation**

In the `PartialEq` impl for `AgentSlotStatus` (around line 63), add:

```rust
(Self::Resting { started_at: a, blocked_state: _, on_resume: _ },
 Self::Resting { started_at: b, blocked_state: _, on_resume: _ }) => a == b,
```

Note: We compare by `started_at` only since BlockedState equality is complex.

- [ ] **Step 5: Update state transition validity**

Find the `valid_transition` match arm and add:
```rust
// Resting can go to Idle (recovery), Error (unrecoverable), or stay Resting
(Self::Resting { .. }, Self::Idle) => true,
(Self::Resting { .. }, Self::Error { .. }) => true,
(Self::Resting { .. }, Self::Resting { .. }) => true,
```

Also allow transitions INTO Resting:
```rust
// BlockedForDecision can go to Resting (rate limit escalation)
(Self::BlockedForDecision { .. }, Self::Resting { .. }) => true,
```

- [ ] **Step 6: Run test to verify it passes**

Run: `cargo test -p agent-core test_agent_slot_status_resting -v`
Expected: PASS

- [ ] **Step 7: Commit**

```bash
git add core/src/agent_slot.rs
git commit -m "feat(core): add Resting status for rate-limited agents"
```

---

## Task 6: Add rate_limit_retry_interval_secs config

**Files:**
- Modify: `core/src/launch_config.rs`

- [ ] **Step 1: Find AgentLaunchBundle struct**

Locate `AgentLaunchBundle` or similar config struct in `core/src/launch_config.rs`.

- [ ] **Step 2: Add config field**

Add field:
```rust
/// Interval in seconds between rate limit recovery attempts
#[serde(default = "default_rate_limit_retry_interval")]
pub rate_limit_retry_interval_secs: u64,
```

Add default function:
```rust
fn default_rate_limit_retry_interval() -> u64 {
    1800 // 30 minutes
}
```

- [ ] **Step 3: Run build to verify**

Run: `cargo build -p agent-core`
Expected: compiles successfully

- [ ] **Step 4: Commit**

```bash
git add core/src/launch_config.rs
git commit -m "feat(core): add rate_limit_retry_interval_secs config"
```

---

## Task 7: Implement recovery timer logic

**Files:**
- Modify: `core/src/agent_slot.rs` (add timer handling in `AgentSlot` impl)

**Recovery timer logic:**

The `AgentSlot` needs to:
1. When entering `Resting` state, store `blocked_state: BlockedState` and record start time
2. On each tick while in `Resting`:
   - If `on_resume = true`: attempt immediate recovery
   - If `on_resume = false`: check if `interval_secs` has passed since last retry
3. On recovery attempt via decision layer using `RateLimitRecoverySituation`:
   - If LLM call succeeds → transition to `Idle`
   - If 429 again → stay in `Resting`, update `last_retry_at`, reset timer
   - If other error → transition to `Error`

**Key data stored in Resting status:**
```rust
Resting {
    started_at: DateTime<Utc>,       // When first 429 occurred
    blocked_state: BlockedState,      // From decision layer
    on_resume: bool,                  // If true, try recovery immediately
}
```

**For resuming with short interval:**
- On snapshot restore, calculate `elapsed = now - last_retry_at` (or `now - started_at` if never retried)
- If `elapsed < interval_secs`: user restarted quickly, keep waiting — set `on_resume = false`
- If `elapsed >= interval_secs`: long restart, try recovery — set `on_resume = true`

- [ ] **Step 1: Add helper methods to AgentSlotStatus**

```rust
impl AgentSlotStatus {
    pub fn is_resting(&self) -> bool {
        matches!(self, Self::Resting { .. })
    }

    pub fn resting_started_at(&self) -> Option<DateTime<Utc>> {
        match self {
            Self::Resting { started_at, .. } => Some(*started_at),
            _ => None,
        }
    }

    pub fn is_resting_with_on_resume(&self) -> bool {
        match self {
            Self::Resting { on_resume, .. } => *on_resume,
            _ => false,
        }
    }
}
```

- [ ] **Step 2: Add AgentSlot::enter_resting method**

```rust
impl AgentSlot {
    pub fn enter_resting(&mut self, blocked_state: BlockedState) {
        let started_at = match blocked_state.reason().reason_type() {
            "rate_limit" => {
                // RateLimitBlockedReason has started_at
                // For other types, use Utc::now()
                Utc::now()
            }
            _ => Utc::now(),
        };

        self.status = AgentSlotStatus::Resting {
            started_at,
            blocked_state,
            on_resume: false,
        };
    }

    pub fn attempt_rate_limit_recovery(&mut self) -> Result<(), DecisionError> {
        // Use RateLimitRecoverySituation with decision engine
        // If succeeds → transition to Idle
        // If 429 → stay Resting, record retry
        // If error → transition to Error
    }
}
```

- [ ] **Step 3: On each tick, check recovery condition**

In the existing tick/loop where agent state is checked:

```rust
if let AgentSlotStatus::Resting { started_at, blocked_state, on_resume } = &self.status {
    if *on_resume || blocked_state.reason().can_retry_now() {
        match self.attempt_rate_limit_recovery() {
            Ok(()) => {
                self.status = AgentSlotStatus::Idle;
            }
            Err(DecisionError::RateLimited) => {
                // Still rate limited, stay resting, record retry
                blocked_state.reason().record_retry();
                // Reset on_resume flag after recovery attempt
                self.status = AgentSlotStatus::Resting {
                    started_at: *started_at,
                    blocked_state: blocked_state.clone(),
                    on_resume: false,
                };
            }
            Err(e) => {
                self.status = AgentSlotStatus::Error { message: e.to_string() };
            }
        }
    }
}
```

- [ ] **Step 4: Commit**

```bash
git add core/src/agent_slot.rs
git commit -m "feat(core): implement recovery timer logic for Resting state"
```

---

## Task 8: Update TUI rendering for Resting state

**Files:**
- Modify: `tui/src/` (rendering components)

- [ ] **Step 1: Find TUI status rendering**

Locate where agent slot statuses are rendered in the TUI.

- [ ] **Step 2: Add Resting display**

Show "💤 Resting (X min)" where X is minutes since `started_at`.

- [ ] **Step 3: Commit**

```bash
git add tui/
git commit -m "feat(tui): display resting status with elapsed time"
```

---

## Task 9: Handle recovery-on-resume logic

**Files:**
- Modify: Snapshot restore logic in core

**Resume behavior:**

When CLI restarts with `--resume` and agent is in `Resting` state:

1. **Load snapshot with Resting status** (contains `started_at`, `blocked_state`, `on_resume`)
2. **Calculate elapsed since last retry**:
   - If `last_retry_at.is_some()`: `elapsed = now - last_retry_at`
   - Else: `elapsed = now - started_at` (never retried)
3. **Determine `on_resume` value**:
   - If `elapsed >= rate_limit_retry_interval_secs`: try immediately → `on_resume = true`
   - If `elapsed < rate_limit_retry_interval_secs`: keep waiting → `on_resume = false`

This handles both cases:
- **Short restart** (5 min after 30-min interval): Don't hammer API, just wait
- **Long restart** (1-2 hours later): Worth trying immediately since quota likely recovered

- [ ] **Step 1: Find snapshot restore code**

Locate where agent slots are restored from snapshot. Look for `AgentSlot::restore`, `AgentSlot::from_snapshot`, or similar.

- [ ] **Step 2: Add resume elapsed calculation**

In the restore path:

```rust
fn restore_from_snapshot(snapshot: &AgentSlotSnapshot) -> Self {
    let status = match snapshot.status {
        AgentSlotStatus::Resting { started_at, blocked_state, on_resume: _ } => {
            // Determine on_resume based on elapsed time
            let interval_secs = config.rate_limit_retry_interval_secs;
            let elapsed = blocked_state.reason().last_retry_at()
                .map(|last| (Utc::now() - last).num_seconds() as u64)
                .unwrap_or_else(|| (Utc::now() - started_at).num_seconds() as u64);

            let on_resume = elapsed >= interval_secs;

            AgentSlotStatus::Resting {
                started_at,
                blocked_state,
                on_resume,
            }
        }
        other => other,
    };

    AgentSlot { status, ... }
}
```

- [ ] **Step 3: Commit**

```bash
git add [snapshot restore files]
git commit -m "feat(core): handle Resting state on resume with proper timer logic"
```

---

## Self-Review Checklist

- [ ] Spec coverage: Each requirement in spec has a corresponding task
- [ ] Placeholder scan: No TBD/TODO in steps
- [ ] Type consistency: `DateTime<Utc>`, `BlockingReason`, `DecisionSituation` types consistent across tasks
- [ ] Test code: Each task has actual failing test before implementation

## Execution Options

**1. Subagent-Driven (recommended)** - I dispatch a fresh subagent per task, review between tasks, fast iteration

**2. Inline Execution** - Execute tasks in this session using executing-plans, batch execution with checkpoints

**Which approach?**
