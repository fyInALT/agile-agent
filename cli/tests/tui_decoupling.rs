use std::process::Command;

#[test]
fn tui_compiles_without_default_features() {
    let repo_root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("workspace root")
        .to_path_buf();

    let output = Command::new("cargo")
        .args(["check", "-p", "agent-tui", "--no-default-features"])
        .current_dir(&repo_root)
        .output()
        .expect("run cargo check");

    assert!(
        output.status.success(),
        "cargo check -p agent-tui --no-default-features failed:\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}
