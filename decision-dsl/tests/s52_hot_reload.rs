use std::cell::RefCell;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::SystemTime;

use decision_dsl::ast::eval::EvaluatorRegistry;
use decision_dsl::ast::reload::DslReloader;
use decision_dsl::ext::traits::{Fs, FsError};

// ── MockFs ──────────────────────────────────────────────────────────────────

#[derive(Clone)]
struct MockFs {
    inner: Arc<RefCell<MockFsInner>>,
}

struct MockFsInner {
    files: HashMap<PathBuf, (String, SystemTime)>,
    broken: bool,
}

impl MockFs {
    fn new() -> Self {
        Self {
            inner: Arc::new(RefCell::new(MockFsInner {
                files: HashMap::new(),
                broken: false,
            })),
        }
    }

    fn insert(&self, path: impl Into<PathBuf>, content: impl Into<String>) {
        self.inner.borrow_mut().files.insert(
            path.into(),
            (content.into(), SystemTime::now()),
        );
    }

    fn set_broken(&self, broken: bool) {
        self.inner.borrow_mut().broken = broken;
    }

    fn touch(&self, path: impl AsRef<Path>) {
        let mut inner = self.inner.borrow_mut();
        if let Some((_, time)) = inner.files.get_mut(path.as_ref()) {
            *time = SystemTime::now();
        }
    }
}

impl Fs for MockFs {
    fn read_to_string(&self, path: &Path) -> Result<String, FsError> {
        let inner = self.inner.borrow();
        if inner.broken {
            return Err(FsError::Io("broken".into()));
        }
        inner
            .files
            .get(path)
            .map(|(content, _)| content.clone())
            .ok_or_else(|| FsError::NotFound(path.into()))
    }

    fn read_dir(&self, path: &Path) -> Result<Vec<PathBuf>, FsError> {
        let mut entries = Vec::new();
        for (p, _) in self.inner.borrow().files.iter() {
            if let Some(parent) = p.parent() {
                if parent == path {
                    entries.push(p.clone());
                }
            }
        }
        Ok(entries)
    }

    fn modified(&self, path: &Path) -> Result<SystemTime, FsError> {
        let inner = self.inner.borrow();
        // Direct file lookup
        if let Some((_, time)) = inner.files.get(path) {
            return Ok(*time);
        }
        // Directory lookup: return max modified time of children
        let mut max_time = None;
        for (p, (_, time)) in inner.files.iter() {
            if let Some(parent) = p.parent() {
                if parent == path {
                    max_time = Some(match max_time {
                        Some(t) if *time > t => *time,
                        _ => *time,
                    });
                }
            }
        }
        max_time.ok_or_else(|| FsError::NotFound(path.into()))
    }
}

// ── Helpers ─────────────────────────────────────────────────────────────────

fn make_tree_yaml(name: &str) -> String {
    format!(
        r#"apiVersion: "decision.agile-agent.io/v1"
kind: BehaviorTree
metadata:
  name: "{name}"
spec:
  root:
    kind: Condition
    payload:
      name: "c"
      evaluator:
        kind: Script
        payload:
          expression: "provider_output == \"\""
"#,
    )
}

fn setup_bundle_fs(fs: &MockFs, tree_name: &str) {
    let dir = PathBuf::from("/rules");
    fs.insert(dir.join("trees").join(format!("{tree_name}.yaml")), make_tree_yaml(tree_name));
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[test]
fn reloader_initial_parse_succeeds() {
    let fs = MockFs::new();
    setup_bundle_fs(&fs, "test");

    let dir = PathBuf::from("/rules");
    let registry = EvaluatorRegistry::new();
    let reloader = DslReloader::new(dir, Box::new(fs.clone()), registry);
    assert!(reloader.is_ok());
}

#[test]
fn reloader_current_returns_bundle() {
    let fs = MockFs::new();
    setup_bundle_fs(&fs, "test");

    let dir = PathBuf::from("/rules");
    let registry = EvaluatorRegistry::new();
    let reloader = DslReloader::new(dir, Box::new(fs.clone()), registry).unwrap();
    let bundle = reloader.current();
    assert!(bundle.trees.contains_key("test"));
}

#[test]
fn reloader_reload_detects_change() {
    let fs = MockFs::new();
    setup_bundle_fs(&fs, "test");

    let dir = PathBuf::from("/rules");
    let registry = EvaluatorRegistry::new();
    let mut reloader = DslReloader::new(dir.clone(), Box::new(fs.clone()), registry).unwrap();

    // First check_and_reload: baseline is None so it reports changed
    let changed = reloader.check_and_reload().unwrap();
    assert!(changed);

    // Second check_and_reload: no change
    let changed = reloader.check_and_reload().unwrap();
    assert!(!changed);
}

#[test]
fn reloader_reload_failure_keeps_old_bundle() {
    let fs = MockFs::new();
    setup_bundle_fs(&fs, "test");

    let dir = PathBuf::from("/rules");
    let registry = EvaluatorRegistry::new();
    let mut reloader = DslReloader::new(dir.clone(), Box::new(fs.clone()), registry).unwrap();

    // Initial bundle loaded
    assert!(reloader.current().trees.contains_key("test"));

    // First reload establishes baseline
    let _ = reloader.check_and_reload().unwrap();

    // Touch file to trigger change detection, then break fs
    let file_path = dir.join("trees").join("test.yaml");
    fs.touch(&file_path);
    fs.set_broken(true);

    // Reload should fail gracefully and keep old bundle
    let changed = reloader.check_and_reload().unwrap();
    assert!(!changed);
    assert!(reloader.current().trees.contains_key("test"));
}
