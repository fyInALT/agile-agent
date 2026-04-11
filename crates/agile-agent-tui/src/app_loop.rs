use std::time::Duration;

use agile_agent_core::app::AppState;
use anyhow::Result;
use crossterm::event;
use crossterm::event::Event;
use crossterm::event::KeyCode;
use crossterm::event::KeyEventKind;

use crate::render::render_app;
use crate::terminal::AppTerminal;

pub fn run(terminal: &mut AppTerminal) -> Result<()> {
    let mut state = AppState::default();

    loop {
        terminal.draw(|frame| render_app(frame, &state))?;

        if state.should_quit {
            break;
        }

        if event::poll(Duration::from_millis(250))? {
            match event::read()? {
                Event::Key(key_event) if key_event.kind == KeyEventKind::Press => {
                    if matches!(key_event.code, KeyCode::Char('q') | KeyCode::Esc) {
                        state.request_quit();
                    }
                }
                Event::Resize(_, _) => {}
                _ => {}
            }
        }
    }

    Ok(())
}
