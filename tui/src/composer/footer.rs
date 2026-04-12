use std::path::Path;

use agent_core::app::AppStatus;
use agent_core::app::LoopPhase;
use ratatui::style::Modifier;
use ratatui::style::Style;
use ratatui::text::Line;
use ratatui::text::Span;
use unicode_width::UnicodeWidthStr;

use crate::ui_state::TuiState;

pub fn build_footer_line(state: &TuiState, width: u16) -> Line<'static> {
    let left = if state.app.status == AppStatus::Responding {
        format!(
            "  {}  ctrl+t transcript  ctrl+j newline",
            state.app.selected_provider.label()
        )
    } else {
        "  enter send  ctrl+j newline  tab new agent  ctrl+t transcript".to_string()
    };

    let cwd_label = display_cwd_label(&state.app.cwd);
    let right = format!(
        "{} · {} · {} · {}",
        state.agent_runtime.codename().as_str(),
        state.app.selected_provider.label(),
        loop_phase_label(state.app.loop_phase),
        cwd_label
    );

    let total_width = width as usize;
    let left_width = left.width();
    let right_width = right.width();
    let gap = if total_width > left_width + right_width {
        total_width - left_width - right_width
    } else {
        2
    };

    Line::from(vec![
        Span::styled(left, Style::default().add_modifier(Modifier::DIM)),
        Span::raw(" ".repeat(gap)),
        Span::styled(right, Style::default().add_modifier(Modifier::DIM)),
    ])
}

fn loop_phase_label(phase: LoopPhase) -> &'static str {
    match phase {
        LoopPhase::Idle => "idle",
        LoopPhase::Planning => "planning",
        LoopPhase::Executing => "executing",
        LoopPhase::Verifying => "verifying",
        LoopPhase::Escalating => "escalating",
    }
}

fn display_cwd_label(path: &Path) -> String {
    path.file_name()
        .and_then(|value| value.to_str())
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| path.display().to_string())
}
