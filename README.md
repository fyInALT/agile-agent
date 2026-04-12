# agile-agent

`agile-agent` is a Rust workspace for building a local autonomous engineering agent on top of existing coding CLIs such as `claude` and `codex`.

Current product scope:

- an interactive terminal UI
- local skill discovery and prompt injection
- persisted backlog, task, and session state
- a single-agent autonomous execution loop
- headless CLI execution for automation and experiments

## Status

Implemented:

- V1: interactive execution substrate
- V2: single-agent autonomous execution loop

Not started yet:

- multi-agent parallel development
- Scrum-style coordination between sub-agents
- workflow self-improvement

## Workspace

```text
agile-agent/
├── cli/   # `agile-agent` binary and subcommands
├── core/  # providers, state, loop runner, persistence, verification
├── docs/  # implementation-facing specs
└── tui/   # interactive terminal UI
```

Workspace crates:

- `agent-cli`: CLI entrypoints such as `doctor`, `probe`, `resume-last`, and headless `run-loop`
- `agent-core`: providers, backlog/task models, persistence, verification, and artifacts
- `agent-tui`: codex-style terminal UI

## Features

### Providers

- `claude` via a stream-JSON bridge
- `codex` via an app-server stdio bridge
- `mock` for local development, tests, and headless fallback

Default provider selection order:

1. `claude`
2. `codex`
3. `mock`

Session continuity is reused when available:

- Claude via `session_id`
- Codex via `thread_id`

### TUI

The TUI provides:

- a codex-style transcript + composer layout
- multiline editing and paste support
- transcript paging and overlay browsing
- width-aware Markdown rendering for assistant output
- a local skill browser
- slash commands for provider inspection, backlog updates, and loop control

Common keybindings:

- `Enter`: submit
- `Ctrl+J`: newline
- `Tab`: switch provider
- `Ctrl+T`: open transcript overlay
- `$`: open skill browser when the composer is empty
- `Ctrl+C`: quit
- `q`: quit when the composer is empty

Local slash commands:

- `/help`
- `/provider`
- `/skills`
- `/doctor`
- `/backlog`
- `/todo-add <title>`
- `/run-once`
- `/run-loop`
- `/quit`

The TUI requires at least one real provider (`claude` or `codex`) to be installed.

### Autonomous Loop

The current loop:

1. picks the next ready todo
2. creates or resumes a task
3. executes through the selected provider
4. continues unfinished work up to the continuation guardrail
5. runs verification
6. marks the task `done`, `failed`, or `blocked`
7. writes task artifacts and escalations when needed

Default headless guardrails:

- `max_iterations = 5`
- `max_continuations_per_task = 3`
- `max_verification_failures = 1`

### Local Skills

Skills are discovered from:

- `<cwd>/.agile-agent/skills`
- `<cwd>/skills`
- the platform config directory under `agile-agent/skills`

Each skill lives in its own directory and must contain a `SKILL.md`. Enabled skills are injected into the next provider prompt as structured context.

## Quick Start

Requirements:

- Rust toolchain
- for the TUI: `claude` or `codex` installed and available on `PATH`
- optional overrides through `AGILE_AGENT_CLAUDE_PATH` or `AGILE_AGENT_CODEX_PATH`

Build:

```bash
cargo build
```

Check provider availability:

```bash
cargo run -p agent-cli -- doctor
```

Launch the TUI:

```bash
cargo run -p agent-cli
```

Resume the most recent saved TUI session:

```bash
cargo run -p agent-cli -- resume-last
```

Run the autonomous loop headlessly:

```bash
cargo run -p agent-cli -- run-loop --max-iterations 5
```

Resume the most recent saved session before the headless loop starts:

```bash
cargo run -p agent-cli -- run-loop --max-iterations 5 --resume-last
```

Print structured probe output:

```bash
cargo run -p agent-cli -- probe --json
```

Developer checks:

```bash
cargo check
cargo test
```

## Provider Configuration

Real providers are resolved through:

- `AGILE_AGENT_CLAUDE_PATH`
- `AGILE_AGENT_CODEX_PATH`

If those variables are unset, `agile-agent` falls back to `claude` and `codex` from `PATH`.

`doctor` reports:

- resolved executable path
- `--version` output
- protocol
- availability or probe errors

## Verification

Verification is intentionally simple today:

- assistant output must be non-empty
- `cargo check` is added automatically when the working directory contains a `Cargo.toml`

Verification evidence is stored with task artifacts so failed runs can be inspected later.

## Persistence

Local state is stored under the platform-local data directory returned by `dirs::data_local_dir()`, inside an `agile-agent/` directory.

Current files and folders include:

- `backlog.json`
- `recent-session.json`
- `sessions/session-*.json`
- `task-artifacts/*.json`
- `escalations/*.json`

Persisted state includes:

- backlog todos and tasks
- transcript history
- selected provider
- Claude session / Codex thread continuity
- enabled skills
- active task and loop state

## Documentation

Implementation-facing specs live under `docs/plan/spec/`.

## Current Boundaries

`agile-agent` is intentionally narrower than its long-term name suggests. Today it is:

- a local TUI + CLI for driving existing AI coding tools
- a persisted single-agent execution substrate
- a small autonomous loop for repo-local engineering work

It is not yet:

- a multi-agent coordinator
- a Scrum automation engine
- a self-improving workflow platform
- a full project-management system
