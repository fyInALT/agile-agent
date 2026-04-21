//! Lightweight workplace resolution for the daemon.
//!
//! Mirrors the path layout used by `WorkplaceStore` in `agent-core` without
//! pulling in the full core dependency tree.

use agent_types::WorkplaceId;
use anyhow::{Context, Result};
use std::env;
use std::path::{Path, PathBuf};
use tokio::fs;

/// Environment variable to override the workplaces root directory.
pub const WORKPLACES_ROOT_ENV: &str = "AGILE_AGENT_WORKPLACES_ROOT";

/// Resolve the workplace for the current working directory.
pub fn resolve_workplace() -> Result<ResolvedWorkplace> {
    let cwd = env::current_dir().context("get current working directory")?;
    let root = resolve_workplaces_root()?;
    ResolvedWorkplace::for_cwd(&cwd, root)
}

/// Resolved workplace with known directory layout.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedWorkplace {
    workplace_id: WorkplaceId,
    path: PathBuf,
    cwd: PathBuf,
}

impl ResolvedWorkplace {
    /// Resolve a workplace from a directory and a root path.
    pub fn for_cwd(cwd: &Path, root: PathBuf) -> Result<Self> {
        let canonical = cwd.canonicalize().unwrap_or_else(|_| cwd.to_path_buf());
        let id = derive_workplace_id(&canonical);
        let path = root.join(id.as_str());
        Ok(Self {
            workplace_id: id,
            path,
            cwd: canonical,
        })
    }

    pub fn workplace_id(&self) -> &WorkplaceId {
        &self.workplace_id
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn cwd(&self) -> &Path {
        &self.cwd
    }

    /// Path where `daemon.json` lives for this workplace.
    pub fn daemon_json_path(&self) -> PathBuf {
        self.path.join("daemon.json")
    }

    /// Path where `snapshot.json` lives for this workplace.
    pub fn snapshot_path(&self) -> PathBuf {
        self.path.join("snapshot.json")
    }

    /// Ensure the workplace directory exists.
    pub async fn ensure(&self) -> Result<()> {
        fs::create_dir_all(&self.path)
            .await
            .with_context(|| format!("create workplace directory {}", self.path.display()))?;
        Ok(())
    }
}

pub fn resolve_workplaces_root() -> Result<PathBuf> {
    if let Ok(custom) = env::var(WORKPLACES_ROOT_ENV) {
        return Ok(PathBuf::from(custom));
    }

    let data_dir = dirs::data_dir().context("data directory is unavailable")?;
    Ok(data_dir.join("agile-agent").join("workplaces"))
}

fn derive_workplace_id(cwd: &Path) -> WorkplaceId {
    let slug = cwd
        .file_name()
        .and_then(|v| v.to_str())
        .map(slugify)
        .filter(|s| !s.is_empty())
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

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_cwd_produces_same_workplace_id() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tempfile::tempdir().unwrap();
        let a = ResolvedWorkplace::for_cwd(tmp.path(), root.path().to_path_buf()).unwrap();
        let b = ResolvedWorkplace::for_cwd(tmp.path(), root.path().to_path_buf()).unwrap();
        assert_eq!(a.workplace_id, b.workplace_id);
    }

    #[test]
    fn workplace_paths_are_correct() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tempfile::tempdir().unwrap();
        let wp = ResolvedWorkplace::for_cwd(tmp.path(), root.path().to_path_buf()).unwrap();

        assert!(wp.daemon_json_path().ends_with("daemon.json"));
        assert!(wp.snapshot_path().ends_with("snapshot.json"));
    }

    #[tokio::test]
    async fn ensure_creates_directory() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tempfile::tempdir().unwrap();
        let wp = ResolvedWorkplace::for_cwd(tmp.path(), root.path().to_path_buf()).unwrap();

        assert!(!wp.path().exists());
        wp.ensure().await.unwrap();
        assert!(wp.path().exists());
    }
}
