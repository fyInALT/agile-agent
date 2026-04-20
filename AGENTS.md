# AGENTS.md

## Vision

`agile-agent` builds local autonomous engineering agents on top of `claude`/`codex` CLIs:
- **Interactive TUI**: Codex-style terminal interface with multi-agent session support and real-time monitoring
- **Autonomous Loop**: Headless task execution, verification, and error recovery
- **Decision Layer**: Tiered decision engine handling ambiguous outputs with human escalation
- **Multi-Agent Coordination**: Scrum-style role coordination with git worktree isolation

## Architecture

Layered architecture: `agent-daemon` owns all runtime state; `agent-cli` and `agent-tui` are thin protocol clients. `agent-protocol` defines the JSON-RPC 2.0 + WebSocket contract. `agent-core` provides the runtime engine (AgentPool, RuntimeSession, EventAggregator). `agent-decision` provides decision-layer capabilities, `agent-kanban` provides Kanban domain model, and `agent-llm-provider` provides LLM client abstraction.

### Frontend-Backend Separation

- **Daemon** (`agent-daemon`): Owns `RuntimeSession`, `AgentPool`, and all provider channels. Serves snapshots and broadcasts events over WebSocket.
- **Protocol** (`agent-protocol`): JSON-RPC 2.0 envelopes, event payloads, state snapshots, `daemon.json` format, and shared client-side auto-link logic.
- **TUI** (`agent-tui`): Protocol-only render state (`ProtocolState`). Embeddable mode still available via `core` feature.
- **CLI** (`agent-cli`): Protocol-only headless client. Embeddable mode still available via `core` feature.

## Focus

- `agile-agent` is the primary implementation target in this workspace.
- Keep this file index-oriented. Add process detail elsewhere only when necessary.

## Engineering Rules

- Development must be TDD-first. Start from a failing test, make it pass, then refactor.
- Every code change must include thorough automated tests. Do not leave new behavior, edge cases, or regressions untested.
- Do not take on technical debt in technical decisions. Prefer clear architecture, explicit boundaries, and durable implementations over shortcuts.
- During every planning step, explicitly re-evaluate whether the current architecture and module boundaries are still the right fit.
- After every completed task, explicitly check whether all requirements were fully delivered and whether there is any worthwhile improvement to make before closing the work.
- All commit messages, PR messages, documentation, comments, and file names must be in **English**.
- Git commit messages must be clear and concise; commit changes and keep the workspace clean when a task is finished.
- Development follows **git-flow** conventions:
  - `main` is the production branch; only merge stable releases into it.
  - Create `feature/<name>` branches from `main` (or `develop` if one exists) for every new feature or refactor.
  - Create `fix/<name>` branches for bug fixes.
  - Keep branches focused and short-lived; rebase onto the latest base branch before opening a PR.
  - Ensure the branch is up to date and all tests pass before merging.

## Multi-Agent Architecture

The multi-agent foundation provides Scrum-style coordination:

### Key Modules (core)

- `agent_pool.rs`: AgentPool managing multiple concurrent agent slots
- `agent_slot.rs`: AgentSlot representing a single agent's runtime state
- `agent_role.rs`: AgentRole enum (ProductOwner, ScrumMaster, Developer)
- `runtime_mode.rs`: RuntimeMode enum for backward compatibility
- `sprint_planning.rs`: SprintPlanningSession for ProductOwner sprint planning
- `standup_report.rs`: DailyStandupReport for daily status generation
- `blocker_escalation.rs`: BlockerEscalation for ScrumMaster blocker resolution
- `worktree_manager.rs`: Git worktree isolation for parallel agent work
- `decision_agent_slot.rs`: Decision-layer integration for agent slots

### Key Modules (decision)

- `tiered_engine.rs`: Tiered decision engine (rule → LLM → human escalation)
- `blocking.rs`: Blocking decision workflows with human intervention
- `concurrent.rs`: Concurrent decision processing
- `context.rs`: Decision context aggregation
- `recovery.rs`: Error recovery decision handling
- `builtin_*.rs`: Built-in actions and situations

### Design Principles

1. **Backward Compatibility**: RuntimeMode defaults to SingleAgent, preserving existing behavior
2. **Role-Based Coordination**: Each role has specific focus, skills, and prompt prefixes
3. **Worktree Isolation**: Agents operate in isolated git worktrees for conflict-free parallel work
4. **Decision Escalation**: Tiered engine escalates ambiguous cases to human intervention
5. **Protocol-First**: TUI and CLI are thin clients; all runtime state lives in the daemon
6. **Feature-Gated Decoupling**: `agent-tui` and `agent-cli` have `core` features for backward compatibility during transition

## Index

### Root

- `README.md`: Project overview, features, quick start, and CLI reference
- `Cargo.toml`: Workspace manifest for Rust crates

### Crates

- `cli/`: `agent-cli` crate — binary entrypoints and CLI-facing integration tests (protocol-first, `core` feature for legacy mode)
- `core/`: `agent-core` crate — runtime, providers, persistence, backlog, verification
- `tui/`: `agent-tui` crate — terminal UI, rendering, transcript, composer, overlays (protocol-first, `core` feature for legacy mode)
- `decision/`: `agent-decision` crate — classifiers, engines, actions, situations
- `kanban/`: `agent-kanban` crate — trait-based Kanban domain model
- `llm-provider/`: `agent-llm-provider` crate — OpenAI client/provider abstraction
- `agent/daemon/`: `agent-daemon` crate — WebSocket server, session manager, event pump, broadcaster
- `agent/protocol/`: `agent-protocol` crate — JSON-RPC types, events, snapshots, auto-link, config
- `agent/commands/`: `agent-commands` crate — command bus and slash command system
- `test-support/`: `agent-test-support` crate — shared test helpers

### Documentation

- `docs/plan/spec/`: Implementation-facing sprint specs
- `docs/plan/spec/multi-agent/`: Multi-agent sprint specs (sprint-01 through sprint-11)
- `docs/plan/spec/decision-layer/`: Decision-layer architecture and sprint specs
- `docs/plan/spec/launch-config/`: Launch configuration sprint specs
- `docs/plan/spec/worktree/`: Git worktree isolation sprint specs
- `docs/plan/spec/kanban/`: Kanban system sprint specs
- `docs/superpowers/specs/`: Design specs written through superpowers workflow
- `docs/superpowers/plans/`: Implementation plans written through superpowers workflow
- `docs/refactor/`: Refactoring analysis and architectural decision records

### Scripts

- `scripts/coverage.sh`: Local coverage helper script
