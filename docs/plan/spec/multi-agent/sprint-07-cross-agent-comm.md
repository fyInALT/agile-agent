# Sprint 7: Cross-Agent Communication

## Metadata

- Sprint ID: `sprint-007`
- Title: `Cross-Agent Communication`
- Duration: 2 weeks
- Priority: P2 (Medium)
- Status: `Backlog`
- Created: 2026-04-13
- Depends On: Sprint 1, Sprint 2, Sprint 4

## Sprint Goal

Implement AgentMail/AgentChat system for async coordination between agents. Agents can request help, share info, and coordinate work.

## Stories

### Story 7.1: AgentMail Structure

**Priority**: P2
**Effort**: 3 points
**Status**: Backlog

Create AgentMail and AgentChat structures for communication.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T7.1.1 | Create `AgentChat` struct for direct messages | Todo | - |
| T7.1.2 | Create `ChatStatus` enum | Todo | - |
| T7.1.3 | Create `AgentMail` struct for async messages | Todo | - |
| T7.1.4 | Create `MailTarget` enum (Direct, Broadcast, ProviderType) | Todo | - |
| T7.1.5 | Create `MailSubject` enum | Todo | - |
| T7.1.6 | Create `MailBody` enum | Todo | - |
| T7.1.7 | Create `CoordinationAction` enum | Todo | - |
| T7.1.8 | Write tests for mail structures | Todo | - |

#### Technical Notes

```rust
pub struct AgentMail {
    mail_id: MailId,
    from: AgentId,
    to: MailTarget,
    subject: MailSubject,
    body: MailBody,
    sent_at: String,
    read_at: Option<String>,
    requires_action: bool,
    deadline: Option<String>,
}

pub enum MailSubject {
    TaskHelpRequest { task_id: TaskId },
    TaskCompleted { task_id: TaskId },
    TaskBlocked { task_id: TaskId, reason: String },
    InfoRequest { query: String },
    InfoResponse { to_mail_id: MailId },
    CoordinationRequest { action: CoordinationAction },
    StatusUpdate { new_status: AgentSlotStatus },
    Custom { label: String },
}
```

---

### Story 7.2: AgentMailbox

**Priority**: P2
**Effort**: 5 points
**Status**: Backlog

Implement mailbox for sending/receiving agent mail.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T7.2.1 | Create `AgentMailbox` struct | Todo | - |
| T7.2.2 | Implement `send_chat()` for direct message | Todo | - |
| T7.2.3 | Implement `send_mail()` for async mail | Todo | - |
| T7.2.4 | Implement `inbox_for()` per agent | Todo | - |
| T7.2.5 | Implement `mark_read()` | Todo | - |
| T7.2.6 | Implement `process_pending()` | Todo | - |
| T7.2.7 | Implement `deliver()` | Todo | - |
| T7.2.8 | Implement `unread_count()` | Todo | - |
| T7.2.9 | Write tests for mailbox operations | Todo | - |

#### Technical Notes

```rust
pub struct AgentMailbox {
    inbox: HashMap<AgentId, Vec<AgentMail>>,
    outgoing: VecDeque<AgentMail>,
    pending_messages: Vec<AgentMail>,
    history: Vec<AgentMail>,
}
```

---

### Story 7.3: Mail Injection into Provider Prompt

**Priority**: P2
**Effort**: 3 points
**Status**: Backlog

Inject unread mail into provider prompt context.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T7.3.1 | Create `build_prompt_with_mail()` function | Todo | - |
| T7.3.2 | Add mail section header to prompt | Todo | - |
| T7.3.3 | Format mail messages for provider | Todo | - |
| T7.3.4 | Mark action-required mail clearly | Todo | - |
| T7.3.5 | Write tests for prompt injection | Todo | - |

---

### Story 7.4: Mail Delivery Event Handling

**Priority**: P2
**Effort**: 3 points
**Status**: Backlog

Process mail delivery events in main event loop.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T7.4.1 | Add mail delivery check in event loop tick | Todo | - |
| T7.4.2 | Deliver pending mail to target agents | Todo | - |
| T7.4.3 | Update inbox on delivery | Todo | - |
| T7.4.4 | Emit notification for new mail | Todo | - |
| T7.4.5 | Write tests for mail delivery | Todo | - |

---

### Story 7.5: TUI Mail Indicator

**Priority**: P2
**Effort**: 2 points
**Status**: Backlog

Show unread mail indicator in TUI status bar.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T7.5.1 | Add unread count to footer | Todo | - |
| T7.5.2 | Add mail notification icon | Todo | - |
| T7.5.3 | Show action-required indicator | Todo | - |
| T7.5.4 | Write tests for mail indicator | Todo | - |

---

## Sprint Risks

| Risk | Probability | Impact | Mitigation |
|------|-------------|--------|------------|
| Mail spam from agents | Medium | Low | Rate limiting, priority filtering |
| Prompt context overflow | Low | Medium | Summarize mail, limit injection count |

## Sprint Deliverables

- `AgentMail` and `AgentChat` structures
- `AgentMailbox` implementation
- Mail prompt injection
- Mail delivery in event loop
- TUI mail indicators

## Dependencies

- Sprint 1: AgentId, AgentPool
- Sprint 2: Provider event handling
- Sprint 4: TUI status bar

## Next Sprint

After completing this sprint, proceed to [Sprint 8: Advanced TUI Modes](./sprint-08-advanced-tui.md).