use std::path::Path;

use super::error::ValidationError;
use super::error::ValidationResult;
use super::spec::LaunchInputSpec;
use crate::provider::ProviderKind;

/// Reserved arguments for Claude provider.
const RESERVED_ARGS_CLAUDE: &[&str] = &[
    "-p",
    "--bare",
    "--output-format",
    "--input-format",
    "--verbose",
    "--strict-mcp-config",
    "--permission-mode",
    "--resume",
];

/// Reserved arguments for Codex provider.
const RESERVED_ARGS_CODEX: &[&str] = &[
    "exec",
    "--json",
    "--full-auto",
];

/// Validate that the executable name matches the selected provider.
pub fn validate_provider_consistency(
    selected: ProviderKind,
    executable: &str,
) -> ValidationResult {
    // Extract basename from path
    let basename = Path::new(executable)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(executable);

    match (selected, basename) {
        (ProviderKind::Claude, "claude" | "claude.exe") => Ok(()),
        (ProviderKind::Codex, "codex" | "codex.exe") => Ok(()),
        (ProviderKind::Mock, _) => {
            // Mock provider doesn't validate executable names
            Ok(())
        }
        (selected, found) => Err(ValidationError::ProviderMismatch {
            selected,
            found: found.to_string(),
        }),
    }
}

/// Validate that user-supplied args don't conflict with provider-reserved arguments.
pub fn validate_reserved_args(
    args: &[String],
    provider: ProviderKind,
) -> ValidationResult {
    let reserved = match provider {
        ProviderKind::Claude => RESERVED_ARGS_CLAUDE,
        ProviderKind::Codex => RESERVED_ARGS_CODEX,
        ProviderKind::Mock => {
            // Mock provider doesn't have reserved args
            return Ok(());
        }
    };

    for arg in args {
        if reserved.contains(&arg.as_str()) {
            return Err(ValidationError::ReservedArgumentConflict(
                arg.clone(),
                provider,
            ));
        }
    }

    Ok(())
}

/// Validate that the provider supports launch config overrides.
/// Mock provider does not support launch config overrides.
pub fn validate_provider_supports_launch_config(
    provider: ProviderKind,
) -> ValidationResult {
    match provider {
        ProviderKind::Mock => Err(ValidationError::MockProviderNoOverrides),
        ProviderKind::Claude | ProviderKind::Codex => Ok(()),
    }
}

/// Validate a complete LaunchInputSpec.
pub fn validate_launch_input_spec(spec: &LaunchInputSpec) -> ValidationResult {
    // First check if provider supports launch config
    validate_provider_supports_launch_config(spec.provider)?;

    // If there's an executable, validate provider consistency
    if let Some(ref executable) = spec.requested_executable {
        validate_provider_consistency(spec.provider, executable)?;
    }

    // Validate reserved args
    validate_reserved_args(&spec.extra_args, spec.provider)?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    #[test]
    fn test_validate_provider_consistency_claude() {
        assert!(validate_provider_consistency(ProviderKind::Claude, "claude").is_ok());
        assert!(validate_provider_consistency(ProviderKind::Claude, "/usr/bin/claude").is_ok());
        assert!(validate_provider_consistency(ProviderKind::Claude, "claude.exe").is_ok());
    }

    #[test]
    fn test_validate_provider_consistency_codex() {
        assert!(validate_provider_consistency(ProviderKind::Codex, "codex").is_ok());
        assert!(validate_provider_consistency(ProviderKind::Codex, "/usr/local/bin/codex").is_ok());
    }

    #[test]
    fn test_validate_provider_consistency_mismatch() {
        let result = validate_provider_consistency(ProviderKind::Claude, "codex");
        assert!(matches!(
            result,
            Err(ValidationError::ProviderMismatch { .. })
        ));

        let result = validate_provider_consistency(ProviderKind::Codex, "claude");
        assert!(matches!(
            result,
            Err(ValidationError::ProviderMismatch { .. })
        ));
    }

    #[test]
    fn test_validate_provider_consistency_mock() {
        // Mock provider accepts any executable
        assert!(validate_provider_consistency(ProviderKind::Mock, "anything").is_ok());
    }

    #[test]
    fn test_validate_reserved_args_claude() {
        let args = vec!["--flag".to_string()];
        assert!(validate_reserved_args(&args, ProviderKind::Claude).is_ok());
    }

    #[test]
    fn test_validate_reserved_args_claude_reserved() {
        let args = vec!["--resume".to_string()];
        let result = validate_reserved_args(&args, ProviderKind::Claude);
        assert!(matches!(
            result,
            Err(ValidationError::ReservedArgumentConflict(..))
        ));
    }

    #[test]
    fn test_validate_reserved_args_codex() {
        let args = vec!["--json".to_string()];
        let result = validate_reserved_args(&args, ProviderKind::Codex);
        assert!(matches!(
            result,
            Err(ValidationError::ReservedArgumentConflict(..))
        ));
    }

    #[test]
    fn test_validate_reserved_args_mock() {
        // Mock provider has no reserved args
        let args = vec!["anything".to_string()];
        assert!(validate_reserved_args(&args, ProviderKind::Mock).is_ok());
    }

    #[test]
    fn test_validate_provider_supports_launch_config_mock() {
        let result = validate_provider_supports_launch_config(ProviderKind::Mock);
        assert!(matches!(result, Err(ValidationError::MockProviderNoOverrides)));
    }

    #[test]
    fn test_validate_provider_supports_launch_config_claude() {
        assert!(validate_provider_supports_launch_config(ProviderKind::Claude).is_ok());
    }

    #[test]
    fn test_validate_provider_supports_launch_config_codex() {
        assert!(validate_provider_supports_launch_config(ProviderKind::Codex).is_ok());
    }

    #[test]
    fn test_validate_launch_input_spec_valid() {
        let spec = LaunchInputSpec::command_fragment(
            ProviderKind::Claude,
            "claude".to_string(),
            vec!["--flag".to_string()],
            BTreeMap::new(),
        );
        assert!(validate_launch_input_spec(&spec).is_ok());
    }

    #[test]
    fn test_validate_launch_input_spec_mock_rejected() {
        let spec = LaunchInputSpec::command_fragment(
            ProviderKind::Mock,
            "mock".to_string(),
            vec![],
            BTreeMap::new(),
        );
        assert!(matches!(
            validate_launch_input_spec(&spec),
            Err(ValidationError::MockProviderNoOverrides)
        ));
    }

    #[test]
    fn test_validate_launch_input_spec_executable_mismatch() {
        let spec = LaunchInputSpec::command_fragment(
            ProviderKind::Claude,
            "codex".to_string(),
            vec![],
            BTreeMap::new(),
        );
        assert!(matches!(
            validate_launch_input_spec(&spec),
            Err(ValidationError::ProviderMismatch { .. })
        ));
    }

    #[test]
    fn test_validate_launch_input_spec_reserved_arg() {
        let spec = LaunchInputSpec::command_fragment(
            ProviderKind::Claude,
            "claude".to_string(),
            vec!["--resume".to_string()],
            BTreeMap::new(),
        );
        assert!(matches!(
            validate_launch_input_spec(&spec),
            Err(ValidationError::ReservedArgumentConflict(..))
        ));
    }
}