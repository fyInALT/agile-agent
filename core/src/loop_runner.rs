use std::sync::mpsc;
use std::time::Duration;

use anyhow::Result;

use crate::app::AppState;
use crate::app::LoopPhase;
use crate::backlog::TaskItem;
use crate::logging;
// Provider types are re-exported at crate root
use crate::ProviderEvent;
use crate::task_engine;
use crate::task_engine::ExecutionGuardrails;
use crate::task_engine::TurnResolution;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LoopGuardrails {
    pub max_iterations: usize,
    pub max_continuations_per_task: u8,
    pub max_verification_failures: usize,
}

impl Default for LoopGuardrails {
    fn default() -> Self {
        Self {
            max_iterations: 5,
            max_continuations_per_task: 3,
            max_verification_failures: 1,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoopRunSummary {
    pub iterations: usize,
    pub verification_failures: usize,
    pub stopped_reason: String,
}

pub fn run_single_iteration(state: &mut AppState) -> Result<Option<LoopRunSummary>> {
    run_single_iteration_with_hook(state, &mut |_state| Ok(()))
}

pub fn run_single_iteration_with_hook<F>(
    state: &mut AppState,
    on_state_change: &mut F,
) -> Result<Option<LoopRunSummary>>
where
    F: FnMut(&AppState) -> Result<()>,
{
    logging::debug_event(
        "loop.single_iteration.start",
        "starting single loop iteration",
        serde_json::json!({
            "has_ready_todo": state.next_ready_todo_id().is_some(),
        }),
    );
    let Some(todo_id) = state.next_ready_todo_id() else {
        state.push_status_message("no ready todo available");
        state.set_loop_phase(LoopPhase::Idle);
        logging::debug_event(
            "loop.stop",
            "stopping loop because no ready todo is available",
            serde_json::json!({
                "stopped_reason": "no ready todo available",
                "iterations": 0,
                "verification_failures": 0,
            }),
        );
        return Ok(None);
    };

    state.set_loop_phase(LoopPhase::Planning);
    let Some(task) = state.begin_task_from_todo(&todo_id) else {
        anyhow::bail!("failed to start task from todo: {todo_id}");
    };
    state.push_status_message(format!("running task: {}", task.id));
    on_state_change(state)?;
    execute_task_until_resolution(state, &task, LoopGuardrails::default(), on_state_change)?;
    logging::debug_event(
        "loop.single_iteration.stop",
        "completed single loop iteration",
        serde_json::json!({
            "task_id": task.id,
        }),
    );

    Ok(Some(LoopRunSummary {
        iterations: 1,
        verification_failures: 0,
        stopped_reason: "single iteration completed".to_string(),
    }))
}

pub fn run_loop(state: &mut AppState, guardrails: LoopGuardrails) -> Result<LoopRunSummary> {
    run_loop_with_hook(state, guardrails, &mut |_state| Ok(()))
}

pub fn run_loop_with_hook<F>(
    state: &mut AppState,
    guardrails: LoopGuardrails,
    on_state_change: &mut F,
) -> Result<LoopRunSummary>
where
    F: FnMut(&AppState) -> Result<()>,
{
    let mut iterations = 0usize;
    let mut verification_failures = 0usize;
    logging::debug_event(
        "loop.start",
        "starting autonomous loop",
        serde_json::json!({
            "max_iterations": guardrails.max_iterations,
            "max_continuations_per_task": guardrails.max_continuations_per_task,
            "max_verification_failures": guardrails.max_verification_failures,
        }),
    );

    loop {
        if iterations >= guardrails.max_iterations {
            state.set_loop_phase(LoopPhase::Escalating);
            state.push_error_message("loop guardrail reached: max iterations");
            logging::warn_event(
                "loop.stop",
                "stopping loop because max iterations was reached",
                serde_json::json!({
                    "stopped_reason": "max iterations reached",
                    "iterations": iterations,
                    "verification_failures": verification_failures,
                }),
            );
            return Ok(LoopRunSummary {
                iterations,
                verification_failures,
                stopped_reason: "max iterations reached".to_string(),
            });
        }

        let task = if let Some(task) = task_engine::current_active_task(state) {
            state.push_status_message(format!("resuming task: {}", task.id));
            task
        } else {
            let Some(todo_id) = state.next_ready_todo_id() else {
                state.set_loop_phase(LoopPhase::Idle);
                logging::debug_event(
                    "loop.stop",
                    "stopping loop because no ready todo is available",
                    serde_json::json!({
                        "stopped_reason": "no ready todo available",
                        "iterations": iterations,
                        "verification_failures": verification_failures,
                    }),
                );
                return Ok(LoopRunSummary {
                    iterations,
                    verification_failures,
                    stopped_reason: "no ready todo available".to_string(),
                });
            };

            state.set_loop_phase(LoopPhase::Planning);
            let Some(task) = state.begin_task_from_todo(&todo_id) else {
                anyhow::bail!("failed to start task from todo: {todo_id}");
            };
            state.push_status_message(format!("running task: {}", task.id));
            on_state_change(state)?;
            task
        };
        logging::debug_event(
            "loop.iteration.start",
            "starting loop iteration",
            serde_json::json!({
                "iteration": iterations + 1,
                "task_id": task.id,
                "todo_id": task.todo_id,
            }),
        );

        let outcome = execute_task_until_resolution(state, &task, guardrails, on_state_change)?;
        if outcome == LoopExecutionOutcome::VerificationFailed {
            verification_failures += 1;
            if verification_failures >= guardrails.max_verification_failures {
                state.set_loop_phase(LoopPhase::Escalating);
                state.push_error_message("loop guardrail reached: max verification failures");
                logging::warn_event(
                    "loop.stop",
                    "stopping loop because max verification failures was reached",
                    serde_json::json!({
                        "stopped_reason": "max verification failures reached",
                        "iterations": iterations + 1,
                        "verification_failures": verification_failures,
                    }),
                );
                return Ok(LoopRunSummary {
                    iterations: iterations + 1,
                    verification_failures,
                    stopped_reason: "max verification failures reached".to_string(),
                });
            }
        }
        iterations += 1;
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LoopExecutionOutcome {
    Completed,
    VerificationFailed,
    Escalated,
}

fn execute_task_until_resolution(
    state: &mut AppState,
    task: &TaskItem,
    guardrails: LoopGuardrails,
    on_state_change: &mut impl FnMut(&AppState) -> Result<()>,
) -> Result<LoopExecutionOutcome> {
    let mut prompt = task_engine::build_task_prompt(task);
    let execution_guardrails = ExecutionGuardrails {
        max_continuations_per_task: guardrails.max_continuations_per_task,
    };

    loop {
        state.set_loop_phase(LoopPhase::Executing);
        state.mark_active_task_running();
        state.begin_provider_response();
        on_state_change(state)?;

        let (event_tx, event_rx) = mpsc::channel();
        let provider_kind = state.selected_provider;
        let session_handle = state.current_session_handle();
        if let Err(err) = crate::start_provider(
            provider_kind,
            prompt.clone(),
            state.cwd.clone(),
            session_handle,
            event_tx,
        ) {
            state.finish_provider_response();
            task_engine::handle_provider_start_failure(state, err.to_string());
            on_state_change(state)?;
            return Ok(LoopExecutionOutcome::Escalated);
        }

        consume_provider_until_finished(state, event_rx, on_state_change)?;
        let resolution = task_engine::resolve_active_task_after_turn(state, execution_guardrails)?;
        on_state_change(state)?;
        match resolution {
            TurnResolution::Continue {
                prompt: next_prompt,
            } => {
                prompt = next_prompt;
                continue;
            }
            TurnResolution::Completed => return Ok(LoopExecutionOutcome::Completed),
            TurnResolution::Failed {
                verification_failed: true,
            } => return Ok(LoopExecutionOutcome::VerificationFailed),
            TurnResolution::Failed {
                verification_failed: false,
            } => return Ok(LoopExecutionOutcome::Completed),
            TurnResolution::Escalated => return Ok(LoopExecutionOutcome::Escalated),
            TurnResolution::Idle => return Ok(LoopExecutionOutcome::Completed),
        }
    }
}

fn consume_provider_until_finished(
    state: &mut AppState,
    rx: mpsc::Receiver<ProviderEvent>,
    on_state_change: &mut impl FnMut(&AppState) -> Result<()>,
) -> Result<()> {
    loop {
        match rx.recv_timeout(Duration::from_millis(100)) {
            Ok(event) => match event {
                ProviderEvent::Status(text) => state.push_status_message(text),
                ProviderEvent::AssistantChunk(chunk) => state.append_assistant_chunk(&chunk),
                ProviderEvent::ThinkingChunk(chunk) => state.append_thinking_chunk(&chunk),
                ProviderEvent::ExecCommandStarted {
                    call_id,
                    input_preview,
                    source,
                } => state.push_exec_command_started(call_id, input_preview, source),
                ProviderEvent::ExecCommandFinished {
                    call_id,
                    output_preview,
                    status,
                    exit_code,
                    duration_ms,
                    source,
                } => state.push_exec_command_finished(
                    call_id,
                    output_preview,
                    status,
                    exit_code,
                    duration_ms,
                    source,
                ),
                ProviderEvent::ExecCommandOutputDelta { call_id, delta } => {
                    state.append_exec_command_output(call_id, &delta)
                }
                ProviderEvent::GenericToolCallStarted {
                    name,
                    call_id,
                    input_preview,
                } => state.push_generic_tool_call_started(name, call_id, input_preview),
                ProviderEvent::GenericToolCallFinished {
                    name,
                    call_id,
                    output_preview,
                    success,
                    exit_code,
                    duration_ms,
                } => state.push_generic_tool_call_finished(
                    name,
                    call_id,
                    output_preview,
                    success,
                    exit_code,
                    duration_ms,
                ),
                ProviderEvent::WebSearchStarted { call_id, query } => {
                    state.push_web_search_started(call_id, query)
                }
                ProviderEvent::WebSearchFinished {
                    call_id,
                    query,
                    action,
                } => state.push_web_search_finished(call_id, query, action),
                ProviderEvent::ViewImage { call_id, path } => state.push_view_image(call_id, path),
                ProviderEvent::ImageGenerationFinished {
                    call_id,
                    revised_prompt,
                    result,
                    saved_path,
                } => state.push_image_generation(call_id, revised_prompt, result, saved_path),
                ProviderEvent::McpToolCallStarted {
                    call_id,
                    invocation,
                } => state.push_mcp_tool_call_started(call_id, invocation),
                ProviderEvent::McpToolCallFinished {
                    call_id,
                    invocation,
                    result_blocks,
                    error,
                    status,
                    is_error,
                } => state.push_mcp_tool_call_finished(
                    call_id,
                    invocation,
                    result_blocks,
                    error,
                    status,
                    is_error,
                ),
                ProviderEvent::PatchApplyStarted { call_id, changes } => {
                    state.push_patch_apply_started(call_id, changes)
                }
                ProviderEvent::PatchApplyOutputDelta { call_id, delta } => {
                    state.append_patch_apply_output(call_id, &delta)
                }
                ProviderEvent::PatchApplyFinished {
                    call_id,
                    changes,
                    status,
                } => state.push_patch_apply_finished(call_id, changes, status),
                ProviderEvent::SessionHandle(handle) => {
                    state.apply_session_handle(handle);
                    on_state_change(state)?;
                }
                ProviderEvent::Error(error) => {
                    state.mark_active_task_error();
                    // Check if this is a session expiry error and clear the session handle
                    if is_session_expired_error(&error) {
                        state.clear_session();
                        state.push_error_message(
                            "session expired - starting fresh conversation. Please retry your request."
                        );
                    } else {
                        state.push_error_message(error);
                    }
                    on_state_change(state)?;
                }
                ProviderEvent::Finished => {
                    state.finish_provider_response();
                    on_state_change(state)?;
                    break;
                }
            },
            Err(mpsc::RecvTimeoutError::Timeout) => continue,
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                state.mark_active_task_error();
                state.push_error_message("provider event stream disconnected");
                state.finish_provider_response();
                on_state_change(state)?;
                break;
            }
        }
    }

    Ok(())
}

/// Check if the error indicates an expired or invalid session/conversation
fn is_session_expired_error(error: &str) -> bool {
    // Claude CLI returns this when --resume session ID doesn't exist
    error.contains("No conversation found with session ID")
        || error.contains("No conversation found")
        // Codex may have similar error patterns
        || error.contains("thread not found")
        || error.contains("session not found")
}

#[cfg(test)]
mod tests {
    use super::LoopGuardrails;
    use super::is_session_expired_error;
    use super::run_loop;
    use super::run_single_iteration;
    use crate::app::AppState;
    use crate::backlog::TodoItem;
    use crate::backlog::TodoStatus;
    use crate::logging;
    use crate::logging::RunMode;
    use crate::ProviderKind;
    use crate::skills::SkillRegistry;
    use crate::workplace_store::WorkplaceStore;

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
                max_verification_failures: 1,
            },
        )
        .expect("run loop");

        assert_eq!(summary.iterations, 1);
        assert_eq!(summary.verification_failures, 0);
        assert_eq!(summary.stopped_reason, "no ready todo available");
        assert_eq!(state.backlog.todos[0].status, TodoStatus::Done);
    }

    #[test]
    fn run_loop_logs_iteration_boundaries_and_stop_reason() {
        let _guard = logging::test_guard();
        let temp = tempfile::TempDir::new().expect("tempdir");
        let workplace = WorkplaceStore::for_cwd(temp.path()).expect("workplace");
        workplace.ensure().expect("ensure");
        logging::init_for_workplace(&workplace, RunMode::RunLoop).expect("init logger");

        let mut state = AppState::with_skills(
            ProviderKind::Mock,
            temp.path().into(),
            SkillRegistry::default(),
        );
        state
            .backlog
            .push_todo(ready_todo("todo-1", "write summary", 1));

        let summary = run_loop(
            &mut state,
            LoopGuardrails {
                max_iterations: 2,
                max_continuations_per_task: 1,
                max_verification_failures: 1,
            },
        )
        .expect("run loop");

        assert_eq!(summary.stopped_reason, "no ready todo available");

        let log_path = logging::current_log_path().expect("log path");
        let contents = std::fs::read_to_string(log_path).expect("log file");
        assert!(contents.contains("\"event\":\"loop.start\""));
        assert!(contents.contains("\"event\":\"loop.iteration.start\""));
        assert!(contents.contains("\"event\":\"task.complete\""));
        assert!(contents.contains("\"event\":\"loop.stop\""));
    }

    #[test]
    fn run_loop_can_resume_existing_active_task() {
        let mut state =
            AppState::with_skills(ProviderKind::Mock, ".".into(), SkillRegistry::default());
        state
            .backlog
            .push_todo(ready_todo("todo-1", "write summary", 1));
        let task = state.begin_task_from_todo("todo-1").expect("task");
        state.active_task_id = Some(task.id.clone());

        let summary = run_loop(
            &mut state,
            LoopGuardrails {
                max_iterations: 1,
                max_continuations_per_task: 1,
                max_verification_failures: 1,
            },
        )
        .expect("run loop");

        assert_eq!(summary.iterations, 1);
        assert_eq!(
            state.backlog.tasks[0].status,
            crate::backlog::TaskStatus::Done
        );
    }

    #[test]
    fn is_session_expired_error_detects_known_patterns() {
        assert!(is_session_expired_error(
            "No conversation found with session ID: abc123"
        ));
        assert!(is_session_expired_error("No conversation found"));
        assert!(is_session_expired_error("thread not found"));
        assert!(is_session_expired_error("session not found"));
        assert!(!is_session_expired_error("provider crashed"));
        assert!(!is_session_expired_error("network error"));
    }
}
