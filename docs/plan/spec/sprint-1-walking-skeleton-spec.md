# Sprint 1 Walking Skeleton Spec

## Metadata

- Sprint: `V1 / Sprint 1`
- Stories covered:
  - `V1-S01` TUI walking skeleton
  - `S1-T01` through `S1-T06`
- Language policy:
  - Specs under `agile-agent/docs/plan/spec/` must be written in English.

## 1. Purpose

This spec defines the engineering execution plan for the first runnable vertical slice of `agile-agent`.

The goal is not to build a real provider integration yet. The goal is to establish a stable interaction shell with:

- a working CLI entrypoint
- a working TUI lifecycle
- a minimal layout
- editable input
- a mock assistant execution path
- safe shutdown and basic error containment

At the end of this work, the team must be able to demo:

1. Launch `agile-agent`
2. Type a message
3. Submit it
4. See a mock assistant response
5. Exit cleanly without breaking the terminal

## 2. Scope

### In scope

- `S1-T01` CLI entrypoint and command routing
- `S1-T02` terminal lifecycle
- `S1-T03` minimal TUI layout
- `S1-T04` basic text input and submit
- `S1-T05` mock / echo provider
- `S1-T06` safe exit and basic error boundary

### Out of scope

- Real Claude integration
- Real Codex integration
- Session persistence
- Skills UI
- Slash commands
- Markdown rendering polish
- Multi-turn state
- Structured transcript storage

## 3. Implementation Principles

### 3.1 Optimize for the first working slice

Do not over-design the architecture before the first demo path works.

### 3.2 Keep the shell reusable

Even though this sprint uses a mock provider, the event and app structure should make it easy to swap in real providers later.

### 3.3 Prefer simple state over premature abstractions

For Sprint 1:

- one app state object
- one event loop
- one mock provider path

is better than introducing several layers with no working behavior.

### 3.4 Protect terminal recovery

Any implementation choice that risks leaving the terminal in raw mode must be treated as a bug.

## 4. Proposed Initial Repo Structure

This sprint should create only the minimum structure needed to support the walking skeleton.

```text
agile-agent/
├── Cargo.toml
├── README.md
├── docs/
│   └── plan/
│       └── spec/
│           └── sprint-1-walking-skeleton-spec.md
├── cli/
│   ├── Cargo.toml
│   └── src/
│       └── main.rs
├── core/
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs
│       ├── app.rs
│       ├── event.rs
│       └── mock_provider.rs
└── tui/
    ├── Cargo.toml
    └── src/
        ├── lib.rs
        ├── terminal.rs
        ├── app_loop.rs
        ├── render.rs
        └── input.rs
```

Notes:

- A dedicated provider package is not required for Sprint 1.
- If it is introduced now, it must not delay the walking skeleton demo.
- Favor fewer crates if the workspace overhead starts slowing execution.

## 5. Runtime Model

Sprint 1 should use this simple runtime model:

1. `main.rs` parses CLI arguments.
2. Default command enters TUI mode.
3. TUI initializes terminal state.
4. App loop runs:
   - read input events
   - update app state
   - render frame
5. On submit:
   - append user message
   - start mock provider action
   - stream or append assistant message
6. On quit:
   - restore terminal state
   - exit with success

## 6. Core App State for Sprint 1

The first implementation should keep state intentionally small:

```rust
pub struct AppState {
    pub transcript: Vec<TranscriptEntry>,
    pub input: String,
    pub status: AppStatus,
    pub should_quit: bool,
}

pub enum TranscriptEntry {
    User(String),
    Assistant(String),
    Status(String),
    Error(String),
}

pub enum AppStatus {
    Idle,
    Responding,
}
```

This does not need to be the final shape. It only needs to support the Sprint 1 demo path.

## 7. Detailed Execution Checklist

## S1-T01 CLI Entry and Command Routing

### Objective

Create the top-level executable and route commands to:

- default TUI mode
- `doctor`
- `probe --json`

Only the TUI path needs to be fully functional in this sprint.

### Engineering Checklist

- Create the workspace `Cargo.toml`.
- Add `agent-cli` as the binary crate.
- Add `clap` with subcommand support.
- Define a CLI contract similar to:

```text
agile-agent
agile-agent doctor
agile-agent probe --json
```

- Make `doctor` and `probe` return placeholder output if their full implementation is not finished yet.
- Make the default branch call `run_tui()`.

### Suggested Files

- `Cargo.toml`
- `cli/Cargo.toml`
- `cli/src/main.rs`

### Acceptance

- `cargo run -p agent-cli --` launches the TUI.
- `cargo run -p agent-cli -- doctor` parses successfully.
- `cargo run -p agent-cli -- probe --json` parses successfully.

### Notes

- Use stable subcommand naming now. Avoid future renames unless necessary.

## S1-T02 Terminal Lifecycle

### Objective

Implement a safe TUI terminal lifecycle:

- enter alternate screen
- enable raw mode
- restore terminal state on exit

### Engineering Checklist

- Add `ratatui` and `crossterm`.
- Implement terminal setup helper:
  - enable raw mode
  - enter alternate screen
  - optionally enable bracketed paste now or stub it
- Implement terminal teardown helper:
  - disable raw mode
  - leave alternate screen
  - show cursor
- Ensure teardown runs:
  - on normal exit
  - on early-return error
  - on panic path if practical

### Suggested Files

- `tui/src/terminal.rs`
- `tui/src/lib.rs`

### Acceptance

- Launching the TUI does not corrupt the terminal.
- Exiting the TUI restores the shell prompt normally.
- A controlled error path still restores terminal state.

### Notes

- A small `TerminalGuard` RAII helper is preferred over manual cleanup spread across many branches.

## S1-T03 Minimal Layout

### Objective

Render a minimal but usable three-part UI:

- header
- transcript pane
- composer pane

### Engineering Checklist

- Define a render entrypoint that takes `AppState`.
- Split the frame into:
  - top status/header row
  - main transcript area
  - bottom input area
- Render static placeholders first.
- Replace placeholders with live state-driven content.
- Handle terminal resize without panicking.

### Suggested Files

- `tui/src/render.rs`
- `tui/src/app_loop.rs`

### Acceptance

- Header is visible.
- Transcript area is visible.
- Composer/input area is visible.
- App survives resize events.

### Notes

- Avoid over-styling in Sprint 1.
- The layout only needs to be readable and stable.

## S1-T04 Basic Text Input and Submit

### Objective

Let the user type text, edit basic content, and submit with Enter.

### Engineering Checklist

- Add a simple input handler for:
  - printable characters
  - backspace
  - Enter
- Bind Enter to:
  - copy current input
  - append a `User(...)` transcript entry
  - clear input buffer
  - trigger mock provider response
- Support at least one minimal quit shortcut:
  - `Ctrl+C` or `Esc`

### Suggested Files

- `tui/src/input.rs`
- `core/src/app.rs`
- `core/src/event.rs`

### Acceptance

- User can type text into the composer.
- Backspace works.
- Enter submits the text.
- Submitted text appears in transcript as a user message.

### Notes

- Multi-line editing is optional for Sprint 1.
- If multi-line support slows progress, use single-line input first.

## S1-T05 Mock / Echo Provider

### Objective

Introduce a fake assistant execution path so the TUI can demonstrate a full interaction loop without real provider integration.

### Engineering Checklist

- Implement a mock provider function or task that accepts submitted text.
- Return one of:
  - echo reply
  - templated assistant reply
  - tiny streamed sequence of chunks
- Update app state so the transcript reflects:
  - responding state
  - assistant message
  - return to idle

### Suggested Files

- `core/src/mock_provider.rs`
- `core/src/app.rs`

### Acceptance

- After user submit, assistant output appears without manual injection.
- There is a visible response lifecycle, even if simple.
- The app returns to idle after the mock reply finishes.

### Notes

- A short async delay is useful to verify redraw behavior.
- If streaming is easy, prefer chunked output over one-shot text, but do not block the sprint on it.

## S1-T06 Safe Exit and Basic Error Boundary

### Objective

Ensure the app can quit safely and report basic internal failures without leaving the terminal broken.

### Engineering Checklist

- Define one explicit app quit path.
- Wire `should_quit` into the event loop.
- Catch or surface recoverable runtime errors into transcript/status.
- Add top-level error handling around `run_tui()`.
- Ensure terminal teardown still happens on error.

### Suggested Files

- `tui/src/app_loop.rs`
- `cli/src/main.rs`

### Acceptance

- User can quit the TUI intentionally.
- Controlled runtime errors produce visible feedback or stderr output.
- Terminal state is restored after exit.

### Notes

- Sprint 1 does not need a sophisticated in-TUI error system.
- It only needs a reliable boundary and a stable exit path.

## 8. Recommended Build Order

Implement in this order:

1. `S1-T01` CLI entry
2. `S1-T02` terminal lifecycle
3. `S1-T03` minimal layout
4. `S1-T04` input and submit
5. `S1-T05` mock provider
6. `S1-T06` safe exit and error boundary

Why this order:

- It yields a working shell as early as possible.
- It reduces the chance of spending time on abstractions without a demo path.

## 9. Test Plan

Sprint 1 does not need heavy test coverage, but it needs enough to prevent regression of the main path.

### Minimum automated checks

- CLI parsing tests for:
  - default invocation
  - `doctor`
  - `probe --json`
- App state tests for:
  - submit adds user transcript
  - mock provider adds assistant transcript
- Terminal lifecycle tests where practical:
  - unit test around guard behavior if direct terminal integration is hard

### Manual checks

- Start TUI
- Type and submit a message
- Observe assistant reply
- Quit
- Confirm shell is healthy afterward

## 10. Done Criteria for Sprint 1 Walking Skeleton

The implementation is done only when all of the following are true:

1. `agile-agent` launches a usable TUI.
2. The user can type and submit a message.
3. A mock assistant response appears.
4. The app can be exited intentionally.
5. The terminal is restored on exit.
6. The default command path is stable enough to serve as the base for Sprint 2.

## 11. Explicit Non-Goals

Do not expand this sprint with:

- real Claude subprocess integration
- real Codex app-server integration
- slash commands
- markdown syntax highlighting
- transcript persistence
- skills
- provider switching

Those belong to later stories and later sprints.

## 12. Review Demo Script

Use this exact demo sequence in Sprint Review:

1. `cargo run -p agent-cli --`
2. Show the TUI frame
3. Type `hello`
4. Press Enter
5. Show the mock assistant response
6. Quit
7. Show the shell prompt still works

If this demo is unstable, Sprint 1 is not complete.
