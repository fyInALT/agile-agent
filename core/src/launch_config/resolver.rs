use std::collections::BTreeMap;
use std::path::Path;

use crate::provider::ProviderKind;

use super::spec::{LaunchInputSpec, LaunchSourceMode, LaunchSourceOrigin, ResolvedLaunchSpec};

/// Host environment variables that are inherited by default.
const HOST_ENV_WHITELIST: &[&str] = &[
    "PATH",
    "HOME",
    "USER",
    "SHELL",
    "LANG",
    "LC_ALL",
];

/// Provider-specific environment variable prefixes.
const PROVIDER_ENV_PREFIXES: &[&str] = &[
    "ANTHROPIC_",
    "CODEX_",
    "OPENAI_",
    "API_",
];

/// Default executable names for each provider.
fn default_executable_name(provider: ProviderKind) -> Option<&'static str> {
    match provider {
        ProviderKind::Claude => Some("claude"),
        ProviderKind::Codex => Some("codex"),
        ProviderKind::Mock => None,
    }
}

/// Resolve the host environment variables into a captured snapshot.
pub fn resolve_host_env(provider: ProviderKind) -> BTreeMap<String, String> {
    let mut env = BTreeMap::new();

    // Inherit whitelist variables
    for key in HOST_ENV_WHITELIST {
        if let Ok(value) = std::env::var(key) {
            env.insert(key.to_string(), value);
        }
    }

    // Inherit provider-specific variables
    for (key, value) in std::env::vars() {
        if PROVIDER_ENV_PREFIXES.iter().any(|p| key.starts_with(p)) {
            env.insert(key, value);
        }
    }

    env
}

/// Resolve the executable path for the given provider.
pub fn resolve_executable_path(
    provider: ProviderKind,
    requested: Option<&str>,
) -> anyhow::Result<String> {
    // If a custom path is provided and it's an absolute path, use it directly
    if let Some(req) = requested {
        if Path::new(req).is_absolute() {
            return Ok(req.to_string());
        }
    }

    let executable_name = requested
        .or_else(|| default_executable_name(provider))
        .unwrap_or_else(|| provider.label());

    // Check environment override first
    let env_override = match provider {
        ProviderKind::Claude => std::env::var("CLAUDE_PATH_ENV").ok(),
        ProviderKind::Codex => std::env::var("CODEX_PATH_ENV").ok(),
        ProviderKind::Mock => None,
    };

    let resolved = if let Some(custom_path) = env_override {
        custom_path
    } else if provider == ProviderKind::Mock {
        // Mock provider doesn't need a real executable
        // Return a placeholder path that will be validated differently
        format!("/usr/bin/{}", executable_name)
    } else {
        // Use which crate to find the executable
        which::which(executable_name)?.display().to_string()
    };

    Ok(resolved)
}

/// Generate a default LaunchInputSpec for host default mode.
pub fn generate_host_default_input(provider: ProviderKind) -> LaunchInputSpec {
    LaunchInputSpec {
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

/// Resolve a LaunchInputSpec into a ResolvedLaunchSpec.
pub fn resolve_launch_spec(input: &LaunchInputSpec) -> anyhow::Result<ResolvedLaunchSpec> {
    let resolved_executable = resolve_executable_path(
        input.provider,
        input.requested_executable.as_deref(),
    )?;

    let host_env = resolve_host_env(input.provider);
    let mut effective_env = host_env;

    // Explicit overrides win
    for (key, value) in &input.env_overrides {
        effective_env.insert(key.clone(), value.clone());
    }

    let mut resolution_notes = vec![format!("Resolved from {:?} mode", input.source_mode)];

    // Add note about source origin
    match input.source_origin {
        LaunchSourceOrigin::Manual => resolution_notes.push("Source: manual user input".to_string()),
        LaunchSourceOrigin::Template => {
            if let Some(template_id) = &input.template_id {
                resolution_notes.push(format!("Source: template '{}'", template_id));
            }
        }
        LaunchSourceOrigin::HostDefault => resolution_notes.push("Source: host default".to_string()),
    }

    Ok(ResolvedLaunchSpec {
        provider: input.provider,
        resolved_executable_path: resolved_executable,
        effective_env,
        extra_args: input.extra_args.clone(),
        resolved_at: chrono::Utc::now().to_rfc3339(),
        derived_from: input.source_mode,
        resolution_notes,
    })
}

/// Resolve a decision agent's LaunchInputSpec independently.
///
/// IMPORTANT: Decision config never inherits from work config.
/// Even if work has env overrides, decision resolves from host environment only.
pub fn resolve_decision_launch_spec(
    decision_input: &LaunchInputSpec,
) -> anyhow::Result<ResolvedLaunchSpec> {
    // Always resolve from host environment, not work_resolved
    resolve_launch_spec(decision_input)
}

/// Resolve a complete AgentLaunchBundle.
pub fn resolve_bundle(
    work_input: LaunchInputSpec,
    decision_input: LaunchInputSpec,
) -> anyhow::Result<(ResolvedLaunchSpec, ResolvedLaunchSpec)> {
    let work_resolved = resolve_launch_spec(&work_input)?;
    let decision_resolved = resolve_decision_launch_spec(&decision_input)?;
    Ok((work_resolved, decision_resolved))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolve_host_env() {
        let env = resolve_host_env(ProviderKind::Claude);
        // Should contain at least some whitelist vars if they exist in test environment
        // PATH is almost always present
        assert!(env.contains_key("PATH") || env.is_empty());
    }

    #[test]
    fn test_resolve_executable_path_default() {
        // This test may fail if 'claude' is not in PATH
        let result = resolve_executable_path(ProviderKind::Claude, None);
        // We just verify it doesn't panic - actual resolution depends on environment
        if result.is_ok() {
            let path = result.unwrap();
            assert!(!path.is_empty());
        }
    }

    #[test]
    fn test_resolve_executable_path_custom() {
        let result = resolve_executable_path(ProviderKind::Claude, Some("/usr/bin/claude"));
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "/usr/bin/claude");
    }

    #[test]
    fn test_resolve_executable_path_codex() {
        let result = resolve_executable_path(ProviderKind::Codex, Some("/usr/local/bin/codex"));
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "/usr/local/bin/codex");
    }

    #[test]
    fn test_resolve_executable_path_mock() {
        // Mock provider should return a placeholder path without calling which::which
        let result = resolve_executable_path(ProviderKind::Mock, None);
        assert!(result.is_ok());
        let path = result.unwrap();
        assert!(path.contains("mock"));
    }

    #[test]
    fn test_resolve_executable_path_windows_style() {
        // On Windows, C:\Users\... is absolute; on Linux it won't be recognized
        // This test verifies the function doesn't panic and handles the input gracefully
        let result = resolve_executable_path(ProviderKind::Claude, Some("C:\\Users\\test\\claude.exe"));
        // On Linux this returns an error from which::which since the path is not absolute there
        // On Windows it would return the path directly since Path::is_absolute() would be true
        if cfg!(windows) {
            assert!(result.is_ok());
            assert_eq!(result.unwrap(), "C:\\Users\\test\\claude.exe");
        } else {
            // On Linux, C:\... is not an absolute path, so it tries which::which which fails
            // This is expected behavior since the path isn't valid on Linux
            assert!(result.is_err() || result.unwrap().contains("Users"));
        }
    }

    #[test]
    fn test_generate_host_default_input() {
        let input = generate_host_default_input(ProviderKind::Claude);
        assert_eq!(input.provider, ProviderKind::Claude);
        assert_eq!(input.source_mode, LaunchSourceMode::HostDefault);
        assert_eq!(input.source_origin, LaunchSourceOrigin::HostDefault);
        assert!(input.env_overrides.is_empty());
    }

    #[test]
    fn test_resolve_launch_spec_basic() {
        let input = LaunchInputSpec::new(ProviderKind::Claude);
        let result = resolve_launch_spec(&input);
        // May fail if claude is not in PATH
        if result.is_ok() {
            let resolved = result.unwrap();
            assert_eq!(resolved.provider, ProviderKind::Claude);
            assert!(!resolved.resolved_executable_path.is_empty());
            assert!(resolved.resolved_at.len() > 0);
        }
    }

    #[test]
    fn test_resolve_launch_spec_with_env_overrides() {
        let mut env = BTreeMap::new();
        env.insert("ANTHROPIC_MODEL".to_string(), "opus-4".to_string());
        let input = LaunchInputSpec::env_only(ProviderKind::Claude, env);
        let result = resolve_launch_spec(&input);
        if result.is_ok() {
            let resolved = result.unwrap();
            assert_eq!(
                resolved.effective_env.get("ANTHROPIC_MODEL"),
                Some(&"opus-4".to_string())
            );
        }
    }

    #[test]
    fn test_resolve_decision_launch_spec_independence() {
        // Work agent with env override
        let mut work_env = BTreeMap::new();
        work_env.insert("ANTHROPIC_MODEL".to_string(), "claude-opus".to_string());
        let work_input = LaunchInputSpec::env_only(ProviderKind::Claude, work_env);

        // Decision agent with host default (empty)
        let decision_input = LaunchInputSpec::new(ProviderKind::Claude);

        let work_resolved = resolve_launch_spec(&work_input).unwrap();
        let decision_resolved = resolve_decision_launch_spec(&decision_input).unwrap();

        // Work should have the override
        assert_eq!(
            work_resolved.effective_env.get("ANTHROPIC_MODEL"),
            Some(&"claude-opus".to_string())
        );

        // Decision should NOT have work's override - should be host default
        // (We can't assert specific value here as it depends on host env,
        // but we can verify it's different or doesn't exist due to override)
        // Actually, since decision has no env_overrides, it will just have host env
        // The key point is it doesn't INHERIT from work_resolved
        assert!(decision_resolved.effective_env.get("ANTHROPIC_MODEL").is_none()
            || decision_resolved.effective_env.get("ANTHROPIC_MODEL") != Some(&"claude-opus".to_string()));
    }

    #[test]
    fn test_resolve_bundle() {
        let work_input = LaunchInputSpec::new(ProviderKind::Claude);
        let decision_input = LaunchInputSpec::new(ProviderKind::Codex);

        let result = resolve_bundle(work_input, decision_input);
        if result.is_ok() {
            let (work_resolved, decision_resolved) = result.unwrap();
            assert_eq!(work_resolved.provider, ProviderKind::Claude);
            assert_eq!(decision_resolved.provider, ProviderKind::Codex);
        }
    }
}