use agent_core::probe;
use anyhow::Result;

mod app_loop;
mod composer;
mod input;
mod markdown;
mod render;
mod terminal;
mod transcript;
mod ui_state;

pub fn run_tui() -> Result<()> {
    run_tui_with_options(false)
}

pub fn run_tui_with_resume_last() -> Result<()> {
    run_tui_with_options(true)
}

fn run_tui_with_options(resume_last: bool) -> Result<()> {
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
