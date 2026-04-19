//! Provider layer - LLM provider adaptation
//!
//! Provides:
//! - ProviderKind (Claude, Codex, etc.)
//! - ProviderEvent (output events from providers)
//! - Initializer (provider initialization)

pub mod provider_kind;
pub mod provider_event;
pub mod initializer;

pub use provider_kind::*;
pub use provider_event::*;
pub use initializer::*;
