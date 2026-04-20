//! Launch Configuration Module
//!
//! This module provides data models, parsers, and validation for agent launch configurations.
//!
//! # Structure
//!
//! - `spec.rs` - Data models: LaunchInputSpec, ResolvedLaunchSpec, AgentLaunchBundle
//! - `parser.rs` - Input parsers: env-only and command-fragment modes
//! - `validation.rs` - Provider consistency, reserved args, Mock exclusion
//! - `resolver.rs` - Host environment and executable resolution
//! - `redaction.rs` - Sensitive data redaction
//! - `restore.rs` - Agent restore with launch bundle
//! - `error.rs` - ParseError, ValidationError types
//!
//! Note: persistence.rs remains in agent-core due to AgentStore dependency.

pub mod context;
pub mod error;
pub mod parser;
pub mod redaction;
pub mod resolver;
pub mod restore;
pub mod spec;
pub mod validation;

// Re-export commonly used types
pub use context::ProviderLaunchContext;
pub use error::{ParseError, ValidationError};
pub use parser::{detect_source_mode, parse, parse_command_fragment, parse_env_only};
pub use redaction::{
    SENSITIVE_KEY_PATTERNS, format_launch_summary, is_sensitive_key, redact_env_map,
    redact_env_value,
};
pub use resolver::{
    generate_host_default_input, resolve_bundle, resolve_decision_launch_spec,
    resolve_executable_path, resolve_host_env, resolve_launch_spec,
};
pub use restore::{
    RestoreError, check_restore_eligibility, validate_bundle_executable, validate_executable_exists,
};
pub use spec::{
    AgentLaunchBundle, LaunchInputSpec, LaunchSourceMode, LaunchSourceOrigin, ResolvedLaunchSpec,
};
pub use validation::{
    validate_launch_input_spec, validate_provider_consistency,
    validate_provider_supports_launch_config, validate_reserved_args,
};
