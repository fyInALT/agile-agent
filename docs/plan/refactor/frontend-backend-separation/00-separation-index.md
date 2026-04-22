# Implementation Documentation Index

> Status: Draft  
> Date: 2026-04-20  
> Scope: Concrete implementation specs for TUI-backend separation

This directory contains the detailed implementation specifications for the architectural refactoring described in `../architecture-blueprint.md`. Every document here is **code-facing**: it discusses concrete types, module boundaries, function signatures, and migration strategies.

---

## Reading Order

These documents have dependencies. Read in this order:

| # | Document | Depends On | What It Covers |
|---|----------|-----------|----------------|
| 1 | [01-protocol-spec.md](./01-protocol-spec.md) | architecture-blueprint §5 | JSON-RPC 2.0 message schema, method namespaces, error codes, lifecycle |
| 2 | [02-agent-protocol-crate.md](./02-agent-protocol-crate.md) | 01-protocol-spec | `agent-protocol` crate: types, serialization, version negotiation |
| 3 | [03-agent-daemon-architecture.md](./03-agent-daemon-architecture.md) | 02-agent-protocol-crate | `agent-daemon` binary: modules, startup, WebSocket server, session manager |
| 4 | [04-state-migration.md](./04-state-migration.md) | 03-agent-daemon-architecture | Moving `RuntimeSession`, `AgentPool`, `EventAggregator`, `Mailbox` into daemon |
| 5 | [05-event-streaming.md](./05-event-streaming.md) | 04-state-migration | `ProviderEvent` → `Event` conversion, broadcast, replay, ordering guarantees |
| 6 | [06-tui-refactor.md](./06-tui-refactor.md) | 05-event-streaming | Removing all `agent_core` imports from TUI, WebSocket client, render state |
| 7 | [07-cli-refactor.md](./07-cli-refactor.md) | 06-tui-refactor | CLI as independent `agent-protocol` client, daemon lifecycle commands |
| 8 | [08-testing-strategy.md](./08-testing-strategy.md) | 03–07 | In-memory WebSocket harness, daemon test fixtures, contract tests |
| 9 | [09-migration-plan.md](./09-migration-plan.md) | all above | Backward-compatible rollout, embedded-mode deprecation, cutover criteria |
| 10 | [10-separation-gaps-and-supplements.md](./10-separation-gaps-and-supplements.md) | all above | Systematic gap analysis: observability, limits, security, config, recovery, platform |

## Supplementary Sprint Specs

The following sprint specs were added during gap analysis to cover operational, security, and cross-platform concerns:

| Sprint | Document | What It Covers |
|--------|----------|---------------|
| Sprint 12 | [sprint-12-observability-operational-readiness.md](../../plan/spec/frontend-backend-separation/sprint-12-observability-operational-readiness.md) | Structured logging, Prometheus metrics, health checks, resource limits, config management |
| Sprint 13 | [sprint-13-security-platform.md](../../plan/spec/frontend-backend-separation/sprint-13-security-platform.md) | Origin validation, token auth, audit logs, disaster recovery, Windows/macOS/Linux support |

---

## Document Conventions

### Code Blocks

- `rust` blocks are **aspirational** — they show the target API, not necessarily the exact current naming. They compile in the reader's head, not in `rustc`.
- `json` blocks are **normative** — these are the exact wire formats. Any deviation is a bug.
- `text` blocks are **illustrative** — directory trees, sequence diagrams, etc.

### Status Markers

Each spec section may include a status marker:

- `📐 DESIGN` — Still under design, multiple options under consideration
- `✅ DECIDED` — Design finalized, ready for implementation
- `🔄 REVIEW` — Implemented but may need revision based on feedback

### Cross-References

References to the architecture blueprint use the format `AB §X.Y` (e.g., `AB §2.1` = architecture blueprint, section 2.1).

References to other implementation docs use the format `IMP §X.Y` with the document number prefix (e.g., `IMP-01 §3.2` = protocol spec, section 3.2).

---

## Design Principles (Reiterated)

These principles govern every decision in this document set:

1. **Protocol is the contract** — The JSON-RPC schema is the single source of truth. Daemon and clients are implemented against it, not against each other.
2. **No technical debt** — If a design decision creates a future problem, we redesign now. No "we'll fix it later".
3. **Testability by design** — Every module must be unit-testable without a full daemon. Use traits, in-memory transports, and dependency injection.
4. **Fail fast, fail clear** — Invalid messages produce precise JSON-RPC errors. Ambiguous states are escalated, not papered over.
5. **Zero-downtime migration** — Existing users must not be broken. Embedded mode stays functional until the full cutover.

---

## Glossary

| Term | Definition |
|------|-----------|
| **Daemon** | The `agent-daemon` process that owns runtime state and serves WebSocket connections |
| **Client** | Any process speaking the protocol: TUI, CLI, IDE plugin, etc. |
| **Workplace** | A working directory identified by a stable UUID, with its own daemon |
| **Session** | A logical agent session within a workplace, identified by a UUID + optional alias |
| **Snapshot** | A complete `SessionState` sent on connect or on explicit request |
| **Event** | An incremental state change sent as a JSON-RPC Notification |
| **Auto-link** | The process by which a client discovers and connects to its workplace's daemon |
