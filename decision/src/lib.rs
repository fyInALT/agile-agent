//! Decision Layer - Autonomous Development Decision Making
//!
//! # Architecture Overview
//!
//! The decision layer is organized into logical layers:
//!
//! ```text
//! Core → Model → Pipeline → Engine → Classifier → Provider → State → Runtime → Config
//! ```
//!
//! # Main Execution Flow
//!
//! 1. Classifier identifies provider output → Situation
//! 2. Context built from situation + history + metadata  
//! 3. Pipeline orchestrates:
//!    - Pre-processors enrich context
//!    - Strategy selects DecisionMaker
//!    - Maker executes via Engine
//!    - Post-processors validate output
//! 4. Actions returned for execution
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────┐
//! │                     Decision Pipeline                         │
//! │                                                               │
//! │  1. Pre-Processors (enrich context)                          │
//! │  2. Strategy Selection (select DecisionMaker type)           │
//! │     - Simple → RuleBased                                      │
//! │     - Medium/Complex → LLM                                    │
//! │     - Critical → Human                                        │
//! │  3. Maker Execution (engine decides)                         │
//! │  4. Post-Processors (validate output)                        │
//! │  5. Recording (history)                                       │
//! └─────────────────────────────────────────────────────────────┘
//! ```

// ============================================================================
// Layer imports
// ============================================================================

// Core layer - foundational types
pub mod core;

// Model layer - business models
pub mod model;

// Pipeline layer - main execution flow
pub mod pipeline;

// Engine layer - decision implementations
pub mod engine;

// Classifier layer - output classification
pub mod classifier;

// Provider layer - LLM provider adaptation
pub mod provider;

// State layer - state management
pub mod state;

// Runtime layer - runtime support
pub mod runtime;

// Config layer - configuration
pub mod config;

// Command layer - pure decision commands (read-only interface)
pub mod command;

// Shared condition expressions (used by multiple layers)
pub mod condition;

// ============================================================================
// Backward compatibility re-exports
// ============================================================================
// These re-exports maintain the original API for existing code

// Command types (read-only decision output)
pub use command::DecisionCommand;

// Core types
pub use core::*;

// Model types
pub use model::situation::*;
pub use model::action::*;
pub use model::task::*;
pub use model::workflow::*;

// Pipeline types (main entry point)
pub use pipeline::*;

// Engine types - selectively export to avoid DecisionTier conflict
pub use engine::engine::*;
pub use engine::rule_engine::*;
pub use engine::llm_engine::*;
pub use engine::cli_engine::*;
pub use engine::mock_engine::*;
pub use engine::task_engine::*;
pub use engine::llm_caller::*;
// Note: tiered_engine exports DecisionTier which conflicts with strategy::DecisionTier
// Export specific types instead
pub use engine::tiered_engine::{
    TierStatistics, TieredDecisionRecord, TieredEngineConfig,
};

// Classifier types
pub use classifier::*;

// Provider types
pub use provider::*;

// State types
pub use state::*;

// Runtime types
pub use runtime::*;

// Config types
pub use config::*;

// ============================================================================
// Main entry points
// ============================================================================

/// Primary decision execution entry point
pub use pipeline::{DecisionPipeline, PipelineBuilder, PipelineConfig};

/// Default engine for most use cases
pub use engine::tiered_engine::TieredDecisionEngine;

/// Task-specific decision engine
pub use engine::task_engine::TaskDecisionEngine;

/// Task entity
pub use model::task::{Task, TaskStatus, TaskId};
