# Agent Runtime Phase 1 Safety and Visibility Spec

## Metadata

- Sprint: `Agent Runtime Phase 1 / Sprint 3`
- Stories covered:
  - `AR1-S05`
  - `AR1-S06`
  - `AR1-S07`

## 1. Purpose

Sprint 3 makes the phase-1 runtime operationally credible:

- operators can see which agent is active
- agent meta stays fresh as runtime state changes
- broken workplace state is handled safely

## 2. Scope

### In scope

- operator-visible current agent identity
- refresh of meta timestamps and provider session fields
- clear handling for missing or invalid workplace state

### Out of scope

- full agent list UI
- memory/message persistence
- multi-agent dashboards

## 3. Sprint Goal

The current agent becomes visible, its meta remains trustworthy, and restore failures are explicit and safe.

## 4. Product Decisions

- `meta.json` must be treated as a trusted runtime projection, not a stale bootstrap artifact
- restore failures must never silently degrade into misleading state
- visibility may start in TUI, CLI, or both, but it must be operator-facing

## 5. Detailed Execution Checklist

## S3-T01 Surface current agent identity

- show `agent_id` or `codename`
- show current provider type

## S3-T02 Refresh meta during runtime changes

- update `updated_at`
- update provider session id when the provider returns a new resumable handle
- keep runtime status current

## S3-T03 Add safe restore failure handling

- distinguish “no prior agent exists” from “restore failed”
- report malformed or unreadable meta clearly
- avoid silently pretending a broken restore succeeded

## 6. Acceptance

Sprint 3 is done when:

1. the active agent is visible to operators
2. agent meta updates as runtime state changes
3. missing or broken workplace state is handled safely and clearly

## 7. Test Plan

- meta freshness tests
- malformed meta tests
- restore warning/error tests
- operator-visible identity smoke tests

## 8. Review Demo

1. show the active agent identity in the running app
2. show `updated_at` and provider session id changing after runtime activity
3. corrupt or remove meta and show clear failure handling
