# TUI Parity with Codex Spec

## Metadata

- Scope: `agile-agent` TUI parity refactor
- Status: approved for implementation
- Primary reference:
  - `../codex/codex-rs/tui`

## 1. Purpose

The current `agile-agent` TUI is functionally usable but structurally closer to a prototype than to
`codex`.

This spec defines the refactor required to make the `agile-agent` terminal experience behave much
closer to `codex`, especially in these areas:

- layout and information hierarchy
- multiline composer behavior
- transcript rendering
- Markdown rendering
- transcript browsing and scrolling

The goal is not to port every `codex` feature. The goal is to make the `agile-agent` shell feel
like the same class of TUI.

## 2. Product Goal

After this refactor, a user opening `agile-agent` should see a TUI that feels aligned with
`codex` in the following ways:

1. the main screen is a viewport plus bottom composer, not two boxed demo panels
2. the composer supports real multiline editing and paste
3. assistant output renders as readable Markdown
4. transcript browsing supports pager-style navigation
5. streaming content, tool activity, and status updates do not visually collapse into one text box

## 3. In Scope

- replace the current boxed transcript/composer layout
- introduce a dedicated TUI state layer
- introduce a codex-like bottom pane with multiline textarea behavior
- render transcript content through typed renderable cells
- introduce a transcript overlay/pager
- add transcript scrolling keybindings
- replace ad-hoc Markdown rendering with a dedicated renderer
- add snapshot-style or unit coverage for layout/Markdown/scroll behavior where practical

## 4. Out of Scope

- full `codex` feature parity
- plugin popups
- approval overlays
- MCP-specific overlays
- multi-agent UI
- voice UI
- external editor integration
- inline terminal scrollback replay and custom terminal backend parity

Those belong to later phases if needed.

## 5. Key References in Codex

The refactor should specifically follow these architectural cues from `codex`:

- `codex-rs/tui/src/chatwidget.rs`
  - transcript cells
  - active in-flight cell
  - viewport-oriented rendering
- `codex-rs/tui/src/bottom_pane/chat_composer.rs`
  - bottom-pane composer state machine
- `codex-rs/tui/src/bottom_pane/textarea.rs`
  - multiline textarea, wrapping, cursor visibility, scrolling
- `codex-rs/tui/src/markdown_render.rs`
  - width-aware Markdown rendering
- `codex-rs/tui/src/pager_overlay.rs`
  - full transcript pager behavior

`agile-agent` should adopt the same shape, but only for the subset needed by the current product.

## 6. Architectural Decisions

### 6.1 Separate domain state from TUI state

`agent_core::app::AppState` should remain the domain/session state.

The TUI crate should introduce a separate UI-facing state model for:

- composer text area state
- transcript viewport state
- overlay open/close state
- selected popup state
- scroll offsets

### 6.2 Replace transcript-as-paragraph with transcript-as-cells

The transcript must no longer be rendered as one large `Paragraph`.

Instead, it should be rendered as a sequence of typed cells:

- user message cell
- assistant message cell
- thinking cell
- tool call cell
- status cell
- error cell

This is required for:

- stable spacing
- per-cell wrapping
- good streaming behavior
- future overlay support

### 6.3 Composer must be stateful and multiline

The composer must own:

- full text buffer
- cursor position
- wrapped-line state
- scroll state inside the composer

This should be implemented in a dedicated textarea module rather than through `AppState.input`.

### 6.4 Markdown must be width-aware

Markdown rendering must happen through a dedicated renderer that accepts width and returns rendered
terminal lines.

This renderer should handle at least:

- headings
- paragraphs
- ordered lists
- unordered lists
- blockquotes
- fenced code blocks
- indented code blocks
- inline code
- emphasis
- strong
- links

### 6.5 Transcript browsing should be pager-style

The main viewport should remain focused on the current conversation surface.

Full transcript browsing should be handled through an overlay/pager similar to `codex`:

- open with `Ctrl+T`
- close with `Esc` or `q`
- scroll with `Up/Down`
- page with `PageUp/PageDown` and `Space`
- jump with `Home/End`

## 7. File and Module Plan

The TUI crate should be reorganized roughly like this:

```text
tui/src/
├── lib.rs
├── app_loop.rs
├── terminal.rs
├── input.rs
├── layout.rs
├── ui_state.rs
├── transcript/
│   ├── mod.rs
│   ├── cells.rs
│   ├── viewport.rs
│   └── overlay.rs
├── composer/
│   ├── mod.rs
│   ├── textarea.rs
│   └── footer.rs
└── markdown.rs
```

Exact filenames may vary, but the design must end up with these responsibilities clearly split.

## 8. Detailed Task List

## T1 Add TUI-specific state

- Add a UI state type in the TUI crate.
- Store transcript viewport state, overlay state, and composer state there.
- Stop using `AppState.input` as the primary editing model.

Acceptance:

- TUI draw path compiles against the new UI state.
- Existing provider/chat flows still work.

## T2 Add multiline textarea module

- Add a dedicated textarea implementation.
- Support:
  - character insertion
  - backspace
  - cursor left/right
  - cursor up/down across wrapped lines
  - paste insertion
  - internal vertical scrolling to keep the cursor visible
- Keep submit behavior simple:
  - `Enter` submits
  - `Ctrl+J` inserts newline

Acceptance:

- composer text can span multiple visible rows
- pasted multiline text remains editable
- cursor remains visible while editing long content

## T3 Replace boxed layout with codex-like viewport + bottom pane

- Remove the current bordered transcript/composer presentation.
- Render:
  - a main viewport for transcript cells
  - a bottom pane for the composer
  - a small footer/status strip
- Keep the skill browser as an overlay if still needed.

Acceptance:

- the UI no longer looks like stacked demo widgets
- transcript and composer have clear visual separation without heavy box chrome

## T4 Introduce transcript cell model

- Create renderable transcript cells from `TranscriptEntry`.
- User, assistant, thinking, tool, status, and error entries should render independently.
- Streaming assistant chunks should update the active assistant cell instead of forcing one large
  monolithic paragraph rebuild.

Acceptance:

- transcript spacing is stable during streaming
- tool/status/error rows render as distinct blocks

## T5 Add width-aware Markdown renderer

- Move Markdown rendering out of `render.rs` into a dedicated module.
- Make it width-aware.
- Add focused tests for the supported Markdown constructs.

Acceptance:

- headings, lists, blockquotes, links, and code blocks render readably
- ordered lists remain ordered after wrapping
- code blocks remain visually distinct

## T6 Add transcript viewport rendering

- Render transcript cells into a viewport area with tail-follow behavior.
- Keep the default main view pinned near the newest content while the user is not actively reading
  old content.
- Track main-view scroll state separately from overlay scroll state.

Acceptance:

- long transcripts remain readable
- the main viewport does not visually collapse when many messages exist

## T7 Add transcript overlay / pager

- Add a full transcript overlay.
- Wire keybindings:
  - `Ctrl+T`
  - `Esc`
  - `q`
  - `Up`
  - `Down`
  - `PageUp`
  - `PageDown`
  - `Home`
  - `End`
  - `Space`
- Render a small pager header/footer with scroll affordance.

Acceptance:

- `Ctrl+T` opens the transcript overlay
- overlay scrolling works independently of the main view
- closing the overlay restores the normal chat surface

## T8 Improve footer and key hints

- Add a codex-like footer/status line under the composer.
- Surface:
  - provider
  - cwd or session context
  - key hints
  - loop state when relevant
- Remove noisy header text from the top line.

Acceptance:

- primary operational hints live near the composer
- the interface feels closer to codex than to a debug console

## T9 Reconcile existing interactions

- Keep skill browser support, but make sure it behaves as an overlay over the new shell.
- Move provider switching off `Tab`.
- Reserve `Tab` for future composer behavior or leave it inert.
- Ensure slash commands still work through the new composer.

Acceptance:

- provider switching still exists through a non-conflicting interaction path
- slash commands still submit correctly
- skill browser does not break the new layout

## T10 Add regression coverage

- Add unit tests or snapshot-style tests for:
  - Markdown rendering
  - textarea wrapping and scrolling
  - transcript cell rendering
  - overlay opening and pager navigation where practical

Acceptance:

- the new TUI structure has targeted test coverage

## 9. Suggested Commit Shape

The implementation should stay in small commits. Suggested order:

1. `docs: add codex-style tui parity spec`
2. `refactor: add tui state and layout scaffolding`
3. `feat: add multiline composer textarea`
4. `feat: render transcript as structured cells`
5. `feat: add width-aware markdown renderer`
6. `feat: add transcript viewport and pager overlay`
7. `feat: add codex-style footer and key handling`
8. `test: cover tui markdown and scrolling behavior`

Commit messages should describe behavior only and should not mention sprint numbers.

## 10. Acceptance Criteria

This work is complete when all of the following are true:

1. The main TUI layout is viewport + bottom pane, not stacked bordered widgets.
2. The composer supports multiline editing and paste in a codex-like way.
3. Transcript content is rendered as typed cells, not one aggregated paragraph.
4. Assistant Markdown is rendered through a dedicated width-aware renderer.
5. The user can open and scroll a full transcript overlay with pager-style controls.
6. Long conversations remain readable without visual collapse.
7. Existing chat/provider/skills flows still work after the refactor.
8. `cargo test` and `cargo check` pass.

## 11. Non-Goals for This Pass

Even after this work, `agile-agent` will still not have:

- codex plugin marketplace UI
- approval modal parity
- multi-agent overlay parity
- custom terminal inline-scrollback parity
- full codex popup catalog

This pass is strictly about the core shell experience.
