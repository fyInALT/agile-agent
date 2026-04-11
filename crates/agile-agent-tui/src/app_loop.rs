use std::time::Duration;

use agile_agent_core::app::AppState;
use agile_agent_core::mock_provider;
use anyhow::Result;
use crossterm::event;
use crossterm::event::Event;
use std::collections::VecDeque;

use crate::input::InputOutcome;
use crate::input::handle_key_event;
use crate::render::render_app;
use crate::terminal::AppTerminal;

pub fn run(terminal: &mut AppTerminal) -> Result<()> {
    let mut state = AppState::default();
    let mut pending_reply_chunks: VecDeque<String> = VecDeque::new();

    loop {
        terminal.draw(|frame| render_app(frame, &state))?;

        if state.should_quit {
            break;
        }

        let poll_timeout = if pending_reply_chunks.is_empty() {
            Duration::from_millis(250)
        } else {
            Duration::from_millis(80)
        };

        if event::poll(poll_timeout)? {
            match event::read()? {
                Event::Key(key_event) => match handle_key_event(&mut state, key_event) {
                    InputOutcome::None => {}
                    InputOutcome::Quit => state.request_quit(),
                    InputOutcome::Submit(user_input) => {
                        state.begin_mock_response();
                        pending_reply_chunks =
                            mock_provider::build_reply_chunks(&user_input).into();
                    }
                },
                Event::Resize(_, _) => {}
                _ => {}
            }
        } else if let Some(chunk) = pending_reply_chunks.pop_front() {
            state.append_assistant_chunk(&chunk);
            if pending_reply_chunks.is_empty() {
                state.finish_mock_response();
            }
        }
    }

    Ok(())
}
