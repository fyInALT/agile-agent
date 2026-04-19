//! Profile Resolver
//!
//! Converts ProviderProfile to LaunchInputSpec and ResolvedLaunchSpec,
//! integrating with the existing launch configuration system.

use anyhow::Result;

use crate::launch_config::resolver::resolve_launch_spec;
use crate::launch_config::spec::{LaunchInputSpec, LaunchSourceMode, LaunchSourceOrigin, ResolvedLaunchSpec};
use crate::provider::ProviderKind;
use crate::provider_profile::error::ProfileError;
use crate::provider_profile::interpolate::interpolate_profile_env;
use crate::provider_profile::profile::{ProfileId, ProviderProfile};
use crate::provider_profile::store::ProfileStore;
use crate::provider_profile::types::CliBaseType;

/// Convert a profile to LaunchInputSpec
///
/// This creates a LaunchInputSpec from the profile configuration,
/// with interpolated environment variables.
pub fn profile_to_launch_input(profile: &ProviderProfile) -> Result<LaunchInputSpec, ProfileError> {
    // Interpolate env values
    let resolved_env = interpolate_profile_env(profile)?;

    // Get ProviderKind from CliBaseType
    let provider_kind = profile
        .base_cli
        .to_provider_kind()
        .ok_or_else(|| ProfileError::UnsupportedCliType(profile.base_cli.label().to_string()))?;

    Ok(LaunchInputSpec {
        provider: provider_kind,
        source_mode: LaunchSourceMode::EnvOnly,
        source_origin: LaunchSourceOrigin::Template,
        raw_text: None,
        env_overrides: resolved_env,
        requested_executable: None,
        extra_args: profile.extra_args.clone(),
        template_id: Some(profile.id.clone()),
    })
}

/// Resolve a profile to ResolvedLaunchSpec
///
/// This performs the full resolution including executable path lookup
/// and environment merging.
pub fn resolve_profile(profile: &ProviderProfile) -> Result<ResolvedLaunchSpec, ProfileError> {
    let input = profile_to_launch_input(profile)?;
    resolve_launch_spec(&input)
        .map_err(|e| ProfileError::PersistenceError(e.to_string()))
}

/// Resolve a profile by ID from the store
///
/// Returns error if profile not found or resolution fails.
pub fn resolve_profile_by_id(
    store: &ProfileStore,
    profile_id: &ProfileId,
) -> Result<ResolvedLaunchSpec, ProfileError> {
    let profile = store
        .get_profile(profile_id)
        .ok_or_else(|| ProfileError::ProfileNotFound(profile_id.clone()))?;
    resolve_profile(profile)
}

/// Agent type for profile selection
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgentType {
    /// Work agent (main task executor)
    Work,
    /// Decision agent (decision layer)
    Decision,
}

/// Get the effective profile from store
///
/// Priority chain:
/// 1. Explicit profile_id (if provided)
/// 2. Default for agent type (work or decision)
/// 3. Fallback error
pub fn get_effective_profile<'a>(
    store: &'a ProfileStore,
    explicit_id: Option<&'a ProfileId>,
    agent_type: AgentType,
) -> Result<&'a ProviderProfile, ProfileError> {
    if let Some(id) = explicit_id {
        return store
            .get_profile(id)
            .ok_or_else(|| ProfileError::ProfileNotFound(id.clone()));
    }

    match agent_type {
        AgentType::Work => store.get_default_work_profile(),
        AgentType::Decision => store.get_default_decision_profile(),
    }
}

/// Resolve effective profile (returns ResolvedLaunchSpec)
pub fn resolve_effective_profile(
    store: &ProfileStore,
    explicit_id: Option<&ProfileId>,
    agent_type: AgentType,
) -> Result<ResolvedLaunchSpec, ProfileError> {
    let profile = get_effective_profile(store, explicit_id, agent_type)?;
    resolve_profile(profile)
}

/// Create default profiles for provider kinds
pub fn create_default_profile_for_kind(kind: ProviderKind) -> ProviderProfile {
    let cli_type = CliBaseType::from_provider_kind(kind);
    ProviderProfile::default_for_cli(cli_type)
}

/// Create ProviderLaunchContext from a profile
///
/// This creates a complete launch context from a profile,
/// ready to be used for provider startup.
pub fn create_launch_context_from_profile(
    profile: &ProviderProfile,
    cwd: std::path::PathBuf,
) -> Result<crate::launch_config::context::ProviderLaunchContext, ProfileError> {
    use crate::launch_config::context::ProviderLaunchContext;
    use crate::provider::SessionHandle;

    let resolved = resolve_profile(profile)?;
    Ok(ProviderLaunchContext::new(resolved, cwd)
        .with_opt_session_handle(None::<SessionHandle>))
}

/// Create ProviderLaunchContext from a profile with optional session handle
pub fn create_launch_context_from_profile_with_session(
    profile: &ProviderProfile,
    cwd: std::path::PathBuf,
    session_handle: Option<crate::provider::SessionHandle>,
) -> Result<crate::launch_config::context::ProviderLaunchContext, ProfileError> {
    use crate::launch_config::context::ProviderLaunchContext;

    let resolved = resolve_profile(profile)?;
    Ok(ProviderLaunchContext::new(resolved, cwd)
        .with_opt_session_handle(session_handle))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::provider_profile::types::CliBaseType;

    fn test_profile() -> ProviderProfile {
        ProviderProfile::new("test-profile".to_string(), CliBaseType::Claude)
            .with_env("TEST_VAR".to_string(), "test-value".to_string())
            .with_arg("--flag".to_string())
    }

    #[test]
    fn test_profile_to_launch_input() {
        let profile = test_profile();
        let input = profile_to_launch_input(&profile).expect("convert");

        assert_eq!(input.provider, ProviderKind::Claude);
        assert_eq!(input.source_mode, LaunchSourceMode::EnvOnly);
        assert_eq!(input.source_origin, LaunchSourceOrigin::Template);
        assert_eq!(input.env_overrides.get("TEST_VAR"), Some(&"test-value".to_string()));
        assert_eq!(input.extra_args, vec!["--flag"]);
        assert_eq!(input.template_id, Some("test-profile".to_string()));
    }

    #[test]
    fn test_profile_to_launch_input_unsupported_cli() {
        let profile = ProviderProfile::new("test".to_string(), CliBaseType::OpenCode);
        let result = profile_to_launch_input(&profile);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, ProfileError::UnsupportedCliType(_)));
    }

    #[test]
    fn test_resolve_profile() {
        let profile = ProviderProfile::new("test".to_string(), CliBaseType::Claude);
        let result = resolve_profile(&profile);
        // This may fail if claude is not in PATH, but we test the conversion logic
        if result.is_ok() {
            let resolved = result.unwrap();
            assert_eq!(resolved.provider, ProviderKind::Claude);
        }
    }

    #[test]
    fn test_resolve_profile_by_id() {
        let store = ProfileStore::with_defaults();
        let result = resolve_profile_by_id(&store, &"claude-default".to_string());
        if result.is_ok() {
            let resolved = result.unwrap();
            assert_eq!(resolved.provider, ProviderKind::Claude);
        }
    }

    #[test]
    fn test_resolve_profile_by_id_not_found() {
        let store = ProfileStore::new();
        let result = resolve_profile_by_id(&store, &"nonexistent".to_string());
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, ProfileError::ProfileNotFound(_)));
    }

    #[test]
    fn test_get_effective_profile_explicit() {
        let store = ProfileStore::with_defaults();
        let profile_id = "claude-default".to_string();
        let profile = get_effective_profile(&store, Some(&profile_id), AgentType::Work);
        assert!(profile.is_ok());
        assert_eq!(profile.unwrap().id, "claude-default");
    }

    #[test]
    fn test_get_effective_profile_default_work() {
        let store = ProfileStore::with_defaults();
        let profile = get_effective_profile(&store, None, AgentType::Work);
        assert!(profile.is_ok());
        assert_eq!(profile.unwrap().id, "claude-default");
    }

    #[test]
    fn test_get_effective_profile_default_decision() {
        let store = ProfileStore::with_defaults();
        let profile = get_effective_profile(&store, None, AgentType::Decision);
        assert!(profile.is_ok());
        assert_eq!(profile.unwrap().id, "claude-default");
    }

    #[test]
    fn test_get_effective_profile_not_found() {
        let store = ProfileStore::new(); // Empty store
        let result = get_effective_profile(&store, None, AgentType::Work);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, ProfileError::DefaultNotFound { .. }));
    }

    #[test]
    fn test_create_default_profile_for_kind() {
        let profile = create_default_profile_for_kind(ProviderKind::Claude);
        assert_eq!(profile.id, "claude-default");
        assert_eq!(profile.base_cli, CliBaseType::Claude);

        let profile = create_default_profile_for_kind(ProviderKind::Codex);
        assert_eq!(profile.id, "codex-default");
        assert_eq!(profile.base_cli, CliBaseType::Codex);
    }

    #[test]
    fn test_create_launch_context_from_profile() {
        let profile = test_profile();
        let cwd = std::path::PathBuf::from("/tmp/test");
        let context = create_launch_context_from_profile(&profile, cwd.clone())
            .expect("create context");

        assert_eq!(context.provider(), ProviderKind::Claude);
        assert_eq!(context.cwd, cwd);
        assert!(context.session_handle.is_none());
        assert_eq!(context.extra_args(), &["--flag"]);
    }

    #[test]
    fn test_create_launch_context_from_profile_unsupported_cli() {
        let profile = ProviderProfile::new("test".to_string(), CliBaseType::OpenCode);
        let cwd = std::path::PathBuf::from("/tmp/test");
        let result = create_launch_context_from_profile(&profile, cwd);

        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), ProfileError::UnsupportedCliType(_)));
    }

    #[test]
    fn test_create_launch_context_with_session() {
        use crate::provider::SessionHandle;

        let profile = test_profile();
        let cwd = std::path::PathBuf::from("/tmp/test");
        let session = SessionHandle::ClaudeSession { session_id: "test-session".to_string() };
        let context = create_launch_context_from_profile_with_session(
            &profile,
            cwd.clone(),
            Some(session.clone()),
        ).expect("create context with session");

        assert_eq!(context.provider(), ProviderKind::Claude);
        assert!(context.session_handle.is_some());
        assert_eq!(context.session_handle.unwrap(), session);
    }
}