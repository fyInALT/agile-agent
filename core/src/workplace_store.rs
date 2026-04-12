use std::fs;
use std::path::Path;
use std::path::PathBuf;

use anyhow::Context;
use anyhow::Result;
use dirs::home_dir;

use crate::agent_runtime::WorkplaceId;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkplaceStore {
    workplace_id: WorkplaceId,
    path: PathBuf,
}

impl WorkplaceStore {
    pub fn from_existing(workplace_id: WorkplaceId, path: PathBuf) -> Self {
        Self { workplace_id, path }
    }

    pub fn for_cwd(cwd: &Path) -> Result<Self> {
        let canonical_cwd = cwd.canonicalize().unwrap_or_else(|_| cwd.to_path_buf());
        let workplace_id = derive_workplace_id(&canonical_cwd);
        let root = workplaces_root()?;
        let path = root.join(workplace_id.as_str());
        Ok(Self { workplace_id, path })
    }

    pub fn ensure(&self) -> Result<()> {
        fs::create_dir_all(self.agents_dir()).with_context(|| {
            format!(
                "failed to create workplace directory {}",
                self.path.display()
            )
        })
    }

    pub fn workplace_id(&self) -> &WorkplaceId {
        &self.workplace_id
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn agents_dir(&self) -> PathBuf {
        self.path.join("agents")
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
    use super::WorkplaceStore;
    use super::slugify;
    use tempfile::TempDir;

    #[test]
    fn same_cwd_produces_same_workplace_id() {
        let temp = TempDir::new().expect("tempdir");
        let a = WorkplaceStore::for_cwd(temp.path()).expect("store");
        let b = WorkplaceStore::for_cwd(temp.path()).expect("store");

        assert_eq!(a.workplace_id(), b.workplace_id());
    }

    #[test]
    fn ensure_creates_agents_dir() {
        let temp = TempDir::new().expect("tempdir");
        let nested = temp.path().join("workspace");
        std::fs::create_dir_all(&nested).expect("create cwd");
        let store = WorkplaceStore::for_cwd(&nested).expect("store");

        store.ensure().expect("ensure");

        assert!(store.agents_dir().ends_with("agents"));
    }

    #[test]
    fn slugify_normalizes_symbols() {
        assert_eq!(slugify("My Project!"), "my-project");
        assert_eq!(slugify("agile_agent"), "agile-agent");
    }
}
