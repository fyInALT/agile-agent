use agent_core::app::AppState;
use agent_core::app::AppStatus;
use agent_core::app::LoopPhase;
use agent_core::app::TranscriptEntry;
use agent_core::logging;
use agent_core::runtime_session::RuntimeSession;
use agent_core::tool_calls::ExecCommandStatus;
use agent_core::tool_calls::McpInvocation;
use agent_core::tool_calls::McpToolCallStatus;
use agent_core::tool_calls::PatchApplyStatus;
use agent_core::tool_calls::PatchChange;
use agent_core::tool_calls::WebSearchAction;
use anyhow::Result;
use std::collections::VecDeque;
use std::time::Instant;

use crate::composer::textarea::TextArea;
use crate::markdown_stream::MarkdownStreamCollector;
use crate::streaming::AdaptiveChunkingPolicy;
use crate::streaming::QueueSnapshot;
use crate::composer::textarea::TextAreaState;
use crate::transcript::cells;
use crate::transcript::overlay::TranscriptOverlayState;

#[derive(Debug)]
pub struct TuiState {
    pub session: RuntimeSession,
    pub active_cell: Option<ActiveCell>,
    pub active_entries_revision: u64,
    pub composer: TextArea,
    pub composer_state: TextAreaState,
    pub transcript_overlay: Option<TranscriptOverlayState>,
    pub composer_width: u16,
    pub transcript_viewport_height: u16,
    pub transcript_render_width: Option<usize>,
    pub transcript_scroll_offset: usize,
    pub transcript_max_scroll: usize,
    pub transcript_follow_tail: bool,
    pub transcript_rendered_lines: Vec<String>,
    pub transcript_last_cell_range: Option<(usize, usize)>,
    pub busy_started_at: Option<Instant>,
}

impl TuiState {
    pub fn from_session(session: RuntimeSession) -> Self {
        let composer = TextArea::from_text(session.app.input.clone());
        Self {
            session,
            active_cell: None,
            active_entries_revision: 0,
            composer,
            composer_state: TextAreaState::default(),
            transcript_overlay: None,
            composer_width: 80,
            transcript_viewport_height: 1,
            transcript_render_width: None,
            transcript_scroll_offset: 0,
            transcript_max_scroll: 0,
            transcript_follow_tail: true,
            transcript_rendered_lines: Vec::new(),
            transcript_last_cell_range: None,
            busy_started_at: None,
        }
    }

    pub fn app(&self) -> &AppState {
        &self.session.app
    }

    pub fn app_mut(&mut self) -> &mut AppState {
        &mut self.session.app
    }

    fn active_tool_ref(&self) -> Option<&ActiveTool> {
        match self.active_cell.as_ref() {
            Some(ActiveCell::Tool(tool)) => Some(tool),
            _ => None,
        }
    }

    fn active_tool_mut(&mut self) -> Option<&mut ActiveTool> {
        match self.active_cell.as_mut() {
            Some(ActiveCell::Tool(tool)) => Some(tool),
            _ => None,
        }
    }

    fn take_active_tool(&mut self) -> Option<ActiveTool> {
        match self.active_cell.take() {
            Some(ActiveCell::Tool(tool)) => Some(tool),
            Some(cell) => {
                self.active_cell = Some(cell);
                None
            }
            None => None,
        }
    }

    pub(crate) fn set_active_tool(&mut self, tool: ActiveTool) {
        self.active_cell = Some(ActiveCell::Tool(tool));
    }

    fn active_stream_ref(&self) -> Option<&ActiveStream> {
        match self.active_cell.as_ref() {
            Some(ActiveCell::Stream(stream)) => Some(stream),
            _ => None,
        }
    }

    fn active_stream_mut(&mut self) -> Option<&mut ActiveStream> {
        match self.active_cell.as_mut() {
            Some(ActiveCell::Stream(stream)) => Some(stream),
            _ => None,
        }
    }

    fn take_active_stream(&mut self) -> Option<ActiveStream> {
        match self.active_cell.take() {
            Some(ActiveCell::Stream(stream)) => Some(stream),
            Some(cell) => {
                self.active_cell = Some(cell);
                None
            }
            None => None,
        }
    }

    pub(crate) fn set_active_stream(&mut self, stream: ActiveStream) {
        self.active_cell = Some(ActiveCell::Stream(stream));
    }

    pub fn active_entries_for_display(&self) -> Vec<TranscriptEntry> {
        self.active_cell.as_ref().map(ActiveCell::as_transcript_entries).unwrap_or_default()
    }

    #[cfg(test)]
    pub fn overlay_entries_for_display(&self) -> Vec<TranscriptEntry> {
        let mut entries = self.session.app.transcript.clone();
        entries.extend(self.active_entries_for_display());
        entries
    }

    #[cfg(test)]
    pub fn set_active_entry_for_test(&mut self, entry: TranscriptEntry) {
        self.active_cell = match entry {
            TranscriptEntry::ExecCommand {
                call_id,
                source,
                allow_exploring_group,
                input_preview,
                output_preview,
                status,
                exit_code,
                duration_ms,
            } => Some(ActiveCell::Tool(ActiveTool::Exec(vec![ActiveExecCall {
                call_id,
                source,
                allow_exploring_group,
                input_preview,
                output_preview,
                status,
                exit_code,
                duration_ms,
            }]))),
            TranscriptEntry::GenericToolCall {
                name,
                call_id,
                input_preview,
                output_preview,
                success,
                started,
                exit_code,
                duration_ms,
            } => Some(ActiveCell::Tool(ActiveTool::Generic(ActiveGenericToolCall {
                name,
                call_id,
                input_preview,
                output_preview,
                success,
                started,
                exit_code,
                duration_ms,
            }))),
            TranscriptEntry::PatchApply {
                call_id,
                changes,
                status,
                output_preview,
            } => Some(ActiveCell::Tool(ActiveTool::Patch(ActivePatchApply {
                call_id,
                changes,
                status,
                output_preview,
            }))),
            TranscriptEntry::WebSearch {
                call_id,
                query,
                action,
                started,
            } => Some(ActiveCell::Tool(ActiveTool::WebSearch(ActiveWebSearch {
                call_id,
                query,
                action,
                started,
            }))),
            TranscriptEntry::McpToolCall {
                call_id,
                invocation,
                result_blocks,
                error,
                status,
                is_error,
            } => Some(ActiveCell::Tool(ActiveTool::Mcp(ActiveMcpToolCall {
                call_id,
                invocation,
                result_blocks,
                error,
                status,
                is_error,
            }))),
            TranscriptEntry::Assistant(text) => {
                self.set_active_stream(ActiveStream {
                    kind: StreamTextKind::Assistant,
                    tail: text,
                    pending_commits: VecDeque::new(),
                    collector: MarkdownStreamCollector::new(
                        self.transcript_render_width,
                        self.app().cwd.as_path(),
                    ),
                    policy: AdaptiveChunkingPolicy::default(),
                });
                self.bump_active_entries_revision();
                return;
            }
            TranscriptEntry::Thinking(text) => {
                self.set_active_stream(ActiveStream {
                    kind: StreamTextKind::Thinking,
                    tail: text,
                    pending_commits: VecDeque::new(),
                    collector: MarkdownStreamCollector::new(
                        self.transcript_render_width,
                        self.app().cwd.as_path(),
                    ),
                    policy: AdaptiveChunkingPolicy::default(),
                });
                self.bump_active_entries_revision();
                return;
            }
            other => panic!("unsupported active test entry: {other:?}"),
        };
        self.bump_active_entries_revision();
    }

    pub fn active_cell_transcript_key(&self) -> Option<ActiveCellTranscriptKey> {
        self.active_cell.as_ref().map(|_| ActiveCellTranscriptKey {
            revision: self.active_entries_revision,
            is_stream_continuation: self.live_tail_is_stream_continuation(),
        })
    }

    pub fn active_cell_transcript_lines(&self, width: u16) -> Option<Vec<ratatui::text::Line<'static>>> {
        let entries = self.active_entries_for_display();
        let lines = cells::flatten_cells(&cells::build_overlay_cells(&entries, width));
        (!lines.is_empty()).then_some(lines)
    }

    pub fn active_cell_preview_cells(&self, width: u16) -> Vec<cells::TranscriptCell> {
        cells::build_live_tail_cells(&self.active_entries_for_display(), width)
    }

    #[cfg(test)]
    pub(crate) fn active_tool_is_empty(&self) -> bool {
        self.active_tool_ref().is_none()
    }

    #[cfg(test)]
    fn active_tool_entries_len(&self) -> usize {
        self.active_tool_ref()
            .map(|tool| tool.as_transcript_entries().len())
            .unwrap_or(0)
    }

    #[cfg(test)]
    pub(crate) fn active_stream_for_test(&self) -> Option<&ActiveStream> {
        self.active_stream_ref()
    }

    pub fn live_tail_is_stream_continuation(&self) -> bool {
        matches!(
            (
                self.app().transcript.last(),
                self.active_stream_ref().map(|stream| stream.kind),
            ),
            (Some(TranscriptEntry::Assistant(_)), Some(StreamTextKind::Assistant))
                | (Some(TranscriptEntry::Thinking(_)), Some(StreamTextKind::Thinking))
        )
    }

    pub fn has_pending_active_stream_commits(&self) -> bool {
        self.active_stream_ref()
            .is_some_and(|stream| !stream.pending_commits.is_empty())
    }

    fn bump_active_entries_revision(&mut self) {
        self.active_entries_revision = self.active_entries_revision.wrapping_add(1);
    }

    pub fn append_active_assistant_chunk(&mut self, chunk: &str) {
        self.append_streaming_text_chunk(StreamTextKind::Assistant, chunk);
    }

    pub fn append_active_thinking_chunk(&mut self, chunk: &str) {
        if chunk.is_empty() {
            return;
        }

        self.append_streaming_text_chunk(StreamTextKind::Thinking, chunk);
    }

    fn append_streaming_text_chunk(&mut self, kind: StreamTextKind, chunk: &str) {
        if chunk.is_empty() {
            return;
        }
        if matches!(self.active_cell, Some(ActiveCell::Tool(_))) {
            self.flush_active_entries_to_transcript();
        }

        let mut committed = None;
        let stream = self.ensure_active_stream(kind);
        stream.collector.push_delta(chunk);
        stream.tail.push_str(chunk);
        if let Some(split_index) = stream.tail.rfind('\n').map(|index| index + 1) {
            let remainder = stream.tail.split_off(split_index);
            let finished = std::mem::replace(&mut stream.tail, remainder);
            if !finished.is_empty() {
                committed = Some(finished);
            }
        }

        if let Some(committed) = committed {
            let rendered_lines = stream.collector.commit_complete_lines().len().max(1);
            stream.pending_commits.push_back(QueuedStreamCommit {
                text: committed,
                rendered_lines,
                enqueued_at: Instant::now(),
            });
        }
        self.drop_empty_active_stream();
        self.bump_active_entries_revision();
    }

    fn ensure_active_stream(&mut self, kind: StreamTextKind) -> &mut ActiveStream {
        if self
            .active_stream_ref()
            .is_some_and(|stream| stream.kind != kind && !stream.tail.is_empty())
        {
            if let Some(stream) = self.take_active_stream() {
                self.flush_stream_to_transcript(stream);
            }
        }

        if self.active_stream_ref().is_none() {
            self.set_active_stream(ActiveStream {
                kind,
                tail: String::new(),
                pending_commits: VecDeque::new(),
                collector: MarkdownStreamCollector::new(
                    self.transcript_render_width,
                    self.app().cwd.as_path(),
                ),
                policy: AdaptiveChunkingPolicy::default(),
            });
        }
        let stream = self.active_stream_mut().expect("active stream exists");
        stream.kind = kind;
        stream
    }

    fn drop_empty_active_stream(&mut self) {
        if self
            .active_stream_ref()
            .is_some_and(|stream| stream.tail.is_empty() && stream.pending_commits.is_empty())
        {
            self.active_cell = None;
        }
    }

    pub fn drain_active_stream_commit_tick(&mut self) -> bool {
        let now = Instant::now();
        let next = self.active_stream_mut().and_then(|stream| {
            let snapshot = QueueSnapshot {
                queued_lines: stream
                    .pending_commits
                    .iter()
                    .map(|commit| commit.rendered_lines)
                    .sum(),
                oldest_age: stream.oldest_queued_age(now),
            };
            let decision = stream.policy.decide(snapshot, now);
            let mut remaining = decision.drain_lines;
            if remaining == 0 {
                return None;
            }
            let mut drained = Vec::new();
            while remaining > 0 {
                let Some(commit) = stream.pending_commits.pop_front() else {
                    break;
                };
                remaining = remaining.saturating_sub(commit.rendered_lines);
                drained.push(commit.text);
            }
            if drained.is_empty() {
                return None;
            }
            Some((stream.kind, drained))
        });
        let Some((kind, drained)) = next else {
            return false;
        };
        for text in drained {
            self.commit_stream_text(kind, &text);
        }
        self.drop_empty_active_stream();
        self.bump_active_entries_revision();
        true
    }

    pub fn push_active_exec_started(
        &mut self,
        call_id: Option<String>,
        input_preview: Option<String>,
        source: Option<String>,
    ) {
        self.prepare_for_active_tool_start(ActiveToolStart::Exec);
        self.flush_active_stream_to_transcript();
        let call = ActiveExecCall {
            call_id,
            source,
            allow_exploring_group: true,
            input_preview,
            output_preview: None,
            status: ExecCommandStatus::InProgress,
            exit_code: None,
            duration_ms: None,
        };
        match self.active_tool_mut() {
            Some(ActiveTool::Exec(group)) => {
                group.retain(|existing| {
                    !(call.call_id.is_some() && existing.call_id == call.call_id)
                });
                group.push(call);
            }
            _ => {
                self.set_active_tool(ActiveTool::Exec(vec![call]));
            }
        }
        self.bump_active_entries_revision();
    }

    pub fn append_active_exec_output(&mut self, call_id: Option<String>, delta: &str) {
        if delta.is_empty() {
            return;
        }

        if let Some(ActiveTool::Exec(group)) = self.active_tool_mut() {
            for entry in group.iter_mut().rev() {
                let matches_call_id = call_id.is_some() && entry.call_id == call_id;
                let matches_latest = call_id.is_none();
                if matches_call_id || matches_latest {
                    entry.output_preview
                        .get_or_insert_with(String::new)
                        .push_str(delta);
                    self.bump_active_entries_revision();
                    return;
                }
            }
        }
    }

    pub fn finish_active_exec(
        &mut self,
        call_id: Option<String>,
        output_preview: Option<String>,
        status: ExecCommandStatus,
        exit_code: Option<i32>,
        duration_ms: Option<u64>,
        source: Option<String>,
    ) {
        if let Some(ActiveTool::Exec(mut group)) = self.take_active_tool()
            && let Some(index) = group.iter().rposition(|entry| {
                call_id.is_some() && entry.call_id == call_id
            })
        {
            let entry = group.remove(index);
            self.session.app.transcript.push(TranscriptEntry::ExecCommand {
                call_id: entry.call_id,
                source: entry.source.or(source),
                allow_exploring_group: entry.allow_exploring_group,
                input_preview: entry.input_preview,
                output_preview: output_preview.or(entry.output_preview),
                status,
                exit_code,
                duration_ms,
            });
            if group.is_empty() {
                self.active_cell = None;
            } else {
                self.set_active_tool(ActiveTool::Exec(group));
            }
            self.bump_active_entries_revision();
            return;
        } else if let Some(tool) = self.take_active_tool() {
            self.set_active_tool(tool);
        }

        self.session.app.push_exec_command_finished(
            call_id,
            output_preview,
            status,
            exit_code,
            duration_ms,
            source,
        );
    }

    pub fn push_active_generic_tool_call_started(
        &mut self,
        name: String,
        call_id: Option<String>,
        input_preview: Option<String>,
    ) {
        self.prepare_for_active_tool_start(ActiveToolStart::Other);
        self.flush_active_stream_to_transcript();
        self.set_active_tool(ActiveTool::Generic(ActiveGenericToolCall {
            name,
            call_id,
            input_preview,
            output_preview: None,
            success: true,
            started: true,
            exit_code: None,
            duration_ms: None,
        }));
        self.bump_active_entries_revision();
    }

    pub fn finish_active_generic_tool_call(
        &mut self,
        name: String,
        call_id: Option<String>,
        output_preview: Option<String>,
        success: bool,
        exit_code: Option<i32>,
        duration_ms: Option<u64>,
    ) {
        if let Some(ActiveTool::Generic(entry)) = self.take_active_tool() {
            let matches_call_id = call_id.is_some() && entry.call_id == call_id;
            let matches_name = entry.name == name;
            if matches_call_id || matches_name {
                self.session.app.transcript.push(TranscriptEntry::GenericToolCall {
                    name: entry.name,
                    call_id: entry.call_id.or(call_id),
                    input_preview: entry.input_preview,
                    output_preview: output_preview.or(entry.output_preview),
                    success,
                    started: false,
                    exit_code,
                    duration_ms,
                });
                self.bump_active_entries_revision();
                return;
            }
            self.set_active_tool(ActiveTool::Generic(entry));
        }

        self.session.app.push_generic_tool_call_finished(
            name,
            call_id,
            output_preview,
            success,
            exit_code,
            duration_ms,
        );
    }

    pub fn push_active_patch_apply_started(
        &mut self,
        call_id: Option<String>,
        changes: Vec<PatchChange>,
    ) {
        self.prepare_for_active_tool_start(ActiveToolStart::Other);
        self.flush_active_stream_to_transcript();
        self.set_active_tool(ActiveTool::Patch(ActivePatchApply {
            call_id,
            changes,
            status: PatchApplyStatus::InProgress,
            output_preview: None,
        }));
        self.bump_active_entries_revision();
    }

    pub fn append_active_patch_apply_output(&mut self, call_id: Option<String>, delta: &str) {
        if delta.is_empty() {
            return;
        }

        if let Some(ActiveTool::Patch(entry)) = self.active_tool_mut() {
            {
                let existing_call_id = &entry.call_id;
                let matches_call_id = call_id.is_some() && existing_call_id == &call_id;
                let matches_latest = call_id.is_none();
                if matches_call_id || matches_latest {
                    entry.output_preview
                        .get_or_insert_with(String::new)
                        .push_str(delta);
                    self.bump_active_entries_revision();
                    return;
                }
            }
        }

        self.session.app.append_patch_apply_output(call_id, delta);
    }

    pub fn finish_active_patch_apply(
        &mut self,
        call_id: Option<String>,
        changes: Vec<PatchChange>,
        status: PatchApplyStatus,
    ) {
        if let Some(ActiveTool::Patch(entry)) = self.take_active_tool() {
            let matches_call_id = call_id.is_some() && entry.call_id == call_id;
            let matches_latest = entry.call_id.is_none();
            if matches_call_id || matches_latest {
                self.session.app.transcript.push(TranscriptEntry::PatchApply {
                    call_id: entry.call_id.or(call_id),
                    changes: if changes.is_empty() {
                        entry.changes
                    } else {
                        changes
                    },
                    status,
                    output_preview: entry.output_preview,
                });
                self.bump_active_entries_revision();
                return;
            }
            self.set_active_tool(ActiveTool::Patch(entry));
        }

        self.session
            .app
            .push_patch_apply_finished(call_id, changes, status);
    }

    pub fn push_active_web_search_started(&mut self, call_id: Option<String>, query: String) {
        self.prepare_for_active_tool_start(ActiveToolStart::Other);
        self.flush_active_stream_to_transcript();
        self.set_active_tool(ActiveTool::WebSearch(ActiveWebSearch {
            call_id,
            query,
            action: None,
            started: true,
        }));
        self.bump_active_entries_revision();
    }

    pub fn finish_active_web_search(
        &mut self,
        call_id: Option<String>,
        query: String,
        action: Option<WebSearchAction>,
    ) {
        if let Some(ActiveTool::WebSearch(entry)) = self.take_active_tool() {
            let matches_call_id = call_id.is_some() && entry.call_id == call_id;
            let matches_latest = entry.call_id.is_none();
            if matches_call_id || matches_latest {
                self.session.app.transcript.push(TranscriptEntry::WebSearch {
                    call_id: entry.call_id.or(call_id),
                    query,
                    action,
                    started: false,
                });
                self.bump_active_entries_revision();
                return;
            }
            self.set_active_tool(ActiveTool::WebSearch(entry));
        }

        self.session
            .app
            .push_web_search_finished(call_id, query, action);
    }

    pub fn push_active_mcp_tool_call_started(
        &mut self,
        call_id: Option<String>,
        invocation: McpInvocation,
    ) {
        self.prepare_for_active_tool_start(ActiveToolStart::Other);
        self.flush_active_stream_to_transcript();
        self.set_active_tool(ActiveTool::Mcp(ActiveMcpToolCall {
            call_id,
            invocation,
            result_blocks: Vec::new(),
            error: None,
            status: McpToolCallStatus::InProgress,
            is_error: false,
        }));
        self.bump_active_entries_revision();
    }

    pub fn finish_active_mcp_tool_call(
        &mut self,
        call_id: Option<String>,
        invocation: McpInvocation,
        result_blocks: Vec<serde_json::Value>,
        error: Option<String>,
        status: McpToolCallStatus,
        is_error: bool,
    ) {
        if let Some(ActiveTool::Mcp(entry)) = self.take_active_tool() {
            let matches_call_id = call_id.is_some() && entry.call_id == call_id;
            let matches_latest = entry.call_id.is_none();
            if matches_call_id || matches_latest {
                self.session.app.transcript.push(TranscriptEntry::McpToolCall {
                    call_id: entry.call_id.or(call_id),
                    invocation,
                    result_blocks,
                    error,
                    status,
                    is_error,
                });
                self.bump_active_entries_revision();
                return;
            }
            self.set_active_tool(ActiveTool::Mcp(entry));
        }

        self.session.app.push_mcp_tool_call_finished(
            call_id,
            invocation,
            result_blocks,
            error,
            status,
            is_error,
        );
    }

    pub fn flush_active_entries_to_transcript(&mut self) {
        self.drain_active_entries(None);
    }

    pub fn finalize_active_entries_after_failure(&mut self, reason: Option<&str>) {
        self.drain_active_entries(reason);
        self.mark_in_progress_transcript_entries_failed(reason);
    }

    pub fn sync_app_input_from_composer(&mut self) {
        self.session.app.input = self.composer.text().to_string();
    }

    pub fn replace_transcript(&mut self, transcript: Vec<TranscriptEntry>) {
        self.session.app.transcript = transcript;
        self.transcript_rendered_lines.clear();
        self.transcript_last_cell_range = None;
        if self.transcript_follow_tail {
            self.transcript_scroll_offset = self.transcript_max_scroll;
        }
    }

    pub fn into_app_state(mut self) -> AppState {
        self.sync_app_input_from_composer();
        self.session.app
    }

    pub fn persist_if_changed(&mut self) -> Result<()> {
        self.session.persist_if_changed()
    }

    pub fn is_overlay_open(&self) -> bool {
        self.transcript_overlay.is_some()
    }

    pub fn open_transcript_overlay(&mut self) {
        if self.transcript_overlay.is_none() {
            self.transcript_overlay = Some(TranscriptOverlayState::pinned_to_bottom());
        }
    }

    pub fn close_transcript_overlay(&mut self) {
        self.transcript_overlay = None;
    }

    pub fn scroll_transcript_up(&mut self, rows: usize) {
        self.transcript_scroll_offset = self.transcript_scroll_offset.saturating_sub(rows);
        self.transcript_follow_tail = false;
    }

    pub fn scroll_transcript_down(&mut self, rows: usize) {
        self.transcript_scroll_offset = self.transcript_scroll_offset.saturating_add(rows);
        if rows > 0 {
            self.transcript_follow_tail =
                self.transcript_scroll_offset >= self.transcript_max_scroll;
        }
    }

    pub fn scroll_transcript_home(&mut self) {
        self.transcript_scroll_offset = 0;
        self.transcript_follow_tail = false;
    }

    pub fn scroll_transcript_end(&mut self) {
        self.transcript_follow_tail = true;
    }

    pub fn sync_busy_started_at(&mut self) {
        if self.is_busy() {
            if self.busy_started_at.is_none() {
                self.busy_started_at = Some(Instant::now());
            }
        } else {
            self.busy_started_at = None;
        }
    }

    pub fn is_busy(&self) -> bool {
        self.session.app.status == AppStatus::Responding
            || !matches!(self.session.app.loop_phase, LoopPhase::Idle)
    }

    pub fn switch_to_new_agent(
        &mut self,
        provider_kind: agent_core::provider::ProviderKind,
    ) -> Result<String> {
        self.sync_app_input_from_composer();
        let summary = self.session.switch_agent(provider_kind)?;
        logging::debug_event(
            "tui.provider_switch",
            "switched to sibling agent from TUI state",
            serde_json::json!({
                "provider": provider_kind.label(),
                "summary": summary,
            }),
        );
        self.composer = TextArea::new();
        self.composer_state = TextAreaState::default();
        self.transcript_overlay = None;
        self.active_cell = None;
        self.bump_active_entries_revision();
        self.transcript_scroll_offset = 0;
        self.transcript_max_scroll = 0;
        self.transcript_follow_tail = true;
        self.transcript_render_width = None;
        self.transcript_rendered_lines.clear();
        self.transcript_last_cell_range = None;
        self.busy_started_at = None;
        Ok(summary)
    }
}

impl TuiState {
    fn drain_active_entries(&mut self, failure_reason: Option<&str>) {
        if self.active_cell.is_none() {
            return;
        }
        if let Some(cell) = self.active_cell.take() {
            match (failure_reason, cell) {
                (_, ActiveCell::Stream(stream)) => self.flush_stream_to_transcript(stream),
                (Some(_), ActiveCell::Tool(ActiveTool::Exec(group))) => {
                    for entry in group {
                        self.session.app.transcript.push(TranscriptEntry::ExecCommand {
                            call_id: entry.call_id,
                            source: entry.source,
                            allow_exploring_group: entry.allow_exploring_group,
                            input_preview: entry.input_preview,
                            output_preview: entry.output_preview,
                            status: ExecCommandStatus::Failed,
                            exit_code: entry.exit_code,
                            duration_ms: entry.duration_ms,
                        });
                    }
                }
                (Some(_), ActiveCell::Tool(ActiveTool::Generic(entry))) => {
                    self.session.app.transcript.push(TranscriptEntry::GenericToolCall {
                        name: entry.name,
                        call_id: entry.call_id,
                        input_preview: entry.input_preview,
                        output_preview: entry.output_preview,
                        success: false,
                        started: false,
                        exit_code: None,
                        duration_ms: None,
                    });
                }
                (Some(_), ActiveCell::Tool(ActiveTool::Patch(entry))) => {
                    self.session.app.transcript.push(TranscriptEntry::PatchApply {
                        call_id: entry.call_id,
                        changes: entry.changes,
                        status: PatchApplyStatus::Failed,
                        output_preview: entry.output_preview,
                    });
                }
                (Some(_), ActiveCell::Tool(ActiveTool::WebSearch(entry))) => {
                    self.session.app.transcript.push(TranscriptEntry::WebSearch {
                        call_id: entry.call_id,
                        query: entry.query,
                        action: entry.action,
                        started: false,
                    });
                }
                (Some(reason), ActiveCell::Tool(ActiveTool::Mcp(entry))) => {
                    self.session.app.transcript.push(TranscriptEntry::McpToolCall {
                        call_id: entry.call_id,
                        invocation: entry.invocation,
                        result_blocks: entry.result_blocks,
                        error: entry.error.or_else(|| Some(reason.to_string())),
                        status: McpToolCallStatus::Failed,
                        is_error: true,
                    });
                }
                (_, ActiveCell::Tool(tool)) => {
                    for entry in tool.as_transcript_entries() {
                        self.session.app.transcript.push(entry);
                    }
                }
            }
        }
        self.bump_active_entries_revision();
    }

    fn commit_stream_text(&mut self, kind: StreamTextKind, text: &str) {
        match kind {
            StreamTextKind::Assistant => self.session.app.append_assistant_chunk(text),
            StreamTextKind::Thinking => self.session.app.append_thinking_chunk(text),
        }
    }

    fn mark_in_progress_transcript_entries_failed(&mut self, reason: Option<&str>) {
        for entry in &mut self.session.app.transcript {
            match entry {
                TranscriptEntry::ExecCommand {
                    status: exec_status, ..
                } if matches!(*exec_status, ExecCommandStatus::InProgress) => {
                    *exec_status = ExecCommandStatus::Failed;
                }
                TranscriptEntry::GenericToolCall {
                    success, started, ..
                } if *started => {
                    *success = false;
                    *started = false;
                }
                TranscriptEntry::PatchApply { status, .. }
                    if matches!(*status, PatchApplyStatus::InProgress) =>
                {
                    *status = PatchApplyStatus::Failed;
                }
                TranscriptEntry::WebSearch { started, .. } if *started => {
                    *started = false;
                }
                TranscriptEntry::McpToolCall {
                    error,
                    status,
                    is_error,
                    ..
                } if matches!(*status, McpToolCallStatus::InProgress) => {
                    *status = McpToolCallStatus::Failed;
                    *is_error = true;
                    if error.is_none() {
                        *error = reason.map(ToOwned::to_owned);
                    }
                }
                _ => {}
            }
        }
    }

    fn flush_stream_to_transcript(&mut self, stream: ActiveStream) {
        let ActiveStream {
            kind,
            tail,
            pending_commits,
            mut collector,
            ..
        } = stream;
        for commit in pending_commits {
            self.commit_stream_text(kind, &commit.text);
        }
        if !tail.is_empty() {
            self.commit_stream_text(kind, &tail);
        }
        let _ = collector.finalize_and_drain();
    }

    fn flush_active_stream_to_transcript(&mut self) {
        if let Some(stream) = self.take_active_stream() {
            self.flush_stream_to_transcript(stream);
            self.bump_active_entries_revision();
        }
    }

    fn prepare_for_active_tool_start(&mut self, start: ActiveToolStart) {
        let should_flush = match start {
            ActiveToolStart::Exec => matches!(self.active_tool_ref(), Some(tool) if !matches!(tool, ActiveTool::Exec(_))),
            ActiveToolStart::Other => self.active_tool_ref().is_some(),
        };
        if should_flush {
            self.flush_active_entries_to_transcript();
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ActiveTool {
    Exec(Vec<ActiveExecCall>),
    Generic(ActiveGenericToolCall),
    Patch(ActivePatchApply),
    WebSearch(ActiveWebSearch),
    Mcp(ActiveMcpToolCall),
}

impl ActiveTool {
    fn as_transcript_entries(&self) -> Vec<TranscriptEntry> {
        match self {
            ActiveTool::Exec(group) => group
                .iter()
                .map(|entry| TranscriptEntry::ExecCommand {
                    call_id: entry.call_id.clone(),
                    source: entry.source.clone(),
                    allow_exploring_group: entry.allow_exploring_group,
                    input_preview: entry.input_preview.clone(),
                    output_preview: entry.output_preview.clone(),
                    status: entry.status,
                    exit_code: entry.exit_code,
                    duration_ms: entry.duration_ms,
                })
                .collect(),
            ActiveTool::Generic(entry) => vec![TranscriptEntry::GenericToolCall {
                name: entry.name.clone(),
                call_id: entry.call_id.clone(),
                input_preview: entry.input_preview.clone(),
                output_preview: entry.output_preview.clone(),
                success: entry.success,
                started: entry.started,
                exit_code: entry.exit_code,
                duration_ms: entry.duration_ms,
            }],
            ActiveTool::Patch(entry) => vec![TranscriptEntry::PatchApply {
                call_id: entry.call_id.clone(),
                changes: entry.changes.clone(),
                status: entry.status,
                output_preview: entry.output_preview.clone(),
            }],
            ActiveTool::WebSearch(entry) => vec![TranscriptEntry::WebSearch {
                call_id: entry.call_id.clone(),
                query: entry.query.clone(),
                action: entry.action.clone(),
                started: entry.started,
            }],
            ActiveTool::Mcp(entry) => vec![TranscriptEntry::McpToolCall {
                call_id: entry.call_id.clone(),
                invocation: entry.invocation.clone(),
                result_blocks: entry.result_blocks.clone(),
                error: entry.error.clone(),
                status: entry.status,
                is_error: entry.is_error,
            }],
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ActiveCell {
    Tool(ActiveTool),
    Stream(ActiveStream),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ActiveCellTranscriptKey {
    pub revision: u64,
    pub is_stream_continuation: bool,
}

impl ActiveCell {
    fn as_transcript_entries(&self) -> Vec<TranscriptEntry> {
        match self {
            ActiveCell::Tool(tool) => tool.as_transcript_entries(),
            ActiveCell::Stream(stream) => vec![stream.as_transcript_entry()],
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActiveExecCall {
    pub(crate) call_id: Option<String>,
    pub(crate) source: Option<String>,
    pub(crate) allow_exploring_group: bool,
    pub(crate) input_preview: Option<String>,
    pub(crate) output_preview: Option<String>,
    pub(crate) status: ExecCommandStatus,
    pub(crate) exit_code: Option<i32>,
    pub(crate) duration_ms: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActiveGenericToolCall {
    pub(crate) name: String,
    pub(crate) call_id: Option<String>,
    pub(crate) input_preview: Option<String>,
    pub(crate) output_preview: Option<String>,
    pub(crate) success: bool,
    pub(crate) started: bool,
    pub(crate) exit_code: Option<i32>,
    pub(crate) duration_ms: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActivePatchApply {
    pub(crate) call_id: Option<String>,
    pub(crate) changes: Vec<PatchChange>,
    pub(crate) status: PatchApplyStatus,
    pub(crate) output_preview: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActiveWebSearch {
    pub(crate) call_id: Option<String>,
    pub(crate) query: String,
    pub(crate) action: Option<WebSearchAction>,
    pub(crate) started: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActiveMcpToolCall {
    pub(crate) call_id: Option<String>,
    pub(crate) invocation: McpInvocation,
    pub(crate) result_blocks: Vec<serde_json::Value>,
    pub(crate) error: Option<String>,
    pub(crate) status: McpToolCallStatus,
    pub(crate) is_error: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActiveStream {
    pub(crate) kind: StreamTextKind,
    pub(crate) tail: String,
    pub(crate) pending_commits: VecDeque<QueuedStreamCommit>,
    pub(crate) collector: MarkdownStreamCollector,
    pub(crate) policy: AdaptiveChunkingPolicy,
}

impl ActiveStream {
    fn as_transcript_entry(&self) -> TranscriptEntry {
        match self.kind {
            StreamTextKind::Assistant => TranscriptEntry::Assistant(self.tail.clone()),
            StreamTextKind::Thinking => TranscriptEntry::Thinking(self.tail.clone()),
        }
    }

    fn oldest_queued_age(&self, now: Instant) -> Option<std::time::Duration> {
        self.pending_commits
            .front()
            .map(|commit| now.saturating_duration_since(commit.enqueued_at))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QueuedStreamCommit {
    pub(crate) text: String,
    pub(crate) rendered_lines: usize,
    pub(crate) enqueued_at: Instant,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum StreamTextKind {
    Assistant,
    Thinking,
}

#[derive(Clone, Copy)]
enum ActiveToolStart {
    Exec,
    Other,
}

#[cfg(test)]
mod tests {
    use super::ActiveCellTranscriptKey;
    use super::ActiveStream;
    use super::StreamTextKind;
    use super::TuiState;
    use agent_core::app::TranscriptEntry;
    use agent_core::provider::ProviderKind;
    use agent_core::runtime_session::RuntimeSession;
    use agent_core::tool_calls::ExecCommandStatus;
    use agent_core::tool_calls::McpInvocation;
    use agent_core::tool_calls::McpToolCallStatus;
    use agent_core::tool_calls::PatchApplyStatus;
    use agent_core::tool_calls::PatchChange;
    use agent_core::tool_calls::PatchChangeKind;
    use agent_core::tool_calls::WebSearchAction;
    use tempfile::TempDir;

    #[test]
    fn switching_provider_creates_new_agent_runtime() {
        let temp = TempDir::new().expect("tempdir");
        let mut session =
            RuntimeSession::bootstrap(temp.path().into(), ProviderKind::Claude, false)
                .expect("bootstrap");
        session.app.push_status_message("existing transcript");

        let mut state = TuiState::from_session(session);
        let previous_agent_id = state.session.agent_runtime.agent_id().as_str().to_string();

        let summary = state
            .switch_to_new_agent(ProviderKind::Codex)
            .expect("switch");

        assert_ne!(
            state.session.agent_runtime.agent_id().as_str(),
            previous_agent_id
        );
        assert_eq!(state.session.app.selected_provider, ProviderKind::Codex);
        assert!(summary.contains("agent_"));
        assert!(matches!(
            state.session.app.transcript.first(),
            Some(TranscriptEntry::Status(text)) if text.contains("created agent:")
        ));
    }

    #[test]
    fn scrolling_down_to_known_tail_restores_follow_mode() {
        let temp = TempDir::new().expect("tempdir");
        let session = RuntimeSession::bootstrap(temp.path().into(), ProviderKind::Claude, false)
            .expect("bootstrap");
        let mut state = TuiState::from_session(session);
        state.transcript_scroll_offset = 6;
        state.transcript_max_scroll = 6;
        state.transcript_follow_tail = false;

        state.scroll_transcript_up(2);
        assert!(!state.transcript_follow_tail);

        state.scroll_transcript_down(2);

        assert_eq!(state.transcript_scroll_offset, 6);
        assert!(state.transcript_follow_tail);
    }

    #[test]
    fn switch_to_new_agent_clears_active_entries() {
        let temp = TempDir::new().expect("tempdir");
        let session = RuntimeSession::bootstrap(temp.path().into(), ProviderKind::Claude, false)
            .expect("bootstrap");
        let mut state = TuiState::from_session(session);
        state.set_active_entry_for_test(TranscriptEntry::ExecCommand {
            call_id: Some("call-1".to_string()),
            source: Some("agent".to_string()),
            allow_exploring_group: true,
            input_preview: Some("printf hello".to_string()),
            output_preview: Some("hello".to_string()),
            status: ExecCommandStatus::InProgress,
            exit_code: None,
            duration_ms: None,
        });

        let _ = state
            .switch_to_new_agent(ProviderKind::Codex)
            .expect("switch");

        assert!(state.active_tool_is_empty());
    }

    #[test]
    fn active_cell_transcript_key_is_none_without_active_cell() {
        let temp = TempDir::new().expect("tempdir");
        let session = RuntimeSession::bootstrap(temp.path().into(), ProviderKind::Claude, false)
            .expect("bootstrap");
        let state = TuiState::from_session(session);

        assert_eq!(state.active_cell_transcript_key(), None);
    }

    #[test]
    fn active_cell_transcript_key_reflects_revision_and_stream_continuation() {
        let temp = TempDir::new().expect("tempdir");
        let session = RuntimeSession::bootstrap(temp.path().into(), ProviderKind::Claude, false)
            .expect("bootstrap");
        let mut state = TuiState::from_session(session);
        state
            .app_mut()
            .transcript
            .push(TranscriptEntry::Assistant("committed".to_string()));
        state.append_active_assistant_chunk("tail");

        assert_eq!(
            state.active_cell_transcript_key(),
            Some(ActiveCellTranscriptKey {
                revision: state.active_entries_revision,
                is_stream_continuation: true,
            })
        );
    }

    #[test]
    fn active_cell_transcript_lines_render_current_live_tail() {
        let temp = TempDir::new().expect("tempdir");
        let session = RuntimeSession::bootstrap(temp.path().into(), ProviderKind::Claude, false)
            .expect("bootstrap");
        let mut state = TuiState::from_session(session);
        state.append_active_assistant_chunk("tail");

        let rendered = state
            .active_cell_transcript_lines(80)
            .expect("active lines")
            .into_iter()
            .map(|line| {
                line.spans
                    .iter()
                    .map(|span| span.content.as_ref())
                    .collect::<String>()
            })
            .collect::<Vec<_>>();

        assert!(rendered.iter().any(|line| line == "tail"));
    }

    #[test]
    fn active_cell_preview_cells_render_active_exec() {
        let temp = TempDir::new().expect("tempdir");
        let session = RuntimeSession::bootstrap(temp.path().into(), ProviderKind::Claude, false)
            .expect("bootstrap");
        let mut state = TuiState::from_session(session);
        state.set_active_entry_for_test(TranscriptEntry::ExecCommand {
            call_id: Some("call-1".to_string()),
            source: Some("agent".to_string()),
            allow_exploring_group: true,
            input_preview: Some("printf hello".to_string()),
            output_preview: Some("hello".to_string()),
            status: ExecCommandStatus::InProgress,
            exit_code: None,
            duration_ms: None,
        });

        let rendered = state
            .active_cell_preview_cells(80)
            .into_iter()
            .flat_map(|cell| cell.lines)
            .map(|line| {
                line.spans
                    .iter()
                    .map(|span| span.content.as_ref())
                    .collect::<String>()
            })
            .collect::<Vec<_>>();

        assert!(rendered.iter().any(|line| line == "• Running printf hello"));
    }

    #[test]
    fn active_exec_lifecycle_commits_completed_entry_to_transcript() {
        let temp = TempDir::new().expect("tempdir");
        let session = RuntimeSession::bootstrap(temp.path().into(), ProviderKind::Claude, false)
            .expect("bootstrap");
        let mut state = TuiState::from_session(session);

        state.push_active_exec_started(
            Some("call-1".to_string()),
            Some("printf hello".to_string()),
            Some("agent".to_string()),
        );
        state.append_active_exec_output(Some("call-1".to_string()), "hello\n");
        state.finish_active_exec(
            Some("call-1".to_string()),
            None,
            agent_core::tool_calls::ExecCommandStatus::Completed,
            Some(0),
            Some(5),
            Some("agent".to_string()),
        );

        assert!(state.active_tool_is_empty());
        assert!(matches!(
            state.app().transcript.last(),
            Some(TranscriptEntry::ExecCommand {
                call_id,
                output_preview,
                status,
                ..
            })
            if call_id.as_deref() == Some("call-1")
                && output_preview.as_deref() == Some("hello\n")
                && *status == ExecCommandStatus::Completed
        ));
    }

    #[test]
    fn active_generic_tool_call_lifecycle_commits_completed_entry_to_transcript() {
        let temp = TempDir::new().expect("tempdir");
        let session = RuntimeSession::bootstrap(temp.path().into(), ProviderKind::Claude, false)
            .expect("bootstrap");
        let mut state = TuiState::from_session(session);

        state.push_active_generic_tool_call_started(
            "shell".to_string(),
            Some("tool-1".to_string()),
            Some("{\"cmd\":\"git status\"}".to_string()),
        );
        state.finish_active_generic_tool_call(
            "shell".to_string(),
            Some("tool-1".to_string()),
            Some("On branch main".to_string()),
            true,
            Some(0),
            Some(20),
        );

        assert!(state.active_tool_is_empty());
        assert!(matches!(
            state.app().transcript.last(),
            Some(TranscriptEntry::GenericToolCall {
                name,
                call_id,
                output_preview,
                success,
                started,
                ..
            })
            if name == "shell"
                && call_id.as_deref() == Some("tool-1")
                && output_preview.as_deref() == Some("On branch main")
                && *success
                && !started
        ));
    }

    #[test]
    fn active_patch_apply_lifecycle_commits_completed_entry_to_transcript() {
        let temp = TempDir::new().expect("tempdir");
        let session = RuntimeSession::bootstrap(temp.path().into(), ProviderKind::Claude, false)
            .expect("bootstrap");
        let mut state = TuiState::from_session(session);
        let changes = vec![PatchChange {
            path: "README.md".to_string(),
            move_path: None,
            kind: PatchChangeKind::Update,
            diff: "@@ -1 +1 @@\n-old\n+new".to_string(),
            added: 1,
            removed: 1,
        }];

        state.push_active_patch_apply_started(Some("patch-1".to_string()), changes.clone());
        state.append_active_patch_apply_output(Some("patch-1".to_string()), "applied");
        state.finish_active_patch_apply(
            Some("patch-1".to_string()),
            changes.clone(),
            PatchApplyStatus::Completed,
        );

        assert!(state.active_tool_is_empty());
        assert!(matches!(
            state.app().transcript.last(),
            Some(TranscriptEntry::PatchApply {
                call_id,
                changes: committed_changes,
                status,
                output_preview,
            })
            if call_id.as_deref() == Some("patch-1")
                && committed_changes == &changes
                && *status == PatchApplyStatus::Completed
                && output_preview.as_deref() == Some("applied")
        ));
    }

    #[test]
    fn active_web_search_lifecycle_commits_completed_entry_to_transcript() {
        let temp = TempDir::new().expect("tempdir");
        let session = RuntimeSession::bootstrap(temp.path().into(), ProviderKind::Claude, false)
            .expect("bootstrap");
        let mut state = TuiState::from_session(session);
        let action = Some(WebSearchAction::OpenPage {
            url: Some("https://example.com".to_string()),
        });

        state.push_active_web_search_started(
            Some("search-1".to_string()),
            "ratatui styling".to_string(),
        );
        state.finish_active_web_search(
            Some("search-1".to_string()),
            "ratatui styling".to_string(),
            action.clone(),
        );

        assert!(state.active_tool_is_empty());
        assert!(matches!(
            state.app().transcript.last(),
            Some(TranscriptEntry::WebSearch {
                call_id,
                query,
                action: committed_action,
                started,
            })
            if call_id.as_deref() == Some("search-1")
                && query == "ratatui styling"
                && committed_action == &action
                && !started
        ));
    }

    #[test]
    fn active_mcp_tool_call_lifecycle_commits_completed_entry_to_transcript() {
        let temp = TempDir::new().expect("tempdir");
        let session = RuntimeSession::bootstrap(temp.path().into(), ProviderKind::Claude, false)
            .expect("bootstrap");
        let mut state = TuiState::from_session(session);
        let invocation = McpInvocation {
            server: "search".to_string(),
            tool: "find_docs".to_string(),
            arguments: Some(serde_json::json!({
                "query": "ratatui styling",
                "limit": 3
            })),
        };
        let result_blocks = vec![serde_json::json!({
            "type": "text",
            "text": "doc-1"
        })];

        state.push_active_mcp_tool_call_started(Some("mcp-1".to_string()), invocation.clone());
        state.finish_active_mcp_tool_call(
            Some("mcp-1".to_string()),
            invocation.clone(),
            result_blocks.clone(),
            None,
            McpToolCallStatus::Completed,
            false,
        );

        assert!(state.active_tool_is_empty());
        assert!(matches!(
            state.app().transcript.last(),
            Some(TranscriptEntry::McpToolCall {
                call_id,
                invocation: committed_invocation,
                result_blocks: committed_result_blocks,
                error,
                status,
                is_error,
            })
            if call_id.as_deref() == Some("mcp-1")
                && committed_invocation == &invocation
                && committed_result_blocks == &result_blocks
                && error.is_none()
                && *status == McpToolCallStatus::Completed
                && !is_error
        ));
    }

    #[test]
    fn transcript_overlay_opens_pinned_to_bottom() {
        let temp = TempDir::new().expect("tempdir");
        let session = RuntimeSession::bootstrap(temp.path().into(), ProviderKind::Claude, false)
            .expect("bootstrap");
        let mut state = TuiState::from_session(session);

        state.open_transcript_overlay();

        assert_eq!(
            state
                .transcript_overlay
                .as_ref()
                .expect("overlay")
                .scroll_offset,
            usize::MAX
        );
    }

    #[test]
    fn replace_transcript_swaps_committed_history_and_clears_scroll_anchors() {
        let temp = TempDir::new().expect("tempdir");
        let session = RuntimeSession::bootstrap(temp.path().into(), ProviderKind::Claude, false)
            .expect("bootstrap");
        let mut state = TuiState::from_session(session);
        state.transcript_rendered_lines = vec!["stale".to_string()];
        state.transcript_last_cell_range = Some((10, 2));

        state.replace_transcript(vec![TranscriptEntry::Status("replaced".to_string())]);

        assert_eq!(
            state.app().transcript,
            vec![TranscriptEntry::Status("replaced".to_string())]
        );
        assert!(state.transcript_rendered_lines.is_empty());
        assert!(state.transcript_last_cell_range.is_none());
    }

    #[test]
    fn active_assistant_chunks_stay_in_live_tail_until_finalize() {
        let temp = TempDir::new().expect("tempdir");
        let session = RuntimeSession::bootstrap(temp.path().into(), ProviderKind::Claude, false)
            .expect("bootstrap");
        let mut state = TuiState::from_session(session);

        state.append_active_assistant_chunk("hello ");
        state.append_active_assistant_chunk("world");

        assert!(state.active_tool_is_empty());
        assert!(matches!(
            state.active_stream_for_test(),
            Some(ActiveStream {
                kind: StreamTextKind::Assistant,
                tail,
                ..
            }) if tail == "hello world"
        ));
        assert!(!state
            .app()
            .transcript
            .iter()
            .any(|entry| matches!(entry, TranscriptEntry::Assistant(text) if text == "hello world")));

        state.finalize_active_entries_after_failure(None);

        assert!(state.active_tool_is_empty());
        assert!(matches!(
            state.app().transcript.last(),
            Some(TranscriptEntry::Assistant(text)) if text == "hello world"
        ));
    }

    #[test]
    fn assistant_chunks_commit_completed_lines_and_keep_partial_tail_active() {
        let temp = TempDir::new().expect("tempdir");
        let session = RuntimeSession::bootstrap(temp.path().into(), ProviderKind::Claude, false)
            .expect("bootstrap");
        let mut state = TuiState::from_session(session);

        state.append_active_assistant_chunk("hello ");
        state.append_active_assistant_chunk("world\nnext");

        assert!(
            !state
                .app()
                .transcript
                .iter()
                .any(|entry| matches!(entry, TranscriptEntry::Assistant(text) if text == "hello world\n"))
        );
        assert!(state.active_tool_is_empty());
        assert!(matches!(
            state.active_stream_for_test(),
            Some(ActiveStream {
                kind: StreamTextKind::Assistant,
                tail,
                ..
            }) if tail == "next"
        ));
    }

    #[test]
    fn assistant_stream_flushes_active_exec_entries_into_committed_transcript() {
        let temp = TempDir::new().expect("tempdir");
        let session = RuntimeSession::bootstrap(temp.path().into(), ProviderKind::Claude, false)
            .expect("bootstrap");
        let mut state = TuiState::from_session(session);

        state.push_active_exec_started(
            Some("call-1".to_string()),
            Some("printf hello".to_string()),
            Some("agent".to_string()),
        );
        state.append_active_assistant_chunk("answer");

        assert!(state.active_tool_is_empty());
        assert!(matches!(
            state.app().transcript.last(),
            Some(TranscriptEntry::ExecCommand {
                call_id,
                status: ExecCommandStatus::InProgress,
                ..
            }) if call_id.as_deref() == Some("call-1")
        ));
        assert!(matches!(
            state.active_stream_for_test(),
            Some(ActiveStream {
                kind: StreamTextKind::Assistant,
                tail,
                ..
            }) if tail == "answer"
        ));
    }

    #[test]
    fn starting_exec_flushes_active_assistant_stream_to_transcript() {
        let temp = TempDir::new().expect("tempdir");
        let session = RuntimeSession::bootstrap(temp.path().into(), ProviderKind::Claude, false)
            .expect("bootstrap");
        let mut state = TuiState::from_session(session);

        state.append_active_assistant_chunk("streaming answer");
        state.push_active_exec_started(
            Some("call-1".to_string()),
            Some("printf hello".to_string()),
            Some("agent".to_string()),
        );

        assert!(state.active_stream_for_test().is_none());
        assert!(matches!(
            state.app().transcript.last(),
            Some(TranscriptEntry::Assistant(text)) if text == "streaming answer"
        ));
        assert!(matches!(
            state.active_entries_for_display().last(),
            Some(TranscriptEntry::ExecCommand {
                call_id,
                status: ExecCommandStatus::InProgress,
                ..
            }) if call_id.as_deref() == Some("call-1")
        ));
    }

    #[test]
    fn starting_web_search_flushes_active_exec_live_tail_to_transcript() {
        let temp = TempDir::new().expect("tempdir");
        let session = RuntimeSession::bootstrap(temp.path().into(), ProviderKind::Claude, false)
            .expect("bootstrap");
        let mut state = TuiState::from_session(session);

        state.push_active_exec_started(
            Some("call-1".to_string()),
            Some("printf hello".to_string()),
            Some("agent".to_string()),
        );
        state.push_active_web_search_started(
            Some("search-1".to_string()),
            "ratatui styling".to_string(),
        );

        assert_eq!(state.active_tool_entries_len(), 1);
        assert!(matches!(
            state.active_entries_for_display().last(),
            Some(TranscriptEntry::WebSearch { call_id, started, .. })
                if call_id.as_deref() == Some("search-1") && *started
        ));
        assert!(matches!(
            state.app().transcript.last(),
            Some(TranscriptEntry::ExecCommand {
                call_id,
                status: ExecCommandStatus::InProgress,
                ..
            }) if call_id.as_deref() == Some("call-1")
        ));
    }

    #[test]
    fn active_thinking_chunks_stay_in_live_tail_until_finalize() {
        let temp = TempDir::new().expect("tempdir");
        let session = RuntimeSession::bootstrap(temp.path().into(), ProviderKind::Claude, false)
            .expect("bootstrap");
        let mut state = TuiState::from_session(session);

        state.append_active_thinking_chunk("step 1 ");
        state.append_active_thinking_chunk("step 2");

        assert!(state.active_tool_is_empty());
        assert!(matches!(
            state.active_stream_for_test(),
            Some(ActiveStream {
                kind: StreamTextKind::Thinking,
                tail,
                ..
            }) if tail == "step 1 step 2"
        ));
        assert!(!state
            .app()
            .transcript
            .iter()
            .any(|entry| matches!(entry, TranscriptEntry::Thinking(text) if text == "step 1 step 2")));

        state.finalize_active_entries_after_failure(None);

        assert!(state.active_tool_is_empty());
        assert!(matches!(
            state.app().transcript.last(),
            Some(TranscriptEntry::Thinking(text)) if text == "step 1 step 2"
        ));
    }

    #[test]
    fn flush_active_entries_to_transcript_commits_live_tail_without_failure_semantics() {
        let temp = TempDir::new().expect("tempdir");
        let session = RuntimeSession::bootstrap(temp.path().into(), ProviderKind::Claude, false)
            .expect("bootstrap");
        let mut state = TuiState::from_session(session);

        state.append_active_assistant_chunk("tail");
        state.flush_active_entries_to_transcript();

        assert!(state.active_tool_is_empty());
        assert!(matches!(
            state.app().transcript.last(),
            Some(TranscriptEntry::Assistant(text)) if text == "tail"
        ));
    }

    #[test]
    fn thinking_chunks_commit_completed_lines_and_keep_partial_tail_active() {
        let temp = TempDir::new().expect("tempdir");
        let session = RuntimeSession::bootstrap(temp.path().into(), ProviderKind::Claude, false)
            .expect("bootstrap");
        let mut state = TuiState::from_session(session);

        state.append_active_thinking_chunk("step 1 ");
        state.append_active_thinking_chunk("step 2\nnext");

        assert!(
            !state
                .app()
                .transcript
                .iter()
                .any(|entry| matches!(entry, TranscriptEntry::Thinking(text) if text == "step 1 step 2\n"))
        );
        assert!(state.active_tool_is_empty());
        assert!(matches!(
            state.active_stream_for_test(),
            Some(ActiveStream {
                kind: StreamTextKind::Thinking,
                tail,
                ..
            }) if tail == "next"
        ));
    }

    #[test]
    fn active_stream_commit_tick_drains_queued_assistant_lines() {
        let temp = TempDir::new().expect("tempdir");
        let session = RuntimeSession::bootstrap(temp.path().into(), ProviderKind::Claude, false)
            .expect("bootstrap");
        let mut state = TuiState::from_session(session);

        state.append_active_assistant_chunk("hello ");
        state.append_active_assistant_chunk("world\nnext");

        assert!(state.drain_active_stream_commit_tick());
        assert!(matches!(
            state.app().transcript.last(),
            Some(TranscriptEntry::Assistant(text)) if text == "hello world\n"
        ));
        assert!(matches!(
            state.active_stream_for_test(),
            Some(ActiveStream {
                kind: StreamTextKind::Assistant,
                tail,
                ..
            }) if tail == "next"
        ));
    }

    #[test]
    fn active_stream_commit_tick_catches_up_large_backlog() {
        let temp = TempDir::new().expect("tempdir");
        let session = RuntimeSession::bootstrap(temp.path().into(), ProviderKind::Claude, false)
            .expect("bootstrap");
        let mut state = TuiState::from_session(session);

        for index in 1..=8 {
            state.append_active_assistant_chunk(&format!("line {index}\n"));
        }
        state.append_active_assistant_chunk("tail");

        assert!(state.drain_active_stream_commit_tick());
        assert!(matches!(
            state.app().transcript.last(),
            Some(TranscriptEntry::Assistant(text))
                if text == "line 1\nline 2\nline 3\nline 4\nline 5\nline 6\nline 7\nline 8\n"
        ));
        assert!(matches!(
            state.active_stream_for_test(),
            Some(ActiveStream {
                kind: StreamTextKind::Assistant,
                tail,
                ..
            }) if tail == "tail"
        ));
        assert!(!state.drain_active_stream_commit_tick());
    }

    #[test]
    fn active_stream_snapshots_render_width_for_commit_line_count() {
        let temp = TempDir::new().expect("tempdir");
        let session = RuntimeSession::bootstrap(temp.path().into(), ProviderKind::Claude, false)
            .expect("bootstrap");
        let mut state = TuiState::from_session(session);
        state.transcript_render_width = Some(8);

        state.append_active_assistant_chunk("123456789\n");

        assert!(matches!(
            state.active_stream_for_test(),
            Some(ActiveStream { pending_commits, .. })
                if pending_commits.front().is_some_and(|commit| commit.rendered_lines == 2)
        ));
    }

    #[test]
    fn finalizing_active_entries_after_failure_commits_failed_history_and_clears_live_tail() {
        let temp = TempDir::new().expect("tempdir");
        let session = RuntimeSession::bootstrap(temp.path().into(), ProviderKind::Claude, false)
            .expect("bootstrap");
        let mut state = TuiState::from_session(session);
        let patch_changes = vec![PatchChange {
            path: "README.md".to_string(),
            move_path: None,
            kind: PatchChangeKind::Update,
            diff: "@@ -1 +1 @@\n-old\n+new".to_string(),
            added: 1,
            removed: 1,
        }];
        let invocation = McpInvocation {
            server: "search".to_string(),
            tool: "find_docs".to_string(),
            arguments: Some(serde_json::json!({ "query": "ratatui styling" })),
        };

        state.push_active_exec_started(
            Some("exec-1".to_string()),
            Some("printf hello".to_string()),
            Some("agent".to_string()),
        );
        state.push_active_generic_tool_call_started(
            "shell".to_string(),
            Some("tool-1".to_string()),
            Some("{\"cmd\":\"git status\"}".to_string()),
        );
        state.push_active_patch_apply_started(Some("patch-1".to_string()), patch_changes.clone());
        state.push_active_web_search_started(
            Some("search-1".to_string()),
            "ratatui styling".to_string(),
        );
        state.push_active_mcp_tool_call_started(Some("mcp-1".to_string()), invocation.clone());

        state.finalize_active_entries_after_failure(Some("provider failed"));

        assert!(state.active_tool_is_empty());
        assert!(state.app().transcript.iter().any(|entry| {
            matches!(
                entry,
                TranscriptEntry::ExecCommand {
                    call_id,
                    status: ExecCommandStatus::Failed,
                    ..
                } if call_id.as_deref() == Some("exec-1")
            )
        }));
        assert!(state.app().transcript.iter().any(|entry| {
            matches!(
                entry,
                TranscriptEntry::GenericToolCall {
                    call_id,
                    success,
                    started,
                    ..
                } if call_id.as_deref() == Some("tool-1") && !success && !started
            )
        }));
        assert!(state.app().transcript.iter().any(|entry| {
            matches!(
                entry,
                TranscriptEntry::PatchApply {
                    call_id,
                    changes,
                    status: PatchApplyStatus::Failed,
                    ..
                } if call_id.as_deref() == Some("patch-1") && changes == &patch_changes
            )
        }));
        assert!(state.app().transcript.iter().any(|entry| {
            matches!(
                entry,
                TranscriptEntry::WebSearch {
                    call_id,
                    started,
                    ..
                } if call_id.as_deref() == Some("search-1") && !started
            )
        }));
        assert!(state.app().transcript.iter().any(|entry| {
            matches!(
                entry,
                TranscriptEntry::McpToolCall {
                    call_id,
                    invocation: committed_invocation,
                    error,
                    status: McpToolCallStatus::Failed,
                    is_error,
                    ..
                } if call_id.as_deref() == Some("mcp-1")
                    && committed_invocation == &invocation
                    && error.as_deref() == Some("provider failed")
                    && *is_error
            )
        }));
    }
}
