//! Environment Variable Interpolation

use std::collections::BTreeMap;

use crate::provider_profile::error::ProfileError;
use crate::provider_profile::profile::ProviderProfile;

/// Interpolate ${ENV_VAR} references in a value string
///
/// Only supports ${VAR} syntax (not $VAR or ${VAR:-default}).
/// Missing environment variables cause an error.
pub fn interpolate_env_value(value: &str) -> Result<String, ProfileError> {
    // Find all ${VAR} patterns
    let mut result = value.to_string();
    let mut missing_vars = Vec::new();
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

        // Validate var_name
        if var_name.is_empty() {
            return Err(ProfileError::InvalidEnvSyntax {
                key: "unknown".to_string(),
                value: value.to_string(),
            });
        }
        let first_char = var_name.chars().next().unwrap();
        if !first_char.is_ascii_alphabetic() && first_char != '_' {
            return Err(ProfileError::InvalidEnvSyntax {
                key: "unknown".to_string(),
                value: value.to_string(),
            });
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
                missing_vars.push(var_name.to_string());
                start = var_end + 1;
            }
        }
    }

    if missing_vars.is_empty() {
        Ok(result)
    } else {
        Err(ProfileError::MissingEnvVars(missing_vars))
    }
}

/// Interpolate all env overrides in a profile
///
/// Returns a new BTreeMap with interpolated values.
pub fn interpolate_profile_env(
    profile: &ProviderProfile,
) -> Result<BTreeMap<String, String>, ProfileError> {
    let mut resolved = BTreeMap::new();
    for (key, value) in &profile.env_overrides {
        let interpolated = interpolate_env_value(value)?;
        resolved.insert(key.clone(), interpolated);
    }
    Ok(resolved)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_interpolate_env_value_plain() {
        // Plain value (no interpolation)
        let result = interpolate_env_value("plain-value");
        assert_eq!(result.unwrap(), "plain-value");
    }

    #[test]
    fn test_interpolate_env_value_with_var() {
        // SAFETY: Test-only code, modifying local process environment
        unsafe {
            std::env::set_var("TEST_INTERPOLATE_VAR", "test-value");
        }

        let result = interpolate_env_value("${TEST_INTERPOLATE_VAR}");
        assert_eq!(result.unwrap(), "test-value");

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
        assert_eq!(result.unwrap(), "prefix-valueA-middle-valueB-suffix");

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

        let result = interpolate_env_value("${TEST_MISSING_VAR}");
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, ProfileError::MissingEnvVars(_)));
        if let ProfileError::MissingEnvVars(vars) = err {
            assert!(vars.contains(&"TEST_MISSING_VAR".to_string()));
        }
    }

    #[test]
    fn test_interpolate_profile_env() {
        // SAFETY: Test-only code
        unsafe {
            std::env::set_var("TEST_API_KEY", "my-key");
            std::env::set_var("TEST_BASE_URL", "https://example.com");
        }

        let profile = ProviderProfile::new("test".to_string(), crate::provider_profile::types::CliBaseType::Claude)
            .with_env("API_KEY".to_string(), "${TEST_API_KEY}".to_string())
            .with_env("BASE_URL".to_string(), "${TEST_BASE_URL}".to_string())
            .with_env("PLAIN".to_string(), "plain-value".to_string());

        let resolved = interpolate_profile_env(&profile).unwrap();
        assert_eq!(resolved.get("API_KEY"), Some(&"my-key".to_string()));
        assert_eq!(resolved.get("BASE_URL"), Some(&"https://example.com".to_string()));
        assert_eq!(resolved.get("PLAIN"), Some(&"plain-value".to_string()));

        // SAFETY: Test-only code
        unsafe {
            std::env::remove_var("TEST_API_KEY");
            std::env::remove_var("TEST_BASE_URL");
        }
    }
}