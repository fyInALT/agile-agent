//! agent-decision crate
//!
//! Decision layer for autonomous development - monitors provider outputs and makes decisions.

pub mod error;
pub mod types;
pub mod situation;
pub mod situation_registry;
pub mod builtin_situations;
pub mod action;
pub mod action_registry;
pub mod builtin_actions;
pub mod output;
pub mod context;
pub mod blocking;

// Sprint 2: Output Classifier
pub mod provider_kind;
pub mod provider_event;
pub mod classifier;
pub mod classifier_registry;
pub mod claude_classifier;
pub mod codex_classifier;
pub mod acp_classifier;
pub mod initializer;

// Sprint 3: Decision Engine
pub mod engine;
pub mod llm_engine;
pub mod cli_engine;
pub mod tiered_engine;
pub mod mock_engine;
pub mod condition;
pub mod rule_engine;

// Sprint 4-5: Context Cache and Lifecycle
pub mod lifecycle;

// Sprint 7: Error Recovery
pub mod recovery;

// Sprint 8: Observability and Integration
pub mod metrics;
pub mod concurrent;

// Re-export core types
pub use error::*;
pub use types::*;
pub use situation::*;
pub use situation_registry::*;

// Re-export builtin situation types and implementations
pub use builtin_situations::{
    waiting_for_choice, claims_completion, partial_completion, error,
    claude_finished, codex_approval, acp_permission,
    WaitingForChoiceSituation, ClaimsCompletionSituation,
    PartialCompletionSituation, ErrorSituation,
    register_situation_builtins,
};

pub use action::*;
pub use action_registry::*;

// Re-export builtin action types and implementations
pub use builtin_actions::{
    select_option, select_first, reject_all, reflect,
    confirm_completion, continue_action, retry,
    request_human, abort, custom_instruction,
    SelectOptionAction, ReflectAction, ConfirmCompletionAction,
    ContinueAction, RetryAction, RequestHumanAction, CustomInstructionAction,
    register_action_builtins,
};

pub use output::*;
pub use context::*;
pub use blocking::*;

// Re-export Sprint 2 types
pub use provider_kind::*;
pub use provider_event::*;
pub use classifier::*;
pub use classifier_registry::*;
pub use initializer::*;

// Re-export Sprint 3 types
pub use engine::*;
pub use llm_engine::*;
pub use cli_engine::*;
pub use tiered_engine::*;
pub use mock_engine::*;
pub use condition::*;
pub use rule_engine::*;

// Re-export Sprint 4-5 types
pub use lifecycle::*;

// Re-export Sprint 7 types
pub use recovery::*;

// Re-export Sprint 8 types
pub use metrics::*;
pub use concurrent::*;