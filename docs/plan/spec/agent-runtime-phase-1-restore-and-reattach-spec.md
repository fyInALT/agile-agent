# Agent Runtime Phase 1 Restore and Reattach Spec

## Metadata

- Sprint: `Agent Runtime Phase 1 / Sprint 2`
- Stories covered:
  - `AR1-S03`
  - `AR1-S04`

## 1. Purpose

Sprint 2 upgrades the persisted identity shell into a real runtime continuity feature:

- restore the previous agent from the same workplace
- restore provider type and provider session binding
- reuse that session on the next provider turn

## 2. Scope

### In scope

- locate the most recent agent for a workplace
- restore `AgentRuntime` from persisted meta
- restore provider type
- restore provider session id
- reuse restored provider session on the next request

### Out of scope

- multi-agent selection
- agent list UI
- damaged-state UX polish beyond clear errors

## 3. Sprint Goal

Restarting from the same workplace restores the prior agent and reattaches to the same provider session continuity.

## 4. Product Decisions

- restore operates at workplace scope
- “same workplace” is the continuity boundary
- provider session continuity belongs to the agent, not to transient UI state

## 5. Detailed Execution Checklist

## S2-T01 Add latest-agent lookup for a workplace

- locate the most recent or current agent meta under one workplace

## S2-T02 Restore agent runtime on startup

- load persisted agent meta
- rebuild `AgentRuntime`

## S2-T03 Rebind provider type

- restored runtime must set the active provider type before the next turn

## S2-T04 Rebind provider session continuity

- Claude -> restored `session_id`
- Codex -> restored `thread_id`

## S2-T05 Reuse restored session on the next provider request

- confirm the provider request path uses the restored session binding instead of starting a new one

## 6. Acceptance

Sprint 2 is done when:

1. the most recent agent can be restored from the same workplace
2. the provider type is restored correctly
3. the provider session binding is restored correctly
4. the next provider request reuses the restored session continuity

## 7. Test Plan

- agent lookup tests
- restore runtime tests
- provider session rebinding tests
- Claude/Codex continuity smoke tests where practical

## 8. Review Demo

1. start an agent and obtain a provider session id
2. shut down the app
3. restart from the same workplace
4. show that the restored runtime uses the prior provider session
