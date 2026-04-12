use agent_core::logging::RunMode;
use agent_core::logging::current_log_path;
use agent_core::logging::debug_event;
use agent_core::logging::init_for_workplace;
use agent_core::workplace_store::WorkplaceStore;
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
