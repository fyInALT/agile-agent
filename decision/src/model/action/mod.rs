#![allow(clippy::module_inception)]

//! Action subsystem - decision execution actions
//!
//! Provides:
//! - DecisionAction trait
//! - ActionRegistry for dynamic registration
//! - Built-in actions (SelectOption, Reflect, Continue, etc.)

pub mod action;
pub mod action_registry;
pub mod builtin_actions;

pub use action::*;
pub use action_registry::*;
pub use builtin_actions::*;
