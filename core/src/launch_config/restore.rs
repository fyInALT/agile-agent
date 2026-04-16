use std::path::Path;

use crate::launch_config::spec::{AgentLaunchBundle, ResolvedLaunchSpec};
use crate::provider::ProviderKind;

/// Errors that can occur during restore.
#[derive(Debug, Clone)]
pub enum RestoreError {
    /// Executable path does not exist.
    ExecutableNotFound {
        path: String,
        provider: ProviderKind,
    },
    /// No launch bundle available.
    MissingLaunchBundle,
}

impl std::fmt::Display for RestoreError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RestoreError::ExecutableNotFound { path, provider } => {
                write!(f, "executable not found: {} for provider {:?}", path, provider)
            }
            RestoreError::MissingLaunchBundle => {
                write!(f, "missing launch bundle")
            }
        }
    }
}

impl std::error::Error for RestoreError {}

/// Validate that the resolved executable path still exists.
pub fn validate_executable_exists(spec: &ResolvedLaunchSpec) -> Result<(), RestoreError> {
    let path = Path::new(&spec.resolved_executable_path);

    if !path.exists() {
        return Err(RestoreError::ExecutableNotFound {
            path: spec.resolved_executable_path.clone(),
            provider: spec.provider,
        });
    }

    Ok(())
}

/// Validate work agent executable from bundle.
pub fn validate_bundle_executable(bundle: &AgentLaunchBundle) -> Result<(), RestoreError> {
    validate_executable_exists(&bundle.work_resolved)
}

/// Check if restore is needed and validate bundle.
pub fn check_restore_eligibility(bundle: &AgentLaunchBundle) -> Option<RestoreError> {
    if let Err(e) = validate_executable_exists(&bundle.work_resolved) {
        return Some(e);
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    use crate::launch_config::spec::LaunchSourceMode;
    use crate::provider::ProviderKind;

    #[test]
    fn test_validate_executable_exists_success() {
        let spec = ResolvedLaunchSpec::new(
            ProviderKind::Mock,
            "/bin/true".to_string(),
            BTreeMap::new(),
            vec![],
            LaunchSourceMode::HostDefault,
        );
        assert!(validate_executable_exists(&spec).is_ok());
    }

    #[test]
    fn test_validate_executable_exists_failure() {
        let spec = ResolvedLaunchSpec::new(
            ProviderKind::Claude,
            "/nonexistent/path/to/claude".to_string(),
            BTreeMap::new(),
            vec![],
            LaunchSourceMode::HostDefault,
        );
        let result = validate_executable_exists(&spec);
        assert!(result.is_err());
        match result {
            Err(super::RestoreError::ExecutableNotFound { path, provider }) => {
                assert_eq!(provider, ProviderKind::Claude);
                assert!(path.contains("nonexistent"));
            }
            _ => panic!("expected ExecutableNotFound error"),
        }
    }

    #[test]
    fn test_restore_error_display() {
        let error = super::RestoreError::ExecutableNotFound {
            path: "/usr/bin/claude".to_string(),
            provider: ProviderKind::Claude,
        };
        let display = format!("{}", error);
        assert!(display.contains("Claude"));
        assert!(display.contains("usr/bin/claude"));
    }
}