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