# agile-agent

`agile-agent` is a Rust workspace for building a local autonomous engineering agent on top of existing coding CLIs such as `claude` and `codex`.

It combines a codex-style terminal UI, persisted workplace and agent state, a headless execution loop, a decision layer for ambiguous provider output, and multi-agent coordination with git worktree isolation.

## Status

Implemented:

- interactive TUI substrate with transcript/composer flow, tool rendering, and session restore
- single-agent runtime identity, workplace persistence, and headless autonomous loop
- local skill discovery and structured prompt injection
- trait-based Kanban domain model with shared test support
- multi-agent foundation with role-aware agents and Scrum-style coordination primitives
- decision-layer foundation in `agent-decision` with classifier and engine building blocks
- git worktree isolation for multi-agent development
- launch configuration overlay for agent creation (Ctrl+N)
- Overview mode for multi-agent activity monitoring

In progress:

- decision-layer runtime integration and persistent human-decision workflows
- full parallel multi-agent runtime across TUI and headless execution
- OpenAI-backed LLM provider integration for decision support

Not started yet:

- workflow self-improvement

## Workspace

```text
agile-agent/
├── cli/           # `agile-agent` binary and CLI integration tests
├── core/          # runtime, providers, persistence, backlog, verification
├── decision/      # decision layer, classifiers, engines, human-decision types
├── docs/          # product specs, sprint specs, and superpowers artifacts
├── kanban/        # trait-based Kanban domain model
├── llm-provider/  # OpenAI client/provider abstraction for internal use
├── scripts/       # developer helper scripts
├── test-support/  # shared test harnesses and fixtures
└── tui/           # terminal UI, rendering, transcript, overlays, command flow
```

Workspace crates:

- `agent-cli`: `agile-agent` binary, TUI entrypoint, and CLI subcommands
- `agent-core`: providers, backlog/task models, runtime loop, persistence, verification, and artifacts
- `agent-decision`: classifier, tiered decision engine, action/situation registry, and human decision types
- `agent-tui`: codex-style terminal UI and slash-command routing
- `agent-kanban`: extensible Kanban domain model with trait and registry architecture
- `agent-llm-provider`: OpenAI client and provider abstraction used by decision-layer work
- `agent-test-support`: shared test helpers for workspace crates

Key docs:

- `docs/plan/spec/`: implementation-facing product and sprint specs
- `docs/plan/spec/decision-layer/`: decision-layer architecture and sprint specs
- `docs/plan/spec/multi-agent/`: multi-agent sprint specs
- `docs/superpowers/specs/`: superpowers design specs
- `docs/superpowers/plans/`: superpowers implementation plans

## Features

### Providers

- `claude` via a stream-JSON bridge
- `codex` via an app-server stdio bridge
- `mock` for local development, tests, and headless fallback

**Provider Profiles**: Configure different LLM backends using named profiles. See [docs/profile-configuration.md](docs/profile-configuration.md) for details.

Default provider selection order:

1. `claude`
2. `codex`
3. `mock`

Runtime mode constraints:

- the TUI requires at least one real provider (`claude` or `codex`) and exits early otherwise
- headless `run-loop` can still bootstrap with `mock` for local development and tests

Session continuity is reused when available:

- Claude via `session_id`
- Codex via `thread_id`

### CLI And Execution Modes

- interactive TUI via `cargo run -p agent-cli`
- TUI resume flow via `cargo run -p agent-cli -- resume-last`
- headless autonomous loop via `cargo run -p agent-cli -- run-loop ...`
- agent and workplace inspection via `cargo run -p agent-cli -- agent ...` and `cargo run -p agent-cli -- workplace current`
- decision request inspection via `cargo run -p agent-cli -- decision ...`
- profile management via `cargo run -p agent-cli -- profile list [--verbose]`
- structured environment probing via `cargo run -p agent-cli -- doctor` and `cargo run -p agent-cli -- probe --json`

Current CLI limitations:

- `agent stop` only records a stop request; fully stopping a live agent still requires the TUI session
- `decision` commands are present, but the end-to-end persisted decision workflow is still being wired through

### TUI

The TUI provides:

- a codex-style transcript and composer layout
- multiline editing and paste support
- transcript paging and overlay browsing
- width-aware Markdown rendering for assistant output
- tool output rendering for exec, web, image, and patch events
- patch summaries with file change statistics
- a local skill browser
- multi-agent session state where provider switching creates a new agent identity
- current agent identity in the footer
- Overview mode for monitoring all agents at a glance
- launch configuration overlay for customizing new agents (Ctrl+N)
- git worktree isolation for parallel agent work

Common keybindings:

- `Enter`: submit
- `Ctrl+J`: newline
- `Tab`: cycle to next agent (when multiple agents exist)
- `Ctrl+N`: open launch config overlay to create a new agent
- `Ctrl+P`: toggle/switch provider
- `Ctrl+T`: open transcript overlay
- `$`: open skill browser when the composer is empty
- `Ctrl+C`: quit (or close overlay when open)
- `Alt+1-6`: switch view modes (Focused, Split, Dashboard, Mail, Skills, Overview)
- `Alt+V`: cycle view modes

Preferred namespaced slash commands:

- `/local help`
- `/local status`
- `/local kanban list`
- `/local config get <key>`
- `/local config set <key> <value>`
- `/agent status`
- `/agent <target> status`
- `/agent summary`
- `/provider /status`
- `/provider <target> /status`

Legacy flat commands still accepted:

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

### Decision Layer

The `agent-decision` crate currently includes:

- provider-output classification by situation type
- rule-based, LLM-backed, CLI, mock, and tiered decision engines
- action and situation registries with built-in cases such as waiting-for-choice, claims-completion, partial-completion, and error recovery
- human decision request and response types for escalation flows

Current limitation:

- the decision-layer crate is real and used by ongoing integration work, but the top-level CLI and TUI decision UX are still partial

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

### Agent Runtime

The runtime treats one `agent` as a first-class object:

- one agent maps to one long-lived runtime identity
- one agent binds to one provider type
- one agent reuses one provider session continuity when available
- switching provider in the TUI creates a new agent instead of mutating the current one

The runtime currently persists:

- `agent_id`
- `codename`
- `workplace_id`
- `provider_type`
- `provider_session_id`
- `created_at`
- `updated_at`
- `status`

On startup from the same working directory, `agile-agent` restores the most recent agent for the derived workplace and reattaches provider session continuity when possible.
When `resume-last` is used, it prefers the current agent's own `state.json` before falling back to older workplace-scoped session files.

### Launch Configuration

When creating a new agent (via Ctrl+N), the launch configuration overlay allows:

- selecting the target provider (claude, codex, or mock)
- configuring environment variable overrides (KEY=VALUE format)
- specifying custom executable paths or command fragments
- previewing parsed configuration before launch

Launch modes:

- `HostDefault`: use the provider's default executable and environment
- `EnvOnly`: use default executable with custom environment variables
- `CommandFragment`: specify full command with executable and arguments

### Git Worktree Isolation

Agents can operate in isolated git worktrees for parallel development:

- each agent can have its own worktree with a dedicated branch
- worktrees are created and managed automatically by the WorktreeManager
- supports pause/resume of agents with worktree state preservation
- enables safe parallel work on different features without conflicts

### Local Skills

Skills are discovered from:

- `<cwd>/.agile-agent/skills`
- `<cwd>/skills`
- the platform config directory under `agile-agent/skills`

Each skill lives in its own directory and must contain a `SKILL.md`. Enabled skills are injected into the next provider prompt as structured context.

## Quick Start

The examples below use `cargo run -p agent-cli -- ...`; the built binary name is `agile-agent`.

Requirements:

- Rust toolchain
- for the TUI: `claude` or `codex` installed and available on `PATH`
- for headless local experimentation: no real provider is required because `run-loop` can fall back to `mock`
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

Inspect the current workplace:

```bash
cargo run -p agent-cli -- workplace current
```

Inspect the current or most recent agent:

```bash
cargo run -p agent-cli -- agent current
```

List known agents in the current workplace, including stopped ones:

```bash
cargo run -p agent-cli -- agent list --all
```

Show detailed status for a specific agent:

```bash
cargo run -p agent-cli -- agent status <agent-id>
```

Spawn a new agent on a specific provider:

```bash
cargo run -p agent-cli -- agent spawn codex
```

Inspect decision requests:

```bash
cargo run -p agent-cli -- decision list
```

Run the autonomous loop headlessly:

```bash
cargo run -p agent-cli -- run-loop --max-iterations 5
```

Resume the most recent saved session before the headless loop starts:

```bash
cargo run -p agent-cli -- run-loop --max-iterations 5 --resume-last
```

Try the experimental multi-agent headless flag:

```bash
cargo run -p agent-cli -- run-loop --max-iterations 5 --multi-agent
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
