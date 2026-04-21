//! Audit log for daemon actions.
//!
//! Appends structured JSON lines for every security-relevant event.

use chrono::Utc;
use serde::Serialize;
use std::path::Path;
use tokio::fs::OpenOptions;
use tokio::io::AsyncWriteExt;

#[derive(Debug, Clone, Serialize)]
pub struct AuditEntry {
    pub timestamp: String,
    pub event: String,
    pub client_addr: String,
    pub details: serde_json::Value,
}

impl AuditEntry {
    pub fn new(event: &str, client_addr: &str, details: serde_json::Value) -> Self {
        Self {
            timestamp: Utc::now().to_rfc3339(),
            event: event.to_string(),
            client_addr: client_addr.to_string(),
            details,
        }
    }
}

/// Append-only audit log.
#[derive(Debug, Clone)]
pub struct AuditLog {
    path: std::path::PathBuf,
}

impl AuditLog {
    pub fn new(path: &Path) -> Self {
        Self {
            path: path.to_path_buf(),
        }
    }

    pub async fn append(&self, entry: &AuditEntry) -> anyhow::Result<()> {
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)
            .await?;
        let line = serde_json::to_vec(entry)?;
        file.write_all(&line).await?;
        file.write_all(b"\n").await?;
        file.flush().await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn audit_log_appends_line() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("audit.jsonl");
        let log = AuditLog::new(&path);

        let entry = AuditEntry::new(
            "agent.spawn",
            "127.0.0.1:1234",
            serde_json::json!({"agent_id": "a1"}),
        );
        log.append(&entry).await.unwrap();

        let contents = std::fs::read_to_string(&path).unwrap();
        assert!(contents.contains("agent.spawn"));
        assert!(contents.contains("127.0.0.1:1234"));
    }
}
