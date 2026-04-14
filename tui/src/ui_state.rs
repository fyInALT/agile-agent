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
use crate::composer::textarea::TextAreaState;
use crate::transcript::overlay::TranscriptOverlayState;

#[derive(Debug)]
pub struct TuiState {
    pub session: RuntimeSession,
    pub active_entries: Vec<TranscriptEntry>,
    pub active_stream: Option<ActiveStream>,
    pub active_entries_revision: u64,
    pub composer: TextArea,
    pub composer_state: TextAreaState,
    pub transcript_overlay: Option<TranscriptOverlayState>,
    pub composer_width: u16,
    pub transcript_viewport_height: u16,
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
            active_entries: Vec::new(),
            active_stream: None,
            active_entries_revision: 0,
            composer,
            composer_state: TextAreaState::default(),
            transcript_overlay: None,
            composer_width: 80,
            transcript_viewport_height: 1,
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

    pub fn active_entries_for_display(&self) -> Vec<TranscriptEntry> {
        let mut entries = self.active_entries.clone();
        if let Some(stream) = self.active_stream.as_ref().filter(|stream| !stream.tail.is_empty()) {
            entries.push(stream.as_transcript_entry());
        }
        entries
    }

    #[cfg(test)]
    pub fn overlay_entries_for_display(&self) -> Vec<TranscriptEntry> {
        let mut entries = self.session.app.transcript.clone();
        entries.extend(self.active_entries_for_display());
        entries
    }

    pub fn active_entries_revision_key(&self) -> Option<u64> {
        (!self.active_entries.is_empty() || self.active_stream.is_some())
            .then_some(self.active_entries_revision)
    }

    pub fn live_tail_is_stream_continuation(&self) -> bool {
        matches!(
            (
                self.app().transcript.last(),
                self.active_stream.as_ref().map(|stream| stream.kind),
            ),
            (Some(TranscriptEntry::Assistant(_)), Some(StreamTextKind::Assistant))
                | (Some(TranscriptEntry::Thinking(_)), Some(StreamTextKind::Thinking))
        )
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
        if !self.active_entries.is_empty() {
            self.flush_active_entries_to_transcript();
        }

        let mut committed = None;
        let stream = self.ensure_active_stream(kind);
        stream.tail.push_str(chunk);
        if let Some(split_index) = stream.tail.rfind('\n').map(|index| index + 1) {
            let remainder = stream.tail.split_off(split_index);
            let finished = std::mem::replace(&mut stream.tail, remainder);
            if !finished.is_empty() {
                committed = Some(finished);
            }
        }

        if let Some(committed) = committed {
            stream.pending_commits.push_back(committed);
        }
        self.drop_empty_active_stream();
        self.bump_active_entries_revision();
    }

    fn ensure_active_stream(&mut self, kind: StreamTextKind) -> &mut ActiveStream {
        if self
            .active_stream
            .as_ref()
            .is_some_and(|stream| stream.kind != kind && !stream.tail.is_empty())
        {
            if let Some(stream) = self.active_stream.take() {
                self.flush_stream_to_transcript(stream);
            }
        }

        let stream = self.active_stream.get_or_insert(ActiveStream {
            kind,
            tail: String::new(),
            pending_commits: VecDeque::new(),
        });
        stream.kind = kind;
        stream
    }

    fn drop_empty_active_stream(&mut self) {
        if self
            .active_stream
            .as_ref()
            .is_some_and(|stream| stream.tail.is_empty() && stream.pending_commits.is_empty())
        {
            self.active_stream = None;
        }
    }

    pub fn drain_active_stream_commit_tick(&mut self) -> bool {
        let next = self
            .active_stream
            .as_mut()
            .and_then(|stream| stream.pending_commits.pop_front().map(|text| (stream.kind, text)));
        let Some((kind, text)) = next else {
            return false;
        };
        self.commit_stream_text(kind, &text);
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
        self.active_entries.retain(|entry| {
            !matches!(entry, TranscriptEntry::ExecCommand { call_id: existing, .. } if call_id.is_some() && existing == &call_id)
        });
        self.active_entries.push(TranscriptEntry::ExecCommand {
            call_id,
            source,
            allow_exploring_group: true,
            input_preview,
            output_preview: None,
            status: ExecCommandStatus::InProgress,
            exit_code: None,
            duration_ms: None,
        });
        self.bump_active_entries_revision();
    }

    pub fn append_active_exec_output(&mut self, call_id: Option<String>, delta: &str) {
        if delta.is_empty() {
            return;
        }

        for entry in self.active_entries.iter_mut().rev() {
            if let TranscriptEntry::ExecCommand {
                call_id: existing_call_id,
                output_preview,
                ..
            } = entry
            {
                let matches_call_id = call_id.is_some() && existing_call_id == &call_id;
                let matches_latest = call_id.is_none();
                if matches_call_id || matches_latest {
                    output_preview
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
        if let Some(index) = self.active_entries.iter().rposition(|entry| {
            matches!(
                entry,
                TranscriptEntry::ExecCommand {
                    call_id: existing_call_id,
                    ..
                } if call_id.is_some() && existing_call_id == &call_id
            )
        }) {
            let entry = self.active_entries.remove(index);
            if let TranscriptEntry::ExecCommand {
                call_id,
                source: existing_source,
                allow_exploring_group,
                input_preview,
                output_preview: existing_output_preview,
                ..
            } = entry
            {
                self.session
                    .app
                    .transcript
                    .push(TranscriptEntry::ExecCommand {
                        call_id,
                        source: existing_source.or(source),
                        allow_exploring_group,
                        input_preview,
                        output_preview: output_preview.or(existing_output_preview),
                        status,
                        exit_code,
                        duration_ms,
                    });
                self.bump_active_entries_revision();
                return;
            }
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
        self.active_entries.retain(|entry| {
            !matches!(entry, TranscriptEntry::GenericToolCall { call_id: existing, .. } if call_id.is_some() && existing == &call_id)
        });
        self.active_entries.push(TranscriptEntry::GenericToolCall {
            name,
            call_id,
            input_preview,
            output_preview: None,
            success: true,
            started: true,
            exit_code: None,
            duration_ms: None,
        });
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
        if let Some(index) = self.active_entries.iter().rposition(|entry| {
            matches!(
                entry,
                TranscriptEntry::GenericToolCall {
                    name: existing_name,
                    call_id: existing_call_id,
                    started: true,
                    ..
                } if (call_id.is_some() && existing_call_id == &call_id) || *existing_name == name
            )
        }) {
            let entry = self.active_entries.remove(index);
            if let TranscriptEntry::GenericToolCall {
                name: existing_name,
                call_id: existing_call_id,
                input_preview,
                output_preview: existing_output_preview,
                ..
            } = entry
            {
                self.session
                    .app
                    .transcript
                    .push(TranscriptEntry::GenericToolCall {
                        name: existing_name,
                        call_id: existing_call_id.or(call_id),
                        input_preview,
                        output_preview: output_preview.or(existing_output_preview),
                        success,
                        started: false,
                        exit_code,
                        duration_ms,
                    });
                self.bump_active_entries_revision();
                return;
            }
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
        self.active_entries.retain(|entry| {
            !matches!(entry, TranscriptEntry::PatchApply { call_id: existing, .. } if call_id.is_some() && existing == &call_id)
        });
        self.active_entries.push(TranscriptEntry::PatchApply {
            call_id,
            changes,
            status: PatchApplyStatus::InProgress,
            output_preview: None,
        });
        self.bump_active_entries_revision();
    }

    pub fn append_active_patch_apply_output(&mut self, call_id: Option<String>, delta: &str) {
        if delta.is_empty() {
            return;
        }

        for entry in self.active_entries.iter_mut().rev() {
            if let TranscriptEntry::PatchApply {
                call_id: existing_call_id,
                output_preview,
                ..
            } = entry
            {
                let matches_call_id = call_id.is_some() && existing_call_id == &call_id;
                let matches_latest = call_id.is_none();
                if matches_call_id || matches_latest {
                    output_preview
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
        if let Some(index) = self.active_entries.iter().rposition(|entry| {
            matches!(
                entry,
                TranscriptEntry::PatchApply {
                    call_id: existing_call_id,
                    status: PatchApplyStatus::InProgress,
                    ..
                } if (call_id.is_some() && existing_call_id == &call_id) || existing_call_id.is_none()
            )
        }) {
            let entry = self.active_entries.remove(index);
            if let TranscriptEntry::PatchApply {
                call_id: existing_call_id,
                changes: existing_changes,
                output_preview,
                ..
            } = entry
            {
                self.session.app.transcript.push(TranscriptEntry::PatchApply {
                    call_id: existing_call_id.or(call_id),
                    changes: if changes.is_empty() {
                        existing_changes
                    } else {
                        changes
                    },
                    status,
                    output_preview,
                });
                self.bump_active_entries_revision();
                return;
            }
        }

        self.session
            .app
            .push_patch_apply_finished(call_id, changes, status);
    }

    pub fn push_active_web_search_started(&mut self, call_id: Option<String>, query: String) {
        self.prepare_for_active_tool_start(ActiveToolStart::Other);
        self.flush_active_stream_to_transcript();
        self.active_entries.retain(|entry| {
            !matches!(entry, TranscriptEntry::WebSearch { call_id: existing, .. } if call_id.is_some() && existing == &call_id)
        });
        self.active_entries.push(TranscriptEntry::WebSearch {
            call_id,
            query,
            action: None,
            started: true,
        });
        self.bump_active_entries_revision();
    }

    pub fn finish_active_web_search(
        &mut self,
        call_id: Option<String>,
        query: String,
        action: Option<WebSearchAction>,
    ) {
        if let Some(index) = self.active_entries.iter().rposition(|entry| {
            matches!(
                entry,
                TranscriptEntry::WebSearch {
                    call_id: existing_call_id,
                    started: true,
                    ..
                } if (call_id.is_some() && existing_call_id == &call_id) || existing_call_id.is_none()
            )
        }) {
            let entry = self.active_entries.remove(index);
            if let TranscriptEntry::WebSearch {
                call_id: existing_call_id,
                ..
            } = entry
            {
                self.session.app.transcript.push(TranscriptEntry::WebSearch {
                    call_id: existing_call_id.or(call_id),
                    query,
                    action,
                    started: false,
                });
                self.bump_active_entries_revision();
                return;
            }
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
        self.active_entries.retain(|entry| {
            !matches!(entry, TranscriptEntry::McpToolCall { call_id: existing, .. } if call_id.is_some() && existing == &call_id)
        });
        self.active_entries.push(TranscriptEntry::McpToolCall {
            call_id,
            invocation,
            result_blocks: Vec::new(),
            error: None,
            status: McpToolCallStatus::InProgress,
            is_error: false,
        });
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
        if let Some(index) = self.active_entries.iter().rposition(|entry| {
            matches!(
                entry,
                TranscriptEntry::McpToolCall {
                    call_id: existing_call_id,
                    status: McpToolCallStatus::InProgress,
                    ..
                } if (call_id.is_some() && existing_call_id == &call_id) || existing_call_id.is_none()
            )
        }) {
            let entry = self.active_entries.remove(index);
            if let TranscriptEntry::McpToolCall {
                call_id: existing_call_id,
                ..
            } = entry
            {
                self.session.app.transcript.push(TranscriptEntry::McpToolCall {
                    call_id: existing_call_id.or(call_id),
                    invocation,
                    result_blocks,
                    error,
                    status,
                    is_error,
                });
                self.bump_active_entries_revision();
                return;
            }
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
        self.active_entries.clear();
        self.active_stream = None;
        self.bump_active_entries_revision();
        self.transcript_scroll_offset = 0;
        self.transcript_max_scroll = 0;
        self.transcript_follow_tail = true;
        self.transcript_rendered_lines.clear();
        self.transcript_last_cell_range = None;
        self.busy_started_at = None;
        Ok(summary)
    }
}

impl TuiState {
    fn drain_active_entries(&mut self, failure_reason: Option<&str>) {
        if self.active_entries.is_empty() && self.active_stream.is_none() {
            return;
        }
        if let Some(stream) = self.active_stream.take() {
            self.flush_stream_to_transcript(stream);
        }
        for entry in std::mem::take(&mut self.active_entries) {
            match (failure_reason, entry) {
                (Some(_), TranscriptEntry::ExecCommand {
                    call_id,
                    source,
                    allow_exploring_group,
                    input_preview,
                    output_preview,
                    status: ExecCommandStatus::InProgress,
                    exit_code,
                    duration_ms,
                }) => self.session.app.transcript.push(TranscriptEntry::ExecCommand {
                    call_id,
                    source,
                    allow_exploring_group,
                    input_preview,
                    output_preview,
                    status: ExecCommandStatus::Failed,
                    exit_code,
                    duration_ms,
                }),
                (Some(_), TranscriptEntry::GenericToolCall {
                    name,
                    call_id,
                    input_preview,
                    output_preview,
                    started: true,
                    ..
                }) => self.session.app.transcript.push(TranscriptEntry::GenericToolCall {
                    name,
                    call_id,
                    input_preview,
                    output_preview,
                    success: false,
                    started: false,
                    exit_code: None,
                    duration_ms: None,
                }),
                (Some(_), TranscriptEntry::PatchApply {
                    call_id,
                    changes,
                    status: PatchApplyStatus::InProgress,
                    output_preview,
                }) => self.session.app.transcript.push(TranscriptEntry::PatchApply {
                    call_id,
                    changes,
                    status: PatchApplyStatus::Failed,
                    output_preview,
                }),
                (Some(_), TranscriptEntry::WebSearch { started: true, .. }) => {}
                (Some(reason), TranscriptEntry::McpToolCall {
                    call_id,
                    invocation,
                    result_blocks,
                    error,
                    status: McpToolCallStatus::InProgress,
                    ..
                }) => self.session.app.transcript.push(TranscriptEntry::McpToolCall {
                    call_id,
                    invocation,
                    result_blocks,
                    error: error.or_else(|| Some(reason.to_string())),
                    status: McpToolCallStatus::Failed,
                    is_error: true,
                }),
                (_, other) => self.session.app.transcript.push(other),
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
        for text in stream.pending_commits {
            self.commit_stream_text(stream.kind, &text);
        }
        if !stream.tail.is_empty() {
            self.commit_stream_text(stream.kind, &stream.tail);
        }
    }

    fn flush_active_stream_to_transcript(&mut self) {
        if let Some(stream) = self.active_stream.take() {
            self.flush_stream_to_transcript(stream);
            self.bump_active_entries_revision();
        }
    }

    fn prepare_for_active_tool_start(&mut self, start: ActiveToolStart) {
        let should_flush = match start {
            ActiveToolStart::Exec => self.active_entries.iter().any(|entry| {
                !matches!(entry, TranscriptEntry::ExecCommand { .. })
            }),
            ActiveToolStart::Other => !self.active_entries.is_empty(),
        };
        if should_flush {
            self.flush_active_entries_to_transcript();
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActiveStream {
    pub(crate) kind: StreamTextKind,
    pub(crate) tail: String,
    pub(crate) pending_commits: VecDeque<String>,
}

impl ActiveStream {
    fn as_transcript_entry(&self) -> TranscriptEntry {
        match self.kind {
            StreamTextKind::Assistant => TranscriptEntry::Assistant(self.tail.clone()),
            StreamTextKind::Thinking => TranscriptEntry::Thinking(self.tail.clone()),
        }
    }
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
        state.active_entries.push(TranscriptEntry::ExecCommand {
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

        assert!(state.active_entries.is_empty());
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

        assert!(state.active_entries.is_empty());
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

        assert!(state.active_entries.is_empty());
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

        assert!(state.active_entries.is_empty());
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

        assert!(state.active_entries.is_empty());
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

        assert!(state.active_entries.is_empty());
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
    fn active_assistant_chunks_stay_in_live_tail_until_finalize() {
        let temp = TempDir::new().expect("tempdir");
        let session = RuntimeSession::bootstrap(temp.path().into(), ProviderKind::Claude, false)
            .expect("bootstrap");
        let mut state = TuiState::from_session(session);

        state.append_active_assistant_chunk("hello ");
        state.append_active_assistant_chunk("world");

        assert!(state.active_entries.is_empty());
        assert!(matches!(
            state.active_stream.as_ref(),
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

        assert!(state.active_entries.is_empty());
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
        assert!(state.active_entries.is_empty());
        assert!(matches!(
            state.active_stream.as_ref(),
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

        assert!(state.active_entries.is_empty());
        assert!(matches!(
            state.app().transcript.last(),
            Some(TranscriptEntry::ExecCommand {
                call_id,
                status: ExecCommandStatus::InProgress,
                ..
            }) if call_id.as_deref() == Some("call-1")
        ));
        assert!(matches!(
            state.active_stream.as_ref(),
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

        assert!(state.active_stream.is_none());
        assert!(matches!(
            state.app().transcript.last(),
            Some(TranscriptEntry::Assistant(text)) if text == "streaming answer"
        ));
        assert!(matches!(
            state.active_entries.last(),
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

        assert_eq!(state.active_entries.len(), 1);
        assert!(matches!(
            state.active_entries.last(),
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

        assert!(state.active_entries.is_empty());
        assert!(matches!(
            state.active_stream.as_ref(),
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

        assert!(state.active_entries.is_empty());
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

        assert!(state.active_entries.is_empty());
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
        assert!(state.active_entries.is_empty());
        assert!(matches!(
            state.active_stream.as_ref(),
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
            state.active_stream.as_ref(),
            Some(ActiveStream {
                kind: StreamTextKind::Assistant,
                tail,
                ..
            }) if tail == "next"
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

        assert!(state.active_entries.is_empty());
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
