# Debug Logging And Observability Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add workplace-scoped debug logging that records the full runtime timeline, including raw Claude/Codex communication, agent lifecycle, loop execution, and persistence activity.

**Architecture:** Add a shared JSONL logger in `agent-core` and initialize it at the outer launch boundary after workplace resolution. Keep one unified per-run log file under the workplace, then instrument lifecycle, persistence, loop, provider, and TUI boundaries with stable structured event names and non-fatal logging behavior.

**Tech Stack:** Rust workspace, standard library `Mutex`/`OnceLock`, `serde_json`, `chrono`, existing `tempfile`-based tests, focused `cargo test` execution

---

## File Map

- Create: `core/src/logging.rs`
- Create: `cli/tests/logging_cli.rs`
- Modify: `core/src/lib.rs`
- Modify: `core/src/workplace_store.rs`
- Modify: `core/src/agent_store.rs`
- Modify: `core/src/agent_runtime.rs`
- Modify: `core/src/runtime_session.rs`
- Modify: `core/src/session_store.rs`
- Modify: `core/src/backlog_store.rs`
- Modify: `core/src/provider.rs`
- Modify: `core/src/loop_runner.rs`
- Modify: `core/src/task_engine.rs`
- Modify: `core/src/providers/claude.rs`
- Modify: `core/src/providers/codex.rs`
- Modify: `cli/src/app_runner.rs`
- Modify: `tui/src/lib.rs`
- Modify: `tui/src/app_loop.rs`
- Modify: `tui/src/shell_tests.rs`

`core/src/logging.rs` owns log file creation, event serialization, run metadata, and safe write behavior.

`cli/tests/logging_cli.rs` owns headless regression coverage that proves a workplace log file is created and populated.

`cli/src/app_runner.rs` owns CLI-mode logger initialization and high-level launch events.

`tui/src/lib.rs` and `tui/src/app_loop.rs` own TUI-mode logger initialization and meaningful UI/runtime action logging.

`core/src/workplace_store.rs`, `core/src/agent_store.rs`, `core/src/session_store.rs`, and `core/src/backlog_store.rs` own storage and persistence instrumentation.

`core/src/agent_runtime.rs` and `core/src/runtime_session.rs` own agent lifecycle, restore, and persist-bundle instrumentation.

`core/src/loop_runner.rs` and `core/src/task_engine.rs` own autonomous loop, task resolution, verification, and escalation instrumentation.

`core/src/provider.rs`, `core/src/providers/claude.rs`, and `core/src/providers/codex.rs` own provider dispatch and raw transport instrumentation.

## Task 1: Add The Shared Workplace Logger

**Files:**
- Create: `core/src/logging.rs`
- Modify: `core/src/lib.rs`

- [ ] **Step 1: Write the failing logger bootstrap tests**

```rust
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
```

- [ ] **Step 2: Run the focused test command and confirm failure**

Run: `cargo test -p agent-core logging::tests::init_creates_workplace_log_file_and_latest_pointer`

Expected: FAIL because `core/src/logging.rs` and `pub mod logging;` do not exist yet.

- [ ] **Step 3: Add the logger module and public export**

```rust
// core/src/lib.rs
pub mod logging;
```

```rust
// core/src/logging.rs
use std::fs;
use std::fs::File;
use std::io::BufWriter;
use std::io::Write;
use std::path::PathBuf;
use std::process;
use std::sync::Mutex;
use std::sync::OnceLock;

use anyhow::Context;
use anyhow::Result;
use chrono::Utc;
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

pub fn init_for_workplace(workplace: &WorkplaceStore, run_mode: RunMode) -> Result<InitializedLogger> {
    let logs_dir = workplace.path().join("logs");
    fs::create_dir_all(&logs_dir).with_context(|| format!("failed to create {}", logs_dir.display()))?;

    let timestamp = Utc::now().format("%Y-%m-%dT%H-%M-%SZ").to_string();
    let run_id = format!("run-{}-{}", timestamp, process::id());
    let log_path = logs_dir.join(format!("{timestamp}-{}-pid{}.jsonl", run_mode.as_str(), process::id()));
    let file = File::create(&log_path).with_context(|| format!("failed to create {}", log_path.display()))?;

    fs::write(logs_dir.join("latest-path.txt"), log_path.display().to_string())
        .with_context(|| format!("failed to update {}", logs_dir.join("latest-path.txt").display()))?;

    let state = LoggerState {
        run_id: run_id.clone(),
        run_mode,
        workplace_id: workplace.workplace_id().as_str().to_string(),
        workplace_path: workplace.path().display().to_string(),
        log_path: log_path.clone(),
        writer: BufWriter::new(file),
    };

    let slot = LOGGER.get_or_init(|| Mutex::new(None));
    *slot.lock().expect("logger lock") = Some(state);

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
```

- [ ] **Step 4: Finish the JSON line writer and rerun the logger tests**

Add the missing pieces to `core/src/logging.rs`:

```rust
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
        "ts": Utc::now().to_rfc3339(),
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

    if serde_json::to_writer(&mut state.writer, &payload).is_ok() {
        let _ = state.writer.write_all(b"\n");
        let _ = state.writer.flush();
    }
}
```

Run: `cargo test -p agent-core logging::tests`

Expected: PASS

- [ ] **Step 5: Commit the logger foundation**

```bash
git add core/src/lib.rs core/src/logging.rs
git commit -m "feat: add workplace debug logger"
```

## Task 2: Initialize Logging At CLI And TUI Launch Boundaries

**Files:**
- Create: `cli/tests/logging_cli.rs`
- Modify: `cli/src/app_runner.rs`
- Modify: `tui/src/lib.rs`

- [ ] **Step 1: Write the failing headless integration test**

```rust
use std::env;
use std::fs;
use std::path::PathBuf;

use agent_core::workplace_store::WorkplaceStore;
use clap::Parser;
use tempfile::TempDir;

#[test]
fn run_loop_creates_workplace_log_with_launch_and_stop_events() {
    let temp = TempDir::new().expect("tempdir");
    let original_cwd = env::current_dir().expect("current dir");
    env::set_current_dir(temp.path()).expect("enter temp cwd");

    let cli = agent_cli::app_runner::Cli::parse_from([
        "agile-agent",
        "run-loop",
        "--max-iterations",
        "1",
    ]);
    agent_cli::app_runner::execute(cli).expect("run loop");

    let workplace = WorkplaceStore::for_cwd(temp.path()).expect("workplace");
    let latest_path = workplace.path().join("logs/latest-path.txt");
    let log_path = PathBuf::from(fs::read_to_string(&latest_path).expect("latest path").trim());
    let contents = fs::read_to_string(&log_path).expect("log file");

    assert!(contents.contains("\"event\":\"app.launch\""));
    assert!(contents.contains("\"event\":\"loop.start\""));
    assert!(contents.contains("\"event\":\"loop.stop\""));

    env::set_current_dir(original_cwd).expect("restore cwd");
}
```

- [ ] **Step 2: Run the focused integration test and confirm failure**

Run: `cargo test -p agent-cli run_loop_creates_workplace_log_with_launch_and_stop_events -- --test-threads=1`

Expected: FAIL because CLI startup does not initialize logging yet.

- [ ] **Step 3: Initialize logging in CLI command execution**

```rust
use agent_core::logging;
use agent_core::logging::RunMode;

fn init_logging_for_mode(launch_cwd: &std::path::Path, run_mode: RunMode) {
    match WorkplaceStore::for_cwd(launch_cwd).and_then(|workplace| {
        workplace.ensure()?;
        logging::init_for_workplace(&workplace, run_mode)
    }) {
        Ok(initialized) => logging::debug_event(
            "app.launch",
            "initialized CLI logging",
            serde_json::json!({
                "log_path": initialized.log_path,
                "cwd": launch_cwd.display().to_string(),
                "run_mode": run_mode.as_str(),
            }),
        ),
        Err(error) => eprintln!("warning: failed to initialize debug logging: {error}"),
    }
}

pub fn execute(cli: Cli) -> Result<()> {
    let launch_cwd = env::current_dir()?;
    let run_mode = match &cli.command {
        None => RunMode::Tui,
        Some(Command::ResumeLast) => RunMode::ResumeLast,
        Some(Command::RunLoop { .. }) => RunMode::RunLoop,
        Some(Command::Doctor) => RunMode::Doctor,
        Some(Command::Probe { .. }) => RunMode::Probe,
        Some(Command::Agent { command: AgentCommand::Current }) => RunMode::AgentCurrent,
        Some(Command::Agent { command: AgentCommand::List }) => RunMode::AgentList,
        Some(Command::Workplace { command: WorkplaceCommand::Current }) => {
            RunMode::WorkplaceCurrent
        }
    };
    init_logging_for_mode(&launch_cwd, run_mode);

    match cli.command {
        None => agent_tui::run_tui(),
        Some(Command::ResumeLast) => agent_tui::run_tui_with_resume_last(),
        Some(Command::Agent {
            command: AgentCommand::Current,
        }) => print_current_agent(),
        Some(Command::Agent {
            command: AgentCommand::List,
        }) => print_agent_list(),
        Some(Command::Workplace {
            command: WorkplaceCommand::Current,
        }) => print_current_workplace(),
        Some(Command::RunLoop {
            max_iterations,
            resume_last,
        }) => run_loop_headless(max_iterations, resume_last),
        Some(Command::Doctor) => {
            print!("{}", probe::render_doctor_text(&probe::probe_report()));
            Ok(())
        }
        Some(Command::Probe { json: true }) => {
            println!("{}", serde_json::to_string_pretty(&probe::probe_report())?);
            Ok(())
        }
        Some(Command::Probe { json: false }) => {
            println!("probe requires --json");
            Ok(())
        }
    }
}
```

- [ ] **Step 4: Initialize logging in TUI startup before entering the app loop**

```rust
use agent_core::logging;
use agent_core::logging::RunMode;
use agent_core::workplace_store::WorkplaceStore;

fn run_tui_with_options(resume_last: bool) -> Result<()> {
    let launch_cwd = std::env::current_dir()?;
    if let Ok(workplace) = WorkplaceStore::for_cwd(&launch_cwd) {
        if workplace.ensure().is_ok() {
            if let Ok(initialized) = logging::init_for_workplace(
                &workplace,
                if resume_last { RunMode::ResumeLast } else { RunMode::Tui },
            ) {
                logging::debug_event(
                    "app.launch",
                    "initialized TUI logging",
                    serde_json::json!({
                        "log_path": initialized.log_path,
                        "resume_last": resume_last,
                        "cwd": launch_cwd.display().to_string(),
                    }),
                );
            }
        }
    }

    if !probe::has_any_real_provider() {
        anyhow::bail!("no real provider detected: install codex or claude, or run `agile-agent doctor`");
    }

    let mut terminal = terminal::setup_terminal()?;
    let result = app_loop::run(terminal.terminal_mut(), resume_last);
    terminal.restore()?;
    result.map(|_| ())
}
```

- [ ] **Step 5: Rerun the integration test**

Run: `cargo test -p agent-cli run_loop_creates_workplace_log_with_launch_and_stop_events -- --test-threads=1`

Expected: PASS

- [ ] **Step 6: Commit the launch wiring**

```bash
git add cli/tests/logging_cli.rs cli/src/app_runner.rs tui/src/lib.rs
git commit -m "feat: initialize debug logging at launch"
```

## Task 3: Instrument Workplace, Persistence, And Agent Lifecycle

**Files:**
- Modify: `core/src/workplace_store.rs`
- Modify: `core/src/agent_store.rs`
- Modify: `core/src/agent_runtime.rs`
- Modify: `core/src/runtime_session.rs`
- Modify: `core/src/session_store.rs`
- Modify: `core/src/backlog_store.rs`

- [ ] **Step 1: Write the failing lifecycle and storage logging tests**

Add one focused test in `core/src/runtime_session.rs` and one in `core/src/workplace_store.rs`:

```rust
#[test]
fn bootstrap_logs_restored_agent_and_transcript_restore() {
    let temp = TempDir::new().expect("tempdir");
    let workplace = WorkplaceStore::for_cwd(temp.path()).expect("workplace");
    workplace.ensure().expect("ensure");
    crate::logging::init_for_workplace(&workplace, crate::logging::RunMode::RunLoop)
        .expect("init logger");

    let mut first = RuntimeSession::bootstrap(temp.path().into(), ProviderKind::Codex, false)
        .expect("bootstrap");
    first.app.push_user_message("hello".to_string());
    first.mark_stopped_and_persist().expect("persist");

    let _restored = RuntimeSession::bootstrap(temp.path().into(), ProviderKind::Mock, false)
        .expect("restore");

    let log_path = crate::logging::current_log_path().expect("log path");
    let contents = std::fs::read_to_string(log_path).expect("log file");
    assert!(contents.contains("\"event\":\"agent.bootstrap\""));
    assert!(contents.contains("\"event\":\"agent.restore_transcript\""));
}
```

```rust
#[test]
fn ensure_logs_workplace_resolution_and_meta_write() {
    let temp = TempDir::new().expect("tempdir");
    let store = WorkplaceStore::for_cwd(temp.path()).expect("store");
    crate::logging::init_for_workplace(&store, crate::logging::RunMode::RunLoop)
        .expect("init logger");
    store.ensure().expect("ensure");

    let log_path = crate::logging::current_log_path().expect("log path");
    let contents = std::fs::read_to_string(log_path).expect("log file");
    assert!(contents.contains("\"event\":\"workplace.ensure\""));
    assert!(contents.contains("\"event\":\"workplace.meta.save\""));
}
```

- [ ] **Step 2: Run the focused storage and lifecycle tests and confirm failure**

Run: `cargo test -p agent-core bootstrap_logs_restored_agent_and_transcript_restore -- --exact`

Expected: FAIL because lifecycle instrumentation does not exist yet.

Run: `cargo test -p agent-core ensure_logs_workplace_resolution_and_meta_write -- --exact`

Expected: FAIL because workplace instrumentation does not exist yet.

- [ ] **Step 3: Add instrumentation to workplace and storage functions**

```rust
// core/src/workplace_store.rs
crate::logging::debug_event(
    "workplace.resolve",
    "resolved workplace from cwd",
    serde_json::json!({
        "cwd": canonical_cwd.display().to_string(),
        "workplace_id": workplace_id.as_str(),
        "path": path.display().to_string(),
    }),
);

crate::logging::debug_event(
    "workplace.meta.save",
    "saved workplace metadata",
    serde_json::json!({
        "path": path.display().to_string(),
        "workplace_id": meta.workplace_id.as_str(),
    }),
);
```

```rust
// core/src/agent_store.rs
crate::logging::debug_event(
    "storage.write",
    "saved agent transcript",
    serde_json::json!({
        "kind": "agent_transcript",
        "agent_id": agent_id.as_str(),
        "path": path.display().to_string(),
    }),
);
```

```rust
// core/src/session_store.rs
crate::logging::debug_event(
    "storage.read",
    "loaded workplace session",
    serde_json::json!({
        "kind": "recent_session",
        "path": pointer_path.display().to_string(),
    }),
);
```

```rust
// core/src/backlog_store.rs
crate::logging::debug_event(
    "storage.read",
    "loaded workplace backlog",
    serde_json::json!({
        "kind": "backlog",
        "path": path.display().to_string(),
    }),
);
```

- [ ] **Step 4: Add instrumentation to runtime bootstrap, restore, and persist paths**

Add these exact lifecycle log calls at the bootstrap and persist boundaries:

```rust
// core/src/agent_runtime.rs
crate::logging::debug_event(
    "agent.bootstrap",
    "bootstrapped agent runtime",
    serde_json::json!({
        "bootstrap_kind": "restored",
        "agent_id": meta.agent_id.as_str(),
        "provider_type": meta.provider_type.label(),
    }),
);
```

```rust
// core/src/runtime_session.rs
crate::logging::debug_event(
    "agent.restore_transcript",
    "restored transcript into session app state",
    serde_json::json!({
        "agent_id": session.agent_runtime.agent_id().as_str(),
        "provider": session.app.selected_provider.label(),
    }),
);

crate::logging::debug_event(
    "agent.persist",
    "persisted runtime bundle",
    serde_json::json!({
        "agent_id": self.agent_runtime.agent_id().as_str(),
        "workplace_id": self.agent_runtime.meta().workplace_id.as_str(),
    }),
);
```

- [ ] **Step 5: Rerun the focused tests**

Run: `cargo test -p agent-core bootstrap_logs_restored_agent_and_transcript_restore -- --exact`

Expected: PASS

Run: `cargo test -p agent-core ensure_logs_workplace_resolution_and_meta_write -- --exact`

Expected: PASS

- [ ] **Step 6: Commit the lifecycle and persistence instrumentation**

```bash
git add core/src/workplace_store.rs core/src/agent_store.rs core/src/agent_runtime.rs core/src/runtime_session.rs core/src/session_store.rs core/src/backlog_store.rs
git commit -m "feat: log agent lifecycle and persistence"
```

## Task 4: Instrument Loop And Task Resolution

**Files:**
- Modify: `core/src/loop_runner.rs`
- Modify: `core/src/task_engine.rs`

- [ ] **Step 1: Write the failing loop logging regression test**

Add this test to `core/src/loop_runner.rs`:

```rust
#[test]
fn run_loop_logs_iteration_boundaries_and_stop_reason() {
    let temp = tempfile::TempDir::new().expect("tempdir");
    let workplace = crate::workplace_store::WorkplaceStore::for_cwd(temp.path()).expect("workplace");
    workplace.ensure().expect("ensure");
    crate::logging::init_for_workplace(&workplace, crate::logging::RunMode::RunLoop)
        .expect("init logger");

    let mut state =
        AppState::with_skills(ProviderKind::Mock, temp.path().into(), SkillRegistry::default());
    state.backlog.push_todo(ready_todo("todo-1", "write summary", 1));

    let summary = run_loop(
        &mut state,
        LoopGuardrails {
            max_iterations: 2,
            max_continuations_per_task: 1,
            max_verification_failures: 1,
        },
    )
    .expect("run loop");

    assert_eq!(summary.stopped_reason, "no ready todo available");

    let log_path = crate::logging::current_log_path().expect("log path");
    let contents = std::fs::read_to_string(log_path).expect("log file");
    assert!(contents.contains("\"event\":\"loop.start\""));
    assert!(contents.contains("\"event\":\"loop.iteration.start\""));
    assert!(contents.contains("\"event\":\"loop.stop\""));
    assert!(contents.contains("\"event\":\"task.complete\""));
}
```

- [ ] **Step 2: Run the focused test and confirm failure**

Run: `cargo test -p agent-core run_loop_logs_iteration_boundaries_and_stop_reason`

Expected: FAIL because loop and task resolution do not emit those events yet.

- [ ] **Step 3: Add loop boundary logging**

```rust
crate::logging::debug_event(
    "loop.start",
    "starting autonomous loop",
    serde_json::json!({
        "max_iterations": guardrails.max_iterations,
        "max_continuations_per_task": guardrails.max_continuations_per_task,
        "max_verification_failures": guardrails.max_verification_failures,
    }),
);

crate::logging::debug_event(
    "loop.iteration.start",
    "starting loop iteration",
    serde_json::json!({
        "iteration": iterations + 1,
        "active_task_id": state.active_task_id,
    }),
);

crate::logging::debug_event(
    "loop.stop",
    "stopping autonomous loop",
    serde_json::json!({
        "iterations": iterations,
        "verification_failures": verification_failures,
        "stopped_reason": "no ready todo available",
    }),
);
```

- [ ] **Step 4: Add task prompt, verification, escalation, and resolution logging**

```rust
crate::logging::debug_event(
    "task.prompt.build",
    "built task prompt",
    serde_json::json!({
        "task_id": task.id,
        "todo_id": task.todo_id,
        "prompt": prompt,
    }),
);

crate::logging::debug_event(
    "task.verify",
    "verification finished",
    serde_json::json!({
        "task_id": task.id,
        "outcome": format!("{:?}", result.outcome),
        "summary": result.summary,
    }),
);

crate::logging::debug_event(
    "task.escalate",
    "escalated active task",
    serde_json::json!({
        "task_id": task_id,
        "reason": reason,
    }),
);
```

- [ ] **Step 5: Rerun the focused test**

Run: `cargo test -p agent-core run_loop_logs_iteration_boundaries_and_stop_reason`

Expected: PASS

- [ ] **Step 6: Commit the loop instrumentation**

```bash
git add core/src/loop_runner.rs core/src/task_engine.rs
git commit -m "feat: log loop and task resolution"
```

## Task 5: Instrument Provider Dispatch And Claude Transport

**Files:**
- Modify: `core/src/provider.rs`
- Modify: `core/src/providers/claude.rs`

- [ ] **Step 1: Write the failing Claude transport logging test**

Add this test to `core/src/providers/claude.rs`:

```rust
#[test]
fn read_stdout_logs_raw_line_and_parsed_events() {
    let temp = tempfile::TempDir::new().expect("tempdir");
    let workplace = crate::workplace_store::WorkplaceStore::for_cwd(temp.path()).expect("workplace");
    workplace.ensure().expect("ensure");
    crate::logging::init_for_workplace(&workplace, crate::logging::RunMode::RunLoop)
        .expect("init logger");

    let input = r#"{"type":"assistant","message":{"role":"assistant","content":[{"type":"text","text":"hello"}]}}"#;
    let (tx, rx) = std::sync::mpsc::channel();
    read_stdout(std::io::Cursor::new(format!("{input}\n")), &tx).expect("read stdout");

    assert_eq!(rx.recv().expect("event"), ProviderEvent::AssistantChunk("hello".to_string()));

    let log_path = crate::logging::current_log_path().expect("log path");
    let contents = std::fs::read_to_string(log_path).expect("log file");
    assert!(contents.contains("\"event\":\"provider.claude.stdout_line\""));
    assert!(contents.contains(input));
    assert!(contents.contains("\"event\":\"provider.claude.event\""));
}
```

- [ ] **Step 2: Run the focused Claude test and confirm failure**

Run: `cargo test -p agent-core read_stdout_logs_raw_line_and_parsed_events`

Expected: FAIL because Claude raw lines and semantic events are not logged yet.

- [ ] **Step 3: Add dispatch-level provider start logging**

```rust
// core/src/provider.rs
debug_event(
    "provider.start",
    "starting provider request",
    serde_json::json!({
        "provider": provider.label(),
        "cwd": cwd.display().to_string(),
        "prompt": prompt,
        "session_handle": format!("{:?}", session_handle),
    }),
);
```

- [ ] **Step 4: Add Claude raw and semantic transport logging**

Add these exact Claude transport log calls:

```rust
crate::logging::debug_event(
    "provider.claude.start",
    "spawning Claude provider",
    serde_json::json!({
        "executable": executable,
        "cwd": cwd.display().to_string(),
        "args": args,
    }),
);

crate::logging::debug_event(
    "provider.claude.stdin_payload",
    "writing raw Claude stdin payload",
    serde_json::json!({
        "payload": payload,
    }),
);

crate::logging::debug_event(
    "provider.claude.stdout_line",
    "read raw Claude stdout line",
    serde_json::json!({
        "line": trimmed,
    }),
);

crate::logging::debug_event(
    "provider.claude.event",
    "parsed Claude provider event",
    serde_json::json!({
        "event_debug": format!("{:?}", event),
    }),
);
```

- [ ] **Step 5: Rerun the focused Claude test**

Run: `cargo test -p agent-core read_stdout_logs_raw_line_and_parsed_events`

Expected: PASS

- [ ] **Step 6: Commit the Claude instrumentation**

```bash
git add core/src/provider.rs core/src/providers/claude.rs
git commit -m "feat: log provider dispatch and claude transport"
```

## Task 6: Instrument Codex JSON-RPC Transport

**Files:**
- Modify: `core/src/providers/codex.rs`

- [ ] **Step 1: Write the failing Codex transport logging test**

Add this test to `core/src/providers/codex.rs`:

```rust
#[test]
fn wait_for_response_logs_raw_jsonrpc_messages() {
    let temp = tempfile::TempDir::new().expect("tempdir");
    let workplace = crate::workplace_store::WorkplaceStore::for_cwd(temp.path()).expect("workplace");
    workplace.ensure().expect("ensure");
    crate::logging::init_for_workplace(&workplace, crate::logging::RunMode::RunLoop)
        .expect("init logger");

    let mut stdout_lines = vec![
        Ok(r#"{"jsonrpc":"2.0","method":"thread/started","params":{"thread":{"id":"thr-123"}}}"#.to_string()),
        Ok(r#"{"jsonrpc":"2.0","id":2,"result":{"thread":{"id":"thr-123"}}}"#.to_string()),
    ]
    .into_iter();
    let mut stdin = Vec::new();
    let (tx, _rx) = std::sync::mpsc::channel();

    let response = wait_for_response(&mut stdout_lines, &mut stdin, 2, &tx, None)
        .expect("response");
    assert_eq!(thread_id_from_result(response.result.as_ref()), Some("thr-123".to_string()));

    let log_path = crate::logging::current_log_path().expect("log path");
    let contents = std::fs::read_to_string(log_path).expect("log file");
    assert!(contents.contains("\"event\":\"provider.codex.notification\""));
    assert!(contents.contains("\"event\":\"provider.codex.response\""));
}
```

- [ ] **Step 2: Run the focused Codex test and confirm failure**

Run: `cargo test -p agent-core wait_for_response_logs_raw_jsonrpc_messages`

Expected: FAIL because Codex raw JSON-RPC traffic is not logged yet.

- [ ] **Step 3: Add request, response, notification, approval, and exit logging**

Add these exact Codex transport log calls:

```rust
crate::logging::debug_event(
    "provider.codex.request",
    "writing Codex JSON-RPC request",
    serde_json::json!({
        "payload": json,
    }),
);

crate::logging::debug_event(
    "provider.codex.notification",
    "received Codex notification",
    serde_json::json!({
        "method": method,
        "params": params,
    }),
);

crate::logging::debug_event(
    "provider.codex.response",
    "received Codex JSON-RPC response",
    serde_json::json!({
        "id": message.id,
        "result": message.result,
        "error": message.error.as_ref().map(|error| error.message.clone()),
    }),
);

crate::logging::debug_event(
    "provider.codex.approval",
    "resolved Codex approval request",
    serde_json::json!({
        "method": method,
        "id": id,
        "decision": decision,
    }),
);

crate::logging::debug_event(
    "provider.codex.exit",
    "finished Codex process shutdown",
    serde_json::json!({
        "forced_kill": false,
    }),
);
```

- [ ] **Step 4: Rerun the focused Codex test**

Run: `cargo test -p agent-core wait_for_response_logs_raw_jsonrpc_messages`

Expected: PASS

- [ ] **Step 5: Commit the Codex instrumentation**

```bash
git add core/src/providers/codex.rs
git commit -m "feat: log codex jsonrpc transport"
```

## Task 7: Instrument TUI Actions And Final Integration Verification

**Files:**
- Modify: `tui/src/app_loop.rs`
- Modify: `tui/src/shell_tests.rs`
- Modify: `cli/tests/logging_cli.rs`

- [ ] **Step 1: Write the failing TUI action logging regression**

Add this test to `tui/src/shell_tests.rs`:

```rust
#[test]
fn provider_switch_logs_tui_action() {
    let temp = tempfile::TempDir::new().expect("tempdir");
    let workplace = agent_core::workplace_store::WorkplaceStore::for_cwd(temp.path())
        .expect("workplace");
    workplace.ensure().expect("ensure");
    agent_core::logging::init_for_workplace(&workplace, agent_core::logging::RunMode::Tui)
        .expect("init logger");

    let mut shell = ShellHarness::new(ProviderKind::Claude);
    shell.state.switch_to_new_agent(ProviderKind::Codex).expect("switch");

    let log_path = agent_core::logging::current_log_path().expect("log path");
    let contents = std::fs::read_to_string(log_path).expect("log file");
    assert!(contents.contains("\"event\":\"tui.provider_switch\""));
}
```

- [ ] **Step 2: Run the focused TUI test and confirm failure**

Run: `cargo test -p agent-tui provider_switch_logs_tui_action`

Expected: FAIL because TUI actions are not logged yet.

- [ ] **Step 3: Log meaningful TUI actions without logging every keystroke**

Add these exact TUI action log calls:

```rust
agent_core::logging::debug_event(
    "tui.submit",
    "submitted prompt from TUI",
    serde_json::json!({
        "provider": state.app().selected_provider.label(),
        "prompt": user_input,
        "active_task_id": state.app().active_task_id,
    }),
);

agent_core::logging::debug_event(
    "tui.command",
    "executed local TUI command",
    serde_json::json!({
        "command": format!("{:?}", command),
    }),
);

agent_core::logging::debug_event(
    "tui.provider_switch",
    "switched to sibling agent",
    serde_json::json!({
        "provider": next_provider.label(),
        "summary": summary,
    }),
);

agent_core::logging::debug_event(
    "tui.loop_control",
    "started autonomous loop from TUI",
    serde_json::json!({
        "remaining_iterations": state.app().remaining_loop_iterations,
    }),
);
```

- [ ] **Step 4: Expand the headless CLI integration assertion set**

Update `cli/tests/logging_cli.rs` to check for more than launch and stop:

```rust
assert!(contents.contains("\"event\":\"agent.bootstrap\""));
assert!(contents.contains("\"event\":\"storage.write\""));
assert!(contents.contains("\"event\":\"loop.iteration.start\""));
```

- [ ] **Step 5: Run the focused TUI and CLI regression tests**

Run: `cargo test -p agent-tui provider_switch_logs_tui_action`

Expected: PASS

Run: `cargo test -p agent-cli run_loop_creates_workplace_log_with_launch_and_stop_events -- --test-threads=1`

Expected: PASS

- [ ] **Step 6: Run the full verification suite**

Run: `cargo test`

Expected: PASS across `agent-core`, `agent-cli`, `agent-tui`, and `agent-test-support`.

- [ ] **Step 7: Commit the final observability pass**

```bash
git add tui/src/app_loop.rs tui/src/shell_tests.rs cli/tests/logging_cli.rs
git commit -m "feat: add end-to-end debug observability"
```
