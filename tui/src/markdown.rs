use std::path::Path;

use pulldown_cmark::CodeBlockKind;
use pulldown_cmark::Event;
use pulldown_cmark::Parser;
use pulldown_cmark::Tag;
use pulldown_cmark::TagEnd;
use ratatui::style::Color;
use ratatui::style::Modifier;
use ratatui::style::Style;
use ratatui::text::Line;
use ratatui::text::Span;
use textwrap::wrap;

pub fn append_markdown(
    text: &str,
    max_width: Option<usize>,
    _cwd: Option<&Path>,
    out: &mut Vec<Line<'static>>,
) {
    out.extend(render_markdown_lines(
        text,
        max_width.unwrap_or(usize::MAX.saturating_sub(1)),
    ));
}

pub fn render_markdown_lines(text: &str, max_width: usize) -> Vec<Line<'static>> {
    if text.is_empty() {
        return vec![Line::from(Span::styled(
            "(waiting...)",
            Style::default().fg(Color::DarkGray),
        ))];
    }

    let mut lines = Vec::new();
    let parser = Parser::new(text);

    let mut current_line_spans: Vec<Span<'static>> = Vec::new();
    let mut in_code_block = false;
    let mut code_block_content = String::new();
    let mut code_block_lang = String::new();
    let mut current_style = Style::default();
    let mut blockquote_depth = 0usize;
    let mut list_stack: Vec<Option<u64>> = Vec::new();

    for event in parser {
        match event {
            Event::Start(Tag::Heading { level, .. }) => {
                current_style = Style::default().add_modifier(Modifier::BOLD);
                current_line_spans.push(Span::styled(
                    "#".repeat(level as usize) + " ",
                    current_style,
                ));
            }
            Event::End(TagEnd::Heading(_)) => {
                flush_wrapped_line(
                    &mut lines,
                    &mut current_line_spans,
                    max_width,
                    blockquote_depth,
                );
            }
            Event::Start(Tag::Paragraph) => {}
            Event::End(TagEnd::Paragraph) => {
                flush_wrapped_line(
                    &mut lines,
                    &mut current_line_spans,
                    max_width,
                    blockquote_depth,
                );
                if !lines.is_empty() {
                    lines.push(Line::from(""));
                }
            }
            Event::Start(Tag::BlockQuote(_)) => {
                blockquote_depth += 1;
            }
            Event::End(TagEnd::BlockQuote(_)) => {
                blockquote_depth = blockquote_depth.saturating_sub(1);
                if !lines.last().is_some_and(|line| line.spans.is_empty()) {
                    lines.push(Line::from(""));
                }
            }
            Event::Start(Tag::CodeBlock(kind)) => {
                in_code_block = true;
                code_block_lang = match kind {
                    CodeBlockKind::Fenced(lang) => lang.to_string(),
                    CodeBlockKind::Indented => String::new(),
                };
                lines.push(styled_line_with_blockquote_prefix(
                    format!("```{}", code_block_lang),
                    Style::default().fg(Color::DarkGray),
                    blockquote_depth,
                ));
            }
            Event::End(TagEnd::CodeBlock) => {
                in_code_block = false;
                for code_line in code_block_content.lines() {
                    lines.push(styled_line_with_blockquote_prefix(
                        format!("    {}", code_line),
                        Style::default().fg(Color::Cyan),
                        blockquote_depth,
                    ));
                }
                lines.push(styled_line_with_blockquote_prefix(
                    "```".to_string(),
                    Style::default().fg(Color::DarkGray),
                    blockquote_depth,
                ));
                lines.push(Line::from(""));
                code_block_content.clear();
                code_block_lang.clear();
            }
            Event::Start(Tag::List(start)) => {
                list_stack.push(start);
            }
            Event::End(TagEnd::List(_)) => {
                if !lines.last().is_some_and(|line| line.spans.is_empty()) {
                    lines.push(Line::from(""));
                }
                list_stack.pop();
            }
            Event::Start(Tag::Item) => {
                let marker = match list_stack.last_mut() {
                    Some(Some(next_index)) => {
                        let marker = format!("{}. ", *next_index);
                        *next_index += 1;
                        marker
                    }
                    _ => "• ".to_string(),
                };
                current_line_spans
                    .push(Span::styled(marker, Style::default().fg(Color::LightBlue)));
            }
            Event::End(TagEnd::Item) => {
                flush_wrapped_line(
                    &mut lines,
                    &mut current_line_spans,
                    max_width,
                    blockquote_depth,
                );
            }
            Event::Start(Tag::Emphasis) => {
                current_style = current_style.add_modifier(Modifier::ITALIC);
            }
            Event::End(TagEnd::Emphasis) => {
                current_style = current_style.remove_modifier(Modifier::ITALIC);
            }
            Event::Start(Tag::Strong) => {
                current_style = current_style.add_modifier(Modifier::BOLD);
            }
            Event::End(TagEnd::Strong) => {
                current_style = current_style.remove_modifier(Modifier::BOLD);
            }
            Event::Code(code) => {
                current_line_spans.push(Span::styled(
                    format!("`{}`", code),
                    Style::default().fg(Color::Cyan),
                ));
            }
            Event::Text(text) => {
                if in_code_block {
                    code_block_content.push_str(&text);
                } else {
                    current_line_spans.push(Span::styled(text.to_string(), current_style));
                }
            }
            Event::SoftBreak | Event::HardBreak => {
                flush_wrapped_line(
                    &mut lines,
                    &mut current_line_spans,
                    max_width,
                    blockquote_depth,
                );
            }
            Event::Start(Tag::Link { .. }) => {
                current_style = Style::default()
                    .fg(Color::LightBlue)
                    .add_modifier(Modifier::UNDERLINED);
            }
            Event::End(TagEnd::Link) => {
                current_style = Style::default();
            }
            _ => {}
        }
    }

    flush_wrapped_line(
        &mut lines,
        &mut current_line_spans,
        max_width,
        blockquote_depth,
    );

    while lines.last().is_some_and(|line| line.spans.is_empty()) {
        lines.pop();
    }

    if lines.is_empty() {
        lines.push(Line::from(text.to_string()));
    }

    lines
}

fn flush_wrapped_line(
    lines: &mut Vec<Line<'static>>,
    current_line_spans: &mut Vec<Span<'static>>,
    max_width: usize,
    blockquote_depth: usize,
) {
    if current_line_spans.is_empty() {
        return;
    }
    let paragraph_text = spans_to_string(current_line_spans);
    let wrapped = wrap_text(&paragraph_text, content_width(max_width, blockquote_depth));
    for wrapped_line in wrapped {
        lines.push(line_with_blockquote_prefix(wrapped_line, blockquote_depth));
    }
    current_line_spans.clear();
}

fn spans_to_string(spans: &[Span<'static>]) -> String {
    spans.iter().map(|span| span.content.as_ref()).collect()
}

fn content_width(max_width: usize, blockquote_depth: usize) -> usize {
    max_width.saturating_sub(blockquote_depth * 2).max(1)
}

fn wrap_text(text: &str, max_width: usize) -> Vec<String> {
    let options = textwrap::Options::new(max_width)
        .word_separator(textwrap::WordSeparator::UnicodeBreakProperties);
    wrap(text, options)
        .into_iter()
        .map(|line| line.into_owned())
        .collect()
}

fn blockquote_prefix(depth: usize) -> String {
    "> ".repeat(depth)
}

fn line_with_blockquote_prefix(text: String, depth: usize) -> Line<'static> {
    if depth == 0 {
        Line::from(text)
    } else {
        Line::from(format!("{}{}", blockquote_prefix(depth), text))
    }
}

fn styled_line_with_blockquote_prefix(text: String, style: Style, depth: usize) -> Line<'static> {
    if depth == 0 {
        Line::from(Span::styled(text, style))
    } else {
        Line::from(vec![
            Span::raw(blockquote_prefix(depth)),
            Span::styled(text, style),
        ])
    }
}

#[cfg(test)]
mod tests {
    use super::render_markdown_lines;
    use ratatui::text::Line;

    fn lines_to_strings(lines: &[Line<'static>]) -> Vec<String> {
        lines
            .iter()
            .map(|line| {
                line.spans
                    .iter()
                    .map(|span| span.content.as_ref())
                    .collect::<String>()
            })
            .collect()
    }

    #[test]
    fn renders_heading_and_paragraph() {
        let lines = render_markdown_lines("# Title\n\nParagraph text.", 60);
        let rendered = lines_to_strings(&lines);
        assert!(rendered.iter().any(|line| line == "# Title"));
        assert!(rendered.iter().any(|line| line == "Paragraph text."));
    }

    #[test]
    fn renders_ordered_list_items() {
        let lines = render_markdown_lines("1. first\n2. second\n", 60);
        let rendered = lines_to_strings(&lines);
        assert!(rendered.iter().any(|line| line == "1. first"));
        assert!(rendered.iter().any(|line| line == "2. second"));
    }

    #[test]
    fn renders_blockquotes() {
        let lines = render_markdown_lines("> quoted", 60);
        let rendered = lines_to_strings(&lines);
        assert!(rendered.iter().any(|line| line == "> quoted"));
    }

    #[test]
    fn renders_code_blocks() {
        let lines = render_markdown_lines("```rs\nfn main() {}\n```", 60);
        let rendered = lines_to_strings(&lines);
        assert!(rendered.iter().any(|line| line == "```rs"));
        assert!(rendered.iter().any(|line| line == "    fn main() {}"));
    }
}
