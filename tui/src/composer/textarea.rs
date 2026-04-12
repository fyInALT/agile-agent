use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Color;
use ratatui::style::Style;
use ratatui::style::Stylize;
use ratatui::text::Line;
use ratatui::text::Span;
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthChar;
use unicode_width::UnicodeWidthStr;

#[derive(Debug, Default, Clone, Copy)]
pub struct TextAreaState {
    pub scroll_row: usize,
}

#[derive(Debug, Clone)]
pub struct TextArea {
    text: String,
    cursor: usize,
    preferred_column: Option<usize>,
}

#[derive(Debug, Clone)]
struct VisualLine {
    start: usize,
    end: usize,
    text: String,
}

impl TextArea {
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn new() -> Self {
        Self {
            text: String::new(),
            cursor: 0,
            preferred_column: None,
        }
    }

    pub fn from_text(text: String) -> Self {
        let cursor = text.len();
        Self {
            text,
            cursor,
            preferred_column: None,
        }
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn is_empty(&self) -> bool {
        self.text.is_empty()
    }

    pub fn insert_char(&mut self, ch: char) {
        self.text.insert(self.cursor, ch);
        self.cursor += ch.len_utf8();
        self.preferred_column = None;
    }

    pub fn insert_text(&mut self, text: &str) {
        self.text.insert_str(self.cursor, text);
        self.cursor += text.len();
        self.preferred_column = None;
    }

    pub fn insert_newline(&mut self) {
        self.insert_char('\n');
    }

    pub fn backspace(&mut self) {
        if self.cursor == 0 {
            return;
        }
        let previous = previous_grapheme_boundary(&self.text, self.cursor);
        self.text.replace_range(previous..self.cursor, "");
        self.cursor = previous;
        self.preferred_column = None;
    }

    pub fn move_left(&mut self) {
        if self.cursor == 0 {
            return;
        }
        self.cursor = previous_grapheme_boundary(&self.text, self.cursor);
        self.preferred_column = None;
    }

    pub fn move_right(&mut self) {
        if self.cursor >= self.text.len() {
            return;
        }
        self.cursor = next_grapheme_boundary(&self.text, self.cursor);
        self.preferred_column = None;
    }

    pub fn move_home(&mut self, width: u16) {
        let lines = self.visual_lines(width);
        let line_index = self.cursor_visual_line_index(&lines);
        self.cursor = lines
            .get(line_index)
            .map(|line| line.start)
            .unwrap_or(self.cursor);
        self.preferred_column = None;
    }

    pub fn move_end(&mut self, width: u16) {
        let lines = self.visual_lines(width);
        let line_index = self.cursor_visual_line_index(&lines);
        self.cursor = lines
            .get(line_index)
            .map(|line| line.end)
            .unwrap_or(self.cursor);
        self.preferred_column = None;
    }

    pub fn move_up(&mut self, width: u16) {
        let lines = self.visual_lines(width);
        let current = self.cursor_visual_line_index(&lines);
        if current == 0 {
            return;
        }
        let target_column = self.cursor_visual_column(&lines, current);
        self.cursor = cursor_for_visual_column(&lines[current - 1], target_column);
        self.preferred_column = Some(target_column);
    }

    pub fn move_down(&mut self, width: u16) {
        let lines = self.visual_lines(width);
        let current = self.cursor_visual_line_index(&lines);
        if current + 1 >= lines.len() {
            return;
        }
        let target_column = self.cursor_visual_column(&lines, current);
        self.cursor = cursor_for_visual_column(&lines[current + 1], target_column);
        self.preferred_column = Some(target_column);
    }

    pub fn take_submission(&mut self) -> Option<String> {
        if self.text.trim().is_empty() {
            return None;
        }
        self.cursor = 0;
        self.preferred_column = None;
        Some(std::mem::take(&mut self.text))
    }

    pub fn desired_height(&self, width: u16, max_rows: u16) -> u16 {
        let line_count = self.visual_lines(width).len().max(1) as u16;
        line_count.clamp(1, max_rows.max(1))
    }

    pub fn cursor_position(&self, area: Rect, state: &mut TextAreaState) -> (u16, u16) {
        let lines = self.visual_lines(area.width);
        let line_index = self.cursor_visual_line_index(&lines);
        let scroll = effective_scroll(
            line_index,
            lines.len(),
            area.height as usize,
            state.scroll_row,
        );
        state.scroll_row = scroll;

        let line = lines
            .get(line_index)
            .cloned()
            .unwrap_or_else(|| VisualLine {
                start: 0,
                end: 0,
                text: String::new(),
            });
        let column = display_width(&self.text[line.start..self.cursor])
            .min(area.width.saturating_sub(2) as usize);
        (
            area.x + 2 + column as u16,
            area.y + line_index.saturating_sub(scroll) as u16,
        )
    }

    pub fn render(
        &self,
        area: Rect,
        buf: &mut Buffer,
        state: &mut TextAreaState,
        placeholder: &str,
    ) {
        let lines = self.visual_lines(area.width);
        let line_index = self.cursor_visual_line_index(&lines);
        let scroll = effective_scroll(
            line_index,
            lines.len(),
            area.height as usize,
            state.scroll_row,
        );
        state.scroll_row = scroll;
        let base_style = composer_base_style();

        for y in 0..area.height {
            for x in 0..area.width {
                buf[(area.x + x, area.y + y)]
                    .set_symbol(" ")
                    .set_style(base_style);
            }
        }

        let visible_start = scroll;
        let visible_end = (scroll + area.height as usize).min(lines.len());
        if self.text.is_empty() {
            let line = Line::from(vec![
                Span::styled("› ", base_style),
                Span::styled(placeholder.to_string(), base_style.dim()),
            ]);
            render_line(buf, area, 0, &line);
            return;
        }

        for (row, line) in lines[visible_start..visible_end].iter().enumerate() {
            let prefix = if row + visible_start == 0 {
                "› "
            } else {
                "  "
            };
            let rendered = Line::from(vec![
                Span::styled(prefix.to_string(), base_style),
                Span::styled(line.text.clone(), base_style),
            ]);
            render_line(buf, area, row as u16, &rendered);
        }
    }

    fn visual_lines(&self, width: u16) -> Vec<VisualLine> {
        let content_width = width.saturating_sub(2).max(1) as usize;
        if self.text.is_empty() {
            return vec![VisualLine {
                start: 0,
                end: 0,
                text: String::new(),
            }];
        }

        let mut lines = Vec::new();
        let bytes = self.text.as_str();
        let mut logical_start = 0usize;

        while logical_start <= bytes.len() {
            let Some(relative_break) = bytes[logical_start..].find('\n') else {
                push_wrapped_segment(bytes, logical_start, bytes.len(), content_width, &mut lines);
                break;
            };
            let logical_end = logical_start + relative_break;
            push_wrapped_segment(bytes, logical_start, logical_end, content_width, &mut lines);
            logical_start = logical_end + 1;
            if logical_start == bytes.len() {
                lines.push(VisualLine {
                    start: logical_start,
                    end: logical_start,
                    text: String::new(),
                });
                break;
            }
        }

        if lines.is_empty() {
            lines.push(VisualLine {
                start: 0,
                end: 0,
                text: String::new(),
            });
        }

        lines
    }

    fn cursor_visual_line_index(&self, lines: &[VisualLine]) -> usize {
        lines
            .iter()
            .enumerate()
            .filter_map(|(index, line)| {
                if self.cursor >= line.start && self.cursor <= line.end {
                    Some(index)
                } else {
                    None
                }
            })
            .last()
            .unwrap_or_else(|| lines.len().saturating_sub(1))
    }

    fn cursor_visual_column(&self, lines: &[VisualLine], line_index: usize) -> usize {
        if let Some(preferred) = self.preferred_column {
            return preferred;
        }
        let line = &lines[line_index];
        display_width(&self.text[line.start..self.cursor])
    }
}

fn render_line(buf: &mut Buffer, area: Rect, row: u16, line: &Line<'static>) {
    if row >= area.height {
        return;
    }
    let mut x = area.x;
    for span in &line.spans {
        for grapheme in span.content.graphemes(true) {
            if x >= area.right() {
                return;
            }
            buf[(x, area.y + row)]
                .set_symbol(grapheme)
                .set_style(span.style);
            let width = grapheme.width().max(1) as u16;
            x = x.saturating_add(width);
        }
    }
}

fn push_wrapped_segment(
    text: &str,
    start: usize,
    end: usize,
    width: usize,
    lines: &mut Vec<VisualLine>,
) {
    if start == end {
        lines.push(VisualLine {
            start,
            end,
            text: String::new(),
        });
        return;
    }

    let segment = &text[start..end];
    let mut line_start = start;
    let mut current_width = 0usize;

    for (offset, grapheme) in segment.grapheme_indices(true) {
        let grapheme_width = grapheme.width().max(1);
        if current_width > 0 && current_width + grapheme_width > width {
            let line_end = start + offset;
            lines.push(VisualLine {
                start: line_start,
                end: line_end,
                text: text[line_start..line_end].to_string(),
            });
            line_start = line_end;
            current_width = 0;
        }
        current_width += grapheme_width;
    }

    lines.push(VisualLine {
        start: line_start,
        end,
        text: text[line_start..end].to_string(),
    });
}

fn previous_grapheme_boundary(text: &str, index: usize) -> usize {
    UnicodeSegmentation::grapheme_indices(text, true)
        .take_while(|(offset, _)| *offset < index)
        .map(|(offset, _)| offset)
        .last()
        .unwrap_or(0)
}

fn next_grapheme_boundary(text: &str, index: usize) -> usize {
    UnicodeSegmentation::grapheme_indices(text, true)
        .find_map(|(offset, grapheme)| {
            if offset > index {
                Some(offset)
            } else if offset == index {
                Some(offset + grapheme.len())
            } else {
                None
            }
        })
        .unwrap_or_else(|| text.len())
}

fn cursor_for_visual_column(line: &VisualLine, target_column: usize) -> usize {
    let mut width = 0usize;
    let mut cursor = line.start;
    for (offset, grapheme) in line.text.grapheme_indices(true) {
        let grapheme_width = grapheme.width().max(1);
        if width + grapheme_width > target_column {
            break;
        }
        width += grapheme_width;
        cursor = line.start + offset + grapheme.len();
    }
    cursor
}

fn effective_scroll(
    cursor_line: usize,
    total_lines: usize,
    visible_rows: usize,
    current_scroll: usize,
) -> usize {
    if visible_rows == 0 || total_lines <= visible_rows {
        return 0;
    }

    let max_scroll = total_lines.saturating_sub(visible_rows);
    let mut scroll = current_scroll.min(max_scroll);
    if cursor_line < scroll {
        scroll = cursor_line;
    } else if cursor_line >= scroll + visible_rows {
        scroll = cursor_line + 1 - visible_rows;
    }
    scroll
}

fn display_width(text: &str) -> usize {
    text.chars()
        .map(|ch| UnicodeWidthChar::width(ch).unwrap_or(0).max(1))
        .sum()
}

fn composer_base_style() -> Style {
    Style::default().fg(Color::White).bg(Color::Rgb(28, 31, 38))
}

#[cfg(test)]
mod tests {
    use super::TextArea;
    use super::TextAreaState;
    use ratatui::buffer::Buffer;
    use ratatui::layout::Rect;

    #[test]
    fn paste_keeps_multiline_content_editable() {
        let mut area = TextArea::new();
        area.insert_text("hello\nworld");
        assert_eq!(area.text(), "hello\nworld");
        area.backspace();
        assert_eq!(area.text(), "hello\nworl");
    }

    #[test]
    fn cursor_moves_across_wrapped_lines() {
        let mut area = TextArea::from_text("abcdefghij".to_string());
        area.move_left();
        area.move_up(6);
        assert!(area.cursor < area.text.len());
        area.move_down(6);
        assert!(area.cursor <= area.text.len());
    }

    #[test]
    fn render_scrolls_to_keep_cursor_visible() {
        let area_rect = Rect::new(0, 0, 8, 2);
        let area = TextArea::from_text("hello world from agile agent".to_string());
        let mut state = TextAreaState::default();
        let mut buffer = Buffer::empty(area_rect);

        area.render(area_rect, &mut buffer, &mut state, "placeholder");

        assert!(state.scroll_row > 0);
    }
}
