//! Profile Persistence
//!
//! Handles loading and saving profile configurations at both global
//! and workplace levels.

use std::fs;
use std::path::PathBuf;

use anyhow::Context;
use anyhow::Result;
use dirs::home_dir;

use crate::profile::store::ProfileStore;

/// Profile persistence manager
///
/// Handles:
/// - Global profiles: ~/.agile-agent/profiles.json
/// - Workplace profiles: <workplace>/.agile-agent/profiles.json
pub struct ProfilePersistence {
    /// Path to global profiles directory
    global_path: PathBuf,

    /// Optional path to workplace directory
    workplace_path: Option<PathBuf>,
}

impl ProfilePersistence {
    /// Create a new persistence manager for global profiles only
    pub fn new() -> Result<Self> {
        let home = home_dir().context("home directory unavailable")?;
        let global_path = home.join(".agile-agent");
        Ok(Self {
            global_path,
            workplace_path: None,
        })
    }

    /// Create a persistence manager with workplace support
    pub fn with_workplace(workplace_path: PathBuf) -> Result<Self> {
        let home = home_dir().context("home directory unavailable")?;
        let global_path = home.join(".agile-agent");
        Ok(Self {
            global_path,
            workplace_path: Some(workplace_path),
        })
    }

    /// Create with explicit paths (for testing)
    pub fn for_paths(global_path: PathBuf, workplace_path: Option<PathBuf>) -> Self {
        Self {
            global_path,
            workplace_path,
        }
    }

    /// Get the global profiles directory
    pub fn global_dir(&self) -> &PathBuf {
        &self.global_path
    }

    /// Get the global profiles file path
    pub fn global_file_path(&self) -> PathBuf {
        self.global_path.join("profiles.json")
    }

    /// Get the workplace profiles file path (if workplace is set)
    pub fn workplace_file_path(&self) -> Option<PathBuf> {
        self.workplace_path
            .as_ref()
            .map(|p| p.join(".agile-agent").join("profiles.json"))
    }

    /// Ensure global directory exists
    pub fn ensure_global_dir(&self) -> Result<()> {
        fs::create_dir_all(&self.global_path)
            .with_context(|| format!("failed to create directory {}", self.global_path.display()))?;
        Ok(())
    }

    /// Load global profiles
    ///
    /// Creates default profiles if file doesn't exist.
    pub fn load_global(&self) -> Result<ProfileStore> {
        self.ensure_global_dir()?;

        let file_path = self.global_file_path();

        if !file_path.exists() {
            // Create default profiles
            let store = ProfileStore::with_defaults();
            self.save_global(&store)?;
            return Ok(store);
        }

        let content = fs::read_to_string(&file_path)
            .with_context(|| format!("failed to read {}", file_path.display()))?;

        let store: ProfileStore = serde_json::from_str(&content)
            .with_context(|| format!("failed to parse {}", file_path.display()))?;

        Ok(store)
    }

    /// Save global profiles
    pub fn save_global(&self, store: &ProfileStore) -> Result<()> {
        self.ensure_global_dir()?;

        let file_path = self.global_file_path();
        let content = serde_json::to_string_pretty(store)
            .context("failed to serialize profiles")?;

        fs::write(&file_path, content)
            .with_context(|| format!("failed to write {}", file_path.display()))?;

        Ok(())
    }

    /// Load workplace profiles (if workplace is set and file exists)
    pub fn load_workplace(&self) -> Result<Option<ProfileStore>> {
        let workplace_path = match self.workplace_file_path() {
            Some(p) if p.exists() => p,
            _ => return Ok(None),
        };

        let content = fs::read_to_string(&workplace_path)
            .with_context(|| format!("failed to read {}", workplace_path.display()))?;

        let store: ProfileStore = serde_json::from_str(&content)
            .with_context(|| format!("failed to parse {}", workplace_path.display()))?;

        Ok(Some(store))
    }

    /// Save workplace profiles
    pub fn save_workplace(&self, store: &ProfileStore) -> Result<()> {
        let workplace_path = self.workplace_file_path()
            .ok_or_else(|| anyhow::anyhow!("workplace path not set"))?;

        // Ensure workplace directory exists
        if let Some(parent) = workplace_path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("failed to create directory {}", parent.display()))?;
        }

        let content = serde_json::to_string_pretty(store)
            .context("failed to serialize profiles")?;

        fs::write(&workplace_path, content)
            .with_context(|| format!("failed to write {}", workplace_path.display()))?;

        Ok(())
    }

    /// Load merged profiles (workplace overrides global)
    ///
    /// This is the main method to get the effective profile store.
    pub fn load_merged(&self) -> Result<ProfileStore> {
        let global = self.load_global()?;

        if let Some(workplace) = self.load_workplace()? {
            Ok(ProfileStore::merged(&global, &workplace))
        } else {
            Ok(global)
        }
    }

    /// Delete a profile from global store
    pub fn delete_global_profile(&self, profile_id: &str) -> Result<bool> {
        let mut store = self.load_global()?;
        let removed = store.remove_profile(&profile_id.to_string());
        if removed.is_some() {
            self.save_global(&store)?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Add a profile to global store
    pub fn add_global_profile(&self, profile: crate::profile::profile::ProviderProfile) -> Result<()> {
        let mut store = self.load_global()?;
        store.add_profile(profile);
        self.save_global(&store)?;
        Ok(())
    }
}

impl Default for ProfilePersistence {
    fn default() -> Self {
        Self::new().expect("failed to create ProfilePersistence")
    }
}

#[cfg(test)]
mod tests {
    use tempfile::TempDir;

    use crate::profile::profile::ProviderProfile;
    use crate::profile::types::CliBaseType;

    use super::*;

    #[test]
    fn test_persistence_new() {
        let persistence = ProfilePersistence::new();
        assert!(persistence.is_ok());
        let p = persistence.unwrap();
        assert!(p.global_dir().ends_with(".agile-agent"));
        assert!(p.workplace_path.is_none());
    }

    #[test]
    fn test_persistence_with_workplace() {
        let temp = TempDir::new().expect("tempdir");
        let workplace = temp.path().to_path_buf();

        let persistence = ProfilePersistence::with_workplace(workplace.clone());
        assert!(persistence.is_ok());
        let p = persistence.unwrap();
        assert_eq!(p.workplace_path, Some(workplace));
    }

    #[test]
    fn test_persistence_load_global_creates_defaults() {
        let temp = TempDir::new().expect("tempdir");
        let global_path = temp.path().to_path_buf();

        let persistence = ProfilePersistence::for_paths(global_path, None);
        let store = persistence.load_global().expect("load global");

        // Should have default profiles
        assert!(store.has_profile(&"claude-default".to_string()));
        assert!(store.has_profile(&"codex-default".to_string()));
        assert!(store.has_profile(&"mock-default".to_string()));

        // File should have been created
        assert!(persistence.global_file_path().exists());
    }

    #[test]
    fn test_persistence_save_and_load() {
        let temp = TempDir::new().expect("tempdir");
        let global_path = temp.path().to_path_buf();

        let persistence = ProfilePersistence::for_paths(global_path, None);

        // Create custom store
        let mut store = ProfileStore::new();
        let profile = ProviderProfile::new("custom".to_string(), CliBaseType::Claude)
            .with_display_name("Custom Profile".to_string());
        store.add_profile(profile);

        // Save
        persistence.save_global(&store).expect("save");

        // Load
        let loaded = persistence.load_global().expect("load");
        assert!(loaded.has_profile(&"custom".to_string()));
    }

    #[test]
    fn test_persistence_workplace_override() {
        let temp_global = TempDir::new().expect("tempdir global");
        let temp_workplace = TempDir::new().expect("tempdir workplace");

        let global_path = temp_global.path().to_path_buf();
        let workplace_path = temp_workplace.path().to_path_buf();

        let persistence = ProfilePersistence::for_paths(global_path, Some(workplace_path));

        // Create global store with default
        let global_store = ProfileStore::with_defaults();
        persistence.save_global(&global_store).expect("save global");

        // Create workplace store with custom profile
        let mut workplace_store = ProfileStore::new();
        let custom = ProviderProfile::new("workplace-custom".to_string(), CliBaseType::Claude)
            .with_display_name("Workplace Custom".to_string());
        workplace_store.add_profile(custom);
        workplace_store
            .set_default_work_profile("workplace-custom".to_string())
            .expect("set default");
        persistence.save_workplace(&workplace_store).expect("save workplace");

        // Load merged
        let merged = persistence.load_merged().expect("load merged");

        // Should have both profiles
        assert!(merged.has_profile(&"claude-default".to_string())); // from global
        assert!(merged.has_profile(&"workplace-custom".to_string())); // from workplace

        // Workplace default should override
        assert_eq!(merged.default_work_profile_id(), "workplace-custom");
    }

    #[test]
    fn test_persistence_load_merged_no_workplace() {
        let temp = TempDir::new().expect("tempdir");
        let global_path = temp.path().to_path_buf();

        let persistence = ProfilePersistence::for_paths(global_path, None);
        let merged = persistence.load_merged().expect("load merged");

        // Should be just global defaults
        assert!(merged.has_profile(&"claude-default".to_string()));
    }
}