use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::process::Command;

use tempfile::TempDir;

#[test]
fn coverage_script_runs_workspace_summary_and_writes_lcov_report() {
    let repo_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("workspace root")
        .to_path_buf();
    let script_path = repo_root.join("scripts/coverage.sh");
    assert!(
        script_path.exists(),
        "missing coverage script at {}",
        script_path.display()
    );

    let lcov_path = repo_root.join("target/coverage/lcov.info");
    if lcov_path.exists() {
        fs::remove_file(&lcov_path).expect("remove stale lcov report");
    }

    let temp = TempDir::new().expect("temp dir");
    let bin_dir = temp.path().join("bin");
    fs::create_dir_all(&bin_dir).expect("bin dir");
    let log_path = temp.path().join("tool.log");
    let cargo_path = bin_dir.join("cargo");
    let rustup_path = bin_dir.join("rustup");

    let fake_cargo = format!(
        r#"#!/usr/bin/env bash
set -euo pipefail
printf 'cargo %s\n' "$*" >> "{}"
if [[ "${{1-}}" == "llvm-cov" && "${{2-}}" == "--version" ]]; then
  echo "cargo-llvm-cov 0.0.0"
  exit 0
fi
if [[ "${{1-}}" == "llvm-cov" && "${{2-}}" == "--workspace" && "${{3-}}" == "--all-features" ]]; then
  echo "TOTAL 85.00%"
  exit 0
fi
if [[ "${{1-}}" == "llvm-cov" && "${{2-}}" == "report" ]]; then
  output=""
  for ((i=1; i<=$#; i++)); do
    if [[ "${{!i}}" == "--output-path" ]]; then
      j=$((i + 1))
      output="${{!j}}"
    fi
  done
  mkdir -p "$(dirname "$output")"
  printf 'TN:\n' > "$output"
  exit 0
fi
echo "unexpected cargo invocation: $*" >&2
exit 1
"#,
        log_path.display()
    );
    let fake_rustup = format!(
        r#"#!/usr/bin/env bash
set -euo pipefail
printf 'rustup %s\n' "$*" >> "{}"
if [[ "${{1-}}" == "component" && "${{2-}}" == "list" && "${{3-}}" == "--installed" ]]; then
  echo "llvm-tools-x86_64-unknown-linux-gnu"
  exit 0
fi
echo "unexpected rustup invocation: $*" >&2
exit 1
"#,
        log_path.display()
    );

    fs::write(&cargo_path, fake_cargo).expect("write cargo stub");
    fs::write(&rustup_path, fake_rustup).expect("write rustup stub");
    chmod_executable(&cargo_path);
    chmod_executable(&rustup_path);

    let original_path = std::env::var("PATH").unwrap_or_default();
    let output = Command::new(&script_path)
        .current_dir(&repo_root)
        .env("PATH", format!("{}:{}", bin_dir.display(), original_path))
        .output()
        .expect("run coverage script");

    assert!(
        output.status.success(),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let log = fs::read_to_string(&log_path).expect("tool log");
    assert!(log.contains("cargo llvm-cov --version"));
    assert!(log.contains("rustup component list --installed"));
    assert!(log.contains("cargo llvm-cov --workspace --all-features"));
    assert!(log.contains("cargo llvm-cov report --lcov --output-path target/coverage/lcov.info"));

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("TOTAL 85.00%"));
    assert!(stdout.contains("target/coverage/lcov.info"));
    assert!(
        lcov_path.exists(),
        "missing lcov output at {}",
        lcov_path.display()
    );
}

fn chmod_executable(path: &PathBuf) {
    let mut permissions = fs::metadata(path).expect("metadata").permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(path, permissions).expect("chmod");
}
