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

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum AppStatus {
    #[default]
    Idle,
    Responding,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TranscriptEntry {
    User(String),
    Assistant(String),
    Thinking(String),
    ToolCall {
        name: String,
        call_id: Option<String>,
        input_preview: Option<String>,
        output_preview: Option<String>,
        success: bool,
        started: bool,
    },
    Status(String),
    Error(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppState {
    pub transcript: Vec<TranscriptEntry>,
    pub input: String,
    pub cwd: PathBuf,
    pub backlog: BacklogState,
    pub selected_provider: ProviderKind,
    pub skills: SkillRegistry,
    pub skill_browser_open: bool,
    pub skill_browser_selected: usize,
    pub active_task_id: Option<String>,
    pub claude_session_id: Option<String>,
    pub codex_thread_id: Option<String>,
    pub status: AppStatus,
    pub should_quit: bool,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            transcript: Vec::new(),
            input: String::new(),
            cwd: PathBuf::from("."),
            backlog: BacklogState::default(),
            selected_provider: ProviderKind::Mock,
            skills: SkillRegistry::default(),
            skill_browser_open: false,
            skill_browser_selected: 0,
            active_task_id: None,
            claude_session_id: None,
            codex_thread_id: None,
            status: AppStatus::Idle,
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

    pub fn push_tool_call_started(
        &mut self,
        name: String,
        call_id: Option<String>,
        input_preview: Option<String>,
    ) {
        self.transcript.push(TranscriptEntry::ToolCall {
            name,
            call_id,
            input_preview,
            output_preview: None,
            success: true,
            started: true,
        });
    }

    pub fn push_tool_call_finished(
        &mut self,
        name: String,
        call_id: Option<String>,
        output_preview: Option<String>,
        success: bool,
    ) {
        // Find the matching started tool call and update it
        for entry in self.transcript.iter_mut().rev() {
            if let TranscriptEntry::ToolCall {
                name: existing_name,
                call_id: existing_call_id,
                started: true,
                ..
            } = entry
            {
                if *existing_name == name && (call_id.is_none() || existing_call_id == &call_id) {
                    *entry = TranscriptEntry::ToolCall {
                        name,
                        call_id,
                        input_preview: None,
                        output_preview,
                        success,
                        started: false,
                    };
                    return;
                }
            }
        }
        // If not found, add as a finished entry
        self.transcript.push(TranscriptEntry::ToolCall {
            name,
            call_id,
            input_preview: None,
            output_preview,
            success,
            started: false,
        });
    }

    pub fn finish_provider_response(&mut self) {
        self.status = AppStatus::Idle;
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
                        TaskStatus::Completed => "completed",
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
        let task = TaskItem {
            id: next_task_id,
            todo_id: todo.id.clone(),
            objective: todo.title.clone(),
            scope: format!("current workspace: {}", self.cwd.display()),
            constraints: Vec::new(),
            verification_plan: Vec::new(),
            status: TaskStatus::Running,
            result_summary: None,
        };
        self.active_task_id = Some(task.id.clone());
        self.backlog.push_task(task.clone());
        Some(task)
    }

    pub fn finish_active_task(&mut self, summary: Option<String>) {
        let Some(active_task_id) = self.active_task_id.take() else {
            return;
        };

        let task_todo_id = self
            .backlog
            .tasks
            .iter_mut()
            .find(|task| task.id == active_task_id)
            .map(|task| {
                task.status = TaskStatus::Completed;
                task.result_summary = summary.clone();
                task.todo_id.clone()
            });

        if let Some(todo_id) = task_todo_id {
            if let Some(todo) = self.backlog.find_todo_mut(&todo_id) {
                todo.status = TodoStatus::InProgress;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::AppState;
    use super::AppStatus;
    use super::TranscriptEntry;
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
    }
}
