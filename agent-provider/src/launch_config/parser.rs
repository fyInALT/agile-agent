use std::collections::BTreeMap;

use super::error::{ParseError, ParseResult};
use super::spec::{LaunchInputSpec, LaunchSourceMode, LaunchSourceOrigin};
use crate::logging;
use crate::provider::ProviderKind;

/// Maximum line length for parser input.
const MAX_LINE_LENGTH: usize = 4096;
/// Maximum total input length.
const MAX_INPUT_LENGTH: usize = 64 * 1024;
/// Maximum number of environment variables.
const MAX_ENV_VARS: usize = 100;

/// Detect the source mode based on input content.
///
/// Returns `LaunchSourceMode::CommandFragment` if the input contains
/// executable-like tokens (non KEY=VALUE patterns), otherwise `EnvOnly`.
pub fn detect_source_mode(input: &str) -> LaunchSourceMode {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return LaunchSourceMode::HostDefault;
    }

    // Tokenize the input to check at token level (not line level)
    let tokens: Vec<String> = match shlex::split(trimmed) {
        Some(t) => t,
        None => return LaunchSourceMode::CommandFragment, // If can't tokenize, treat as command
    };

    if tokens.is_empty() {
        return LaunchSourceMode::HostDefault;
    }

    // If any token doesn't look like an env var assignment, it's a command fragment
    for token in &tokens {
        // Check if this token is an env var assignment (KEY=VALUE)
        if let Some(eq_pos) = token.find('=') {
            let key = &token[..eq_pos];
            // If the key is valid env var format, this could be env-only
            if !is_valid_key(key) {
                return LaunchSourceMode::CommandFragment;
            }
        } else {
            // No '=' means this is definitely not an env var - it's a command token
            return LaunchSourceMode::CommandFragment;
        }
    }

    LaunchSourceMode::EnvOnly
}

/// Check if a key is valid (non-empty, valid identifier).
fn is_valid_key(key: &str) -> bool {
    if key.is_empty() {
        return false;
    }

    // Key must start with letter or underscore, and contain only
    // alphanumeric characters and underscores
    let mut chars = key.chars();
    match chars.next() {
        Some(c) if c.is_ascii_alphabetic() || c == '_' => {}
        _ => return false,
    }

    chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// Parse environment variable only input (KEY=VALUE per line).
pub fn parse_env_only(provider: ProviderKind, input: &str) -> ParseResult<LaunchInputSpec> {
    let trimmed = input.trim();

    if trimmed.is_empty() {
        return Err(ParseError::EmptyInput);
    }

    if trimmed.len() > MAX_INPUT_LENGTH {
        return Err(ParseError::InputTooLong {
            max: MAX_INPUT_LENGTH,
            actual: trimmed.len(),
        });
    }

    let mut env_overrides = BTreeMap::new();
    let mut line_number = 0;

    for line in trimmed.lines() {
        line_number += 1;
        let line = line.trim();

        // Skip empty lines
        if line.is_empty() {
            continue;
        }

        // Check line length
        if line.len() > MAX_LINE_LENGTH {
            return Err(ParseError::LineTooLong {
                max: MAX_LINE_LENGTH,
                actual: line.len(),
                line: line_number,
            });
        }

        // Parse KEY=VALUE
        let Some(pos) = line.find('=') else {
            // If we find a non-env line in env-only mode, it's an error
            // But we allow lines that don't have '=' as they might be comments
            // Actually, based on design, env-only should only have KEY=VALUE
            // Let's be lenient and skip lines without '='
            continue;
        };

        let key = &line[..pos];
        let value = &line[pos + 1..];

        if key.is_empty() {
            return Err(ParseError::EmptyKey);
        }

        if !is_valid_key(key) {
            return Err(ParseError::InvalidKeyFormat(key.to_string()));
        }

        if env_overrides.len() >= MAX_ENV_VARS {
            return Err(ParseError::TooManyEnvVars {
                max: MAX_ENV_VARS,
                actual: env_overrides.len() + 1,
            });
        }

        env_overrides.insert(key.to_string(), value.to_string());
    }

    if env_overrides.is_empty() {
        return Err(ParseError::EmptyInput);
    }

    Ok(LaunchInputSpec {
        provider,
        source_mode: LaunchSourceMode::EnvOnly,
        source_origin: LaunchSourceOrigin::Manual,
        raw_text: Some(trimmed.to_string()),
        env_overrides,
        requested_executable: None,
        extra_args: Vec::new(),
        template_id: None,
    })
}

/// Parse command fragment input with env prefix, executable, and args.
pub fn parse_command_fragment(provider: ProviderKind, input: &str) -> ParseResult<LaunchInputSpec> {
    let trimmed = input.trim();

    if trimmed.is_empty() {
        return Err(ParseError::EmptyInput);
    }

    if trimmed.len() > MAX_INPUT_LENGTH {
        return Err(ParseError::InputTooLong {
            max: MAX_INPUT_LENGTH,
            actual: trimmed.len(),
        });
    }

    // Use shell-words to tokenize the input
    let tokens: Vec<String> = match shlex::split(trimmed) {
        Some(t) => t,
        None => {
            return Err(ParseError::InvalidKeyFormat(
                "failed to tokenize command fragment".to_string(),
            ));
        }
    };

    if tokens.is_empty() {
        return Err(ParseError::EmptyInput);
    }

    let mut env_overrides = BTreeMap::new();
    let mut executable: Option<String> = None;
    let mut extra_args: Vec<String> = Vec::new();

    let mut tokens_iter = tokens.iter().peekable();

    while let Some(token) = tokens_iter.next() {
        // Check if this token is an env var assignment (KEY=VALUE or KEY="value")
        if let Some(eq_pos) = token.find('=') {
            let key = &token[..eq_pos];
            let value = &token[eq_pos + 1..];

            // Check if key is valid and this looks like an env var
            if is_valid_key(key) && !value.is_empty() {
                env_overrides.insert(key.to_string(), value.to_string());
                continue;
            }
        }

        // This is the executable (first non-env token)
        if executable.is_none() {
            // Reject executable names that start with '=' (malformed env var syntax)
            if token.starts_with('=') {
                return Err(ParseError::InvalidKeyFormat(format!(
                    "invalid token: {}",
                    token
                )));
            }
            executable = Some(token.clone());
        } else {
            extra_args.push(token.clone());
        }
    }

    let requested_executable = executable.ok_or(ParseError::NoExecutableFound)?;

    Ok(LaunchInputSpec {
        provider,
        source_mode: LaunchSourceMode::CommandFragment,
        source_origin: LaunchSourceOrigin::Manual,
        raw_text: Some(trimmed.to_string()),
        env_overrides,
        requested_executable: Some(requested_executable),
        extra_args,
        template_id: None,
    })
}

/// Parse input and auto-detect the appropriate mode.
pub fn parse(provider: ProviderKind, input: &str) -> ParseResult<LaunchInputSpec> {
    let trimmed = input.trim();
    let mode = detect_source_mode(trimmed);

    logging::debug_event(
        "launch_config.parse.start",
        "starting launch config parse",
        serde_json::json!({
            "provider": provider.label(),
            "source_mode_guess": format!("{:?}", mode),
            "input_length": trimmed.len(),
        }),
    );

    let result = match mode {
        LaunchSourceMode::EnvOnly => parse_env_only(provider, input),
        LaunchSourceMode::CommandFragment => parse_command_fragment(provider, input),
        LaunchSourceMode::HostDefault => {
            // For host default, return empty spec
            Ok(LaunchInputSpec::new(provider))
        }
    };

    match &result {
        Ok(spec) => {
            logging::debug_event(
                "launch_config.parse.success",
                "launch config parsed successfully",
                serde_json::json!({
                    "provider": provider.label(),
                    "source_mode": format!("{:?}", spec.source_mode),
                    "executable": spec.requested_executable,
                    "env_count": spec.env_overrides.len(),
                    "arg_count": spec.extra_args.len(),
                }),
            );
        }
        Err(e) => {
            logging::debug_event(
                "launch_config.parse.failed",
                "launch config parse failed",
                serde_json::json!({
                    "provider": provider.label(),
                    "error": e.to_string(),
                }),
            );
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_source_mode_empty() {
        assert_eq!(detect_source_mode(""), LaunchSourceMode::HostDefault);
        assert_eq!(detect_source_mode("   "), LaunchSourceMode::HostDefault);
    }

    #[test]
    fn test_detect_source_mode_env_only() {
        assert_eq!(detect_source_mode("KEY=value"), LaunchSourceMode::EnvOnly);
        assert_eq!(
            detect_source_mode("KEY1=val1\nKEY2=val2"),
            LaunchSourceMode::EnvOnly
        );
        assert_eq!(
            detect_source_mode("ANTHROPIC_MODEL=opus-4"),
            LaunchSourceMode::EnvOnly
        );
    }

    #[test]
    fn test_detect_source_mode_command_fragment() {
        assert_eq!(
            detect_source_mode("claude"),
            LaunchSourceMode::CommandFragment
        );
        assert_eq!(
            detect_source_mode("claude --flag"),
            LaunchSourceMode::CommandFragment
        );
        assert_eq!(
            detect_source_mode("ANTHROPIC_MODEL=X claude"),
            LaunchSourceMode::CommandFragment
        );
    }

    #[test]
    fn test_parse_env_only_basic() {
        let result = parse_env_only(ProviderKind::Claude, "KEY=value");
        assert!(result.is_ok());
        let spec = result.unwrap();
        assert_eq!(spec.source_mode, LaunchSourceMode::EnvOnly);
        assert_eq!(spec.env_overrides.get("KEY"), Some(&"value".to_string()));
    }

    #[test]
    fn test_parse_env_only_multiple() {
        let result = parse_env_only(ProviderKind::Claude, "KEY1=val1\nKEY2=val2");
        assert!(result.is_ok());
        let spec = result.unwrap();
        assert_eq!(spec.env_overrides.len(), 2);
    }

    #[test]
    fn test_parse_env_only_whitespace() {
        let result = parse_env_only(ProviderKind::Claude, "  KEY=value  ");
        assert!(result.is_ok());
        let spec = result.unwrap();
        assert_eq!(spec.env_overrides.get("KEY"), Some(&"value".to_string()));
    }

    #[test]
    fn test_parse_env_only_value_with_spaces() {
        let result = parse_env_only(ProviderKind::Claude, "KEY=value with spaces");
        assert!(result.is_ok());
        let spec = result.unwrap();
        assert_eq!(
            spec.env_overrides.get("KEY"),
            Some(&"value with spaces".to_string())
        );
    }

    #[test]
    fn test_parse_env_only_empty_input() {
        assert!(matches!(
            parse_env_only(ProviderKind::Claude, ""),
            Err(ParseError::EmptyInput)
        ));
    }

    #[test]
    fn test_parse_env_only_empty_key() {
        assert!(matches!(
            parse_env_only(ProviderKind::Claude, "=value"),
            Err(ParseError::EmptyKey)
        ));
    }

    #[test]
    fn test_parse_env_only_invalid_key() {
        assert!(matches!(
            parse_env_only(ProviderKind::Claude, "INVALID KEY=value"),
            Err(ParseError::InvalidKeyFormat(_))
        ));
    }

    #[test]
    fn test_parse_command_fragment_basic() {
        let result = parse_command_fragment(ProviderKind::Claude, "claude");
        assert!(result.is_ok());
        let spec = result.unwrap();
        assert_eq!(spec.source_mode, LaunchSourceMode::CommandFragment);
        assert_eq!(spec.requested_executable, Some("claude".to_string()));
        assert!(spec.extra_args.is_empty());
    }

    #[test]
    fn test_parse_command_fragment_with_env() {
        let result =
            parse_command_fragment(ProviderKind::Claude, "ANTHROPIC_MODEL=X claude --flag");
        assert!(result.is_ok());
        let spec = result.unwrap();
        assert_eq!(spec.requested_executable, Some("claude".to_string()));
        assert_eq!(spec.extra_args, vec!["--flag".to_string()]);
        assert_eq!(
            spec.env_overrides.get("ANTHROPIC_MODEL"),
            Some(&"X".to_string())
        );
    }

    #[test]
    fn test_parse_command_fragment_empty_input() {
        assert!(matches!(
            parse_command_fragment(ProviderKind::Claude, ""),
            Err(ParseError::EmptyInput)
        ));
    }

    #[test]
    fn test_parse_command_fragment_no_executable() {
        // This is tricky - KEY=value alone could be env-only
        // But if we call parse_command_fragment directly, we should fail
        assert!(matches!(
            parse_command_fragment(ProviderKind::Claude, "KEY=value"),
            Err(ParseError::NoExecutableFound)
        ));
    }

    #[test]
    fn test_parse_auto_env() {
        let result = parse(ProviderKind::Claude, "KEY=value");
        assert!(result.is_ok());
        assert_eq!(result.unwrap().source_mode, LaunchSourceMode::EnvOnly);
    }

    #[test]
    fn test_parse_auto_fragment() {
        let result = parse(ProviderKind::Claude, "claude --flag");
        assert!(result.is_ok());
        assert_eq!(
            result.unwrap().source_mode,
            LaunchSourceMode::CommandFragment
        );
    }
}
