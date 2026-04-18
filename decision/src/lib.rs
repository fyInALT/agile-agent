//! agent-decision crate
//!
//! Decision layer for autonomous development - monitors provider outputs and makes decisions.
//!
//! # Architecture Overview
//!
//! The decision layer uses a flexible `DecisionMaker` abstraction that allows
//! different decision strategies to be plugged in:
//!
//! ```text
//! ┌──────────────────┐    ┌──────────────────┐
//! │ DecisionMaker    │    │ DecisionStrategy │
//! │ (executes        │    │ (selects which   │
//! │  decisions)      │    │  maker to use)   │
//! └──────────────────┘    └──────────────────┘
//!          │                       │
//!          ▼                       ▼
//! ┌─────────────────────────────────────────────┐
//! │            DecisionPipeline                 │
//! │  (orchestrates the decision flow)           │
//! └─────────────────────────────────────────────┘
//! ```
//!
//! # Extensibility
//!
//! - `DecisionMaker`: Implement custom decision execution logic
//! - `DecisionStrategy`: Implement custom maker selection logic
//! - `DecisionPreProcessor`: Pre-process context before decision
//! - `DecisionPostProcessor`: Post-process output after decision

pub mod action;
pub mod action_registry;
pub mod blocking;
pub mod builtin_actions;
pub mod builtin_situations;
pub mod context;
pub mod error;
pub mod output;
pub mod situation;
pub mod situation_registry;
pub mod types;

// Sprint 2: Output Classifier
pub mod acp_classifier;
pub mod classifier;
pub mod classifier_registry;
pub mod claude_classifier;
pub mod codex_classifier;
pub mod initializer;
pub mod provider_event;
pub mod provider_kind;

// Sprint 3: Decision Engine
pub mod cli_engine;
pub mod condition;
pub mod engine;
pub mod llm_caller;
pub mod llm_engine;
pub mod mock_engine;
pub mod rule_engine;
pub mod tiered_engine;

// Sprint 4-5: Context Cache and Lifecycle
pub mod lifecycle;

// Sprint 7: Error Recovery
pub mod recovery;

// Sprint 8: Observability and Integration
pub mod concurrent;
pub mod metrics;

// Sprint 9: Configurable Prompts
pub mod prompts;

// Sprint 10: Decision Maker Abstraction
pub mod maker;
pub mod maker_registry;
pub mod pipeline;
pub mod strategy;

// Task Concept: Task Entity and Workflow
pub mod task;
pub mod workflow;
pub mod automation;

// Sprint 13: Task Decision Engine
pub mod task_engine;

// Sprint 15: Task Metrics
pub mod task_metrics;

// Re-export core types
pub use error::*;
pub use situation::*;
pub use situation_registry::*;
pub use types::*;

// Re-export builtin situation types and implementations
pub use builtin_situations::{
    AgentIdleSituation, ClaimsCompletionSituation, ErrorSituation, PartialCompletionSituation,
    WaitingForChoiceSituation, acp_permission, agent_idle, claims_completion, claude_finished,
    codex_approval, error, partial_completion, register_situation_builtins, waiting_for_choice,
};

pub use action::*;
pub use action_registry::*;

// Re-export builtin action types and implementations
pub use builtin_actions::{
    ConfirmCompletionAction, ContinueAction, ContinueAllTasksAction, CustomInstructionAction,
    ReflectAction, RequestHumanAction, RetryAction, SelectOptionAction, StopIfCompleteAction,
    abort, confirm_completion, continue_action, continue_all_tasks, custom_instruction, reflect,
    register_action_builtins, reject_all, request_human, retry, select_first, select_option,
    stop_if_complete,
};

pub use blocking::*;
pub use context::*;
pub use output::*;

// Re-export Sprint 2 types
pub use classifier::*;
pub use classifier_registry::*;
pub use initializer::*;
pub use provider_event::*;
pub use provider_kind::*;

// Re-export Sprint 3 types
pub use cli_engine::*;
pub use condition::*;
pub use engine::*;
pub use llm_caller::*;
pub use llm_engine::*;
pub use mock_engine::*;
pub use rule_engine::*;
// Note: tiered_engine exports DecisionTier which conflicts with strategy::DecisionTier
// We export specific types instead to avoid ambiguity
pub use tiered_engine::{
    TierStatistics, TieredDecisionEngine, TieredDecisionRecord, TieredEngineConfig,
};

// Re-export Sprint 4-5 types
pub use lifecycle::*;

// Re-export Sprint 7 types
pub use recovery::*;

// Re-export Sprint 8 types
pub use concurrent::*;
pub use metrics::*;

// Re-export Sprint 9 types
pub use prompts::*;

// Re-export Sprint 10 types
pub use maker::*;
pub use maker_registry::*;
pub use pipeline::*;
pub use strategy::*;

// Re-export Task Concept types (avoiding conflict with lifecycle::TaskId)
pub use task::{Task, TaskStatus};
// Note: task::TaskId is separate from lifecycle::TaskId
// - task::TaskId: UUID-based, for Task entity
// - lifecycle::TaskId: String-based, for decision context tracking

// Re-export Workflow types (avoiding conflict with condition::Condition)
pub use workflow::{
    DecisionProcess, DecisionStage, ProcessConfig, ProcessValidationError,
    StageId, StageTransition, WorkflowAction,
};
// Note: workflow::Condition is separate from condition::Condition
// - workflow::Condition: for workflow stage conditions
// - condition::Condition: for decision logic conditions

pub use automation::*;

// Sprint 12: Task Persistence
pub mod persistence;

// Re-export Persistence types
pub use persistence::{
    ExecutionRecord, FileTaskStore, RecoveryError, StoreError, TaskRegistry, TaskStore, TaskUpdate,
};

// Sprint 13: Task Decision Engine
pub use task_engine::{
    AgentOutput, HumanResponse, TaskDecisionAction, TaskDecisionEngine,
};

// Sprint 15: Task Metrics
pub use task_metrics::TaskMetrics;
