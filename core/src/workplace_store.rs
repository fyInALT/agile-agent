use std::fs;
use std::path::Path;
use std::path::PathBuf;

use anyhow::Context;
use anyhow::Result;
use chrono::Utc;
use dirs::home_dir;
use serde::Deserialize;
use serde::Serialize;

use crate::agent_runtime::WorkplaceId;
use crate::logging;
use crate::shutdown_snapshot::ShutdownSnapshot;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkplaceStore {
    workplace_id: WorkplaceId,
    path: PathBuf,
    cwd: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkplaceMeta {
    pub workplace_id: WorkplaceId,
    pub root_cwd: String,
    pub created_at: String,
    pub updated_at: String,
    /// List of agent IDs in this workplace (for multi-agent discovery)
    pub agent_ids: Vec<String>,
}

impl WorkplaceStore {
    pub fn from_existing(workplace_id: WorkplaceId, path: PathBuf, cwd: PathBuf) -> Self {
        Self {
            workplace_id,
            path,
            cwd,
        }
    }

    pub fn for_cwd(cwd: &Path) -> Result<Self> {
        let root = workplaces_root()?;
        Self::for_root(cwd, root)
    }

    pub fn for_root(cwd: &Path, root: PathBuf) -> Result<Self> {
        let canonical_cwd = cwd.canonicalize().unwrap_or_else(|_| cwd.to_path_buf());
        let workplace_id = derive_workplace_id(&canonical_cwd);
        let path = root.join(workplace_id.as_str());
        logging::debug_event(
            "workplace.resolve",
            "resolved workplace from cwd",
            serde_json::json!({
                "cwd": canonical_cwd.display().to_string(),
                "workplace_id": workplace_id.as_str(),
                "path": path.display().to_string(),
            }),
        );
        Ok(Self {
            workplace_id,
            path,
            cwd: canonical_cwd,
        })
    }

    pub fn ensure(&self) -> Result<()> {
        fs::create_dir_all(self.agents_dir()).with_context(|| {
            format!(
                "failed to create workplace directory {}",
                self.path.display()
            )
        })?;
        logging::debug_event(
            "workplace.ensure",
            "ensured workplace directories",
            serde_json::json!({
                "workplace_id": self.workplace_id.as_str(),
                "path": self.path.display().to_string(),
                "agents_dir": self.agents_dir().display().to_string(),
            }),
        );
        self.ensure_meta()?;
        Ok(())
    }

    pub fn workplace_id(&self) -> &WorkplaceId {
        &self.workplace_id
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn root_cwd(&self) -> &Path {
        &self.cwd
    }

    pub fn agents_dir(&self) -> PathBuf {
        self.path.join("agents")
    }

    pub fn meta_path(&self) -> PathBuf {
        self.path.join("meta.json")
    }

    /// Path for shutdown snapshot
    pub fn shutdown_snapshot_path(&self) -> PathBuf {
        self.path.join("shutdown_snapshot.json")
    }

    pub fn kanban_dir(&self) -> PathBuf {
        self.path.join("kanban")
    }

    pub fn kanban_elements_dir(&self) -> PathBuf {
        self.kanban_dir().join("elements")
    }

    pub fn load_meta(&self) -> Result<WorkplaceMeta> {
        let path = self.meta_path();
        let payload = fs::read_to_string(&path)
            .with_context(|| format!("failed to read {}", path.display()))?;
        let meta = serde_json::from_str(&payload)
            .with_context(|| format!("failed to parse {}", path.display()))?;
        logging::debug_event(
            "workplace.meta.load",
            "loaded workplace metadata",
            serde_json::json!({
                "workplace_id": self.workplace_id.as_str(),
                "path": path.display().to_string(),
            }),
        );
        Ok(meta)
    }

    pub fn save_meta(&self, meta: &WorkplaceMeta) -> Result<PathBuf> {
        fs::create_dir_all(&self.path)
            .with_context(|| format!("failed to create {}", self.path.display()))?;
        let path = self.meta_path();
        let payload =
            serde_json::to_string_pretty(meta).context("failed to serialize workplace meta")?;
        fs::write(&path, payload).with_context(|| format!("failed to write {}", path.display()))?;
        logging::debug_event(
            "workplace.meta.save",
            "saved workplace metadata",
            serde_json::json!({
                "workplace_id": meta.workplace_id.as_str(),
                "path": path.display().to_string(),
            }),
        );
        Ok(path)
    }

    pub fn touch_meta(&self) -> Result<()> {
        let mut meta = if self.meta_path().exists() {
            self.load_meta()?
        } else {
            self.default_meta()
        };
        meta.updated_at = Utc::now().to_rfc3339();
        self.save_meta(&meta)?;
        logging::debug_event(
            "workplace.meta.touch",
            "touched workplace metadata",
            serde_json::json!({
                "workplace_id": self.workplace_id.as_str(),
                "path": self.meta_path().display().to_string(),
            }),
        );
        Ok(())
    }

    /// Register an agent ID in the workplace meta
    pub fn register_agent(&self, agent_id: &str) -> Result<()> {
        let mut meta = if self.meta_path().exists() {
            self.load_meta()?
        } else {
            self.default_meta()
        };
        if !meta.agent_ids.contains(&agent_id.to_string()) {
            meta.agent_ids.push(agent_id.to_string());
            meta.updated_at = Utc::now().to_rfc3339();
            self.save_meta(&meta)?;
            logging::debug_event(
                "workplace.meta.register_agent",
                "registered agent in workplace metadata",
                serde_json::json!({
                    "workplace_id": self.workplace_id.as_str(),
                    "agent_id": agent_id,
                }),
            );
        }
        Ok(())
    }

    /// Unregister an agent ID from the workplace meta
    pub fn unregister_agent(&self, agent_id: &str) -> Result<()> {
        if !self.meta_path().exists() {
            return Ok(());
        }
        let mut meta = self.load_meta()?;
        meta.agent_ids.retain(|id| id != agent_id);
        meta.updated_at = Utc::now().to_rfc3339();
        self.save_meta(&meta)?;
        logging::debug_event(
            "workplace.meta.unregister_agent",
            "unregistered agent from workplace metadata",
            serde_json::json!({
                "workplace_id": self.workplace_id.as_str(),
                "agent_id": agent_id,
            }),
        );
        Ok(())
    }

    /// Get list of agent IDs from workplace meta
    pub fn agent_ids(&self) -> Result<Vec<String>> {
        if !self.meta_path().exists() {
            return Ok(Vec::new());
        }
        let meta = self.load_meta()?;
        Ok(meta.agent_ids)
    }

    /// Save shutdown snapshot
    pub fn save_shutdown_snapshot(&self, snapshot: &ShutdownSnapshot) -> Result<PathBuf> {
        let path = self.shutdown_snapshot_path();
        let payload = serde_json::to_string_pretty(snapshot)
            .context("failed to serialize shutdown snapshot")?;
        fs::write(&path, payload).with_context(|| format!("failed to write {}", path.display()))?;
        logging::debug_event(
            "workplace.shutdown.save",
            "saved shutdown snapshot",
            serde_json::json!({
                "workplace_id": self.workplace_id.as_str(),
                "reason": snapshot.shutdown_reason,
                "agents_count": snapshot.agents.len(),
                "path": path.display().to_string(),
            }),
        );
        Ok(path)
    }

    /// Load shutdown snapshot if exists
    pub fn load_shutdown_snapshot(&self) -> Result<Option<ShutdownSnapshot>> {
        let path = self.shutdown_snapshot_path();
        if !path.exists() {
            return Ok(None);
        }
        let payload = fs::read_to_string(&path)
            .with_context(|| format!("failed to read {}", path.display()))?;
        let snapshot: ShutdownSnapshot = serde_json::from_str(&payload)
            .with_context(|| format!("failed to parse {}", path.display()))?;
        logging::debug_event(
            "workplace.shutdown.load",
            "loaded shutdown snapshot",
            serde_json::json!({
                "workplace_id": self.workplace_id.as_str(),
                "reason": snapshot.shutdown_reason,
                "agents_count": snapshot.agents.len(),
            }),
        );
        Ok(Some(snapshot))
    }

    /// Clear shutdown snapshot (after successful restore)
    pub fn clear_shutdown_snapshot(&self) -> Result<()> {
        let path = self.shutdown_snapshot_path();
        if path.exists() {
            fs::remove_file(&path)
                .with_context(|| format!("failed to remove {}", path.display()))?;
            logging::debug_event(
                "workplace.shutdown.clear",
                "cleared shutdown snapshot",
                serde_json::json!({
                    "workplace_id": self.workplace_id.as_str(),
                }),
            );
        }
        Ok(())
    }

    /// Check if shutdown snapshot exists
    pub fn has_shutdown_snapshot(&self) -> bool {
        self.shutdown_snapshot_path().exists()
    }

    fn ensure_meta(&self) -> Result<()> {
        if !self.meta_path().exists() {
            self.save_meta(&self.default_meta())?;
        }
        Ok(())
    }

    fn default_meta(&self) -> WorkplaceMeta {
        let now = Utc::now().to_rfc3339();
        WorkplaceMeta {
            workplace_id: self.workplace_id.clone(),
            root_cwd: self.cwd.display().to_string(),
            created_at: now.clone(),
            updated_at: now,
            agent_ids: Vec::new(),
        }
    }
}

pub fn workplaces_root() -> Result<PathBuf> {
    let home = home_dir().context("home directory is unavailable")?;
    Ok(home.join(".agile-agent").join("workplaces"))
}

fn derive_workplace_id(cwd: &Path) -> WorkplaceId {
    let slug = cwd
        .file_name()
        .and_then(|value| value.to_str())
        .map(slugify)
        .filter(|slug| !slug.is_empty())
        .unwrap_or_else(|| "root".to_string());
    let hash = stable_hash_hex(cwd.display().to_string().as_bytes());
    WorkplaceId::new(format!("wp_{}_{}", slug, &hash[..10]))
}

fn slugify(value: &str) -> String {
    let mut slug = String::new();
    let mut last_was_sep = false;
    for ch in value.chars() {
        let normalized = if ch.is_ascii_alphanumeric() {
            Some(ch.to_ascii_lowercase())
        } else if ch == '-' || ch == '_' || ch == '.' {
            Some('-')
        } else {
            None
        };

        match normalized {
            Some('-') => {
                if !last_was_sep && !slug.is_empty() {
                    slug.push('-');
                    last_was_sep = true;
                }
            }
            Some(ch) => {
                slug.push(ch);
                last_was_sep = false;
            }
            None => {
                if !last_was_sep && !slug.is_empty() {
                    slug.push('-');
                    last_was_sep = true;
                }
            }
        }
    }
    slug.trim_matches('-').to_string()
}

fn stable_hash_hex(bytes: &[u8]) -> String {
    let mut hash: u64 = 0xcbf29ce484222325;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("{hash:016x}")
}

#[cfg(test)]
mod tests {
    use super::WorkplaceMeta;
    use super::WorkplaceStore;
    use super::slugify;
    use crate::logging;
    use crate::logging::RunMode;
    use tempfile::TempDir;

    #[test]
    fn same_cwd_produces_same_workplace_id() {
        let temp = TempDir::new().expect("tempdir");
        let root = TempDir::new().expect("root");
        let a = WorkplaceStore::for_root(temp.path(), root.path().to_path_buf()).expect("store");
        let b = WorkplaceStore::for_root(temp.path(), root.path().to_path_buf()).expect("store");

        assert_eq!(a.workplace_id(), b.workplace_id());
    }

    #[test]
    fn ensure_creates_agents_dir() {
        let temp = TempDir::new().expect("tempdir");
        let root = TempDir::new().expect("root");
        let nested = temp.path().join("workspace");
        std::fs::create_dir_all(&nested).expect("create cwd");
        let store = WorkplaceStore::for_root(&nested, root.path().to_path_buf()).expect("store");

        store.ensure().expect("ensure");

        assert!(store.agents_dir().ends_with("agents"));
        assert!(store.meta_path().exists());
    }

    #[test]
    fn ensure_logs_workplace_resolution_and_meta_write() {
        let _guard = logging::test_guard();
        let temp = TempDir::new().expect("tempdir");
        let store = WorkplaceStore::for_cwd(temp.path()).expect("store");
        logging::init_for_workplace(&store, RunMode::RunLoop).expect("init logger");

        store.ensure().expect("ensure");

        let log_path = logging::current_log_path().expect("log path");
        let contents = std::fs::read_to_string(log_path).expect("log file");
        assert!(contents.contains("\"event\":\"workplace.ensure\""));
        assert!(contents.contains("\"event\":\"workplace.meta.save\""));
    }

    #[test]
    fn slugify_normalizes_symbols() {
        assert_eq!(slugify("My Project!"), "my-project");
        assert_eq!(slugify("agile_agent"), "agile-agent");
    }

    #[test]
    fn saves_and_loads_workplace_meta() {
        let temp = TempDir::new().expect("tempdir");
        let root = TempDir::new().expect("root");
        let store =
            WorkplaceStore::for_root(temp.path(), root.path().to_path_buf()).expect("store");
        store.ensure().expect("ensure");
        let meta = WorkplaceMeta {
            workplace_id: store.workplace_id().clone(),
            root_cwd: store.root_cwd().display().to_string(),
            created_at: "2026-04-12T00:00:00Z".to_string(),
            updated_at: "2026-04-12T00:10:00Z".to_string(),
            agent_ids: vec!["agent_001".to_string()],
        };

        store.save_meta(&meta).expect("save meta");
        let loaded = store.load_meta().expect("load meta");

        assert_eq!(loaded.workplace_id, *store.workplace_id());
        assert_eq!(loaded.root_cwd, store.root_cwd().display().to_string());
        assert_eq!(loaded.agent_ids, vec!["agent_001"]);
    }

    #[test]
    fn register_agent_adds_to_list() {
        let temp = TempDir::new().expect("tempdir");
        let store = WorkplaceStore::for_cwd(temp.path()).expect("store");
        store.ensure().expect("ensure");

        store.register_agent("agent_001").expect("register");
        let ids = store.agent_ids().expect("agent ids");

        assert_eq!(ids, vec!["agent_001"]);

        // Duplicate registration should not add again
        store.register_agent("agent_001").expect("register again");
        let ids = store.agent_ids().expect("agent ids");

        assert_eq!(ids.len(), 1);
    }

    #[test]
    fn unregister_agent_removes_from_list() {
        let temp = TempDir::new().expect("tempdir");
        let store = WorkplaceStore::for_cwd(temp.path()).expect("store");
        store.ensure().expect("ensure");

        store.register_agent("agent_001").expect("register 1");
        store.register_agent("agent_002").expect("register 2");

        store.unregister_agent("agent_001").expect("unregister");
        let ids = store.agent_ids().expect("agent ids");

        assert_eq!(ids, vec!["agent_002"]);
    }

    #[test]
    fn kanban_dirs_are_correct() {
        let temp = TempDir::new().expect("tempdir");
        let root = TempDir::new().expect("root");
        let store =
            WorkplaceStore::for_root(temp.path(), root.path().to_path_buf()).expect("store");

        assert!(store.kanban_dir().ends_with("kanban"));
        assert!(store.kanban_elements_dir().ends_with("kanban/elements"));
    }
}
