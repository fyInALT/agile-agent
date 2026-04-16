use std::path::PathBuf;

use crate::provider::{ProviderKind, SessionHandle};
use crate::launch_config::spec::ResolvedLaunchSpec;

/// Context for launching a provider with structured configuration.
///
/// This replaces the implicit environment-based startup with explicit
/// configuration from ResolvedLaunchSpec.
#[derive(Debug, Clone, PartialEq)]
pub struct ProviderLaunchContext {
    /// The resolved launch specification.
    pub spec: ResolvedLaunchSpec,
    /// Current working directory for the provider process.
    pub cwd: PathBuf,
    /// Session handle for conversation continuity (if resuming).
    pub session_handle: Option<SessionHandle>,
}

impl ProviderLaunchContext {
    /// Create a new context with a resolved spec and working directory.
    pub fn new(spec: ResolvedLaunchSpec, cwd: PathBuf) -> Self {
        Self {
            spec,
            cwd,
            session_handle: None,
        }
    }

    /// Create a new context with an optional session handle.
    pub fn with_session_handle(mut self, handle: SessionHandle) -> Self {
        self.session_handle = Some(handle);
        self
    }

    /// Create a new context with an optional session handle (Option variant).
    pub fn with_opt_session_handle(mut self, handle: Option<SessionHandle>) -> Self {
        self.session_handle = handle;
        self
    }

    /// Get the provider kind from the spec.
    pub fn provider(&self) -> ProviderKind {
        self.spec.provider
    }

    /// Get the resolved executable path.
    pub fn executable_path(&self) -> &str {
        &self.spec.resolved_executable_path
    }

    /// Get the effective environment variables.
    pub fn effective_env(&self) -> &std::collections::BTreeMap<String, String> {
        &self.spec.effective_env
    }

    /// Get the extra arguments.
    pub fn extra_args(&self) -> &[String] {
        &self.spec.extra_args
    }

    /// Create a default context from a provider kind (uses host defaults).
    pub fn from_provider(provider: ProviderKind, cwd: PathBuf) -> anyhow::Result<Self> {
        use crate::launch_config::resolver::resolve_launch_spec;
        use crate::launch_config::spec::LaunchInputSpec;

        let input = LaunchInputSpec::new(provider);
        let spec = resolve_launch_spec(&input)?;
        Ok(Self::new(spec, cwd))
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;
    use crate::launch_config::spec::LaunchSourceMode;
    use crate::provider::ProviderKind;

    #[test]
    fn test_provider_launch_context_new() {
        let spec = ResolvedLaunchSpec::new(
            ProviderKind::Claude,
            "/usr/bin/claude".to_string(),
            BTreeMap::new(),
            vec![],
            LaunchSourceMode::HostDefault,
        );
        let context = ProviderLaunchContext::new(spec, PathBuf::from("/home/user"));
        assert_eq!(context.provider(), ProviderKind::Claude);
        assert_eq!(context.executable_path(), "/usr/bin/claude");
        assert!(context.session_handle.is_none());
    }

    #[test]
    fn test_provider_launch_context_with_session() {
        let spec = ResolvedLaunchSpec::new(
            ProviderKind::Claude,
            "/usr/bin/claude".to_string(),
            BTreeMap::new(),
            vec![],
            LaunchSourceMode::HostDefault,
        );
        let handle = SessionHandle::ClaudeSession {
            session_id: "test-session".to_string(),
        };
        let context = ProviderLaunchContext::new(spec, PathBuf::from("/home/user"))
            .with_session_handle(handle.clone());
        assert!(context.session_handle.is_some());
    }

    #[test]
    fn test_provider_launch_context_with_opt_session() {
        let spec = ResolvedLaunchSpec::new(
            ProviderKind::Claude,
            "/usr/bin/claude".to_string(),
            BTreeMap::new(),
            vec![],
            LaunchSourceMode::HostDefault,
        );
        let context = ProviderLaunchContext::new(spec, PathBuf::from("/home/user"))
            .with_opt_session_handle(None);
        assert!(context.session_handle.is_none());
    }

    #[test]
    fn test_provider_launch_context_extra_args() {
        let mut spec = ResolvedLaunchSpec::new(
            ProviderKind::Claude,
            "/usr/bin/claude".to_string(),
            BTreeMap::new(),
            vec!["--flag".to_string()],
            LaunchSourceMode::CommandFragment,
        );
        spec.add_note("test note");
        let context = ProviderLaunchContext::new(spec, PathBuf::from("/home/user"));
        assert_eq!(context.extra_args(), &["--flag"]);
    }
}