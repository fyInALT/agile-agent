use std::fs;
use std::path::PathBuf;

use anyhow::Context;
use anyhow::Result;
use chrono::Utc;
use serde::Deserialize;
use serde::Serialize;

use crate::storage;
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EscalationRecord {
    pub task_id: String,
    pub reason: String,
    pub context_summary: String,
    pub recommended_actions: Vec<String>,
    pub created_at: String,
}

pub fn save_escalation(record: &EscalationRecord) -> Result<PathBuf> {
    let dir = storage::app_data_root()?.join("escalations");
    fs::create_dir_all(&dir).context("failed to create escalation directory")?;

    let file_path = dir.join(format!(
        "{}-{}.json",
        record.task_id,
        Utc::now().timestamp()
    ));
    let payload = serde_json::to_string_pretty(record).context("failed to serialize escalation")?;
    fs::write(&file_path, payload).context("failed to write escalation file")?;
    Ok(file_path)
}

pub fn save_escalation_under(root: &Path, record: &EscalationRecord) -> Result<PathBuf> {
    let dir = root.join("artifacts").join("escalations");
    fs::create_dir_all(&dir).context("failed to create escalation directory")?;

    let file_path = dir.join(format!(
        "{}-{}.json",
        record.task_id,
        Utc::now().timestamp()
    ));
    let payload = serde_json::to_string_pretty(record).context("failed to serialize escalation")?;
    fs::write(&file_path, payload).context("failed to write escalation file")?;
    Ok(file_path)
}

#[cfg(test)]
mod tests {
    use super::EscalationRecord;

    #[test]
    fn escalation_record_holds_reason() {
        let record = EscalationRecord {
            task_id: "task-1".to_string(),
            reason: "verification failed".to_string(),
            context_summary: "summary".to_string(),
            recommended_actions: vec!["inspect verification".to_string()],
            created_at: "2026-01-01T00:00:00Z".to_string(),
        };

        assert_eq!(record.reason, "verification failed");
    }
}
