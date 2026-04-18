use agent_core::logging;
use agent_core::logging::RunMode;
use agent_core::probe;
use agent_core::workplace_store::WorkplaceStore;
use anyhow::Result;

mod app_loop;
mod command_runtime;
mod composer;
mod confirmation_overlay;
mod diff_render;
mod exec_command;
mod exec_semantics;
mod history_cell;
mod human_decision_overlay;
mod input;
mod launch_config_overlay;
mod markdown;
mod markdown_stream;
mod overview_row;
mod overview_state;
mod provider_overlay;
mod render;
mod resume_overlay;
mod streaming;
mod task_decision_overlay;
mod task_detail_view;
mod task_panel;
mod terminal;
mod text_formatting;
mod tool_output;
mod transcript;
mod tui_snapshot;
mod ui_state;
mod view_mode;

#[cfg(test)]
mod shell_tests;
#[cfg(test)]
mod test_support;

pub use tui_snapshot::TuiShutdownSnapshot;

pub fn run_tui() -> Result<()> {
    run_tui_with_options(false)
}

pub fn run_tui_with_resume_last() -> Result<()> {
    run_tui_with_options(true)
}

fn run_tui_with_options(resume_last: bool) -> Result<()> {
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
