//! Pipeline layer - core decision execution flow
//!
//! This layer orchestrates the complete decision flow:
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────┐
//! │                     Decision Pipeline                         │
//! │                                                               │
//! │  1. Pre-Processors (enrich context)                          │
//! │  2. Strategy Selection (select DecisionMaker type)           │
//! │  3. Maker Execution (engine decides)                         │
//! │  4. Post-Processors (validate output)                        │
//! │  5. Recording (history)                                       │
//! └─────────────────────────────────────────────────────────────┘
//! ```
//!
//! Provides:
//! - DecisionPipeline (main entry point)
//! - DecisionMaker trait and types
//! - DecisionMakerRegistry for maker management
//! - DecisionStrategy for maker selection
//! - Pre/Post Processors for context/output manipulation

pub mod pipeline;
pub mod maker;
pub mod maker_registry;
pub mod strategy;

pub use pipeline::*;
pub use maker::*;
pub use maker_registry::*;
pub use strategy::*;
