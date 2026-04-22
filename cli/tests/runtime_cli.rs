#![allow(deprecated)]

#![cfg(feature = "core")]

use std::fs;

use serde_json::Value;

use agent_test_support::RuntimeHarness;

#[test]
fn run_loop_creates_agent_runtime_files() {
    let harness = RuntimeHarness::new();
    harness.write_backlog_with_ready_todo("write summary");

    let output = harness.run_cli(&["run-loop", "--max-iterations", "1"]);
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let agent_dir = harness.agent_dir();
    assert!(agent_dir.join("meta.json").exists());
    assert!(agent_dir.join("state.json").exists());
    assert!(agent_dir.join("transcript.json").exists());
    assert!(agent_dir.join("messages.json").exists());
    assert!(agent_dir.join("memory.json").exists());
}

#[test]
fn resume_last_reuses_claude_session_id() {
    let harness = RuntimeHarness::new();
    harness.write_backlog_with_ready_todo("write summary");

    let first = harness.run_cli(&["run-loop", "--max-iterations", "1"]);
    assert!(
        first.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&first.stderr)
    );

    harness.overwrite_backlog_with_ready_todo("write summary again");
    let second = harness.run_cli(&["run-loop", "--max-iterations", "1", "--resume-last"]);
    assert!(
        second.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&second.stderr)
    );

    let log = harness.read_provider_log();
    let lines: Vec<&str> = log.lines().collect();
    assert!(lines.contains(&"resume=<none>"));
    assert!(lines.contains(&"resume=sess-cli-1"));
}

#[test]
fn resume_last_reuses_codex_thread_id() {
    let harness = RuntimeHarness::new();
    harness.write_backlog_with_ready_todo("write summary");

    let first = harness.run_cli_with_codex(&["run-loop", "--max-iterations", "1"]);
    assert!(
        first.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&first.stderr)
    );

    harness.overwrite_backlog_with_ready_todo("write summary again");
    let second =
        harness.run_cli_with_codex(&["run-loop", "--max-iterations", "1", "--resume-last"]);
    assert!(
        second.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&second.stderr)
    );

    let log = harness.read_provider_log();
    let lines: Vec<&str> = log.lines().collect();
    assert!(lines.contains(&"resume=thr-cli-1"));
}

#[test]
fn inspect_commands_read_workplace_state() {
    let harness = RuntimeHarness::new();
    harness.write_backlog_with_ready_todo("write summary");

    let bootstrap = harness.run_cli(&["run-loop", "--max-iterations", "0"]);
    assert!(bootstrap.status.success());

    let workplace = harness.run_cli(&["workplace", "current"]);
    assert!(workplace.status.success());
    let workplace_stdout = String::from_utf8_lossy(&workplace.stdout);
    assert!(workplace_stdout.contains("workplace_id:"));
    assert!(workplace_stdout.contains("path:"));

    let current = harness.run_cli(&["agent", "current"]);
    assert!(current.status.success());
    let current_stdout = String::from_utf8_lossy(&current.stdout);
    assert!(current_stdout.contains("agent_id: agent_001"));
    assert!(current_stdout.contains("codename: alpha"));

    let list = harness.run_cli(&["agent", "list"]);
    assert!(list.status.success());
    let list_stdout = String::from_utf8_lossy(&list.stdout);
    assert!(list_stdout.contains("agent_001 alpha"));

    let messages: Value = serde_json::from_str(
        &fs::read_to_string(harness.agent_dir().join("messages.json")).expect("messages"),
    )
    .expect("parse messages json");
    assert!(messages.get("entries").is_some());
}
