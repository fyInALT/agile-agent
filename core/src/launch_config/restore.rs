use std::path::Path;

use chrono::{DateTime, Utc};
use crate::launch_config::spec::{AgentLaunchBundle, ResolvedLaunchSpec};
use crate::logging;
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
                write!(
                    f,
                    "executable not found: {} for provider {:?}",
                    path, provider
                )
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
        logging::debug_event(
            "launch_config.restore.failed",
            "executable validation failed for restore",
            serde_json::json!({
                "agent_id": "unknown",
                "error": e.to_string(),
                "provider": bundle.work_resolved.provider.label(),
            }),
        );
        return Some(e);
    }
    None
}

/// Calculate on_resume flag for Resting state recovery.
///
/// When an agent in Resting state is restored from a snapshot, this function
/// determines whether the agent should immediately attempt recovery (on_resume=true)
/// or continue waiting (on_resume=false).
///
/// Returns true if:
/// - The elapsed time since last retry (or first 429 if never retried) exceeds the interval
/// - This handles both short restarts (keep waiting) and long restarts (try immediately)
pub fn calculate_resting_on_resume(
    started_at: DateTime<Utc>,
    last_retry_at: Option<DateTime<Utc>>,
    interval_secs: u64,
) -> bool {
    let reference_time = last_retry_at.unwrap_or(started_at);
    let elapsed = (Utc::now() - reference_time).num_seconds() as u64;
    elapsed >= interval_secs
}

/// Get the effective retry interval for a bundle.
///
/// Returns the configured interval from the bundle, or the default 1800 seconds (30 min).
pub fn effective_retry_interval(bundle: &AgentLaunchBundle) -> u64 {
    bundle.rate_limit_retry_interval_secs
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

    #[test]
    fn test_calculate_resting_on_resume_elapsed_exceeds_interval() {
        // Started 35 minutes ago, never retried, 30 min interval
        let started_at = Utc::now() - chrono::Duration::minutes(35);
        let on_resume = super::calculate_resting_on_resume(started_at, None, 1800);
        assert!(on_resume);
    }

    #[test]
    fn test_calculate_resting_on_resume_elapsed_within_interval() {
        // Started 5 minutes ago, never retried, 30 min interval
        let started_at = Utc::now() - chrono::Duration::minutes(5);
        let on_resume = super::calculate_resting_on_resume(started_at, None, 1800);
        assert!(!on_resume);
    }

    #[test]
    fn test_calculate_resting_on_resume_with_last_retry_elapsed() {
        // Started 60 minutes ago, last retry 10 minutes ago, 30 min interval
        let started_at = Utc::now() - chrono::Duration::minutes(60);
        let last_retry_at = Some(Utc::now() - chrono::Duration::minutes(10));
        let on_resume = super::calculate_resting_on_resume(started_at, last_retry_at, 1800);
        assert!(!on_resume); // Only 10 min since last retry, need 30
    }

    #[test]
    fn test_calculate_resting_on_resume_with_last_retry_exceeded() {
        // Started 60 minutes ago, last retry 35 minutes ago, 30 min interval
        let started_at = Utc::now() - chrono::Duration::minutes(60);
        let last_retry_at = Some(Utc::now() - chrono::Duration::minutes(35));
        let on_resume = super::calculate_resting_on_resume(started_at, last_retry_at, 1800);
        assert!(on_resume); // 35 min since last retry > 30 min interval
    }

    #[test]
    fn test_effective_retry_interval_default() {
        let input = crate::launch_config::spec::LaunchInputSpec::new(ProviderKind::Claude);
        let resolved = ResolvedLaunchSpec::new(
            ProviderKind::Claude,
            "/usr/bin/claude".to_string(),
            BTreeMap::new(),
            vec![],
            LaunchSourceMode::HostDefault,
        );
        let bundle = AgentLaunchBundle::symmetric(ProviderKind::Claude, input, resolved);
        assert_eq!(super::effective_retry_interval(&bundle), 1800);
    }

    #[test]
    fn test_effective_retry_interval_custom() {
        let input = crate::launch_config::spec::LaunchInputSpec::new(ProviderKind::Claude);
        let resolved = ResolvedLaunchSpec::new(
            ProviderKind::Claude,
            "/usr/bin/claude".to_string(),
            BTreeMap::new(),
            vec![],
            LaunchSourceMode::HostDefault,
        );
        let bundle = AgentLaunchBundle::symmetric(ProviderKind::Claude, input, resolved)
            .with_rate_limit_retry_interval(600);
        assert_eq!(super::effective_retry_interval(&bundle), 600);
    }
}
