#![allow(dead_code)]

//! Scenario-driven Mock LLM for integration testing.
//!
//! `MockLlm` implements the `Session` trait and allows presetting responses
//! based on prompt matchers. It supports delay simulation, multi-turn
//! sequences, and dynamic response generation.

use std::cell::RefCell;
use std::collections::VecDeque;

use decision_dsl::ext::error::{SessionError, SessionErrorKind};
use decision_dsl::ext::traits::Session;

use super::presets::Preset;

// ── PromptMatcher ───────────────────────────────────────────────────────────

/// Matches an incoming prompt against a condition.
#[derive(Debug, Clone)]
pub enum PromptMatcher {
    /// Matches any prompt.
    Any,
    /// Matches if prompt contains the given substring.
    Contains(String),
    /// Matches if prompt starts with the given prefix.
    StartsWith(String),
    /// Exact match.
    Exact(String),
    /// Matches if prompt matches a regex pattern.
    Regex(String),
}

impl PromptMatcher {
    /// Check whether the given prompt matches this matcher.
    pub fn matches(&self, prompt: &str) -> bool {
        match self {
            PromptMatcher::Any => true,
            PromptMatcher::Contains(sub) => prompt.contains(sub),
            PromptMatcher::StartsWith(pre) => prompt.starts_with(pre),
            PromptMatcher::Exact(ex) => prompt == ex,
            PromptMatcher::Regex(pat) => {
                regex::Regex::new(pat)
                    .map(|re| re.is_match(prompt))
                    .unwrap_or(false)
            }
        }
    }
}

// ── ResponseStrategy ────────────────────────────────────────────────────────

/// Defines how the Mock LLM responds to a matched prompt.
#[derive(Debug, Clone)]
pub enum ResponseStrategy {
    /// Respond immediately with a raw string.
    Immediate(String),
    /// Respond after N ticks with the given string.
    AfterTicks(usize, String),
    /// Return the next item from a sequence on each tick.
    Sequence(Vec<String>),
    /// Use a preset response pattern.
    Preset(Preset),
    /// Dynamic generation based on the full conversation history.
    Dynamic(fn(&[String]) -> String),
}

impl ResponseStrategy {
    /// Get the initial response text (for immediate strategies).
    fn initial_text(&self) -> Option<String> {
        match self {
            ResponseStrategy::Immediate(text) => Some(text.clone()),
            ResponseStrategy::Preset(p) => Some(p.render()),
            ResponseStrategy::Dynamic(_) => None, // requires history
            ResponseStrategy::AfterTicks(_, text) => Some(text.clone()),
            ResponseStrategy::Sequence(seq) => seq.first().cloned(),
        }
    }
}

// ── Scenario ────────────────────────────────────────────────────────────────

/// A single scenario: when a prompt matches, respond with a strategy.
#[derive(Debug, Clone)]
pub struct Scenario {
    pub matcher: PromptMatcher,
    pub response: ResponseStrategy,
    /// Human-readable description for test diagnostics.
    pub description: &'static str,
}

impl Scenario {
    /// Create a new scenario.
    pub fn new(
        matcher: PromptMatcher,
        response: ResponseStrategy,
        description: &'static str,
    ) -> Self {
        Self {
            matcher,
            response,
            description,
        }
    }

    /// Convenience: match any prompt, respond with preset.
    pub fn always(preset: Preset) -> Self {
        Self::new(
            PromptMatcher::Any,
            ResponseStrategy::Preset(preset),
            "matches any prompt",
        )
    }

    /// Convenience: match prompt containing text, respond with preset.
    pub fn when_contains(text: &'static str, preset: Preset) -> Self {
        Self::new(
            PromptMatcher::Contains(text.into()),
            ResponseStrategy::Preset(preset),
            "matches prompt containing keyword",
        )
    }

    /// Convenience: match prompt containing text, respond with raw string.
    pub fn when_contains_str(text: &'static str, response: &'static str) -> Self {
        Self::new(
            PromptMatcher::Contains(text.into()),
            ResponseStrategy::Immediate(response.into()),
            "matches prompt containing keyword",
        )
    }
}

// ── MockLlm ─────────────────────────────────────────────────────────────────

/// A scenario-driven mock LLM implementing the `Session` trait.
///
/// # Example
///
/// ```
/// let llm = MockLlm::new()
///     .scenario(Scenario::when_contains("approve", Preset::CodexApprove))
///     .scenario(Scenario::when_contains("reject", Preset::CodexReject));
/// ```
pub struct MockLlm {
    scenarios: Vec<Scenario>,
    /// All messages sent to the LLM (for inspection).
    sent_messages: RefCell<Vec<String>>,
    /// Pending replies queue (for multi-turn / delayed responses).
    reply_queue: RefCell<VecDeque<String>>,
    /// Whether the LLM is "ready" with a reply.
    ready: RefCell<bool>,
    /// Tracks how many ticks have passed since the last send.
    ticks_since_send: RefCell<usize>,
    /// Configured delay for the current pending response.
    pending_delay: RefCell<usize>,
    /// The active scenario that matched the last send.
    active_scenario: RefCell<Option<usize>>,
    /// Sequence iterator state.
    sequence_index: RefCell<usize>,
}

impl MockLlm {
    /// Create an empty MockLlm with no scenarios.
    pub fn new() -> Self {
        Self {
            scenarios: Vec::new(),
            sent_messages: RefCell::new(Vec::new()),
            reply_queue: RefCell::new(VecDeque::new()),
            ready: RefCell::new(false),
            ticks_since_send: RefCell::new(0),
            pending_delay: RefCell::new(0),
            active_scenario: RefCell::new(None),
            sequence_index: RefCell::new(0),
        }
    }

    /// Add a scenario. Scenarios are checked in order; the first match wins.
    pub fn scenario(mut self, scenario: Scenario) -> Self {
        self.scenarios.push(scenario);
        self
    }

    /// Add multiple scenarios at once.
    pub fn scenarios(mut self, scenarios: Vec<Scenario>) -> Self {
        self.scenarios.extend(scenarios);
        self
    }

    /// Push a raw reply directly (bypassing scenario matching).
    pub fn push_reply(&self, reply: impl Into<String>) {
        self.reply_queue.borrow_mut().push_back(reply.into());
        *self.ready.borrow_mut() = true;
    }

    /// Set ready state directly.
    pub fn set_ready(&self, ready: bool) {
        *self.ready.borrow_mut() = ready;
    }

    /// Get all sent messages (for assertions).
    pub fn sent_messages(&self) -> Vec<String> {
        self.sent_messages.borrow().clone()
    }

    /// Get the last sent message, if any.
    pub fn last_sent(&self) -> Option<String> {
        self.sent_messages.borrow().last().cloned()
    }

    /// Get the number of messages sent.
    pub fn send_count(&self) -> usize {
        self.sent_messages.borrow().len()
    }

    /// Advance the internal tick counter (called automatically by tick
    /// loops, but can be called manually for fine-grained control).
    pub fn advance_tick(&self) {
        *self.ticks_since_send.borrow_mut() += 1;
        let delay = *self.pending_delay.borrow();
        if delay > 0 {
            let elapsed = *self.ticks_since_send.borrow();
            if elapsed >= delay {
                // Delay expired: make ready if we have a queued reply
                if !self.reply_queue.borrow().is_empty() {
                    *self.ready.borrow_mut() = true;
                    *self.pending_delay.borrow_mut() = 0;
                }
            }
        }
    }

    /// Reset internal state (clear queues, counters, sent messages).
    pub fn reset(&self) {
        self.sent_messages.borrow_mut().clear();
        self.reply_queue.borrow_mut().clear();
        *self.ready.borrow_mut() = false;
        *self.ticks_since_send.borrow_mut() = 0;
        *self.pending_delay.borrow_mut() = 0;
        *self.active_scenario.borrow_mut() = None;
        *self.sequence_index.borrow_mut() = 0;
    }

    /// Match a prompt against scenarios and queue the appropriate response.
    fn match_and_queue(&self, prompt: &str) {
        for (idx, scenario) in self.scenarios.iter().enumerate() {
            if scenario.matcher.matches(prompt) {
                *self.active_scenario.borrow_mut() = Some(idx);
                match &scenario.response {
                    ResponseStrategy::Immediate(text) => {
                        self.reply_queue.borrow_mut().push_back(text.clone());
                        *self.ready.borrow_mut() = true;
                        *self.pending_delay.borrow_mut() = 0;
                    }
                    ResponseStrategy::AfterTicks(delay, text) => {
                        self.reply_queue.borrow_mut().push_back(text.clone());
                        *self.ready.borrow_mut() = false;
                        *self.pending_delay.borrow_mut() = *delay;
                        *self.ticks_since_send.borrow_mut() = 0;
                    }
                    ResponseStrategy::Sequence(seq) => {
                        for item in seq {
                            self.reply_queue.borrow_mut().push_back(item.clone());
                        }
                        *self.ready.borrow_mut() = true;
                    }
                    ResponseStrategy::Preset(preset) => {
                        let text = preset.render();
                        self.reply_queue.borrow_mut().push_back(text);
                        *self.ready.borrow_mut() = true;
                    }
                    ResponseStrategy::Dynamic(f) => {
                        let history = self.sent_messages.borrow().clone();
                        let text = f(&history);
                        self.reply_queue.borrow_mut().push_back(text);
                        *self.ready.borrow_mut() = true;
                    }
                }
                return;
            }
        }
        // No scenario matched: queue empty string as fallback
        self.reply_queue.borrow_mut().push_back(String::new());
        *self.ready.borrow_mut() = true;
    }
}

impl Default for MockLlm {
    fn default() -> Self {
        Self::new()
    }
}

impl Session for MockLlm {
    fn send(&mut self, message: &str) -> Result<(), SessionError> {
        self.sent_messages.borrow_mut().push(message.to_string());
        *self.ticks_since_send.borrow_mut() = 0;
        self.match_and_queue(message);
        Ok(())
    }

    fn send_with_hint(&mut self, message: &str, model: &str) -> Result<(), SessionError> {
        // Include model hint in the stored message for inspection
        let annotated = format!("[model={}] {}", model, message);
        self.sent_messages.borrow_mut().push(annotated);
        *self.ticks_since_send.borrow_mut() = 0;
        // Match against the original message (not annotated)
        self.match_and_queue(message);
        Ok(())
    }

    fn is_ready(&self) -> bool {
        // Check if delay has expired
        let delay = *self.pending_delay.borrow();
        if delay > 0 {
            let elapsed = *self.ticks_since_send.borrow();
            if elapsed >= delay {
                *self.ready.borrow_mut() = true;
                *self.pending_delay.borrow_mut() = 0;
            }
        }
        *self.ready.borrow()
    }

    fn receive(&mut self) -> Result<String, SessionError> {
        self.set_ready(false);
        self.reply_queue
            .borrow_mut()
            .pop_front()
            .ok_or(SessionError {
                kind: SessionErrorKind::UnexpectedFormat,
                message: "no reply queued in MockLlm".into(),
            })
    }
}

// ── Builder helpers ───────────────────────────────────────────────────────────

/// Convenience builder for common MockLlm configurations.
pub struct MockLlmBuilder {
    scenarios: Vec<Scenario>,
}

impl MockLlmBuilder {
    pub fn new() -> Self {
        Self {
            scenarios: Vec::new(),
        }
    }

    /// Add a scenario.
    pub fn on(mut self, matcher: PromptMatcher, response: ResponseStrategy) -> Self {
        self.scenarios.push(Scenario {
            matcher,
            response,
            description: "builder scenario",
        });
        self
    }

    /// Add a preset scenario.
    pub fn on_preset(mut self, matcher: PromptMatcher, preset: Preset) -> Self {
        self.scenarios.push(Scenario {
            matcher,
            response: ResponseStrategy::Preset(preset),
            description: "preset scenario",
        });
        self
    }

    /// Build the MockLlm.
    pub fn build(self) -> MockLlm {
        MockLlm {
            scenarios: self.scenarios,
            ..Default::default()
        }
    }
}

impl Default for MockLlmBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mock_llm_basic_send_receive() {
        let mut llm = MockLlm::new().scenario(Scenario::always(Preset::CodexApprove));

        llm.send("hello").unwrap();
        assert!(llm.is_ready());
        assert_eq!(llm.receive().unwrap(), "yes");
    }

    #[test]
    fn mock_llm_matches_contains() {
        let mut llm = MockLlm::new().scenario(Scenario::when_contains_str("danger", "STOP"));

        llm.send("detected danger").unwrap();
        assert_eq!(llm.receive().unwrap(), "STOP");
    }

    #[test]
    fn mock_llm_no_match_fallback_empty() {
        let mut llm = MockLlm::new(); // no scenarios

        llm.send("anything").unwrap();
        assert!(llm.is_ready());
        assert_eq!(llm.receive().unwrap(), "");
    }

    #[test]
    fn mock_llm_send_with_hint_records_model() {
        let mut llm = MockLlm::new().scenario(Scenario::always(Preset::CodexApprove));

        llm.send_with_hint("prompt", "claude-sonnet").unwrap();
        let sent = llm.sent_messages();
        assert_eq!(sent.len(), 1);
        assert!(sent[0].contains("claude-sonnet"));
    }

    #[test]
    fn mock_llm_sequence_response() {
        let mut llm = MockLlm::new().scenario(Scenario::new(
            PromptMatcher::Any,
            ResponseStrategy::Sequence(vec!["first".into(), "second".into()]),
            "sequence",
        ));

        llm.send("go").unwrap();
        assert_eq!(llm.receive().unwrap(), "first");
        // Queue still has "second" but is_ready is false after receive
        llm.push_reply("second"); // re-push for testing
        assert_eq!(llm.receive().unwrap(), "second");
    }

    #[test]
    fn mock_llm_delay_response() {
        let mut llm = MockLlm::new().scenario(Scenario::new(
            PromptMatcher::Any,
            ResponseStrategy::AfterTicks(2, "delayed".into()),
            "delayed",
        ));

        llm.send("go").unwrap();
        assert!(!llm.is_ready());
        llm.advance_tick();
        assert!(!llm.is_ready());
        llm.advance_tick();
        assert!(llm.is_ready());
        assert_eq!(llm.receive().unwrap(), "delayed");
    }

    #[test]
    fn mock_llm_dynamic_response() {
        let mut llm = MockLlm::new().scenario(Scenario::new(
            PromptMatcher::Any,
            ResponseStrategy::Dynamic(|history| format!("seen {} messages", history.len())),
            "dynamic",
        ));

        llm.send("a").unwrap();
        assert_eq!(llm.receive().unwrap(), "seen 1 messages");
    }

    #[test]
    fn mock_llm_sent_messages_inspection() {
        let mut llm = MockLlm::new().scenario(Scenario::always(Preset::CodexApprove));

        llm.send("msg1").unwrap();
        llm.send("msg2").unwrap();
        assert_eq!(llm.send_count(), 2);
        assert_eq!(llm.last_sent(), Some("msg2".into()));
    }

    #[test]
    fn mock_llm_reset_clears_state() {
        let mut llm = MockLlm::new().scenario(Scenario::always(Preset::CodexApprove));

        llm.send("msg").unwrap();
        llm.reset();
        assert!(!llm.is_ready());
        assert!(llm.sent_messages().is_empty());
    }
}
