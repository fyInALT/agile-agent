# Sprint 2 First Real Provider Spec

## Metadata

- Sprint: `V1 / Sprint 2`
- Primary stories covered:
  - `V1-S04` TUI conversation with Claude
  - `V1-S07` provider switching and visible provider state
  - `V1-S12` clear provider error surfacing
- Language policy:
  - Specs under `agile-agent/docs/plan/spec/` must be written in English.

## 1. Purpose

This spec defines the engineering plan for the second runnable vertical slice of `agile-agent`.

Sprint 1 proved that the shell itself works:

- the CLI exists
- the TUI exists
- input can be submitted
- a mock assistant reply can be rendered
- `doctor` and `probe --json` work

Sprint 2 must prove that the shell can drive one real provider end to end.

For this sprint, the first real provider will be **Claude**.

At the end of this work, the team must be able to demo:

1. Launch `agile-agent`
2. See that the current provider is `mock` or `claude`
3. Switch to `claude`
4. Submit a real prompt
5. Observe streamed Claude output in the transcript
6. See a useful error if Claude is unavailable or fails
7. Quit cleanly

## 2. Scope

### In scope

- Minimal provider abstraction sufficient for `mock` and `claude`
- Visible current-provider state in the TUI
- Provider switching between `mock` and `claude`
- Real Claude subprocess execution
- Streamed assistant output from Claude into the transcript
- Clear provider startup / runtime / parsing error feedback

### Out of scope

- Codex integration
- Multi-turn session reuse
- Session persistence
- Skills
- Slash commands
- Rich Markdown rendering changes
- Background workers
- Task orchestration

## 3. Sprint Goal

Convert `agile-agent` from a mock-only shell into a shell that can successfully execute one real provider interaction while keeping the mock path available as a fallback and debugging aid.

## 4. Product Decisions

### 4.1 Claude first

Sprint 2 will integrate Claude before Codex.

Rationale:

- Claude already supports a straightforward print-style streaming interface.
- It avoids introducing JSON-RPC app-server complexity too early.
- It validates the main provider-bridge architecture with less moving parts.

### 4.2 Keep mock provider alive

The mock provider must remain available in Sprint 2.

Rationale:

- It is still useful for UI debugging.
- It gives a known-good path when real provider debugging is in progress.
- It reduces the chance that all interactive testing becomes blocked on external tool issues.

### 4.3 Do not introduce Codex in the same sprint

Codex is intentionally excluded from Sprint 2.

Rationale:

- Real provider integration is already a large uncertainty surface.
- Claude and Codex use different execution contracts.
- Mixing both in one sprint would reduce feedback quality and blur the sprint review.

## 5. Current Baseline

The repo already has:

- a workspace
- CLI routing
- a TUI event loop
- terminal lifecycle handling
- a three-pane layout
- local input and submit behavior
- a mock reply path
- `doctor`
- `probe --json`

Sprint 2 should build on this baseline, not replace it.

## 6. Runtime Design for Sprint 2

Sprint 2 should use this runtime shape:

1. The app state stores a selected provider kind.
2. On submit:
   - append the user message
   - dispatch to the selected provider
3. The selected provider emits a stream of normalized app events
4. The app loop consumes those events and updates transcript state
5. Errors are surfaced into the transcript and/or status area

The runtime should remain single-session and single-request-at-a-time for this sprint.

## 7. Proposed Provider Contract

Sprint 2 does not need the final provider system, but it does need a clean enough interface to avoid hard-coding Claude-specific behavior into the TUI loop.

Suggested shape:

```rust
pub enum ProviderKind {
    Mock,
    Claude,
}

pub enum ProviderEvent {
    Status(String),
    AssistantChunk(String),
    Error(String),
    Finished,
}

pub trait ProviderRunner {
    fn run(
        prompt: String,
        event_tx: std::sync::mpsc::Sender<ProviderEvent>,
    ) -> anyhow::Result<()>;
}
```

The final signature may differ, but Sprint 2 must preserve these ideas:

- provider execution is not owned by the renderer
- provider output is normalized before rendering
- mock and Claude share the same high-level event path

## 8. Claude Execution Strategy

Sprint 2 should use the simplest stable Claude path available:

```text
claude -p --output-format stream-json --input-format stream-json --verbose
```

Input model:

- write a single user message to stdin
- close stdin
- read NDJSON events from stdout

Initial event mapping:

- assistant text => transcript assistant chunks
- system/log => status or debug lines if helpful
- result => mark request complete
- malformed output => error event

Sprint 2 does **not** need:

- control request handling
- approval request handling
- session resume
- tool-use rendering beyond minimal tolerance

If Claude emits unsupported event shapes, the app should fail clearly rather than pretending full support.

## 9. Detailed Execution Checklist

## S2-T01 Introduce provider selection state

### Objective

Add selected provider state to the app so the shell can switch between `mock` and `claude`.

### Engineering Checklist

- Define a `ProviderKind` enum in core.
- Add selected provider to app state.
- Default to:
  - `claude` if available, or
  - `mock` if Claude is unavailable
- Keep this decision explicit and visible.

### Acceptance

- The app always knows which provider is selected.
- The selected provider is rendered in the UI.

## S2-T02 Add a minimal provider switching UX

### Objective

Let the user switch between `mock` and `claude` before submitting a prompt.

### Engineering Checklist

- Add one minimal switching mechanism.
- Acceptable choices for this sprint:
  - `Tab`
  - a single-key toggle
  - a tiny provider picker popup
- Keep it discoverable in the header or footer.

### Acceptance

- The user can switch providers without restarting the app.
- The current provider indicator updates immediately.
- The next submitted message uses the new provider.

### Notes

- Do not introduce a full slash command system just for this.

## S2-T03 Move mock execution behind the provider contract

### Objective

Refactor the current mock reply path so it uses the same normalized event flow that Claude will use.

### Engineering Checklist

- Convert the mock provider from direct transcript mutation to provider events.
- Ensure the TUI loop consumes provider events uniformly.

### Acceptance

- Mock provider behavior remains working.
- The TUI no longer needs provider-specific branching in the render layer.

## S2-T04 Implement Claude runner

### Objective

Spawn Claude, write input, and stream output back into the app.

### Engineering Checklist

- Add a Claude runner implementation.
- Start the subprocess with the chosen CLI invocation.
- Write a single structured user message to stdin.
- Read stdout line by line.
- Parse JSON lines into a minimal internal representation.
- Emit normalized provider events.

### Suggested file targets

- `core/src/provider.rs`
- `core/src/providers/mock.rs`
- `core/src/providers/claude.rs`

The exact file layout can vary, but keep Claude-specific code out of the TUI crate.

### Acceptance

- A prompt submitted with provider `claude` launches the real Claude subprocess.
- At least assistant text output reaches the transcript.

## S2-T05 Stream assistant output into the transcript

### Objective

Render Claude output incrementally rather than waiting for the full final result.

### Engineering Checklist

- Add a provider event queue into the app loop.
- Support incremental assistant updates.
- Reuse the existing assistant transcript entry when possible.
- Keep UI state stable while the provider is running.

### Acceptance

- Users can observe visible incremental output for a real Claude response.
- The UI returns to idle after the provider finishes.

## S2-T06 Add provider execution state and request locking

### Objective

Prevent overlapping requests and make execution state visible.

### Engineering Checklist

- While a provider request is running:
  - mark app status as busy
  - disable or ignore new submit attempts
- Surface the busy state in header or footer text.

### Acceptance

- The app cannot start a second provider request while one is already active.
- The user can tell when the app is waiting on provider output.

## S2-T07 Add clear provider startup failure messaging

### Objective

Surface meaningful feedback when Claude cannot be started.

### Engineering Checklist

- Detect common failure paths:
  - executable not found
  - startup failure
  - stdout pipe failure
  - stdin pipe failure
- Convert them into visible error output.

### Acceptance

- A broken Claude path results in an understandable error message.
- The app remains usable afterward, at least with `mock`.

## S2-T08 Add clear provider output / parsing failure messaging

### Objective

Avoid silent failures when Claude output cannot be parsed or finishes unexpectedly.

### Engineering Checklist

- Handle malformed JSON lines
- Handle empty or incomplete streams
- Handle non-zero exit status
- Surface transcript-visible or status-visible errors

### Acceptance

- Unsupported or malformed output does not crash the shell silently.
- The user sees that the provider failed.

## S2-T09 Expand doctor / probe to support provider usability hints

### Objective

Use the existing diagnostics path to help the user understand why Claude may not be selectable or runnable.

### Engineering Checklist

- Keep `doctor` and `probe --json` consistent with Sprint 1 shape.
- Add any minimal extra fields only if genuinely necessary.
- Prefer stable structure over verbose output.

### Acceptance

- Diagnostics remain backward-stable for Sprint 1 consumers.
- Any new provider-related hints stay additive, not breaking.

## S2-T10 Add automated coverage for provider selection and Claude parsing

### Objective

Ensure Sprint 2 does not rely only on manual TTY tests.

### Engineering Checklist

- Add tests for:
  - provider selection state
  - mock provider event flow
  - Claude line parsing
  - error mapping
- Use fixture lines for Claude stream-json output where possible.

### Acceptance

- Core provider behavior has automated coverage.
- Parsing logic is testable without launching a real TTY.

## 10. Proposed File Layout After Sprint 2

```text
agile-agent/
├── cli/
├── core/
│   └── src/
│       ├── app.rs
│       ├── event.rs
│       ├── probe.rs
│       ├── provider.rs
│       └── providers/
│           ├── mod.rs
│           ├── mock.rs
│           └── claude.rs
└── tui/
    └── src/
        ├── app_loop.rs
        ├── input.rs
        ├── render.rs
        ├── terminal.rs
        └── lib.rs
```

This is a recommendation, not a hard constraint. Keep the file structure small if a simpler layout works better.

## 11. Recommended Build Order

Implement in this order:

1. `S2-T01` provider selection state
2. `S2-T03` mock provider behind provider contract
3. `S2-T02` provider switching UX
4. `S2-T04` Claude runner
5. `S2-T05` streamed transcript updates
6. `S2-T06` request locking and busy state
7. `S2-T07` startup error handling
8. `S2-T08` output / parsing error handling
9. `S2-T09` diagnostics polish
10. `S2-T10` automated tests

Why this order:

- It preserves a working app at each step.
- It upgrades the mock path before introducing the real provider.
- It keeps provider execution concerns out of the rendering code.

## 12. Test Plan

### Automated checks

- `cargo fmt`
- `cargo test`
- `cargo check`

### Automated test areas

- provider selection state
- mock provider event stream
- Claude line parser
- startup error mapping
- malformed output mapping

### Manual smoke checks

#### Mock path

1. Launch app
2. Submit prompt with `mock`
3. Observe reply
4. Quit cleanly

#### Claude path

1. Launch app
2. Switch provider to `claude`
3. Submit a simple prompt
4. Observe streamed output
5. Confirm app returns to idle
6. Quit cleanly

#### Error path

1. Override Claude path to a missing binary
2. Launch app
3. Switch to `claude`
4. Submit prompt
5. Observe clear failure feedback
6. Confirm shell remains usable

## 13. Done Criteria for Sprint 2

Sprint 2 is done only when all of the following are true:

1. The shell supports `mock` and `claude` as visible provider choices.
2. The user can switch providers without restarting.
3. Claude can execute a real prompt from inside the TUI.
4. Claude output is rendered back into the transcript.
5. A broken Claude path produces a clear visible error.
6. The app remains stable and exits cleanly after both success and failure paths.

## 14. Explicit Non-Goals

Do not expand this sprint with:

- Codex integration
- session resume
- multi-turn memory
- session persistence
- skills
- slash commands
- task orchestration

Those belong to later stories and later sprints.

## 15. Review Demo Script

Use this exact demo sequence in Sprint Review:

### Demo A: Mock path still works

1. `cargo run -p agent-cli --`
2. Submit a prompt with `mock`
3. Observe reply

### Demo B: Real Claude path works

1. Switch provider to `claude`
2. Submit a simple prompt
3. Observe streamed output
4. Return to idle

### Demo C: Error path is visible

1. Run with broken Claude path override
2. Submit with `claude`
3. Show clear error handling

If Demo B or Demo C is unstable, Sprint 2 is not complete.

## 16. Retrospective Prompts

At the end of Sprint 2, ask:

1. Did Claude-first reduce complexity as expected?
2. Is the provider contract clean enough to support Codex next?
3. Did we accidentally hard-code provider logic into the TUI?
4. Is provider error handling good enough for real-world iteration?
5. Should Sprint 3 prioritize Codex integration or multi-turn session reuse?
