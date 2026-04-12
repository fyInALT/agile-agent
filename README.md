# agile-agent

`agile-agent` is a Rust-based autonomous engineering agent built on top of existing AI coding tools such as `codex` and `claude`.

The current codebase has completed:

- V1: interactive execution substrate
- V2: single-agent autonomous execution loop

This means the project already supports:

- a TUI chat shell with the same general interaction model as `codex`
- real provider bridging for `claude` and `codex`
- local skills discovery and prompt injection
- backlog / todo / task persistence
- a single-agent autonomous loop:
  - pick one todo
  - create one task
  - execute through a provider
  - auto-continue unfinished work
  - run verification
  - mark the task as done / failed / blocked
  - write structured artifacts
- headless `run-loop` execution from the CLI

## Status

Current phase:

- V1 complete
- V2 complete

Not started yet:

- multi-agent parallel development
- Scrum-style coordination events between sub-agents
- workflow self-improvement

Those are reserved for V3+.

## Workspace

The workspace is intentionally small:

```text
agile-agent/
├── cli/
├── core/
├── docs/
└── tui/
```

Package roles:

- `agent-cli`: command-line entrypoints such as `doctor`, `probe`, and headless `run-loop`
- `agent-core`: providers, state models, loop runner, verification, persistence, and artifacts
- `agent-tui`: interactive terminal UI

## Implemented Capabilities

### Providers

- `claude`
- `codex`
- `mock` for local development and tests

The TUI and loop runner reuse provider session continuity when available:

- Claude via `session_id`
- Codex via `thread_id`

### TUI

The TUI provides a codex-style shell interface with:

- viewport + bottom pane layout (not stacked boxed widgets)
- multiline composer with real text editing
- paste support for multiline content
- transcript rendering as typed cells (user, assistant, status, error)
- width-aware Markdown rendering for assistant output
- transcript overlay/pager for browsing full conversation history
- skill browser overlay

Keybindings:

- `Enter` — submit current input
- `Ctrl+J` — insert newline (for multiline input)
- `Tab` — switch provider (claude ↔ codex)
- `Ctrl+T` — open transcript overlay/pager
- `Ctrl+C` — quit
- `q` — quit (when composer is empty)
- `$` — open skill browser (when composer is empty)
- `Up/Down` — scroll transcript when the composer is empty
- `PageUp/PageDown` — page transcript in the main shell
- `Home/End` — jump to transcript top/bottom when the composer is empty

Transcript overlay controls:

- `Esc` or `q` — close overlay
- `Up/Down` — scroll
- `PageUp/PageDown` or `Space` — page
- `Home/End` — jump to top/bottom

Slash commands:

- `/help`
- `/provider`
- `/skills`
- `/doctor`
- `/backlog`
- `/todo-add <title>`
- `/run-once`
- `/run-loop`
- `/quit`

### Autonomous Loop

The V2 autonomous loop currently provides:

1. persisted backlog and task state
2. todo-to-task generation
3. automatic continuation for unfinished turns
4. completion judgment before verification
5. verification execution
6. distinct terminal outcomes:
   - `done`
   - `failed`
   - `blocked`
7. escalation artifacts for blocked work
8. structured task artifacts for completed and failed work
9. guardrails for:
   - max iterations
   - max continuations per task
   - max verification failures
10. recent-session and recent-loop continuity

## Build and Run

Requirements:

- Rust toolchain
- at least one real provider installed for TUI or real autonomous execution:
  - `claude`
  - `codex`

Build:

```bash
cargo build
```

Run the TUI:

```bash
cargo run -p agent-cli
```

Resume the most recent TUI session:

```bash
cargo run -p agent-cli -- resume-last
```

Check provider availability:

```bash
cargo run -p agent-cli -- doctor
```

Print structured probe output:

```bash
cargo run -p agent-cli -- probe --json
```

Run the autonomous loop headlessly:

```bash
cargo run -p agent-cli -- run-loop --max-iterations 5
```

Resume the most recent loop/session state in headless mode:

```bash
cargo run -p agent-cli -- run-loop --max-iterations 5 --resume-last
```

Run tests:

```bash
cargo test
cargo check
```

## Persistence and Artifacts

`agile-agent` stores local state under the platform-local data directory returned by `dirs::data_local_dir()`.

Current persisted data includes:

- `backlog.json`
- recent sessions
- escalation artifacts
- structured task artifacts

Structured task artifacts are written for successful and failed execution paths so later review can inspect:

- task objective
- provider used
- assistant summary
- verification result
- failure or escalation reason

## Documentation

Planning documents live in two places:

- repo-level implementation specs in `docs/plan/spec/`
- project-level phase planning in the sibling workspace docs directory:
  - `../docs/agile-agent-v1-requirements-plan-zh.md`
  - `../docs/agile-agent-v2-requirements-plan-zh.md`
  - `../docs/agile-agent-ideas-zh.md`

## Current Boundaries

This project is still intentionally narrow.

It is not yet:

- a multi-agent coordination system
- a Scrum automation engine
- a self-improving workflow platform
- a full project-management system

Its current product thesis is simpler:

- build a reliable single-agent execution substrate first
- prove that one autonomous engineering loop can work
- then scale to multi-agent coordination later
