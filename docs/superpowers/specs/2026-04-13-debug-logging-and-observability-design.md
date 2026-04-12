# Debug Logging and Observability Design

## Metadata

- Date: `2026-04-13`
- Project: `agile-agent`
- Status: `ready for user review`
- Language: `English`

## 1. Purpose

`agile-agent` currently has almost no real runtime observability. It emits a small number of
status and warning messages, but it does not provide a durable, structured execution log that can
be used to debug provider communication, agent lifecycle transitions, loop execution, persistence,
or task resolution.

The goal of this change is to add a first-class debug logging system that:

1. writes durable per-run logs under the current workplace
2. defaults to `debug` level for all runs
3. captures the full execution timeline from launch to shutdown
4. records all important runtime state transitions
5. records complete raw communication with `claude` and `codex`
6. supports future filtering, redaction, and analysis work without redesigning the logging model

This design intentionally prioritizes diagnosability over log minimization.

## 2. Scope

### In scope

- add a shared logging subsystem in `agent-core`
- initialize logging for both TUI and headless CLI entrypoints
- write structured log files under the current workplace
- log runtime environment, agent lifecycle, provider lifecycle, provider raw I/O, loop execution,
  persistence, verification, escalation, and shutdown
- log complete raw prompts and raw provider protocol traffic
- add automated tests for log path creation, logger bootstrap, and key runtime instrumentation

### Out of scope

- redaction or masking of sensitive content
- remote log shipping
- log rotation or retention cleanup policy
- a user-facing log viewer
- metrics dashboards or tracing backends
- configurable logging levels in this change

## 3. Current Baseline

The current codebase has these observability characteristics:

- no shared logging abstraction
- no persistent debug log file
- a few `println!` and `eprintln!` calls in CLI paths
- provider activity only appears indirectly through transcript entries
- no durable record of raw Claude stream output or Codex JSON-RPC traffic
- no uniform event naming or structured fields across modules

This means debugging failures currently depends on transient terminal output and manual inference.

## 4. Recommendation

The repository should adopt a structured per-run logger implemented in `agent-core`, with every
major runtime path writing JSON Lines events into the current workplace log directory.

This is the recommended approach because:

- logging must work across `agent-core`, `agent-cli`, `agent-tui`, and provider worker threads
- provider communication requires both raw protocol logging and semantic event logging
- future redaction and filtering are much easier on structured records than ad-hoc text lines
- one ordered event stream per run is easier to inspect than many fragmented files

## 5. Logging Model

### 5.1 Log destination

Logs should live under the current workplace:

`~/.agile-agent/workplaces/<workplace_id>/logs/`

Each run should create one log file:

`<utc-timestamp>-<run-mode>-pid<pid>.jsonl`

Example:

`2026-04-13T10-15-30Z-tui-pid43120.jsonl`

The directory should also contain:

`latest-path.txt`

This file stores the absolute path to the newest log file for quick discovery.

### 5.2 Log format

The log format should be JSON Lines, one event per line.

Each event should include at least:

- `ts`
- `level`
- `target`
- `event`
- `message`
- `run_id`
- `pid`
- `thread`
- `workplace_id`
- `workplace_path`
- `agent_id` when known
- `provider` when known
- `task_id` when known
- `loop_phase` when known

Additional event-specific fields should be attached for protocol payloads, file paths, command
arguments, guardrails, errors, and state transitions.

### 5.3 Default level

The default log level must be `debug`.

This change should assume that detailed troubleshooting is the default developer workflow.
Future configurability may be added later, but it must not change the default for now.

### 5.4 Failure policy

Logging must never become a correctness dependency.

- If logger initialization fails, execution should continue and emit one stderr warning.
- If individual log writes fail after initialization, the runtime should continue.
- Provider threads, loop execution, persistence, and task handling must not abort because logging
  failed.

## 6. Explicit Content Policy

The current policy is deliberate and must be encoded directly in implementation:

- log complete prompts sent to providers
- log complete raw Claude stdout lines
- log complete raw Claude stderr output
- log complete raw Codex JSON-RPC requests
- log complete raw Codex JSON-RPC responses
- log complete raw Codex notifications
- log complete raw Codex stderr output

This design does **not** redact sensitive material. Future redaction support will be a separate
feature.

## 7. Architecture

### 7.1 Shared logging module

Add a new module in `agent-core`, for example:

`core/src/logging.rs`

Responsibilities:

- derive the per-run log path from the workplace
- create the `logs/` directory
- create the JSONL file
- update `latest-path.txt`
- expose a shared initialization entrypoint
- expose helper functions or macros for structured debug logging
- attach stable run metadata such as `run_id`, `workplace_id`, and `workplace_path`

### 7.2 Initialization boundary

Logging should initialize after workplace resolution but before provider probing, runtime bootstrap,
or session restore. That ensures startup failures are visible in the log.

Initialization should be performed by the outer launch path:

- headless CLI commands in `cli/src/app_runner.rs`
- TUI startup path in `tui/src/lib.rs` and `tui/src/app_loop.rs`

### 7.3 One file per run

Use one unified log file per run rather than separate files for provider, loop, persistence, or UI.

Rationale:

- the most important debugging task is reconstructing event order
- provider traffic must be understood in the context of loop and lifecycle state
- one file is simpler for developers to discover and archive

## 8. Instrumentation Coverage

### 8.1 Launch and environment

Instrument startup paths to log:

- run mode: `tui`, `resume-last`, `run-loop`, `doctor`, `probe`, and other supported modes
- launch cwd
- effective workplace id and path
- selected default provider
- resume flags
- provider probe results
- configured provider path environment values

Suggested event names:

- `app.launch`
- `app.probe`
- `app.resume_option`

### 8.2 Workplace and storage roots

Instrument workplace and storage resolution to log:

- workplace id derivation
- resolved workplace root
- creation of workplace directories
- meta load/save/touch operations

Suggested event names:

- `workplace.resolve`
- `workplace.ensure`
- `workplace.meta.load`
- `workplace.meta.save`

### 8.3 Agent lifecycle

Instrument runtime bootstrap and session handling to log:

- agent bootstrap kind: created, restored, recreated after error
- selected provider binding
- agent id and codename
- state restore, transcript restore, snapshot restore, fallback restore
- session handle restoration
- sibling agent creation on provider switch
- stop and persist on shutdown

Suggested event names:

- `agent.bootstrap`
- `agent.restore_snapshot`
- `agent.restore_transcript`
- `agent.switch`
- `agent.persist`
- `agent.shutdown`

### 8.4 Persistence

Instrument agent, backlog, and session persistence layers to log:

- file paths read and written
- save/load success
- missing-file fallbacks
- parse failures
- structured bundle persistence stages

Files to cover include:

- `meta.json`
- `state.json`
- `transcript.json`
- `messages.json`
- `memory.json`
- `backlog.json`
- recent session files

Suggested event names:

- `storage.read`
- `storage.write`
- `storage.missing`
- `storage.parse_error`

### 8.5 Loop and task execution

Instrument the autonomous loop and task engine to log:

- loop guardrails
- iteration start and stop
- todo selection
- task creation or task resume
- prompt construction
- provider start failure
- continuation decision
- verification plan and outcome
- escalation decision
- completion or failure resolution
- final stopped reason

Suggested event names:

- `loop.start`
- `loop.iteration.start`
- `loop.iteration.stop`
- `task.begin`
- `task.resume`
- `task.prompt.build`
- `task.turn.resolve`
- `task.verify`
- `task.escalate`
- `task.complete`
- `task.fail`

### 8.6 Claude provider communication

Instrument the Claude provider path to log:

- executable resolution
- command args
- cwd
- process spawn
- exact payload written to stdin
- every raw stdout line before parsing
- stderr output
- parsed semantic events
- session id updates
- exit status and failures

Suggested event names:

- `provider.claude.start`
- `provider.claude.stdin_payload`
- `provider.claude.stdout_line`
- `provider.claude.stderr`
- `provider.claude.event`
- `provider.claude.exit`

### 8.7 Codex provider communication

Instrument the Codex provider path to log:

- executable resolution
- command args
- cwd
- process spawn
- every JSON-RPC request written
- every JSON-RPC response received
- every notification received
- approval request handling
- thread id creation or reuse
- turn start and completion
- stderr output
- shutdown polling and forced kill path

Suggested event names:

- `provider.codex.start`
- `provider.codex.request`
- `provider.codex.response`
- `provider.codex.notification`
- `provider.codex.approval`
- `provider.codex.thread`
- `provider.codex.stderr`
- `provider.codex.exit`

### 8.8 TUI control flow

The TUI should log meaningful user and runtime actions, but not every keystroke.

Log:

- provider switch requests
- local command execution
- prompt submission
- loop start and stop
- transcript overlay open or close
- provider event handling boundaries

Do **not** log every character typed. The submitted prompt and provider payload already provide the
useful trace without turning the log into noise.

Suggested event names:

- `tui.submit`
- `tui.command`
- `tui.provider_switch`
- `tui.loop_control`
- `tui.overlay`

## 9. Data Flow

The expected runtime flow is:

1. resolve workplace
2. initialize logger and log launch metadata
3. probe provider availability
4. bootstrap or restore agent runtime
5. restore state and transcript when applicable
6. start provider process when a user or loop action requires it
7. log raw provider traffic and parsed provider events
8. log loop and task resolution decisions
9. log persistence writes and shutdown

Provider worker threads must inherit logging context explicitly so their events retain the correct
run, workplace, and provider metadata.

## 10. Testing Strategy

This change must include strong automated tests.

### 10.1 Logging bootstrap tests

Add tests that verify:

- log directory creation under the workplace
- correct log filename shape
- `latest-path.txt` update
- JSONL records contain expected core metadata fields

### 10.2 Runtime and lifecycle tests

Add tests that verify:

- bootstrap created/restored/recreated flows emit lifecycle events
- snapshot restore and transcript restore emit logs
- persistence operations emit file-path-bearing events

### 10.3 Provider logging tests

Add tests that verify:

- Claude logging captures raw stdin payloads and raw stdout lines
- Codex logging captures raw JSON-RPC requests and notifications
- semantic parsed events are logged in addition to raw traffic

These tests should rely on the existing mocked or fixture-driven provider parsing surfaces rather
than real provider binaries.

### 10.4 Loop and task tests

Add tests that verify:

- loop iteration boundaries are logged
- task creation or resumption is logged
- verification and escalation decisions are logged
- stopped reason is logged

### 10.5 Headless integration test

Add at least one integration test that:

1. creates a temporary workplace
2. runs a headless flow
3. asserts that a log file is created under that workplace
4. asserts that the log contains key events from launch through completion

## 11. Acceptance Criteria

This design is complete when:

1. every run writes one JSONL log file under the current workplace logs directory
2. default log level is `debug`
3. startup metadata and environment are logged
4. agent lifecycle events are logged
5. loop and task resolution events are logged
6. persistence read and write paths are logged
7. full raw Claude communication is logged
8. full raw Codex communication is logged
9. logging failures do not abort runtime behavior
10. automated tests cover logger bootstrap and key instrumentation paths

## 12. Resolved Decisions

The following decisions are fixed for implementation:

- logging is workplace-scoped, not process-global only
- one run maps to one unified JSONL file
- default log level is `debug`
- raw provider traffic is logged in full
- no redaction is performed in this change
- TUI logs meaningful actions, not every keystroke
- logging failure is non-fatal
