# Agent Launch Config And Resume Design

## Metadata

- Date: `2026-04-16`
- Project: `agile-agent`
- Status: `draft`
- Language: `English`
- Related:
  - `docs/plan/spec/multi-agent/sprint-04-basic-tui.md`
  - `docs/plan/spec/multi-agent/sprint-06-shutdown-restore.md`
  - `docs/plan/spec/multi-agent/sprint-11-integration.md`
  - `docs/superpowers/specs/2026-04-16-multi-agent-logging-design.md`

## Overview

`agile-agent` can already create new worker agents from the TUI with `Ctrl+N`, but provider startup configuration still depends entirely on the environment of the host `agile-agent` process. If a user wants one new Claude or Codex agent to run against a different model, they currently have to change the shell environment before launching the entire application. They cannot choose a different model at the moment that a single agent is created.

That constraint no longer fits a multi-agent system. One of the main reasons to run multiple agents is to let different agents use different provider configurations in parallel, including different base URLs, models, tokens, timeouts, and provider-specific flags. At the same time, `agile-agent` already has multi-agent shutdown snapshots and TUI resume support. If provider configuration exists only as temporary in-memory state, resume becomes inaccurate immediately.

This design promotes "agent launch configuration" to a first-class model. Each worker agent should own its own provider startup configuration at creation time, and its paired decision agent should be allowed to use a separate configuration. The configuration must be structurally parsed, fully persisted, reliably restored, and directly reusable by the future agent template system.

## Goals

### Functional goals

- `Ctrl+N` should create an agent by first selecting a provider and then entering startup configuration.
- `Claude` and `Codex` should both support separate `work env` and `decision env` inputs.
- Input should support both:
  - pure environment variable syntax
  - full command fragment syntax
- The system should always normalize input into a structured internal representation rather than storing only raw command text.
- If the user leaves input empty, the system should use the host default environment.
- The default decision-agent behavior should use the host default environment rather than inheriting the work-agent environment.
- After agent creation, launch configuration must be restorable through both TUI shutdown snapshots and resume.
- Resume must not depend on the host environment still matching the original launch-time environment.

### Engineering goals

- Provider launch configuration must become its own explicit data model instead of remaining scattered around `Command::spawn` call sites.
- `tui` should collect and display configuration, `core` should parse, validate, persist, and restore it, and provider code should consume only structured launch data.
- The existing multi-agent restore architecture should be extended rather than bypassed.
- The data model must be capable of carrying future template metadata from the start.

### UX goals

- The creation flow should remain linear: `select provider -> configure -> confirm`.
- The user should be able to see how the system interpreted the configuration.
- Errors should be surfaced before creation whenever possible.
- Resume failures should leave agents visible and diagnosable instead of silently dropping them.

## Non-goals

This design does not attempt to deliver the following in this phase:

- a complete agent template UI
- secret encryption or OS keychain integration
- a generic semantic abstraction of provider-native flags
- a full argument allowlist system for every provider
- automatic replay of unfinished requests that were active before shutdown

## Architecture fit

The core problem is not that the TUI is missing one more form. The deeper issue is that the system has no "per-agent launch config" layer at all. Today the Claude and Codex providers read executables and environment variables implicitly from the current process and then call `Command::spawn`. That means:

- a single new agent cannot choose an independent model configuration
- a decision agent cannot own an independent configuration
- snapshots can save session handles but not launch identity
- resume can restore "conversation traces" but not "startup conditions"

The right boundary is therefore not a temporary `tui` patch that forwards a few environment variables. The correct boundary is a dedicated launch-config model and parser in `core`. That keeps provider startup code simple while giving future template functionality a stable foundation.

## User stories

- As a user, I want to enter environment variables when creating a Claude agent with `Ctrl+N` so that it can use a different model from the host default.
- As a user, I want the decision agent to be able to use another model configuration, for example a cheaper or faster model.
- As a user, I want the system to keep working with host defaults if I leave the configuration empty.
- As a user, I want resumed agents to use the configuration that was resolved when they were created, rather than re-reading the current host environment.
- As a user, if I selected `Claude`, I want the system to reject a later `codex` command fragment explicitly instead of guessing.
- As a future template-system implementer, I want templates and manual input to share the same underlying data model.

## Core decisions

### 1. Provider selection stays first

`Ctrl+N` still begins with provider selection. The user chooses `Claude`, `Codex`, or `Mock`, and the provider shown in the configuration step is locked and read-only.

This matters because it:

- prevents ambiguity such as "the UI selected Claude, but the command fragment says codex"
- keeps provider choice separate from command parsing
- preserves clear provider ownership for future templates

### 2. Work and decision config are both first-class

Creating a `Claude` or `Codex` agent opens a configuration overlay with two independent inputs:

- `Work Agent Config`
- `Decision Agent Config`

Both inputs may be empty. Empty means:

- `work config` empty -> use the host default environment
- `decision config` empty -> also use the host default environment

The important rule is that an empty `decision config` does not inherit from the work config. That is an explicit product decision so that the decision layer can later use a different model strategy from the work agent.

### 3. Two input syntaxes, one internal model

Users may provide configuration in two external forms:

1. pure environment variable mode
2. full command fragment mode

Internally, both forms must be parsed into the same structured model. Raw command text is not the long-term source of truth.

### 4. Resume must use captured launch identity

Even if the user leaves configuration empty, the system must not store only "uses host default". Before the agent is created, the system must resolve and capture the actual launch identity, including at least:

- provider
- resolved executable path
- effective environment
- user-supplied extra arguments

Resume must use this resolved configuration, not the current host environment.

### 5. Mock is explicitly excluded from launch overrides

`Mock` has no real model or environment semantics. In the first version:

- selecting `Mock` skips the configuration overlay entirely
- if internal code still tries to attach launch overrides to `Mock`, that is an explicit error

## Interaction flow

### New agent creation

Recommended interaction flow:

1. The user presses `Ctrl+N`.
2. The provider-selection overlay opens.
3. The user chooses `Claude`, `Codex`, or `Mock`.
4. If the provider is `Mock`, create the agent immediately.
5. If the provider is `Claude` or `Codex`, open the launch-config overlay.
6. The overlay shows:
   - the selected provider
   - `Work Agent Config`
   - `Decision Agent Config`
   - a parse preview area
   - an error area
7. On confirmation, the system:
   - parses input
   - validates input
   - resolves host defaults
   - persists the launch bundle
   - creates the work agent
   - binds decision-agent configuration
8. After success, the status line shows a compact summary of the parsed result.

### Input interpretation

The rules are:

- If every non-empty line matches `KEY=VALUE`, parse it as pure env mode.
- Otherwise, parse it as command-fragment mode.
- The provider is already fixed by the previous UI step, so any executable inside the fragment must match the selected provider.
- The user may specify an executable path and extra arguments.
- Provider-owned protocol arguments remain reserved and may not be overridden.

Example 1: env-only

```text
ANTHROPIC_BASE_URL=https://api.minimaxi.com/anthropic
ANTHROPIC_AUTH_TOKEN=sk-xxx
ANTHROPIC_MODEL=MiniMax-M2.7
API_TIMEOUT_MS=3000000
```

Example 2: command fragment

```text
ANTHROPIC_BASE_URL=https://api.minimaxi.com/anthropic ANTHROPIC_MODEL=MiniMax-M2.7 claude --some-extra-flag
```

### Decision-agent creation semantics

At runtime the decision agent remains an auxiliary capability attached to the work agent rather than a separately visible TUI worker slot. Its launch config is still part of the work agent's launch bundle and must be stored, restored, and resolved independently.

Once work-agent creation succeeds:

- if the provider supports a decision agent, create the corresponding decision caller
- the decision caller uses the `decision launch spec`
- if `decision config` is empty, the `decision launch spec` resolves from the host default environment at creation time and stores the resolved result

## Data model

### LaunchInputSpec

`LaunchInputSpec` represents declarative user input. It serves both the current UI and the future template system.

Suggested shape:

```rust
pub struct LaunchInputSpec {
    pub provider: ProviderKind,
    pub source_mode: LaunchSourceMode,
    pub source_origin: LaunchSourceOrigin,
    pub raw_text: Option<String>,
    pub env_overrides: BTreeMap<String, String>,
    pub requested_executable: Option<String>,
    pub extra_args: Vec<String>,
    pub template_id: Option<String>,
}
```

Supporting enums:

```rust
pub enum LaunchSourceMode {
    HostDefault,
    EnvOnly,
    CommandFragment,
}

pub enum LaunchSourceOrigin {
    Manual,
    Template,
    HostDefault,
}
```

### ResolvedLaunchSpec

`ResolvedLaunchSpec` is the actual launch result used to start the provider. It is the factual source for resume.

Suggested shape:

```rust
pub struct ResolvedLaunchSpec {
    pub provider: ProviderKind,
    pub resolved_executable_path: String,
    pub effective_env: BTreeMap<String, String>,
    pub extra_args: Vec<String>,
    pub resolved_at: String,
    pub derived_from: LaunchSourceMode,
    pub resolution_notes: Vec<String>,
}
```

`effective_env` is not just the explicit override set. It is the full environment snapshot the child process will actually receive. That is what makes deterministic resume possible even when the user entered nothing manually.

### AgentLaunchBundle

`AgentLaunchBundle` binds the two launch configurations that belong to one work agent:

```rust
pub struct AgentLaunchBundle {
    pub work_input: LaunchInputSpec,
    pub work_resolved: ResolvedLaunchSpec,
    pub decision_input: LaunchInputSpec,
    pub decision_resolved: ResolvedLaunchSpec,
}
```

### Runtime ownership

At runtime, `AgentLaunchBundle` should be attached to the work-agent slot rather than to the decision-agent slot. That matches the actual ownership model:

- the decision agent is not an independent TUI worker
- work and decision configs belong to the same agent identity
- restoring a work slot should restore its decision launch identity at the same time

## Persistence model

### Where data is stored

To avoid making the TUI snapshot the only source of truth, launch configuration should be persisted in at least two places:

1. `launch-config.json` inside the agent directory
2. a per-agent launch bundle inside `TuiResumeSnapshot`

Recommended rules:

- the agent directory is the long-lived factual state
- the TUI snapshot is the consistent per-shutdown restore view
- if both the snapshot and the agent file exist and disagree, resume uses the snapshot for that restore operation and logs a warning

### Agent-level persistence

Each work agent directory gets a structured config file:

```text
<workplace>/agents/<agent-id>/launch-config.json
```

The file stores the full `AgentLaunchBundle`.

### TUI snapshot persistence

`PersistedAgentSnapshot` should gain:

```rust
pub struct PersistedAgentSnapshot {
    pub agent_id: AgentId,
    pub codename: AgentCodename,
    pub provider_type: ProviderType,
    pub role: AgentRole,
    pub status: PersistedAgentStatus,
    pub provider_session_id: Option<String>,
    pub transcript: Vec<TranscriptEntry>,
    pub assigned_task_id: Option<TaskId>,
    pub launch_bundle: Option<AgentLaunchBundle>,
}
```

The resume snapshot must include the launch bundle because TUI restore needs a single coherent view of all restored agents.

## Resume semantics

### Restore behavior

Resume should restore:

- the agent list
- the focused agent
- transcripts
- provider session handles
- backlog state
- mailbox state
- view state
- per-agent launch bundles

### Work-agent restore

When restoring a work agent:

- restore the slot itself
- restore the `provider_session_id`
- restore the `launch_bundle`
- do not automatically replay unfinished provider requests
- if the previous status was `Active`, always restore it as `Idle` and emit a clear status/log message such as `restored from interrupted session; not auto-resumed`

### Decision-agent restore

The decision agent does not restore an old live thread. It restores only:

- decision launch configuration
- the binding relationship to the work agent

When the decision layer needs a real LLM call again, it reinitializes the caller from `decision_resolved`.

### Failure behavior

If restore finds any of the following:

- `resolved_executable_path` no longer exists
- `launch-config.json` is corrupted
- the snapshot launch bundle is missing or incompatible

then the system should:

- still restore the agent into the visible list
- put the slot into `Error`
- keep transcripts and historical session handles when possible
- expose the concrete error to the user
- never silently fall back to the current host default environment

## Provider execution changes

### Core/provider boundary

Provider code should no longer rely implicitly on the host process environment. The new boundary should be:

- `tui` collects user input
- `core::launch_config` parses, validates, and resolves it into `ResolvedLaunchSpec`
- provider startup code consumes `ResolvedLaunchSpec`

Recommended provider-side start context:

```rust
pub struct ProviderLaunchContext {
    pub spec: ResolvedLaunchSpec,
    pub cwd: PathBuf,
    pub session_handle: Option<SessionHandle>,
}
```

### Child-process startup behavior

Child-process startup should follow these rules:

- executable comes from `resolved_executable_path`
- environment comes from `effective_env`
- extra args come from `extra_args`
- provider-owned protocol arguments are still injected by provider code

Provider-owned required arguments remain under provider control rather than being exposed as part of the long-term launch bundle. That keeps snapshot data stable even if provider protocol details evolve later.

## Validation rules

### Provider consistency

The user selected the provider first, so the command fragment executable must match that provider.

Example:

- selected provider = `Claude`
- command fragment executable = `codex`

Result:

- parsing fails
- the overlay stays open
- original user input remains intact

### Input format validation

In pure env mode:

- every non-empty line must match `KEY=VALUE`
- the key cannot be empty

In command-fragment mode:

- shell-style tokenization must succeed
- the boundaries between env prefix, executable, and extra args must be unambiguous
- an executable must be present; fragments with only env prefixes or only flags are invalid

### Reserved-argument conflicts

Provider-owned reserved arguments cannot be overridden. Examples include:

- Claude stream-json input/output protocol arguments
- Claude permission-mode arguments
- Codex `exec --json` protocol entry arguments

If user-supplied arguments conflict with reserved arguments, validation should fail explicitly.

### Mock validation

`Mock` does not accept launch overrides. Any internal attempt to attach a launch bundle to `Mock` should return an explicit error.

## UI feedback and error handling

### Preview

The launch-config overlay should show a parse preview before confirmation, at least:

- provider
- executable
- env override count
- extra arg count
- decision config source

Example:

```text
Work agent:
  provider: claude
  executable: /usr/local/bin/claude
  env: 4 overrides
  args: 1 extra arg

Decision agent:
  provider: claude
  source: host default
```

### Error types

There are three error classes:

1. input-time errors
2. creation-time errors
3. restore-time errors

Input-time errors:

- happen inside the overlay
- do not close the overlay
- preserve original input
- show a direct error message

Creation-time errors:

- parsing succeeded, but spawn or persistence failed
- the agent is not created
- the status line shows the concrete failure reason

Restore-time errors:

- the agent still appears in the list
- the state becomes `Error`
- the system does not silently fall back to host defaults

### Redaction

Even if this phase allows full local plaintext persistence, UI and logs should still redact by default for readability. Recommended default redaction targets include:

- `*_TOKEN`
- `*_API_KEY`
- `*_AUTH_TOKEN`
- `Authorization`

Storage may keep full values, but overlays, status output, and logs should show redacted versions by default.

## Logging

Recommended new log events:

| Event | When | Fields |
|------|------|--------|
| `launch_config.parse.start` | parse begins | provider, target (`work`/`decision`), source_mode_guess |
| `launch_config.parse.success` | parse succeeds | provider, target, source_mode, executable, env_count, arg_count |
| `launch_config.parse.failed` | parse fails | provider, target, error |
| `launch_config.resolve.success` | host-default resolution succeeds | provider, target, executable, env_count |
| `launch_config.persist` | launch bundle is saved | agent_id, provider |
| `launch_config.restore` | launch bundle is restored | agent_id, provider, source |
| `launch_config.restore.failed` | launch bundle restore fails | agent_id, error |

## Testing requirements

Implementation must remain TDD-first and cover at least:

- parser unit tests
  - env-only success
  - command-fragment success
  - provider-mismatch failure
  - reserved-argument conflict failure
- resolver unit tests
  - host default resolves to a captured env snapshot
  - empty decision config resolves independently from work config
- persistence round-trip tests
  - `launch-config.json` save/load
  - `TuiResumeSnapshot` save/load with launch bundle
- TUI integration tests
  - `Ctrl+N` for Claude/Codex opens the config overlay
  - `Mock` bypasses the config overlay
  - parse failure keeps the user's input intact
- resume tests
  - restored slots contain launch bundles
  - missing executables yield visible `Error` state
  - the decision caller reuses restored decision config

## Template evolution path

Future template support should build directly on `LaunchInputSpec` rather than inventing another provider-profile model.

Recommended approach:

- a template is fundamentally a prefilled `LaunchInputSpec`
- `Ctrl+N` later supports:
  - manual input
  - template prefill
- template instantiation still produces an independent `ResolvedLaunchSpec`
- resume always uses the resolved result of instantiation, never re-evaluates the template name

That ensures:

- changing a template does not retroactively affect existing agents
- each created agent keeps a self-contained launch identity
- template-based creation and manual creation share one persistence model

## Future extensions

The most valuable follow-on capabilities on top of this design are:

1. launch-config test runs
2. launch summaries in overview/dashboard
3. cloning config from an existing agent
4. saving the current config as a template
5. diffing two agents' launch configs
6. a resume-time repair flow for broken executable paths

## Summary

This is not a small change that "adds a few env vars before provider start". It turns an agent's model identity into a persisted object. That is the only way to make all of the following coherent at the same time:

- each agent can choose its own model
- the decision layer can use a different model strategy
- resume can restore real launch identity
- templates can later land on a stable foundation

The correct implementation boundary is to add a structured launch-config model and parser in `core`, add a creation overlay in `tui`, let provider code consume `ResolvedLaunchSpec`, and persist `AgentLaunchBundle` both in TUI snapshots and at the agent level.
