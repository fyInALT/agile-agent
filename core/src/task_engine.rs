use std::path::PathBuf;

use anyhow::Result;
use chrono::Utc;

use crate::app::AppState;
use crate::app::LoopPhase;
use crate::autonomy;
use crate::autonomy::CompletionDecision;
use crate::backlog::TaskItem;
use crate::escalation;
use crate::escalation::EscalationRecord;
use crate::logging;
use crate::task_artifacts;
use crate::task_artifacts::TaskArtifact;
use crate::task_artifacts::TaskArtifactOutcome;
use crate::verification;
use crate::verification::VerificationOutcome;
use crate::verification::VerificationResult;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExecutionGuardrails {
    pub max_continuations_per_task: u8,
}

impl Default for ExecutionGuardrails {
    fn default() -> Self {
        Self {
            max_continuations_per_task: 3,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TurnResolution {
    Continue { prompt: String },
    Completed,
    Failed { verification_failed: bool },
    Escalated,
    Idle,
}

pub fn build_task_prompt(task: &TaskItem) -> String {
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

    logging::debug_event(
        "task.prompt.build",
        "built task prompt",
        serde_json::json!({
            "task_id": task.id,
            "todo_id": task.todo_id,
            "prompt": prompt,
        }),
    );

    prompt
}

pub fn current_active_task(state: &AppState) -> Option<TaskItem> {
    let active_task_id = state.active_task_id.as_ref()?;
    state
        .backlog
        .tasks
        .iter()
        .find(|task| &task.id == active_task_id)
        .cloned()
}

pub fn handle_provider_start_failure(state: &mut AppState, reason: impl Into<String>) {
    let reason = reason.into();
    logging::error_event(
        "task.provider_start_failed",
        "provider failed to start for task execution",
        serde_json::json!({
            "reason": reason,
            "active_task_id": state.active_task_id,
        }),
    );
    if let Some(task) = current_active_task(state) {
        state.set_loop_phase(LoopPhase::Escalating);
        let escalation_path =
            escalate_active_task(state, format!("provider start failed: {reason}"))
                .ok()
                .flatten();
        let artifact_path = save_task_artifact(
            &task,
            state,
            TaskArtifactOutcome::Escalated,
            Some(reason.clone()),
            None,
            escalation_path.as_ref(),
        )
        .ok();
        state.push_error_message(format!("failed to start provider: {reason}"));
        if let Some(path) = artifact_path {
            state.push_status_message(format!("task artifact: {}", path.display()));
        }
    } else {
        state.push_error_message(format!("failed to start provider: {reason}"));
        state.set_loop_phase(LoopPhase::Idle);
    }
}

pub fn resolve_active_task_after_turn(
    state: &mut AppState,
    guardrails: ExecutionGuardrails,
) -> Result<TurnResolution> {
    let Some(task) = current_active_task(state) else {
        state.set_loop_phase(LoopPhase::Idle);
        return Ok(TurnResolution::Idle);
    };

    let summary = state.active_task_summary();
    if state.active_task_had_error {
        state.set_loop_phase(LoopPhase::Idle);
        state.fail_active_task("provider execution failed");
        let artifact_path = save_task_artifact(
            &task,
            state,
            TaskArtifactOutcome::Failed,
            Some("provider execution failed".to_string()),
            None,
            None,
        )?;
        state.push_error_message(format!(
            "failed task: {} (provider execution failed)",
            task.id
        ));
        state.push_status_message(format!("task artifact: {}", artifact_path.display()));
        return Ok(TurnResolution::Failed {
            verification_failed: false,
        });
    }

    let Some(summary_text) = summary.clone() else {
        state.set_loop_phase(LoopPhase::Idle);
        state.fail_active_task("provider returned no assistant summary");
        let artifact_path = save_task_artifact(
            &task,
            state,
            TaskArtifactOutcome::Failed,
            Some("provider returned no assistant summary".to_string()),
            None,
            None,
        )?;
        state.push_error_message(format!(
            "failed task: {} (provider returned no assistant summary)",
            task.id
        ));
        state.push_status_message(format!("task artifact: {}", artifact_path.display()));
        return Ok(TurnResolution::Failed {
            verification_failed: false,
        });
    };

    if let Some(next_prompt) = autonomy::continuation_prompt(&summary_text) {
        if state.continuation_attempts >= guardrails.max_continuations_per_task {
            state.set_loop_phase(LoopPhase::Escalating);
            let escalation_path = escalate_active_task(state, "continuation limit reached")?;
            let artifact_path = save_task_artifact(
                &task,
                state,
                TaskArtifactOutcome::Escalated,
                Some("continuation limit reached".to_string()),
                None,
                escalation_path.as_ref(),
            )?;
            state.push_status_message(format!("task artifact: {}", artifact_path.display()));
            logging::warn_event(
                "task.escalate",
                "escalated task because continuation limit was reached",
                serde_json::json!({
                    "task_id": task.id,
                    "reason": "continuation limit reached",
                }),
            );
            return Ok(TurnResolution::Escalated);
        }

        state.continuation_attempts += 1;
        state.set_loop_phase(LoopPhase::Executing);
        state.push_status_message(format!(
            "continuing active task automatically (attempt {})",
            state.continuation_attempts
        ));
        return Ok(TurnResolution::Continue {
            prompt: next_prompt,
        });
    }

    match autonomy::judge_completion(&task, &summary_text) {
        CompletionDecision::ReadyForVerification => {
            state.set_loop_phase(LoopPhase::Verifying);
            state.mark_active_task_verifying();
            let plan = verification::build_verification_plan(&state.cwd, &task);
            let result = verification::execute_verification(&plan, &state.cwd, Some(&summary_text));
            record_verification_messages(state, &result);
            logging::debug_event(
                "task.verify",
                "verification finished",
                serde_json::json!({
                    "task_id": task.id,
                    "summary": result.summary,
                    "outcome": format!("{:?}", result.outcome),
                }),
            );
            match result.outcome {
                VerificationOutcome::Passed => {
                    state.complete_active_task(summary);
                    state.set_loop_phase(LoopPhase::Idle);
                    let artifact_path = save_task_artifact(
                        &task,
                        state,
                        TaskArtifactOutcome::Completed,
                        None,
                        Some(result),
                        None,
                    )?;
                    state
                        .push_status_message(format!("task artifact: {}", artifact_path.display()));
                    logging::debug_event(
                        "task.complete",
                        "completed task after successful verification",
                        serde_json::json!({
                            "task_id": task.id,
                            "artifact_path": artifact_path.display().to_string(),
                        }),
                    );
                    Ok(TurnResolution::Completed)
                }
                VerificationOutcome::Failed | VerificationOutcome::NotRunnable => {
                    let failure_reason = result.summary.clone();
                    state.fail_active_task(failure_reason.clone());
                    state.set_loop_phase(LoopPhase::Idle);
                    let artifact_path = save_task_artifact(
                        &task,
                        state,
                        TaskArtifactOutcome::Failed,
                        Some(failure_reason.clone()),
                        Some(result),
                        None,
                    )?;
                    state.push_error_message(format!(
                        "failed task: {} ({})",
                        task.id, failure_reason
                    ));
                    state
                        .push_status_message(format!("task artifact: {}", artifact_path.display()));
                    logging::warn_event(
                        "task.fail",
                        "failed task during verification",
                        serde_json::json!({
                            "task_id": task.id,
                            "reason": failure_reason,
                            "artifact_path": artifact_path.display().to_string(),
                        }),
                    );
                    Ok(TurnResolution::Failed {
                        verification_failed: true,
                    })
                }
            }
        }
        CompletionDecision::Incomplete { reason } => {
            state.set_loop_phase(LoopPhase::Escalating);
            let escalation_path = escalate_active_task(state, reason.clone())?;
            let artifact_path = save_task_artifact(
                &task,
                state,
                TaskArtifactOutcome::Escalated,
                Some(reason.clone()),
                None,
                escalation_path.as_ref(),
            )?;
            state.push_status_message(format!("task artifact: {}", artifact_path.display()));
            logging::warn_event(
                "task.escalate",
                "escalated incomplete task",
                serde_json::json!({
                    "task_id": task.id,
                    "reason": reason,
                    "artifact_path": artifact_path.display().to_string(),
                }),
            );
            Ok(TurnResolution::Escalated)
        }
    }
}

fn record_verification_messages(state: &mut AppState, result: &VerificationResult) {
    state.push_status_message(result.summary.clone());
    for evidence in &result.evidence {
        state.push_status_message(format!("evidence: {}", evidence));
    }
}

fn save_task_artifact(
    task: &TaskItem,
    state: &AppState,
    outcome: TaskArtifactOutcome,
    reason: Option<String>,
    verification: Option<VerificationResult>,
    escalation_path: Option<&PathBuf>,
) -> Result<PathBuf> {
    let artifact = TaskArtifact {
        saved_at: Utc::now().to_rfc3339(),
        task_id: task.id.clone(),
        todo_id: task.todo_id.clone(),
        objective: task.objective.clone(),
        provider: state.selected_provider,
        outcome,
        assistant_summary: state.active_task_summary(),
        verification,
        reason,
        escalation_path: escalation_path.map(|path| path.display().to_string()),
    };
    if let Some(root) = state.agent_storage_root.as_ref() {
        task_artifacts::save_task_artifact_under(root, &artifact)
    } else {
        task_artifacts::save_task_artifact(&artifact)
    }
}

fn escalate_active_task(
    state: &mut AppState,
    reason: impl Into<String>,
) -> Result<Option<PathBuf>> {
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
        created_at: Utc::now().to_rfc3339(),
    };

    let artifact_path = state
        .agent_storage_root
        .as_ref()
        .map(|root| escalation::save_escalation_under(root, &record))
        .transpose()
        .ok()
        .flatten()
        .or_else(|| escalation::save_escalation(&record).ok());
    state.block_active_task(reason.clone());
    state.push_error_message(format!("escalated task: {} ({})", task_id, reason));
    if let Some(path) = artifact_path.as_ref() {
        state.push_status_message(format!("escalation artifact: {}", path.display()));
    }
    Ok(artifact_path)
}
