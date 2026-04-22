//! Decision action executor for executing decision layer outputs
//!
//! Provides DecisionExecutor that handles execution of decision layer
//! actions on work agents. This module extracts the execute_decision_action
//! logic from AgentPool to improve code organization.

use crate::agent_runtime::AgentId;
use crate::agent_slot::{AgentSlot, AgentSlotStatus};
use crate::logging;
use crate::pool::{DecisionExecutionResult, WorktreeCoordinator};
use agent_decision::{DecisionCommand, HumanDecisionQueue, HumanDecisionResponse, HumanSelection};

/// Decision action executor - executes decision layer outputs
///
/// This struct provides methods to execute decision actions on work agents.
/// It operates on slots and other pool components.
pub struct DecisionExecutor;

impl DecisionExecutor {
    /// Translate a decision output into pure `DecisionCommand` values.
    ///
    /// This is a pure function — it does not mutate any state. The returned
    /// commands must be interpreted and executed by the EventLoop.
    pub fn translate(
        work_agent_id: &AgentId,
        output: &agent_decision::output::DecisionOutput,
    ) -> Vec<DecisionCommand> {
        let mut commands = Vec::new();

        for action in &output.actions {
            let action_type_name = action.action_type().name;
            let action_type = action_type_name.as_str();
            let params_str = action.serialize_params();
            let params: serde_json::Value =
                serde_json::from_str(&params_str).unwrap_or(serde_json::json!({}));

            let cmd = match action_type {
                "select_option" => {
                    let option_id = params["option_id"].as_str().unwrap_or("a").to_string();
                    DecisionCommand::SelectOption { option_id }
                }
                "skip" => DecisionCommand::SkipDecision,
                "request_human" => DecisionCommand::EscalateToHuman {
                    reason: "awaiting_human".to_string(),
                    context: None,
                },
                "custom_instruction" => {
                    let instruction = params["instruction"].as_str().unwrap_or("").to_string();
                    DecisionCommand::SendCustomInstruction {
                        prompt: instruction,
                        target_agent: work_agent_id.as_str().to_string(),
                    }
                }
                "continue" => DecisionCommand::ApproveAndContinue,
                "reflect" => {
                    let prompt = params["prompt"]
                        .as_str()
                        .unwrap_or("Please verify your work is complete.")
                        .to_string();
                    DecisionCommand::Reflect { prompt }
                }
                "confirm_completion" => DecisionCommand::ConfirmCompletion,
                "retry" => {
                    let prompt = params["prompt"]
                        .as_str()
                        .unwrap_or("Please retry the previous action.")
                        .to_string();
                    DecisionCommand::SendCustomInstruction {
                        prompt,
                        target_agent: work_agent_id.as_str().to_string(),
                    }
                }
                "continue_all_tasks" => {
                    let instruction = params["instruction"]
                        .as_str()
                        .unwrap_or("continue finish all tasks")
                        .to_string();
                    DecisionCommand::SendCustomInstruction {
                        prompt: instruction,
                        target_agent: work_agent_id.as_str().to_string(),
                    }
                }
                "stop_if_complete" => {
                    let reason = params["reason"]
                        .as_str()
                        .unwrap_or("All tasks complete")
                        .to_string();
                    DecisionCommand::StopIfComplete { reason }
                }
                "prepare_task_start" => {
                    let task_id = params["task_id"].as_str().unwrap_or("unknown").to_string();
                    let task_description = params["task_description"]
                        .as_str()
                        .unwrap_or("")
                        .to_string();
                    DecisionCommand::PrepareTaskStart {
                        task_id,
                        task_description,
                    }
                }
                "wake_up" => DecisionCommand::WakeUp,
                "commit_changes" => {
                    let message = params["commit_message"].as_str().unwrap_or("").to_string();
                    let is_wip = params["is_wip"].as_bool().unwrap_or(false);
                    let worktree_path = params["worktree_path"].as_str().map(String::from);
                    DecisionCommand::CommitChanges {
                        message,
                        is_wip,
                        worktree_path,
                    }
                }
                "stash_changes" => {
                    let description = params["description"].as_str().unwrap_or("").to_string();
                    let include_untracked = params["include_untracked"].as_bool().unwrap_or(true);
                    let worktree_path = params["worktree_path"].as_str().map(String::from);
                    DecisionCommand::StashChanges {
                        description,
                        include_untracked,
                        worktree_path,
                    }
                }
                "discard_changes" => {
                    let worktree_path = params["worktree_path"].as_str().map(String::from);
                    DecisionCommand::DiscardChanges { worktree_path }
                }
                "suggest_commit" => {
                    let message = params["suggested_message"].as_str().unwrap_or("").to_string();
                    let mandatory = params["mandatory"].as_bool().unwrap_or(false);
                    let reason = params["reason"].as_str().unwrap_or("").to_string();
                    DecisionCommand::SuggestCommit {
                        message,
                        mandatory,
                        reason,
                    }
                }
                "prepare_pr" => {
                    let title = params["title"].as_str().unwrap_or("").to_string();
                    let description = params["description"].as_str().unwrap_or("").to_string();
                    let base_branch = params["base_branch"].as_str().unwrap_or("main").to_string();
                    let as_draft = params["as_draft"].as_bool().unwrap_or(true);
                    DecisionCommand::PreparePr {
                        title,
                        description,
                        base_branch,
                        as_draft,
                    }
                }
                "create_task_branch" => {
                    let branch_name = params["branch_name"].as_str().unwrap_or("").to_string();
                    let base_branch = params["base_branch"].as_str().unwrap_or("main").to_string();
                    let worktree_path = params["worktree_path"].as_str().map(String::from);
                    DecisionCommand::CreateTaskBranch {
                        branch_name,
                        base_branch,
                        worktree_path,
                    }
                }
                "rebase_to_main" => {
                    let base_branch = params["base_branch"].as_str().unwrap_or("main").to_string();
                    DecisionCommand::RebaseToMain { base_branch }
                }
                _ => DecisionCommand::Unknown {
                    action_type: action_type.to_string(),
                    params: params_str,
                },
            };
            commands.push(cmd);
        }

        commands
    }

    /// Execute a decision action on a work agent
    ///
    /// Takes decision output and executes the appropriate action on the agent.
    /// Returns execution result indicating what happened.
    ///
    /// **Deprecated**: This method performs side effects directly. Prefer using
    /// `translate()` to get pure `DecisionCommand` values, then interpret them
    /// through the EventLoop's effect system.
    pub fn execute(
        slots: &mut [AgentSlot],
        human_queue: &mut HumanDecisionQueue,
        worktree_coordinator: &WorktreeCoordinator,
        work_agent_id: &AgentId,
        output: &agent_decision::output::DecisionOutput,
    ) -> DecisionExecutionResult {
        // Find the work agent
        let slot_index = slots.iter()
            .position(|s| s.agent_id() == work_agent_id);

        let slot_index = match slot_index {
            Some(idx) => idx,
            None => return DecisionExecutionResult::AgentNotFound,
        };

        let slot = &mut slots[slot_index];

        // Check if agent is blocked (most decisions require blocked state)
        // Allow idle state and waiting_for_input state for some decisions like continue_all_tasks
        if !slot.status().is_blocked()
            && !slot.status().is_idle()
            && !slot.status().is_waiting_for_input()
        {
            return DecisionExecutionResult::NotBlocked;
        }

        // Execute the first action from the output
        if let Some(action) = output.actions.first() {
            let action_type = action.action_type().name.clone();
            let params_str = action.serialize_params();

            // Log: Decision action execution started
            logging::debug_event(
                "decision_layer.action_executing",
                "executing decision action on work agent",
                serde_json::json!({
                    "work_agent_id": work_agent_id.as_str(),
                    "action_type": action_type,
                    "action_params": params_str,
                    "reasoning": output.reasoning,
                    "confidence": output.confidence,
                }),
            );

            match action_type.as_str() {
                "select_option" => Self::execute_select_option(
                    slots,
                    human_queue,
                    work_agent_id,
                    params_str,
                ),
                "skip" => Self::execute_skip(
                    slots,
                    human_queue,
                    work_agent_id,
                ),
                "request_human" => Self::execute_request_human(work_agent_id),
                "custom_instruction" => Self::execute_custom_instruction(
                    slots,
                    slot_index,
                    params_str,
                ),
                "continue" => Self::execute_continue(
                    slots,
                    slot_index,
                ),
                "reflect" => Self::execute_reflect(
                    slots,
                    slot_index,
                    params_str,
                ),
                "confirm_completion" => Self::execute_confirm_completion(
                    slots,
                    slot_index,
                ),
                "retry" => Self::execute_retry(
                    slots,
                    slot_index,
                    params_str,
                ),
                "continue_all_tasks" => Self::execute_continue_all_tasks(
                    slots,
                    slot_index,
                    params_str,
                ),
                "stop_if_complete" => Self::execute_stop_if_complete(
                    slots,
                    slot_index,
                    params_str,
                ),
                "prepare_task_start" => Self::execute_prepare_task_start(
                    slots,
                    slot_index,
                    worktree_coordinator,
                    params_str,
                ),
                _ => Self::execute_unknown(work_agent_id, action_type, params_str),
            }
        } else {
            // No actions in output - nothing to execute
            logging::warn_event(
                "decision_layer.no_actions",
                "decision output has no actions - nothing to execute",
                serde_json::json!({
                    "work_agent_id": work_agent_id.as_str(),
                    "reasoning": output.reasoning,
                }),
            );
            DecisionExecutionResult::Cancelled
        }
    }

    fn execute_select_option(
        slots: &mut [AgentSlot],
        human_queue: &mut HumanDecisionQueue,
        work_agent_id: &AgentId,
        params_str: String,
    ) -> DecisionExecutionResult {
        let params: serde_json::Value =
            serde_json::from_str(&params_str).unwrap_or(serde_json::json!({}));
        let option_id = params["option_id"].as_str().unwrap_or("a").to_string();

        // Execute the selection - find pending request for THIS agent
        let pending_request = human_queue.find_by_agent_id(work_agent_id.as_str());
        if let Some(request) = pending_request {
            // Verify this request belongs to our agent (double-check)
            if request.agent_id != work_agent_id.as_str() {
                logging::warn_event(
                    "decision_layer.agent_mismatch",
                    "request agent_id mismatch",
                    serde_json::json!({
                        "work_agent_id": work_agent_id.as_str(),
                        "request_agent_id": request.agent_id,
                    }),
                );
                return DecisionExecutionResult::Cancelled;
            }

            // Create response with the selection
            let selection = HumanSelection::selected(option_id.clone());
            let response = HumanDecisionResponse::new(request.id.clone(), selection);

            // Process the response (this will update the slot status)
            Self::process_human_response_internal(slots, human_queue, response, work_agent_id);

            // Log: Selection executed
            logging::debug_event(
                "decision_layer.action_completed",
                "select_option action executed",
                serde_json::json!({
                    "work_agent_id": work_agent_id.as_str(),
                    "action_type": "select_option",
                    "option_id": option_id,
                }),
            );
            DecisionExecutionResult::Executed { option_id }
        } else {
            // No pending request for this agent - might not be blocked correctly
            logging::warn_event(
                "decision_layer.no_pending_request",
                "no pending request for this agent",
                serde_json::json!({
                    "work_agent_id": work_agent_id.as_str(),
                }),
            );
            DecisionExecutionResult::NotBlocked
        }
    }

    fn execute_skip(
        slots: &mut [AgentSlot],
        human_queue: &mut HumanDecisionQueue,
        work_agent_id: &AgentId,
    ) -> DecisionExecutionResult {
        // Skip the current task for THIS agent
        let pending_request = human_queue.find_by_agent_id(work_agent_id.as_str());
        if let Some(request) = pending_request {
            let response =
                HumanDecisionResponse::new(request.id.clone(), HumanSelection::skip());
            Self::process_human_response_internal(slots, human_queue, response, work_agent_id);
        }
        // Log: Skip action executed
        logging::debug_event(
            "decision_layer.action_completed",
            "skip action executed",
            serde_json::json!({
                "work_agent_id": work_agent_id.as_str(),
                "action_type": "skip",
            }),
        );
        DecisionExecutionResult::Skipped
    }

    fn execute_request_human(work_agent_id: &AgentId) -> DecisionExecutionResult {
        // Already in human decision queue - nothing additional to do
        // Log: Request human action
        logging::debug_event(
            "decision_layer.action_completed",
            "request_human action - awaiting human input",
            serde_json::json!({
                "work_agent_id": work_agent_id.as_str(),
                "action_type": "request_human",
                "agent_status": "blocked_for_human",
            }),
        );
        DecisionExecutionResult::AcceptedRecommendation
    }

    fn execute_custom_instruction(
        slots: &mut [AgentSlot],
        slot_index: usize,
        params_str: String,
    ) -> DecisionExecutionResult {
        let params: serde_json::Value =
            serde_json::from_str(&params_str).unwrap_or(serde_json::json!({}));
        let instruction = params["instruction"].as_str().unwrap_or("").to_string();

        // Store instruction for the agent to use in next turn
        let slot = &mut slots[slot_index];
        if !instruction.is_empty() {
            slot.append_transcript(crate::app::TranscriptEntry::User(instruction.clone()));
        }
        // Log: Work agent prompt sent
        logging::debug_event(
            "decision_layer.work_agent_prompt",
            "custom instruction sent to work agent",
            serde_json::json!({
                "work_agent_id": slot.agent_id().as_str(),
                "prompt_type": "custom_instruction",
                "instruction": instruction,
            }),
        );
        DecisionExecutionResult::CustomInstruction { instruction }
    }

    fn execute_continue(
        slots: &mut [AgentSlot],
        slot_index: usize,
    ) -> DecisionExecutionResult {
        let slot = &mut slots[slot_index];
        // Continue with normal processing - agent should transition to idle
        // Handle Resting state (rate limit recovery) or blocked state
        if matches!(slot.status(), AgentSlotStatus::Resting { .. }) || slot.status().is_blocked() {
            let _ = slot.transition_to(AgentSlotStatus::idle());
        }
        // Log: Continue action executed
        logging::debug_event(
            "decision_layer.action_completed",
            "continue action executed - agent transitioning to idle",
            serde_json::json!({
                "work_agent_id": slot.agent_id().as_str(),
                "action_type": "continue",
                "agent_status_after": "idle",
            }),
        );
        DecisionExecutionResult::AcceptedRecommendation
    }

    fn execute_reflect(
        slots: &mut [AgentSlot],
        slot_index: usize,
        params_str: String,
    ) -> DecisionExecutionResult {
        let params: serde_json::Value =
            serde_json::from_str(&params_str).unwrap_or(serde_json::json!({}));
        let prompt = params["prompt"]
            .as_str()
            .unwrap_or("Please verify your work is complete.")
            .to_string();

        let slot = &mut slots[slot_index];
        // Add reflection prompt as a user message to trigger verification
        slot.append_transcript(crate::app::TranscriptEntry::User(format!(
            "Reflect: {}",
            prompt
        )));

        // Transition agent back to idle so it can process the reflection prompt
        if slot.status().is_blocked() {
            let _ = slot.transition_to(AgentSlotStatus::idle());
        }

        // Log: Work agent prompt sent
        logging::debug_event(
            "decision_layer.work_agent_prompt",
            "reflection prompt sent to work agent",
            serde_json::json!({
                "work_agent_id": slot.agent_id().as_str(),
                "prompt_type": "reflect",
                "prompt": prompt,
                "agent_status_after": "idle",
            }),
        );

        DecisionExecutionResult::CustomInstruction { instruction: prompt }
    }

    fn execute_confirm_completion(
        slots: &mut [AgentSlot],
        slot_index: usize,
    ) -> DecisionExecutionResult {
        let slot = &mut slots[slot_index];
        // Clear task assignment and transition agent to idle
        // Note: backlog completion should be handled externally via complete_task_with_backlog
        if slot.status().is_blocked() {
            let _ = slot.transition_to(AgentSlotStatus::idle());
        }
        slot.clear_task();

        // Log: Completion confirmed
        logging::debug_event(
            "decision_layer.action_completed",
            "task completion confirmed by decision layer",
            serde_json::json!({
                "work_agent_id": slot.agent_id().as_str(),
                "action_type": "confirm_completion",
                "task_id": slot.assigned_task_id().map(|t| t.as_str()).unwrap_or("none"),
                "agent_status_after": "idle",
            }),
        );

        DecisionExecutionResult::Executed {
            option_id: "confirm_completion".to_string(),
        }
    }

    fn execute_retry(
        slots: &mut [AgentSlot],
        slot_index: usize,
        params_str: String,
    ) -> DecisionExecutionResult {
        let params: serde_json::Value =
            serde_json::from_str(&params_str).unwrap_or(serde_json::json!({}));
        let prompt = params["prompt"]
            .as_str()
            .unwrap_or("Please retry the previous action.")
            .to_string();

        let slot = &mut slots[slot_index];
        // Add retry prompt as a user message
        slot.append_transcript(crate::app::TranscriptEntry::User(prompt.clone()));

        // For Resting state (rate limit recovery), retry keeps us in Resting
        // The agent stays resting until the retry succeeds and "continue" is called
        if matches!(slot.status(), AgentSlotStatus::Resting { .. }) {
            logging::debug_event(
                "decision_layer.work_agent_prompt",
                "retry while resting - rate limit recovery attempted",
                serde_json::json!({
                    "work_agent_id": slot.agent_id().as_str(),
                    "prompt_type": "retry",
                    "agent_status": "resting",
                }),
            );
        } else if slot.status().is_blocked() {
            let _ = slot.transition_to(AgentSlotStatus::idle());
        }

        DecisionExecutionResult::CustomInstruction { instruction: prompt }
    }

    fn execute_continue_all_tasks(
        slots: &mut [AgentSlot],
        slot_index: usize,
        params_str: String,
    ) -> DecisionExecutionResult {
        let params: serde_json::Value =
            serde_json::from_str(&params_str).unwrap_or(serde_json::json!({}));
        let instruction = params["instruction"]
            .as_str()
            .unwrap_or("continue finish all tasks")
            .to_string();

        let slot = &mut slots[slot_index];

        // Guard: if agent is idle with no assigned task, don't start provider
        if slot.status().is_idle() && slot.assigned_task_id().is_none() {
            logging::debug_event(
                "decision_layer.continue_all_tasks_no_op",
                "agent idle with no assigned task, skipping provider start",
                serde_json::json!({
                    "work_agent_id": slot.agent_id().as_str(),
                    "codename": slot.codename().as_str(),
                }),
            );
            return DecisionExecutionResult::AcceptedRecommendation;
        }

        // Add continue instruction as a user message to trigger work
        slot.append_transcript(crate::app::TranscriptEntry::User(instruction.clone()));

        // NOTE: We do NOT transition status here. The state transition
        // will be handled by start_provider_for_agent_with_mode when
        // the provider thread starts.
        //
        // State transition rules:
        // - Blocked → Responding: VALID (handled by start_provider_for_agent_with_mode)
        // - WaitingForInput → Responding: VALID (handled by start_provider_for_agent_with_mode)
        // - Idle → Responding: INVALID
        //
        // For Idle agents, the proper flow is:
        // - Idle → Starting → Responding (handled in ui_state.rs)

        // Log: Continue all tasks action
        logging::debug_event(
            "decision_layer.work_agent_prompt",
            "continue_all_tasks instruction sent to work agent",
            serde_json::json!({
                "work_agent_id": slot.agent_id().as_str(),
                "prompt_type": "continue_all_tasks",
                "instruction": instruction,
                "agent_status": slot.status().label(),
            }),
        );

        DecisionExecutionResult::CustomInstruction { instruction }
    }

    fn execute_stop_if_complete(
        slots: &mut [AgentSlot],
        slot_index: usize,
        params_str: String,
    ) -> DecisionExecutionResult {
        let params: serde_json::Value =
            serde_json::from_str(&params_str).unwrap_or(serde_json::json!({}));
        let reason = params["reason"]
            .as_str()
            .unwrap_or("All tasks complete")
            .to_string();

        let slot = &mut slots[slot_index];
        // Only stop if there are no pending tasks (decision layer's responsibility to check)
        // Transition agent to stopped state
        let _ = slot.transition_to(AgentSlotStatus::stopped(reason.clone()));

        // Log: Stop if complete action
        logging::debug_event(
            "decision_layer.action_completed",
            "stop_if_complete action executed - agent stopped",
            serde_json::json!({
                "work_agent_id": slot.agent_id().as_str(),
                "action_type": "stop_if_complete",
                "reason": reason,
                "agent_status_after": "stopped",
            }),
        );

        DecisionExecutionResult::Executed {
            option_id: "stop_if_complete".to_string(),
        }
    }

    fn execute_prepare_task_start(
        slots: &mut [AgentSlot],
        slot_index: usize,
        worktree_coordinator: &WorktreeCoordinator,
        params_str: String,
    ) -> DecisionExecutionResult {
        let Some(executor) = worktree_coordinator.git_flow_executor() else {
            logging::warn_event(
                "git_flow.executor_missing",
                "git_flow_executor not available",
                serde_json::json!({
                    "work_agent_id": slots[slot_index].agent_id().as_str(),
                }),
            );
            return DecisionExecutionResult::Cancelled;
        };

        // Parse params
        let params: serde_json::Value =
            serde_json::from_str(&params_str).unwrap_or(serde_json::json!({}));
        let task_id = params["task_id"].as_str().unwrap_or("unknown");
        let task_description = params["task_description"]
            .as_str()
            .unwrap_or("")
            .to_string();

        // Get worktree path for this agent
        let slot = &slots[slot_index];
        let worktree_path = slot.cwd().to_path_buf();
        let work_agent_id = slot.agent_id().clone();

        // Execute preparation
        match executor.prepare_for_task(&worktree_path, task_id, &task_description) {
            Ok(result) => {
                // Log success
                logging::debug_event(
                    "git_flow.preparation.completed",
                    "task preparation succeeded",
                    serde_json::json!({
                        "work_agent_id": work_agent_id.as_str(),
                        "task_id": task_id,
                        "branch": result.branch_name,
                        "success": result.success,
                    }),
                );

                // Send preparation context to the agent
                let context_message = result.to_context_message();
                slots[slot_index].append_transcript(crate::app::TranscriptEntry::Status(context_message));

                DecisionExecutionResult::TaskPrepared {
                    branch: result.branch_name,
                    worktree_path: result.worktree_path,
                }
            }
            Err(e) => {
                // Log error
                logging::warn_event(
                    "git_flow.preparation.failed",
                    "task preparation failed",
                    serde_json::json!({
                        "work_agent_id": work_agent_id.as_str(),
                        "task_id": task_id,
                        "error": e.to_string(),
                    }),
                );

                // Send error context to agent
                let error_msg = format!(
                    "Task preparation failed: {}. Please resolve issues manually.",
                    e
                );
                slots[slot_index].append_transcript(crate::app::TranscriptEntry::Status(error_msg));

                DecisionExecutionResult::PreparationFailed {
                    reason: e.to_string(),
                }
            }
        }
    }

    fn execute_unknown(
        work_agent_id: &AgentId,
        action_type: String,
        params_str: String,
    ) -> DecisionExecutionResult {
        // Unknown action type
        logging::warn_event(
            "decision_layer.unknown_action",
            "unknown decision action type - action cancelled",
            serde_json::json!({
                "work_agent_id": work_agent_id.as_str(),
                "action_type": action_type,
                "action_params": params_str,
            }),
        );
        DecisionExecutionResult::Cancelled
    }

    /// Process human response internally (updates slot status)
    ///
    /// This is a helper method used by select_option and skip actions.
    fn process_human_response_internal(
        slots: &mut [AgentSlot],
        human_queue: &mut HumanDecisionQueue,
        response: HumanDecisionResponse,
        work_agent_id: &AgentId,
    ) {
        // Find the agent slot using the work_agent_id (passed from caller)
        let slot = slots.iter_mut().find(|s| s.agent_id() == work_agent_id);
        if let Some(slot) = slot {
            // Apply the selection based on response
            match &response.selection {
                HumanSelection::Selected { option_id: _ } => {
                    // Execute the selected option - transition to idle
                    let _ = slot.transition_to(AgentSlotStatus::idle());
                }
                HumanSelection::Skipped => {
                    // Skip - transition to idle
                    let _ = slot.transition_to(AgentSlotStatus::idle());
                }
                HumanSelection::Cancelled => {
                    // Cancelled - transition to idle
                    let _ = slot.transition_to(AgentSlotStatus::idle());
                }
                HumanSelection::AcceptedRecommendation => {
                    // Accepted - transition to idle
                    let _ = slot.transition_to(AgentSlotStatus::idle());
                }
                HumanSelection::Custom { instruction: _ } => {
                    // Custom instruction - transition to idle
                    let _ = slot.transition_to(AgentSlotStatus::idle());
                }
            }
        }
        // Complete the request (removes from queue and adds to history)
        human_queue.complete(response);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decision_executor_execute_agent_not_found() {
        let mut slots: Vec<AgentSlot> = vec![];
        let mut human_queue = HumanDecisionQueue::default();
        let worktree_coordinator = WorktreeCoordinator::new();
        let work_agent_id = AgentId::new("nonexistent-agent");
        let output = agent_decision::output::DecisionOutput::new(vec![], "test reasoning");

        let result = DecisionExecutor::execute(
            &mut slots,
            &mut human_queue,
            &worktree_coordinator,
            &work_agent_id,
            &output,
        );

        assert!(matches!(result, DecisionExecutionResult::AgentNotFound));
    }

    #[test]
    fn decision_executor_execute_empty_actions_cancelled() {
        use crate::agent_runtime::{AgentCodename, ProviderType};
        use crate::ProviderKind;

        let work_agent_id = AgentId::new("work-agent");
        let slot = AgentSlot::new(
            work_agent_id.clone(),
            AgentCodename::new("TEST"),
            ProviderType::from_provider_kind(ProviderKind::Mock),
        );
        let mut slots = vec![slot];
        let mut human_queue = HumanDecisionQueue::default();
        let worktree_coordinator = WorktreeCoordinator::new();
        let output = agent_decision::output::DecisionOutput::new(vec![], "test reasoning");

        let result = DecisionExecutor::execute(
            &mut slots,
            &mut human_queue,
            &worktree_coordinator,
            &work_agent_id,
            &output,
        );

        // Empty actions result in Cancelled
        assert!(matches!(result, DecisionExecutionResult::Cancelled));
    }

    // ── translate() tests ───────────────────────────────────────

    #[test]
    fn translate_empty_actions() {
        let work_agent_id = AgentId::new("ag-1");
        let output = agent_decision::output::DecisionOutput::new(vec![], "test reasoning");
        let commands = DecisionExecutor::translate(&work_agent_id, &output);
        assert!(commands.is_empty());
    }

    #[test]
    fn translate_custom_instruction() {
        use agent_decision::model::action::CustomInstructionAction;
        use agent_decision::model::action::DecisionAction;

        let work_agent_id = AgentId::new("ag-1");
        let action = CustomInstructionAction::new("do something");
        let output = agent_decision::output::DecisionOutput::new(
            vec![action.clone_boxed()],
            "test reasoning",
        );
        let commands = DecisionExecutor::translate(&work_agent_id, &output);
        assert_eq!(commands.len(), 1);
        assert!(matches!(
            &commands[0],
            DecisionCommand::SendCustomInstruction { prompt, target_agent }
            if prompt == "do something" && target_agent == "ag-1"
        ));
    }

    #[test]
    fn translate_continue() {
        use agent_decision::model::action::ContinueAction;
        use agent_decision::model::action::DecisionAction;

        let work_agent_id = AgentId::new("ag-1");
        let action = ContinueAction::new("keep going");
        let output = agent_decision::output::DecisionOutput::new(
            vec![action.clone_boxed()],
            "test reasoning",
        );
        let commands = DecisionExecutor::translate(&work_agent_id, &output);
        assert_eq!(commands.len(), 1);
        assert!(matches!(&commands[0], DecisionCommand::ApproveAndContinue));
    }

    #[test]
    fn translate_stop_if_complete() {
        use agent_decision::model::action::StopIfCompleteAction;
        use agent_decision::model::action::DecisionAction;

        let work_agent_id = AgentId::new("ag-1");
        let action = StopIfCompleteAction::new("all done");
        let output = agent_decision::output::DecisionOutput::new(
            vec![action.clone_boxed()],
            "test reasoning",
        );
        let commands = DecisionExecutor::translate(&work_agent_id, &output);
        assert_eq!(commands.len(), 1);
        assert!(matches!(
            &commands[0],
            DecisionCommand::StopIfComplete { reason } if reason == "all done"
        ));
    }

    #[test]
    fn translate_reflect() {
        use agent_decision::model::action::ReflectAction;
        use agent_decision::model::action::DecisionAction;

        let work_agent_id = AgentId::new("ag-1");
        let action = ReflectAction::new("verify this");
        let output = agent_decision::output::DecisionOutput::new(
            vec![action.clone_boxed()],
            "test reasoning",
        );
        let commands = DecisionExecutor::translate(&work_agent_id, &output);
        assert_eq!(commands.len(), 1);
        assert!(matches!(
            &commands[0],
            DecisionCommand::Reflect { prompt } if prompt == "verify this"
        ));
    }

    #[test]
    fn translate_multiple_actions() {
        use agent_decision::model::action::{ContinueAction, RetryAction};
        use agent_decision::model::action::DecisionAction;

        let work_agent_id = AgentId::new("ag-1");
        let a1 = ContinueAction::new("go");
        let a2 = RetryAction::new("retry");
        let output = agent_decision::output::DecisionOutput::new(
            vec![a1.clone_boxed(), a2.clone_boxed()],
            "test reasoning",
        );
        let commands = DecisionExecutor::translate(&work_agent_id, &output);
        assert_eq!(commands.len(), 2);
        assert!(matches!(&commands[0], DecisionCommand::ApproveAndContinue));
        assert!(matches!(&commands[1], DecisionCommand::SendCustomInstruction { .. }));
    }
}