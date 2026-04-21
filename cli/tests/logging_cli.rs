#![cfg(feature = "core")]

use std::fs;
use std::path::PathBuf;

use agent_test_support::RuntimeHarness;

#[test]
fn run_loop_creates_workplace_log_with_launch_event() {
    let harness = RuntimeHarness::new();
    harness.write_backlog_with_ready_todo("write summary");

    let output = harness.run_cli(&["run-loop", "--max-iterations", "1"]);
    assert!(
        output.status.success(),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let latest_path = harness.workplace().path().join("logs/latest-path.txt");
    assert!(latest_path.exists(), "missing {}", latest_path.display());

    let log_path = PathBuf::from(
        fs::read_to_string(&latest_path)
            .expect("latest path")
            .trim(),
    );
    assert!(log_path.exists(), "missing {}", log_path.display());

    let contents = fs::read_to_string(&log_path).expect("log contents");
    assert!(contents.contains("\"event\":\"app.launch\""));
    assert!(contents.contains("\"run_mode\":\"run-loop\""));
    assert!(contents.contains("\"event\":\"agent.bootstrap\""));
    assert!(contents.contains("\"event\":\"storage.write\""));
    assert!(contents.contains("\"event\":\"loop.iteration.start\""));
}
