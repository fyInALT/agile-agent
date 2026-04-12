# Agent Runtime Phase 1 Agent Identity Spec

## Metadata

- Sprint: `Agent Runtime Phase 1 / Sprint 1`
- Stories covered:
  - `AR1-S01`
  - `AR1-S02`

## 1. Purpose

Sprint 1 establishes `agent` as a first-class runtime object and persists its meta information
under a canonical workplace path.

This sprint is the identity substrate. It does not attempt restore yet.

## 2. Scope

### In scope

- `AgentMeta` model
- `AgentRuntime` in-memory identity holder
- `WorkplaceStore` path resolution
- `AgentStore` meta persistence
- shutdown-time meta flush

### Out of scope

- restore flows
- provider session reattach
- operator-facing agent identity UI
- broken-state recovery UX

## 3. Sprint Goal

The system can create one formal agent identity and write its meta to a canonical workplace path.

## 4. Product Decisions

- `agent` is a formal runtime concept, not implicit app state
- workplace roots live under `~/.agile-agent/workplaces`
- JSON uses `snake_case`
- the first persisted file is `meta.json`

## 5. Detailed Execution Checklist

## S1-T01 Add core identity types

- add `AgentId`
- add `WorkplaceId`
- add `AgentCodename`
- add `AgentStatus`
- add `ProviderType`
- add `ProviderSessionId`
- add `AgentMeta`

## S1-T02 Add workplace path resolution

- resolve the platform root for `~/.agile-agent`
- resolve `workplaces/{workplace_id}`
- create directories when missing

## S1-T03 Add agent meta store

- resolve `agents/{agent_id}`
- write `meta.json`
- read `meta.json`

## S1-T04 Add in-memory agent runtime

- create a current `AgentRuntime`
- hold identity fields in memory
- expose provider binding accessors

## S1-T05 Wire shutdown meta persistence

- flush current agent meta on TUI shutdown
- flush current agent meta on headless shutdown

## 6. Acceptance

Sprint 1 is done when:

1. the system can create a formal agent identity
2. `meta.json` is written under `workplaces/{workplace_id}/agents/{agent_id}`
3. the JSON shape is stable and readable
4. shutdown persists the current meta

## 7. Test Plan

- identity model serialization tests
- workplace path resolution tests
- agent store save/load tests
- shutdown persistence smoke

## 8. Review Demo

1. launch the app in a workspace path
2. show the created workplace/agent directories
3. show the generated `meta.json`
4. close the app and confirm meta is still present
