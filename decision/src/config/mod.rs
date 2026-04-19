//! Config layer - configuration management
//!
//! Provides:
//! - YAML configuration loading
//! - Prompt templates

pub mod yaml_loader;
pub mod prompts;

pub use yaml_loader::*;
