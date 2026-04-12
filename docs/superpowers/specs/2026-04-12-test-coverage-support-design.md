# Test Coverage Support Design

## Metadata

- Date: `2026-04-12`
- Project: `agile-agent`
- Status: `ready for user review`
- Language: `English`

## 1. Purpose

`agile-agent` currently documents `cargo check` and `cargo test` for local development, but it does not provide
a stable workspace command for collecting test coverage.

The goal of this design is to add a repository-supported coverage workflow that:

1. prints a readable text coverage summary in the terminal
2. writes an `lcov.info` artifact for editor tooling and future CI integration
3. works at the Rust workspace level rather than per-crate one-off commands
4. fails clearly when required tools are missing

This is a developer-experience change. It does not change product runtime behavior.

## 2. Scope

### In scope

- add one repository script for collecting workspace coverage
- store the generated LCOV artifact under `target/coverage/lcov.info`
- document required developer setup for the coverage command
- add automated regression coverage for the script contract

### Out of scope

- HTML coverage reports
- CI workflow integration
- coverage thresholds or build enforcement
- per-crate custom filtering options
- replacing the existing `cargo test` workflow

## 3. Recommendation

The repository should use `cargo-llvm-cov` behind a checked-in script.

Rationale:

- Rust's official `-C instrument-coverage` flow depends on LLVM tools that must be compatible with the current
  `rustc` toolchain.
- `cargo-llvm-cov` wraps the official instrumentation flow and reduces workspace-specific command complexity.
- `cargo-llvm-cov` can print terminal summaries and export `lcov.info`, which matches the requested workflow.

The script should be the canonical entrypoint so developers do not need to remember a long sequence of commands.

## 4. Current Baseline

The repository currently has:

- a Cargo workspace rooted at `agile-agent/Cargo.toml`
- documented developer commands for `cargo check` and `cargo test`
- no `scripts/` directory
- no `justfile`, `Makefile`, or existing coverage automation

Current test locations include:

- integration coverage in `cli/tests/runtime_cli.rs`
- TUI-focused test coverage in `tui/src/shell_tests.rs`

The new coverage flow should cover the whole workspace rather than target only one crate.

## 5. Design Overview

### 5.1 User entrypoint

Add a checked-in shell script at:

`scripts/coverage.sh`

The script becomes the recommended local coverage command for the repository.

### 5.2 Output contract

Running the script should:

1. ensure the required coverage tools are available
2. run workspace tests with coverage instrumentation through `cargo llvm-cov`
3. print the terminal summary to standard output
4. write `target/coverage/lcov.info`
5. print the final LCOV path for easy discovery

### 5.3 Output location

Coverage artifacts should live under:

`target/coverage/`

Rationale:

- the directory is already ignored by the repository because `target/` is ignored
- generated artifacts stay near other build outputs
- no additional `.gitignore` rules are required

## 6. Script Behavior

The script should be strict and deterministic:

- use `#!/usr/bin/env bash`
- enable `set -euo pipefail`
- resolve the repository root relative to the script location
- run commands from the repository root so the workspace manifest is used

Expected command sequence:

1. verify `cargo` exists on `PATH`
2. verify `cargo llvm-cov` is installed
3. verify the Rust component `llvm-tools-preview` is available or provide a clear remediation command
4. create `target/coverage/` if needed
5. run `cargo llvm-cov --workspace --all-features`
6. run `cargo llvm-cov report --lcov --output-path target/coverage/lcov.info`

Step 6 must reuse the coverage data collected by step 5. It should export LCOV without re-running the test suite.

The script should not attempt to auto-install missing tools. It should stop with actionable guidance instead.

## 7. Dependency and Tooling Policy

The repository should rely on the following external tooling for coverage:

- `cargo-llvm-cov`
- Rust component `llvm-tools-preview`

Recommended install commands to document:

```bash
cargo install cargo-llvm-cov
rustup component add llvm-tools-preview
```

This keeps the repository lightweight while still giving developers a single stable entrypoint.

## 8. Error Handling

Failure cases should be explicit.

### Missing `cargo-llvm-cov`

If `cargo llvm-cov --version` fails, the script should exit non-zero and print an install hint.

### Missing LLVM tools component

If the local Rust toolchain lacks `llvm-tools-preview`, the script should exit non-zero and print the matching
`rustup component add llvm-tools-preview` hint.

### Coverage command failure

If test execution or report generation fails, the script should propagate the non-zero exit code from the
underlying command.

The script should not swallow cargo output because the developer needs the failing test details.

## 9. Automated Test Strategy

This change should include direct automated coverage for the script contract.

### 9.1 Test target

Add an integration-style test that executes the script in a temporary environment with a mocked `cargo`
executable at the front of `PATH`.

### 9.2 What the test verifies

The test should assert:

- the script exits successfully when the mocked toolchain reports success
- the script invokes `cargo llvm-cov --workspace --all-features`
- the script invokes `cargo llvm-cov report --lcov --output-path target/coverage/lcov.info`
- the target LCOV path is printed or otherwise discoverable in script output

### 9.3 Why this test level is sufficient

The repository does not need an end-to-end real coverage run inside tests because that would be slow and
environment-sensitive.

Mocking the `cargo` executable gives stable regression coverage for:

- argument shape
- command order
- artifact path contract
- user-facing setup guidance

## 10. Documentation Changes

Update `README.md` to add a short coverage section under developer commands.

The documentation should include:

- required one-time setup commands
- the repository coverage command
- the output location of `target/coverage/lcov.info`
- a note that the script prints a terminal summary and writes LCOV output

## 11. Acceptance Criteria

This design is complete when:

1. `scripts/coverage.sh` exists and is executable
2. `scripts/coverage.sh` prints terminal coverage summary data
3. `scripts/coverage.sh` writes `target/coverage/lcov.info`
4. missing tool dependencies produce actionable errors
5. README documents the coverage workflow
6. automated tests cover the script's command contract

## 12. Open Decisions Resolved

The following decisions are now fixed for implementation:

- No HTML report generation in this change.
- The primary artifact is `target/coverage/lcov.info`.
- The repository standardizes on `cargo-llvm-cov`, not a hand-written raw LLVM coverage pipeline.
- The script remains opt-in developer tooling and does not enforce thresholds.
