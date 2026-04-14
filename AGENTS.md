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
- Git commit messages must be clear, concise, when u finished a task, commit changes then keep workspace clean.

## Multi-Agent Architecture

The multi-agent foundation provides Scrum-style coordination:

### Key Modules

- `core/src/agent_role.rs`: AgentRole enum (ProductOwner, ScrumMaster, Developer) with role-specific behaviors
- `core/src/runtime_mode.rs`: RuntimeMode enum for backward compatibility (SingleAgent, MultiAgent)
- `core/src/sprint_planning.rs`: SprintPlanningSession for ProductOwner sprint planning
- `core/src/standup_report.rs`: DailyStandupReport for daily status generation
- `core/src/blocker_escalation.rs`: BlockerEscalation for ScrumMaster blocker resolution
- `core/src/data_migration.rs`: DataMigrator for converting legacy single-agent data

### Design Principles

1. **Backward Compatibility**: RuntimeMode defaults to SingleAgent, preserving existing behavior
2. **Role-Based Coordination**: Each role has specific focus, skills, and prompt prefixes
3. **Foundation First**: Sprint 10-11 implements foundational Scrum concepts; advanced lifecycle (ScrumEvent, RolePermissions) is future work

### CLI Commands

```bash
# List all agents
agent list --all

# Show agent status
agent status <agent-id>

# Spawn new agent (placeholder for future)
agent spawn <provider>

# Stop agent (placeholder for future)
agent stop <agent-id>

# Run with multi-agent flag (future implementation)
run-loop --multi-agent
```

## Index

- `README.md`: project overview, scope, runtime model, and developer entrypoints.
- `Cargo.toml`: workspace manifest for the Rust crates.
- `cli/`: `agent-cli` crate, binary entrypoints and CLI-facing integration tests.
- `core/`: `agent-core` crate, providers, runtime loop, persistence, backlog/task state, and verification logic.
- `tui/`: `agent-tui` crate, terminal UI, rendering, transcript, composer, and interaction flow.
- `test-support/`: shared test helpers for workspace crates.
- `kanban/`: `agent-kanban` crate, trait-based Kanban domain model.
- `docs/plan/spec/`: implementation-facing product and sprint specs.
- `docs/plan/spec/multi-agent/`: multi-agent sprint specs (sprint-01 through sprint-11).
- `docs/superpowers/specs/`: design specs written through the superpowers workflow.
- `docs/superpowers/plans/`: implementation plans written through the superpowers workflow.
- `scripts/coverage.sh`: local coverage helper script.
- `target/`: build artifacts and generated output; not a source directory.
