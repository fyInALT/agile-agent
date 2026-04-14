use diffy::Patch;
use ratatui::style::Color;
use ratatui::style::Modifier;
use ratatui::style::Style;
use ratatui::text::Line;
use ratatui::text::Span;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DiffFileSummary {
    pub(crate) path: String,
    pub(crate) added: usize,
    pub(crate) removed: usize,
}

pub(crate) fn diff_style_for_line(line: &str) -> Style {
    if line.starts_with('+') && !line.starts_with("+++") {
        Style::default().fg(Color::Green)
    } else if line.starts_with('-') && !line.starts_with("---") {
        Style::default().fg(Color::Red)
    } else if line.starts_with("@@") {
        Style::default().fg(Color::Cyan)
    } else if line.starts_with("diff --git")
        || line.starts_with("index ")
        || line.starts_with("--- ")
        || line.starts_with("+++ ")
    {
        Style::default()
            .fg(Color::DarkGray)
            .add_modifier(Modifier::DIM)
    } else {
        Style::default().add_modifier(Modifier::DIM)
    }
}

pub(crate) fn summarize_unified_diff(diff: &str) -> Vec<DiffFileSummary> {
    let mut summaries = Vec::new();
    let mut current: Option<DiffFileSummary> = None;

    for line in diff.lines() {
        if let Some(rest) = line.strip_prefix("diff --git a/") {
            if let Some(summary) = current.take() {
                summaries.push(summary);
            }
            let path = rest
                .split_whitespace()
                .next()
                .unwrap_or(rest)
                .trim_end_matches(" b/")
                .to_string();
            current = Some(DiffFileSummary {
                path,
                added: 0,
                removed: 0,
            });
            continue;
        }

        if let Some(summary) = current.as_mut() {
            if line.starts_with('+') && !line.starts_with("+++") {
                summary.added += 1;
            } else if line.starts_with('-') && !line.starts_with("---") {
                summary.removed += 1;
            }
        }
    }

    if let Some(summary) = current.take() {
        summaries.push(summary);
    }

    summaries
}

pub(crate) fn format_hunk_range(start: usize, len: usize) -> String {
    if len == 1 {
        start.to_string()
    } else {
        format!("{start},{len}")
    }
}

pub(crate) fn diff_line(
    prefix: &str,
    line_no_width: usize,
    line_no: Option<usize>,
    sign: char,
    content: &str,
    style: Style,
) -> Line<'static> {
    let number = line_no
        .map(|value| format!("{value:>width$}", width = line_no_width))
        .unwrap_or_else(|| " ".repeat(line_no_width));
    Line::from(vec![
        Span::styled(
            prefix.to_string(),
            Style::default()
                .fg(Color::DarkGray)
                .add_modifier(Modifier::DIM),
        ),
        Span::styled(
            number,
            Style::default()
                .fg(Color::DarkGray)
                .add_modifier(Modifier::DIM),
        ),
        Span::raw(" "),
        Span::styled(format!("{sign}"), style),
        Span::raw(" "),
        Span::styled(content.to_string(), style),
    ])
}

pub(crate) fn render_unified_diff_lines(
    output: &str,
    initial_prefix: &str,
    continuation_prefix: &str,
) -> Vec<Line<'static>> {
    let Ok(patch) = Patch::from_str(output) else {
        return output
            .lines()
            .map(|line| {
                Line::from(vec![
                    Span::styled(
                        continuation_prefix.to_string(),
                        Style::default()
                            .fg(Color::DarkGray)
                            .add_modifier(Modifier::DIM),
                    ),
                    Span::styled(line.to_string(), diff_style_for_line(line)),
                ])
            })
            .collect();
    };

    let max_line_number = patch
        .hunks()
        .iter()
        .flat_map(|hunk| [hunk.old_range().start(), hunk.new_range().start()])
        .max()
        .unwrap_or(1);
    let line_no_width = max_line_number.to_string().len().max(1);

    let mut out = Vec::new();
    let mut first_hunk = true;
    for hunk in patch.hunks() {
        if !first_hunk {
            out.push(Line::from(""));
        }
        first_hunk = false;
        out.push(Line::from(vec![
            Span::styled(
                initial_prefix.to_string(),
                Style::default()
                    .fg(Color::DarkGray)
                    .add_modifier(Modifier::DIM),
            ),
            Span::styled(
                format!(
                    "@@ -{} +{} @@",
                    format_hunk_range(hunk.old_range().start(), hunk.old_range().len()),
                    format_hunk_range(hunk.new_range().start(), hunk.new_range().len())
                ),
                Style::default().fg(Color::Cyan),
            ),
        ]));

        let mut old_ln = hunk.old_range().start();
        let mut new_ln = hunk.new_range().start();
        for line in hunk.lines() {
            match line {
                diffy::Line::Insert(text) => {
                    out.push(diff_line(
                        continuation_prefix,
                        line_no_width,
                        Some(new_ln),
                        '+',
                        text.trim_end_matches('\n'),
                        Style::default().fg(Color::Green),
                    ));
                    new_ln += 1;
                }
                diffy::Line::Delete(text) => {
                    out.push(diff_line(
                        continuation_prefix,
                        line_no_width,
                        Some(old_ln),
                        '-',
                        text.trim_end_matches('\n'),
                        Style::default().fg(Color::Red),
                    ));
                    old_ln += 1;
                }
                diffy::Line::Context(text) => {
                    out.push(diff_line(
                        continuation_prefix,
                        line_no_width,
                        Some(new_ln),
                        ' ',
                        text.trim_end_matches('\n'),
                        Style::default().add_modifier(Modifier::DIM),
                    ));
                    old_ln += 1;
                    new_ln += 1;
                }
            }
        }
    }

    out
}
