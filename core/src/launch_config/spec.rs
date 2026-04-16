use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::provider::ProviderKind;

/// Specifies how the launch config was provided by the user.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LaunchSourceMode {
    /// Use provider's default executable and environment.
    HostDefault,
    /// Only environment variable overrides (KEY=VALUE lines).
    EnvOnly,
    /// Full command fragment with executable and arguments.
    CommandFragment,
}

/// Specifies where the launch config originated from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LaunchSourceOrigin {
    /// User manually entered the config.
    Manual,
    /// Loaded from a saved template.
    Template,
    /// No config provided, using provider defaults.
    HostDefault,
}

/// Declarative user input representation for agent launch configuration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LaunchInputSpec {
    /// The provider to use for this agent.
    pub provider: ProviderKind,
    /// How the config was provided (HostDefault, EnvOnly, CommandFragment).
    pub source_mode: LaunchSourceMode,
    /// Where the config originated (Manual, Template, HostDefault).
    pub source_origin: LaunchSourceOrigin,
    /// Raw text as entered by user (for reconstruction/display).
    pub raw_text: Option<String>,
    /// Environment variable overrides (KEY=VALUE pairs).
    pub env_overrides: BTreeMap<String, String>,
    /// Requested executable path or name.
    pub requested_executable: Option<String>,
    /// Extra arguments to pass after the executable.
    pub extra_args: Vec<String>,
    /// Template ID if loaded from a template.
    pub template_id: Option<String>,
}

impl LaunchInputSpec {
    /// Create a new LaunchInputSpec with default values.
    pub fn new(provider: ProviderKind) -> Self {
        Self {
            provider,
            source_mode: LaunchSourceMode::HostDefault,
            source_origin: LaunchSourceOrigin::HostDefault,
            raw_text: None,
            env_overrides: BTreeMap::new(),
            requested_executable: None,
            extra_args: Vec::new(),
            template_id: None,
        }
    }

    /// Create a LaunchInputSpec with env overrides only.
    pub fn env_only(provider: ProviderKind, env_overrides: BTreeMap<String, String>) -> Self {
        Self {
            provider,
            source_mode: LaunchSourceMode::EnvOnly,
            source_origin: LaunchSourceOrigin::Manual,
            raw_text: None,
            env_overrides,
            requested_executable: None,
            extra_args: Vec::new(),
            template_id: None,
        }
    }

    /// Create a LaunchInputSpec with command fragment.
    pub fn command_fragment(
        provider: ProviderKind,
        requested_executable: String,
        extra_args: Vec<String>,
        env_overrides: BTreeMap<String, String>,
    ) -> Self {
        Self {
            provider,
            source_mode: LaunchSourceMode::CommandFragment,
            source_origin: LaunchSourceOrigin::Manual,
            raw_text: None,
            env_overrides,
            requested_executable: Some(requested_executable),
            extra_args,
            template_id: None,
        }
    }
}

/// Resolved launch configuration used by provider execution.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResolvedLaunchSpec {
    /// The provider for this launch.
    pub provider: ProviderKind,
    /// The fully resolved executable path.
    pub resolved_executable_path: String,
    /// Effective environment variables (including provider defaults and overrides).
    pub effective_env: BTreeMap<String, String>,
    /// Extra arguments to pass to the executable.
    pub extra_args: Vec<String>,
    /// Timestamp when this spec was resolved (ISO 8601 format).
    pub resolved_at: String,
    /// The source mode that was used to resolve this spec.
    pub derived_from: LaunchSourceMode,
    /// Notes about how the resolution was performed (for debugging).
    pub resolution_notes: Vec<String>,
}

impl ResolvedLaunchSpec {
    /// Create a new ResolvedLaunchSpec.
    pub fn new(
        provider: ProviderKind,
        resolved_executable_path: String,
        effective_env: BTreeMap<String, String>,
        extra_args: Vec<String>,
        derived_from: LaunchSourceMode,
    ) -> Self {
        Self {
            provider,
            resolved_executable_path,
            effective_env,
            extra_args,
            resolved_at: chrono::Utc::now().to_rfc3339(),
            derived_from,
            resolution_notes: Vec::new(),
        }
    }

    /// Add a resolution note.
    pub fn add_note(&mut self, note: impl Into<String>) {
        self.resolution_notes.push(note.into());
    }
}

/// Bundle combining work and decision agent configs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentLaunchBundle {
    /// Input spec for the work agent.
    pub work_input: LaunchInputSpec,
    /// Resolved spec for the work agent.
    pub work_resolved: ResolvedLaunchSpec,
    /// Input spec for the decision agent (can be empty for host default).
    pub decision_input: LaunchInputSpec,
    /// Resolved spec for the decision agent.
    pub decision_resolved: ResolvedLaunchSpec,
}

impl AgentLaunchBundle {
    /// Create a new bundle with the same config for both agents.
    pub fn symmetric(provider: ProviderKind, input: LaunchInputSpec, resolved: ResolvedLaunchSpec) -> Self {
        Self {
            work_input: input.clone(),
            work_resolved: resolved.clone(),
            decision_input: input,
            decision_resolved: resolved,
        }
    }

    /// Create a bundle with different configs for work and decision agents.
    pub fn asymmetric(
        work_input: LaunchInputSpec,
        work_resolved: ResolvedLaunchSpec,
        decision_input: LaunchInputSpec,
        decision_resolved: ResolvedLaunchSpec,
    ) -> Self {
        Self {
            work_input,
            work_resolved,
            decision_input,
            decision_resolved,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_launch_input_spec_default() {
        let spec = LaunchInputSpec::new(ProviderKind::Claude);
        assert_eq!(spec.provider, ProviderKind::Claude);
        assert_eq!(spec.source_mode, LaunchSourceMode::HostDefault);
        assert_eq!(spec.source_origin, LaunchSourceOrigin::HostDefault);
        assert!(spec.env_overrides.is_empty());
        assert!(spec.requested_executable.is_none());
        assert!(spec.extra_args.is_empty());
    }

    #[test]
    fn test_launch_input_spec_env_only() {
        let mut env = BTreeMap::new();
        env.insert("ANTHROPIC_MODEL".to_string(), "opus-4".to_string());
        let spec = LaunchInputSpec::env_only(ProviderKind::Claude, env);
        assert_eq!(spec.source_mode, LaunchSourceMode::EnvOnly);
        assert_eq!(spec.source_origin, LaunchSourceOrigin::Manual);
        assert_eq!(spec.env_overrides.get("ANTHROPIC_MODEL"), Some(&"opus-4".to_string()));
    }

    #[test]
    fn test_launch_input_spec_command_fragment() {
        let env = BTreeMap::new();
        let spec = LaunchInputSpec::command_fragment(
            ProviderKind::Claude,
            "claude".to_string(),
            vec!["--flag".to_string()],
            env,
        );
        assert_eq!(spec.source_mode, LaunchSourceMode::CommandFragment);
        assert_eq!(spec.requested_executable, Some("claude".to_string()));
        assert_eq!(spec.extra_args, vec!["--flag".to_string()]);
    }

    #[test]
    fn test_resolved_launch_spec() {
        let env = BTreeMap::new();
        let spec = ResolvedLaunchSpec::new(
            ProviderKind::Claude,
            "/usr/bin/claude".to_string(),
            env,
            vec![],
            LaunchSourceMode::HostDefault,
        );
        assert_eq!(spec.resolved_executable_path, "/usr/bin/claude");
        assert!(!spec.resolved_at.is_empty());
    }

    #[test]
    fn test_resolved_launch_spec_add_note() {
        let env = BTreeMap::new();
        let mut spec = ResolvedLaunchSpec::new(
            ProviderKind::Claude,
            "/usr/bin/claude".to_string(),
            env,
            vec![],
            LaunchSourceMode::HostDefault,
        );
        spec.add_note("found via which command");
        assert_eq!(spec.resolution_notes.len(), 1);
    }

    #[test]
    fn test_agent_launch_bundle_symmetric() {
        let input = LaunchInputSpec::new(ProviderKind::Claude);
        let resolved = ResolvedLaunchSpec::new(
            ProviderKind::Claude,
            "/usr/bin/claude".to_string(),
            BTreeMap::new(),
            vec![],
            LaunchSourceMode::HostDefault,
        );
        let bundle = AgentLaunchBundle::symmetric(ProviderKind::Claude, input, resolved);
        assert_eq!(bundle.work_input.provider, ProviderKind::Claude);
        assert_eq!(bundle.decision_input.provider, ProviderKind::Claude);
    }

    #[test]
    fn test_agent_launch_bundle_asymmetric() {
        let work_input = LaunchInputSpec::new(ProviderKind::Claude);
        let decision_input = LaunchInputSpec::new(ProviderKind::Codex);
        let work_resolved = ResolvedLaunchSpec::new(
            ProviderKind::Claude,
            "/usr/bin/claude".to_string(),
            BTreeMap::new(),
            vec![],
            LaunchSourceMode::HostDefault,
        );
        let decision_resolved = ResolvedLaunchSpec::new(
            ProviderKind::Codex,
            "/usr/bin/codex".to_string(),
            BTreeMap::new(),
            vec![],
            LaunchSourceMode::HostDefault,
        );
        let bundle = AgentLaunchBundle::asymmetric(work_input, work_resolved, decision_input, decision_resolved);
        assert_eq!(bundle.work_input.provider, ProviderKind::Claude);
        assert_eq!(bundle.decision_input.provider, ProviderKind::Codex);
    }
}