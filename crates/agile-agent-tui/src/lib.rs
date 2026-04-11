use anyhow::Result;

mod app_loop;
mod input;
mod render;
mod terminal;

pub fn run_tui() -> Result<()> {
    run_tui_with_options(false)
}

pub fn run_tui_with_resume_last() -> Result<()> {
    run_tui_with_options(true)
}

fn run_tui_with_options(resume_last: bool) -> Result<()> {
    let mut terminal = terminal::setup_terminal()?;
    let result = app_loop::run(terminal.terminal_mut(), resume_last);
    terminal.restore()?;
    result.map(|_| ())
}
