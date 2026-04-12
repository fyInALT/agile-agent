# Test Coverage Support Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a repository-supported Rust coverage workflow that prints a terminal summary and writes `target/coverage/lcov.info`.

**Architecture:** Use a checked-in Bash script as the single entrypoint for coverage collection. Keep production Rust code untouched, add one regression test that drives the script through a mocked `cargo`, and document the workflow in the repository README.

**Tech Stack:** Rust workspace tests, Bash, `cargo-llvm-cov`, `tempfile`, standard library process execution

---

## File Map

- Create: `scripts/coverage.sh`
- Create: `cli/tests/coverage_script.rs`
- Modify: `README.md`

`scripts/coverage.sh` owns the developer-facing coverage workflow and dependency checks.

`cli/tests/coverage_script.rs` owns regression coverage for the script contract by mocking `cargo`.

`README.md` owns the documented setup and usage path for local coverage collection.

### Task 1: Add Regression Coverage For The Coverage Script

**Files:**
- Create: `cli/tests/coverage_script.rs`
- Existing references: `cli/tests/runtime_cli.rs`, `test-support/src/lib.rs`

- [ ] **Step 1: Write the failing test**

```rust
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
    assert!(script_path.exists(), "missing coverage script at {}", script_path.display());

    let temp = TempDir::new().expect("temp dir");
    let bin_dir = temp.path().join("bin");
    fs::create_dir_all(&bin_dir).expect("bin dir");
    let log_path = temp.path().join("cargo.log");
    let cargo_path = bin_dir.join("cargo");
    let rustup_path = bin_dir.join("rustup");
    let fake_cargo = format!(
        r#"#!/usr/bin/env bash
set -euo pipefail
printf '%s\n' "$*" >> "{}"
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
    let fake_rustup = r#"#!/usr/bin/env bash
set -euo pipefail
if [[ "${1-}" == "component" && "${2-}" == "list" && "${3-}" == "--installed" ]]; then
  echo "llvm-tools-x86_64-unknown-linux-gnu"
  exit 0
fi
echo "unexpected rustup invocation: $*" >&2
exit 1
"#;
    fs::write(&cargo_path, fake_cargo).expect("write cargo stub");
    fs::write(&rustup_path, fake_rustup).expect("write rustup stub");
    let mut permissions = fs::metadata(&cargo_path).expect("cargo metadata").permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&cargo_path, permissions).expect("chmod cargo stub");
    let mut rustup_permissions = fs::metadata(&rustup_path).expect("rustup metadata").permissions();
    rustup_permissions.set_mode(0o755);
    fs::set_permissions(&rustup_path, rustup_permissions).expect("chmod rustup stub");

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

    let log = fs::read_to_string(&log_path).expect("cargo log");
    assert!(log.contains("llvm-cov --version"));
    assert!(log.contains("llvm-cov --workspace --all-features"));
    assert!(log.contains("llvm-cov report --lcov --output-path target/coverage/lcov.info"));

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("TOTAL 85.00%"));
    assert!(stdout.contains("target/coverage/lcov.info"));

    let lcov_path = repo_root.join("target/coverage/lcov.info");
    assert!(lcov_path.exists(), "missing lcov output at {}", lcov_path.display());
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p agent-cli coverage_script_runs_workspace_summary_and_writes_lcov_report`

Expected: FAIL because `scripts/coverage.sh` does not exist yet.

- [ ] **Step 3: Commit**

```bash
git add cli/tests/coverage_script.rs
git commit -m "test: add coverage script regression test"
```

### Task 2: Implement The Coverage Script

**Files:**
- Create: `scripts/coverage.sh`
- Test: `cli/tests/coverage_script.rs`

- [ ] **Step 1: Write minimal implementation**

```bash
#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "${script_dir}/.." && pwd)"
coverage_dir="${repo_root}/target/coverage"
lcov_path="${coverage_dir}/lcov.info"

if ! command -v cargo >/dev/null 2>&1; then
  echo "error: cargo is required but was not found on PATH." >&2
  exit 1
fi

if ! cargo llvm-cov --version >/dev/null 2>&1; then
  cat >&2 <<'EOF'
error: cargo-llvm-cov is required.
install it with: cargo install cargo-llvm-cov
EOF
  exit 1
fi

if ! command -v rustup >/dev/null 2>&1; then
  cat >&2 <<'EOF'
error: rustup is required to verify llvm-tools-preview.
install it with: https://rustup.rs/
EOF
  exit 1
fi

if ! rustup component list --installed 2>/dev/null | grep -q '^llvm-tools'; then
  cat >&2 <<'EOF'
error: llvm-tools-preview is required.
install it with: rustup component add llvm-tools-preview
EOF
  exit 1
fi

mkdir -p "${coverage_dir}"

(
  cd "${repo_root}"
  cargo llvm-cov --workspace --all-features
  cargo llvm-cov report --lcov --output-path target/coverage/lcov.info
)

echo "LCOV report written to ${lcov_path}"
```

- [ ] **Step 2: Run test to verify it passes**

Run: `cargo test -p agent-cli coverage_script_runs_workspace_summary_and_writes_lcov_report`

Expected: PASS

- [ ] **Step 3: Refactor the script checks if needed while staying green**

Keep behavior fixed:

- dependency checks stay explicit
- workspace command remains `cargo llvm-cov --workspace --all-features`
- LCOV export remains `target/coverage/lcov.info`

- [ ] **Step 4: Commit**

```bash
git add scripts/coverage.sh cli/tests/coverage_script.rs
git commit -m "feat: add workspace coverage script"
```

### Task 3: Document The Coverage Workflow

**Files:**
- Modify: `README.md`
- Test: `cli/tests/coverage_script.rs`

- [ ] **Step 1: Write the documentation change**

Add a coverage subsection after the existing developer checks block:

```md
Developer checks:

~~~bash
cargo check
cargo test
~~~

Coverage setup:

~~~bash
cargo install cargo-llvm-cov
rustup component add llvm-tools-preview
~~~

Coverage report:

~~~bash
./scripts/coverage.sh
~~~

This prints a terminal coverage summary and writes `target/coverage/lcov.info`.
```

- [ ] **Step 2: Run the focused regression test again**

Run: `cargo test -p agent-cli coverage_script_runs_workspace_summary_and_writes_lcov_report`

Expected: PASS

- [ ] **Step 3: Commit**

```bash
git add README.md
git commit -m "docs: add coverage workflow"
```

### Task 4: Final Verification

**Files:**
- Verify: `scripts/coverage.sh`
- Verify: `cli/tests/coverage_script.rs`
- Verify: `README.md`

- [ ] **Step 1: Run the focused coverage-script test**

Run: `cargo test -p agent-cli coverage_script_runs_workspace_summary_and_writes_lcov_report`

Expected: PASS with 1 test passed.

- [ ] **Step 2: Run the full workspace test suite**

Run: `cargo test`

Expected: PASS with no failing tests.

- [ ] **Step 3: Smoke-test the script dependency checks or real help path**

Run: `bash scripts/coverage.sh`

Expected:
- either the command succeeds and writes `target/coverage/lcov.info`
- or it fails with actionable install guidance for `cargo-llvm-cov` / `llvm-tools-preview`

- [ ] **Step 4: Review repository diff**

Run: `git status --short`

Expected:
- only `README.md`, `scripts/coverage.sh`, and `cli/tests/coverage_script.rs` changed for implementation
- plus the plan/spec docs already committed earlier

- [ ] **Step 5: Final commit**

```bash
git add README.md scripts/coverage.sh cli/tests/coverage_script.rs
git commit -m "feat: add test coverage workflow"
```
