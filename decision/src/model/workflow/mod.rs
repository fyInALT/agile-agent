#![allow(clippy::module_inception)]

//! Workflow subsystem - decision process definition
//!
//! Provides:
//! - DecisionProcess with stages
//! - DecisionStage with transitions
//! - Condition for stage entry/exit
//! - WorkflowAction for decisions

pub mod workflow;

pub use workflow::*;
