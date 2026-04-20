//! Provider Profile Definition

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::profile::types::CliBaseType;

/// Provider profile identifier (user-defined name)
pub type ProfileId = String;

/// Named provider profile with configuration
///
/// A profile combines:
/// - Base CLI type (Claude, Codex, etc.)
/// - Environment variable overrides (supports ${ENV_VAR} interpolation)
/// - Extra CLI arguments
/// - Display metadata (name, description, icon)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderProfile {
    /// Unique profile identifier
    pub id: ProfileId,

    /// Base CLI executable type
    pub base_cli: CliBaseType,

    /// Environment variable overrides (supports ${ENV_VAR} interpolation)
    #[serde(default)]
    pub env_overrides: BTreeMap<String, String>,

    /// Extra arguments for the CLI
    #[serde(default)]
    pub extra_args: Vec<String>,

    /// Display name for UI
    pub display_name: String,

    /// Optional description
    #[serde(default)]
    pub description: Option<String>,

    /// Optional icon/emoji for UI
    #[serde(default)]
    pub icon: Option<String>,
}

impl ProviderProfile {
    /// Create a new profile with minimal configuration
    pub fn new(id: ProfileId, base_cli: CliBaseType) -> Self {
        Self {
            id,
            base_cli,
            env_overrides: BTreeMap::new(),
            extra_args: Vec::new(),
            display_name: format!("{} Profile", base_cli.display_name()),
            description: None,
            icon: None,
        }
    }

    /// Create default profile for a CLI type
    pub fn default_for_cli(cli: CliBaseType) -> Self {
        Self {
            id: format!("{}-default", cli.label()),
            base_cli: cli,
            env_overrides: BTreeMap::new(),
            extra_args: Vec::new(),
            display_name: format!("{} (Default)", cli.display_name()),
            description: Some(format!("Default profile for {} CLI", cli.display_name())),
            icon: None,
        }
    }

    /// Add an environment variable override
    pub fn with_env(mut self, key: String, value: String) -> Self {
        self.env_overrides.insert(key, value);
        self
    }

    /// Add multiple environment variable overrides
    pub fn with_envs(mut self, envs: BTreeMap<String, String>) -> Self {
        for (key, value) in envs {
            self.env_overrides.insert(key, value);
        }
        self
    }

    /// Add an extra CLI argument
    pub fn with_arg(mut self, arg: String) -> Self {
        self.extra_args.push(arg);
        self
    }

    /// Add multiple extra CLI arguments
    pub fn with_args(mut self, args: Vec<String>) -> Self {
        for arg in args {
            self.extra_args.push(arg);
        }
        self
    }

    /// Set the display name
    pub fn with_display_name(mut self, name: String) -> Self {
        self.display_name = name;
        self
    }

    /// Set the description
    pub fn with_description(mut self, desc: String) -> Self {
        self.description = Some(desc);
        self
    }

    /// Set the icon
    pub fn with_icon(mut self, icon: String) -> Self {
        self.icon = Some(icon);
        self
    }

    /// Check if an env value uses interpolation syntax
    pub fn uses_env_reference(value: &str) -> bool {
        value.starts_with("${") && value.ends_with("}")
    }

    /// Extract env var name from ${VAR} syntax
    pub fn extract_env_var_name(value: &str) -> Option<String> {
        if Self::uses_env_reference(value) {
            let inner = &value[2..value.len() - 1];
            // Validate: must be valid env var name (alphanumeric + underscore, starts with letter/underscore)
            if inner.is_empty() {
                return None;
            }
            let first_char = inner.chars().next().unwrap();
            if !first_char.is_ascii_alphabetic() && first_char != '_' {
                return None;
            }
            if !inner.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
                return None;
            }
            Some(inner.to_string())
        } else {
            None
        }
    }

    /// Get all env var references used in this profile
    pub fn env_var_references(&self) -> Vec<String> {
        self.env_overrides
            .values()
            .filter_map(|v| Self::extract_env_var_name(v))
            .collect()
    }

    /// Check if this profile uses any env references
    pub fn has_env_references(&self) -> bool {
        self.env_overrides.values().any(|v| Self::uses_env_reference(v))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_profile_new() {
        let profile = ProviderProfile::new("test-profile".to_string(), CliBaseType::Claude);
        assert_eq!(profile.id, "test-profile");
        assert_eq!(profile.base_cli, CliBaseType::Claude);
        assert!(profile.env_overrides.is_empty());
        assert!(profile.extra_args.is_empty());
        assert_eq!(profile.display_name, "Claude CLI Profile");
    }

    #[test]
    fn test_profile_default_for_cli() {
        let profile = ProviderProfile::default_for_cli(CliBaseType::Claude);
        assert_eq!(profile.id, "claude-default");
        assert_eq!(profile.base_cli, CliBaseType::Claude);
        assert_eq!(profile.display_name, "Claude CLI (Default)");
    }

    #[test]
    fn test_profile_with_env() {
        let profile = ProviderProfile::new("test".to_string(), CliBaseType::Claude)
            .with_env("API_KEY".to_string(), "${MY_API_KEY}".to_string());
        assert_eq!(
            profile.env_overrides.get("API_KEY"),
            Some(&"${MY_API_KEY}".to_string())
        );
    }

    #[test]
    fn test_profile_with_args() {
        let profile = ProviderProfile::new("test".to_string(), CliBaseType::Claude)
            .with_arg("--model".to_string())
            .with_arg("opus".to_string());
        assert_eq!(profile.extra_args, vec!["--model", "opus"]);
    }

    #[test]
    fn test_profile_uses_env_reference() {
        assert!(ProviderProfile::uses_env_reference("${API_KEY}"));
        assert!(ProviderProfile::uses_env_reference("${MY_VAR_123}"));
        assert!(!ProviderProfile::uses_env_reference("API_KEY"));
        assert!(!ProviderProfile::uses_env_reference("$API_KEY"));
        assert!(!ProviderProfile::uses_env_reference("${API_KEY"));
        assert!(ProviderProfile::uses_env_reference("${API_KEY}"));
    }

    #[test]
    fn test_profile_extract_env_var_name() {
        assert_eq!(
            ProviderProfile::extract_env_var_name("${API_KEY}"),
            Some("API_KEY".to_string())
        );
        assert_eq!(
            ProviderProfile::extract_env_var_name("${MY_VAR_123}"),
            Some("MY_VAR_123".to_string())
        );
        assert_eq!(
            ProviderProfile::extract_env_var_name("${_UNDERSCORE}"),
            Some("_UNDERSCORE".to_string())
        );
        // Invalid cases
        assert_eq!(ProviderProfile::extract_env_var_name("${}"), None);
        assert_eq!(ProviderProfile::extract_env_var_name("${123}"), None); // starts with digit
        assert_eq!(ProviderProfile::extract_env_var_name("${VAR-DASH}"), None); // contains dash
        assert_eq!(ProviderProfile::extract_env_var_name("plain"), None);
    }

    #[test]
    fn test_profile_env_var_references() {
        let profile = ProviderProfile::new("test".to_string(), CliBaseType::Claude)
            .with_env("KEY1".to_string(), "${VAR1}".to_string())
            .with_env("KEY2".to_string(), "${VAR2}".to_string())
            .with_env("KEY3".to_string(), "plain-value".to_string());
        let refs = profile.env_var_references();
        assert_eq!(refs.len(), 2);
        assert!(refs.contains(&"VAR1".to_string()));
        assert!(refs.contains(&"VAR2".to_string()));
    }

    #[test]
    fn test_profile_has_env_references() {
        let with_refs = ProviderProfile::new("test".to_string(), CliBaseType::Claude)
            .with_env("KEY".to_string(), "${VAR}".to_string());
        assert!(with_refs.has_env_references());

        let without_refs = ProviderProfile::new("test".to_string(), CliBaseType::Claude)
            .with_env("KEY".to_string(), "plain".to_string());
        assert!(!without_refs.has_env_references());
    }

    #[test]
    fn test_profile_serde() {
        let profile = ProviderProfile::new("test".to_string(), CliBaseType::Claude)
            .with_env("KEY".to_string(), "${VAR}".to_string())
            .with_display_name("Test Profile".to_string());
        let json = serde_json::to_string(&profile).unwrap();
        let parsed: ProviderProfile = serde_json::from_str(&json).unwrap();
        assert_eq!(profile.id, parsed.id);
        assert_eq!(profile.base_cli, parsed.base_cli);
        assert_eq!(profile.env_overrides, parsed.env_overrides);
    }
}