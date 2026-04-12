use std::fs;
use std::path::PathBuf;

use anyhow::Context;
use anyhow::Result;
use chrono::Utc;
use serde::Deserialize;
use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EscalationRecord {
    pub task_id: String,
    pub reason: String,
    pub context_summary: String,
    pub recommended_actions: Vec<String>,
    pub created_at: String,
}

pub fn save_escalation(record: &EscalationRecord) -> Result<PathBuf> {
    let data_dir = dirs::data_local_dir().context("local data directory is unavailable")?;
    let dir = data_dir.join("agile-agent").join("escalations");
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
