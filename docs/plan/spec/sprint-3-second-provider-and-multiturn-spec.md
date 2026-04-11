# Sprint 3 Second Provider and Multi-Turn Spec

## Metadata

- Sprint: `V1 / Sprint 3`
- Primary stories covered:
  - `V1-S05` TUI conversation with Codex
  - `V1-S06` multi-turn continuity per provider
  - `V1-S08` readable assistant transcript rendering
- Language policy:
  - Specs under `agile-agent/docs/plan/spec/` must be written in English.

## 1. Purpose

Sprint 2 proved that the shell can drive one real provider end to end.

Sprint 3 must prove that `agile-agent` is no longer a single-provider demo shell. It must become a real multi-provider shell with:

- a second real provider (`codex`)
- conversation continuity across turns
- a transcript that is more readable than raw text blobs

At the end of this sprint, the team must be able to demo:

1. Launch `agile-agent`
2. Switch between `mock`, `claude`, and `codex`
3. Submit a prompt with `codex` and see a real response
4. Continue the conversation with the same provider and preserve context
5. Observe a transcript that renders headings, lists, paragraphs, and code blocks readably

## 2. Scope

### In scope

- Real Codex provider execution through `codex app-server --listen stdio://`
- Minimal normalized event mapping from Codex JSON-RPC to the app transcript
- Provider-specific session continuity:
  - Claude via `session_id`
  - Codex via `thread_id`
- Per-provider multi-turn execution within a single TUI session
- Readable transcript rendering for common Markdown structures

### Out of scope

- Cross-process session restore
- Skills integration into real providers
- Slash commands
- Persistent conversation history across app restarts
- Codex approval request UI
- Tool call visualization beyond minimal tolerance
- Multi-provider shared memory or context translation

## 3. Sprint Goal

Turn `agile-agent` into a real multi-provider interactive shell that can maintain conversation continuity with both Claude and Codex and present outputs in a readable transcript format.

## 4. Product Decisions

### 4.1 Codex is the only new provider in this sprint

Sprint 3 should add Codex and stop there.

Rationale:

- `mock` is already the fallback/debug path
- `claude` is already the first real provider
- `codex` is the second real provider needed by the V1 backlog
- adding more providers would reduce feedback quality

### 4.2 Multi-turn continuity is session-local only

Sprint 3 must preserve context across turns during the current app session, but it does not need restart persistence.

Rationale:

- this delivers the real user value of conversation continuity
- it avoids mixing Sprint 3 with persistence complexity

### 4.3 Transcript readability should improve, not become a full document engine

Sprint 3 should render common Markdown structures readably, but it does not need a perfect or full-fidelity Markdown implementation.

The goal is practical readability, not feature completeness.

## 5. Current Baseline

The repo already has:

- a working TUI shell
- a mock provider
- a real Claude provider
- provider selection UX
- visible provider status
- diagnostics via `doctor` and `probe --json`

Sprint 3 should extend this baseline rather than re-architect it.

## 6. Runtime Design for Sprint 3

Sprint 3 should preserve the Sprint 2 event-driven provider model and extend it with provider session continuity.

Runtime shape:

1. App state stores:
   - selected provider
   - current in-memory provider session handles
2. On submit:
   - append user message
   - dispatch to selected provider
   - include any existing session handle for that provider
3. Provider returns:
   - stream events
   - optional updated session handle
4. App stores the updated handle for future turns with that provider
5. Transcript rendering consumes normalized content rather than raw response blobs where possible

## 7. Session Continuity Model

Sprint 3 needs a minimal but explicit session-continuity model.

Suggested shape:

```rust
pub enum SessionHandle {
    Claude { session_id: String },
    Codex { thread_id: String },
}

pub struct SessionRegistry {
    pub claude: Option<String>,
    pub codex: Option<String>,
}
```

The exact type can differ, but the design must preserve:

- provider-specific handle storage
- update-after-success semantics
- no attempt to share session handles across providers

## 8. Codex Execution Strategy

Sprint 3 should use the Codex app-server stdio path:

```text
codex app-server --listen stdio://
```

Minimal flow:

1. start the process
2. send `initialize`
3. start or resume a thread
4. send `turn/start`
5. read notifications
6. normalize assistant text/status/error signals
7. capture the resulting `thread_id`
8. shut down cleanly after the turn

Sprint 3 does not need:

- approval request UI
- remote app-server support
- thread list / read / rollback
- advanced item rendering

If unsupported server requests appear, fail clearly rather than pretending support.

## 9. Transcript Rendering Strategy

Sprint 3 should replace plain one-line assistant rendering with a small readable rendering layer.

Minimum readable support:

- paragraphs
- headings
- unordered and ordered lists
- fenced code blocks
- inline code
- line wrapping

Recommended implementation direction:

- parse Markdown with `pulldown-cmark`
- wrap text with `textwrap`
- use `unicode-width` for width-aware line handling

This sprint does not need:

- syntax highlighting
- clickable links
- tables
- blockquote styling beyond readability

## 10. Detailed Execution Checklist

## S3-T01 Introduce provider session handles in app state

### Objective

Store provider-specific session continuity data in the app state.

### Engineering Checklist

- Add a session handle type in core.
- Add provider session registry to app state.
- Ensure the app can update the handle after provider completion.

### Acceptance

- The app can remember a Claude session handle and a Codex thread handle independently.

## S3-T02 Extend provider event contract with session updates

### Objective

Allow providers to report updated session handles back to the app loop.

### Engineering Checklist

- Add an event variant for session-handle updates or equivalent.
- Keep the event model normalized and provider-agnostic at the app-loop boundary.

### Acceptance

- The app loop can receive and store a provider session update without provider-specific branching in the renderer.

## S3-T03 Refactor Claude provider for multi-turn continuity

### Objective

Use `session_id` returned by Claude to continue the same conversation in later turns.

### Engineering Checklist

- Extract `session_id` from Claude stream-json output.
- Persist it in session-local app state.
- On later turns, invoke Claude with `--resume <session_id>`.

### Acceptance

- Two consecutive Claude turns in one TUI session preserve context.

## S3-T04 Add Codex provider runner

### Objective

Implement a real Codex runner behind the provider contract.

### Engineering Checklist

- Add a Codex provider module.
- Spawn `codex app-server --listen stdio://`.
- Implement minimal JSON-RPC request/response handling.
- Support:
  - `initialize`
  - `thread/start`
  - `thread/resume`
  - `turn/start`
- Stream normalized output events back to the app.

### Acceptance

- A prompt submitted with provider `codex` produces a real response in the transcript.

## S3-T05 Add Codex session continuity

### Objective

Continue a Codex conversation across turns using `thread_id`.

### Engineering Checklist

- Capture `thread_id` from Codex startup or response flow.
- Store it in app state.
- Use `thread/resume` for later turns.

### Acceptance

- Two consecutive Codex turns in one TUI session preserve context.

## S3-T06 Extend provider switching to include Codex

### Objective

Make `codex` a visible and selectable provider in the current switching UX.

### Engineering Checklist

- Update provider enum and selection cycle
- ensure the header reflects `mock`, `claude`, and `codex`
- keep the switching UX minimal and stable

### Acceptance

- The user can cycle to Codex from the TUI.

## S3-T07 Add Codex startup and runtime error handling

### Objective

Make Codex failure paths visible and non-destructive.

### Engineering Checklist

- surface startup failure clearly
- surface JSON-RPC transport failure clearly
- surface malformed output clearly
- keep the shell usable after failure

### Acceptance

- A broken Codex path or protocol failure creates visible user-facing feedback.

## S3-T08 Introduce readable transcript rendering

### Objective

Render assistant output in a more readable form than one raw line per transcript entry.

### Engineering Checklist

- Add a rendering layer for assistant transcript content.
- Parse assistant text as Markdown.
- Render common blocks as wrapped terminal lines.

### Acceptance

- Headings, paragraphs, lists, and code blocks are visibly easier to read than in Sprint 2.

## S3-T09 Keep non-Markdown transcript entries stable

### Objective

Do not degrade user, status, and error transcript readability while improving assistant rendering.

### Engineering Checklist

- Preserve distinct formatting for:
  - user entries
  - assistant entries
  - status entries
  - error entries

### Acceptance

- Provider status and errors remain legible after transcript rendering changes.

## S3-T10 Add automated coverage for Codex parsing and multi-turn state

### Objective

Ensure the second-provider path is not only manually tested.

### Engineering Checklist

- Add tests for:
  - Codex JSON-RPC parsing
  - session handle updates
  - provider switching
  - Claude multi-turn session reuse
  - readable transcript rendering behavior where feasible

### Acceptance

- The riskiest Sprint 3 behaviors have automated test coverage.

## 11. Proposed File Layout After Sprint 3

```text
agile-agent/
└── crates/
    ├── agile-agent-core/
    │   └── src/
    │       ├── app.rs
    │       ├── probe.rs
    │       ├── provider.rs
    │       ├── transcript.rs
    │       └── providers/
    │           ├── claude.rs
    │           ├── codex.rs
    │           └── mod.rs
    └── agile-agent-tui/
        └── src/
            ├── app_loop.rs
            ├── input.rs
            ├── render.rs
            ├── transcript_render.rs
            └── terminal.rs
```

This is a recommendation. Keep the structure small if a leaner layout is enough.

## 12. Recommended Build Order

Implement in this order:

1. `S3-T01` session handles in app state
2. `S3-T02` session-update event contract
3. `S3-T03` Claude multi-turn continuity
4. `S3-T04` Codex runner
5. `S3-T05` Codex multi-turn continuity
6. `S3-T06` Codex provider selection
7. `S3-T07` Codex error handling
8. `S3-T08` readable transcript rendering
9. `S3-T09` transcript readability for status/error/user entries
10. `S3-T10` automated tests

Why this order:

- It upgrades continuity for the already-working provider first.
- It then adds the second provider.
- It leaves rendering polish until after provider behavior is stable.

## 13. Test Plan

### Automated checks

- `cargo fmt`
- `cargo test`
- `cargo check`

### Manual smoke checks

#### Claude continuity

1. launch the app
2. submit a Claude prompt
3. submit a second Claude prompt that depends on the first response
4. confirm context continuity

#### Codex success path

1. switch to Codex
2. submit a simple prompt
3. confirm transcript output appears
4. submit a follow-up turn
5. confirm context continuity

#### Codex failure path

1. force a broken Codex path
2. switch to Codex
3. submit a prompt
4. confirm visible error feedback

#### Rendering path

1. submit or inject assistant content containing:
   - headings
   - lists
   - code block
2. confirm transcript readability is improved

## 14. Done Criteria for Sprint 3

Sprint 3 is done only when all of the following are true:

1. `mock`, `claude`, and `codex` are visible provider choices.
2. Claude can maintain context across multiple turns in one TUI session.
3. Codex can execute a real prompt from inside the TUI.
4. Codex can maintain context across multiple turns in one TUI session.
5. The transcript is materially more readable for normal assistant Markdown output.
6. Provider failures remain visible and non-destructive.

## 15. Explicit Non-Goals

Do not expand this sprint with:

- persistent session restore across app restarts
- skills on real providers
- slash commands
- Codex approval UX
- transcript syntax highlighting
- V2 backlog/task automation

Those belong to later work.

## 16. Review Demo Script

Use this review sequence:

### Demo A: Claude continuity

1. launch app
2. use Claude
3. ask a question
4. ask a follow-up that depends on earlier context

### Demo B: Codex integration

1. switch to Codex
2. submit a prompt
3. observe result
4. submit a follow-up prompt

### Demo C: Transcript readability

1. show assistant output containing Markdown structure
2. compare readability against Sprint 2 style

### Demo D: Failure visibility

1. break Codex path
2. submit prompt
3. show visible error

If Demo B or Demo C is unstable, Sprint 3 is not complete.

## 17. Retrospective Prompts

At the end of Sprint 3, ask:

1. Is the provider contract still clean after adding Codex?
2. Is the session continuity model sufficient for V1, or already showing V2 pressure?
3. Did transcript rendering stay simple enough, or is a separate formatting layer now required?
4. Is the shell ready for Sprint 4, or do provider abstractions need another cleanup pass first?
