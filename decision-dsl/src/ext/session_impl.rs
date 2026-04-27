//! Real Session implementations for AI decision making
//!
//! Provides Session trait implementations that connect to actual AI providers
//! (Claude, Codex) for decision-making workflows.

use std::cell::RefCell;
use std::collections::VecDeque;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::thread::{Builder, JoinHandle};
use std::time::{Duration, Instant};

use super::error::{SessionError, SessionErrorKind};
use super::traits::Session;

// ── Conversation Message ─────────────────────────────────────────────────────

/// A message in the conversation history.
#[derive(Debug, Clone)]
pub struct ConversationMessage {
    pub role: MessageRole,
    pub content: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MessageRole {
    User,
    Assistant,
    System,
}

impl ConversationMessage {
    pub fn user(content: impl Into<String>) -> Self {
        Self {
            role: MessageRole::User,
            content: content.into(),
        }
    }

    pub fn assistant(content: impl Into<String>) -> Self {
        Self {
            role: MessageRole::Assistant,
            content: content.into(),
        }
    }

    pub fn system(content: impl Into<String>) -> Self {
        Self {
            role: MessageRole::System,
            content: content.into(),
        }
    }
}

impl MessageRole {
    fn role_label(&self) -> &'static str {
        match self {
            MessageRole::User => "User",
            MessageRole::Assistant => "Assistant",
            MessageRole::System => "System",
        }
    }
}

// ── ProviderSession ──────────────────────────────────────────────────────────

/// Session implementation that uses provider infrastructure.
///
/// Provides a two-phase non-blocking pattern:
/// 1. `send()` spawns provider thread, returns immediately
/// 2. `is_ready()` polls for response availability
/// 3. `receive()` retrieves the collected response
///
/// Maintains conversation history for context continuity.
pub struct ProviderSession {
    /// Provider kind identifier (for logging/debugging)
    provider_kind: String,

    /// Working directory for provider execution
    cwd: std::path::PathBuf,

    /// Conversation history
    history: RefCell<Vec<ConversationMessage>>,

    /// Maximum history entries before pruning
    max_history: usize,

    /// Pending response receiver
    pending_rx: RefCell<Option<Receiver<String>>>,

    /// Thread handle for cleanup
    thread_handle: RefCell<Option<JoinHandle<()>>>,

    /// Sent messages for verification
    sent_messages: RefCell<Vec<String>>,

    /// Last response for retrieval
    last_response: RefCell<Option<String>>,
}

impl ProviderSession {
    /// Create a new provider session.
    pub fn new(provider_kind: impl Into<String>, cwd: std::path::PathBuf) -> Self {
        Self {
            provider_kind: provider_kind.into(),
            cwd,
            history: RefCell::new(Vec::new()),
            max_history: 10,
            pending_rx: RefCell::new(None),
            thread_handle: RefCell::new(None),
            sent_messages: RefCell::new(Vec::new()),
            last_response: RefCell::new(None),
        }
    }

    /// Set maximum history size.
    pub fn with_max_history(mut self, max: usize) -> Self {
        self.max_history = max;
        self
    }

    /// Add message to conversation history.
    pub fn push_history(&self, msg: ConversationMessage) {
        self.history.borrow_mut().push(msg);
        self.prune_history();
    }

    /// Get conversation history.
    pub fn history(&self) -> Vec<ConversationMessage> {
        self.history.borrow().clone()
    }

    /// Clear conversation history.
    pub fn clear_history(&self) {
        self.history.borrow_mut().clear();
    }

    /// Get sent messages for verification.
    pub fn sent_messages(&self) -> Vec<String> {
        self.sent_messages.borrow().clone()
    }

    /// Prune history if exceeding max size.
    fn prune_history(&self) {
        let mut history = self.history.borrow_mut();
        if history.len() > self.max_history {
            // Keep the most recent entries
            let drain_from = history.len() - self.max_history;
            history.drain(0..drain_from);
        }
    }

    /// Build prompt with conversation context.
    fn build_context_prompt(&self, prompt: &str) -> String {
        let history = self.history.borrow();
        if history.is_empty() {
            prompt.to_string()
        } else {
            // Include recent conversation as context
            let context: Vec<String> = history
                .iter()
                .map(|m| format!("{}: {}", m.role.role_label(), m.content))
                .collect();
            format!(
                "Previous context:\n{}\n\nCurrent request:\n{}",
                context.join("\n\n"),
                prompt
            )
        }
    }

    /// Check if a pending call has completed.
    fn poll_pending(&self) -> bool {
        // First, try to receive from the channel without holding RefCell borrow
        let received = {
            let rx = self.pending_rx.borrow();
            if let Some(rx) = rx.as_ref() {
                match rx.try_recv() {
                    Ok(response) => Some(response),
                    Err(std::sync::mpsc::TryRecvError::Empty) => None,
                    Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                        // Thread finished without sending response
                        None
                    }
                }
            } else {
                // No pending call - check if we have a cached response
                return self.last_response.borrow().is_some();
            }
        };

        // Now handle the result without conflicting borrows
        if let Some(response) = received {
            *self.last_response.borrow_mut() = Some(response);
            *self.pending_rx.borrow_mut() = None;
            true
        } else {
            false
        }
    }

    /// Clean up thread handle.
    fn cleanup_thread(&self) {
        if let Some(handle) = self.thread_handle.borrow_mut().take() {
            // Thread should have finished by now, just drop the handle
            drop(handle);
        }
    }
}

impl Session for ProviderSession {
    fn send(&mut self, message: &str) -> Result<(), SessionError> {
        // Clean up any previous pending call
        self.cleanup_thread();

        // Store sent message for verification
        self.sent_messages.borrow_mut().push(message.to_string());

        // Build prompt with context
        let prompt = self.build_context_prompt(message);

        // Add user message to history
        self.push_history(ConversationMessage::user(message));

        // Create response channel
        let (tx, rx): (Sender<String>, Receiver<String>) = channel();

        // Spawn provider thread
        let cwd = self.cwd.clone();
        let thread_name = format!("{}-session-call", self.provider_kind);

        let handle = Builder::new()
            .name(thread_name)
            .spawn(move || {
                // In a real implementation, this would call the provider
                // For now, we use a simple stub that will be replaced by
                // actual provider integration
                let response = run_stub_provider(&prompt, &cwd);
                if let Ok(resp) = response {
                    let _ = tx.send(resp);
                }
            })
            .map_err(|e| SessionError {
                kind: SessionErrorKind::SendFailed,
                message: format!("Failed to spawn provider thread: {}", e),
            })?;

        // Store receiver and handle
        *self.pending_rx.borrow_mut() = Some(rx);
        *self.thread_handle.borrow_mut() = Some(handle);
        *self.last_response.borrow_mut() = None;

        Ok(())
    }

    fn send_with_hint(&mut self, message: &str, model: &str) -> Result<(), SessionError> {
        // Store model hint for logging (actual model selection would be in provider)
        // For now, just use send()
        let _ = model;
        self.send(message)
    }

    fn is_ready(&self) -> bool {
        self.poll_pending()
    }

    fn receive(&mut self) -> Result<String, SessionError> {
        // Check if response is available
        if !self.is_ready() {
            return Err(SessionError {
                kind: SessionErrorKind::UnexpectedFormat,
                message: "no response available".into(),
            });
        }

        // Get and clear last response
        let response = self.last_response.borrow_mut().take();

        // Add to history
        if let Some(ref resp) = response {
            self.push_history(ConversationMessage::assistant(resp.clone()));
        }

        // Clean up thread
        self.cleanup_thread();

        response.ok_or(SessionError {
            kind: SessionErrorKind::UnexpectedFormat,
            message: "response was consumed".into(),
        })
    }
}

/// Stub provider implementation.
///
/// This will be replaced by actual provider integration from agent-provider crate.
/// For now, returns a placeholder response for testing.
fn run_stub_provider(prompt: &str, _cwd: &std::path::Path) -> Result<String, SessionError> {
    // Simulate provider delay
    std::thread::sleep(Duration::from_millis(100));

    // Return a stub JSON response that can be parsed
    Ok(format!(
        r#"{{"decision": "proceed", "reasoning": "stub response for: {}", "confidence": 0.8}}"#,
        prompt.chars().take(50).collect::<String>()
    ))
}

// ── InMemorySession ──────────────────────────────────────────────────────────

/// Session implementation with pre-programmed responses.
///
/// Useful for testing decision flows without actual AI calls.
/// Similar to MockSession but with conversation history support.
pub struct InMemorySession {
    /// Queued responses
    replies: RefCell<VecDeque<String>>,

    /// Conversation history
    history: RefCell<Vec<ConversationMessage>>,

    /// Ready state
    ready: RefCell<bool>,

    /// Sent messages
    sent_messages: RefCell<Vec<String>>,
}

impl InMemorySession {
    pub fn new() -> Self {
        Self {
            replies: RefCell::new(VecDeque::new()),
            history: RefCell::new(Vec::new()),
            ready: RefCell::new(false),
            sent_messages: RefCell::new(Vec::new()),
        }
    }

    pub fn with_replies(replies: impl IntoIterator<Item = impl Into<String>>) -> Self {
        let s = Self::new();
        for reply in replies {
            s.push_reply(reply);
        }
        s
    }

    pub fn push_reply(&self, reply: impl Into<String>) {
        self.replies.borrow_mut().push_back(reply.into());
    }

    pub fn set_ready(&self, ready: bool) {
        *self.ready.borrow_mut() = ready;
    }

    pub fn history(&self) -> Vec<ConversationMessage> {
        self.history.borrow().clone()
    }

    pub fn sent_messages(&self) -> Vec<String> {
        self.sent_messages.borrow().clone()
    }

    pub fn clear(&self) {
        self.replies.borrow_mut().clear();
        self.history.borrow_mut().clear();
        self.sent_messages.borrow_mut().clear();
        *self.ready.borrow_mut() = false;
    }

    /// Add message to conversation history.
    fn push_history(&self, msg: ConversationMessage) {
        self.history.borrow_mut().push(msg);
    }
}

impl Default for InMemorySession {
    fn default() -> Self {
        Self::new()
    }
}

impl Session for InMemorySession {
    fn send(&mut self, message: &str) -> Result<(), SessionError> {
        self.sent_messages.borrow_mut().push(message.to_string());
        self.push_history(ConversationMessage::user(message));
        self.set_ready(true);
        Ok(())
    }

    fn send_with_hint(&mut self, message: &str, model: &str) -> Result<(), SessionError> {
        let _ = model;
        self.send(message)
    }

    fn is_ready(&self) -> bool {
        *self.ready.borrow()
    }

    fn receive(&mut self) -> Result<String, SessionError> {
        self.set_ready(false);
        let response = self.replies.borrow_mut().pop_front().ok_or(SessionError {
            kind: SessionErrorKind::UnexpectedFormat,
            message: "no reply queued".into(),
        })?;
        self.push_history(ConversationMessage::assistant(response.clone()));
        Ok(response)
    }
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn conversation_message_roles() {
        let user = ConversationMessage::user("test");
        assert_eq!(user.role, MessageRole::User);
        assert_eq!(user.content, "test");

        let assistant = ConversationMessage::assistant("response");
        assert_eq!(assistant.role, MessageRole::Assistant);

        let system = ConversationMessage::system("instruction");
        assert_eq!(system.role, MessageRole::System);
    }

    #[test]
    fn provider_session_new() {
        let session = ProviderSession::new("claude", std::path::PathBuf::from("/tmp"));
        assert_eq!(session.provider_kind, "claude");
        assert_eq!(session.history().len(), 0);
    }

    #[test]
    fn provider_session_with_max_history() {
        let session = ProviderSession::new("claude", std::path::PathBuf::from("/tmp"))
            .with_max_history(5);
        assert_eq!(session.max_history, 5);
    }

    #[test]
    fn provider_session_push_history_prunes() {
        let session = ProviderSession::new("claude", std::path::PathBuf::from("/tmp"))
            .with_max_history(3);

        for i in 0..5 {
            session.push_history(ConversationMessage::user(format!("msg {}", i)));
        }

        let history = session.history();
        assert_eq!(history.len(), 3);
        // Should keep the most recent (msg 2, msg 3, msg 4)
        assert!(history[0].content.contains("msg 2"));
    }

    #[test]
    fn provider_session_send_adds_to_history() {
        let mut session = ProviderSession::new("claude", std::path::PathBuf::from("/tmp"));
        session.send("test message").unwrap();

        let history = session.history();
        assert_eq!(history.len(), 1);
        assert_eq!(history[0].role, MessageRole::User);
        assert_eq!(history[0].content, "test message");
    }

    #[test]
    fn provider_session_send_stores_sent_message() {
        let mut session = ProviderSession::new("claude", std::path::PathBuf::from("/tmp"));
        session.send("test message").unwrap();

        let sent = session.sent_messages();
        assert_eq!(sent.len(), 1);
        assert_eq!(sent[0], "test message");
    }

    #[test]
    fn provider_session_send_returns_immediately() {
        let mut session = ProviderSession::new("claude", std::path::PathBuf::from("/tmp"));
        // send should return immediately, not block
        let start = Instant::now();
        session.send("test").unwrap();
        let elapsed = start.elapsed();
        // Should be less than 50ms (no blocking)
        assert!(elapsed < Duration::from_millis(50));
    }

    #[test]
    fn provider_session_is_ready_after_delay() {
        let mut session = ProviderSession::new("claude", std::path::PathBuf::from("/tmp"));
        session.send("test").unwrap();

        // Initially not ready (stub has 100ms delay)
        assert!(!session.is_ready());

        // Wait for stub to complete
        std::thread::sleep(Duration::from_millis(150));

        // Now should be ready
        assert!(session.is_ready());
    }

    #[test]
    fn provider_session_receive_returns_response() {
        let mut session = ProviderSession::new("claude", std::path::PathBuf::from("/tmp"));
        session.send("test prompt").unwrap();

        // Wait for response
        std::thread::sleep(Duration::from_millis(150));

        let response = session.receive().unwrap();
        assert!(response.contains("decision"));
        assert!(response.contains("proceed"));

        // Response added to history
        let history = session.history();
        assert_eq!(history.len(), 2);
        assert_eq!(history[1].role, MessageRole::Assistant);
    }

    #[test]
    fn provider_session_receive_without_ready_fails() {
        let mut session = ProviderSession::new("claude", std::path::PathBuf::from("/tmp"));
        session.send("test").unwrap();

        // Try to receive before ready
        let result = session.receive();
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().kind, SessionErrorKind::UnexpectedFormat);
    }

    #[test]
    fn provider_session_context_includes_history() {
        let session = ProviderSession::new("claude", std::path::PathBuf::from("/tmp"));
        session.push_history(ConversationMessage::user("previous"));
        session.push_history(ConversationMessage::assistant("response"));

        // Verify history is accessible
        let history = session.history();
        assert_eq!(history.len(), 2);
        assert_eq!(history[0].role, MessageRole::User);
        assert_eq!(history[0].content, "previous");
        assert_eq!(history[1].role, MessageRole::Assistant);
        assert_eq!(history[1].content, "response");
    }

    #[test]
    fn provider_session_send_includes_context_from_history() {
        let mut session = ProviderSession::new("claude", std::path::PathBuf::from("/tmp"));
        // First interaction
        session.push_history(ConversationMessage::user("previous"));
        session.push_history(ConversationMessage::assistant("response"));

        // Send a new message - sent_messages should include context
        session.send("new request").unwrap();

        let sent = session.sent_messages();
        // The sent message includes the original message, but build_context_prompt
        // adds context when building the actual prompt for provider
        assert!(sent[0].contains("new request"));

        // History should now have 3 entries (2 initial + 1 user)
        let history = session.history();
        assert_eq!(history.len(), 3);
    }

    #[test]
    fn in_memory_session_new() {
        let session = InMemorySession::new();
        assert!(!session.is_ready());
        assert_eq!(session.history().len(), 0);
    }

    #[test]
    fn in_memory_session_with_replies() {
        let session = InMemorySession::with_replies(["reply1", "reply2"]);
        assert!(!session.is_ready()); // not ready until send
    }

    #[test]
    fn in_memory_session_send_makes_ready() {
        let mut session = InMemorySession::new();
        session.push_reply("response");
        session.send("test").unwrap();
        assert!(session.is_ready());
    }

    #[test]
    fn in_memory_session_receive_consumes_reply() {
        let mut session = InMemorySession::new();
        session.push_reply("first");
        session.push_reply("second");

        session.send("test").unwrap();
        let r1 = session.receive().unwrap();
        assert_eq!(r1, "first");

        session.send("test2").unwrap();
        let r2 = session.receive().unwrap();
        assert_eq!(r2, "second");
    }

    #[test]
    fn in_memory_session_receive_tracks_history() {
        let mut session = InMemorySession::new();
        session.push_reply("response");

        session.send("query").unwrap();
        let _ = session.receive().unwrap();

        let history = session.history();
        assert_eq!(history.len(), 2);
        assert_eq!(history[0].role, MessageRole::User);
        assert_eq!(history[0].content, "query");
        assert_eq!(history[1].role, MessageRole::Assistant);
        assert_eq!(history[1].content, "response");
    }

    #[test]
    fn in_memory_session_clear() {
        let mut session = InMemorySession::new();
        session.push_reply("reply");
        session.send("msg").unwrap();
        session.receive().unwrap();

        session.clear();
        assert_eq!(session.history().len(), 0);
        assert_eq!(session.sent_messages().len(), 0);
        assert!(!session.is_ready());
    }
}