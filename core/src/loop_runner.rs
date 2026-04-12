use std::sync::mpsc;
use std::time::Duration;

use anyhow::Result;

use crate::app::AppState;
use crate::app::LoopPhase;
use crate::autonomy;
use crate::autonomy::CompletionDecision;
use crate::backlog::TaskItem;
use crate::escalation;
use crate::escalation::EscalationRecord;
use crate::provider;
use crate::provider::ProviderEvent;
use crate::verification;
use crate::verification::VerificationOutcome;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LoopGuardrails {
    pub max_iterations: usize,
    pub max_continuations_per_task: u8,
}

impl Default for LoopGuardrails {
    fn default() -> Self {
        Self {
            max_iterations: 5,
            max_continuations_per_task: 3,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoopRunSummary {
    pub iterations: usize,
    pub stopped_reason: String,
}

pub fn run_single_iteration(state: &mut AppState) -> Result<Option<LoopRunSummary>> {
    let Some(todo_id) = state.next_ready_todo_id() else {
        state.push_status_message("no ready todo available");
        state.set_loop_phase(LoopPhase::Idle);
        return Ok(None);
    };

    state.set_loop_phase(LoopPhase::Planning);
    let Some(task) = state.begin_task_from_todo(&todo_id) else {
        anyhow::bail!("failed to start task from todo: {todo_id}");
    };
    state.push_status_message(format!("running task: {}", task.id));
    execute_task_until_resolution(state, &task, LoopGuardrails::default())?;

    Ok(Some(LoopRunSummary {
        iterations: 1,
        stopped_reason: "single iteration completed".to_string(),
    }))
}

pub fn run_loop(state: &mut AppState, guardrails: LoopGuardrails) -> Result<LoopRunSummary> {
    let mut iterations = 0usize;

    loop {
        if iterations >= guardrails.max_iterations {
            state.set_loop_phase(LoopPhase::Escalating);
            state.push_error_message("loop guardrail reached: max iterations");
            return Ok(LoopRunSummary {
                iterations,
                stopped_reason: "max iterations reached".to_string(),
            });
        }

        let Some(todo_id) = state.next_ready_todo_id() else {
            state.set_loop_phase(LoopPhase::Idle);
            return Ok(LoopRunSummary {
                iterations,
                stopped_reason: "no ready todo available".to_string(),
            });
        };

        state.set_loop_phase(LoopPhase::Planning);
        let Some(task) = state.begin_task_from_todo(&todo_id) else {
            anyhow::bail!("failed to start task from todo: {todo_id}");
        };
        state.push_status_message(format!("running task: {}", task.id));
        execute_task_until_resolution(state, &task, guardrails)?;
        iterations += 1;
    }
}

fn execute_task_until_resolution(
    state: &mut AppState,
    task: &TaskItem,
    guardrails: LoopGuardrails,
) -> Result<()> {
    let mut prompt = build_task_prompt(task);

    loop {
        state.set_loop_phase(LoopPhase::Executing);
        state.begin_provider_response();

        let (event_tx, event_rx) = mpsc::channel();
        let provider_kind = state.selected_provider;
        let session_handle = state.current_session_handle();
        provider::start_provider(
            provider_kind,
            prompt,
            state.cwd.clone(),
            session_handle,
            event_tx,
        )?;

        consume_provider_until_finished(state, event_rx);

        let summary = state.active_task_summary();
        if state.active_task_had_error {
            state.set_loop_phase(LoopPhase::Escalating);
            escalate_active_task(state, "provider execution failed");
            return Ok(());
        }

        let Some(summary_text) = summary.clone() else {
            state.set_loop_phase(LoopPhase::Escalating);
            escalate_active_task(state, "no assistant summary available");
            return Ok(());
        };

        if let Some(next_prompt) = autonomy::continuation_prompt(&summary_text) {
            if state.continuation_attempts >= guardrails.max_continuations_per_task {
                state.set_loop_phase(LoopPhase::Escalating);
                escalate_active_task(state, "continuation limit reached");
                return Ok(());
            }
            state.continuation_attempts += 1;
            state.push_status_message(format!(
                "continuing active task automatically (attempt {})",
                state.continuation_attempts
            ));
            prompt = next_prompt;
            continue;
        }

        match autonomy::judge_completion(&summary_text) {
            CompletionDecision::Complete => {
                state.set_loop_phase(LoopPhase::Verifying);
                let plan = verification::build_verification_plan(&state.cwd, task);
                let result =
                    verification::execute_verification(&plan, &state.cwd, Some(&summary_text));
                state.push_status_message(result.summary.clone());
                for evidence in result.evidence {
                    state.push_status_message(format!("evidence: {}", evidence));
                }
                match result.outcome {
                    VerificationOutcome::Passed => {
                        state.complete_active_task(summary);
                        state.set_loop_phase(LoopPhase::Idle);
                    }
                    VerificationOutcome::Failed | VerificationOutcome::NotRunnable => {
                        state.set_loop_phase(LoopPhase::Escalating);
                        escalate_active_task(state, "verification failed");
                    }
                }
            }
            CompletionDecision::Incomplete { reason } => {
                state.set_loop_phase(LoopPhase::Escalating);
                escalate_active_task(state, reason);
            }
        }

        return Ok(());
    }
}

fn consume_provider_until_finished(state: &mut AppState, rx: mpsc::Receiver<ProviderEvent>) {
    loop {
        match rx.recv_timeout(Duration::from_millis(100)) {
            Ok(event) => match event {
                ProviderEvent::Status(text) => state.push_status_message(text),
                ProviderEvent::AssistantChunk(chunk) => state.append_assistant_chunk(&chunk),
                ProviderEvent::ThinkingChunk(chunk) => state.append_thinking_chunk(&chunk),
                ProviderEvent::ToolCallStarted {
                    name,
                    call_id,
                    input_preview,
                } => state.push_tool_call_started(name, call_id, input_preview),
                ProviderEvent::ToolCallFinished {
                    name,
                    call_id,
                    output_preview,
                    success,
                } => state.push_tool_call_finished(name, call_id, output_preview, success),
                ProviderEvent::SessionHandle(handle) => state.apply_session_handle(handle),
                ProviderEvent::Error(error) => {
                    state.mark_active_task_error();
                    state.push_error_message(error);
                }
                ProviderEvent::Finished => {
                    state.finish_provider_response();
                    break;
                }
            },
            Err(mpsc::RecvTimeoutError::Timeout) => continue,
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                state.mark_active_task_error();
                state.push_error_message("provider event stream disconnected");
                state.finish_provider_response();
                break;
            }
        }
    }
}

fn build_task_prompt(task: &TaskItem) -> String {
    let mut prompt = format!(
        "Execute the following task.\n\nObjective: {}\nScope: {}\n",
        task.objective, task.scope
    );

    if !task.constraints.is_empty() {
        prompt.push_str("Constraints:\n");
        for constraint in &task.constraints {
            prompt.push_str(&format!("- {}\n", constraint));
        }
    }

    if !task.verification_plan.is_empty() {
        prompt.push_str("\nVerification plan:\n");
        for item in &task.verification_plan {
            prompt.push_str(&format!("- {}\n", item));
        }
    }

    prompt
}

fn escalate_active_task(state: &mut AppState, reason: impl Into<String>) {
    let reason = reason.into();
    let task_id = state
        .active_task_id
        .clone()
        .unwrap_or_else(|| "unknown-task".to_string());
    let context_summary = state
        .active_task_summary()
        .unwrap_or_else(|| "no assistant summary".to_string());

    let record = EscalationRecord {
        task_id: task_id.clone(),
        reason: reason.clone(),
        context_summary,
        recommended_actions: vec![
            "inspect task output".to_string(),
            "review verification evidence".to_string(),
        ],
        created_at: chrono::Utc::now().to_rfc3339(),
    };

    let artifact_path = escalation::save_escalation(&record).ok();
    state.block_active_task(reason.clone());
    state.push_error_message(format!("escalated task: {} ({})", task_id, reason));
    if let Some(path) = artifact_path {
        state.push_status_message(format!("escalation artifact: {}", path.display()));
    }
}

#[cfg(test)]
mod tests {
    use super::LoopGuardrails;
    use super::run_loop;
    use super::run_single_iteration;
    use crate::app::AppState;
    use crate::backlog::TodoItem;
    use crate::backlog::TodoStatus;
    use crate::provider::ProviderKind;
    use crate::skills::SkillRegistry;

    fn ready_todo(id: &str, title: &str, priority: u8) -> TodoItem {
        TodoItem {
            id: id.to_string(),
            title: title.to_string(),
            description: title.to_string(),
            priority,
            status: TodoStatus::Ready,
            acceptance_criteria: Vec::new(),
            dependencies: Vec::new(),
            source: "test".to_string(),
        }
    }

    #[test]
    fn run_single_iteration_returns_none_when_no_ready_todo_exists() {
        let mut state =
            AppState::with_skills(ProviderKind::Mock, ".".into(), SkillRegistry::default());
        let summary = run_single_iteration(&mut state).expect("run once");
        assert!(summary.is_none());
    }

    #[test]
    fn run_loop_stops_when_no_ready_todos_remain() {
        let mut state =
            AppState::with_skills(ProviderKind::Mock, ".".into(), SkillRegistry::default());
        state
            .backlog
            .push_todo(ready_todo("todo-1", "write summary", 1));

        let summary = run_loop(
            &mut state,
            LoopGuardrails {
                max_iterations: 2,
                max_continuations_per_task: 1,
            },
        )
        .expect("run loop");

        assert_eq!(summary.iterations, 1);
        assert_eq!(summary.stopped_reason, "no ready todo available");
        assert_eq!(state.backlog.todos[0].status, TodoStatus::Done);
    }
}
