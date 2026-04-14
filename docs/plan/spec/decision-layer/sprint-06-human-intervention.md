# Sprint 6: Human Intervention (BlockingReason Trait)

## Metadata

- Sprint ID: `decision-sprint-006`
- Title: `Human Intervention System (Trait-Based)`
- Duration: 2 weeks
- Priority: P0 (Critical)
- Status: `Backlog`
- Created: 2026-04-14
- Updated: 2026-04-14 (Architecture Evolution)

## TDD Reference

See [Test Specification](test-specification.md) for detailed TDD test tasks:
- Sprint 6 Tests: T6.1.T1-T6.5.T6 (27 tests)
- Test execution: Write failing tests first, implement minimum, refactor

## Architecture Evolution

Human intervention now uses **BlockingReason trait**:
- HumanDecisionBlocking implements BlockingReason
- AgentSlotStatus uses generic Blocked(BlockedState)
- BlockingReasonRegistry for extensible blocking types

See Sprint 1 Story 1.6 for BlockingReason trait definition.

## Sprint Goal

Implement critical decision escalation to human users using BlockingReason trait. When decisions are important, Decision Agent creates HumanDecisionBlocking, Main Agent enters Blocked(BlockedState), and waits for human decision.

## Stories

### Story 6.1: Criticality Evaluation Integration

**Priority**: P0
**Effort**: 2 points
**Status**: Backlog

Integrate criticality evaluation into DecisionSituation trait.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T6.1.1 | Add `requires_human()` to DecisionSituation trait | Todo | - |
| T6.1.2 | Add `human_urgency()` to DecisionSituation trait | Todo | - |
| T6.1.3 | Implement criticality in WaitingForChoiceSituation | Todo | - |
| T6.1.4 | Implement criticality in ClaimsCompletionSituation | Todo | - |
| T6.1.5 | Write unit tests for requires_human | Todo | - |

#### TDD Test Tasks

| Test ID | Definition |
|---------|------------|
| T6.1.T1 | requires_human() returns true for critical |
| T6.1.T2 | human_urgency() returns correct level |
| T6.1.T3 | Critical command detected in WaitingForChoice |
| T6.1.T4 | Low confidence detected in ClaimsCompletion |
| T6.1.T5 | Project rule affects criticality |
| T6.1.T6 | Situation builds correct blocking reason |

#### Acceptance Criteria

- DecisionSituation trait includes human escalation methods
- Criticality determined by situation data
- BlockingReason created when requires_human

#### Technical Notes

```rust
/// Extended DecisionSituation trait (from Sprint 1)
/// Already includes:
/// - requires_human() -> bool
/// - human_urgency() -> UrgencyLevel

/// WaitingForChoiceSituation criticality
impl WaitingForChoiceSituation {
    pub fn new_with_criticality(options: Vec<ChoiceOption>, permission_type: String, params: &Value) -> Self {
        let critical = Self::is_critical_permission(&permission_type, params);
        
        Self {
            options,
            permission_type: Some(permission_type),
            critical,
        }
    }
    
    fn is_critical_permission(permission_type: &str, params: &Value) -> bool {
        match permission_type {
            "execute" | "exec" => {
                params.get("command")
                    .and_then(|c| c.as_str())
                    .map(|cmd| {
                        cmd.contains("rm -rf") ||
                        cmd.contains("sudo") ||
                        cmd.contains("chmod 777") ||
                        cmd.contains("DROP") ||
                        cmd.contains("DELETE FROM")
                    })
                    .unwrap_or(false)
            }
            "write" | "edit" => {
                params.get("path")
                    .and_then(|p| p.as_str())
                    .map(|path| {
                        path.contains(".env") ||
                        path.contains("credentials") ||
                        path.contains("secrets") ||
                        path.contains("auth")
                    })
                    .unwrap_or(false)
            }
            _ => false,
        }
    }
}

/// ClaimsCompletionSituation criticality
impl ClaimsCompletionSituation {
    pub fn is_critical(&self) -> bool {
        // Critical if: reflection exhausted + low confidence
        self.reflection_rounds >= self.max_reflection_rounds && self.confidence < 0.7
    }
}
```

---

### Story 6.2: HumanDecisionBlocking Implementation

**Priority**: P0
**Effort**: 3 points
**Status**: Backlog

Implement HumanDecisionBlocking as BlockingReason trait implementation.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T6.2.1 | Implement `HumanDecisionBlocking` struct | Todo | - |
| T6.2.2 | Implement BlockingReason trait for HumanDecisionBlocking | Todo | - |
| T6.2.3 | Create `HumanDecisionRequest` struct | Todo | - |
| T6.2.4 | Create `HumanDecisionResponse` struct | Todo | - |
| T6.2.5 | Implement timeout handling | Todo | - |
| T6.2.6 | Write unit tests for blocking implementation | Todo | - |

#### TDD Test Tasks

| Test ID | Definition |
|---------|------------|
| T6.2.T1 | reason_type() returns "human_decision" |
| T6.2.T2 | urgency() matches situation.human_urgency() |
| T6.2.T3 | expires_at() returns timeout time |
| T6.2.T4 | can_auto_resolve() returns true if recommendation exists |
| T6.2.T5 | auto_resolve_action() returns FollowRecommendation |
| T6.2.T6 | clone_boxed() works correctly |

#### Acceptance Criteria

- HumanDecisionBlocking implements BlockingReason trait
- Timeout expiration tracked
- Auto-resolve with recommendation supported

#### Technical Notes

```rust
/// Human decision blocking - implements BlockingReason
/// (From Sprint 1, now with full implementation)
pub struct HumanDecisionBlocking {
    /// Decision request ID
    request_id: DecisionRequestId,
    
    /// Source situation
    situation: Box<dyn DecisionSituation>,
    
    /// Available options
    options: Vec<ChoiceOption>,
    
    /// Decision Agent recommendation
    recommendation: Option<Recommendation>,
    
    /// Expiration time
    expires_at: DateTime<Utc>,
    
    /// Blocking context
    context: BlockingContext,
}

/// Recommendation from Decision Agent
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Recommendation {
    /// Recommended action
    action: Box<dyn DecisionAction>,
    
    /// Reasoning
    reasoning: String,
    
    /// Confidence (0.0-1.0)
    confidence: f64,
}

/// Human decision request (for queue)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HumanDecisionRequest {
    pub id: DecisionRequestId,
    pub agent_id: AgentId,
    pub situation_type: SituationType,
    pub situation_data: String, // Serialized situation
    pub options: Vec<ChoiceOption>,
    pub recommendation: Option<Recommendation>,
    pub urgency: UrgencyLevel,
    pub created_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub context: BlockingContext,
}

/// Human decision response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HumanDecisionResponse {
    pub request_id: DecisionRequestId,
    pub selection: HumanSelection,
    pub responded_at: DateTime<Utc>,
    pub response_time_ms: u64,
}

/// Human selection
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum HumanSelection {
    /// Selected specific option
    Selected { option_id: String },
    
    /// Accepted recommendation
    AcceptedRecommendation,
    
    /// Custom instruction provided
    Custom { instruction: String },
    
    /// Task skipped
    Skipped,
    
    /// Operation cancelled
    Cancelled,
}

impl HumanDecisionBlocking {
    pub fn new(
        situation: Box<dyn DecisionSituation>,
        options: Vec<ChoiceOption>,
        recommendation: Option<Recommendation>,
        timeout_config: &HumanDecisionTimeoutConfig,
    ) -> Self {
        let urgency = situation.human_urgency();
        let timeout_ms = match urgency {
            UrgencyLevel::Critical => timeout_config.critical_timeout_ms,
            UrgencyLevel::High => timeout_config.high_timeout_ms,
            UrgencyLevel::Medium => timeout_config.default_timeout_ms,
            UrgencyLevel::Low => timeout_config.low_timeout_ms,
        };
        
        Self {
            request_id: DecisionRequestId::new(),
            situation,
            options,
            recommendation,
            expires_at: Utc::now() + Duration::milliseconds(timeout_ms as i64),
            context: BlockingContext::default(),
        }
    }
    
    pub fn to_request(&self, agent_id: AgentId) -> HumanDecisionRequest {
        HumanDecisionRequest {
            id: self.request_id.clone(),
            agent_id,
            situation_type: self.situation.situation_type(),
            situation_data: self.situation.to_prompt_text(),
            options: self.options.clone(),
            recommendation: self.recommendation.clone(),
            urgency: self.situation.human_urgency(),
            created_at: Utc::now(),
            expires_at: self.expires_at,
            context: self.context.clone(),
        }
    }
    
    pub fn create_blocked_state(&self) -> BlockedState {
        BlockedState::new(Box::new(self.clone()))
    }
}

impl BlockingReason for HumanDecisionBlocking {
    fn reason_type(&self) -> &str { "human_decision" }
    
    fn urgency(&self) -> UrgencyLevel {
        self.situation.human_urgency()
    }
    
    fn expires_at(&self) -> Option<DateTime<Utc>> {
        Some(self.expires_at)
    }
    
    fn can_auto_resolve(&self) -> bool {
        self.recommendation.is_some()
    }
    
    fn auto_resolve_action(&self) -> Option<AutoAction> {
        if self.recommendation.is_some() {
            Some(AutoAction::FollowRecommendation)
        } else if !self.options.is_empty() {
            Some(AutoAction::SelectDefault) // Select first option
        } else {
            Some(AutoAction::Cancel)
        }
    }
    
    fn description(&self) -> String {
        format!(
            "Waiting for human decision: {} (urgency: {})",
            self.situation.situation_type().name,
            self.urgency()
        )
    }
    
    fn clone_boxed(&self) -> Box<dyn BlockingReason> {
        Box::new(self.clone())
    }
}

/// Timeout configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HumanDecisionTimeoutConfig {
    /// Default timeout (1 hour)
    pub default_timeout_ms: u64,
    
    /// High urgency timeout (30 minutes)
    pub high_timeout_ms: u64,
    
    /// Critical urgency timeout (15 minutes)
    pub critical_timeout_ms: u64,
    
    /// Low urgency timeout (2 hours)
    pub low_timeout_ms: u64,
    
    /// Warning before timeout (1 minute)
    pub warning_before_ms: u64,
    
    /// Default action on timeout
    pub timeout_default: AutoAction,
}

impl Default for HumanDecisionTimeoutConfig {
    fn default() -> Self {
        Self {
            default_timeout_ms: 3600000,  // 1 hour
            high_timeout_ms: 1800000,     // 30 min
            critical_timeout_ms: 900000,  // 15 min
            low_timeout_ms: 7200000,      // 2 hours
            warning_before_ms: 60000,     // 1 min
            timeout_default: AutoAction::FollowRecommendation,
        }
    }
}
```

---

### Story 6.3: HumanDecisionQueue with Priority

**Priority**: P0
**Effort**: 3 points
**Status**: Backlog

Implement queue for managing pending human decisions with priority ordering.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T6.3.1 | Create `HumanDecisionQueue` struct | Todo | - |
| T6.3.2 | Implement priority queues (Critical/High/Medium/Low) | Todo | - |
| T6.3.3 | Implement `push()` with urgency-based routing | Todo | - |
| T6.3.4 | Implement `pop()` with priority order | Todo | - |
| T6.3.5 | Implement timeout checking | Todo | - |
| T6.3.6 | Implement history tracking | Todo | - |
| T6.3.7 | Write unit tests for queue | Todo | - |

#### TDD Test Tasks

| Test ID | Definition |
|---------|------------|
| T6.3.T1 | Critical request added to critical queue |
| T6.3.T2 | Pop returns Critical > High > Medium > Low |
| T6.3.T3 | Expired requests detected |
| T6.3.T4 | Request removed on completion |
| T6.3.T5 | History preserved correctly |
| T6.3.T6 | Warning notification sent before timeout |

#### Acceptance Criteria

- Queue stores pending human decisions by priority
- Priority ordering works correctly
- Timeout handling implemented

#### Technical Notes

```rust
/// Human decision queue with priority ordering
pub struct HumanDecisionQueue {
    /// Priority queues
    critical: VecDeque<HumanDecisionRequest>,
    high: VecDeque<HumanDecisionRequest>,
    medium: VecDeque<HumanDecisionRequest>,
    low: VecDeque<HumanDecisionRequest>,
    
    /// Completed requests (history)
    history: Vec<HumanDecisionResponse>,
    
    /// Timeout configuration
    timeout_config: HumanDecisionTimeoutConfig,
    
    /// Notification sender
    notification_tx: mpsc::Sender<HumanDecisionNotification>,
}

impl HumanDecisionQueue {
    pub fn new(timeout_config: HumanDecisionTimeoutConfig, notification_tx: mpsc::Sender<HumanDecisionNotification>) -> Self {
        Self {
            critical: VecDeque::new(),
            high: VecDeque::new(),
            medium: VecDeque::new(),
            low: VecDeque::new(),
            history: Vec::new(),
            timeout_config,
            notification_tx,
        }
    }
    
    /// Push request to appropriate priority queue
    pub fn push(&mut self, request: HumanDecisionRequest) {
        match request.urgency {
            UrgencyLevel::Critical => self.critical.push_back(request),
            UrgencyLevel::High => self.high.push_back(request),
            UrgencyLevel::Medium => self.medium.push_back(request),
            UrgencyLevel::Low => self.low.push_back(request),
        }
        
        // Send notification
        let _ = self.notification_tx.send(HumanDecisionNotification::NewRequest {
            request,
            unread_count: self.total_pending(),
        });
    }
    
    /// Pop next request (priority order)
    pub fn pop(&mut self) -> Option<HumanDecisionRequest> {
        // Priority: Critical > High > Medium > Low
        if let Some(req) = self.critical.pop_front() { return Some(req); }
        if let Some(req) = self.high.pop_front() { return Some(req); }
        if let Some(req) = self.medium.pop_front() { return Some(req); }
        self.low.pop_front()
    }
    
    /// Complete request
    pub fn complete(&mut self, response: HumanDecisionResponse) -> Option<HumanDecisionRequest> {
        // Find and remove request
        let request = self.find_and_remove(&response.request_id)?;
        
        // Add to history
        self.history.push(response);
        
        // Send completion notification
        let _ = self.notification_tx.send(HumanDecisionNotification::Completed {
            request_id: response.request_id.clone(),
        });
        
        Some(request)
    }
    
    fn find_and_remove(&mut self, id: &DecisionRequestId) -> Option<HumanDecisionRequest> {
        for queue in [&mut self.critical, &mut self.high, &mut self.medium, &mut self.low] {
            if let Some(pos) = queue.iter().position(|r| r.id == *id) {
                return queue.remove(pos);
            }
        }
        None
    }
    
    /// Check for timeout expiring
    pub fn check_timeout(&mut self) -> Vec<ExpiredDecision> {
        let now = Utc::now();
        let warning_threshold = now + Duration::milliseconds(self.timeout_config.warning_before_ms as i64);
        
        let mut expired = Vec::new();
        
        for queue in [&self.critical, &self.high, &self.medium, &self.low] {
            for req in queue {
                // Expired
                if now > req.expires_at {
                    expired.push(ExpiredDecision {
                        request_id: req.id.clone(),
                        default_action: self.timeout_config.timeout_default.clone(),
                    });
                }
                // Approaching timeout - send warning
                else if req.expires_at < warning_threshold {
                    let remaining_ms = (req.expires_at - now).num_milliseconds() as u64;
                    let _ = self.notification_tx.send(HumanDecisionNotification::ApproachingTimeout {
                        request_id: req.id.clone(),
                        remaining_seconds: remaining_ms / 1000,
                    });
                }
            }
        }
        
        expired
    }
    
    pub fn total_pending(&self) -> usize {
        self.critical.len() + self.high.len() + self.medium.len() + self.low.len()
    }
    
    pub fn history(&self) -> &[HumanDecisionResponse] {
        &self.history
    }
}

/// Expired decision
pub struct ExpiredDecision {
    pub request_id: DecisionRequestId,
    pub default_action: AutoAction,
}

/// Human decision notification
pub enum HumanDecisionNotification {
    /// New request added
    NewRequest {
        request: HumanDecisionRequest,
        unread_count: usize,
    },
    
    /// Request approaching timeout
    ApproachingTimeout {
        request_id: DecisionRequestId,
        remaining_seconds: u64,
    },
    
    /// Request timed out
    TimeoutExpired {
        request_id: DecisionRequestId,
        default_action: AutoAction,
    },
    
    /// Request completed
    Completed {
        request_id: DecisionRequestId,
    },
    
    /// Urgent request
    UrgentRequest {
        request: HumanDecisionRequest,
    },
}
```

---

### Story 6.4a: TUI Decision Modal Display

**Priority**: P0
**Effort**: 2 points
**Status**: Backlog

Implement TUI modal to display decision request and recommendation.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T6.4a.1 | Create `HumanDecisionModal` component | Todo | - |
| T6.4a.2 | Display request details | Todo | - |
| T6.4a.3 | Display situation.to_prompt_text() | Todo | - |
| T6.4a.4 | Display recommendation | Todo | - |
| T6.4a.5 | Display urgency badge | Todo | - |
| T6.4a.6 | Write unit tests for display | Todo | - |

#### TDD Test Tasks

| Test ID | Definition |
|---------|------------|
| T6.4a.T1 | Modal displays request ID |
| T6.4a.T2 | Modal displays situation text |
| T6.4a.T3 | Modal displays options |
| T6.4a.T4 | Modal displays recommendation |
| T6.4a.T5 | Urgency badge shown for Critical |

#### Acceptance Criteria

- Modal displays all request information
- Situation text rendered from trait
- Urgency indicator visible

---

### Story 6.4b: TUI Decision Modal Interaction

**Priority**: P0
**Effort**: 3 points
**Status**: Backlog

Implement keyboard interaction for decision modal.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T6.4b.1 | Implement option selection (A/B/C/D) | Todo | - |
| T6.4b.2 | Implement Enter to confirm | Todo | - |
| T6.4b.3 | Implement R for recommendation | Todo | - |
| T6.4b.4 | Implement C for custom input | Todo | - |
| T6.4b.5 | Implement S for skip | Todo | - |
| T6.4b.6 | Implement Esc for cancel | Todo | - |
| T6.4b.7 | Write unit tests for interaction | Todo | - |

#### TDD Test Tasks

| Test ID | Definition |
|---------|------------|
| T6.4b.T1 | A key selects first option |
| T6.4b.T2 | Enter confirms current selection |
| T6.4b.T3 | R accepts recommendation |
| T6.4b.T4 | C opens custom input |
| T6.4b.T5 | S sends skip response |
| T6.4b.T6 | Esc sends cancel response |

#### Acceptance Criteria

- All keyboard shortcuts work
- Custom input field works
- Response sent to queue

#### Technical Notes

```
TUI Human Decision Interface:

┌─────────────────────────────────────────────────────────────────┐
│ ⚠️ Human Decision Request                                [URGENT]│
├─────────────────────────────────────────────────────────────────┤
│                                                                  │
│ Request: dec-001                                                │
│ Agent: alpha [claude] (task-1: Implement auth)                  │
│ Expires: 2026-04-14 14:45:00 (Remaining: 15 minutes)            │
│                                                                  │
│ ─────────────────────────────────────────────────────────────── │
│                                                                  │
│ Situation: waiting_for_choice                                   │
│ Permission: execute                                              │
│ Command: rm -rf ./build                                          │
│                                                                  │
│ ─────────────────────────────────────────────────────────────── │
│                                                                  │
│ Options:                                                         │
│   [A] Approve - Allow this command                              │
│   [B] Approve for session - Allow for entire session            │
│   [C] Deny - Reject this command                                │
│   [D] Abort - Stop current operation                            │
│                                                                  │
│ ─────────────────────────────────────────────────────────────── │
│                                                                  │
│ Recommendation: [C] Deny                                         │
│ Reason: Command contains "rm -rf" which is potentially dangerous│
│ Confidence: 95%                                                  │
│                                                                  │
│ ─────────────────────────────────────────────────────────────── │
│                                                                  │
│ [A] Approve  [B] Session  [C] Deny  [D] Abort  [R] Recommend    │
│                                                                  │
│ Current: C (Enter to confirm, or select other)                  │
├─────────────────────────────────────────────────────────────────┤
│ a/b/c/d select  r recommend  i custom  s skip  Esc cancel       │
└─────────────────────────────────────────────────────────────────┘

Keyboard Shortcuts:
| Key | Action |
|-----|--------|
| A/B/C/D | Select specific option |
| Enter | Confirm current selection |
| R | Accept recommendation |
| I | Input custom instruction |
| S | Skip current task |
| Esc | Cancel operation |
```

---

### Story 6.5: CLI Human Decision Commands

**Priority**: P1
**Effort**: 3 points
**Status**: Backlog

Implement CLI commands for human decision handling in headless mode.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T6.5.1 | Implement `decision list --pending` | Todo | - |
| T6.5.2 | Implement `decision show <id>` | Todo | - |
| T6.5.3 | Implement `decision respond <id> --select` | Todo | - |
| T6.5.4 | Implement `decision respond --accept` | Todo | - |
| T6.5.5 | Implement `decision respond --custom` | Todo | - |
| T6.5.6 | Implement `decision history` | Todo | - |
| T6.5.7 | Write unit tests for CLI | Todo | - |

#### TDD Test Tasks

| Test ID | Definition |
|---------|------------|
| T6.5.T1 | List shows pending by priority |
| T6.5.T2 | Show displays full request |
| T6.5.T3 | Select sends response |
| T6.5.T4 | Accept uses recommendation |
| T6.5.T5 | Custom sends instruction |
| T6.5.T6 | History shows past decisions |

#### Acceptance Criteria

- All decision operations via CLI
- Output format human-readable
- Commands documented

---

## Blocked Agent State (Generic)

Using BlockingReason trait from Sprint 1:

```rust
/// AgentSlotStatus with generic Blocked
pub enum AgentSlotStatus {
    Running,
    Blocked(BlockedState),  // Holds any BlockingReason
    Idle,
    Stopped,
}

/// BlockedState holds BlockingReason trait reference
pub struct BlockedState {
    reason: Box<dyn BlockingReason>,
    blocked_at: Instant,
    context: BlockingContext,
}

/// When human decision required:
/// 1. DecisionSituation.requires_human() returns true
/// 2. Create HumanDecisionBlocking implements BlockingReason
/// 3. Create BlockedState with HumanDecisionBlocking
/// 4. Set AgentSlotStatus::Blocked(blocked_state)
```

**Benefits of Trait Approach**:
- Adding new blocking types (ResourceBlocking, DependencyBlocking) doesn't require modifying AgentSlotStatus
- Each blocking type has its own timeout logic
- BlockingReasonRegistry can manage custom blocking types

---

## Sprint Risks

| Risk | Probability | Impact | Mitigation |
|------|-------------|--------|------------|
| TUI rendering complexity | Medium | Medium | Use existing modal patterns |
| Notification timing | Low | Low | Periodic timeout checks |
| Queue overflow | Low | Medium | Priority ordering, escalation |

## Sprint Deliverables

- `decision/src/human_blocking.rs` - HumanDecisionBlocking
- `decision/src/human_queue.rs` - HumanDecisionQueue
- `decision/src/human_request.rs` - HumanDecisionRequest/Response
- `decision/src/notification.rs` - Notification system
- `tui/src/components/human_decision.rs` - TUI interface
- CLI commands for decision handling

## Dependencies

- Sprint 1: BlockingReason trait, BlockedState
- Sprint 2: DecisionSituation with requires_human()
- Sprint 3: DecisionOutput with action sequences

## Next Sprint

After completing this sprint, proceed to [Sprint 7: Error Recovery](sprint-07-error-recovery.md) for retry logic and recovery strategies.