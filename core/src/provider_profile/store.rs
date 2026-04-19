//! Profile Store

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::provider_profile::error::ProfileError;
use crate::provider_profile::profile::{ProfileId, ProviderProfile};
use crate::provider_profile::types::CliBaseType;

fn default_work_profile() -> ProfileId {
    "claude-default".to_string()
}

fn default_decision_profile() -> ProfileId {
    "claude-default".to_string()
}

/// Provider profile store
///
/// Manages a collection of profiles with default profile settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfileStore {
    /// All defined profiles
    #[serde(default)]
    profiles: BTreeMap<ProfileId, ProviderProfile>,

    /// Default profile for work agents
    #[serde(default = "default_work_profile")]
    default_work_profile: ProfileId,

    /// Default profile for decision layer
    #[serde(default = "default_decision_profile")]
    default_decision_profile: ProfileId,
}

impl Default for ProfileStore {
    fn default() -> Self {
        Self::new()
    }
}

impl ProfileStore {
    /// Create an empty profile store
    pub fn new() -> Self {
        Self {
            profiles: BTreeMap::new(),
            default_work_profile: default_work_profile(),
            default_decision_profile: default_decision_profile(),
        }
    }

    /// Create store with default profiles for each CLI type
    pub fn with_defaults() -> Self {
        Self::with_detected_clis()
    }

    /// Create store with auto-detected available CLI tools
    ///
    /// Scans PATH for available CLI executables and creates default
    /// profiles only for tools that are actually installed.
    /// Always includes Mock profile for testing.
    /// Sets the first discovered real CLI as default.
    pub fn with_detected_clis() -> Self {
        let mut store = Self::new();

        // Always add Mock for testing
        store.add_profile(ProviderProfile::default_for_cli(CliBaseType::Mock));

        // Detect available CLI tools
        let mut first_available: Option<CliBaseType> = None;
        for cli_type in CliBaseType::detectable() {
            if cli_type.is_available() {
                if first_available.is_none() {
                    first_available = Some(cli_type);
                }
                store.add_profile(ProviderProfile::default_for_cli(cli_type));
            }
        }

        // Set default to first available CLI (or Mock if none found)
        if let Some(cli) = first_available {
            let default_id = format!("{}-default", cli.label());
            store.default_work_profile = default_id.clone();
            store.default_decision_profile = default_id;
        } else {
            // Fallback to Mock if no CLI tools found
            store.default_work_profile = "mock-default".to_string();
            store.default_decision_profile = "mock-default".to_string();
        }

        store
    }

    /// Get list of available CLI types (detected in PATH)
    pub fn available_clis() -> Vec<CliBaseType> {
        CliBaseType::detectable()
            .iter()
            .filter(|c| c.is_available())
            .copied()
            .collect()
    }

    /// Add a profile to the store
    pub fn add_profile(&mut self, profile: ProviderProfile) {
        self.profiles.insert(profile.id.clone(), profile);
    }

    /// Remove a profile from the store
    pub fn remove_profile(&mut self, id: &ProfileId) -> Option<ProviderProfile> {
        self.profiles.remove(id)
    }

    /// Get a profile by ID
    pub fn get_profile(&self, id: &ProfileId) -> Option<&ProviderProfile> {
        self.profiles.get(id)
    }

    /// Check if a profile exists
    pub fn has_profile(&self, id: &ProfileId) -> bool {
        self.profiles.contains_key(id)
    }

    /// List all profiles
    pub fn list_profiles(&self) -> Vec<&ProviderProfile> {
        self.profiles.values().collect()
    }

    /// Get all profile IDs
    pub fn profile_ids(&self) -> Vec<&ProfileId> {
        self.profiles.keys().collect()
    }

    /// Get number of profiles
    pub fn profile_count(&self) -> usize {
        self.profiles.len()
    }

    /// Set the default work profile
    pub fn set_default_work_profile(&mut self, id: ProfileId) -> Result<(), ProfileError> {
        if self.profiles.contains_key(&id) {
            self.default_work_profile = id;
            Ok(())
        } else {
            Err(ProfileError::ProfileNotFound(id))
        }
    }

    /// Set the default decision profile
    pub fn set_default_decision_profile(&mut self, id: ProfileId) -> Result<(), ProfileError> {
        if self.profiles.contains_key(&id) {
            self.default_decision_profile = id;
            Ok(())
        } else {
            Err(ProfileError::ProfileNotFound(id))
        }
    }

    /// Get the default work profile ID
    pub fn default_work_profile_id(&self) -> &ProfileId {
        &self.default_work_profile
    }

    /// Get the default decision profile ID
    pub fn default_decision_profile_id(&self) -> &ProfileId {
        &self.default_decision_profile
    }

    /// Get the default work profile (returns error if not found)
    pub fn get_default_work_profile(&self) -> Result<&ProviderProfile, ProfileError> {
        self.profiles
            .get(&self.default_work_profile)
            .ok_or_else(|| ProfileError::DefaultNotFound {
                profile_type: "work".to_string(),
                profile_id: self.default_work_profile.clone(),
            })
    }

    /// Get the default decision profile (returns error if not found)
    pub fn get_default_decision_profile(&self) -> Result<&ProviderProfile, ProfileError> {
        self.profiles
            .get(&self.default_decision_profile)
            .ok_or_else(|| ProfileError::DefaultNotFound {
                profile_type: "decision".to_string(),
                profile_id: self.default_decision_profile.clone(),
            })
    }

    /// Merge another store into this one (other overrides this)
    pub fn merge(&mut self, other: &ProfileStore) {
        // Other profiles override this
        for (id, profile) in &other.profiles {
            self.profiles.insert(id.clone(), profile.clone());
        }

        // Other defaults override if profiles exist
        if other.profiles.contains_key(&other.default_work_profile) {
            self.default_work_profile = other.default_work_profile.clone();
        }
        if other.profiles.contains_key(&other.default_decision_profile) {
            self.default_decision_profile = other.default_decision_profile.clone();
        }
    }

    /// Create a merged store (workplace overrides global)
    pub fn merged(global: &ProfileStore, workplace: &ProfileStore) -> Self {
        let mut result = global.clone();
        result.merge(workplace);
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_profile_store_new() {
        let store = ProfileStore::new();
        assert_eq!(store.profile_count(), 0);
        assert_eq!(store.default_work_profile_id(), "claude-default");
        assert_eq!(store.default_decision_profile_id(), "claude-default");
    }

    #[test]
    fn test_profile_store_with_defaults() {
        let store = ProfileStore::with_defaults();
        // Mock is always included
        assert!(store.has_profile(&"mock-default".to_string()));
        // At least one profile (Mock)
        assert!(store.profile_count() >= 1);
        // Default should be set to something valid
        assert!(store.has_profile(&store.default_work_profile_id().clone()));
        assert!(store.has_profile(&store.default_decision_profile_id().clone()));
    }

    #[test]
    fn test_profile_store_with_detected_clis_always_has_mock() {
        let store = ProfileStore::with_detected_clis();
        assert!(store.has_profile(&"mock-default".to_string()));
    }

    #[test]
    fn test_profile_store_default_matches_available_profile() {
        let store = ProfileStore::with_detected_clis();
        // Default profile IDs should exist in store
        assert!(store.has_profile(&store.default_work_profile_id().clone()));
        assert!(store.has_profile(&store.default_decision_profile_id().clone()));
    }

    #[test]
    fn test_available_clis_detects_correctly() {
        let available = ProfileStore::available_clis();
        // Result depends on what's installed on the system
        // Just check it doesn't crash and returns a vec
        for cli in &available {
            assert!(cli.is_available());
        }
    }

    #[test]
    fn test_profile_store_add_remove() {
        let mut store = ProfileStore::new();
        let profile = ProviderProfile::new("test".to_string(), CliBaseType::Claude);
        store.add_profile(profile);

        assert_eq!(store.profile_count(), 1);
        assert!(store.has_profile(&"test".to_string()));

        let removed = store.remove_profile(&"test".to_string());
        assert!(removed.is_some());
        assert_eq!(store.profile_count(), 0);
    }

    #[test]
    fn test_profile_store_set_default() {
        let mut store = ProfileStore::with_defaults();

        // Add a custom profile
        let custom = ProviderProfile::new("custom".to_string(), CliBaseType::Claude);
        store.add_profile(custom);

        // Set as default
        let result = store.set_default_work_profile("custom".to_string());
        assert!(result.is_ok());
        assert_eq!(store.default_work_profile_id(), "custom");

        // Try to set non-existent as default
        let result = store.set_default_work_profile("nonexistent".to_string());
        assert!(result.is_err());
    }

    #[test]
    fn test_profile_store_get_default() {
        let store = ProfileStore::with_defaults();

        let work_default = store.get_default_work_profile();
        assert!(work_default.is_ok());
        // Default profile should exist in store
        let profile = work_default.unwrap();
        assert!(store.has_profile(&profile.id.clone()));

        let decision_default = store.get_default_decision_profile();
        assert!(decision_default.is_ok());
    }

    #[test]
    fn test_profile_store_merge() {
        let mut global = ProfileStore::with_defaults();
        let mut workplace = ProfileStore::new();

        // Workplace has custom profile
        let custom = ProviderProfile::new("custom-work".to_string(), CliBaseType::Claude)
            .with_display_name("Custom Work Profile".to_string());
        workplace.add_profile(custom);
        workplace.set_default_work_profile("custom-work".to_string()).unwrap();

        // Merge
        global.merge(&workplace);

        // Should have both profiles
        assert!(global.has_profile(&"claude-default".to_string()));
        assert!(global.has_profile(&"custom-work".to_string()));

        // Workplace default should override
        assert_eq!(global.default_work_profile_id(), "custom-work");
    }

    #[test]
    fn test_profile_store_serde() {
        let store = ProfileStore::with_defaults();
        let json = serde_json::to_string(&store).unwrap();
        let parsed: ProfileStore = serde_json::from_str(&json).unwrap();

        assert_eq!(store.profile_count(), parsed.profile_count());
        assert_eq!(store.default_work_profile_id(), parsed.default_work_profile_id());
    }
}