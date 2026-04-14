# Sprint 6: Human Intervention

## Metadata

- Sprint ID: `decision-sprint-006`
- Title: `Human Intervention System`
- Duration: 2 weeks
- Priority: P0 (Critical)
- Status: `Backlog`
- Created: 2026-04-14

## TDD Reference

See [Test Specification](test-specification.md) for detailed TDD test tasks:
- Sprint 6 Tests: T6.1.T1-T6.5.T6 (27 tests)
- Test execution: Write failing tests first, implement minimum, refactor

## Sprint Goal

Implement critical decision escalation to human users. When decisions are important, Decision Agent reports to the human user, Main Agent enters Blocked state, and waits for human decision.

## Stories

### Story 6.1: CriticalDecisionCriteria Evaluation

**Priority**: P0
**Effort**: 3 points
**Status**: Backlog

Implement criteria evaluation for determining if decision needs human escalation.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T6.1.1 | Implement `CriticalDecisionCriteria` evaluation | Todo | - |
| T6.1.2 | Implement `criticality_score()` calculation | Todo | - |
| T6.1.3 | Implement threshold comparison | Todo | - |
| T6.1.4 | Implement `CriticalDecisionReason` generation | Todo | - |
| T6.1.5 | Write unit tests for criteria evaluation | Todo | - |

#### TDD Test Tasks

| Test ID | Definition |
|---------|------------|
| T6.1.T1 | Multi-agent impact detected |
| T6.1.T2 | Irreversible operations detected |
| T6.1.T3 | High risk operations detected |
| T6.1.T4 | Low confidence detected |
| T6.1.T5 | Project rule requires human detected |
| T6.1.T6 | Score weights match specification |

#### Acceptance Criteria

- Criteria correctly identifies critical decisions
- Score calculation matches weights
- Threshold configurable

#### Technical Notes

```rust
impl CriticalDecisionCriteria {
    pub fn evaluate(context: &DecisionContext, decision: &DecisionOutput) -> Self {
        let mut criteria = CriticalDecisionCriteria::default();
        
        // 1. Multi-agent impact: Decision affects other agents
        if decision.affects_other_agents() {
            criteria.multi_agent_impact = true;
        }
        
        // 2. Irreversible: PR merge, file deletion, database migration
        if decision.is_irreversible() {
            criteria.irreversible = true;
        }
        
        // 3. High risk: Production deployment, destructive commands
        if decision.is_high_risk() {
            criteria.high_risk = true;
        }
        
        // 4. Low confidence: Decision engine confidence below threshold
        if context.decision_engine_confidence < context.config.confidence_threshold {
            criteria.low_confidence = true;
        }
        
        // 5. High cost: Expensive API calls, long-running tasks
        if decision.estimated_cost() > context.config.cost_threshold {
            criteria.high_cost = true;
        }
        
        // 6. Project rule requires human: CLAUDE.md has "requires_human" tag
        if context.project_rules.requires_human_for(&decision) {
            criteria.requires_human = true;
        }
        
        criteria
    }
    
    pub fn is_critical(&self) -> bool {
        self.criticality_score() >= 3 // Default threshold
    }
    
    pub fn criticality_score(&self) -> u8 {
        let mut score = 0;
        if self.multi_agent_impact { score += 2; }
        if self.irreversible { score += 3; }
        if self.high_risk { score += 3; }
        if self.low_confidence { score += 1; }
        if self.high_cost { score += 1; }
        if self.requires_human { score += 2; }
        score
    }
    
    pub fn reason(&self) -> CriticalDecisionReason {
        // Return the primary reason (highest weight)
        if self.irreversible {
            return CriticalDecisionReason::IrreversibleOperation {
                operation: "This operation cannot be undone".to_string(),
            };
        }
        if self.high_risk {
            return CriticalDecisionReason::HighRiskOperation {
                risk_description: "This operation has high risk".to_string(),
            };
        }
        if self.multi_agent_impact {
            return CriticalDecisionReason::MultiAgentImpact {
                affected_agents: vec![], // Will be filled
            };
        }
        if self.requires_human {
            return CriticalDecisionReason::ProjectRuleRequiresHuman {
                rule: "Project rule requires human confirmation".to_string(),
            };
        }
        if self.low_confidence {
            return CriticalDecisionReason::LowConfidence {
                confidence: 0.0, // Will be filled
            };
        }
        if self.high_cost {
            return CriticalDecisionReason::HighCost {
                estimated_cost: "High cost operation".to_string(),
            };
        }
        
        CriticalDecisionReason::ProjectRuleRequiresHuman {
            rule: "Unknown reason".to_string(),
        }
    }
}
```

---

### Story 6.2: HumanDecisionQueue Implementation

**Priority**: P0
**Effort**: 4 points
**Status**: Backlog

Implement queue for managing pending human decisions.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T6.2.1 | Create `HumanDecisionQueue` struct | Todo | - |
| T6.2.2 | Implement `push()` for new requests | Todo | - |
| T6.2.3 | Implement `pop()` for next request | Todo | - |
| T6.2.4 | Implement priority ordering | Todo | - |
| T6.2.5 | Implement timeout checking | Todo | - |
| T6.2.6 | Implement history tracking | Todo | - |
| T6.2.7 | Write unit tests for queue operations | Todo | - |

#### TDD Test Tasks

| Test ID | Definition |
|---------|------------|
| T6.2.T1 | Request added to correct priority queue |
| T6.2.T2 | High > Medium > Low priority order |
| T6.2.T3 | Expired requests detected |
| T6.2.T4 | Request removed, response archived |
| T6.2.T5 | History preserved correctly |

#### Acceptance Criteria

- Queue stores pending human decisions
- Priority ordering works correctly
- Timeout handling implemented

#### Technical Notes

```rust
pub struct HumanDecisionQueue {
    /// Pending requests by priority
    high_priority: VecDeque<HumanDecisionRequest>,
    medium_priority: VecDeque<HumanDecisionRequest>,
    low_priority: VecDeque<HumanDecisionRequest>,
    
    /// Completed requests (history)
    history: Vec<HumanDecisionResponse>,
    
    /// Timeout configuration
    timeout_config: HumanDecisionTimeoutConfig,
    
    /// Notification dispatcher
    notification_tx: Sender<HumanDecisionNotification>,
}

pub struct HumanDecisionTimeoutConfig {
    /// Default timeout in milliseconds
    default_timeout_ms: u64,  // Default: 3600000 (1 hour)
    
    /// Urgent timeout in milliseconds
    urgent_timeout_ms: u64,   // Default: 1800000 (30 minutes)
    
    /// Warning before timeout
    warning_before_ms: u64,   // Default: 60000 (1 minute)
    
    /// Default action on timeout
    timeout_default: DefaultAction,
}

pub enum DefaultAction {
    FollowRecommendation,
    SelectDefault,
    Cancel,
    MarkTaskFailed,
}

impl HumanDecisionQueue {
    pub fn push(&mut self, request: HumanDecisionRequest) {
        match request.priority {
            DecisionPriority::High => self.high_priority.push_back(request),
            DecisionPriority::Medium => self.medium_priority.push_back(request),
            DecisionPriority::Low => self.low_priority.push_back(request),
        }
        
        // Send notification
        self.notification_tx.send(HumanDecisionNotification::NewRequest {
            request,
            unread_count: self.total_pending(),
        });
    }
    
    pub fn pop(&mut self) -> Option<HumanDecisionRequest> {
        // Priority order: High > Medium > Low
        if let Some(req) = self.high_priority.pop_front() {
            return Some(req);
        }
        if let Some(req) = self.medium_priority.pop_front() {
            return Some(req);
        }
        self.low_priority.pop_front()
    }
    
    pub fn complete(&mut self, response: HumanDecisionResponse) -> Option<HumanDecisionRequest> {
        // Remove from queue and store in history
        let request = self.find_and_remove(&response.request_id)?;
        self.history.push(response);
        
        Some(request)
    }
    
    pub fn check_timeout(&mut self) -> Vec<ExpiredDecision> {
        let now = Utc::now();
        let mut expired = Vec::new();
        
        for queue in [&self.high_priority, &self.medium_priority, &self.low_priority] {
            for req in queue {
                if now > req.expires_at {
                    expired.push(ExpiredDecision {
                        request_id: req.id.clone(),
                        default_action: self.timeout_config.timeout_default.clone(),
                    });
                }
            }
        }
        
        expired
    }
    
    pub fn total_pending(&self) -> usize {
        self.high_priority.len() + self.medium_priority.len() + self.low_priority.len()
    }
}
```

---

### Story 6.3: Human Decision Notification System

**Priority**: P0
**Effort**: 3 points
**Status**: Backlog

Implement notification system for human decision requests.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T6.3.1 | Create `HumanDecisionNotification` enum | Todo | - |
| T6.3.2 | Implement TUI notification dispatch | Todo | - |
| T6.3.3 | Implement CLI notification dispatch | Todo | - |
| T6.3.4 | Implement timeout warning notification | Todo | - |
| T6.3.5 | Implement urgent notification | Todo | - |
| T6.3.6 | Write unit tests for notifications | Todo | - |

#### TDD Test Tasks

| Test ID | Definition |
|---------|------------|
| T6.3.T1 | NewRequest notification sent |
| T6.3.T2 | ApproachingTimeout notification sent |
| T6.3.T3 | TimeoutExpired notification sent |
| T6.3.T4 | UrgentRequest notification sent |

#### Acceptance Criteria

- Notifications sent to appropriate channels
- Timeout warnings work correctly
- Urgent notifications distinguished

#### Technical Notes

```rust
pub enum HumanDecisionNotification {
    /// New decision request
    NewRequest {
        request: HumanDecisionRequest,
        unread_count: usize,
    },
    
    /// Decision request approaching timeout
    ApproachingTimeout {
        request_id: DecisionRequestId,
        remaining_seconds: u64,
    },
    
    /// Decision request expired
    TimeoutExpired {
        request_id: DecisionRequestId,
        default_action: DefaultAction,
    },
    
    /// Urgent decision request
    UrgentRequest {
        request: HumanDecisionRequest,
    },
}

pub struct NotificationDispatcher {
    /// TUI channel
    tui_tx: Option<Sender<TuiNotification>>,
    
    /// Log channel (for headless)
    log_tx: Sender<LogNotification>,
    
    /// External webhook (optional)
    webhook_url: Option<String>,
}

impl NotificationDispatcher {
    pub fn notify(&self, notification: HumanDecisionNotification) {
        // 1. TUI notification (if TUI active)
        if let Some(tx) = &self.tui_tx {
            tx.send(TuiNotification::HumanDecision(notification.clone()));
        }
        
        // 2. Log notification (always)
        self.log_tx.send(LogNotification::HumanDecision(notification.clone()));
        
        // 3. External webhook (if configured)
        if let Some(url) = &self.webhook_url {
            self.send_webhook(url, &notification);
        }
    }
    
    fn send_webhook(&self, url: &str, notification: &HumanDecisionNotification) {
        // POST to webhook URL
        // ...
    }
}
```

---

### Story 6.4a: TUI Decision Modal Display

**Priority**: P0
**Effort**: 2 points
**Status**: Backlog

Implement TUI modal component to display decision request and analysis.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T6.4a.1 | Create human decision modal component | Todo | - |
| T6.4a.2 | Display decision request details (source, time, type) | Todo | - |
| T6.4a.3 | Display Decision Agent analysis (options, scores) | Todo | - |
| T6.4a.4 | Display recommendation and confidence | Todo | - |
| T6.4a.5 | Implement urgency indicator (urgent badge) | Todo | - |
| T6.4a.6 | Write unit tests for display | Todo | - |

#### TDD Test Tasks

| Test ID | Definition |
|---------|------------|
| T6.4.T1 | Request details displayed correctly |
| T6.4.T2 | Decision Agent analysis shown |

#### Acceptance Criteria

- Modal displays all request information
- Decision Agent analysis rendered correctly
- Urgent requests have visual indicator

---

### Story 6.4b: TUI Decision Modal Interaction

**Priority**: P0
**Effort**: 3 points
**Status**: Backlog

Implement keyboard interaction for decision modal.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T6.4b.1 | Implement option selection keyboard shortcuts (A/B/C/D) | Todo | - |
| T6.4b.2 | Implement Enter to confirm selection | Todo | - |
| T6.4b.3 | Implement recommendation acceptance (R key) | Todo | - |
| T6.4b.4 | Implement custom instruction input (C key) | Todo | - |
| T6.4b.5 | Implement skip/cancel operations (S/Esc) | Todo | - |
| T6.4b.6 | Implement help display (? key) | Todo | - |
| T6.4b.7 | Write unit tests for interaction | Todo | - |

#### TDD Test Tasks

| Test ID | Definition |
|---------|------------|
| T6.4.T3 | Options selectable via keyboard |
| T6.4.T4 | Recommendation accepted with R key |
| T6.4.T5 | Custom instruction input works |
| T6.4.T6 | Skip and cancel work correctly |

#### Acceptance Criteria

- All keyboard shortcuts work correctly
- Custom instruction input field works
- Skip/cancel operations trigger correct responses

#### Technical Notes

```
TUI Human Decision Interface:

┌─────────────────────────────────────────────────────────────────┐
│ ⚠️ Human Decision Request                                    [urgent]│
├─────────────────────────────────────────────────────────────────┤
│                                                                  │
│ Source: alpha [claude] (task-1: Implement auth system)          │
│ Time: 2026-04-14 14:30:00  (Remaining: 5 minutes)               │
│                                                                  │
│ Decision Type: Architecture Selection                            │
│ Reason: Irreversible operation                                  │
│                                                                  │
│ ─────────────────────────────────────────────────────────────── │
│                                                                  │
│ Decision Agent Analysis:                                         │
│                                                                  │
│ Option [A] Use existing OAuth library                           │
│   Pros: Fast implementation, good community support             │
│   Cons: May not fully meet custom needs                         │
│   Risks: Library version compatibility                          │
│   Score: ★★★★☆ (Recommended)                                    │
│                                                                  │
│ Option [B] Implement custom auth system                         │
│   Pros: Fully customizable, learning opportunity                │
│   Cons: Longer dev time, higher maintenance                    │
│   Risks: Security vulnerability risk                            │
│   Score: ★★☆☆☆                                                 │
│                                                                  │
│ ─────────────────────────────────────────────────────────────── │
│                                                                  │
│ Decision Agent Recommendation: Select [A]                       │
│ Reason: Matches project rule "don't reinvent the wheel"        │
│ Confidence: 75%                                                  │
│                                                                  │
│ ─────────────────────────────────────────────────────────────── │
│                                                                  │
│ [A] Use OAuth  [B] Custom impl  [C] Custom cmd  [S] Skip       │
│                                                                  │
│ Current: A (Enter to confirm, or select other)                  │
├─────────────────────────────────────────────────────────────────┤
│ a accept  r recommend  c custom  s skip  Esc cancel             │
└─────────────────────────────────────────────────────────────────┘
```

**Keyboard Shortcuts**:

| Key | Action |
|-----|--------|
| `A/B/C/D` | Select specific option |
| `Enter` | Confirm current selection |
| `R` | Accept Decision Agent recommendation |
| `C` | Input custom instruction |
| `S` | Skip current task |
| `Esc` | Cancel operation |
| `?` | Show more analysis details |

---

### Story 6.5: CLI Human Decision Commands

**Priority**: P1
**Effort**: 3 points
**Status**: Backlog

Implement CLI commands for human decision handling in headless mode.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T6.5.1 | Implement `decision list --pending` command | Todo | - |
| T6.5.2 | Implement `decision show <id>` command | Todo | - |
| T6.5.3 | Implement `decision respond <id> --select <opt>` | Todo | - |
| T6.5.4 | Implement `decision respond <id> --accept-recommendation` | Todo | - |
| T6.5.5 | Implement `decision respond <id> --custom <text>` | Todo | - |
| T6.5.6 | Implement `decision history` command | Todo | - |
| T6.5.7 | Write unit tests for CLI commands | Todo | - |

#### TDD Test Tasks

| Test ID | Definition |
|---------|------------|
| T6.5.T1 | Pending requests listed correctly |
| T6.5.T2 | Request details displayed |
| T6.5.T3 | Select option works |
| T6.5.T4 | Accept recommendation works |
| T6.5.T5 | Custom instruction works |
| T6.5.T6 | Skip task works |

#### Acceptance Criteria

- All decision operations via CLI
- Output formats human-readable
- Commands documented

#### Technical Notes

```bash
# List pending decisions
agile-agent decision list --pending

# Output:
# ID       | Agent   | Task     | Type        | Urgency | Expires
# dec-001  | alpha   | task-1   | architecture| high    | 5 min
# dec-002  | bravo   | task-2   | deployment  | medium  | 10 min

# Show decision details
agile-agent decision show dec-001

# Output:
# ┌─────────────────────────────────────────
# │ Decision Request: dec-001
# │ Agent: alpha [claude]
# │ Task: task-1 (Implement auth system)
# │ Type: Architecture Selection
# │ Reason: Irreversible operation
# │ Urgency: high
# │ Expires: 2026-04-14 14:35:00
# │
# │ Options:
# │   [A] Use existing OAuth library (Recommended)
# │   [B] Implement custom auth system
# │
# │ Decision Agent Recommendation: [A]
# │ Reason: Matches project rules
# │ Confidence: 75%
# └─────────────────────────────────────────

# Respond with selection
agile-agent decision respond dec-001 --select A

# Accept recommendation
agile-agent decision respond dec-001 --accept-recommendation

# Custom instruction
agile-agent decision respond dec-001 --custom "Please research other options first"

# Skip task
agile-agent decision respond dec-001 --skip

# Cancel operation
agile-agent decision respond dec-001 --cancel

# View history
agile-agent decision history --agent alpha

# Configure timeout behavior
agile-agent decision config --timeout-default follow-recommendation
```

---

## Blocked Agent State

When a decision requires human intervention, the Main Agent enters `BlockedForHumanDecision` state:

```rust
pub enum AgentSlotStatus {
    // Existing states...
    
    /// Blocked for human decision
    BlockedForHumanDecision {
        /// Decision request ID
        decision_request_id: DecisionRequestId,
        
        /// Blocking reason
        reason: CriticalDecisionReason,
        
        /// Blocked start time
        blocked_at: Instant,
        
        /// Decision Agent analysis summary
        preliminary_analysis: String,
        
        /// Available options
        options: Vec<ChoiceOption>,
        
        /// Decision Agent recommendation
        recommended: Option<String>,
    },
}
```

**Blocked State Behavior**:

| Behavior | In BlockedForHumanDecision |
|----------|---------------------------|
| Provider input | Paused, no input sent |
| Event processing | Continue caching but no new decisions |
| Transcript | Continue recording wait state |
| Other agents | Unaffected, continue working |
| Human decision | Must respond before timeout |

---

## Sprint Risks

| Risk | Probability | Impact | Mitigation |
|------|-------------|--------|------------|
| TUI rendering complexity | Medium | Medium | Use existing modal patterns |
| Notification timing | Low | Low | Periodic timeout checks |
| Queue overflow | Low | Medium | Priority ordering, escalation |

## Sprint Deliverables

- `decision/src/criteria.rs` - CriticalDecisionCriteria
- `decision/src/human_queue.rs` - HumanDecisionQueue
- `decision/src/human_request.rs` - Request/Response types
- `decision/src/notification.rs` - Notification system
- `tui/src/components/human_decision.rs` - TUI interface
- CLI commands for decision handling

## Dependencies

- Sprint 1: Core Types (CriticalDecisionCriteria, HumanDecisionRequest)
- Sprint 3: Decision Engine (produces recommendations)
- Sprint 5: Lifecycle (blocked state handling)

## Next Sprint

After completing this sprint, proceed to [Sprint 7: Error Recovery](./sprint-07-error-recovery.md) for retry logic and recovery strategies.