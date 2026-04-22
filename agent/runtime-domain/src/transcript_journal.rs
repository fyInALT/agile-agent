//! TranscriptJournal — dedicated type for transcript management.
//!
//! Extracted from Worker to allow independent testing and future
//! optimization (e.g., ring buffers, lazy context generation).

use agent_events::DomainEvent;

/// A structured entry in the transcript journal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JournalEntry {
    /// User-provided input text
    UserInput { text: String },
    /// Assistant response (accumulated chunks)
    AssistantResponse { text: String },
    /// Tool call with name, input preview, and result
    ToolCall {
        name: String,
        input: Option<String>,
        success: bool,
        output: Option<String>,
    },
    /// System event (lifecycle, errors, session handles, etc.)
    SystemEvent { event: DomainEvent },
}

/// TranscriptJournal — records events in a structured, queryable format.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TranscriptJournal {
    entries: Vec<JournalEntry>,
    next_seq: u64,
}

impl Default for TranscriptJournal {
    fn default() -> Self {
        Self::new()
    }
}

impl TranscriptJournal {
    /// Create an empty journal.
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            next_seq: 0,
        }
    }

    /// Append a domain event to the journal.
    ///
    /// Events are classified into structured entry types:
    /// - Streaming chunks → AssistantResponse (merged) or SystemEvent
    /// - Tool calls → ToolCall (started/finished paired)
    /// - User input → UserInput
    /// - Everything else → SystemEvent
    pub fn append(&mut self, event: DomainEvent) {
        let entry = match &event {
            DomainEvent::AssistantChunk(text) => {
                // Merge consecutive assistant chunks
                if let Some(JournalEntry::AssistantResponse { text: prev }) = self.entries.last_mut()
                {
                    prev.push_str(text);
                    self.next_seq += 1;
                    return;
                }
                JournalEntry::AssistantResponse { text: text.clone() }
            }
            DomainEvent::ThinkingChunk(text) => {
                // Thinking chunks are recorded as system events
                JournalEntry::SystemEvent { event }
            }
            DomainEvent::ExecCommandStarted { input_preview, .. } => {
                JournalEntry::ToolCall {
                    name: "exec".to_string(),
                    input: input_preview.clone(),
                    success: false,
                    output: None,
                }
            }
            DomainEvent::ExecCommandFinished {
                output_preview,
                status,
                ..
            } => {
                // Update last tool call if it's an exec
                if let Some(JournalEntry::ToolCall { name, success, output, .. }) =
                    self.entries.last_mut()
                {
                    if name == "exec" && !*success {
                        *success = matches!(status, agent_events::ExecCommandStatus::Completed);
                        *output = output_preview.clone();
                        self.next_seq += 1;
                        return;
                    }
                }
                JournalEntry::ToolCall {
                    name: "exec".to_string(),
                    input: None,
                    success: matches!(status, agent_events::ExecCommandStatus::Completed),
                    output: output_preview.clone(),
                }
            }
            DomainEvent::GenericToolCallStarted { name, input_preview, .. } => {
                JournalEntry::ToolCall {
                    name: name.clone(),
                    input: input_preview.clone(),
                    success: false,
                    output: None,
                }
            }
            DomainEvent::GenericToolCallFinished {
                name,
                output_preview,
                success,
                ..
            } => {
                if let Some(JournalEntry::ToolCall {
                    name: prev_name,
                    success: prev_success,
                    output,
                    ..
                }) = self.entries.last_mut()
                {
                    if prev_name == name && !*prev_success {
                        *prev_success = *success;
                        *output = output_preview.clone();
                        self.next_seq += 1;
                        return;
                    }
                }
                JournalEntry::ToolCall {
                    name: name.clone(),
                    input: None,
                    success: *success,
                    output: output_preview.clone(),
                }
            }
            DomainEvent::McpToolCallStarted { .. } => JournalEntry::ToolCall {
                name: "mcp".to_string(),
                input: None,
                success: false,
                output: None,
            },
            DomainEvent::McpToolCallFinished { error, .. } => {
                if let Some(JournalEntry::ToolCall { name, success, output, .. }) =
                    self.entries.last_mut()
                {
                    if name == "mcp" && !*success {
                        *success = error.is_none();
                        *output = error.clone();
                        self.next_seq += 1;
                        return;
                    }
                }
                JournalEntry::ToolCall {
                    name: "mcp".to_string(),
                    input: None,
                    success: error.is_none(),
                    output: error.clone(),
                }
            }
            DomainEvent::PatchApplyStarted { .. } => JournalEntry::ToolCall {
                name: "patch".to_string(),
                input: None,
                success: false,
                output: None,
            },
            DomainEvent::PatchApplyFinished { status, .. } => {
                if let Some(JournalEntry::ToolCall { name, success, .. }) =
                    self.entries.last_mut()
                {
                    if name == "patch" && !*success {
                        *success = matches!(status, agent_events::PatchApplyStatus::Completed);
                        self.next_seq += 1;
                        return;
                    }
                }
                JournalEntry::ToolCall {
                    name: "patch".to_string(),
                    input: None,
                    success: matches!(status, agent_events::PatchApplyStatus::Completed),
                    output: None,
                }
            }
            DomainEvent::WebSearchStarted { query, .. } => JournalEntry::ToolCall {
                name: "websearch".to_string(),
                input: Some(query.clone()),
                success: false,
                output: None,
            },
            DomainEvent::WebSearchFinished { .. } => {
                if let Some(JournalEntry::ToolCall { name, success, .. }) =
                    self.entries.last_mut()
                {
                    if name == "websearch" && !*success {
                        *success = true;
                        self.next_seq += 1;
                        return;
                    }
                }
                JournalEntry::ToolCall {
                    name: "websearch".to_string(),
                    input: None,
                    success: true,
                    output: None,
                }
            }
            _ => JournalEntry::SystemEvent { event },
        };

        self.entries.push(entry);
        self.next_seq += 1;
    }

    /// Get all entries
    pub fn entries(&self) -> &[JournalEntry] {
        &self.entries
    }

    /// Get the last n entries (chronological order)
    pub fn last_n(&self, n: usize) -> &[JournalEntry] {
        let start = self.entries.len().saturating_sub(n);
        &self.entries[start..]
    }

    /// Get all tool call entries
    pub fn tool_calls(&self) -> Vec<&JournalEntry> {
        self.entries
            .iter()
            .filter(|e| matches!(e, JournalEntry::ToolCall { .. }))
            .collect()
    }

    /// Get the number of entries
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Check if the journal is empty
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Generate a text summary suitable for decision layer context.
    ///
    /// This is a lightweight formatting — the full DecisionContext
    /// construction happens in the decision layer (Sprint 4).
    pub fn to_text_summary(&self, max_entries: usize) -> String {
        let entries = self.last_n(max_entries);
        let mut lines = Vec::new();
        for entry in entries {
            match entry {
                JournalEntry::UserInput { text } => {
                    lines.push(format!("[user] {}", text));
                }
                JournalEntry::AssistantResponse { text } => {
                    lines.push(format!("[assistant] {}", text));
                }
                JournalEntry::ToolCall {
                    name,
                    input,
                    success,
                    output,
                } => {
                    let status = if *success { "ok" } else { "fail" };
                    let detail = input
                        .as_ref()
                        .or(output.as_ref())
                        .map(|s| format!(" — {}", s))
                        .unwrap_or_default();
                    lines.push(format!("[tool:{}] {}{}", status, name, detail));
                }
                JournalEntry::SystemEvent { event } => {
                    lines.push(format!("[system] {:?}", event));
                }
            }
        }
        lines.join("\n")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use agent_events::{
        DomainEvent, ExecCommandStatus, McpInvocation, McpToolCallStatus, PatchApplyStatus,
        PatchChange, PatchChangeKind,
    };

    // ── Basic append ────────────────────────────────────────────

    #[test]
    fn append_status_creates_system_event() {
        let mut journal = TranscriptJournal::new();
        journal.append(DomainEvent::Status("working".to_string()));
        assert_eq!(journal.len(), 1);
        assert!(matches!(journal.entries()[0], JournalEntry::SystemEvent { .. }));
    }

    #[test]
    fn append_assistant_chunk_creates_response() {
        let mut journal = TranscriptJournal::new();
        journal.append(DomainEvent::AssistantChunk("hello".to_string()));
        assert_eq!(journal.len(), 1);
        assert!(
            matches!(journal.entries()[0], JournalEntry::AssistantResponse { ref text } if text == "hello")
        );
    }

    #[test]
    fn consecutive_assistant_chunks_merge() {
        let mut journal = TranscriptJournal::new();
        journal.append(DomainEvent::AssistantChunk("hello ".to_string()));
        journal.append(DomainEvent::AssistantChunk("world".to_string()));
        assert_eq!(journal.len(), 1);
        assert!(
            matches!(journal.entries()[0], JournalEntry::AssistantResponse { ref text } if text == "hello world")
        );
    }

    // ── Tool call pairing ───────────────────────────────────────

    #[test]
    fn exec_command_started_and_finished_pair() {
        let mut journal = TranscriptJournal::new();
        journal.append(DomainEvent::ExecCommandStarted {
            call_id: None,
            input_preview: Some("ls -la".to_string()),
            source: None,
        });
        journal.append(DomainEvent::ExecCommandFinished {
            call_id: None,
            output_preview: Some("file1 file2".to_string()),
            status: ExecCommandStatus::Completed,
            exit_code: Some(0),
            duration_ms: Some(100),
            source: None,
        });
        assert_eq!(journal.len(), 1);
        assert!(matches!(
            journal.entries()[0],
            JournalEntry::ToolCall {
                name: ref n,
                input: Some(ref i),
                success: true,
                output: Some(ref o),
            } if n == "exec" && i == "ls -la" && o == "file1 file2"
        ));
    }

    #[test]
    fn generic_tool_call_pair() {
        let mut journal = TranscriptJournal::new();
        journal.append(DomainEvent::GenericToolCallStarted {
            name: "read_file".to_string(),
            call_id: None,
            input_preview: Some("/path".to_string()),
        });
        journal.append(DomainEvent::GenericToolCallFinished {
            name: "read_file".to_string(),
            call_id: None,
            output_preview: Some("content".to_string()),
            success: true,
            exit_code: None,
            duration_ms: None,
        });
        assert_eq!(journal.len(), 1);
        assert!(matches!(
            journal.entries()[0],
            JournalEntry::ToolCall { name: ref n, success: true, .. } if n == "read_file"
        ));
    }

    #[test]
    fn mcp_tool_call_pair() {
        let mut journal = TranscriptJournal::new();
        journal.append(DomainEvent::McpToolCallStarted {
            call_id: None,
            invocation: McpInvocation {
                server: "s".to_string(),
                tool: "t".to_string(),
                arguments: None,
            },
        });
        journal.append(DomainEvent::McpToolCallFinished {
            call_id: None,
            invocation: McpInvocation {
                server: "s".to_string(),
                tool: "t".to_string(),
                arguments: None,
            },
            result_blocks: vec![],
            error: None,
            status: McpToolCallStatus::Completed,
            is_error: false,
        });
        assert_eq!(journal.len(), 1);
        assert!(matches!(
            journal.entries()[0],
            JournalEntry::ToolCall { name: ref n, success: true, .. } if n == "mcp"
        ));
    }

    #[test]
    fn patch_apply_pair() {
        let mut journal = TranscriptJournal::new();
        journal.append(DomainEvent::PatchApplyStarted {
            call_id: None,
            changes: vec![PatchChange {
                path: "/f.rs".to_string(),
                move_path: None,
                kind: PatchChangeKind::Add,
                diff: "".to_string(),
                added: 1,
                removed: 0,
            }],
        });
        journal.append(DomainEvent::PatchApplyFinished {
            call_id: None,
            changes: vec![],
            status: PatchApplyStatus::Completed,
        });
        assert_eq!(journal.len(), 1);
        assert!(matches!(
            journal.entries()[0],
            JournalEntry::ToolCall { name: ref n, success: true, .. } if n == "patch"
        ));
    }

    #[test]
    fn websearch_pair() {
        let mut journal = TranscriptJournal::new();
        journal.append(DomainEvent::WebSearchStarted {
            call_id: None,
            query: "rust".to_string(),
        });
        journal.append(DomainEvent::WebSearchFinished {
            call_id: None,
            query: "rust".to_string(),
            action: None,
        });
        assert_eq!(journal.len(), 1);
        assert!(matches!(
            journal.entries()[0],
            JournalEntry::ToolCall { name: ref n, input: Some(ref q), success: true, .. }
            if n == "websearch" && q == "rust"
        ));
    }

    // ── Query methods ───────────────────────────────────────────

    #[test]
    fn last_n_returns_last_entries() {
        let mut journal = TranscriptJournal::new();
        for i in 0..5 {
            journal.append(DomainEvent::Status(format!("{}", i)));
        }
        let last_2 = journal.last_n(2);
        assert_eq!(last_2.len(), 2);
    }

    #[test]
    fn tool_calls_filters_correctly() {
        let mut journal = TranscriptJournal::new();
        journal.append(DomainEvent::Status("ok".to_string()));
        journal.append(DomainEvent::ExecCommandStarted {
            call_id: None,
            input_preview: None,
            source: None,
        });
        journal.append(DomainEvent::ExecCommandFinished {
            call_id: None,
            output_preview: None,
            status: ExecCommandStatus::Completed,
            exit_code: None,
            duration_ms: None,
            source: None,
        });
        let calls = journal.tool_calls();
        assert_eq!(calls.len(), 1);
    }

    // ── Text summary ────────────────────────────────────────────

    #[test]
    fn text_summary_includes_entries() {
        let mut journal = TranscriptJournal::new();
        journal.append(DomainEvent::AssistantChunk("hello".to_string()));
        journal.append(DomainEvent::ExecCommandStarted {
            call_id: None,
            input_preview: Some("ls".to_string()),
            source: None,
        });
        journal.append(DomainEvent::ExecCommandFinished {
            call_id: None,
            output_preview: None,
            status: ExecCommandStatus::Completed,
            exit_code: None,
            duration_ms: None,
            source: None,
        });

        let summary = journal.to_text_summary(10);
        assert!(summary.contains("[assistant]"));
        assert!(summary.contains("[tool:ok] exec"));
    }

    #[test]
    fn text_summary_limits_entries() {
        let mut journal = TranscriptJournal::new();
        for i in 0..10 {
            journal.append(DomainEvent::Status(format!("event-{}", i)));
        }
        let summary = journal.to_text_summary(3);
        let lines: Vec<_> = summary.lines().collect();
        assert_eq!(lines.len(), 3);
    }
}
