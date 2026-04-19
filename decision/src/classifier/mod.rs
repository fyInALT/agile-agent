//! Classifier layer - output classification
//!
//! Classifies provider outputs into situation types:
//! - OutputClassifier trait
//! - ClassifierRegistry for classifier management
//! - ACPClassifier (ACP protocol outputs)
//! - ClaudeClassifier (Claude outputs)
//! - CodexClassifier (Codex outputs)

pub mod classifier;
pub mod classifier_registry;
pub mod acp_classifier;
pub mod claude_classifier;
pub mod codex_classifier;

pub use classifier::*;
pub use classifier_registry::*;
pub use acp_classifier::*;
pub use claude_classifier::*;
pub use codex_classifier::*;
