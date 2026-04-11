use anyhow::Result;

mod app_loop;
mod input;
mod render;
mod terminal;

pub fn run_tui() -> Result<()> {
    let mut terminal = terminal::setup_terminal()?;
    let result = app_loop::run(terminal.terminal_mut());
    terminal.restore()?;
    result
}
