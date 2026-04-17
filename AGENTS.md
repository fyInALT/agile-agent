# AGENTS.md

## Vision

`agile-agent` builds local autonomous engineering agents on top of `claude`/`codex` CLIs:
- **Interactive TUI**: Codex-style terminal interface with multi-agent session support and real-time monitoring
- **Autonomous Loop**: Headless task execution, verification, and error recovery
- **Decision Layer**: Tiered decision engine handling ambiguous outputs with human escalation
- **Multi-Agent Coordination**: Scrum-style role coordination with git worktree isolation

## Architecture

Layered architecture: `agent-cli` as the entry point, coordinating `agent-tui` (interactive interface) and `agent-core` (runtime core). `agent-decision` provides decision-layer capabilities, `agent-kanban` provides Kanban domain model, and `agent-llm-provider` provides LLM client abstraction.

## Focus

- `agile-agent` is the primary implementation target in this workspace.
- Keep this file index-oriented. Add process detail elsewhere only when necessary.

## Engineering Rules

- Development must be TDD-first. Start from a failing test, make it pass, then refactor.
- Every code change must include thorough automated tests. Do not leave new behavior, edge cases, or regressions untested.
- Do not take on technical debt in technical decisions. Prefer clear architecture, explicit boundaries, and durable implementations over shortcuts.
- During every planning step, explicitly re-evaluate whether the current architecture and module boundaries are still the right fit.
- After every completed task, explicitly check whether all requirements were fully delivered and whether there is any worthwhile improvement to make before closing the work.
- Git commit messages must be clear, concise, when u finished a task, commit changes then keep workspace clean.

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

## Index

### Root

- `README.md`: Project overview, features, quick start, and CLI reference
- `Cargo.toml`: Workspace manifest for Rust crates

### Crates

- `cli/`: `agent-cli` crate — binary entrypoints and CLI-facing integration tests
- `core/`: `agent-core` crate — runtime, providers, persistence, backlog, verification
- `tui/`: `agent-tui` crate — terminal UI, rendering, transcript, composer, overlays
- `decision/`: `agent-decision` crate — classifiers, engines, actions, situations
- `kanban/`: `agent-kanban` crate — trait-based Kanban domain model
- `llm-provider/`: `agent-llm-provider` crate — OpenAI client/provider abstraction
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

### Scripts

- `scripts/coverage.sh`: Local coverage helper script
