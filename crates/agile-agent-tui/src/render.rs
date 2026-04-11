use agile_agent_core::app::AppState;
use agile_agent_core::app::AppStatus;
use agile_agent_core::app::TranscriptEntry;
use ratatui::Frame;
use ratatui::layout::Constraint;
use ratatui::layout::Direction;
use ratatui::layout::Layout;
use ratatui::style::Modifier;
use ratatui::style::Style;
use ratatui::text::Line;
use ratatui::widgets::Block;
use ratatui::widgets::Borders;
use ratatui::widgets::Paragraph;
use ratatui::widgets::Wrap;

pub fn render_app(frame: &mut Frame<'_>, state: &AppState) {
    let areas = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Min(5),
            Constraint::Length(3),
        ])
        .split(frame.area());

    let status_text = match state.status {
        AppStatus::Idle => "idle",
        AppStatus::Responding => "responding",
    };
    let header = Paragraph::new(Line::from(format!(
        "agile-agent | provider: {} | status: {status_text} | tab: switch | q/esc: quit",
        state.selected_provider.label()
    )))
    .style(Style::default().add_modifier(Modifier::BOLD));
    frame.render_widget(header, areas[0]);

    let transcript_lines = if state.transcript.is_empty() {
        vec![Line::from("No messages yet.")]
    } else {
        state
            .transcript
            .iter()
            .map(|entry| match entry {
                TranscriptEntry::User(text) => Line::from(format!("You: {text}")),
                TranscriptEntry::Assistant(text) => Line::from(format!("Assistant: {text}")),
                TranscriptEntry::Status(text) => Line::from(format!("Status: {text}")),
                TranscriptEntry::Error(text) => Line::from(format!("Error: {text}")),
            })
            .collect()
    };
    let transcript = Paragraph::new(transcript_lines)
        .block(Block::default().title("Transcript").borders(Borders::ALL))
        .wrap(Wrap { trim: false });
    frame.render_widget(transcript, areas[1]);

    let composer_text = if state.input.is_empty() {
        "> ".to_string()
    } else {
        format!("> {}", state.input)
    };
    let composer = Paragraph::new(composer_text)
        .block(Block::default().title("Composer").borders(Borders::ALL))
        .wrap(Wrap { trim: false });
    frame.render_widget(composer, areas[2]);
}
