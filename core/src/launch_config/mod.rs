//! Launch Configuration Module
//!
//! This module provides data models, parsers, and validation for agent launch configurations.
//!
//! # Structure
//!
//! - `spec.rs` - Data models: LaunchInputSpec, ResolvedLaunchSpec, AgentLaunchBundle
//! - `parser.rs` - Input parsers: env-only and command-fragment modes
//! - `validation.rs` - Provider consistency, reserved args, Mock exclusion
//! - `error.rs` - ParseError, ValidationError types

pub mod error;
pub mod parser;
pub mod spec;
pub mod validation;

// Re-export commonly used types
pub use error::{ParseError, ValidationError};
pub use spec::{AgentLaunchBundle, LaunchInputSpec, LaunchSourceMode, LaunchSourceOrigin, ResolvedLaunchSpec};
pub use parser::{detect_source_mode, parse, parse_command_fragment, parse_env_only};
pub use validation::{
    validate_launch_input_spec, validate_provider_consistency, validate_provider_supports_launch_config,
    validate_reserved_args,
};