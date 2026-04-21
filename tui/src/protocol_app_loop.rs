//! Protocol-only TUI event loop (zero agent_core dependency).

use crate::protocol_state::{ConnectionState, ProtocolState};
use crate::websocket_client::{ServerMessage, WebSocketClient};
use anyhow::Result;
use crossterm::event::{self, Event as CEvent, KeyCode, KeyModifiers};
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};
use ratatui::Frame;
use std::time::Duration;

pub fn run() -> Result<()> {
    run_with_resume_last()
}

pub fn run_with_resume_last() -> Result<()> {
    let rt = tokio::runtime::Runtime::new()?;
    rt.block_on(async_run())
}

async fn async_run() -> Result<()> {
    let mut terminal = crate::terminal::setup_terminal()?;

    let mut state = ProtocolState::default();
    state.connection_state = ConnectionState::Connecting;

    // Auto-link: discover existing daemon or spawn a new one.
    let workplace = agent_protocol::workplace::resolve_workplace()?;
    workplace.ensure().await?;

    let result = agent_protocol::client::auto_link::auto_link(
        workplace.workplace_id(),
        &workplace.daemon_json_path(),
        find_daemon_binary().as_deref(),
        Duration::from_secs(10),
    )
    .await?;

    // Connect WebSocket.
    let (client, mut event_rx) = WebSocketClient::connect(&result.websocket_url).await?;

    // Initialize session.
    let init_params = agent_protocol::methods::InitializeParams {
        client_type: agent_protocol::methods::ClientType::Tui,
        client_version: env!("CARGO_PKG_VERSION").to_string(),
        resume_snapshot_id: None,
        protocol_version: Some(agent_protocol::PROTOCOL_VERSION.to_string()),
    };
    let _resp = client
        .call("session.initialize", Some(serde_json::to_value(&init_params)?))
        .await?;

    state.connection_state = ConnectionState::Connected;

    let mut last_tick = std::time::Instant::now();
    let tick_rate = Duration::from_millis(250);

    loop {
        let timeout = tick_rate.saturating_sub(last_tick.elapsed());
        if crossterm::event::poll(timeout)? {
            if let CEvent::Key(key) = event::read()? {
                match key.code {
                    KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => break,
                    KeyCode::Char('q') if key.modifiers.contains(KeyModifiers::CONTROL) => break,
                    KeyCode::Enter => {
                        if let Some(text) = state.composer.take_submission() {
                            let _ = client.notify(
                                "session.sendInput",
                                Some(serde_json::json!({ "text": text })),
                            );
                        }
                    }
                    KeyCode::Char(ch) => {
                        state.composer.insert_char(ch);
                    }
                    KeyCode::Backspace => {
                        state.composer.backspace();
                    }
                    _ => {}
                }
            }
        }

        // Drain incoming events.
        while let Ok(msg) = event_rx.try_recv() {
            match msg {
                ServerMessage::Notification(ev) => {
                    crate::event_handler::apply_event(&mut state, &ev);
                }
                _ => {}
            }
        }

        if last_tick.elapsed() >= tick_rate {
            last_tick = std::time::Instant::now();
        }

        terminal.terminal_mut().draw(|f| draw_ui(f, &state))?;
    }

    terminal.restore()?;
    Ok(())
}

fn find_daemon_binary() -> Option<std::path::PathBuf> {
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            let candidate = dir.join("agent-daemon");
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }
    None
}

fn draw_ui(frame: &mut Frame, state: &ProtocolState) {
    let area = frame.area();

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(3), Constraint::Length(3)])
        .split(area);

    draw_transcript(frame, state, chunks[0]);
    draw_status_bar(frame, state, chunks[1]);
}

fn draw_transcript(frame: &mut Frame, state: &ProtocolState, area: ratatui::layout::Rect) {
    let mut text = Vec::new();
    for item in &state.transcript_items {
        let prefix = match item.kind {
            agent_protocol::state::ItemKind::UserInput => "you: ",
            agent_protocol::state::ItemKind::AssistantOutput => "assistant: ",
            agent_protocol::state::ItemKind::ToolCall => "tool: ",
            agent_protocol::state::ItemKind::ToolResult => "result: ",
            agent_protocol::state::ItemKind::SystemMessage => "system: ",
        };
        text.push(Line::from(vec![
            Span::styled(prefix, Style::default().fg(Color::Cyan)),
            Span::raw(&item.content),
        ]));
    }

    let paragraph = Paragraph::new(text)
        .block(Block::default().title("Transcript").borders(Borders::ALL))
        .wrap(Wrap { trim: true });
    frame.render_widget(paragraph, area);
}

fn draw_status_bar(frame: &mut Frame, state: &ProtocolState, area: ratatui::layout::Rect) {
    let conn = match state.connection_state {
        ConnectionState::Connected => Span::styled("●", Style::default().fg(Color::Green)),
        ConnectionState::Connecting => Span::styled("◐", Style::default().fg(Color::Yellow)),
        ConnectionState::Reconnecting => Span::styled("◐", Style::default().fg(Color::Yellow)),
        ConnectionState::Disconnected => Span::styled("●", Style::default().fg(Color::Red)),
        ConnectionState::Error => Span::styled("●", Style::default().fg(Color::Red)),
    };

    let busy = if state.is_busy() {
        Span::styled(" [busy]", Style::default().fg(Color::Yellow))
    } else {
        Span::raw("")
    };

    let status = Line::from(vec![
        conn,
        Span::raw(format!(" {} agents", state.agents.len())),
        busy,
    ]);

    let paragraph = Paragraph::new(status).block(Block::default().borders(Borders::ALL));
    frame.render_widget(paragraph, area);
}
