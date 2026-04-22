/// Patterns for sensitive environment variables that should be redacted.
pub const SENSITIVE_KEY_PATTERNS: &[&str] = &[
    "*_TOKEN",
    "*_API_KEY",
    "*_AUTH_TOKEN",
    "*_SECRET",
    "*_PASSWORD",
    "ANTHROPIC_API_KEY",
    "CODEX_API_KEY",
    "OPENAI_API_KEY",
    "Authorization",
    "API_KEY",
];

/// Redact a sensitive environment value for display.
///
/// Shows first 8 characters + "..." if value is longer than 8 chars,
/// otherwise shows "***".
pub fn redact_env_value(key: &str, value: &str) -> String {
    if is_sensitive_key(key) {
        if value.len() > 8 {
            format!("{}...", &value[..8])
        } else {
            "***".to_string()
        }
    } else {
        value.to_string()
    }
}

/// Check if a key matches sensitive patterns.
pub fn is_sensitive_key(key: &str) -> bool {
    SENSITIVE_KEY_PATTERNS
        .iter()
        .any(|pattern| matches_pattern(key, pattern))
}

/// Check if a key matches a pattern (supporting * wildcard).
fn matches_pattern(key: &str, pattern: &str) -> bool {
    if let Some(suffix) = pattern.strip_prefix('*') {
        key.ends_with(suffix)
    } else {
        key == pattern
    }
}

/// Redact all sensitive values in an environment map for display.
pub fn redact_env_map(
    env: &std::collections::BTreeMap<String, String>,
) -> std::collections::BTreeMap<String, String> {
    env.iter()
        .map(|(k, v)| (k.clone(), redact_env_value(k, v)))
        .collect()
}

/// Format a launch summary line with redaction.
pub fn format_launch_summary(
    codename: &str,
    provider_label: &str,
    executable: &str,
    env_count: usize,
    decision_source: &str,
) -> String {
    format!(
        "{} [{}]: exec={}, env={} overrides, decision={}",
        codename, provider_label, executable, env_count, decision_source
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_sensitive_key_token() {
        assert!(is_sensitive_key("ANTHROPIC_API_KEY"));
        assert!(is_sensitive_key("CODEX_TOKEN"));
        assert!(is_sensitive_key("MY_SERVICE_TOKEN"));
    }

    #[test]
    fn test_is_sensitive_key_secret() {
        assert!(is_sensitive_key("API_SECRET"));
        assert!(is_sensitive_key("MY_SECRET"));
    }

    #[test]
    fn test_is_sensitive_key_password() {
        // Only patterns ending with _PASSWORD match, not bare PASSWORD
        assert!(is_sensitive_key("MY_PASSWORD"));
        assert!(is_sensitive_key("SERVICE_PASSWORD"));
        assert!(!is_sensitive_key("PASSWORD")); // bare PASSWORD doesn't match *_PASSWORD
    }

    #[test]
    fn test_is_sensitive_key_authorization() {
        assert!(is_sensitive_key("Authorization"));
    }

    #[test]
    fn test_is_not_sensitive_key() {
        assert!(!is_sensitive_key("PATH"));
        assert!(!is_sensitive_key("HOME"));
        assert!(!is_sensitive_key("ANTHROPIC_MODEL"));
    }

    #[test]
    fn test_redact_env_value_long() {
        // "my_secret_key" = 14 chars, first 8 = "my_secre", plus "..." = 11 chars total
        let result = redact_env_value("API_KEY", "my_secret_key_here");
        assert_eq!(result.len(), 11); // first 8 chars + "..."
        assert!(result.starts_with("my_secre"));
        assert!(result.ends_with("..."));
    }

    #[test]
    fn test_redact_env_value_short() {
        let result = redact_env_value("API_KEY", "short");
        assert_eq!(result, "***");
    }

    #[test]
    fn test_redact_env_value_non_sensitive() {
        let result = redact_env_value("PATH", "/usr/bin");
        assert_eq!(result, "/usr/bin");
    }

    #[test]
    fn test_redact_env_map() {
        let mut env = std::collections::BTreeMap::new();
        env.insert("PATH".to_string(), "/usr/bin".to_string());
        env.insert(
            "ANTHROPIC_API_KEY".to_string(),
            "sk-ant-secret-value".to_string(),
        );

        let redacted = redact_env_map(&env);

        assert_eq!(redacted.get("PATH"), Some(&"/usr/bin".to_string()));
        assert_eq!(
            redacted.get("ANTHROPIC_API_KEY"),
            Some(&"sk-ant-s...".to_string())
        );
    }

    #[test]
    fn test_format_launch_summary() {
        let summary =
            format_launch_summary("alpha", "claude", "/usr/bin/claude", 2, "host default");
        assert_eq!(
            summary,
            "alpha [claude]: exec=/usr/bin/claude, env=2 overrides, decision=host default"
        );
    }
}
