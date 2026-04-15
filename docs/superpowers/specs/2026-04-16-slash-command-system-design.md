# Slash Command System Design

## Overview

`agile-agent` currently has a minimal slash command parser for a few local commands such as `/help`, `/provider`, `/backlog`, `/todo-add`, `/run-once`, `/run-loop`, and `/quit`.

That implementation is sufficient for early bootstrap work, but it is not a safe base for a long-lived command system. The next version must support:

- local commands executed directly by `agile-agent`
- semantic commands addressed to an agent
- raw provider-native slash command passthrough

The system must also work in a multi-agent TUI where focus, Overview mode, provider sessions, kanban state, and local runtime state all coexist.

## Goals

### Functional goals

The command framework must support three explicit namespaces:

- `/local ...`
- `/agent ...`
- `/provider /...`

The initial feature targets are:

- show runtime status
- show current kanban tasks
- query and update selected configuration values
- inspect agent state
- forward provider-native slash commands

### Engineering goals

The design must provide:

- explicit namespaces instead of implicit command guessing
- clean separation between parsing, resolution, execution, and rendering
- support for default targets and explicit targets in multi-agent mode
- provider capability checks and clear errors
- extension points for help, autocomplete, auditing, and permissions

### UX goals

The system should make it obvious:

- which layer owns a command
- which target an agent/provider command resolved to
- whether a result is local, agent-level, or provider-native

## Non-goals

This design does not try to build:

- a shell or DSL
- command piping or scripting
- full provider semantic normalization
- a large initial command catalog

The focus is a stable framework and a controlled initial command set.

## Core decisions

### Explicit namespaces

Slash commands must always begin with one of these namespaces:

- `/local`
- `/agent`
- `/provider`

Unknown top-level namespaces fail immediately. They are never reinterpreted as normal chat input.

Examples:

```text
/local status
/local kanban list
/local config get tui.agent_list_rows
/local config set tui.agent_list_rows 10

/agent status
/agent alpha status
/agent alpha summary

/provider /status
/provider alpha /status
/provider overview /model
```

### `/agent` default target resolution

If `/agent ...` does not explicitly specify a target:

1. use the currently focused worker agent if one is focused
2. if the user is effectively in Overview context, use `OVERVIEW`
3. allow explicit target override, for example `/agent alpha status`

This keeps the default behavior fast while still allowing precise control.

### `/provider` uses raw provider-native command text

`/provider` must carry the raw provider command tail:

- `/provider /status`
- `/provider alpha /status`

The trailing `/status` is not reinterpreted by `agile-agent`. It is passed through unchanged to the owning provider session.

The framework should reject `/provider status` as invalid syntax. The explicit raw-slash form preserves provider-native semantics and avoids building a translation layer inside `agile-agent`.

### `/agent` and `/provider` stay semantically separate

These namespaces deliberately mean different things:

- `/agent`: stable `agile-agent` product semantics
- `/provider`: unstable but native provider semantics

Examples:

- `/agent status` means "show `agile-agent`'s view of the target agent"
- `/provider /status` means "send `/status` unchanged to Codex, Claude, or OpenCode"

This boundary is critical. Once it is blurred, the command layer becomes difficult to extend safely.

## Architecture

The command system should be built in four layers.

### 1. Command parser

The parser turns raw slash input into a typed intermediate form. It only performs syntax parsing.

Suggested shape:

```rust
pub struct CommandInvocation {
    pub namespace: CommandNamespace,
    pub target: Option<CommandTarget>,
    pub path: Vec<String>,
    pub args: Vec<String>,
    pub raw_tail: Option<String>,
}
```

Notes:

- `namespace`: `local | agent | provider`
- `target`: explicit target such as `alpha`
- `path`: structured local or agent command path, for example `["kanban", "list"]`
- `args`: ordinary arguments
- `raw_tail`: reserved for `/provider /status`-style passthrough

The parser must not:

- decide whether a command exists
- decide whether a target is valid
- execute any behavior

### 2. Command resolver

The resolver combines a parsed command with current TUI/runtime context.

Responsibilities:

- resolve the default target for `/agent` and `/provider`
- honor focused worker vs Overview semantics
- validate that explicit targets exist
- find the correct provider session for passthrough

Examples:

- `/agent status` resolves to the focused worker if one is focused
- `/agent status` resolves to `OVERVIEW` in Overview context
- `/provider /status` resolves to the focused agent's provider session

### 3. Command registry

The registry defines command metadata rather than hardcoding command meaning in a single parser match.

Suggested shape:

```rust
pub struct CommandSpec {
    pub id: &'static str,
    pub namespace: CommandNamespace,
    pub path: &'static [&'static str],
    pub summary: &'static str,
    pub requires_target: bool,
    pub execution_kind: CommandExecutionKind,
}
```

This enables:

- generated help output
- future autocomplete
- consistent validation
- easier extension without central parser bloat

### 4. Command executors

Execution should be split by namespace:

- `LocalCommandExecutor`
- `AgentCommandExecutor`
- `ProviderPassthroughExecutor`

Behavior by namespace:

- `local`: immediate local result
- `agent`: structured internal action or controlled agent prompt injection
- `provider`: raw passthrough via an existing provider session

## Namespace semantics

### `/local`

`/local` commands are owned entirely by `agile-agent`.

Properties:

- no provider round-trip required unless explicitly designed otherwise
- deterministic and immediately inspectable
- suitable for runtime, config, and kanban control

Recommended initial commands:

- `/local help`
- `/local status`
- `/local kanban list`
- `/local config get <key>`
- `/local config set <key> <value>`

Future candidates:

- `/local session show`
- `/local mailbox list`
- `/local agent list`
- `/local provider current`

### `/agent`

`/agent` commands represent stable `agile-agent` semantics for a target agent.

These commands are defined by `agile-agent`, not by Codex/Claude/OpenCode.

Recommended initial commands:

- `/agent status`
- `/agent <target> status`
- `/agent summary`
- `/agent <target> summary`

Possible future commands:

- `/agent interrupt`
- `/agent continue`
- `/agent reassign`
- `/agent prompt "<text>"`

Important rule:

- prefer internal structured actions
- only fall back to prompt injection when the command truly needs model reasoning

### `/provider`

`/provider` is for raw slash passthrough to the underlying provider session.

Examples:

- `/provider /status`
- `/provider <target> /status`

Required behavior:

- reject execution if the target has no provider session
- reject execution if the provider does not support slash passthrough
- clearly label the result as provider-native output

## Syntax rules

### Top-level form

```text
/<namespace> [target] <path...> [args...]
```

Where:

- `namespace` is required
- `target` is optional for `agent` and `provider`
- `path` is the command path for local/agent commands
- `args` are structured arguments

### `/provider` special form

`/provider` must carry a raw slash tail:

```text
/provider /status
/provider alpha /status
```

That means the parser must recognize:

- namespace = `provider`
- target = optional
- raw tail = `/status`

### Quoting

The parser should support:

```text
/local config set ui.title "My Agile Agent"
/agent alpha prompt "summarize current blockers"
```

Minimum support required:

- quoted strings
- basic escaping
- explicit parse errors on malformed quoting

## Error model

The framework should classify errors into clear stages.

### ParseError

Examples:

- malformed quotes
- empty `/provider`
- invalid slash structure

### ResolutionError

Examples:

- target agent not found
- no valid default target exists
- provider session missing

### CapabilityError

Examples:

- provider does not support slash passthrough
- command requires a feature the target does not expose

### ExecutionError

Examples:

- invalid config key
- invalid config value type
- kanban query failure

### ProviderError

Examples:

- raw downstream provider slash command failure
- expired or broken provider session

### Error presentation rules

All command errors must:

- be visible to the user
- identify the failing stage
- never silently fall back into normal chat submission

## Output and rendering

### Local commands

Local commands should render structured and concise outputs:

- status as a compact summary
- config get/set with clear before/after values
- kanban list as a stable multi-line task view

### Agent commands

`/agent` commands may return:

- an immediate structured result
- an async action start/result pair if they trigger work

### Provider passthrough

Provider passthrough output must clearly distinguish:

- successful routing
- provider execution
- returned provider-native output

The user should never confuse provider-native results with local command results.

## Extension points

To remain useful as the product grows, the framework should reserve space for:

- generated help and discoverability
- command autocomplete
- provider capability advertisement
- auditing and debug logging
- permission and confirmation policies for mutating commands
- future machine-readable output mode for panels and headless CLI reuse

## Initial command set recommendation

### `/local`

- `/local help`
- `/local status`
- `/local kanban list`
- `/local config get <key>`
- `/local config set <key> <value>`

### `/agent`

- `/agent status`
- `/agent <target> status`
- `/agent summary`
- `/agent <target> summary`

### `/provider`

- `/provider /status`
- `/provider <target> /status`

This is enough to validate all three namespaces without over-expanding the first implementation.

## Suggested module split

To avoid another large command switch file, prefer:

- `core/src/command_bus/model.rs`
- `core/src/command_bus/parse.rs`
- `core/src/command_bus/registry.rs`
- `core/src/command_bus/resolve.rs`
- `core/src/command_bus/local.rs`
- `core/src/command_bus/agent.rs`
- `core/src/command_bus/provider.rs`

If the first implementation wants fewer files, the minimum acceptable split is:

- parser
- registry
- executor

But provider passthrough should still remain isolated from local command logic.

## Migration strategy

The current flat local slash command parser should not be expanded indefinitely.

Recommended migration path:

1. keep the existing minimal parser temporarily
2. introduce the new parser + registry + executors
3. remap current flat commands into `/local ...`
4. add `/agent ...`
5. add `/provider ...`
6. retire the old flat entrypoints once compatibility is no longer needed

A temporary compatibility layer is acceptable:

- `/help` -> `/local help`
- `/backlog` -> `/local kanban list`

But the long-term documented syntax should be namespace-first.

## Key risks and mitigations

### Risk 1: semantic confusion between local, agent, and provider layers

Mitigation:

- enforce explicit namespaces
- keep `/agent` and `/provider` semantically separate

### Risk 2: target resolution feels implicit or surprising

Mitigation:

- always surface the resolved target in the result when defaulting occurs

### Risk 3: provider capability differences fragment the UX

Mitigation:

- add capability checks before passthrough
- fail explicitly instead of silently degrading

### Risk 4: command growth recreates parser sprawl

Mitigation:

- registry-driven metadata
- parser and executor separation

## Recommendation

The recommended design is:

- explicit namespace command system
- `/local`, `/agent`, `/provider` separation
- context-aware default target resolution with explicit override support
- raw slash passthrough only under `/provider /...`
- layered implementation: parser -> resolver -> registry -> executor

This is not the smallest design, but it is the one least likely to collapse under future command growth, multi-agent routing, and provider-specific behavior.
