use ratatui::text::Line;
use std::path::Path;
use std::path::PathBuf;

use crate::markdown;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct MarkdownStreamCollector {
    buffer: String,
    committed_line_count: usize,
    width: Option<usize>,
    cwd: PathBuf,
}

impl MarkdownStreamCollector {
    pub(crate) fn new(width: Option<usize>, cwd: &Path) -> Self {
        Self {
            buffer: String::new(),
            committed_line_count: 0,
            width,
            cwd: cwd.to_path_buf(),
        }
    }

    pub(crate) fn clear(&mut self) {
        self.buffer.clear();
        self.committed_line_count = 0;
    }

    pub(crate) fn push_delta(&mut self, delta: &str) {
        self.buffer.push_str(delta);
    }

    pub(crate) fn commit_complete_lines(&mut self) -> Vec<Line<'static>> {
        let source = self.buffer.clone();
        let Some(last_newline_idx) = source.rfind('\n') else {
            return Vec::new();
        };
        let source = source[..=last_newline_idx].to_string();
        let mut rendered = Vec::new();
        markdown::append_markdown(&source, self.width, Some(self.cwd.as_path()), &mut rendered);
        let mut complete_line_count = rendered.len();
        if complete_line_count > 0
            && rendered[complete_line_count - 1]
                .spans
                .iter()
                .all(|span| span.content.trim().is_empty())
        {
            complete_line_count -= 1;
        }
        if self.committed_line_count >= complete_line_count {
            return Vec::new();
        }
        let out = rendered[self.committed_line_count..complete_line_count].to_vec();
        self.committed_line_count = complete_line_count;
        out
    }

    pub(crate) fn finalize_and_drain(&mut self) -> Vec<Line<'static>> {
        let mut source = self.buffer.clone();
        if !source.ends_with('\n') {
            source.push('\n');
        }
        let mut rendered = Vec::new();
        markdown::append_markdown(&source, self.width, Some(self.cwd.as_path()), &mut rendered);
        let out = if self.committed_line_count >= rendered.len() {
            Vec::new()
        } else {
            rendered[self.committed_line_count..].to_vec()
        };
        self.clear();
        out
    }
}

#[cfg(test)]
mod tests {
    use super::MarkdownStreamCollector;

    fn lines_to_strings(lines: &[ratatui::text::Line<'static>]) -> Vec<String> {
        lines.iter()
            .map(|line| {
                line.spans
                    .iter()
                    .map(|span| span.content.as_ref())
                    .collect::<String>()
            })
            .collect()
    }

    #[test]
    fn does_not_commit_until_newline() {
        let mut collector = MarkdownStreamCollector::new(None, &std::env::temp_dir());
        collector.push_delta("Hello, world");
        assert!(collector.commit_complete_lines().is_empty());

        collector.push_delta("!\n");
        let rendered = lines_to_strings(&collector.commit_complete_lines());
        assert_eq!(rendered, vec!["Hello, world!".to_string()]);
    }

    #[test]
    fn finalize_commits_partial_line() {
        let mut collector = MarkdownStreamCollector::new(None, &std::env::temp_dir());
        collector.push_delta("Line without newline");
        let rendered = lines_to_strings(&collector.finalize_and_drain());
        assert_eq!(rendered, vec!["Line without newline".to_string()]);
    }

    #[test]
    fn heading_split_across_chunks_starts_on_new_line() {
        let mut collector = MarkdownStreamCollector::new(None, &std::env::temp_dir());
        collector.push_delta("Hello.\n");
        let first = lines_to_strings(&collector.commit_complete_lines());
        assert_eq!(first, vec!["Hello.".to_string()]);

        collector.push_delta("## Heading");
        assert!(collector.commit_complete_lines().is_empty());

        collector.push_delta("\n");
        let second = lines_to_strings(&collector.commit_complete_lines());
        assert_eq!(second, vec!["".to_string(), "## Heading".to_string()]);
    }
}
