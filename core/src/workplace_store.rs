use std::env;
use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::sync::OnceLock;

use anyhow::Context;
use anyhow::Result;
use chrono::Utc;
use dirs::home_dir;
use serde::Deserialize;
use serde::Serialize;

use crate::agent_runtime::WorkplaceId;
use crate::logging;
use crate::shutdown_snapshot::ShutdownSnapshot;

/// Environment variable to override workplaces root directory
pub const WORKPLACES_ROOT_ENV: &str = "AGILE_AGENT_WORKPLACES_ROOT";

/// Check if running in test environment (cargo test)
fn is_test_environment() -> bool {
    // Check for explicit override - if user set the root, respect it
    if env::var(WORKPLACES_ROOT_ENV).is_ok() {
        return false;
    }

    // Detect cargo test: CARGO_MANIFEST_DIR is set (cargo-managed process)
    // This covers cargo test, cargo run, cargo build, etc.
    // We want to use temp dir for tests, but not for production runs.
    // The key distinction: production runs have a non-temp cwd
    env::var("CARGO_MANIFEST_DIR").is_ok()
}

/// Global temporary workplaces root for test processes
static TEST_WORKPLACES_ROOT: OnceLock<std::path::PathBuf> = OnceLock::new();

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

/// Decision state for persistence
///
/// Stores the decision agent's state for restore on startup.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DecisionState {
    /// Decision agent ID
    pub decision_agent_id: String,

    /// Main agent ID this decision agent is attached to
    pub main_agent_id: String,

    /// Provider type (e.g., "claude", "codex")
    pub provider_type: String,

    /// Decision agent role (e.g., "Reviewer", "Architect")
    pub role: String,

    /// Last decision timestamp
    pub last_decision_at: Option<String>,

    /// Total decisions made
    pub total_decisions: u32,

    /// Created timestamp
    pub created_at: String,

    /// Updated timestamp
    pub updated_at: String,
}

/// Decision history entry for audit trail
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DecisionHistoryEntry {
    /// Decision ID
    pub decision_id: String,

    /// Agent ID that made the decision
    pub agent_id: String,

    /// Decision type (e.g., "human_decision", "auto_approval")
    pub decision_type: String,

    /// Situation type (e.g., "action_required", "question")
    pub situation_type: String,

    /// Outcome (e.g., "approved", "rejected", "custom")
    pub outcome: String,

    /// Timestamp
    pub timestamp: String,

    /// Duration in milliseconds
    pub duration_ms: u64,

    /// Optional notes
    pub notes: Option<String>,
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

    /// Path for decision layer directory
    pub fn decision_dir(&self) -> PathBuf {
        self.path.join("decision")
    }

    /// Path for decision state persistence
    pub fn decision_state_path(&self) -> PathBuf {
        self.decision_dir().join("state.json")
    }

    /// Path for project rules (CLAUDE.md in workplace)
    pub fn project_rules_path(&self) -> PathBuf {
        self.path.join("project_rules.md")
    }

    /// Path for decision history log
    pub fn decision_history_path(&self) -> PathBuf {
        self.decision_dir().join("history.json")
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

    /// Ensure decision directory exists
    pub fn ensure_decision_dir(&self) -> Result<()> {
        fs::create_dir_all(self.decision_dir()).with_context(|| {
            format!(
                "failed to create decision directory {}",
                self.decision_dir().display()
            )
        })?;
        logging::debug_event(
            "workplace.decision.ensure",
            "ensured decision directory",
            serde_json::json!({
                "workplace_id": self.workplace_id.as_str(),
                "decision_dir": self.decision_dir().display().to_string(),
            }),
        );
        Ok(())
    }

    /// Save decision state
    pub fn save_decision_state(&self, state: &DecisionState) -> Result<PathBuf> {
        self.ensure_decision_dir()?;
        let path = self.decision_state_path();
        let payload =
            serde_json::to_string_pretty(state).context("failed to serialize decision state")?;
        fs::write(&path, payload).with_context(|| format!("failed to write {}", path.display()))?;
        logging::debug_event(
            "workplace.decision.save_state",
            "saved decision state",
            serde_json::json!({
                "workplace_id": self.workplace_id.as_str(),
                "path": path.display().to_string(),
            }),
        );
        Ok(path)
    }

    /// Load decision state if exists
    pub fn load_decision_state(&self) -> Result<Option<DecisionState>> {
        let path = self.decision_state_path();
        if !path.exists() {
            return Ok(None);
        }
        let payload = fs::read_to_string(&path)
            .with_context(|| format!("failed to read {}", path.display()))?;
        let state: DecisionState = serde_json::from_str(&payload)
            .with_context(|| format!("failed to parse {}", path.display()))?;
        logging::debug_event(
            "workplace.decision.load_state",
            "loaded decision state",
            serde_json::json!({
                "workplace_id": self.workplace_id.as_str(),
            }),
        );
        Ok(Some(state))
    }

    /// Clear decision state (after restore)
    pub fn clear_decision_state(&self) -> Result<()> {
        let path = self.decision_state_path();
        if path.exists() {
            fs::remove_file(&path)
                .with_context(|| format!("failed to remove {}", path.display()))?;
            logging::debug_event(
                "workplace.decision.clear_state",
                "cleared decision state",
                serde_json::json!({
                    "workplace_id": self.workplace_id.as_str(),
                }),
            );
        }
        Ok(())
    }

    /// Check if decision state exists
    pub fn has_decision_state(&self) -> bool {
        self.decision_state_path().exists()
    }

    /// Save project rules to workplace
    pub fn save_project_rules(&self, rules: &str) -> Result<PathBuf> {
        self.ensure_decision_dir()?;
        let path = self.project_rules_path();
        fs::write(&path, rules)
            .with_context(|| format!("failed to write {}", path.display()))?;
        logging::debug_event(
            "workplace.decision.save_rules",
            "saved project rules",
            serde_json::json!({
                "workplace_id": self.workplace_id.as_str(),
                "path": path.display().to_string(),
            }),
        );
        Ok(path)
    }

    /// Load project rules from workplace (CLAUDE.md copy)
    pub fn load_project_rules(&self) -> Result<Option<String>> {
        // First check workplace-level project rules
        let workplace_rules_path = self.project_rules_path();
        if workplace_rules_path.exists() {
            let rules = fs::read_to_string(&workplace_rules_path)
                .with_context(|| format!("failed to read {}", workplace_rules_path.display()))?;
            return Ok(Some(rules));
        }

        // Fall back to root cwd's CLAUDE.md
        let cwd_claude_md = self.cwd.join("CLAUDE.md");
        if cwd_claude_md.exists() {
            let rules = fs::read_to_string(&cwd_claude_md)
                .with_context(|| format!("failed to read {}", cwd_claude_md.display()))?;
            return Ok(Some(rules));
        }

        Ok(None)
    }

    /// Append decision to history log
    pub fn append_decision_history(&self, entry: &DecisionHistoryEntry) -> Result<()> {
        self.ensure_decision_dir()?;
        let path = self.decision_history_path();

        // Load existing history or create new
        let mut history: Vec<DecisionHistoryEntry> = if path.exists() {
            let payload = fs::read_to_string(&path)
                .with_context(|| format!("failed to read {}", path.display()))?;
            serde_json::from_str(&payload)
                .with_context(|| format!("failed to parse {}", path.display()))?
        } else {
            Vec::new()
        };

        history.push(entry.clone());

        let payload =
            serde_json::to_string_pretty(&history).context("failed to serialize decision history")?;
        fs::write(&path, payload).with_context(|| format!("failed to write {}", path.display()))?;

        logging::debug_event(
            "workplace.decision.append_history",
            "appended decision history entry",
            serde_json::json!({
                "workplace_id": self.workplace_id.as_str(),
                "decision_id": entry.decision_id,
            }),
        );
        Ok(())
    }

    /// Load decision history
    pub fn load_decision_history(&self) -> Result<Vec<DecisionHistoryEntry>> {
        let path = self.decision_history_path();
        if !path.exists() {
            return Ok(Vec::new());
        }
        let payload = fs::read_to_string(&path)
            .with_context(|| format!("failed to read {}", path.display()))?;
        let history: Vec<DecisionHistoryEntry> = serde_json::from_str(&payload)
            .with_context(|| format!("failed to parse {}", path.display()))?;
        Ok(history)
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
    // Check for environment variable override first
    if let Ok(custom_root) = env::var(WORKPLACES_ROOT_ENV) {
        return Ok(PathBuf::from(custom_root));
    }

    // If running in test environment, use a per-process temporary directory
    if is_test_environment() {
        let path = TEST_WORKPLACES_ROOT.get_or_init(|| {
            // Create tempdir and keep its path
            // The TempDir is leaked intentionally to persist for the process lifetime
            let tempdir = tempfile::tempdir().expect("test workplaces root tempdir");
            // Leak the TempDir to prevent cleanup during process lifetime
            let path = tempdir.path().to_path_buf();
            std::mem::forget(tempdir); // Don't drop, keep directory alive
            path
        });
        return Ok(path.clone());
    }

    // Default: use home directory
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
    use super::DecisionState;
    use super::DecisionHistoryEntry;
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
        let root = TempDir::new().expect("root");
        let store = WorkplaceStore::for_root(temp.path(), root.path().to_path_buf()).expect("store");
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
        let root = TempDir::new().expect("root");
        let store = WorkplaceStore::for_root(temp.path(), root.path().to_path_buf()).expect("store");
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
        let root = TempDir::new().expect("root");
        let store = WorkplaceStore::for_root(temp.path(), root.path().to_path_buf()).expect("store");
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

    // Story 8.4: Decision Layer Integration Tests

    #[test]
    fn decision_dir_created() {
        let temp = TempDir::new().expect("tempdir");
        let store = WorkplaceStore::for_cwd(temp.path()).expect("store");
        store.ensure().expect("ensure");

        store.ensure_decision_dir().expect("ensure decision dir");

        assert!(store.decision_dir().exists());
        assert!(store.decision_dir().ends_with("decision"));
    }

    #[test]
    fn decision_state_persisted() {
        let temp = TempDir::new().expect("tempdir");
        let store = WorkplaceStore::for_cwd(temp.path()).expect("store");
        store.ensure().expect("ensure");

        let state = DecisionState {
            decision_agent_id: "decision_001".to_string(),
            main_agent_id: "agent_001".to_string(),
            provider_type: "claude".to_string(),
            role: "Reviewer".to_string(),
            last_decision_at: Some("2026-04-15T10:00:00Z".to_string()),
            total_decisions: 5,
            created_at: "2026-04-15T09:00:00Z".to_string(),
            updated_at: "2026-04-15T10:00:00Z".to_string(),
        };

        store.save_decision_state(&state).expect("save state");

        assert!(store.has_decision_state());

        let loaded = store.load_decision_state().expect("load state").expect("state");
        assert_eq!(loaded.decision_agent_id, "decision_001");
        assert_eq!(loaded.main_agent_id, "agent_001");
        assert_eq!(loaded.total_decisions, 5);
    }

    #[test]
    fn decision_state_restored() {
        let temp = TempDir::new().expect("tempdir");
        let store = WorkplaceStore::for_cwd(temp.path()).expect("store");
        store.ensure().expect("ensure");

        // Save state
        let state = DecisionState {
            decision_agent_id: "decision_002".to_string(),
            main_agent_id: "agent_002".to_string(),
            provider_type: "codex".to_string(),
            role: "Architect".to_string(),
            last_decision_at: None,
            total_decisions: 0,
            created_at: "2026-04-15T09:00:00Z".to_string(),
            updated_at: "2026-04-15T09:00:00Z".to_string(),
        };

        store.save_decision_state(&state).expect("save state");

        // Simulate restore by loading
        let loaded = store.load_decision_state().expect("load state").expect("state");

        // Verify restore works
        assert_eq!(loaded.role, "Architect");
        assert_eq!(loaded.provider_type, "codex");

        // Clear after restore
        store.clear_decision_state().expect("clear state");
        assert!(!store.has_decision_state());
    }

    #[test]
    fn project_rules_loaded() {
        let temp = TempDir::new().expect("tempdir");
        let store = WorkplaceStore::for_cwd(temp.path()).expect("store");
        store.ensure().expect("ensure");

        // Test loading from workplace-level rules
        let rules = "# Project Rules\n\n- Use TDD\n- Write tests\n";
        store.save_project_rules(rules).expect("save rules");

        let loaded = store.load_project_rules().expect("load rules").expect("rules");
        assert!(loaded.contains("Use TDD"));
    }

    #[test]
    fn project_rules_loaded_from_cwd_claude_md() {
        let temp = TempDir::new().expect("tempdir");
        let store = WorkplaceStore::for_cwd(temp.path()).expect("store");
        store.ensure().expect("ensure");

        // Create CLAUDE.md in cwd
        let claude_md_path = temp.path().join("CLAUDE.md");
        let rules = "# CLAUDE.md\n\n- Use TDD-first\n";
        std::fs::write(&claude_md_path, rules).expect("write CLAUDE.md");

        // Should fall back to cwd's CLAUDE.md
        let loaded = store.load_project_rules().expect("load rules").expect("rules");
        assert!(loaded.contains("TDD-first"));
    }

    #[test]
    fn decision_history_append_and_load() {
        let temp = TempDir::new().expect("tempdir");
        let store = WorkplaceStore::for_cwd(temp.path()).expect("store");
        store.ensure().expect("ensure");

        let entry1 = DecisionHistoryEntry {
            decision_id: "dec_001".to_string(),
            agent_id: "agent_001".to_string(),
            decision_type: "human_decision".to_string(),
            situation_type: "action_required".to_string(),
            outcome: "approved".to_string(),
            timestamp: "2026-04-15T10:00:00Z".to_string(),
            duration_ms: 5000,
            notes: Some("Approved quickly".to_string()),
        };

        let entry2 = DecisionHistoryEntry {
            decision_id: "dec_002".to_string(),
            agent_id: "agent_001".to_string(),
            decision_type: "auto_approval".to_string(),
            situation_type: "confirmation".to_string(),
            outcome: "approved".to_string(),
            timestamp: "2026-04-15T10:05:00Z".to_string(),
            duration_ms: 100,
            notes: None,
        };

        store.append_decision_history(&entry1).expect("append 1");
        store.append_decision_history(&entry2).expect("append 2");

        let history = store.load_decision_history().expect("load history");
        assert_eq!(history.len(), 2);
        assert_eq!(history[0].decision_id, "dec_001");
        assert_eq!(history[1].decision_type, "auto_approval");
    }
}
