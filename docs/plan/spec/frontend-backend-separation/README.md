# Frontend-Backend Separation Sprint Plan

> Status: Planned  
> Date: 2026-04-20  
> Scope: Complete Scrum sprint breakdown for TUI-backend separation refactor

This directory contains the sprint-by-sprint execution plan for the frontend-backend separation refactor. Each sprint is approximately 2 weeks and follows Scrum agile principles.

## Sprint Overview

| Sprint | Title | Goal | Effort |
|--------|-------|------|--------|
| [Sprint 1](./sprint-01-protocol-foundation.md) | Protocol Foundation | `agent-protocol` crate: all JSON-RPC types, events, states | 11 pts |
| [Sprint 2](./sprint-02-daemon-skeleton.md) | Daemon Skeleton | WebSocket server, connection handler, router | 12 pts |
| [Sprint 3](./sprint-03-auto-link-lifecycle.md) | Auto-Link + Lifecycle | Daemon startup/shutdown, auto-discovery, heartbeat | 11 pts |
| [Sprint 4](./sprint-04-session-manager-snapshot.md) | SessionManager + Snapshot | Runtime state ownership moves to daemon | 11 pts |
| [Sprint 5](./sprint-05-agent-lifecycle-event-pump.md) | Agent Lifecycle + Event Pump | Protocol agent ops, ProviderEvent→Event conversion | 10 pts |
| [Sprint 6](./sprint-06-event-broadcast-persistence.md) | Event Broadcast + Persistence | Multi-client broadcast, events.jsonl, gap recovery | 10 pts |
| [Sprint 7](./sprint-07-tui-client-event-handler.md) | TUI Client + Event Handler | TUI connects to daemon, renders from events | 10 pts |
| [Sprint 8](./sprint-08-tui-decoupling.md) | TUI Decoupling | Remove all `agent_core` imports from TUI | 9 pts |
| [Sprint 9](./sprint-09-cli-refactor.md) | CLI Refactor | CLI becomes independent protocol client | 10 pts |
| [Sprint 10](./sprint-10-hardening.md) | Hardening | Reconnect, approval flow, error handling | 9 pts |
| [Sprint 11](./sprint-11-cleanup-performance.md) | Cleanup + Performance | Remove legacy code, benchmark, release | 8 pts |
| [Sprint 12](./sprint-12-observability-operational-readiness.md) | Observability & Operational Readiness | Structured logging, metrics, resource limits, configuration | 10 pts |
| [Sprint 13](./sprint-13-security-platform.md) | Security Hardening & Platform Support | Origin validation, audit log, disaster recovery, cross-platform | 9 pts |

**Total**: 26 weeks (≈ 6.5 months), 130 story points

## Gap Analysis

A systematic gap analysis identified critical omissions in the original plan. See the full analysis in:

- [`docs/refactor/frontend-backend-separation/10-separation-gaps-and-supplements.md`](../../../refactor/frontend-backend-separation/10-separation-gaps-and-supplements.md)

Key gaps that required new sprints:

| Gap | Severity | Sprint |
|-----|----------|--------|
| Observability & Diagnostics | P0 | Sprint 12 |
| Resource Limits & Backpressure | P0 | Sprint 12 |
| Configuration Management | P1 | Sprint 12 |
| Security Hardening (v1) | P1 | Sprint 13 |
| Disaster Recovery | P1 | Sprint 13 |
| Multi-Platform Support | P2 | Sprint 13 |
| Protocol Extension Points | P2 | Sprint 13 |

## Design Documents

These sprints are derived from the following design documents:

| Document | Purpose |
|----------|---------|
| [`docs/refactor/architecture-blueprint.md`](../../../refactor/architecture-blueprint.md) | High-level architecture direction |
| [`docs/refactor/frontend-backend-separation/01-separation-protocol-spec.md`](../../../refactor/frontend-backend-separation/01-separation-protocol-spec.md) | JSON-RPC 2.0 wire protocol |
| [`docs/refactor/frontend-backend-separation/02-separation-agent-protocol-crate.md`](../../../refactor/frontend-backend-separation/02-separation-agent-protocol-crate.md) | Protocol crate design |
| [`docs/refactor/frontend-backend-separation/03-separation-agent-daemon-architecture.md`](../../../refactor/frontend-backend-separation/03-separation-agent-daemon-architecture.md) | Daemon internal architecture |
| [`docs/refactor/frontend-backend-separation/04-separation-state-migration.md`](../../../refactor/frontend-backend-separation/04-separation-state-migration.md) | State ownership migration |
| [`docs/refactor/frontend-backend-separation/05-separation-event-streaming.md`](../../../refactor/frontend-backend-separation/05-separation-event-streaming.md) | Event streaming implementation |
| [`docs/refactor/frontend-backend-separation/06-separation-tui-refactor.md`](../../../refactor/frontend-backend-separation/06-separation-tui-refactor.md) | TUI decoupling details |
| [`docs/refactor/frontend-backend-separation/07-separation-cli-refactor.md`](../../../refactor/frontend-backend-separation/07-separation-cli-refactor.md) | CLI peer client refactor |
| [`docs/refactor/frontend-backend-separation/08-separation-testing-strategy.md`](../../../refactor/frontend-backend-separation/08-separation-testing-strategy.md) | Testing strategy |
| [`docs/refactor/frontend-backend-separation/09-separation-migration-plan.md`](../../../refactor/frontend-backend-separation/09-separation-migration-plan.md) | Migration and rollout plan |

## Agile Principles Applied

### INVEST Story Criteria

Every story in this plan satisfies INVEST:

- **Independent**: Each story can be developed and tested in isolation (may depend on prior sprint completion, but not on other stories in the same sprint)
- **Negotiable**: Implementation details (e.g., specific serde attributes, exact error messages) are open to discussion
- **Valuable**: Every story delivers user-visible or system-capability value
- **Estimable**: Stories are sized 1–5 points based on complexity and risk
- **Small**: No story exceeds 5 points; all fit within a 2-week sprint
- **Testable**: Every story has explicit acceptance criteria

### Definition of Done

A story is done when:

1. Code is implemented per acceptance criteria
2. Unit tests pass (target: 90%+ coverage for new code)
3. Integration tests pass where applicable
4. Code review is complete
5. Documentation is updated (if user-facing)
6. No compiler warnings introduced
7. CI passes on the feature branch

### Sprint Boundaries

- **Sprint Planning**: Monday morning, 2 hours
- **Daily Standup**: 15 minutes, async via text
- **Sprint Review**: Friday afternoon, demo to stakeholders
- **Sprint Retrospective**: Friday afternoon, 1 hour

### Risk Management

Each sprint document contains a risk table with probability, impact, and mitigation. High-probability × high-impact risks are tracked on the project backlog.

## Architecture Summary

The frontend-backend separation is complete. The system now follows a strict daemon-centric architecture:

- **`agent-daemon`** is the sole owner of runtime state. It exposes a WebSocket JSON-RPC 2.0 API on `127.0.0.1` (configurable via `daemon.toml`).
- **`agent-protocol`** defines all wire types: JSON-RPC envelopes, event payloads, state snapshots, and method parameters. It is consumed by both the daemon and all clients.
- **Clients (TUI, CLI, IDE plugins)** are thin protocol consumers. They connect over WebSocket, receive events via broadcast, and send commands via JSON-RPC requests.
- **State isolation** is enforced through `SessionManager`, which owns `AppState`, `AgentPool`, and `EventAggregator`. No client has direct access to `agent-core` types.
- **Persistence** is handled by the daemon: `events.jsonl` for append-only event replay and `snapshot.json` for graceful-shutdown state recovery (with CRC32 checksum validation).
- **Observability** includes Prometheus-compatible metrics via `session.metrics`, structured logging via `tracing`, and a health endpoint via `session.health`.
- **Security** includes localhost-only binding, WebSocket `Origin` header validation, input size limits (`MAX_INPUT_SIZE = 1MB`), and connection limits (default 10).
- **Protocol extensibility** is supported via optional `ext` fields on all JSON-RPC message types and a `capabilities` list returned during `session.initialize`.

### Component Diagram

```
┌─────────┐  ┌─────────┐  ┌─────────┐
│   TUI   │  │   CLI   │  │  IDE    │
│  (tui)  │  │  (cli)  │  │ (future)│
└────┬────┘  └────┬────┘  └────┬────┘
     │            │            │
     └────────────┼────────────┘
                  │ WebSocket / JSON-RPC 2.0
                  ▼
         ┌─────────────────┐
         │  agent-daemon   │
         │  - router       │
         │  - broadcaster  │
         │  - session_mgr  │
         │  - event_log    │
         └────────┬────────┘
                  │
                  ▼
         ┌─────────────────┐
         │   agent-core    │
         │  - AgentPool    │
         │  - AppState     │
         │  - Mailbox      │
         └─────────────────┘
```

### Migration Notes

- The `--embedded-mode` flag and all embedded-mode code paths have been removed. The daemon is mandatory.
- Clients auto-discover the daemon via `daemon.json` written to the workplace directory.
- All breaking changes are documented in sprint specs; no undocumented breaking changes remain.
