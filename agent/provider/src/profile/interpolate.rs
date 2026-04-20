//! Environment Variable Interpolation

use std::collections::BTreeMap;

use crate::profile::profile::ProviderProfile;

/// Interpolate ${ENV_VAR} references in a value string
///
/// Only supports ${VAR} syntax (not $VAR or ${VAR:-default}).
/// Missing environment variables are left as literal ${VAR} in the output.
pub fn interpolate_env_value(value: &str) -> String {
    // Find all ${VAR} patterns
    let mut result = value.to_string();
    let mut start = 0;

    while start < result.len() {
        // Find ${
        let var_start = result[start..].find("${");
        if var_start.is_none() {
            break;
        }
        let var_start = start + var_start.unwrap();

        // Find closing }
        let var_end_offset = result[var_start + 2..].find("}");
        if var_end_offset.is_none() {
            break;
        }
        let var_end = var_start + 2 + var_end_offset.unwrap();

        // Extract variable name
        let var_name = &result[var_start + 2..var_end];

        // Validate var_name (skip if invalid)
        if var_name.is_empty() {
            start = var_start + 2;
            continue;
        }
        let first_char = var_name.chars().next().unwrap();
        if !first_char.is_ascii_alphabetic() && first_char != '_' {
            start = var_start + 2;
            continue;
        }

        // Get environment variable value
        match std::env::var(var_name) {
            Ok(env_value) => {
                // Replace ${VAR} with the actual value
                let before = &result[..var_start];
                let after = &result[var_end + 1..];
                result = format!("{}{}{}", before, env_value, after);
                // Continue from the same position (env_value might have different length)
                start = var_start + env_value.len();
            }
            Err(_) => {
                // Var not set - leave ${VAR} literal in place, continue after it
                start = var_end + 1;
            }
        }
    }

    result
}

/// Interpolate all env overrides in a profile
///
/// Returns a new BTreeMap with interpolated values.
/// Missing env vars are left as literal ${VAR} in the output.
pub fn interpolate_profile_env(profile: &ProviderProfile) -> BTreeMap<String, String> {
    let mut resolved = BTreeMap::new();
    for (key, value) in &profile.env_overrides {
        let interpolated = interpolate_env_value(value);
        resolved.insert(key.clone(), interpolated);
    }
    resolved
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_interpolate_env_value_plain() {
        // Plain value (no interpolation)
        let result = interpolate_env_value("plain-value");
        assert_eq!(result, "plain-value");
    }

    #[test]
    fn test_interpolate_env_value_with_var() {
        // SAFETY: Test-only code, modifying local process environment
        unsafe {
            std::env::set_var("TEST_INTERPOLATE_VAR", "test-value");
        }

        let result = interpolate_env_value("${TEST_INTERPOLATE_VAR}");
        assert_eq!(result, "test-value");

        // SAFETY: Test-only code, cleaning up local process environment
        unsafe {
            std::env::remove_var("TEST_INTERPOLATE_VAR");
        }
    }

    #[test]
    fn test_interpolate_env_value_multiple_vars() {
        // SAFETY: Test-only code
        unsafe {
            std::env::set_var("TEST_VAR_A", "valueA");
            std::env::set_var("TEST_VAR_B", "valueB");
        }

        let result = interpolate_env_value("prefix-${TEST_VAR_A}-middle-${TEST_VAR_B}-suffix");
        assert_eq!(result, "prefix-valueA-middle-valueB-suffix");

        // SAFETY: Test-only code
        unsafe {
            std::env::remove_var("TEST_VAR_A");
            std::env::remove_var("TEST_VAR_B");
        }
    }

    #[test]
    fn test_interpolate_env_value_missing_var() {
        // Ensure the var doesn't exist
        // SAFETY: Test-only code
        unsafe {
            std::env::remove_var("TEST_MISSING_VAR");
        }

        // Missing var is left as literal ${VAR}
        let result = interpolate_env_value("${TEST_MISSING_VAR}");
        assert_eq!(result, "${TEST_MISSING_VAR}");
    }

    #[test]
    fn test_interpolate_profile_env() {
        // SAFETY: Test-only code
        unsafe {
            std::env::set_var("TEST_API_KEY", "my-key");
            std::env::set_var("TEST_BASE_URL", "https://example.com");
        }

        let profile = ProviderProfile::new("test".to_string(), crate::profile::types::CliBaseType::Claude)
            .with_env("API_KEY".to_string(), "${TEST_API_KEY}".to_string())
            .with_env("BASE_URL".to_string(), "${TEST_BASE_URL}".to_string())
            .with_env("PLAIN".to_string(), "plain-value".to_string());

        let resolved = interpolate_profile_env(&profile);
        assert_eq!(resolved.get("API_KEY"), Some(&"my-key".to_string()));
        assert_eq!(resolved.get("BASE_URL"), Some(&"https://example.com".to_string()));
        assert_eq!(resolved.get("PLAIN"), Some(&"plain-value".to_string()));

        // SAFETY: Test-only code
        unsafe {
            std::env::remove_var("TEST_API_KEY");
            std::env::remove_var("TEST_BASE_URL");
        }
    }

    #[test]
    fn test_interpolate_env_value_missing_var_in_middle() {
        // SAFETY: Test-only code
        unsafe {
            std::env::remove_var("TEST_MISSING_VAR");
        }

        // Missing var in the middle of a string
        let result = interpolate_env_value("prefix-${TEST_MISSING_VAR}-suffix");
        assert_eq!(result, "prefix-${TEST_MISSING_VAR}-suffix");
    }
}