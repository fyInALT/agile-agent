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
//! - `persistence.rs` - JSON file save/load for launch configs
//! - `redaction.rs` - Sensitive data redaction
//! - `restore.rs` - Agent restore with launch bundle
//! - `error.rs` - ParseError, ValidationError types

pub mod context;
pub mod error;
pub mod parser;
pub mod persistence;
pub mod redaction;
pub mod resolver;
pub mod restore;
pub mod spec;
pub mod validation;

// Re-export commonly used types
pub use context::ProviderLaunchContext;
pub use error::{ParseError, ValidationError};
pub use spec::{
    AgentLaunchBundle, LaunchInputSpec, LaunchSourceMode, LaunchSourceOrigin,
    ResolvedLaunchSpec,
};
pub use parser::{
    detect_source_mode, parse, parse_command_fragment, parse_env_only,
};
pub use validation::{
    validate_launch_input_spec, validate_provider_consistency,
    validate_provider_supports_launch_config, validate_reserved_args,
};
pub use resolver::{
    resolve_bundle, resolve_decision_launch_spec, resolve_executable_path,
    resolve_host_env, resolve_launch_spec, generate_host_default_input,
};
pub use persistence::{
    save_launch_config, load_launch_config, has_launch_config, delete_launch_config,
    launch_config_path, LAUNCH_CONFIG_FILENAME,
};
pub use redaction::{
    is_sensitive_key, redact_env_map, redact_env_value, format_launch_summary,
    SENSITIVE_KEY_PATTERNS,
};
pub use restore::{
    RestoreError, validate_executable_exists, validate_bundle_executable,
    check_restore_eligibility,
};