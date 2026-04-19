//! Provider Profile System
//!
//! This module provides a configurable provider profile system that allows
//! users to define named profiles with custom environment variables and CLI
//! arguments for different LLM backends.
//!
//! # Overview
//!
//! - `CliBaseType`: Enum for CLI executable types (Claude, Codex, etc.)
//! - `ProviderProfile`: Named profile with configuration
//! - `ProfileStore`: Storage for profiles with defaults
//! - `interpolate`: Environment variable interpolation (${VAR} syntax)
//!
//! # Example
//!
//! ```ignore
//! use crate::provider_profile::{ProviderProfile, CliBaseType, ProfileStore};
//!
//! let profile = ProviderProfile::new("claude-glm-5", CliBaseType::Claude)
//!     .with_env("ANTHROPIC_API_KEY", "${GLM_API_KEY}".to_string())
//!     .with_env("ANTHROPIC_BASE_URL", "https://glm.example.com/v1".to_string())
//!     .with_display_name("Claude by GLM-5".to_string());
//!
//! let store = ProfileStore::with_defaults();
//! store.add_profile(profile);
//! ```

pub mod error;
pub mod interpolate;
pub mod profile;
pub mod store;
pub mod types;

pub use error::ProfileError;
pub use interpolate::{interpolate_env_value, interpolate_profile_env};
pub use profile::{ProviderProfile, ProfileId};
pub use store::ProfileStore;
pub use types::CliBaseType;