//! Decision agent spawning coordinator
//!
//! Provides utilities for spawning decision agents for work agents.
//! This module extracts decision agent creation logic from AgentPool.

use std::path::PathBuf;
use std::sync::Arc;

use crate::agent_runtime::AgentId;
use crate::agent_slot::AgentSlot;
use crate::decision_agent_slot::DecisionAgentSlot;
use crate::decision_mail::DecisionMail;
use crate::llm_caller::ProviderLLMCaller;
use crate::logging;
use crate::pool::WorkerDecisionRouter;
use crate::provider_profile::ProfileId;
use crate::ProviderKind;

/// Spawn decision agent for a work agent
///
/// Creates a decision agent that handles decision requests for the specified work agent.
/// The decision agent uses the same provider as the work agent.
pub fn spawn_decision_agent_for(
    slots: &[AgentSlot],
    decision_coordinator: &mut WorkerDecisionRouter,
    cwd: &PathBuf,
    work_agent_id: &AgentId,
) -> Result<(), String> {
    let slot_index = slots
        .iter()
        .position(|s| s.agent_id() == work_agent_id)
        .ok_or_else(|| format!("Agent {} not found in pool", work_agent_id.as_str()))?;

    let work_slot = &slots[slot_index];
    let provider_kind_opt = work_slot.provider_type().to_provider_kind();

    let provider_kind = provider_kind_opt.ok_or_else(|| {
        format!(
            "Provider type {} doesn't have a ProviderKind mapping",
            work_slot.provider_type().label()
        )
    })?;

    spawn_with_provider(decision_coordinator, cwd, work_agent_id, provider_kind, None)
}

/// Spawn decision agent for a work agent with optional profile
///
/// Creates a decision agent with an optional profile_id for independent
/// decision layer LLM backend configuration.
pub fn spawn_decision_agent_with_profile_for(
    slots: &[AgentSlot],
    decision_coordinator: &mut WorkerDecisionRouter,
    cwd: &PathBuf,
    work_agent_id: &AgentId,
    profile_id: Option<&ProfileId>,
) -> Result<(), String> {
    let slot_index = slots
        .iter()
        .position(|s| s.agent_id() == work_agent_id)
        .ok_or_else(|| format!("Agent {} not found in pool", work_agent_id.as_str()))?;

    let work_slot = &slots[slot_index];
    let provider_kind_opt = work_slot.provider_type().to_provider_kind();

    let provider_kind = provider_kind_opt.ok_or_else(|| {
        format!(
            "Provider type {} doesn't have a ProviderKind mapping",
            work_slot.provider_type().label()
        )
    })?;

    spawn_with_provider(
        decision_coordinator,
        cwd,
        work_agent_id,
        provider_kind,
        profile_id,
    )
}

/// Internal helper to spawn decision agent with provider kind and optional profile
fn spawn_with_provider(
    decision_coordinator: &mut WorkerDecisionRouter,
    cwd: &PathBuf,
    work_agent_id: &AgentId,
    provider_kind: ProviderKind,
    profile_id: Option<&ProfileId>,
) -> Result<(), String> {
    // Create decision mail channel
    let mail = DecisionMail::new();
    let (sender, receiver) = mail.split();

    // Create decision agent slot
    let mut decision_agent = DecisionAgentSlot::new(
        work_agent_id.as_str().to_string(),
        provider_kind,
        receiver,
        cwd.clone(),
        decision_coordinator.components(),
    );

    // Set profile_id if provided
    if let Some(pid) = profile_id {
        decision_agent.set_profile_id(pid.clone());
    }

    // Inject ProviderLLMCaller for real LLM calls
    let llm_caller = Arc::new(ProviderLLMCaller::new(provider_kind, cwd.clone()));
    decision_agent.set_llm_caller(llm_caller);

    // Store the decision agent and mail sender using coordinator
    decision_coordinator.insert_agent(work_agent_id.clone(), decision_agent);
    decision_coordinator.insert_mail_sender(work_agent_id.clone(), sender);

    logging::debug_event(
        "pool.decision_agent.spawn_with_profile",
        "spawned decision agent for work agent with profile",
        serde_json::json!({
            "work_agent_id": work_agent_id.as_str(),
            "provider_kind": provider_kind.label(),
            "profile_id": profile_id,
        }),
    );

    Ok(())
}

/// Stop the decision agent for a work agent
pub fn stop_decision_agent_for(
    decision_coordinator: &mut WorkerDecisionRouter,
    work_agent_id: &AgentId,
) -> Result<(), String> {
    if let Some(mut decision_agent) = decision_coordinator.remove_agent(work_agent_id) {
        decision_agent.stop("work agent stopping");

        logging::debug_event(
            "pool.decision_agent.stop",
            "stopped decision agent for work agent",
            serde_json::json!({
                "work_agent_id": work_agent_id.as_str(),
            }),
        );
    }
    Ok(())
}