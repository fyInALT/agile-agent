use std::fs;
use std::fs::File;
use std::io::BufWriter;
use std::io::Write;
use std::path::PathBuf;
use std::process;
use std::sync::Mutex;
use std::sync::OnceLock;
use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering;

use anyhow::Context;
use anyhow::Result;
use serde::Serialize;
use serde_json::json;

use crate::workplace_store::WorkplaceStore;

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum RunMode {
    Tui,
    ResumeLast,
    RunLoop,
    Doctor,
    Probe,
    AgentCurrent,
    AgentList,
    WorkplaceCurrent,
}

#[derive(Debug, Clone)]
pub struct InitializedLogger {
    pub log_path: PathBuf,
    pub run_id: String,
}

#[derive(Debug)]
struct LoggerState {
    run_id: String,
    run_mode: RunMode,
    workplace_id: String,
    workplace_path: String,
    log_path: PathBuf,
    writer: BufWriter<File>,
}

static LOGGER: OnceLock<Mutex<Option<LoggerState>>> = OnceLock::new();
static LOG_SEQUENCE: AtomicU64 = AtomicU64::new(1);

impl RunMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Tui => "tui",
            Self::ResumeLast => "resume-last",
            Self::RunLoop => "run-loop",
            Self::Doctor => "doctor",
            Self::Probe => "probe",
            Self::AgentCurrent => "agent-current",
            Self::AgentList => "agent-list",
            Self::WorkplaceCurrent => "workplace-current",
        }
    }
}

pub fn init_for_workplace(workplace: &WorkplaceStore, run_mode: RunMode) -> Result<InitializedLogger> {
    let logs_dir = workplace.path().join("logs");
    fs::create_dir_all(&logs_dir)
        .with_context(|| format!("failed to create {}", logs_dir.display()))?;

    let timestamp = chrono::Utc::now().format("%Y-%m-%dT%H-%M-%S%.3fZ").to_string();
    let sequence = LOG_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let run_id = format!("run-{timestamp}-{}-{sequence}", process::id());
    let log_path = logs_dir.join(format!(
        "{timestamp}-{}-pid{}-{sequence}.jsonl",
        run_mode.as_str(),
        process::id()
    ));
    let file = File::create(&log_path)
        .with_context(|| format!("failed to create {}", log_path.display()))?;

    let latest_path = logs_dir.join("latest-path.txt");
    fs::write(&latest_path, log_path.display().to_string())
        .with_context(|| format!("failed to update {}", latest_path.display()))?;

    let state = LoggerState {
        run_id: run_id.clone(),
        run_mode,
        workplace_id: workplace.workplace_id().as_str().to_string(),
        workplace_path: workplace.path().display().to_string(),
        log_path: log_path.clone(),
        writer: BufWriter::new(file),
    };

    let slot = LOGGER.get_or_init(|| Mutex::new(None));
    *slot.lock().expect("logger lock poisoned") = Some(state);

    debug_event("logging.initialized", "logger initialized", json!({}));

    Ok(InitializedLogger { log_path, run_id })
}

pub fn debug_event(event: &str, message: &str, fields: serde_json::Value) {
    write_event("debug", event, message, fields);
}

pub fn warn_event(event: &str, message: &str, fields: serde_json::Value) {
    write_event("warn", event, message, fields);
}

pub fn error_event(event: &str, message: &str, fields: serde_json::Value) {
    write_event("error", event, message, fields);
}

pub fn current_log_path() -> Option<PathBuf> {
    let slot = LOGGER.get()?;
    let guard = slot.lock().ok()?;
    guard.as_ref().map(|state| state.log_path.clone())
}

fn write_event(level: &str, event: &str, message: &str, fields: serde_json::Value) {
    let Some(slot) = LOGGER.get() else {
        return;
    };
    let Ok(mut guard) = slot.lock() else {
        return;
    };
    let Some(state) = guard.as_mut() else {
        return;
    };

    let payload = json!({
        "ts": chrono::Utc::now().to_rfc3339(),
        "level": level,
        "target": "agile-agent",
        "event": event,
        "message": message,
        "run_id": state.run_id,
        "run_mode": state.run_mode.as_str(),
        "pid": process::id(),
        "thread": format!("{:?}", std::thread::current().id()),
        "workplace_id": state.workplace_id,
        "workplace_path": state.workplace_path,
        "fields": fields,
    });

    if serde_json::to_writer(&mut state.writer, &payload).is_err() {
        return;
    }
    if state.writer.write_all(b"\n").is_err() {
        return;
    }
    let _ = state.writer.flush();
}

#[cfg(test)]
mod tests {
    use super::RunMode;
    use super::current_log_path;
    use super::debug_event;
    use super::init_for_workplace;
    use crate::workplace_store::WorkplaceStore;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn init_creates_workplace_log_file_and_latest_pointer() {
        let temp = TempDir::new().expect("tempdir");
        let workplace = WorkplaceStore::for_cwd(temp.path()).expect("workplace");
        workplace.ensure().expect("ensure");

        let initialized = init_for_workplace(&workplace, RunMode::RunLoop).expect("init logger");
        debug_event("test.bootstrap", "hello logger", serde_json::json!({ "scope": "unit" }));

        let log_path = current_log_path().expect("log path");
        assert_eq!(log_path, initialized.log_path);
        assert!(log_path.exists(), "missing {}", log_path.display());

        let latest_path = workplace.path().join("logs/latest-path.txt");
        assert_eq!(
            fs::read_to_string(&latest_path).expect("latest pointer").trim(),
            log_path.display().to_string()
        );

        let contents = fs::read_to_string(&log_path).expect("log contents");
        assert!(contents.contains("\"event\":\"test.bootstrap\""));
        assert!(contents.contains("\"run_mode\":\"run-loop\""));
        assert!(contents.contains("\"workplace_id\""));
    }

    #[test]
    fn reinit_replaces_active_log_destination() {
        let root = TempDir::new().expect("tempdir");
        let workspace_a = root.path().join("one");
        let workspace_b = root.path().join("two");
        fs::create_dir_all(&workspace_a).expect("workspace a");
        fs::create_dir_all(&workspace_b).expect("workspace b");

        let first = WorkplaceStore::for_cwd(&workspace_a).expect("first workplace");
        let second = WorkplaceStore::for_cwd(&workspace_b).expect("second workplace");
        first.ensure().expect("ensure first");
        second.ensure().expect("ensure second");

        let first_init = init_for_workplace(&first, RunMode::RunLoop).expect("first init");
        debug_event("test.first", "first logger", serde_json::json!({}));
        let second_init = init_for_workplace(&second, RunMode::Doctor).expect("second init");
        debug_event("test.second", "second logger", serde_json::json!({}));

        let first_contents = fs::read_to_string(first_init.log_path).expect("first contents");
        let second_contents = fs::read_to_string(second_init.log_path).expect("second contents");
        assert!(first_contents.contains("\"event\":\"test.first\""));
        assert!(!first_contents.contains("\"event\":\"test.second\""));
        assert!(second_contents.contains("\"event\":\"test.second\""));
    }
}
