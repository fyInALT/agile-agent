use agent_core::logging;
use agent_core::logging::RunMode;
use agent_core::probe;
use agent_core::workplace_store::WorkplaceStore;
use anyhow::Result;

mod app_loop;
mod composer;
mod input;
mod markdown;
mod render;
mod terminal;
mod transcript;
mod ui_state;

#[cfg(test)]
mod shell_tests;
#[cfg(test)]
mod test_support;

pub fn run_tui() -> Result<()> {
    run_tui_with_options(false)
}

pub fn run_tui_with_resume_last() -> Result<()> {
    run_tui_with_options(true)
}

fn run_tui_with_options(resume_last: bool) -> Result<()> {
    if logging::current_log_path().is_none() {
        let launch_cwd = std::env::current_dir()?;
        if let Ok(workplace) = WorkplaceStore::for_cwd(&launch_cwd) {
            if workplace.ensure().is_ok() {
                if let Ok(initialized) = logging::init_for_workplace(
                    &workplace,
                    if resume_last {
                        RunMode::ResumeLast
                    } else {
                        RunMode::Tui
                    },
                ) {
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
