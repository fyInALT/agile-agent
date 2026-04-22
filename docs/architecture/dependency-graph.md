# Dependency Graph

## Current Architecture (Post Sprint 4)

```text
                    ┌─────────────────┐
                    │   agent-cli     │
                    └────────┬────────┘
                             │
                    ┌────────▼────────┐
                    │  agent-daemon   │
                    └────────┬────────┘
                             │
        ┌────────────────────┼────────────────────┐
        │                    │                    │
┌───────▼───────┐   ┌───────▼───────┐   ┌───────▼───────┐
│  agent-core   │   │  agent-worktree│   │  agent-backlog │
└───────┬───────┘   └───────────────┘   └───────────────┘
        │
        │            ┌─────────────────┐
        └────────────►  agent-decision │  ← read-only, no cycles
                     └────────┬────────┘
                              │
                     ┌────────▼────────┐
                     │  agent-events   │  ← shared kernel
                     └────────┬────────┘
                              │
              ┌───────────────┼───────────────┐
              │               │               │
     ┌────────▼──────┐ ┌──────▼──────┐ ┌─────▼──────┐
     │  agent-types  │ │ agent-toolkit│ │ agent-provider│
     └───────────────┘ └─────────────┘ └─────────────┘
```

## Direction Rules

1. **`agent-decision` is read-only**: It consumes `agent-events` and produces
   `DecisionCommand` pure data. It has **no** dependency on `agent-core`,
   `agent-daemon`, or any runtime types.

2. **`agent-events` is the shared kernel**: Only depends on `agent-types` and
   `agent-toolkit`. All other crates may depend on it.

3. **No cycles**: `cargo tree --workspace` confirms zero circular dependencies.

4. **Effect flow**: `DecisionCommand` → `DecisionCommandInterpreter` →
   `RuntimeCommand` → `EffectHandler` → side effects.

## Crate Details

| Crate | Depends On | Used By |
|-------|-----------|---------|
| `agent-types` | serde | all |
| `agent-toolkit` | agent-types, serde | agent-events, agent-provider |
| `agent-events` | agent-types, agent-toolkit | agent-decision, agent-provider, agent-core |
| `agent-provider` | agent-events, agent-toolkit | agent-core |
| `agent-decision` | agent-events, serde | agent-core |
| `agent-core` | agent-events, agent-decision, agent-provider, agent-worktree, agent-backlog, agent-storage | agent-daemon, agent-cli |
| `agent-daemon` | agent-core, agent-protocol | agent-cli |
| `agent-protocol` | serde | agent-daemon |
| `agent-worktree` | git2, serde | agent-core |
| `agent-backlog` | serde | agent-core |
| `agent-storage` | serde | agent-core |
| `agent-tui` | crossterm, ratatui | agent-cli |
| `agent-cli` | agent-daemon, agent-core, agent-tui | — |
