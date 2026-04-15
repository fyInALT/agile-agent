# Slash Command System Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a namespaced slash command framework for `agile-agent` that supports local commands, agent-scoped semantic commands, and raw provider slash-command passthrough with explicit target resolution.

**Architecture:** Introduce a core command bus for parsing, command metadata, and legacy alias mapping, then add a TUI command runtime that resolves defaults from the current focus and executes `local`, `agent`, and `provider` namespaces against `TuiState`. Keep provider-native slash commands raw under `/provider /...`, while `/agent ...` stays product-semantic and local-first.

**Tech Stack:** Rust, `shlex`, existing `agent-core` command/provider/session types, existing `agent-tui` TUI runtime and multi-agent helpers

---

## File Structure

**Create:**
- `core/src/command_bus/mod.rs` - exports parser/model/registry modules for the new slash command framework
- `core/src/command_bus/model.rs` - typed command AST, namespace enum, target spec, and parse error types
- `core/src/command_bus/parse.rs` - parser for `/local`, `/agent`, `/provider` and quoted args
- `core/src/command_bus/registry.rs` - initial command metadata, help rendering, and namespace/path lookup
- `tui/src/command_runtime.rs` - target resolution and execution against `TuiState`

**Modify:**
- `core/Cargo.toml` - add `shlex.workspace = true`
- `core/src/lib.rs` - export `command_bus`
- `core/src/commands.rs` - convert the current flat parser into a legacy alias shim
- `core/src/provider.rs` - add provider capability metadata for passthrough slash support
- `tui/src/app_loop.rs` - replace direct `parse_local_command()` routing with the new command runtime
- `tui/src/ui_state.rs` - add agent/provider target lookup helpers and raw provider prompt helpers
- `tui/src/lib.rs` - export the new command runtime module if tests need it

**Test files:**
- `core/src/command_bus/parse.rs` - parser unit tests
- `core/src/command_bus/registry.rs` - command registry/help tests
- `core/src/commands.rs` - legacy alias tests
- `tui/src/command_runtime.rs` - resolution/execution tests
- `tui/src/app_loop.rs` - end-to-end slash command routing tests

## Scope Lock

This plan implements the framework and a small but real v1 command set:

- `/local help`
- `/local status`
- `/local kanban list`
- `/local config get <key>`
- `/local config set <key> <value>`
- `/agent status`
- `/agent <target> status`
- `/agent summary`
- `/agent <target> summary`
- `/provider /status`
- `/provider <target> /status`
- compatibility aliases for existing flat commands:
  - `/help`
  - `/provider`
  - `/skills`
  - `/doctor`
  - `/backlog`
  - `/todo-add`
  - `/run-once`
  - `/run-loop`
  - `/quit`

The initial mutable config keys are intentionally small:

- `tui.overview.agent_list_rows`
- `runtime.selected_provider`

Both are session-scoped for v1. No new global config file is introduced in this plan.

### Task 1: Build the Core Command AST and Parser

**Files:**
- Create: `core/src/command_bus/mod.rs`
- Create: `core/src/command_bus/model.rs`
- Create: `core/src/command_bus/parse.rs`
- Modify: `core/Cargo.toml`
- Modify: `core/src/lib.rs`
- Test: `core/src/command_bus/parse.rs`

- [ ] **Step 1: Write the failing parser tests**

```rust
// core/src/command_bus/parse.rs

#[cfg(test)]
mod tests {
    use super::parse_slash_command;
    use crate::command_bus::model::{
        CommandInvocation, CommandNamespace, CommandTargetSpec, ParsedSlashCommand,
    };

    #[test]
    fn parses_local_command_path_and_args() {
        let parsed = parse_slash_command("/local kanban list").expect("parse");
        assert_eq!(
            parsed,
            ParsedSlashCommand::Invocation(CommandInvocation {
                namespace: CommandNamespace::Local,
                target: None,
                path: vec!["kanban".to_string(), "list".to_string()],
                args: vec![],
                raw_tail: None,
            })
        );
    }

    #[test]
    fn parses_agent_command_with_explicit_target() {
        let parsed = parse_slash_command("/agent alpha status").expect("parse");
        assert_eq!(
            parsed,
            ParsedSlashCommand::Invocation(CommandInvocation {
                namespace: CommandNamespace::Agent,
                target: Some(CommandTargetSpec::AgentName("alpha".to_string())),
                path: vec!["status".to_string()],
                args: vec![],
                raw_tail: None,
            })
        );
    }

    #[test]
    fn parses_provider_passthrough_without_target() {
        let parsed = parse_slash_command("/provider /status").expect("parse");
        assert_eq!(
            parsed,
            ParsedSlashCommand::Invocation(CommandInvocation {
                namespace: CommandNamespace::Provider,
                target: None,
                path: vec![],
                args: vec![],
                raw_tail: Some("/status".to_string()),
            })
        );
    }

    #[test]
    fn parses_provider_passthrough_with_target() {
        let parsed = parse_slash_command("/provider alpha /status").expect("parse");
        assert_eq!(
            parsed,
            ParsedSlashCommand::Invocation(CommandInvocation {
                namespace: CommandNamespace::Provider,
                target: Some(CommandTargetSpec::AgentName("alpha".to_string())),
                path: vec![],
                args: vec![],
                raw_tail: Some("/status".to_string()),
            })
        );
    }

    #[test]
    fn parses_quoted_arguments_for_local_config() {
        let parsed = parse_slash_command(
            "/local config set ui.title \"My Agile Agent\"",
        )
        .expect("parse");
        let ParsedSlashCommand::Invocation(invocation) = parsed else {
            panic!("expected invocation");
        };
        assert_eq!(invocation.path, vec!["config", "set"]);
        assert_eq!(invocation.args, vec!["ui.title", "My Agile Agent"]);
    }

    #[test]
    fn rejects_provider_without_raw_slash_tail() {
        let error = parse_slash_command("/provider status").expect_err("must fail");
        assert_eq!(
            error.to_string(),
            "provider commands must use raw slash passthrough syntax like `/provider /status`"
        );
    }
}
```

- [ ] **Step 2: Run the parser tests to verify they fail**

Run: `cargo test -p agent-core parses_provider_passthrough_with_target -- --nocapture`
Expected: FAIL with missing `command_bus` module or missing `parse_slash_command`

- [ ] **Step 3: Add the command bus model types**

```rust
// core/src/command_bus/model.rs

use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommandNamespace {
    Local,
    Agent,
    Provider,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommandTargetSpec {
    AgentName(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandInvocation {
    pub namespace: CommandNamespace,
    pub target: Option<CommandTargetSpec>,
    pub path: Vec<String>,
    pub args: Vec<String>,
    pub raw_tail: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParsedSlashCommand {
    Invocation(CommandInvocation),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandParseError {
    message: String,
}

impl CommandParseError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl fmt::Display for CommandParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for CommandParseError {}
```

- [ ] **Step 4: Implement the parser**

```rust
// core/src/command_bus/parse.rs

use crate::command_bus::model::{
    CommandInvocation, CommandNamespace, CommandParseError, CommandTargetSpec,
    ParsedSlashCommand,
};

pub fn parse_slash_command(input: &str) -> Result<ParsedSlashCommand, CommandParseError> {
    let trimmed = input.trim();
    if !trimmed.starts_with('/') {
        return Err(CommandParseError::new("slash commands must start with `/`"));
    }

    let tokens = shlex::split(trimmed)
        .ok_or_else(|| CommandParseError::new("invalid slash command quoting"))?;
    let first = tokens
        .first()
        .ok_or_else(|| CommandParseError::new("empty slash command"))?;

    match first.as_str() {
        "/local" => parse_local(tokens),
        "/agent" => parse_agent(tokens),
        "/provider" => parse_provider(tokens),
        other => Err(CommandParseError::new(format!(
            "unsupported slash namespace: {}",
            other
        ))),
    }
}

fn parse_local(tokens: Vec<String>) -> Result<ParsedSlashCommand, CommandParseError> {
    if tokens.len() < 2 {
        return Err(CommandParseError::new("usage: /local <path...>"));
    }

    let path = collect_path(&tokens[1..]);
    if path.is_empty() {
        return Err(CommandParseError::new("usage: /local <path...>"));
    }

    Ok(ParsedSlashCommand::Invocation(CommandInvocation {
        namespace: CommandNamespace::Local,
        target: None,
        path,
        args: collect_args(&tokens[1..]),
        raw_tail: None,
    }))
}

fn parse_agent(tokens: Vec<String>) -> Result<ParsedSlashCommand, CommandParseError> {
    if tokens.len() < 2 {
        return Err(CommandParseError::new("usage: /agent [target] <path...>"));
    }

    let (target, start_index) = if tokens.get(1).is_some_and(|token| !token.starts_with('/'))
        && tokens.len() >= 3
        && is_agent_target_token(&tokens[1])
    {
        (Some(CommandTargetSpec::AgentName(tokens[1].clone())), 2usize)
    } else {
        (None, 1usize)
    };

    let path = collect_path(&tokens[start_index..]);
    if path.is_empty() {
        return Err(CommandParseError::new("usage: /agent [target] <path...>"));
    }

    Ok(ParsedSlashCommand::Invocation(CommandInvocation {
        namespace: CommandNamespace::Agent,
        target,
        path,
        args: collect_args(&tokens[start_index..]),
        raw_tail: None,
    }))
}

fn parse_provider(tokens: Vec<String>) -> Result<ParsedSlashCommand, CommandParseError> {
    if tokens.len() < 2 {
        return Err(CommandParseError::new(
            "usage: /provider [target] /provider-native-command",
        ));
    }

    let (target, tail_index) = if tokens.len() >= 3
        && is_agent_target_token(&tokens[1])
        && tokens[2].starts_with('/')
    {
        (Some(CommandTargetSpec::AgentName(tokens[1].clone())), 2usize)
    } else {
        (None, 1usize)
    };

    let Some(raw_tail) = tokens.get(tail_index) else {
        return Err(CommandParseError::new(
            "usage: /provider [target] /provider-native-command",
        ));
    };
    if !raw_tail.starts_with('/') {
        return Err(CommandParseError::new(
            "provider commands must use raw slash passthrough syntax like `/provider /status`",
        ));
    }

    Ok(ParsedSlashCommand::Invocation(CommandInvocation {
        namespace: CommandNamespace::Provider,
        target,
        path: vec![],
        args: vec![],
        raw_tail: Some(raw_tail.clone()),
    }))
}

fn collect_path(tokens: &[String]) -> Vec<String> {
    tokens
        .iter()
        .take_while(|token| !token.starts_with("--") && !token.contains('='))
        .cloned()
        .collect()
}

fn collect_args(tokens: &[String]) -> Vec<String> {
    tokens.iter().skip_while(|token| !token.contains('.') && !token.contains('=')
        && !token.starts_with("--"))
        .cloned()
        .collect()
}

fn is_agent_target_token(token: &str) -> bool {
    !token.contains('/')
}
```

- [ ] **Step 5: Export the new module**

```rust
// core/src/command_bus/mod.rs

pub mod model;
pub mod parse;
pub mod registry;
```

```rust
// core/src/lib.rs

pub mod command_bus;
```

```toml
# core/Cargo.toml

[dependencies]
shlex.workspace = true
```

- [ ] **Step 6: Run the parser test group**

Run: `cargo test -p agent-core parse_slash_command -- --nocapture`
Expected: PASS for the new parser tests

- [ ] **Step 7: Commit**

```bash
git add core/Cargo.toml core/src/lib.rs core/src/command_bus/mod.rs core/src/command_bus/model.rs core/src/command_bus/parse.rs
git commit -m "feat(core): add slash command parser"
```

### Task 2: Add the Registry and Legacy Alias Bridge

**Files:**
- Create: `core/src/command_bus/registry.rs`
- Modify: `core/src/commands.rs`
- Test: `core/src/command_bus/registry.rs`
- Test: `core/src/commands.rs`

- [ ] **Step 1: Write the failing registry and alias tests**

```rust
// core/src/command_bus/registry.rs

#[cfg(test)]
mod tests {
    use super::{command_spec, render_local_help_lines};
    use crate::command_bus::model::CommandNamespace;

    #[test]
    fn registry_contains_local_status() {
        let spec = command_spec(CommandNamespace::Local, &["status"]).expect("spec");
        assert_eq!(spec.summary, "Show agile-agent runtime status");
    }

    #[test]
    fn help_lines_include_namespaced_commands() {
        let help = render_local_help_lines();
        assert!(help.iter().any(|line| line.contains("/local status")));
        assert!(help.iter().any(|line| line.contains("/agent status")));
        assert!(help.iter().any(|line| line.contains("/provider /status")));
    }
}
```

```rust
// core/src/commands.rs

#[test]
fn legacy_help_alias_maps_to_local_help() {
    let parsed = parse_legacy_alias("/help").expect("alias");
    assert_eq!(parsed.namespace, crate::command_bus::model::CommandNamespace::Local);
    assert_eq!(parsed.path, vec!["help".to_string()]);
}

#[test]
fn unsupported_flat_slash_command_is_not_a_legacy_alias() {
    assert!(parse_legacy_alias("/status").is_none());
}
```

- [ ] **Step 2: Run the failing tests**

Run: `cargo test -p agent-core legacy_help_alias_maps_to_local_help -- --nocapture`
Expected: FAIL with missing `command_spec`, `render_local_help_lines`, or `parse_legacy_alias`

- [ ] **Step 3: Define the registry metadata**

```rust
// core/src/command_bus/registry.rs

use crate::command_bus::model::CommandNamespace;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandSpec {
    pub namespace: CommandNamespace,
    pub path: &'static [&'static str],
    pub summary: &'static str,
    pub requires_target: bool,
    pub provider_passthrough: bool,
}

const COMMAND_SPECS: &[CommandSpec] = &[
    CommandSpec {
        namespace: CommandNamespace::Local,
        path: &["help"],
        summary: "Show slash command help",
        requires_target: false,
        provider_passthrough: false,
    },
    CommandSpec {
        namespace: CommandNamespace::Local,
        path: &["status"],
        summary: "Show agile-agent runtime status",
        requires_target: false,
        provider_passthrough: false,
    },
    CommandSpec {
        namespace: CommandNamespace::Local,
        path: &["kanban", "list"],
        summary: "List current kanban tasks",
        requires_target: false,
        provider_passthrough: false,
    },
    CommandSpec {
        namespace: CommandNamespace::Local,
        path: &["config", "get"],
        summary: "Read a session-scoped config value",
        requires_target: false,
        provider_passthrough: false,
    },
    CommandSpec {
        namespace: CommandNamespace::Local,
        path: &["config", "set"],
        summary: "Update a session-scoped config value",
        requires_target: false,
        provider_passthrough: false,
    },
    CommandSpec {
        namespace: CommandNamespace::Agent,
        path: &["status"],
        summary: "Show the resolved agent state",
        requires_target: false,
        provider_passthrough: false,
    },
    CommandSpec {
        namespace: CommandNamespace::Agent,
        path: &["summary"],
        summary: "Show a concise agent work summary",
        requires_target: false,
        provider_passthrough: false,
    },
    CommandSpec {
        namespace: CommandNamespace::Provider,
        path: &[],
        summary: "Pass a raw slash command to the resolved provider session",
        requires_target: false,
        provider_passthrough: true,
    },
];

pub fn command_spec(
    namespace: CommandNamespace,
    path: &[&str],
) -> Option<&'static CommandSpec> {
    COMMAND_SPECS
        .iter()
        .find(|spec| spec.namespace == namespace && spec.path == path)
}

pub fn render_local_help_lines() -> Vec<String> {
    vec![
        "available slash commands:".to_string(),
        "/local help".to_string(),
        "/local status".to_string(),
        "/local kanban list".to_string(),
        "/local config get <key>".to_string(),
        "/local config set <key> <value>".to_string(),
        "/agent status".to_string(),
        "/agent <target> status".to_string(),
        "/agent summary".to_string(),
        "/provider /status".to_string(),
        "/provider <target> /status".to_string(),
    ]
}
```

- [ ] **Step 4: Convert the flat parser into a legacy alias shim**

```rust
// core/src/commands.rs

use crate::command_bus::model::{CommandInvocation, CommandNamespace};

pub fn parse_legacy_alias(input: &str) -> Option<CommandInvocation> {
    let trimmed = input.trim();
    match trimmed {
        "/help" => Some(local_invocation(&["help"], &[])),
        "/provider" => Some(local_invocation(&["status"], &["runtime.selected_provider"])),
        "/skills" => Some(local_invocation(&["help"], &[])),
        "/doctor" => Some(local_invocation(&["status"], &["runtime.providers"])),
        "/backlog" => Some(local_invocation(&["kanban", "list"], &[])),
        "/run-once" => Some(local_invocation(&["legacy", "run-once"], &[])),
        "/run-loop" => Some(local_invocation(&["legacy", "run-loop"], &[])),
        "/quit" => Some(local_invocation(&["legacy", "quit"], &[])),
        _ if trimmed.starts_with("/todo-add ") => Some(local_invocation(
            &["legacy", "todo-add"],
            &[trimmed.trim_start_matches("/todo-add ").trim()],
        )),
        _ => None,
    }
}

fn local_invocation(path: &[&str], args: &[&str]) -> CommandInvocation {
    CommandInvocation {
        namespace: CommandNamespace::Local,
        target: None,
        path: path.iter().map(|value| value.to_string()).collect(),
        args: args.iter().map(|value| value.to_string()).collect(),
        raw_tail: None,
    }
}
```

- [ ] **Step 5: Run the registry and alias tests**

Run: `cargo test -p agent-core registry_contains_local_status -- --nocapture`
Expected: PASS

Run: `cargo test -p agent-core legacy_help_alias_maps_to_local_help -- --nocapture`
Expected: PASS

- [ ] **Step 6: Commit**

```bash
git add core/src/command_bus/registry.rs core/src/commands.rs
git commit -m "feat(core): add slash command registry"
```

### Task 3: Add TUI Command Resolution and Local Command Execution

**Files:**
- Create: `tui/src/command_runtime.rs`
- Modify: `tui/src/lib.rs`
- Modify: `tui/src/ui_state.rs`
- Test: `tui/src/command_runtime.rs`

- [ ] **Step 1: Write the failing resolution and local execution tests**

```rust
// tui/src/command_runtime.rs

#[cfg(test)]
mod tests {
    use super::{execute_local_command, resolve_agent_target};
    use crate::test_support::ShellHarness;
    use agent_core::provider::ProviderKind;

    #[test]
    fn resolves_agent_target_from_focused_worker() {
        let mut shell = ShellHarness::new_with_overview(ProviderKind::Mock);
        let alpha_id = shell.state.spawn_agent(ProviderKind::Mock).expect("spawn");
        shell.state.focus_agent(&alpha_id);

        let resolved = resolve_agent_target(&shell.state, None).expect("target");
        assert_eq!(resolved.agent_id, alpha_id);
    }

    #[test]
    fn resolves_agent_target_to_overview_in_overview_context() {
        let mut shell = ShellHarness::new_with_overview(ProviderKind::Mock);
        let resolved = resolve_agent_target(&shell.state, None).expect("target");
        assert_eq!(resolved.codename, "OVERVIEW");
    }

    #[test]
    fn local_config_set_updates_agent_list_rows() {
        let mut shell = ShellHarness::new_with_overview(ProviderKind::Mock);
        let lines = execute_local_command(
            &mut shell.state,
            &["config", "set"],
            &["tui.overview.agent_list_rows", "10"],
        )
        .expect("command");

        assert_eq!(shell.state.view_state.overview.agent_list_rows, 10);
        assert!(lines.iter().any(|line| line.contains("tui.overview.agent_list_rows = 10")));
    }
}
```

- [ ] **Step 2: Run the failing TUI command runtime tests**

Run: `cargo test -p agent-tui resolves_agent_target_from_focused_worker -- --nocapture`
Expected: FAIL with missing `command_runtime` module or missing helpers

- [ ] **Step 3: Add the resolution and local execution types**

```rust
// tui/src/command_runtime.rs

use anyhow::{anyhow, Result};
use agent_core::command_bus::registry::render_local_help_lines;
use agent_core::provider::ProviderKind;

use crate::ui_state::TuiState;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedAgentTarget {
    pub agent_id: agent_core::agent_runtime::AgentId,
    pub codename: String,
    pub provider: ProviderKind,
}

pub fn resolve_agent_target(
    state: &TuiState,
    explicit: Option<&str>,
) -> Result<ResolvedAgentTarget> {
    let statuses = state.agent_statuses();
    let status = if let Some(explicit) = explicit {
        statuses
            .iter()
            .find(|status| {
                status.codename.as_str() == explicit || status.agent_id.as_str() == explicit
            })
            .ok_or_else(|| anyhow!("agent target `{explicit}` not found"))?
    } else {
        state
            .focused_agent_status()
            .as_ref()
            .ok_or_else(|| anyhow!("no focused agent available"))?
    };

    Ok(ResolvedAgentTarget {
        agent_id: status.agent_id.clone(),
        codename: status.codename.as_str().to_string(),
        provider: status
            .provider_type
            .to_provider_kind()
            .unwrap_or(state.app().selected_provider),
    })
}

pub fn execute_local_command(
    state: &mut TuiState,
    path: &[&str],
    args: &[&str],
) -> Result<Vec<String>> {
    match path {
        ["help"] => Ok(render_local_help_lines()),
        ["status"] => Ok(vec![
            format!("focused agent: {}", state.focused_agent_codename()),
            format!("selected provider: {}", state.app().selected_provider.label()),
            format!("loop phase: {:?}", state.app().loop_phase),
        ]),
        ["kanban", "list"] => Ok(state.workplace().backlog.render_lines()),
        ["config", "get"] => execute_config_get(state, args),
        ["config", "set"] => execute_config_set(state, args),
        ["legacy", "run-once"] => Ok(vec!["legacy alias: /run-once".to_string()]),
        ["legacy", "run-loop"] => Ok(vec!["legacy alias: /run-loop".to_string()]),
        ["legacy", "quit"] => Ok(vec!["legacy alias: /quit".to_string()]),
        ["legacy", "todo-add"] => Ok(vec![format!("legacy alias: /todo-add {}", args.join(" "))]),
        _ => Err(anyhow!("unsupported local command: /local {}", path.join(" "))),
    }
}

fn execute_config_get(state: &TuiState, args: &[&str]) -> Result<Vec<String>> {
    match args {
        ["tui.overview.agent_list_rows"] => Ok(vec![format!(
            "tui.overview.agent_list_rows = {}",
            state.view_state.overview.agent_list_rows
        )]),
        ["runtime.selected_provider"] => Ok(vec![format!(
            "runtime.selected_provider = {}",
            state.app().selected_provider.label()
        )]),
        [other] => Err(anyhow!("unsupported config key: {other}")),
        _ => Err(anyhow!("usage: /local config get <key>")),
    }
}

fn execute_config_set(state: &mut TuiState, args: &[&str]) -> Result<Vec<String>> {
    match args {
        ["tui.overview.agent_list_rows", value] => {
            let rows = value.parse::<usize>().map_err(|_| anyhow!("invalid usize value: {value}"))?;
            state.view_state.overview.set_agent_list_rows(rows);
            Ok(vec![format!(
                "tui.overview.agent_list_rows = {}",
                state.view_state.overview.agent_list_rows
            )])
        }
        ["runtime.selected_provider", "mock"] => {
            state.app_mut().selected_provider = ProviderKind::Mock;
            Ok(vec!["runtime.selected_provider = mock".to_string()])
        }
        ["runtime.selected_provider", "claude"] => {
            state.app_mut().selected_provider = ProviderKind::Claude;
            Ok(vec!["runtime.selected_provider = claude".to_string()])
        }
        ["runtime.selected_provider", "codex"] => {
            state.app_mut().selected_provider = ProviderKind::Codex;
            Ok(vec!["runtime.selected_provider = codex".to_string()])
        }
        [other, _] => Err(anyhow!("unsupported config key: {other}")),
        _ => Err(anyhow!("usage: /local config set <key> <value>")),
    }
}
```

- [ ] **Step 4: Export the runtime module and add target helpers**

```rust
// tui/src/lib.rs
mod command_runtime;
```

```rust
// tui/src/ui_state.rs

pub fn focused_agent_target_name(&self) -> Option<String> {
    self.focused_agent_status()
        .map(|status| status.codename.as_str().to_string())
}
```

- [ ] **Step 5: Run the TUI command runtime tests**

Run: `cargo test -p agent-tui local_config_set_updates_agent_list_rows -- --nocapture`
Expected: PASS

- [ ] **Step 6: Commit**

```bash
git add tui/src/command_runtime.rs tui/src/lib.rs tui/src/ui_state.rs
git commit -m "feat(tui): add slash command resolution"
```

### Task 4: Implement `/agent` Semantic Commands

**Files:**
- Modify: `tui/src/command_runtime.rs`
- Test: `tui/src/command_runtime.rs`

- [ ] **Step 1: Write the failing `/agent` command tests**

```rust
#[test]
fn agent_status_reports_role_provider_and_status() {
    let mut shell = ShellHarness::new_with_overview(ProviderKind::Mock);
    let alpha_id = shell.state.spawn_agent(ProviderKind::Codex).expect("spawn");
    shell.state.focus_agent(&alpha_id);

    let lines = execute_agent_command(&shell.state, None, &["status"], &[]).expect("command");
    assert!(lines.iter().any(|line| line.contains("target: alpha")));
    assert!(lines.iter().any(|line| line.contains("provider: codex")));
    assert!(lines.iter().any(|line| line.contains("status: idle")));
}

#[test]
fn agent_summary_uses_recent_transcript_and_task() {
    let mut shell = ShellHarness::new_with_overview(ProviderKind::Mock);
    let alpha_id = shell.state.spawn_agent(ProviderKind::Mock).expect("spawn");
    if let Some(pool) = shell.state.agent_pool.as_mut()
        && let Some(slot) = pool.get_slot_mut_by_id(&alpha_id)
    {
        slot.append_transcript(agent_core::app::TranscriptEntry::Assistant(
            "finished reviewing parser layout".to_string(),
        ));
    }

    let lines = execute_agent_command(&shell.state, Some("alpha"), &["summary"], &[])
        .expect("summary");
    assert!(lines.iter().any(|line| line.contains("finished reviewing parser layout")));
}
```

- [ ] **Step 2: Run the failing `/agent` tests**

Run: `cargo test -p agent-tui agent_status_reports_role_provider_and_status -- --nocapture`
Expected: FAIL with missing `execute_agent_command`

- [ ] **Step 3: Implement the `/agent` executor**

```rust
// tui/src/command_runtime.rs

pub fn execute_agent_command(
    state: &TuiState,
    explicit_target: Option<&str>,
    path: &[&str],
    _args: &[&str],
) -> Result<Vec<String>> {
    let target = resolve_agent_target(state, explicit_target)?;
    let status = state
        .agent_statuses()
        .into_iter()
        .find(|status| status.agent_id == target.agent_id)
        .ok_or_else(|| anyhow!("resolved agent target disappeared"))?;

    match path {
        ["status"] => Ok(vec![
            format!("target: {}", target.codename),
            format!("provider: {}", target.provider.label()),
            format!("role: {}", status.role.name()),
            format!("status: {}", status.status.label()),
            format!(
                "task: {}",
                status
                    .assigned_task_id
                    .as_ref()
                    .map(|task| task.as_str())
                    .unwrap_or("<none>")
            ),
        ]),
        ["summary"] => {
            let transcript = state
                .agent_pool
                .as_ref()
                .and_then(|pool| pool.get_slot_by_id(&target.agent_id))
                .map(|slot| slot.transcript())
                .unwrap_or(&[]);
            let latest = transcript.iter().rev().find_map(|entry| match entry {
                agent_core::app::TranscriptEntry::Assistant(text) if !text.is_empty() => {
                    Some(text.as_str())
                }
                agent_core::app::TranscriptEntry::Status(text) if !text.is_empty() => {
                    Some(text.as_str())
                }
                _ => None,
            });
            Ok(vec![
                format!("target: {}", target.codename),
                format!("provider: {}", target.provider.label()),
                format!(
                    "latest: {}",
                    latest.unwrap_or("no summary available")
                ),
            ])
        }
        _ => Err(anyhow!("unsupported agent command: /agent {}", path.join(" "))),
    }
}
```

- [ ] **Step 4: Run the `/agent` test group**

Run: `cargo test -p agent-tui execute_agent_command -- --nocapture`
Expected: PASS for the `/agent status` and `/agent summary` tests

- [ ] **Step 5: Commit**

```bash
git add tui/src/command_runtime.rs
git commit -m "feat(tui): add semantic agent commands"
```

### Task 5: Add Provider Capabilities and `/provider` Passthrough

**Files:**
- Modify: `core/src/provider.rs`
- Modify: `tui/src/command_runtime.rs`
- Modify: `tui/src/ui_state.rs`
- Test: `tui/src/command_runtime.rs`

- [ ] **Step 1: Write the failing provider passthrough tests**

```rust
#[test]
fn provider_passthrough_rejects_mock_provider() {
    let shell = ShellHarness::new_with_overview(ProviderKind::Mock);
    let error = execute_provider_command(&shell.state, None, "/status").expect_err("must fail");
    assert_eq!(
        error.to_string(),
        "provider `mock` does not support raw slash passthrough"
    );
}

#[test]
fn provider_passthrough_requires_existing_session_handle() {
    let mut shell = ShellHarness::new_with_overview(ProviderKind::Claude);
    let alpha_id = shell.state.spawn_agent(ProviderKind::Claude).expect("spawn");
    shell.state.focus_agent(&alpha_id);

    let error = execute_provider_command(&shell.state, None, "/status").expect_err("must fail");
    assert_eq!(
        error.to_string(),
        "agent `alpha` has no active provider session for passthrough commands"
    );
}
```

- [ ] **Step 2: Run the failing provider tests**

Run: `cargo test -p agent-tui provider_passthrough_rejects_mock_provider -- --nocapture`
Expected: FAIL with missing provider capability helpers or missing `execute_provider_command`

- [ ] **Step 3: Add provider capability metadata**

```rust
// core/src/provider.rs

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProviderCapabilities {
    pub supports_slash_passthrough: bool,
}

impl ProviderKind {
    pub fn capabilities(self) -> ProviderCapabilities {
        match self {
            Self::Mock => ProviderCapabilities {
                supports_slash_passthrough: false,
            },
            Self::Claude | Self::Codex => ProviderCapabilities {
                supports_slash_passthrough: true,
            },
        }
    }
}
```

- [ ] **Step 4: Add raw provider prompt execution helpers**

```rust
// tui/src/ui_state.rs

pub fn agent_has_provider_session(&self, agent_id: &AgentId) -> bool {
    self.agent_pool
        .as_ref()
        .and_then(|pool| pool.get_slot_by_id(agent_id))
        .and_then(|slot| slot.session_handle())
        .is_some()
}

pub fn start_raw_provider_prompt_for_agent(
    &mut self,
    agent_id: &AgentId,
    prompt: String,
) -> Option<std::sync::mpsc::Receiver<agent_core::provider::ProviderEvent>> {
    self.start_provider_for_agent(agent_id, prompt)
}
```

- [ ] **Step 5: Implement the `/provider` executor**

```rust
// tui/src/command_runtime.rs

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderCommandRequest {
    pub agent_id: agent_core::agent_runtime::AgentId,
    pub codename: String,
    pub raw_tail: String,
}

pub fn execute_provider_command(
    state: &TuiState,
    explicit_target: Option<&str>,
    raw_tail: &str,
) -> Result<ProviderCommandRequest> {
    let target = resolve_agent_target(state, explicit_target)?;
    if !target.provider.capabilities().supports_slash_passthrough {
        return Err(anyhow!(
            "provider `{}` does not support raw slash passthrough",
            target.provider.label()
        ));
    }
    if !state.agent_has_provider_session(&target.agent_id) {
        return Err(anyhow!(
            "agent `{}` has no active provider session for passthrough commands",
            target.codename
        ));
    }
    Ok(ProviderCommandRequest {
        agent_id: target.agent_id,
        codename: target.codename,
        raw_tail: raw_tail.to_string(),
    })
}
```

- [ ] **Step 6: Run the provider tests**

Run: `cargo test -p agent-tui provider_passthrough_requires_existing_session_handle -- --nocapture`
Expected: PASS

- [ ] **Step 7: Commit**

```bash
git add core/src/provider.rs tui/src/command_runtime.rs tui/src/ui_state.rs
git commit -m "feat(tui): add provider slash passthrough"
```

### Task 6: Integrate the Command Runtime into the TUI Loop

**Files:**
- Modify: `tui/src/app_loop.rs`
- Modify: `core/src/commands.rs`
- Test: `tui/src/app_loop.rs`

- [ ] **Step 1: Write the failing app-loop routing tests**

```rust
#[test]
fn namespaced_local_command_does_not_fall_through_to_chat() {
    let temp = TempDir::new().expect("tempdir");
    let session = RuntimeSession::bootstrap(temp.path().into(), ProviderKind::Mock, false)
        .expect("bootstrap");
    let mut state = TuiState::from_session(session);

    let handled = handle_command_submission(&mut state, "/local status".to_string()).expect("ok");
    assert!(handled);
    assert!(state.app().transcript.iter().all(|entry| {
        !matches!(entry, TranscriptEntry::User(text) if text == "/local status")
    }));
}

#[test]
fn provider_passthrough_uses_raw_tail_without_skill_injection() {
    let temp = TempDir::new().expect("tempdir");
    let session = RuntimeSession::bootstrap(temp.path().into(), ProviderKind::Claude, false)
        .expect("bootstrap");
    let mut state = TuiState::from_session(session);
    state.ensure_overview_agent();

    let handled = handle_command_submission(&mut state, "/provider /status".to_string()).expect("ok");
    assert!(handled);
    assert!(state.app().transcript.iter().all(|entry| {
        !matches!(entry, TranscriptEntry::User(text) if text.contains("/provider /status"))
    }));
}
```

- [ ] **Step 2: Run the failing app-loop tests**

Run: `cargo test -p agent-tui namespaced_local_command_does_not_fall_through_to_chat -- --nocapture`
Expected: FAIL with missing `handle_command_submission`

- [ ] **Step 3: Add a dedicated command submission path**

```rust
// tui/src/app_loop.rs

fn handle_command_submission(state: &mut TuiState, user_input: String) -> Result<bool> {
    if !user_input.trim_start().starts_with('/') {
        return Ok(false);
    }

    if let Some(alias) = agent_core::commands::parse_legacy_alias(&user_input) {
        return execute_invocation(state, alias).map(|_| true);
    }

    let parsed = agent_core::command_bus::parse::parse_slash_command(&user_input)
        .map_err(|error| anyhow::anyhow!(error.to_string()))?;
    let agent_core::command_bus::model::ParsedSlashCommand::Invocation(invocation) = parsed;
    execute_invocation(state, invocation)?;
    Ok(true)
}

fn execute_invocation(
    state: &mut TuiState,
    invocation: agent_core::command_bus::model::CommandInvocation,
) -> Result<()> {
    use agent_core::command_bus::model::CommandNamespace;

    match invocation.namespace {
        CommandNamespace::Local => {
            let path = invocation.path.iter().map(|value| value.as_str()).collect::<Vec<_>>();
            let args = invocation.args.iter().map(|value| value.as_str()).collect::<Vec<_>>();
            let lines = crate::command_runtime::execute_local_command(state, &path, &args)?;
            for line in lines {
                state.app_mut().push_status_message(line);
            }
        }
        CommandNamespace::Agent => {
            let path = invocation.path.iter().map(|value| value.as_str()).collect::<Vec<_>>();
            let lines = crate::command_runtime::execute_agent_command(
                state,
                invocation.target.as_ref().map(|target| match target {
                    agent_core::command_bus::model::CommandTargetSpec::AgentName(value) => value.as_str(),
                }),
                &path,
                &[],
            )?;
            for line in lines {
                state.app_mut().push_status_message(line);
            }
        }
        CommandNamespace::Provider => {
            let request = crate::command_runtime::execute_provider_command(
                state,
                invocation.target.as_ref().map(|target| match target {
                    agent_core::command_bus::model::CommandTargetSpec::AgentName(value) => value.as_str(),
                }),
                invocation.raw_tail.as_deref().unwrap_or(""),
            )?;
            let _ = start_multi_agent_provider_request_for_agent(
                state,
                request.agent_id.clone(),
                request.raw_tail.clone(),
            );
            state.app_mut().push_status_message(format!(
                "sent provider command `{}` to {}",
                request.raw_tail, request.codename
            ));
        }
    }

    Ok(())
}
```

- [ ] **Step 4: Call the new command path before normal chat submission**

```rust
// tui/src/app_loop.rs inside InputOutcome::Submit(user_input)

if handle_command_submission(&mut state, user_input.clone())? {
    continue;
}
```

- [ ] **Step 5: Run the app-loop routing tests**

Run: `cargo test -p agent-tui handle_command_submission -- --nocapture`
Expected: PASS

- [ ] **Step 6: Commit**

```bash
git add tui/src/app_loop.rs core/src/commands.rs
git commit -m "feat(tui): route namespaced slash commands"
```

### Task 7: Final Regression Coverage and Cleanup

**Files:**
- Modify: `tui/src/app_loop.rs`
- Modify: `tui/src/command_runtime.rs`
- Modify: `core/src/command_bus/parse.rs`
- Test: `core/src/command_bus/parse.rs`
- Test: `tui/src/command_runtime.rs`
- Test: `tui/src/app_loop.rs`

- [ ] **Step 1: Add final regression tests**

```rust
#[test]
fn provider_syntax_without_raw_slash_returns_visible_error() {
    let error = agent_core::command_bus::parse::parse_slash_command("/provider status")
        .expect_err("must fail");
    assert_eq!(
        error.to_string(),
        "provider commands must use raw slash passthrough syntax like `/provider /status`"
    );
}

#[test]
fn agent_status_defaults_to_overview_in_overview_context() {
    let shell = ShellHarness::new_with_overview(ProviderKind::Mock);
    let lines = crate::command_runtime::execute_agent_command(&shell.state, None, &["status"], &[])
        .expect("status");
    assert!(lines.iter().any(|line| line.contains("target: OVERVIEW")));
}

#[test]
fn legacy_backlog_alias_maps_to_local_kanban_list() {
    let alias = agent_core::commands::parse_legacy_alias("/backlog").expect("alias");
    assert_eq!(alias.path, vec!["kanban".to_string(), "list".to_string()]);
}
```

- [ ] **Step 2: Run focused regression suites**

Run: `cargo test -p agent-core command_bus -- --nocapture`
Expected: PASS

Run: `cargo test -p agent-tui command_runtime -- --nocapture`
Expected: PASS

Run: `cargo test -p agent-tui handle_command_submission -- --nocapture`
Expected: PASS

- [ ] **Step 3: Run broader verification**

Run: `cargo test -p agent-core --lib`
Expected: PASS

Run: `cargo test -p agent-tui --lib`
Expected: PASS

- [ ] **Step 4: Commit**

```bash
git add core/src/command_bus/parse.rs tui/src/command_runtime.rs tui/src/app_loop.rs core/src/commands.rs
git commit -m "test: cover slash command routing"
```

## Self-Review

### Spec coverage

Covered requirements:

- explicit namespaces: Tasks 1, 2, and 6
- `/agent` default target semantics: Tasks 3 and 4
- `/provider /...` raw passthrough: Tasks 1, 5, and 6
- local status/kanban/config commands: Task 3
- agent semantic commands: Task 4
- provider capability checks: Task 5
- clear non-chat fallback behavior: Task 6
- future help/autocomplete foundation via registry: Task 2

No spec gaps remain for the scoped v1 feature set.

### Placeholder scan

This plan does not use TODO/TBD placeholders.

### Type consistency

The plan consistently uses:

- `CommandInvocation`
- `CommandNamespace`
- `CommandTargetSpec`
- `ParsedSlashCommand`
- `CommandSpec`
- `ResolvedAgentTarget`
- `ProviderCommandRequest`

These names are reused consistently across tasks.

## Execution Handoff

Plan complete and saved to `docs/superpowers/plans/2026-04-16-slash-command-system-implementation.md`. Two execution options:

1. Subagent-Driven (recommended) - I dispatch a fresh subagent per task, review between tasks, fast iteration

2. Inline Execution - Execute tasks in this session using executing-plans, batch execution with checkpoints

Which approach?
