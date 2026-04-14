# Kanban System Design

## Metadata

- Date: `2026-04-13`
- Project: `agile-agent`
- Status: `Draft`
- Language: `English`

## 1. Purpose

`agile-agent` needs a shared, Git-backed kanban system that multiple agents can use concurrently to manage agile development work. The system manages sprints, stories, tasks, ideas, issues, and tips — all accessible to both agents and human developers directly as readable JSON files.

## 2. Scope

### In scope

- six element types: sprint, story, task, idea, issue, tips
- shared status state machine across all element types
- dependency and reference relationships between elements
- Git-based concurrent access with standard Git flow
- human-readable JSON file format under `~/.agile-agent/workplaces/{id}/kanban/`

### Out of scope

- TUI rendering of the kanban board
- automated sprint planning or story point estimation
- Git conflict resolution UI
- real-time multi-agent locking or transactions

## 3. Element Types

| Type | Description | Hierarchy |
|------|-------------|----------|
| `sprint` | A time-boxed development iteration | Top-level container |
| `story` | A user-facing feature or requirement, belongs to a sprint | Child of sprint |
| `task` | A granular work item, belongs to a story | Child of story |
| `idea` | An underdeveloped thought, often just a sentence | Independent, can reference others |
| `issue` | A problem or concern to address | Independent, can reference others |
| `tips` | A small note or reminder, always attached to a sprint | Independent, references a target task |

### 3.1 Tips Appending

Agents may append tips to any task (including tasks assigned to other agents). A tip is an independent element file that references the target task via the `references` field. Tips are append-only to prevent conflicts. The tip records its `agent_id` as the creator.

## 4. Common Fields

All elements share the following fields:

| Field | Type | Description |
|-------|------|-------------|
| `id` | string | Human-readable unique identifier, format `{type}-{number}` e.g. `sprint-001` |
| `type` | string | Element type: `sprint`, `story`, `task`, `idea`, `issue`, `tips` |
| `title` | string | Short title |
| `content` | string | Full content or description |
| `keywords` | string[] | Keywords for search and AI context |
| `status` | string | Current status (see State Machine) |
| `dependencies` | string[] | IDs of elements this item blocks on (execution order) |
| `references` | string[] | IDs of related elements AI should consult when working on this |
| `parent` | string? | ID of parent element (story→sprint, task→story) |
| `created_at` | string (ISO 8601) | Creation timestamp |
| `updated_at` | string (ISO 8601) | Last modification timestamp |
| `priority` | string? | `high`, `medium`, `low` |
| `assignee` | string? | Agent or person responsible |
| `effort` | number? | Estimated work units |
| `blocked_reason` | string? | Reason when status is `blocked` |
| `tags` | string[] | Additional labels |

Tips-specific fields:

| Field | Type | Description |
|-------|------|-------------|
| `target_task` | string | ID of the task this tip is appended to |
| `agent_id` | string | ID of the agent that created this tip |

## 5. State Machine

All element types share one status model:

```
plan → backlog → blocked / ready / todo → in_progress → done → verified
```

| Status | Description |
|--------|-------------|
| `plan` | Item is being planned |
| `backlog` | Item is in the backlog |
| `blocked` | Item cannot proceed |
| `ready` | Item is ready to be worked on immediately |
| `todo` | Item is ready but intentionally deferred |
| `in_progress` | Item is being actively worked |
| `done` | Item is completed |
| `verified` | Item's completion has been verified |

## 6. Relationships

### 6.1 Dependencies

`dependencies` is a list of element IDs. It represents **blocking execution order**: element A depends on B means A cannot start until B is done. This applies to all element types and is many-to-many.

### 6.2 References

`references` is a list of element IDs. It represents a **one-way informational link**: when an agent works on element A, it should consult the content of all elements in A's `references` list. This applies to all element types and is many-to-many.

### 6.3 Parent

`parent` establishes the Scrum hierarchy:

- `task.parent` = `story.id`
- `story.parent` = `sprint.id`
- `idea.parent` = null (independent)
- `issue.parent` = null (independent)
- `tips.parent` = sprint.id (tips belong to a sprint, reference a task)

## 7. Storage Structure

```
~/.agile-agent/workplaces/{workplace_id}/kanban/
├── index.json       # Minimal ID registry
└── elements/
    ├── sprint-001.json
    ├── sprint-002.json
    ├── story-001.json
    ├── story-002.json
    ├── task-001.json
    ├── task-002.json
    ├── idea-001.json
    ├── issue-001.json
    └── tip-001.json
```

### 7.1 index.json

`index.json` is intentionally minimal to avoid merge conflicts:

```json
{
  "elements": [
    "sprint-001",
    "story-001",
    "task-001",
    "idea-001",
    "issue-001",
    "tip-001"
  ]
}
```

Agents discover elements by traversing the `elements/` directory, not by relying on `index.json`. This ensures agents never miss elements due to a stale index and always observe each other's work.

### 7.2 Element File Format

Each element is stored as a single human-readable JSON file:

```json
{
  "id": "task-001",
  "type": "task",
  "title": "Implement kanban persistence",
  "content": "Add JSON file read/write for all element types...",
  "keywords": ["persistence", "json", "storage"],
  "status": "in_progress",
  "dependencies": ["story-001"],
  "references": ["idea-002", "tip-001"],
  "parent": "story-001",
  "created_at": "2026-04-13T10:00:00Z",
  "updated_at": "2026-04-13T14:30:00Z",
  "priority": "high",
  "assignee": "agent-alpha",
  "effort": 5,
  "blocked_reason": null,
  "tags": ["backend", "storage"]
}
```

## 8. Git-Based Collaboration

The `kanban/` directory lives inside a Git repository (the workplace itself). Agents collaborate using standard Git flow:

1. Each agent works on its own branch
2. Before committing, agent fetches and rebases/merges from the shared branch
3. Agent commits its changes and pushes
4. Conflicts are resolved via Git merge/rebase tools

### 8.1 Permissions Model

The permission model is advisory, not enforced technically:

- Agents should only modify elements where `assignee` matches their identity
- Agents may append tips to any task (append-only operation)
- In special cases, agents may modify elements outside their assignment — this is allowed but should be reviewed
- Conflicts in Git indicate a design problem: if two agents modify the same element, the design should be reconsidered to give each agent distinct ownership

### 8.2 Conflict Discovery

Since agents traverse the `elements/` directory directly, they naturally discover each other's work. Git's merge conflict markers surface any concurrent modifications to the same file, making conflicts visible and solvable via standard Git workflows.

## 9. ID Generation

Element IDs follow a human-readable format: `{type}-{number}`

Examples: `sprint-001`, `story-042`, `task-123`, `idea-007`, `issue-001`, `tip-001`

The number is sequential within each type, starting from 001. IDs must be unique across all element types.

## 10. Storage Architecture Considerations

The design separates storage concerns from the core domain model to allow future refactoring:

- **File format** (JSON) is human-readable and Git-mergeable, not a binary or opaque format
- **Directory layout** uses a flat `elements/` folder — future partitioning (e.g. by sprint) can be added as a naming convention without changing file contents
- **index.json** is intentionally minimal — the source of truth is the actual element files
- **No database** — plain files enable Git versioning, diffing, and branching
- **Append-only tips** — tips as independent files prevent read-write conflicts when multiple agents add notes to the same task

## 11. Resolved Decisions

- All six element types share one status machine
- Dependencies and references are both many-to-many ID lists
- `index.json` stores only an ID list, not full metadata
- Elements live in `elements/{id}.json` (no type subdirectory in path)
- Tips are independent elements, not embedded in task files
- Git flow is the collaboration model; permissions are advisory
- Storage path: `~/.agile-agent/workplaces/{id}/kanban/`

## 12. Open Questions

None at this time. All key design decisions have been resolved through the brainstorming process.

## 13. References

- `docs/plan/multi-agent-parallel-runtime-design.md` — related multi-agent architecture
- `docs/superpowers/specs/2026-04-13-debug-logging-and-observability-design.md` — logging system
- `docs/plan/v2-sprint-1-backlog-and-task-spec.md` — existing backlog and task model
