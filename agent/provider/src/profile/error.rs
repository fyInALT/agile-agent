//! Profile Error Types

use crate::profile::ProfileId;

/// Errors that can occur during profile operations
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProfileError {
    /// Profile not found in store
    ProfileNotFound(ProfileId),

    /// Environment variable(s) not found during interpolation
    MissingEnvVars(Vec<String>),

    /// Profile store not loaded
    NoProfileStore,

    /// Invalid profile definition
    InvalidProfile {
        id: ProfileId,
        reason: String,
    },

    /// Profile persistence error
    PersistenceError(String),

    /// Profile ID already exists
    DuplicateProfileId(ProfileId),

    /// Default profile not found
    DefaultNotFound {
        profile_type: String, // "work" or "decision"
        profile_id: ProfileId,
    },

    /// CLI type not supported as ProviderKind
    UnsupportedCliType(String),

    /// Invalid environment variable syntax
    InvalidEnvSyntax {
        key: String,
        value: String,
    },
}

impl std::fmt::Display for ProfileError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ProfileNotFound(id) => write!(f, "Profile '{}' not found", id),
            Self::MissingEnvVars(vars) => {
                write!(f, "Missing environment variables: {}", vars.join(", "))
            }
            Self::NoProfileStore => write!(f, "Profile store not loaded"),
            Self::InvalidProfile { id, reason } => {
                write!(f, "Invalid profile '{}': {}", id, reason)
            }
            Self::PersistenceError(msg) => write!(f, "Profile persistence error: {}", msg),
            Self::DuplicateProfileId(id) => write!(f, "Profile '{}' already exists", id),
            Self::DefaultNotFound { profile_type, profile_id } => {
                write!(
                    f,
                    "Default {} profile '{}' not found",
                    profile_type, profile_id
                )
            }
            Self::UnsupportedCliType(cli) => write!(f, "CLI type '{}' not supported", cli),
            Self::InvalidEnvSyntax { key, value } => {
                write!(f, "Invalid env syntax for '{}': '{}'", key, value)
            }
        }
    }
}

impl std::error::Error for ProfileError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_profile_error_display() {
        assert_eq!(
            ProfileError::ProfileNotFound("test-profile".to_string()).to_string(),
            "Profile 'test-profile' not found"
        );

        assert_eq!(
            ProfileError::MissingEnvVars(vec!["API_KEY".to_string(), "BASE_URL".to_string()])
                .to_string(),
            "Missing environment variables: API_KEY, BASE_URL"
        );

        assert_eq!(ProfileError::NoProfileStore.to_string(), "Profile store not loaded");
    }

    #[test]
    fn test_profile_error_invalid_profile() {
        let err = ProfileError::InvalidProfile {
            id: "bad-profile".to_string(),
            reason: "missing CLI type".to_string(),
        };
        assert_eq!(err.to_string(), "Invalid profile 'bad-profile': missing CLI type");
    }
}