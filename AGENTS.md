# AGENTS.md

## Focus

- `agile-agent` is the primary implementation target in this workspace.
- Keep this file index-oriented. Add process detail elsewhere only when necessary.

## Engineering Rules

- Development must be TDD-first. Start from a failing test, make it pass, then refactor.
- Every code change must include thorough automated tests. Do not leave new behavior, edge cases, or regressions untested.
- Do not take on technical debt in technical decisions. Prefer clear architecture, explicit boundaries, and durable implementations over shortcuts.
- During every planning step, explicitly re-evaluate whether the current architecture and module boundaries are still the right fit.
- After every completed task, explicitly check whether all requirements were fully delivered and whether there is any worthwhile improvement to make before closing the work.

## Index

- `README.md`: project overview, scope, runtime model, and developer entrypoints.
- `Cargo.toml`: workspace manifest for the Rust crates.
- `cli/`: `agent-cli` crate, binary entrypoints and CLI-facing integration tests.
- `core/`: `agent-core` crate, providers, runtime loop, persistence, backlog/task state, and verification logic.
- `tui/`: `agent-tui` crate, terminal UI, rendering, transcript, composer, and interaction flow.
- `test-support/`: shared test helpers for workspace crates.
- `docs/plan/spec/`: implementation-facing product and sprint specs.
- `docs/superpowers/specs/`: design specs written through the superpowers workflow.
- `docs/superpowers/plans/`: implementation plans written through the superpowers workflow.
- `scripts/coverage.sh`: local coverage helper script.
- `target/`: build artifacts and generated output; not a source directory.
