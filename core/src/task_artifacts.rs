use std::fs;
use std::path::Path;
use std::path::PathBuf;

use anyhow::Context;
use anyhow::Result;
use chrono::Utc;
use serde::Deserialize;
use serde::Serialize;

use crate::provider::ProviderKind;
use crate::storage;
use crate::verification::VerificationResult;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TaskArtifactOutcome {
    Completed,
    Failed,
    Escalated,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskArtifact {
    pub saved_at: String,
    pub task_id: String,
    pub todo_id: String,
    pub objective: String,
    pub provider: ProviderKind,
    pub outcome: TaskArtifactOutcome,
    pub assistant_summary: Option<String>,
    pub verification: Option<VerificationResult>,
    pub reason: Option<String>,
    pub escalation_path: Option<String>,
}

pub fn save_task_artifact(record: &TaskArtifact) -> Result<PathBuf> {
    let root = storage::app_data_root()?;
    save_task_artifact_to_root(record, &root.join("task-artifacts"))
}

pub fn save_task_artifact_under(root: &Path, record: &TaskArtifact) -> Result<PathBuf> {
    save_task_artifact_to_root(record, &root.join("artifacts").join("task-artifacts"))
}

fn save_task_artifact_to_root(record: &TaskArtifact, root: &Path) -> Result<PathBuf> {
    fs::create_dir_all(root).context("failed to create task artifact directory")?;
    let file_path = root.join(format!(
        "{}-{}.json",
        record.task_id,
        Utc::now().timestamp_millis()
    ));
    let payload =
        serde_json::to_string_pretty(record).context("failed to serialize task artifact")?;
    fs::write(&file_path, payload).context("failed to write task artifact file")?;
    Ok(file_path)
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::TempDir;

    use super::TaskArtifact;
    use super::TaskArtifactOutcome;
    use super::save_task_artifact_to_root;
    use crate::provider::ProviderKind;
    use crate::verification::VerificationOutcome;
    use crate::verification::VerificationResult;

    #[test]
    fn saves_structured_task_artifact() {
        let temp = TempDir::new().expect("tempdir");
        let artifact = TaskArtifact {
            saved_at: "2026-01-01T00:00:00Z".to_string(),
            task_id: "task-1".to_string(),
            todo_id: "todo-1".to_string(),
            objective: "write tests".to_string(),
            provider: ProviderKind::Mock,
            outcome: TaskArtifactOutcome::Completed,
            assistant_summary: Some("done".to_string()),
            verification: Some(VerificationResult {
                outcome: VerificationOutcome::Passed,
                checks: Vec::new(),
                failed_checks: Vec::new(),
                evidence: vec!["cargo_check=pass".to_string()],
                summary: "verification passed".to_string(),
            }),
            reason: None,
            escalation_path: None,
        };

        let path = save_task_artifact_to_root(&artifact, temp.path()).expect("save artifact");
        let payload = fs::read_to_string(path).expect("read artifact");

        assert!(payload.contains("\"task_id\": \"task-1\""));
        assert!(payload.contains("\"outcome\": \"Completed\""));
        assert!(payload.contains("\"verification\""));
    }
}
