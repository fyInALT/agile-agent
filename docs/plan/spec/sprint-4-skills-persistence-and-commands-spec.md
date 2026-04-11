# Sprint 4 Skills, Persistence, and Commands Spec

## Metadata

- Sprint: `V1 / Sprint 4`
- Primary stories covered:
  - `V1-S09` discover and browse local skills
  - `V1-S10` enable skills and apply them to the next turn
  - `V1-S11` persist transcript and restore the most recent session
  - `V1-S13` minimal slash commands
- Language policy:
  - Specs under `agile-agent/docs/plan/spec/` must be written in English.

## 1. Purpose

Sprint 3 made `agile-agent` a real multi-provider shell with multi-turn continuity and readable assistant transcript rendering.

Sprint 4 must complete the remaining V1 user-completeness slice:

- discover local skills from disk
- let users browse and toggle skills in the TUI
- inject enabled skills into the next provider turn
- persist the current transcript locally
- restore the most recent saved session
- provide a small command vocabulary inside the composer

At the end of this sprint, the team must be able to demo:

1. launch `agile-agent`
2. open a skills view from inside the TUI
3. enable or disable a skill
4. submit a prompt and show that the enabled skill is included in the provider request
5. quit the app, relaunch with recent-session restore, and see the prior transcript again
6. use minimal slash commands like `/help`, `/provider`, `/skills`, `/doctor`, and `/quit`

## 2. Scope

### In scope

- local skill discovery from agreed directories
- lightweight skill metadata extraction
- TUI skill browsing
- skill enable/disable state
- prompt injection of enabled skill context for the next provider turn
- local transcript persistence
- recent-session restore
- minimal slash commands

### Out of scope

- remote or marketplace skill installation
- executing scripts from skills
- advanced skill dependency graphs
- cross-project session search
- full command palette
- full persistent session indexing
- provider-specific session restore across process restarts beyond “most recent transcript restore”

## 3. Sprint Goal

Turn `agile-agent` from a capable shell into a shell that feels usable in day-to-day work by adding local skills, basic continuity across restarts, and a small but real command surface.

## 4. Product Decisions

### 4.1 Skills are local context files, not active automation

In Sprint 4, a skill is a local `SKILL.md` file whose body is injected into the next provider turn.

This sprint does not execute skill scripts or assets.

### 4.2 The skill UX should be simple and visible

The goal is not a full plugin browser. The user only needs to:

- discover local skills
- browse them
- toggle them
- see which ones are enabled

### 4.3 Session restore is “recent session first”

Sprint 4 only needs a practical continuity feature:

- save the current transcript
- restore the most recent session

It does not need a full session history browser yet.

### 4.4 Slash commands remain intentionally small

Sprint 4 should not grow into a command framework. Only the agreed minimal commands are in scope.

## 5. Current Baseline

The repo already has:

- real `mock`, `claude`, and `codex` providers
- multi-turn continuity inside the current app session
- readable transcript rendering
- provider switching
- diagnostics via `doctor` and `probe --json`

Sprint 4 should extend this baseline without undoing the provider/event architecture.

## 6. Runtime Design for Sprint 4

Sprint 4 should add three small subsystems:

1. `SkillRegistry`
   - discovers skills
   - tracks enabled state
2. `SessionStore`
   - saves transcript snapshots
   - restores the most recent session
3. `CommandRouter`
   - parses and handles a minimal set of slash commands

These should integrate into the existing app loop and not replace it.

## 7. Skill Model

Suggested minimal shape:

```rust
pub struct SkillMetadata {
    pub name: String,
    pub description: String,
    pub path: PathBuf,
    pub body: String,
}

pub struct SkillRegistry {
    pub discovered: Vec<SkillMetadata>,
    pub enabled_names: BTreeSet<String>,
}
```

Required properties:

- stable skill identity
- readable display name
- source path
- injectable body text

### Discovery order

Sprint 4 should support:

1. `<cwd>/.agile-agent/skills/*/SKILL.md`
2. `<cwd>/skills/*/SKILL.md`
3. `~/.config/agile-agent/skills/*/SKILL.md`

## 8. Skill Injection Strategy

Sprint 4 should inject enabled skills into the next turn as plain text context.

Recommended structure:

```text
[Agile Agent Skill Context]
The following local skills are enabled for this turn.

## Skill: <name>
Path: <path>
<body>

[End Agile Agent Skill Context]
```

Provider-specific injection:

- Claude:
  - prepend or append the skill context to the user prompt for this sprint
- Codex:
  - prepend or append the skill context to the user prompt for this sprint

Sprint 4 does not need a separate developer-instructions channel per provider.
It only needs a reliable and visible skill effect path.

## 9. Session Persistence Model

Suggested minimal shape:

```rust
pub struct PersistedSession {
    pub saved_at: String,
    pub selected_provider: String,
    pub claude_session_id: Option<String>,
    pub codex_thread_id: Option<String>,
    pub transcript: Vec<PersistedTranscriptEntry>,
}
```

Requirements:

- enough information to rebuild the transcript view
- enough provider state to preserve the current in-memory continuity direction when possible
- a stable on-disk format

Suggested storage locations:

- sessions root:
  - `~/.local/share/agile-agent/sessions/`
- recent-session pointer:
  - `~/.local/share/agile-agent/recent-session.json`

## 10. Slash Command Model

Sprint 4 should support this exact minimum set:

- `/help`
- `/provider`
- `/skills`
- `/doctor`
- `/quit`

Behavior expectations:

- `/help`
  - prints available commands into the transcript
- `/provider`
  - prints the current provider and switching hint into the transcript
- `/skills`
  - opens the skill browser or prints the current skill state
- `/doctor`
  - renders a short diagnostic summary into the transcript
- `/quit`
  - exits the app cleanly

Unsupported slash commands must return a visible error message.

## 11. Detailed Execution Checklist

## S4-T01 Add a skill registry in core

### Objective

Introduce a minimal local skill model and discovery path.

### Engineering Checklist

- add `SkillMetadata`
- add `SkillRegistry`
- implement discovery for the agreed directories
- ignore unreadable or malformed entries gracefully

### Acceptance

- the app can discover local skills from disk without crashing on bad files

## S4-T02 Parse minimal skill metadata

### Objective

Extract enough information to render a useful skill list.

### Engineering Checklist

- derive skill name from frontmatter if present, otherwise directory name
- derive short description from frontmatter if present, otherwise first meaningful paragraph
- load full skill body for later injection

### Acceptance

- each discovered skill has:
  - name
  - description
  - path
  - body

## S4-T03 Add a TUI skills browser

### Objective

Make skills visible and browsable in the TUI.

### Engineering Checklist

- add a minimal skill browser surface
- allow opening it via one of:
  - `$`
  - `/skills`
  - both
- show name, description, and enabled state

### Acceptance

- the user can open a skill list from inside the TUI
- the user can browse multiple entries

## S4-T04 Add skill enable/disable behavior

### Objective

Allow the user to toggle local skills on and off.

### Engineering Checklist

- add per-session enabled state
- support keyboard toggling inside the skill browser
- reflect enabled state in the header, transcript, or browser

### Acceptance

- the user can enable and disable at least one skill
- enabled state is visible

## S4-T05 Inject enabled skills into the next turn

### Objective

Make enabled skills actually affect provider input.

### Engineering Checklist

- build a single skill-context block from enabled skills
- inject it into the next submitted turn
- do not inject anything when no skill is enabled

### Acceptance

- enabling a skill changes the next provider request payload
- disabling it removes that injected context from later turns

## S4-T06 Persist the current session locally

### Objective

Save the current transcript and relevant provider state to disk.

### Engineering Checklist

- define a persisted session format
- write it on clean exit
- update the recent-session pointer

### Acceptance

- after using the app, a recent session file exists on disk

## S4-T07 Restore the most recent session

### Objective

Rebuild the transcript from the last saved session.

### Engineering Checklist

- add a CLI entry for recent restore
- load the recent-session pointer
- rebuild transcript and provider/session state

Suggested UX:

- `agile-agent --resume-last`
  or
- `agile-agent resume-last`

Choose one and keep it stable.

### Acceptance

- the user can relaunch and restore the most recent saved transcript

## S4-T08 Add minimal slash command parsing

### Objective

Introduce a small command vocabulary without building a full command framework.

### Engineering Checklist

- detect slash commands when the submitted input starts with `/`
- route the minimal commands only
- keep normal prompt submission unchanged otherwise

### Acceptance

- `/help`, `/provider`, `/skills`, `/doctor`, and `/quit` all work

## S4-T09 Add transcript-visible command feedback

### Objective

Make slash commands observable and understandable.

### Engineering Checklist

- push command results into transcript entries
- surface unsupported commands as transcript errors

### Acceptance

- users can tell what a command did without inspecting stderr

## S4-T10 Add automated tests for skills, persistence, and slash commands

### Objective

Avoid relying only on manual demos for the last V1 completeness features.

### Engineering Checklist

- add skill discovery tests with temp directories
- add session save/load tests
- add slash command routing tests
- add prompt-injection tests for enabled skills

### Acceptance

- the new Sprint 4 behavior has direct automated coverage

## 12. Recommended Build Order

Implement in this order:

1. `S4-T01` skill registry
2. `S4-T02` skill metadata extraction
3. `S4-T03` skill browser
4. `S4-T04` skill toggling
5. `S4-T05` skill injection
6. `S4-T06` session persistence
7. `S4-T07` recent restore
8. `S4-T08` slash command parsing
9. `S4-T09` command feedback
10. `S4-T10` automated tests

Why this order:

- it delivers visible skill value before persistence
- it keeps persistence independent from the command system
- it preserves the existing provider/runtime path while adding local usability features

## 13. Test Plan

### Automated checks

- `cargo fmt`
- `cargo test`
- `cargo check`

### Manual smoke checks

#### Skills path

1. create one or more local `SKILL.md` files
2. launch app
3. open the skill browser
4. enable one skill
5. submit a prompt
6. confirm the enabled skill context affects the next provider request

#### Recent restore path

1. launch app
2. create a short conversation
3. exit cleanly
4. relaunch via recent-session restore
5. confirm the transcript is restored

#### Slash command path

1. launch app
2. run `/help`
3. run `/provider`
4. run `/skills`
5. run `/doctor`
6. run `/quit`

## 14. Done Criteria for Sprint 4

Sprint 4 is done only when all of the following are true:

1. local skills can be discovered in the app
2. skills can be browsed and toggled
3. enabled skills affect the next provider turn
4. the current session is saved locally
5. the most recent session can be restored
6. the minimal slash command set works and produces visible feedback

## 15. Explicit Non-Goals

Do not expand this sprint with:

- skill script execution
- marketplace install flows
- full session history browsing
- slash command framework generalization
- V2 task/backlog automation

Those belong to later work.

## 16. Review Demo Script

Use this sequence in Sprint Review:

### Demo A: Skills

1. show local skill discovery
2. enable a skill
3. submit a prompt
4. show that the skill affects the turn

### Demo B: Recent restore

1. create a short conversation
2. exit
3. restore the recent session
4. show transcript continuity

### Demo C: Slash commands

1. run `/help`
2. run `/provider`
3. run `/skills`
4. run `/doctor`
5. run `/quit`

If Demo A or Demo B is unstable, Sprint 4 is not complete.

## 17. Retrospective Prompts

At the end of Sprint 4, ask:

1. Is the shell now complete enough to count as V1 done?
2. Did the skill model stay intentionally small, or did it start drifting toward a plugin system?
3. Is the persistence model stable enough for V2 to build on?
4. Did slash commands stay small and useful, or are they pushing toward a larger command framework?
