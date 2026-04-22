//! Cross-Agent Communication via AgentMail
//!
//! Provides async coordination between agents through mail system.
//! Agents can request help, share info, and coordinate work.
//!
//! # Thread Safety
//!
//! AgentMail is designed for single-owner (main thread) operation:
//! - Mail is created on main thread
//! - Mailbox is managed on main thread
//! - Delivery happens during event loop tick
//!
//! ```text
//! ┌─────────────────┐     ┌─────────────────┐
//! │ Agent 1         │     │ Agent 2         │
//! │ (Main Thread)   │     │ (Main Thread)   │
//! │                 │     │                 │
//! │ send_mail()     │────▶│ inbox           │
//! │                 │     │                 │
//! └─────────────────┘     └─────────────────┘
//! ```

use chrono::Utc;
use serde::Deserialize;
use serde::Serialize;
use std::collections::HashMap;
use std::collections::VecDeque;

use crate::agent_runtime::AgentId;
use crate::agent_slot::TaskId;

/// Unique identifier for a mail message
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct MailId(String);

impl MailId {
    /// Create a new mail ID
    pub fn new() -> Self {
        Self(format!("mail-{}", Utc::now().timestamp_millis()))
    }

    /// Create mail ID from string
    #[allow(clippy::should_implement_trait)]
    pub fn from_str(s: impl Into<String>) -> Self {
        Self(s.into())
    }

    /// Get the mail ID as string
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Default for MailId {
    fn default() -> Self {
        Self::new()
    }
}

/// Target for mail delivery
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MailTarget {
    /// Direct message to specific agent
    Direct(AgentId),
    /// Broadcast to all agents
    Broadcast,
    /// Broadcast to agents with specific provider type
    ProviderType(String),
}

/// Subject/category of the mail
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MailSubject {
    /// Request help on a task
    TaskHelpRequest { task_id: TaskId },
    /// Task completed notification
    TaskCompleted { task_id: TaskId },
    /// Task blocked notification
    TaskBlocked { task_id: TaskId, reason: String },
    /// Request information
    InfoRequest { query: String },
    /// Response to information request
    InfoResponse { to_mail_id: MailId },
    /// Coordination request
    CoordinationRequest { action: CoordinationAction },
    /// Status update notification
    StatusUpdate { new_status: String },
    /// Custom subject
    Custom { label: String },
}

impl MailSubject {
    /// Get display label for this subject
    pub fn label(&self) -> String {
        match self {
            Self::TaskHelpRequest { task_id } => format!("Help: {}", task_id.as_str()),
            Self::TaskCompleted { task_id } => format!("Done: {}", task_id.as_str()),
            Self::TaskBlocked { task_id, reason } => {
                format!("Blocked: {} - {}", task_id.as_str(), reason)
            }
            Self::InfoRequest { query } => format!("Info: {}", query),
            Self::InfoResponse { to_mail_id } => format!("Reply to {}", to_mail_id.as_str()),
            Self::CoordinationRequest { action } => format!("Coord: {:?}", action),
            Self::StatusUpdate { new_status } => format!("Status: {}", new_status),
            Self::Custom { label } => label.clone(),
        }
    }
}

/// Coordination action types
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CoordinationAction {
    /// Request agent to pause work
    PauseWork,
    /// Request agent to resume work
    ResumeWork,
    /// Request agent to switch focus
    SwitchFocus { to_task: TaskId },
    /// Request agent to share context
    ShareContext { context_type: String },
    /// Request agent to review output
    ReviewOutput { task_id: TaskId },
    /// Custom coordination action
    Custom { action: String },
}

/// Body/content of the mail
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MailBody {
    /// Text message
    Text(String),
    /// Task context
    TaskContext { summary: String, details: String },
    /// Code snippet
    CodeSnippet { language: String, code: String },
    /// File reference
    FileReference { path: String, description: String },
    /// Custom structured body
    Custom { payload: serde_json::Value },
}

/// Mail message for async agent communication
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentMail {
    /// Unique mail identifier
    pub mail_id: MailId,
    /// Sender agent ID
    pub from: AgentId,
    /// Target recipient(s)
    pub to: MailTarget,
    /// Subject/category
    pub subject: MailSubject,
    /// Body content
    pub body: MailBody,
    /// Timestamp when sent
    pub sent_at: String,
    /// Timestamp when read (if read)
    pub read_at: Option<String>,
    /// Whether this mail requires action from recipient
    pub requires_action: bool,
    /// Optional deadline for action
    pub deadline: Option<String>,
}

impl AgentMail {
    /// Create a new mail message
    pub fn new(from: AgentId, to: MailTarget, subject: MailSubject, body: MailBody) -> Self {
        Self {
            mail_id: MailId::new(),
            from,
            to,
            subject,
            body,
            sent_at: Utc::now().to_rfc3339(),
            read_at: None,
            requires_action: false,
            deadline: None,
        }
    }

    /// Mark this mail as requiring action
    pub fn with_action_required(mut self) -> Self {
        self.requires_action = true;
        self
    }

    /// Set a deadline for this mail
    pub fn with_deadline(mut self, deadline: String) -> Self {
        self.deadline = Some(deadline);
        self
    }

    /// Mark this mail as read
    pub fn mark_read(&mut self) {
        self.read_at = Some(Utc::now().to_rfc3339());
    }

    /// Check if mail is read
    pub fn is_read(&self) -> bool {
        self.read_at.is_some()
    }

    /// Format mail for display in prompt
    pub fn format_for_prompt(&self) -> String {
        let action_marker = if self.requires_action {
            "[ACTION REQUIRED] "
        } else {
            ""
        };
        let deadline_text = self
            .deadline
            .as_ref()
            .map(|d| format!(" (deadline: {})", d))
            .unwrap_or_default();

        let body_text = match &self.body {
            MailBody::Text(text) => text.clone(),
            MailBody::TaskContext { summary, .. } => summary.clone(),
            MailBody::CodeSnippet { code, .. } => format!("Code: {}", code),
            MailBody::FileReference { path, .. } => format!("File: {}", path),
            MailBody::Custom { payload } => payload.to_string(),
        };

        let subject_text = match &self.subject {
            MailSubject::TaskHelpRequest { task_id } => {
                format!("Help requested for {}", task_id.as_str())
            }
            MailSubject::TaskCompleted { task_id } => {
                format!("Task {} completed", task_id.as_str())
            }
            MailSubject::TaskBlocked { task_id, reason } => {
                format!("Task {} blocked: {}", task_id.as_str(), reason)
            }
            MailSubject::InfoRequest { query } => query.clone(),
            MailSubject::InfoResponse { to_mail_id } => {
                format!("Response to {}", to_mail_id.as_str())
            }
            MailSubject::CoordinationRequest { action } => format!("Coordination: {:?}", action),
            MailSubject::StatusUpdate { new_status } => format!("Status: {}", new_status),
            MailSubject::Custom { label } => label.clone(),
        };

        format!(
            "{}From {}: {}{}\n  {}",
            action_marker,
            self.from.as_str(),
            subject_text,
            deadline_text,
            body_text
        )
    }
}

/// Direct chat message (instant, no queuing)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentChat {
    /// Chat message ID
    pub chat_id: String,
    /// Sender agent ID
    pub from: AgentId,
    /// Recipient agent ID
    pub to: AgentId,
    /// Message content
    pub message: String,
    /// Timestamp
    pub sent_at: String,
}

impl AgentChat {
    /// Create a new chat message
    pub fn new(from: AgentId, to: AgentId, message: String) -> Self {
        Self {
            chat_id: format!("chat-{}", Utc::now().timestamp_millis()),
            from,
            to,
            message,
            sent_at: Utc::now().to_rfc3339(),
        }
    }
}

/// Mailbox for managing agent mail
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentMailbox {
    /// Inbox per agent (unread mail)
    inbox: HashMap<AgentId, Vec<AgentMail>>,
    /// Outgoing mail queue
    outgoing: VecDeque<AgentMail>,
    /// Mail pending delivery (being processed)
    pending_delivery: Vec<AgentMail>,
    /// History of all processed mail
    history: Vec<AgentMail>,
}

impl AgentMailbox {
    /// Create a new empty mailbox
    pub fn new() -> Self {
        Self::default()
    }

    /// Send a direct chat message (immediate delivery)
    pub fn send_chat(&mut self, chat: AgentChat) -> Option<AgentMail> {
        // Convert chat to mail for consistency
        let mail = AgentMail::new(
            chat.from.clone(),
            MailTarget::Direct(chat.to.clone()),
            MailSubject::Custom {
                label: "Chat".to_string(),
            },
            MailBody::Text(chat.message),
        );
        self.deliver_to_inbox(mail);
        self.history.last().cloned()
    }

    /// Send async mail (queued delivery)
    pub fn send_mail(&mut self, mail: AgentMail) {
        self.outgoing.push_back(mail);
    }

    /// Get inbox for specific agent
    pub fn inbox_for(&self, agent_id: &AgentId) -> Option<&Vec<AgentMail>> {
        self.inbox.get(agent_id)
    }

    /// Get unread count for agent
    pub fn unread_count(&self, agent_id: &AgentId) -> usize {
        self.inbox
            .get(agent_id)
            .map(|inbox| inbox.iter().filter(|m| !m.is_read()).count())
            .unwrap_or(0)
    }

    /// Get action-required count for agent
    pub fn action_required_count(&self, agent_id: &AgentId) -> usize {
        self.inbox
            .get(agent_id)
            .map(|inbox| {
                inbox
                    .iter()
                    .filter(|m| m.requires_action && !m.is_read())
                    .count()
            })
            .unwrap_or(0)
    }

    /// Get mail requiring action for agent (returns references)
    pub fn action_required(&self, agent_id: &AgentId) -> Vec<&AgentMail> {
        self.inbox
            .get(agent_id)
            .map(|inbox| {
                inbox
                    .iter()
                    .filter(|m| m.requires_action && !m.is_read())
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Mark a mail as read
    pub fn mark_read(&mut self, agent_id: &AgentId, mail_id: &MailId) -> bool {
        if let Some(inbox) = self.inbox.get_mut(agent_id) {
            for mail in inbox.iter_mut() {
                if &mail.mail_id == mail_id {
                    mail.mark_read();
                    return true;
                }
            }
        }
        false
    }

    /// Mark all mail as read for an agent
    pub fn mark_all_read(&mut self, agent_id: &AgentId) -> usize {
        if let Some(inbox) = self.inbox.get_mut(agent_id) {
            let count = inbox.iter().filter(|m| !m.is_read()).count();
            for mail in inbox.iter_mut() {
                mail.mark_read();
            }
            return count;
        }
        0
    }

    /// Process pending delivery queue
    ///
    /// Returns list of agents that received new mail.
    pub fn process_pending(&mut self) -> Vec<AgentId> {
        let mut recipients = Vec::new();

        while let Some(mail) = self.outgoing.pop_front() {
            let recipient = self.deliver_to_inbox(mail);
            if let Some(id) = recipient {
                recipients.push(id);
            }
        }

        recipients
    }

    /// Deliver mail to inbox(es)
    ///
    /// Returns the primary recipient agent ID if direct, or first broadcast recipient.
    fn deliver_to_inbox(&mut self, mail: AgentMail) -> Option<AgentId> {
        let recipient_id = match &mail.to {
            MailTarget::Direct(agent_id) => agent_id.clone(),
            MailTarget::Broadcast | MailTarget::ProviderType(_) => {
                // Broadcast handled externally - return None for now
                self.pending_delivery.push(mail);
                return None;
            }
        };

        self.inbox
            .entry(recipient_id.clone())
            .or_default()
            .push(mail.clone());
        self.history.push(mail);
        Some(recipient_id)
    }

    /// Deliver broadcast mail to all provided agent IDs
    pub fn deliver_broadcast(&mut self, mail: AgentMail, agent_ids: &[AgentId]) {
        for agent_id in agent_ids {
            self.inbox
                .entry(agent_id.clone())
                .or_default()
                .push(mail.clone());
        }
        self.history.push(mail);
    }

    /// Clear inbox for agent (after processing)
    pub fn clear_inbox(&mut self, agent_id: &AgentId) {
        if let Some(inbox) = self.inbox.get_mut(agent_id) {
            inbox.clear();
        }
    }

    /// Get total unread count across all agents
    pub fn total_unread(&self) -> usize {
        self.inbox
            .values()
            .map(|inbox| inbox.iter().filter(|m| !m.is_read()).count())
            .sum()
    }

    /// Get pending delivery count
    pub fn pending_count(&self) -> usize {
        self.pending_delivery.len()
    }

    /// Get outgoing queue count
    pub fn outgoing_count(&self) -> usize {
        self.outgoing.len()
    }

    /// Restore pending mail from snapshot (used on session restore)
    pub fn restore_pending(&mut self, pending_mail: &[AgentMail]) {
        for mail in pending_mail {
            self.pending_delivery.push(mail.clone());
        }
    }

    /// Get all pending mail for snapshot
    pub fn pending_mail_for_snapshot(&self) -> Vec<AgentMail> {
        self.pending_delivery.clone()
    }

    /// Deliver pending mail and generate delivery events
    pub fn deliver(&mut self, agent_ids: &[AgentId]) -> Vec<MailDeliveryEvent> {
        let mut events = Vec::new();
        while let Some(mail) = self.pending_delivery.pop() {
            let delivered_at = Utc::now().to_rfc3339();
            for agent_id in agent_ids {
                self.inbox
                    .entry(agent_id.clone())
                    .or_default()
                    .push(mail.clone());
                events.push(MailDeliveryEvent {
                    mail: mail.clone(),
                    target: agent_id.clone(),
                    delivered_at: delivered_at.clone(),
                });
            }
            self.history.push(mail);
        }
        events
    }

    /// Get mail history
    pub fn history(&self) -> &[AgentMail] {
        &self.history
    }
}

/// Event generated when mail is delivered to an agent
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MailDeliveryEvent {
    /// The delivered mail
    pub mail: AgentMail,
    /// Target agent that received the mail
    pub target: AgentId,
    /// Timestamp when delivered
    pub delivered_at: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_agent_id(name: &str) -> AgentId {
        AgentId::new(name)
    }

    fn make_task_id(name: &str) -> TaskId {
        TaskId::new(name)
    }

    #[test]
    fn mail_id_new_is_unique() {
        let id1 = MailId::new();
        std::thread::sleep(std::time::Duration::from_millis(1));
        let id2 = MailId::new();
        assert_ne!(id1, id2);
    }

    #[test]
    fn mail_id_from_str() {
        let id = MailId::from_str("mail-123");
        assert_eq!(id.as_str(), "mail-123");
    }

    #[test]
    fn mail_target_direct() {
        let target = MailTarget::Direct(make_agent_id("agent_001"));
        assert!(matches!(target, MailTarget::Direct(_)));
    }

    #[test]
    fn mail_target_broadcast() {
        let target = MailTarget::Broadcast;
        assert!(matches!(target, MailTarget::Broadcast));
    }

    #[test]
    fn mail_subject_task_help_request() {
        let subject = MailSubject::TaskHelpRequest {
            task_id: make_task_id("task-1"),
        };
        assert!(matches!(subject, MailSubject::TaskHelpRequest { .. }));
    }

    #[test]
    fn mail_body_text() {
        let body = MailBody::Text("Hello".to_string());
        assert!(matches!(body, MailBody::Text(_)));
    }

    #[test]
    fn agent_mail_new() {
        let mail = AgentMail::new(
            make_agent_id("sender"),
            MailTarget::Direct(make_agent_id("recipient")),
            MailSubject::Custom {
                label: "Test".to_string(),
            },
            MailBody::Text("Message".to_string()),
        );
        assert!(!mail.is_read());
        assert!(!mail.requires_action);
    }

    #[test]
    fn agent_mail_with_action_required() {
        let mail = AgentMail::new(
            make_agent_id("sender"),
            MailTarget::Direct(make_agent_id("recipient")),
            MailSubject::Custom {
                label: "Test".to_string(),
            },
            MailBody::Text("Message".to_string()),
        )
        .with_action_required();
        assert!(mail.requires_action);
    }

    #[test]
    fn agent_mail_mark_read() {
        let mut mail = AgentMail::new(
            make_agent_id("sender"),
            MailTarget::Direct(make_agent_id("recipient")),
            MailSubject::Custom {
                label: "Test".to_string(),
            },
            MailBody::Text("Message".to_string()),
        );
        assert!(!mail.is_read());
        mail.mark_read();
        assert!(mail.is_read());
    }

    #[test]
    fn agent_mail_format_for_prompt() {
        let mail = AgentMail::new(
            make_agent_id("alpha"),
            MailTarget::Direct(make_agent_id("bravo")),
            MailSubject::TaskHelpRequest {
                task_id: make_task_id("task-1"),
            },
            MailBody::Text("Need help with task-1".to_string()),
        )
        .with_action_required();

        let formatted = mail.format_for_prompt();
        assert!(formatted.contains("[ACTION REQUIRED]"));
        assert!(formatted.contains("From alpha"));
        assert!(formatted.contains("Help requested for task-1"));
    }

    #[test]
    fn agent_chat_new() {
        let chat = AgentChat::new(
            make_agent_id("sender"),
            make_agent_id("recipient"),
            "Hello!".to_string(),
        );
        assert_eq!(chat.message, "Hello!");
    }

    #[test]
    fn mailbox_new_is_empty() {
        let mailbox = AgentMailbox::new();
        assert_eq!(mailbox.unread_count(&make_agent_id("agent_001")), 0);
    }

    #[test]
    fn mailbox_send_mail_queues_outgoing() {
        let mut mailbox = AgentMailbox::new();
        let mail = AgentMail::new(
            make_agent_id("sender"),
            MailTarget::Direct(make_agent_id("recipient")),
            MailSubject::Custom {
                label: "Test".to_string(),
            },
            MailBody::Text("Message".to_string()),
        );
        mailbox.send_mail(mail);
        assert_eq!(mailbox.outgoing_count(), 1);
    }

    #[test]
    fn mailbox_process_pending_delivers_direct_mail() {
        let mut mailbox = AgentMailbox::new();
        let recipient = make_agent_id("recipient");
        let mail = AgentMail::new(
            make_agent_id("sender"),
            MailTarget::Direct(recipient.clone()),
            MailSubject::Custom {
                label: "Test".to_string(),
            },
            MailBody::Text("Message".to_string()),
        );
        mailbox.send_mail(mail);

        let recipients = mailbox.process_pending();
        assert_eq!(recipients.len(), 1);
        assert_eq!(recipients[0], recipient);
        assert_eq!(mailbox.unread_count(&recipient), 1);
    }

    #[test]
    fn mailbox_mark_read() {
        let mut mailbox = AgentMailbox::new();
        let recipient = make_agent_id("recipient");
        let mail = AgentMail::new(
            make_agent_id("sender"),
            MailTarget::Direct(recipient.clone()),
            MailSubject::Custom {
                label: "Test".to_string(),
            },
            MailBody::Text("Message".to_string()),
        );
        let mail_id = mail.mail_id.clone();
        mailbox.send_mail(mail);
        mailbox.process_pending();

        assert_eq!(mailbox.unread_count(&recipient), 1);
        mailbox.mark_read(&recipient, &mail_id);
        assert_eq!(mailbox.unread_count(&recipient), 0);
    }

    #[test]
    fn mailbox_send_chat_delivers_immediately() {
        let mut mailbox = AgentMailbox::new();
        let sender = make_agent_id("sender");
        let recipient = make_agent_id("recipient");

        let chat = AgentChat::new(sender.clone(), recipient.clone(), "Hello!".to_string());
        mailbox.send_chat(chat);

        assert_eq!(mailbox.unread_count(&recipient), 1);
        assert_eq!(mailbox.outgoing_count(), 0);
    }

    #[test]
    fn mailbox_action_required_count() {
        let mut mailbox = AgentMailbox::new();
        let recipient = make_agent_id("recipient");

        let mail1 = AgentMail::new(
            make_agent_id("sender"),
            MailTarget::Direct(recipient.clone()),
            MailSubject::Custom {
                label: "Test1".to_string(),
            },
            MailBody::Text("Message1".to_string()),
        )
        .with_action_required();

        let mail2 = AgentMail::new(
            make_agent_id("sender"),
            MailTarget::Direct(recipient.clone()),
            MailSubject::Custom {
                label: "Test2".to_string(),
            },
            MailBody::Text("Message2".to_string()),
        );

        mailbox.send_mail(mail1);
        mailbox.send_mail(mail2);
        mailbox.process_pending();

        assert_eq!(mailbox.action_required_count(&recipient), 1);
        assert_eq!(mailbox.unread_count(&recipient), 2);
    }

    #[test]
    fn mailbox_total_unread() {
        let mut mailbox = AgentMailbox::new();

        let agent1 = make_agent_id("agent1");
        let agent2 = make_agent_id("agent2");

        mailbox.send_mail(AgentMail::new(
            make_agent_id("sender"),
            MailTarget::Direct(agent1.clone()),
            MailSubject::Custom {
                label: "Test".to_string(),
            },
            MailBody::Text("To agent1".to_string()),
        ));

        mailbox.send_mail(AgentMail::new(
            make_agent_id("sender"),
            MailTarget::Direct(agent2.clone()),
            MailSubject::Custom {
                label: "Test".to_string(),
            },
            MailBody::Text("To agent2".to_string()),
        ));

        mailbox.process_pending();

        assert_eq!(mailbox.total_unread(), 2);
    }

    #[test]
    fn mailbox_broadcast_queues_pending() {
        let mut mailbox = AgentMailbox::new();
        let mail = AgentMail::new(
            make_agent_id("sender"),
            MailTarget::Broadcast,
            MailSubject::Custom {
                label: "Broadcast".to_string(),
            },
            MailBody::Text("To all".to_string()),
        );
        mailbox.send_mail(mail);
        mailbox.process_pending();

        assert_eq!(mailbox.pending_count(), 1);
    }

    #[test]
    fn mailbox_deliver_broadcast() {
        let mut mailbox = AgentMailbox::new();

        let agent1 = make_agent_id("agent1");
        let agent2 = make_agent_id("agent2");

        let mail = AgentMail::new(
            make_agent_id("sender"),
            MailTarget::Broadcast,
            MailSubject::Custom {
                label: "Broadcast".to_string(),
            },
            MailBody::Text("To all".to_string()),
        );

        mailbox.deliver_broadcast(mail, &[agent1.clone(), agent2.clone()]);

        assert_eq!(mailbox.unread_count(&agent1), 1);
        assert_eq!(mailbox.unread_count(&agent2), 1);
    }

    #[test]
    fn mailbox_mark_all_read_keeps_mail_history() {
        let mut mailbox = AgentMailbox::new();
        let recipient = make_agent_id("recipient");

        // Send multiple mails
        let mail1 = AgentMail::new(
            make_agent_id("sender1"),
            MailTarget::Direct(recipient.clone()),
            MailSubject::Custom {
                label: "Test1".to_string(),
            },
            MailBody::Text("Message1".to_string()),
        );
        let mail2 = AgentMail::new(
            make_agent_id("sender2"),
            MailTarget::Direct(recipient.clone()),
            MailSubject::Custom {
                label: "Test2".to_string(),
            },
            MailBody::Text("Message2".to_string()),
        );

        mailbox.send_mail(mail1);
        mailbox.send_mail(mail2);
        mailbox.process_pending();

        // Verify unread count is 2
        assert_eq!(mailbox.unread_count(&recipient), 2);

        // Mark all as read
        let marked_count = mailbox.mark_all_read(&recipient);
        assert_eq!(marked_count, 2);

        // Verify unread count is 0 but inbox still has 2 mails
        assert_eq!(mailbox.unread_count(&recipient), 0);
        let inbox = mailbox.inbox_for(&recipient);
        assert!(inbox.is_some());
        assert_eq!(inbox.unwrap().len(), 2); // Mail history preserved

        // All mails should be marked read
        for mail in inbox.unwrap() {
            assert!(mail.is_read());
        }
    }

    #[test]
    fn mailbox_mark_all_read_empty_inbox() {
        let mut mailbox = AgentMailbox::new();
        let agent = make_agent_id("empty_agent");

        let count = mailbox.mark_all_read(&agent);
        assert_eq!(count, 0);
    }

    #[test]
    fn broadcast_process_pending_delivers_to_all() {
        let mut mailbox = AgentMailbox::new();
        let agent1 = make_agent_id("agent1");
        let agent2 = make_agent_id("agent2");

        // Send broadcast mail
        let mail = AgentMail::new(
            make_agent_id("sender"),
            MailTarget::Broadcast,
            MailSubject::Custom {
                label: "Announcement".to_string(),
            },
            MailBody::Text("Hello everyone!".to_string()),
        );
        mailbox.send_mail(mail);

        // Process pending - broadcast mail goes to pending_delivery
        let recipients = mailbox.process_pending();
        assert!(recipients.is_empty()); // Broadcast returns no recipient
        assert_eq!(mailbox.pending_count(), 1); // Pending for broadcast

        // Get pending mail and deliver to agents
        let pending_mail = mailbox.pending_delivery.pop().unwrap();
        mailbox.deliver_broadcast(pending_mail, &[agent1.clone(), agent2.clone()]);

        assert_eq!(mailbox.unread_count(&agent1), 1);
        assert_eq!(mailbox.unread_count(&agent2), 1);
    }

    #[test]
    fn action_required_returns_unread_action_mails() {
        let mut mailbox = AgentMailbox::new();
        let recipient = make_agent_id("recipient");

        // Send normal mail
        let normal_mail = AgentMail::new(
            make_agent_id("sender"),
            MailTarget::Direct(recipient.clone()),
            MailSubject::Custom {
                label: "Normal".to_string(),
            },
            MailBody::Text("Just info".to_string()),
        );
        // Send action-required mail
        let action_mail = AgentMail::new(
            make_agent_id("sender"),
            MailTarget::Direct(recipient.clone()),
            MailSubject::TaskHelpRequest {
                task_id: make_task_id("task-1"),
            },
            MailBody::Text("Need help".to_string()),
        )
        .with_action_required();

        mailbox.send_mail(normal_mail);
        mailbox.send_mail(action_mail);
        mailbox.process_pending();

        // action_required should return only the action-required mail
        let action_mails = mailbox.action_required(&recipient);
        assert_eq!(action_mails.len(), 1);
        assert!(action_mails[0].requires_action);
    }

    #[test]
    fn restore_pending_from_snapshot() {
        let mut mailbox = AgentMailbox::new();
        let agent1 = make_agent_id("agent1");

        // Create pending mail
        let mail1 = AgentMail::new(
            make_agent_id("sender"),
            MailTarget::Broadcast,
            MailSubject::Custom {
                label: "Broadcast1".to_string(),
            },
            MailBody::Text("Message1".to_string()),
        );
        let mail2 = AgentMail::new(
            make_agent_id("sender"),
            MailTarget::Broadcast,
            MailSubject::Custom {
                label: "Broadcast2".to_string(),
            },
            MailBody::Text("Message2".to_string()),
        );

        // Restore pending mail
        mailbox.restore_pending(&[mail1.clone(), mail2.clone()]);
        assert_eq!(mailbox.pending_count(), 2);

        // Deliver pending mail
        let events = mailbox.deliver(std::slice::from_ref(&agent1));
        assert_eq!(events.len(), 2);
        assert_eq!(mailbox.unread_count(&agent1), 2);
    }

    #[test]
    fn deliver_generates_delivery_events() {
        let mut mailbox = AgentMailbox::new();
        let agent1 = make_agent_id("agent1");
        let agent2 = make_agent_id("agent2");

        // Create pending mail
        let mail = AgentMail::new(
            make_agent_id("sender"),
            MailTarget::Broadcast,
            MailSubject::Custom {
                label: "Test".to_string(),
            },
            MailBody::Text("Test message".to_string()),
        );
        mailbox.restore_pending(std::slice::from_ref(&mail));

        // Deliver and get events
        let events = mailbox.deliver(&[agent1.clone(), agent2.clone()]);
        assert_eq!(events.len(), 2); // One event per agent

        // Verify event structure
        assert_eq!(events[0].target, agent1);
        assert_eq!(events[1].target, agent2);
        assert!(!events[0].delivered_at.is_empty());
    }

    #[test]
    fn pending_mail_for_snapshot() {
        let mut mailbox = AgentMailbox::new();

        let mail = AgentMail::new(
            make_agent_id("sender"),
            MailTarget::Broadcast,
            MailSubject::Custom {
                label: "Test".to_string(),
            },
            MailBody::Text("Test".to_string()),
        );
        mailbox.restore_pending(std::slice::from_ref(&mail));

        let snapshot_mails = mailbox.pending_mail_for_snapshot();
        assert_eq!(snapshot_mails.len(), 1);
    }

    #[test]
    fn mailbox_history_tracks_all_mails() {
        let mut mailbox = AgentMailbox::new();
        let recipient = make_agent_id("recipient");

        let mail1 = AgentMail::new(
            make_agent_id("sender"),
            MailTarget::Direct(recipient.clone()),
            MailSubject::Custom {
                label: "Test1".to_string(),
            },
            MailBody::Text("Message1".to_string()),
        );
        mailbox.send_mail(mail1);
        mailbox.process_pending();

        assert_eq!(mailbox.history().len(), 1);
    }
}
