//! Shared test fixtures for decision-dsl integration tests.
//!
//! This module provides:
//! - `mock_llm`: A scenario-driven Mock LLM implementing `Session`
//! - `presets`: Realistic LLM output patterns (Codex, Claude, Agent)
//! - `harness`: High-level `IntegrationHarness` for behavior tree tests

pub mod harness;
pub mod mock_llm;
pub mod presets;

// Re-export commonly used items for convenience
pub use harness::{IntegrationHarness, RecordingRunner, llm_always_approves, llm_always_escalates, llm_delayed_approval};
pub use mock_llm::{MockLlm, PromptMatcher, ResponseStrategy, Scenario};
pub use presets::Preset;
