//! Data Migration for Multi-Agent Runtime
//!
//! Provides migration logic to convert legacy single-agent data
//! to the multi-agent format.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use chrono::Utc;
use serde::{Deserialize, Serialize};

use crate::agent_runtime::{AgentId, WorkplaceId};

/// Migration status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MigrationStatus {
    /// No migration needed
    NotNeeded,
    /// Migration in progress
    InProgress,
    /// Migration completed successfully
    Completed,
    /// Migration failed
    Failed,
    /// Rolled back to original
    RolledBack,
}

impl Default for MigrationStatus {
    fn default() -> Self {
        Self::NotNeeded
    }
}

/// Migration result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MigrationResult {
    /// Status of migration
    pub status: MigrationStatus,
    /// Workplace path that was migrated
    pub workplace_path: PathBuf,
    /// Agent ID created for legacy data
    pub agent_id: Option<AgentId>,
    /// Files migrated
    pub migrated_files: Vec<String>,
    /// Files that failed to migrate
    pub failed_files: Vec<String>,
    /// Timestamp of migration
    pub migrated_at: Option<String>,
    /// Error message if failed
    pub error: Option<String>,
}

impl Default for MigrationResult {
    fn default() -> Self {
        Self {
            status: MigrationStatus::default(),
            workplace_path: PathBuf::new(),
            agent_id: None,
            migrated_files: Vec::new(),
            failed_files: Vec::new(),
            migrated_at: None,
            error: None,
        }
    }
}

impl MigrationResult {
    /// Check if migration succeeded
    pub fn is_success(&self) -> bool {
        self.status == MigrationStatus::Completed
    }

    /// Check if migration failed
    pub fn is_failed(&self) -> bool {
        self.status == MigrationStatus::Failed
    }

    /// Get summary for display
    pub fn summary(&self) -> String {
        match self.status {
            MigrationStatus::NotNeeded => "No migration needed".to_string(),
            MigrationStatus::Completed => format!(
                "Migrated {} files to agent {}",
                self.migrated_files.len(),
                self.agent_id.as_ref().map(|a| a.as_str()).unwrap_or("unknown")
            ),
            MigrationStatus::Failed => format!("Migration failed: {}", self.error.clone().unwrap_or_default()),
            MigrationStatus::RolledBack => "Migration rolled back to original".to_string(),
            MigrationStatus::InProgress => "Migration in progress".to_string(),
        }
    }
}

/// Legacy data detector
#[derive(Debug, Clone)]
pub struct LegacyDetector {
    /// Files that indicate legacy single-agent format
    legacy_files: Vec<String>,
}

impl Default for LegacyDetector {
    fn default() -> Self {
        Self::new()
    }
}

impl LegacyDetector {
    /// Create a new legacy detector
    pub fn new() -> Self {
        Self {
            legacy_files: vec![
                "meta.json".to_string(),
                "state.json".to_string(),
                "transcript.json".to_string(),
            ],
        }
    }

    /// Check if workplace has legacy single-agent data
    pub fn has_legacy_data(&self, workplace_path: &Path) -> bool {
        // Check for legacy files at workplace root
        for file in &self.legacy_files {
            let file_path = workplace_path.join(file);
            if file_path.exists() {
                return true;
            }
        }

        // Also check if agents/ directory doesn't exist
        let agents_dir = workplace_path.join("agents");
        !agents_dir.exists()
    }

    /// Get list of legacy files found
    pub fn found_legacy_files(&self, workplace_path: &Path) -> Vec<PathBuf> {
        self.legacy_files
            .iter()
            .filter_map(|file| {
                let path = workplace_path.join(file);
                if path.exists() {
                    Some(path)
                } else {
                    None
                }
            })
            .collect()
    }

    /// Check if already migrated (has agents/ directory)
    pub fn is_already_migrated(&self, workplace_path: &Path) -> bool {
        workplace_path.join("agents").exists()
    }
}

/// Data migrator for converting legacy to multi-agent format
#[derive(Debug)]
pub struct DataMigrator {
    /// Detector for legacy data
    detector: LegacyDetector,
    /// Agent ID for migrated data
    default_agent_id: AgentId,
}

impl DataMigrator {
    /// Create a new data migrator
    pub fn new() -> Self {
        Self {
            detector: LegacyDetector::new(),
            default_agent_id: AgentId::new("agent_001".to_string()),
        }
    }

    /// Create migrator with custom agent ID
    pub fn with_agent_id(agent_id: AgentId) -> Self {
        Self {
            detector: LegacyDetector::new(),
            default_agent_id: agent_id,
        }
    }

    /// Check if migration is needed
    pub fn needs_migration(&self, workplace_path: &Path) -> bool {
        self.detector.has_legacy_data(workplace_path) && !self.detector.is_already_migrated(workplace_path)
    }

    /// Run migration
    pub fn migrate(&self, workplace_path: &Path) -> MigrationResult {
        if !self.needs_migration(workplace_path) {
            return MigrationResult {
                status: MigrationStatus::NotNeeded,
                workplace_path: workplace_path.to_path_buf(),
                ..Default::default()
            };
        }

        let mut result = MigrationResult {
            status: MigrationStatus::InProgress,
            workplace_path: workplace_path.to_path_buf(),
            agent_id: Some(self.default_agent_id.clone()),
            migrated_at: Some(Utc::now().to_rfc3339()),
            ..Default::default()
        };

        // Create agents directory
        let agents_dir = workplace_path.join("agents");
        let agent_dir = agents_dir.join(self.default_agent_id.as_str());

        if let Err(e) = fs::create_dir_all(&agent_dir) {
            result.status = MigrationStatus::Failed;
            result.error = Some(format!("Failed to create agent directory: {}", e));
            return result;
        }

        // Move legacy files to agent directory
        let legacy_files = self.detector.found_legacy_files(workplace_path);
        for legacy_file in &legacy_files {
            // Safely get file name, skip if invalid
            let file_name = match legacy_file.file_name().and_then(|n| n.to_str()) {
                Some(name) => name,
                None => {
                    result.failed_files.push("unknown".to_string());
                    result.error = Some("Invalid file name in legacy path".to_string());
                    continue;
                }
            };
            let new_path = agent_dir.join(file_name);

            if let Err(e) = self.move_file_with_backup(legacy_file, &new_path) {
                result.failed_files.push(file_name.to_string());
                result.error = Some(format!("Failed to move {}: {}", file_name, e));
                // Rollback on failure
                self.rollback_migration(workplace_path, &result.migrated_files);
                result.status = MigrationStatus::RolledBack;
                return result;
            }
            result.migrated_files.push(file_name.to_string());
        }

        // Create workplace-level meta.json
        if let Err(e) = self.create_workplace_meta(workplace_path) {
            result.status = MigrationStatus::Failed;
            result.error = Some(format!("Failed to create workplace meta: {}", e));
            self.rollback_migration(workplace_path, &result.migrated_files);
            result.status = MigrationStatus::RolledBack;
            return result;
        }

        result.status = MigrationStatus::Completed;
        result
    }

    /// Move file with backup
    fn move_file_with_backup(&self, source: &Path, dest: &Path) -> io::Result<()> {
        // Create backup if destination exists
        if dest.exists() {
            let backup_path = dest.with_extension("json.bak");
            fs::copy(dest, &backup_path)?;
        }

        // Copy file to destination (preserve original for rollback)
        fs::copy(source, dest)?;
        // Remove original after successful copy
        fs::remove_file(source)?;
        Ok(())
    }

    /// Rollback migration
    fn rollback_migration(&self, workplace_path: &Path, migrated_files: &[String]) {
        let agents_dir = workplace_path.join("agents");
        let agent_dir = agents_dir.join(self.default_agent_id.as_str());

        // Move files back from agent_dir to workplace root
        for file in migrated_files {
            let agent_file = agent_dir.join(file);
            let workplace_file = workplace_path.join(file);

            if agent_file.exists() {
                if let Err(e) = fs::copy(&agent_file, &workplace_file) {
                    eprintln!("Rollback failed for {}: {}", file, e);
                }
                // Remove from agent dir
                if let Err(e) = fs::remove_file(&agent_file) {
                    eprintln!("Failed to remove {}: {}", file, e);
                }
            }
        }

        // Remove agents directory if empty
        if agent_dir.exists() {
            if let Err(e) = fs::remove_dir(&agent_dir) {
                eprintln!("Failed to remove agent dir: {}", e);
            }
        }
        if agents_dir.exists() && fs::read_dir(&agents_dir).map(|d| d.count()).unwrap_or(1) == 0 {
            if let Err(e) = fs::remove_dir(&agents_dir) {
                eprintln!("Failed to remove agents dir: {}", e);
            }
        }
    }

    /// Create workplace-level meta.json
    fn create_workplace_meta(&self, workplace_path: &Path) -> io::Result<()> {
        let meta_path = workplace_path.join("workplace_meta.json");

        // Don't overwrite if exists
        if meta_path.exists() {
            return Ok(());
        }

        let meta = WorkplaceMeta {
            workplace_id: WorkplaceId::new(format!("workplace-{}", Utc::now().timestamp_millis())),
            created_at: Utc::now().to_rfc3339(),
            runtime_mode: "multi_agent".to_string(),
            migrated_from: Some("single_agent".to_string()),
            version: 1,
        };

        let json = serde_json::to_string_pretty(&meta)?;
        fs::write(&meta_path, json)?;
        Ok(())
    }

    /// Get migration preview (what would be migrated)
    pub fn preview(&self, workplace_path: &Path) -> MigrationPreview {
        let legacy_files = self.detector.found_legacy_files(workplace_path);

        MigrationPreview {
            needs_migration: self.needs_migration(workplace_path),
            legacy_files: legacy_files.iter().map(|p| p.display().to_string()).collect(),
            target_directory: workplace_path.join("agents").join(self.default_agent_id.as_str()).display().to_string(),
        }
    }
}

impl Default for DataMigrator {
    fn default() -> Self {
        Self::new()
    }
}

/// Workplace metadata for multi-agent format
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkplaceMeta {
    /// Workplace ID
    pub workplace_id: WorkplaceId,
    /// Creation timestamp
    pub created_at: String,
    /// Runtime mode
    pub runtime_mode: String,
    /// Source of migration (if migrated)
    pub migrated_from: Option<String>,
    /// Version number
    pub version: u32,
}

/// Migration preview for display
#[derive(Debug, Clone)]
pub struct MigrationPreview {
    /// Whether migration is needed
    pub needs_migration: bool,
    /// Legacy files found
    pub legacy_files: Vec<String>,
    /// Target directory for migration
    pub target_directory: String,
}

impl MigrationPreview {
    /// Format preview for display
    pub fn format(&self) -> String {
        let mut output = String::new();

        if !self.needs_migration {
            output.push_str("No migration needed\n");
            return output;
        }

        output.push_str("Migration Preview:\n");
        output.push_str("==================\n\n");
        output.push_str("Legacy files to migrate:\n");
        for file in &self.legacy_files {
            output.push_str(&format!("  - {}\n", file));
        }
        output.push_str(&format!("\nTarget: {}\n", self.target_directory));

        output
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn create_legacy_workplace() -> TempDir {
        let temp = TempDir::new().unwrap();

        // Create legacy files
        fs::write(temp.path().join("meta.json"), "{}").unwrap();
        fs::write(temp.path().join("state.json"), "{}").unwrap();
        fs::write(temp.path().join("transcript.json"), "[]").unwrap();

        temp
    }

    fn create_migrated_workplace() -> TempDir {
        let temp = TempDir::new().unwrap();

        // Create agents directory
        let agent_dir = temp.path().join("agents").join("agent_001");
        fs::create_dir_all(&agent_dir).unwrap();

        // Create migrated files
        fs::write(agent_dir.join("meta.json"), "{}").unwrap();
        fs::write(agent_dir.join("state.json"), "{}").unwrap();

        temp
    }

    #[test]
    fn migration_status_default() {
        let status = MigrationStatus::default();
        assert_eq!(status, MigrationStatus::NotNeeded);
    }

    #[test]
    fn migration_result_default() {
        let result = MigrationResult::default();
        assert_eq!(result.status, MigrationStatus::NotNeeded);
        assert!(result.migrated_files.is_empty());
    }

    #[test]
    fn migration_result_is_success() {
        let mut result = MigrationResult::default();
        result.status = MigrationStatus::Completed;
        assert!(result.is_success());
        assert!(!result.is_failed());
    }

    #[test]
    fn migration_result_summary() {
        let result = MigrationResult::default();
        assert!(result.summary().contains("No migration"));
    }

    #[test]
    fn legacy_detector_has_legacy() {
        let temp = create_legacy_workplace();
        let detector = LegacyDetector::new();

        assert!(detector.has_legacy_data(temp.path()));
    }

    #[test]
    fn legacy_detector_no_legacy() {
        let temp = create_migrated_workplace();
        let detector = LegacyDetector::new();

        assert!(!detector.has_legacy_data(temp.path()));
    }

    #[test]
    fn legacy_detector_found_files() {
        let temp = create_legacy_workplace();
        let detector = LegacyDetector::new();

        let files = detector.found_legacy_files(temp.path());
        assert_eq!(files.len(), 3);
    }

    #[test]
    fn legacy_detector_already_migrated() {
        let temp = create_migrated_workplace();
        let detector = LegacyDetector::new();

        assert!(detector.is_already_migrated(temp.path()));
    }

    #[test]
    fn data_migrator_new() {
        let migrator = DataMigrator::new();
        assert!(migrator.needs_migration(&PathBuf::from("/nonexistent")));
    }

    #[test]
    fn data_migrator_needs_migration() {
        let temp = create_legacy_workplace();
        let migrator = DataMigrator::new();

        assert!(migrator.needs_migration(temp.path()));
    }

    #[test]
    fn data_migrator_no_migration_needed() {
        let temp = create_migrated_workplace();
        let migrator = DataMigrator::new();

        assert!(!migrator.needs_migration(temp.path()));
    }

    #[test]
    fn data_migrator_migrate_success() {
        let temp = create_legacy_workplace();
        let migrator = DataMigrator::new();

        let result = migrator.migrate(temp.path());

        assert_eq!(result.status, MigrationStatus::Completed);
        assert_eq!(result.migrated_files.len(), 3);
        assert!(temp.path().join("agents").exists());
        assert!(temp.path().join("agents/agent_001/meta.json").exists());
    }

    #[test]
    fn data_migrator_migrate_already_migrated() {
        let temp = create_migrated_workplace();
        let migrator = DataMigrator::new();

        let result = migrator.migrate(temp.path());

        assert_eq!(result.status, MigrationStatus::NotNeeded);
    }

    #[test]
    fn data_migrator_preview() {
        let temp = create_legacy_workplace();
        let migrator = DataMigrator::new();

        let preview = migrator.preview(temp.path());

        assert!(preview.needs_migration);
        assert_eq!(preview.legacy_files.len(), 3);
    }

    #[test]
    fn migration_preview_format() {
        let preview = MigrationPreview {
            needs_migration: true,
            legacy_files: vec!["meta.json".to_string()],
            target_directory: "/test/agents/agent_001".to_string(),
        };

        let formatted = preview.format();
        assert!(formatted.contains("Migration Preview"));
        assert!(formatted.contains("meta.json"));
    }

    #[test]
    fn workplace_meta_serialization() {
        let meta = WorkplaceMeta {
            workplace_id: WorkplaceId::new("workplace-test".to_string()),
            created_at: "2026-04-15T00:00:00Z".to_string(),
            runtime_mode: "multi_agent".to_string(),
            migrated_from: Some("single_agent".to_string()),
            version: 1,
        };

        let json = serde_json::to_string(&meta).unwrap();
        assert!(json.contains("workplace_id"));
        assert!(json.contains("migrated_from"));
    }

    #[test]
    fn migration_status_serialization() {
        let status = MigrationStatus::Completed;
        let json = serde_json::to_string(&status).unwrap();
        assert_eq!(json, "\"completed\"");
    }
}