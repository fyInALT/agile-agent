# Multi-Agent Data Migration Guide

## Overview

When upgrading from a legacy single-agent setup to the multi-agent architecture, `agile-agent` automatically migrates existing data to the new format.

## Migration Process

### Automatic Detection

The migration is triggered when:

1. Legacy files exist at workplace root (`meta.json`, `state.json`, `transcript.json`)
2. The `agents/` directory does not exist

### File Migration

Legacy files are moved to the agent subdirectory:

```
Before (legacy):
~/.agile-agent/workplaces/{workplace_id}/
├── meta.json
├── state.json
├── transcript.json
├── messages.json
├── memory.json
└── backlog.json

After (multi-agent):
~/.agile-agent/workplaces/{workplace_id}/
├── agents/
│   └── agent_001/
│       ├── meta.json
│       ├── state.json
│       ├── transcript.json
│       ├── messages.json
│       └── memory.json
├── workplace_meta.json
└── backlog.json
```

### Workplace Metadata

A new `workplace_meta.json` is created:

```json
{
  "workplace_id": "workplace-...",
  "created_at": "2026-04-15T00:00:00Z",
  "runtime_mode": "multi_agent",
  "migrated_from": "single_agent",
  "version": 1
}
```

## Rollback

If migration fails, files are automatically restored to their original locations.

## Manual Migration

To trigger migration manually:

```bash
# Start the TUI - migration happens automatically
cargo run -p agent-cli

# Or run headless
cargo run -p agent-cli -- run-loop --max-iterations 1
```

## Backward Compatibility

After migration:

- Single-agent mode still works via `RuntimeMode::SingleAgent`
- Existing backlog is preserved unchanged
- Agent identity is maintained with the same agent_id

## CLI Commands After Migration

```bash
# List agents (the migrated agent becomes agent_001)
cargo run -p agent-cli -- agent list --all

# Check agent status
cargo run -p agent-cli -- agent status agent_001

# View workplace info
cargo run -p agent-cli -- workplace current
```

## Troubleshooting

### Migration Failed

1. Check logs: `~/.agile-agent/workplaces/{workplace_id}/logs/`
2. Files should be restored automatically
3. Manual recovery: copy files back from agent subdirectory if needed

### Already Migrated

If `agents/` directory exists, migration is skipped. The system detects this via `LegacyDetector.is_already_migrated()`.

### Invalid Files

If file names contain non-UTF8 characters, those files are skipped gracefully without causing migration failure.