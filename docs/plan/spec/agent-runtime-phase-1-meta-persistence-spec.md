# Agent Runtime Phase 1 Meta Persistence Spec

## Metadata

- Scope: `agent runtime` phase 1
- Language: English by spec-folder policy
- Primary references:
  - `../docs/agile-agent-agent-runtime-ideas-zh.md`
  - `../docs/agile-agent-terminology-glossary-zh.md`

## 1. Purpose

Phase 1 introduces `agent` as a first-class runtime object.

The purpose of this phase is not to build a full multi-agent system.
The purpose is to establish one durable execution unit that:

- has a stable identity
- is bound to one provider type
- is bound to one provider session
- persists minimal meta information on shutdown
- can be restored from disk on the next startup

## 2. Scope

### In scope

- canonical `agent` runtime types in core
- canonical `workplace` path layout
- `AgentMeta` persistence as JSON
- shutdown-time meta flush
- startup-time agent restore
- restore of provider type and provider session id

### Out of scope

- `memory.json`
- `messages.json`
- full `state.json`
- multi-agent orchestration
- coordinator/worker/reviewer role scheduling
- meeting/workflow systems

## 3. Naming Decisions

This spec assumes these canonical names:

- `agent`
- `workplace`
- `provider_type`
- `provider_session_id`
- `meta.json`

Do not use `workspace` for the top-level project container in this feature.

## 4. Storage Layout

Phase 1 should use this layout:

```text
~/.agile-agent/
└── workplaces/
    └── {workplace_id}/
        └── agents/
            └── {agent_id}/
                └── meta.json
```

The implementation may later add more files under the same directories, but Phase 1 only requires
`meta.json`.

## 5. Required Runtime Types

Add core-layer types for:

- `AgentId`
- `WorkplaceId`
- `AgentCodename`
- `AgentStatus`
- `ProviderType`
- `ProviderSessionId`
- `AgentMeta`
- `AgentRuntime`

Suggested shape:

```rust
pub struct AgentMeta {
    pub agent_id: String,
    pub codename: String,
    pub workplace_id: String,
    pub provider_type: ProviderType,
    pub provider_session_id: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub status: AgentStatus,
}
```

`provider_session_id` may start as optional because the runtime can exist before the provider has
returned a resumable session handle.

## 6. Required Store Types

Add core-layer store support for:

- `WorkplaceStore`
- `AgentStore`

Responsibilities:

### `WorkplaceStore`

- resolve `~/.agile-agent/workplaces/{workplace_id}`
- create directories when missing

### `AgentStore`

- resolve `agents/{agent_id}`
- write `meta.json`
- read `meta.json`
- locate the current or most recent agent for a workplace

## 7. Agent Runtime Semantics

`AgentRuntime` should be the owner of:

- stable agent identity
- provider type
- provider session binding
- meta timestamps

It should not own:

- full transcript persistence
- full message bus persistence
- full loop persistence

Those belong to later phases.

## 8. Provider Binding Rules

One agent runtime must be bound to one provider type.

Examples:

- one agent runtime bound to `codex`
- one agent runtime bound to `claude`

That binding should be treated as stable identity, not a casual UI toggle.

Phase 1 may keep the current UI provider switch alive temporarily, but the runtime/store layer must
be designed so provider binding is recorded as part of agent identity.

## 9. Lifecycle

### First startup

1. resolve workplace path
2. create a new agent id if no existing agent is selected
3. assign codename
4. assign provider type
5. start provider interaction
6. record provider session id when available
7. write `meta.json`

### Running

- refresh `updated_at` on meaningful runtime state changes
- refresh `provider_session_id` when a provider returns a resumable handle

### Shutdown

- flush `meta.json`

### Restart

1. resolve workplace path
2. load the previous agent meta
3. restore provider type
4. restore provider session id
5. reuse that session binding on the next provider request

## 10. Integration Points

This work will touch at least these areas:

- core session persistence
- provider session handle storage
- app startup / restore path
- app shutdown path
- CLI/TUI startup entrypoints

Likely modules:

- `core/src/agent_runtime.rs`
- `core/src/agent_store.rs`
- `core/src/workplace_store.rs`
- `core/src/session_store.rs`
- `cli/src/main.rs`
- `tui/src/app_loop.rs`

Exact file names may vary, but the responsibilities must remain clear.

## 11. JSON Contract

Use `snake_case` keys.

Phase 1 required keys:

- `agent_id`
- `codename`
- `workplace_id`
- `provider_type`
- `provider_session_id`
- `created_at`
- `updated_at`
- `status`

Avoid:

- `agentId`
- `providerSessionId`
- `provider` when the field specifically means provider type

## 12. Detailed Execution Checklist

## T1 Add core naming and model types

- add `AgentMeta`
- add `AgentStatus`
- add `ProviderType`
- add id wrapper types or stable string aliases

Acceptance:

- types compile
- JSON serialization shape is stable

## T2 Add workplace path resolution

- resolve `~/.agile-agent/workplaces`
- resolve `{workplace_id}`
- create missing directories

Acceptance:

- tests can create and resolve temporary workplace roots

## T3 Add agent store

- write `meta.json`
- read `meta.json`
- detect missing agent meta cleanly

Acceptance:

- save/load tests pass

## T4 Add in-memory agent runtime

- hold current agent meta in memory
- expose provider binding and session binding accessors
- update `updated_at`

Acceptance:

- runtime can exist before provider session id is known
- runtime can update provider session id later

## T5 Integrate provider session updates

- when current provider returns a resumable handle, update the agent runtime
- map:
  - Claude -> `session_id`
  - Codex -> `thread_id`

Acceptance:

- agent meta reflects the current resumable provider session id

## T6 Flush meta on shutdown

- TUI exit path flushes current agent meta
- headless exit path flushes current agent meta

Acceptance:

- closing the app writes `meta.json`

## T7 Restore runtime on startup

- load prior agent meta from the same workplace
- rebind provider type
- rebind provider session id

Acceptance:

- restart continues on the prior provider session binding

## T8 Add tests

- model serialization tests
- workplace path resolution tests
- agent store save/load tests
- provider session rebinding tests
- startup/restore tests where practical

## 13. Acceptance Criteria

Phase 1 is complete when:

1. the system has a first-class `agent` runtime concept
2. `meta.json` is persisted under the `workplaces/{workplace_id}/agents/{agent_id}` layout
3. shutdown flushes agent meta
4. restart restores the prior agent meta
5. provider type and provider session id are restored correctly
6. the JSON contract is readable and manually inspectable
7. `cargo test` and `cargo check` pass

## 14. Non-Goals

This phase explicitly does not require:

- agent memory persistence
- agent message queues
- multi-agent messaging
- agent meetings
- workflow persistence
- role scheduling

## 15. Suggested Commit Shape

Suggested commit order:

1. `docs: add agent runtime phase 1 spec`
2. `feat: add agent meta model and store`
3. `feat: persist agent meta on shutdown`
4. `feat: restore agent runtime from workplace state`
5. `test: cover agent runtime meta persistence`
