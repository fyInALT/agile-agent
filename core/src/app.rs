use serde::Deserialize;
use serde::Serialize;
use std::path::PathBuf;

use crate::backlog::BacklogState;
use crate::backlog::TaskItem;
use crate::backlog::TaskStatus;
use crate::backlog::TodoItem;
use crate::backlog::TodoStatus;
use crate::provider::ProviderKind;
use crate::provider::SessionHandle;
use crate::skills::SkillRegistry;
use crate::tool_calls::PatchChange;
use crate::verification;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum AppStatus {
    #[default]
    Idle,
    Responding,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum LoopPhase {
    #[default]
    Idle,
    Planning,
    Executing,
    Verifying,
    Escalating,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TranscriptEntry {
    User(String),
    Assistant(String),
    Thinking(String),
    ExecCommand {
        call_id: Option<String>,
        source: Option<String>,
        input_preview: Option<String>,
        output_preview: Option<String>,
        success: bool,
        started: bool,
        exit_code: Option<i32>,
        duration_ms: Option<u64>,
    },
    PatchApply {
        call_id: Option<String>,
        changes: Vec<PatchChange>,
        success: bool,
        started: bool,
    },
    GenericToolCall {
        name: String,
        call_id: Option<String>,
        input_preview: Option<String>,
        output_preview: Option<String>,
        success: bool,
        started: bool,
        exit_code: Option<i32>,
        duration_ms: Option<u64>,
    },
    Status(String),
    Error(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppState {
    pub transcript: Vec<TranscriptEntry>,
    pub input: String,
    pub cwd: PathBuf,
    pub agent_storage_root: Option<PathBuf>,
    pub backlog: BacklogState,
    pub selected_provider: ProviderKind,
    pub skills: SkillRegistry,
    pub skill_browser_open: bool,
    pub skill_browser_selected: usize,
    pub active_task_id: Option<String>,
    pub active_task_had_error: bool,
    pub continuation_attempts: u8,
    pub loop_run_active: bool,
    pub remaining_loop_iterations: usize,
    pub claude_session_id: Option<String>,
    pub codex_thread_id: Option<String>,
    pub status: AppStatus,
    pub loop_phase: LoopPhase,
    pub should_quit: bool,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            transcript: Vec::new(),
            input: String::new(),
            cwd: PathBuf::from("."),
            agent_storage_root: None,
            backlog: BacklogState::default(),
            selected_provider: ProviderKind::Mock,
            skills: SkillRegistry::default(),
            skill_browser_open: false,
            skill_browser_selected: 0,
            active_task_id: None,
            active_task_had_error: false,
            continuation_attempts: 0,
            loop_run_active: false,
            remaining_loop_iterations: 0,
            claude_session_id: None,
            codex_thread_id: None,
            status: AppStatus::Idle,
            loop_phase: LoopPhase::Idle,
            should_quit: false,
        }
    }
}

impl AppState {
    pub fn new(selected_provider: ProviderKind) -> Self {
        Self {
            selected_provider,
            ..Self::default()
        }
    }

    pub fn with_skills(
        selected_provider: ProviderKind,
        cwd: PathBuf,
        skills: SkillRegistry,
    ) -> Self {
        Self {
            cwd,
            selected_provider,
            skills,
            ..Self::default()
        }
    }

    pub fn request_quit(&mut self) {
        self.should_quit = true;
    }

    pub fn insert_char(&mut self, ch: char) {
        self.input.push(ch);
    }

    pub fn insert_text(&mut self, text: &str) {
        self.input.push_str(text);
    }

    pub fn backspace(&mut self) {
        self.input.pop();
    }

    pub fn take_input(&mut self) -> Option<String> {
        if self.input.is_empty() {
            return None;
        }

        Some(std::mem::take(&mut self.input))
    }

    pub fn push_user_message(&mut self, text: String) {
        self.transcript.push(TranscriptEntry::User(text));
    }

    pub fn begin_provider_response(&mut self) {
        self.status = AppStatus::Responding;
        self.transcript
            .push(TranscriptEntry::Assistant(String::new()));
    }

    pub fn append_assistant_chunk(&mut self, chunk: &str) {
        match self.transcript.last_mut() {
            Some(TranscriptEntry::Assistant(text)) => text.push_str(chunk),
            _ => self
                .transcript
                .push(TranscriptEntry::Assistant(chunk.to_string())),
        }
    }

    pub fn append_thinking_chunk(&mut self, chunk: &str) {
        match self.transcript.last_mut() {
            Some(TranscriptEntry::Thinking(text)) => text.push_str(chunk),
            _ => self
                .transcript
                .push(TranscriptEntry::Thinking(chunk.to_string())),
        }
    }

    pub fn push_exec_command_started(
        &mut self,
        call_id: Option<String>,
        input_preview: Option<String>,
        source: Option<String>,
    ) {
        self.transcript.push(TranscriptEntry::ExecCommand {
            call_id,
            source,
            input_preview,
            output_preview: None,
            success: true,
            started: true,
            exit_code: None,
            duration_ms: None,
        });
    }

    pub fn push_exec_command_finished(
        &mut self,
        call_id: Option<String>,
        output_preview: Option<String>,
        success: bool,
        exit_code: Option<i32>,
        duration_ms: Option<u64>,
        source: Option<String>,
    ) {
        // Find the matching started tool call and update it
        for entry in self.transcript.iter_mut().rev() {
            match entry {
                TranscriptEntry::ExecCommand {
                    call_id: existing_call_id,
                    source: existing_source,
                    input_preview: existing_input_preview,
                    started: true,
                    ..
                } => {
                    let matches_call_id = call_id.is_some() && existing_call_id == &call_id;
                    let matches_name = existing_call_id.is_none();
                    if matches_call_id || matches_name {
                        *entry = TranscriptEntry::ExecCommand {
                            call_id: existing_call_id.clone().or(call_id),
                            source: existing_source.clone().or(source),
                            input_preview: existing_input_preview.clone(),
                            output_preview,
                            success,
                            started: false,
                            exit_code,
                            duration_ms,
                        };
                        return;
                    }
                }
                _ => {}
            }
        }
        self.transcript.push(TranscriptEntry::ExecCommand {
            call_id,
            source,
            input_preview: None,
            output_preview,
            success,
            started: false,
            exit_code,
            duration_ms,
        });
    }

    pub fn push_generic_tool_call_started(
        &mut self,
        name: String,
        call_id: Option<String>,
        input_preview: Option<String>,
    ) {
        self.transcript.push(TranscriptEntry::GenericToolCall {
            name,
            call_id,
            input_preview,
            output_preview: None,
            success: true,
            started: true,
            exit_code: None,
            duration_ms: None,
        });
    }

    pub fn push_generic_tool_call_finished(
        &mut self,
        name: String,
        call_id: Option<String>,
        output_preview: Option<String>,
        success: bool,
        exit_code: Option<i32>,
        duration_ms: Option<u64>,
    ) {
        for entry in self.transcript.iter_mut().rev() {
            if let TranscriptEntry::GenericToolCall {
                name: existing_name,
                call_id: existing_call_id,
                input_preview: existing_input_preview,
                started: true,
                ..
            } = entry
            {
                let matches_call_id = call_id.is_some() && existing_call_id == &call_id;
                let matches_name = *existing_name == name;
                if matches_call_id || matches_name {
                    *entry = TranscriptEntry::GenericToolCall {
                        name: existing_name.clone(),
                        call_id: existing_call_id.clone().or(call_id),
                        input_preview: existing_input_preview.clone(),
                        output_preview,
                        success,
                        started: false,
                        exit_code,
                        duration_ms,
                    };
                    return;
                }
            }
        }

        self.transcript.push(TranscriptEntry::GenericToolCall {
            name,
            call_id,
            input_preview: None,
            output_preview,
            success,
            started: false,
            exit_code,
            duration_ms,
        });
    }

    pub fn finish_provider_response(&mut self) {
        self.status = AppStatus::Idle;
    }

    pub fn push_patch_apply_started(
        &mut self,
        call_id: Option<String>,
        changes: Vec<PatchChange>,
    ) {
        self.transcript.push(TranscriptEntry::PatchApply {
            call_id,
            changes,
            success: true,
            started: true,
        });
    }

    pub fn push_patch_apply_finished(
        &mut self,
        call_id: Option<String>,
        changes: Vec<PatchChange>,
        success: bool,
    ) {
        for entry in self.transcript.iter_mut().rev() {
            if let TranscriptEntry::PatchApply {
                call_id: existing_call_id,
                changes: existing_changes,
                started: true,
                ..
            } = entry
            {
                let matches_call_id = call_id.is_some() && existing_call_id == &call_id;
                let matches_name = existing_call_id.is_none();
                if matches_call_id || matches_name {
                    *entry = TranscriptEntry::PatchApply {
                        call_id: existing_call_id.clone().or(call_id),
                        changes: if changes.is_empty() {
                            existing_changes.clone()
                        } else {
                            changes
                        },
                        success,
                        started: false,
                    };
                    return;
                }
            }
        }

        self.transcript.push(TranscriptEntry::PatchApply {
            call_id,
            changes,
            success,
            started: false,
        });
    }

    pub fn set_loop_phase(&mut self, phase: LoopPhase) {
        self.loop_phase = phase;
    }

    pub fn toggle_provider(&mut self) {
        self.selected_provider = self.selected_provider.next();
    }

    pub fn open_skill_browser(&mut self) {
        if self.skills.is_empty() {
            self.push_status_message("no skills available");
            return;
        }
        self.skill_browser_open = true;
        self.skill_browser_selected = self
            .skill_browser_selected
            .min(self.skills.len().saturating_sub(1));
    }

    pub fn close_skill_browser(&mut self) {
        self.skill_browser_open = false;
    }

    pub fn move_skill_selection_up(&mut self) {
        if self.skill_browser_selected > 0 {
            self.skill_browser_selected -= 1;
        }
    }

    pub fn move_skill_selection_down(&mut self) {
        if self.skill_browser_selected + 1 < self.skills.len() {
            self.skill_browser_selected += 1;
        }
    }

    pub fn toggle_selected_skill(&mut self) {
        let Some(skill) = self.skills.discovered.get(self.skill_browser_selected) else {
            return;
        };
        let name = skill.name.clone();
        self.skills.toggle(&name);
        let enabled = self.skills.is_enabled(&name);
        self.push_status_message(format!(
            "{} skill: {}",
            if enabled { "enabled" } else { "disabled" },
            name
        ));
    }

    pub fn push_status_message(&mut self, text: impl Into<String>) {
        self.transcript.push(TranscriptEntry::Status(text.into()));
    }

    pub fn push_error_message(&mut self, text: impl Into<String>) {
        self.transcript.push(TranscriptEntry::Error(text.into()));
    }

    pub fn current_session_handle(&self) -> Option<SessionHandle> {
        match self.selected_provider {
            ProviderKind::Mock => None,
            ProviderKind::Claude => {
                self.claude_session_id
                    .as_ref()
                    .map(|session_id| SessionHandle::ClaudeSession {
                        session_id: session_id.clone(),
                    })
            }
            ProviderKind::Codex => {
                self.codex_thread_id
                    .as_ref()
                    .map(|thread_id| SessionHandle::CodexThread {
                        thread_id: thread_id.clone(),
                    })
            }
        }
    }

    pub fn apply_session_handle(&mut self, handle: SessionHandle) {
        match handle {
            SessionHandle::ClaudeSession { session_id } => {
                self.claude_session_id = Some(session_id);
            }
            SessionHandle::CodexThread { thread_id } => {
                self.codex_thread_id = Some(thread_id);
            }
        }
    }

    /// Clear the session handle to start a fresh conversation
    pub fn clear_session(&mut self) {
        self.claude_session_id = None;
        self.codex_thread_id = None;
        self.transcript.clear();
    }

    pub fn start_loop_run(&mut self, max_iterations: usize) {
        self.loop_run_active = true;
        self.remaining_loop_iterations = max_iterations;
    }

    pub fn stop_loop_run(&mut self, reason: impl Into<String>) {
        self.loop_run_active = false;
        self.remaining_loop_iterations = 0;
        self.push_status_message(reason);
    }

    pub fn add_todo(&mut self, title: String) -> String {
        let id = format!("todo-{}", self.backlog.todos.len() + 1);
        let todo = TodoItem {
            id: id.clone(),
            title: title.clone(),
            description: title,
            priority: self.backlog.todos.len() as u8 + 1,
            status: TodoStatus::Ready,
            acceptance_criteria: Vec::new(),
            dependencies: Vec::new(),
            source: "manual".to_string(),
        };
        self.backlog.push_todo(todo);
        id
    }

    pub fn render_backlog_lines(&self) -> Vec<String> {
        if self.backlog.todos.is_empty() && self.backlog.tasks.is_empty() {
            return vec!["backlog is empty".to_string()];
        }

        let mut lines: Vec<String> = self
            .backlog
            .todos
            .iter()
            .map(|todo| {
                format!(
                    "{} [{}] {}",
                    todo.id,
                    match todo.status {
                        TodoStatus::Candidate => "candidate",
                        TodoStatus::Ready => "ready",
                        TodoStatus::InProgress => "in_progress",
                        TodoStatus::Blocked => "blocked",
                        TodoStatus::Done => "done",
                        TodoStatus::Dropped => "dropped",
                    },
                    todo.title
                )
            })
            .collect();

        if !self.backlog.tasks.is_empty() {
            lines.push("tasks:".to_string());
            lines.extend(self.backlog.tasks.iter().map(|task| {
                format!(
                    "{} [{}] {}",
                    task.id,
                    match task.status {
                        TaskStatus::Draft => "draft",
                        TaskStatus::Ready => "ready",
                        TaskStatus::Running => "running",
                        TaskStatus::Verifying => "verifying",
                        TaskStatus::Done => "done",
                        TaskStatus::Blocked => "blocked",
                        TaskStatus::Failed => "failed",
                    },
                    task.objective
                )
            }));
        }

        lines
    }

    pub fn next_ready_todo_id(&self) -> Option<String> {
        self.backlog
            .ready_todos()
            .first()
            .map(|todo| todo.id.clone())
    }

    pub fn begin_task_from_todo(&mut self, todo_id: &str) -> Option<TaskItem> {
        let next_task_id = format!("task-{}", self.backlog.tasks.len() + 1);
        let todo = self.backlog.find_todo_mut(todo_id)?;
        todo.status = TodoStatus::InProgress;
        let mut task = TaskItem {
            id: next_task_id,
            todo_id: todo.id.clone(),
            objective: todo.title.clone(),
            scope: format!("current workspace: {}", self.cwd.display()),
            constraints: Vec::new(),
            verification_plan: Vec::new(),
            status: TaskStatus::Ready,
            result_summary: None,
        };
        let plan = verification::build_verification_plan(&self.cwd, &task);
        task.verification_plan = verification::describe_verification_plan(&plan);
        self.active_task_id = Some(task.id.clone());
        self.active_task_had_error = false;
        self.continuation_attempts = 0;
        self.backlog.push_task(task.clone());
        Some(task)
    }

    pub fn mark_active_task_error(&mut self) {
        self.active_task_had_error = true;
    }

    pub fn mark_active_task_running(&mut self) {
        let Some(active_task_id) = self.active_task_id.as_ref() else {
            return;
        };
        if let Some(task) = self
            .backlog
            .tasks
            .iter_mut()
            .find(|task| &task.id == active_task_id)
        {
            task.status = TaskStatus::Running;
        }
    }

    pub fn mark_active_task_verifying(&mut self) {
        let Some(active_task_id) = self.active_task_id.as_ref() else {
            return;
        };
        if let Some(task) = self
            .backlog
            .tasks
            .iter_mut()
            .find(|task| &task.id == active_task_id)
        {
            task.status = TaskStatus::Verifying;
        }
    }

    pub fn active_task_summary(&self) -> Option<String> {
        self.transcript.iter().rev().find_map(|entry| match entry {
            TranscriptEntry::Assistant(text) if !text.is_empty() => Some(text.clone()),
            _ => None,
        })
    }

    pub fn complete_active_task(&mut self, summary: Option<String>) {
        let Some(active_task_id) = self.active_task_id.take() else {
            return;
        };

        let task_todo_id = self
            .backlog
            .tasks
            .iter_mut()
            .find(|task| task.id == active_task_id)
            .map(|task| {
                task.status = TaskStatus::Done;
                task.result_summary = summary.clone();
                task.todo_id.clone()
            });

        if let Some(todo_id) = task_todo_id {
            if let Some(todo) = self.backlog.find_todo_mut(&todo_id) {
                todo.status = TodoStatus::Done;
            }
        }
        self.active_task_had_error = false;
        self.continuation_attempts = 0;
    }

    pub fn fail_active_task(&mut self, reason: impl Into<String>) {
        let reason = reason.into();
        let Some(active_task_id) = self.active_task_id.take() else {
            return;
        };

        let task_todo_id = self
            .backlog
            .tasks
            .iter_mut()
            .find(|task| task.id == active_task_id)
            .map(|task| {
                task.status = TaskStatus::Failed;
                task.result_summary = Some(reason.clone());
                task.todo_id.clone()
            });

        if let Some(todo_id) = task_todo_id {
            if let Some(todo) = self.backlog.find_todo_mut(&todo_id) {
                if todo.status == TodoStatus::InProgress {
                    todo.status = TodoStatus::Ready;
                }
            }
        }
        self.active_task_had_error = false;
        self.continuation_attempts = 0;
    }

    pub fn block_active_task(&mut self, reason: impl Into<String>) {
        let reason = reason.into();
        let Some(active_task_id) = self.active_task_id.take() else {
            return;
        };

        let task_todo_id = self
            .backlog
            .tasks
            .iter_mut()
            .find(|task| task.id == active_task_id)
            .map(|task| {
                task.status = TaskStatus::Blocked;
                task.result_summary = Some(reason.clone());
                task.todo_id.clone()
            });

        if let Some(todo_id) = task_todo_id {
            if let Some(todo) = self.backlog.find_todo_mut(&todo_id) {
                todo.status = TodoStatus::Blocked;
            }
        }
        self.active_task_had_error = false;
        self.continuation_attempts = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::AppState;
    use super::AppStatus;
    use super::LoopPhase;
    use super::TranscriptEntry;
    use crate::backlog::TaskStatus;
    use crate::backlog::TodoStatus;
    use crate::provider::ProviderKind;
    use crate::provider::SessionHandle;
    use crate::skills::SkillRegistry;

    #[test]
    fn take_input_clears_buffer() {
        let mut state = AppState::default();
        state.insert_char('h');
        state.insert_char('i');

        let submitted = state.take_input();

        assert_eq!(submitted, Some("hi".to_string()));
        assert!(state.input.is_empty());
    }

    #[test]
    fn insert_text_appends_multiple_characters() {
        let mut state = AppState::default();

        state.insert_char('h');
        state.insert_text("ello\nworld");

        assert_eq!(state.input, "hello\nworld");
    }

    #[test]
    fn append_assistant_chunk_updates_last_assistant_message() {
        let mut state = AppState::new(ProviderKind::Mock);
        state.begin_provider_response();
        state.append_assistant_chunk("hello");
        state.append_assistant_chunk(" world");
        state.finish_provider_response();

        assert_eq!(state.status, AppStatus::Idle);
        assert_eq!(
            state.transcript,
            vec![TranscriptEntry::Assistant("hello world".to_string())]
        );
    }

    #[test]
    fn toggle_provider_switches_between_mock_and_claude() {
        let mut state = AppState::new(ProviderKind::Mock);
        state.toggle_provider();
        assert_eq!(state.selected_provider, ProviderKind::Claude);
        state.toggle_provider();
        assert_eq!(state.selected_provider, ProviderKind::Codex);
        state.toggle_provider();
        assert_eq!(state.selected_provider, ProviderKind::Mock);
    }

    #[test]
    fn session_handles_are_stored_per_provider() {
        let mut state = AppState::new(ProviderKind::Mock);
        state.apply_session_handle(SessionHandle::ClaudeSession {
            session_id: "s1".to_string(),
        });
        state.apply_session_handle(SessionHandle::CodexThread {
            thread_id: "t1".to_string(),
        });

        state.selected_provider = ProviderKind::Claude;
        assert_eq!(
            state.current_session_handle(),
            Some(SessionHandle::ClaudeSession {
                session_id: "s1".to_string()
            })
        );

        state.selected_provider = ProviderKind::Codex;
        assert_eq!(
            state.current_session_handle(),
            Some(SessionHandle::CodexThread {
                thread_id: "t1".to_string()
            })
        );
    }

    #[test]
    fn skill_browser_navigation_and_toggle_work() {
        let mut skills = SkillRegistry::default();
        skills.discovered.push(crate::skills::SkillMetadata {
            name: "reviewer".to_string(),
            description: "Reviews code".to_string(),
            path: "reviewer/SKILL.md".into(),
            body: "body".to_string(),
        });
        skills.discovered.push(crate::skills::SkillMetadata {
            name: "planner".to_string(),
            description: "Plans work".to_string(),
            path: "planner/SKILL.md".into(),
            body: "body".to_string(),
        });
        let mut state = AppState::with_skills(ProviderKind::Mock, ".".into(), skills);

        state.open_skill_browser();
        assert!(state.skill_browser_open);
        state.move_skill_selection_down();
        assert_eq!(state.skill_browser_selected, 1);
        state.toggle_selected_skill();
        assert!(state.skills.is_enabled("planner"));
        state.close_skill_browser();
        assert!(!state.skill_browser_open);
    }

    #[test]
    fn adds_and_lists_todos() {
        let mut state = AppState::default();
        let id = state.add_todo("write tests".to_string());

        assert_eq!(id, "todo-1");
        assert_eq!(state.backlog.todos.len(), 1);
        assert!(state.render_backlog_lines()[0].contains("write tests"));
    }

    #[test]
    fn begins_task_from_todo_and_marks_it_in_progress() {
        let mut state = AppState::default();
        let todo_id = state.add_todo("write tests".to_string());

        let task = state.begin_task_from_todo(&todo_id).expect("task");

        assert_eq!(task.todo_id, todo_id);
        assert_eq!(state.backlog.tasks.len(), 1);
        assert_eq!(state.backlog.todos[0].status, TodoStatus::InProgress);
        assert!(!task.verification_plan.is_empty());
        assert_eq!(task.status, TaskStatus::Ready);
    }

    #[test]
    fn marks_active_task_running_and_verifying() {
        let mut state = AppState::default();
        let todo_id = state.add_todo("write tests".to_string());
        state.begin_task_from_todo(&todo_id).expect("task");

        state.mark_active_task_running();
        assert_eq!(state.backlog.tasks[0].status, TaskStatus::Running);

        state.mark_active_task_verifying();
        assert_eq!(state.backlog.tasks[0].status, TaskStatus::Verifying);
    }

    #[test]
    fn finished_tool_call_matches_started_entry_by_call_id_and_preserves_started_metadata() {
        let mut state = AppState::new(ProviderKind::Mock);
        state.push_exec_command_started(
            Some("call-1".to_string()),
            Some("git diff README.md".to_string()),
            Some("agent".to_string()),
        );

        state.push_exec_command_finished(
            Some("call-1".to_string()),
            Some("diff --git a/README.md b/README.md".to_string()),
            true,
            Some(0),
            Some(180),
            Some("agent".to_string()),
        );

        assert_eq!(state.transcript.len(), 1);
        assert!(matches!(
            &state.transcript[0],
            TranscriptEntry::ExecCommand {
                call_id,
                source,
                input_preview,
                output_preview,
                success,
                started,
                exit_code,
                duration_ms,
            }
            if call_id.as_deref() == Some("call-1")
                && source.as_deref() == Some("agent")
                && input_preview.as_deref() == Some("git diff README.md")
                && output_preview.as_deref() == Some("diff --git a/README.md b/README.md")
                && *success
                && !*started
                && *exit_code == Some(0)
                && *duration_ms == Some(180)
        ));
    }

    #[test]
    fn patch_apply_uses_dedicated_transcript_entry() {
        let mut state = AppState::new(ProviderKind::Mock);
        state.push_patch_apply_started(
            Some("patch-1".to_string()),
            vec![crate::tool_calls::PatchChange {
                path: "README.md".to_string(),
                move_path: None,
                kind: crate::tool_calls::PatchChangeKind::Update,
                diff: "@@ -1 +1 @@\n-old\n+new".to_string(),
                added: 1,
                removed: 1,
            }],
        );
        state.push_patch_apply_finished(
            Some("patch-1".to_string()),
            vec![crate::tool_calls::PatchChange {
                path: "README.md".to_string(),
                move_path: None,
                kind: crate::tool_calls::PatchChangeKind::Update,
                diff: "@@ -1 +1 @@\n-old\n+new".to_string(),
                added: 1,
                removed: 1,
            }],
            true,
        );

        assert_eq!(state.transcript.len(), 1);
        assert!(matches!(
            &state.transcript[0],
            TranscriptEntry::PatchApply {
                call_id,
                changes,
                success,
                started,
            }
            if call_id.as_deref() == Some("patch-1")
                && changes == &vec![crate::tool_calls::PatchChange {
                    path: "README.md".to_string(),
                    move_path: None,
                    kind: crate::tool_calls::PatchChangeKind::Update,
                    diff: "@@ -1 +1 @@\n-old\n+new".to_string(),
                    added: 1,
                    removed: 1,
                }]
                && *success
                && !*started
        ));
    }

    #[test]
    fn generic_tool_result_uses_generic_tool_call_entry() {
        let mut state = AppState::new(ProviderKind::Mock);
        state.push_generic_tool_call_started(
            "read_file".to_string(),
            Some("call-2".to_string()),
            Some("{\"path\":\"README.md\"}".to_string()),
        );
        state.push_generic_tool_call_finished(
            "read_file".to_string(),
            Some("call-2".to_string()),
            Some("done".to_string()),
            true,
            None,
            None,
        );

        assert_eq!(state.transcript.len(), 1);
        assert!(matches!(
            &state.transcript[0],
            TranscriptEntry::GenericToolCall {
                name,
                call_id,
                input_preview,
                output_preview,
                success,
                started,
                ..
            }
            if name == "read_file"
                && call_id.as_deref() == Some("call-2")
                && input_preview.as_deref() == Some("{\"path\":\"README.md\"}")
                && output_preview.as_deref() == Some("done")
                && *success
                && !*started
        ));
    }

    #[test]
    fn failing_active_task_marks_task_failed_and_todo_ready() {
        let mut state = AppState::default();
        let todo_id = state.add_todo("write tests".to_string());
        state.begin_task_from_todo(&todo_id).expect("task");

        state.fail_active_task("verification failed");

        assert_eq!(state.backlog.tasks[0].status, TaskStatus::Failed);
        assert_eq!(state.backlog.todos[0].status, TodoStatus::Ready);
    }

    #[test]
    fn loop_phase_can_be_updated() {
        let mut state = AppState::default();
        state.set_loop_phase(LoopPhase::Planning);
        assert_eq!(state.loop_phase, LoopPhase::Planning);
    }
}
