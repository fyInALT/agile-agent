//! agent-tui — terminal UI for the agile-agent daemon.

use anyhow::Result;

// Protocol-only modules (no agent_core dependency).
mod event_handler;
mod protocol_client;
mod protocol_state;
mod reconnecting_client;
mod websocket_client;

// Core-dependent modules (embedded mode).
#[cfg(feature = "core")]
mod app_loop;
#[cfg(feature = "core")]
mod command_runtime;
#[cfg(feature = "core")]
mod composer;
#[cfg(feature = "core")]
mod confirmation_overlay;
#[cfg(feature = "core")]
mod diff_render;
#[cfg(feature = "core")]
mod exec_command;
#[cfg(feature = "core")]
mod exec_semantics;
#[cfg(feature = "core")]
mod history_cell;
#[cfg(feature = "core")]
mod human_decision_overlay;
#[cfg(feature = "core")]
mod input;
#[cfg(feature = "core")]
mod launch_config_overlay;
#[cfg(feature = "core")]
mod markdown;
#[cfg(feature = "core")]
mod markdown_stream;
#[cfg(feature = "core")]
mod metrics_panel;
#[cfg(feature = "core")]
mod overview_row;
#[cfg(feature = "core")]
mod overview_state;
#[cfg(feature = "core")]
mod provider_overlay;
#[cfg(feature = "core")]
mod profile_selection_overlay;
#[cfg(feature = "core")]
mod render;
#[cfg(feature = "core")]
mod resume_overlay;
#[cfg(feature = "core")]
mod streaming;
#[cfg(feature = "core")]
mod task_decision_overlay;
#[cfg(feature = "core")]
mod task_detail_view;
#[cfg(feature = "core")]
mod task_panel;
#[cfg(feature = "core")]
mod terminal;
#[cfg(feature = "core")]
mod text_formatting;
#[cfg(feature = "core")]
mod tool_output;
#[cfg(feature = "core")]
mod transcript;
#[cfg(feature = "core")]
mod tui_snapshot;
#[cfg(feature = "core")]
mod ui_state;
#[cfg(feature = "core")]
mod view_mode;

#[cfg(all(test, feature = "core"))]
mod shell_tests;
#[cfg(all(test, feature = "core"))]
mod test_support;

pub use protocol_state::{AgentStatusView, ConnectionState, ProtocolState};

/// Run the TUI (embedded mode, requires `core` feature).
#[cfg(feature = "core")]
pub fn run_tui() -> Result<()> {
    run_tui_with_options(false)
}

/// Resume the last session (embedded mode, requires `core` feature).
#[cfg(feature = "core")]
pub fn run_tui_with_resume_last() -> Result<()> {
    run_tui_with_options(true)
}

#[cfg(feature = "core")]
fn run_tui_with_options(resume_last: bool) -> Result<()> {
    use agent_core::logging;
    use agent_core::logging::RunMode;
    use agent_core::probe;
    use agent_core::workplace_store::WorkplaceStore;

    if logging::current_log_path().is_none() {
        let launch_cwd = std::env::current_dir()?;
        if let Ok(workplace) = WorkplaceStore::for_cwd(&launch_cwd)
            && workplace.ensure().is_ok()
            && let Ok(initialized) = logging::init_for_workplace(
                &workplace,
                if resume_last {
                    RunMode::ResumeLast
                } else {
                    RunMode::Tui
                },
            )
        {
            logging::debug_event(
                "app.launch",
                "initialized TUI logging",
                serde_json::json!({
                    "cwd": launch_cwd.display().to_string(),
                    "resume_last": resume_last,
                    "log_path": initialized.log_path.display().to_string(),
                }),
            );
        }
    }

    if !probe::has_any_real_provider() {
        anyhow::bail!(
            "no real provider detected: install codex or claude, or run `agile-agent doctor`"
        );
    }

    let mut terminal = terminal::setup_terminal()?;
    let result = app_loop::run(terminal.terminal_mut(), resume_last);
    terminal.restore()?;
    result.map(|_| ())
}

/// Stub when `core` feature is disabled (protocol-only mode).
#[cfg(not(feature = "core"))]
pub fn run_tui() -> Result<()> {
    anyhow::bail!("TUI embedded mode requires the `core` feature; use protocol mode instead")
}

/// Stub when `core` feature is disabled.
#[cfg(not(feature = "core"))]
pub fn run_tui_with_resume_last() -> Result<()> {
    run_tui()
}
