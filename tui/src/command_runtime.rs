use anyhow::{Result, anyhow};
use agent_core::command_bus::registry::render_local_help_lines;
use agent_core::provider::ProviderKind;

use crate::ui_state::TuiState;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedAgentTarget {
    pub agent_id: agent_core::agent_runtime::AgentId,
    pub codename: String,
    pub provider: ProviderKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderCommandRequest {
    pub agent_id: agent_core::agent_runtime::AgentId,
    pub codename: String,
    pub raw_tail: String,
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
                status.codename.as_str().eq_ignore_ascii_case(explicit)
                    || status.agent_id.as_str().eq_ignore_ascii_case(explicit)
            })
            .ok_or_else(|| anyhow!("agent target `{explicit}` not found"))?
    } else {
        let focused_status = state
            .focused_agent_status()
            .ok_or_else(|| anyhow!("no focused agent available"))?;
        return Ok(ResolvedAgentTarget {
            agent_id: focused_status.agent_id,
            codename: focused_status.codename.as_str().to_string(),
            provider: focused_status
                .provider_type
                .to_provider_kind()
                .unwrap_or(state.app().selected_provider),
        });
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
        ["kanban", "list"] => Ok(state.app().render_backlog_lines()),
        ["config", "get"] => execute_config_get(state, args),
        ["config", "set"] => execute_config_set(state, args),
        ["legacy", "provider"] => Ok(vec![format!(
            "current agent: {} · provider: {} (tab creates a new agent on the next provider)",
            state.session.agent_runtime.summary(),
            state.app().selected_provider.label()
        )]),
        ["legacy", "skills"] => {
            state.app_mut().open_skill_browser();
            Ok(vec!["opened skill browser".to_string()])
        }
        ["legacy", "doctor"] => Ok(agent_core::probe::render_doctor_text(
            &agent_core::probe::probe_report(),
        )
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| line.to_string())
        .collect()),
        ["legacy", "run-once"] => Ok(vec!["legacy alias: /run-once".to_string()]),
        ["legacy", "run-loop"] => Ok(vec!["legacy alias: /run-loop".to_string()]),
        ["legacy", "quit"] => Ok(vec!["legacy alias: /quit".to_string()]),
        ["legacy", "todo-add"] => Ok(vec![format!("legacy alias: /todo-add {}", args.join(" "))]),
        _ => Err(anyhow!("unsupported local command: /local {}", path.join(" "))),
    }
}

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
            let latest = state
                .agent_pool
                .as_ref()
                .and_then(|pool| pool.get_slot_by_id(&target.agent_id))
                .and_then(|slot| {
                    slot.transcript().iter().rev().find_map(|entry| match entry {
                        agent_core::app::TranscriptEntry::Assistant(text) if !text.is_empty() => {
                            Some(text.as_str())
                        }
                        agent_core::app::TranscriptEntry::Status(text) if !text.is_empty() => {
                            Some(text.as_str())
                        }
                        _ => None,
                    })
                })
                .unwrap_or("no summary available");
            Ok(vec![
                format!("target: {}", target.codename),
                format!("provider: {}", target.provider.label()),
                format!("latest: {}", latest),
            ])
        }
        _ => Err(anyhow!("unsupported agent command: /agent {}", path.join(" "))),
    }
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
            let rows = value
                .parse::<usize>()
                .map_err(|_| anyhow!("invalid usize value: {value}"))?;
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
        ["runtime.selected_provider", other] => {
            Err(anyhow!("unsupported provider value: {other}"))
        }
        [other, _] => Err(anyhow!("unsupported config key: {other}")),
        _ => Err(anyhow!("usage: /local config set <key> <value>")),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        execute_agent_command, execute_local_command, execute_provider_command,
        resolve_agent_target,
    };
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
        let shell = ShellHarness::new_with_overview(ProviderKind::Mock);
        let resolved = resolve_agent_target(&shell.state, None).expect("target");
        assert_eq!(resolved.codename, "OVERVIEW");
    }

    #[test]
    fn resolves_explicit_overview_target_case_insensitively() {
        let shell = ShellHarness::new_with_overview(ProviderKind::Mock);
        let resolved = resolve_agent_target(&shell.state, Some("overview")).expect("target");
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
        assert!(lines
            .iter()
            .any(|line| line.contains("tui.overview.agent_list_rows = 10")));
    }

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

        let lines =
            execute_agent_command(&shell.state, Some("alpha"), &["summary"], &[]).expect("summary");
        assert!(lines
            .iter()
            .any(|line| line.contains("finished reviewing parser layout")));
    }

    #[test]
    fn agent_status_defaults_to_overview_in_overview_context() {
        let shell = ShellHarness::new_with_overview(ProviderKind::Mock);
        let lines = execute_agent_command(&shell.state, None, &["status"], &[]).expect("status");
        assert!(lines.iter().any(|line| line.contains("target: OVERVIEW")));
    }

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
}
