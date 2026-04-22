#![allow(deprecated)]

//! Integration test: daemon decision action → provider thread closure
//!
//! Verifies that when a decision agent returns a `custom_instruction` action,
//! the daemon's `tick()` automatically starts a provider thread for the agent.

use agent_daemon::session_mgr::SessionManager;
use agent_types::WorkplaceId;

/// Decision action `continue_all_tasks` with assigned task triggers provider thread.
#[tokio::test]
#[serial_test::serial]
async fn daemon_custom_instruction_starts_provider_thread() {
    let temp = tempfile::TempDir::new().expect("temp dir");
    let workdir = temp.path().to_path_buf();
    let workplace_id = WorkplaceId::new("test-daemon-custom-instruction");

    let session_mgr = SessionManager::bootstrap(workdir.clone(), workplace_id.clone())
        .await
        .expect("bootstrap");

    // 1. Spawn a Mock agent and its decision agent.
    let agent = session_mgr
        .spawn_agent(agent_types::ProviderKind::Mock)
        .await
        .expect("spawn agent");
    let agent_id = agent.id.clone();

    session_mgr
        .spawn_decision_agent_for(&agent_id)
        .await
        .expect("spawn decision agent");

    // 2. Assign a task so `continue_all_tasks` returns CustomInstruction.
    session_mgr
        .assign_task(&agent_id, "task-001")
        .await
        .expect("assign task");

    // 3. Send a decision request for `agent_idle` situation.
    let situation = agent_decision::builtin_situations::AgentIdleSituation::default();
    let context = agent_decision::context::DecisionContext::new(
        Box::new(situation),
        &agent_id,
    );
    session_mgr
        .send_decision_request(
            &agent_id,
            agent_decision::types::SituationType::new("agent_idle"),
            context,
        )
        .await
        .expect("send decision request");

    // 4. Pump tick() until the decision response is processed and a provider
    //    thread is started (or we time out).
    let mut provider_started = false;
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);

    while std::time::Instant::now() < deadline {
        let _events = session_mgr.tick().await.expect("tick");

        let status = session_mgr.agent_status(&agent_id).await;
        let transcript = session_mgr
            .agent_transcript(&agent_id)
            .await
            .unwrap_or_default();

        // The decision action `continue_all_tasks` appends a User transcript
        // entry with the instruction, then `tick()` starts a provider thread.
        // If the provider thread started, we should see the agent status change
        // from idle to starting/responding.
        if status.as_deref() == Some("responding")
            || status.as_deref() == Some("starting")
        {
            provider_started = true;
            break;
        }

        // Also check if transcript contains the continue instruction.
        let has_instruction = transcript.iter().any(|e| {
            matches!(
                e,
                agent_core::app::TranscriptEntry::User(text)
                    if text.contains("continue finish all tasks")
            )
        });

        // If we see the instruction but status is still idle, the provider
        // start may have failed (e.g. agent was not in valid state). We break
        // early to avoid waiting forever.
        if has_instruction && status.as_deref() == Some("idle") {
            break;
        }

        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }

    assert!(
        provider_started,
        "decision action should have started a provider thread"
    );

    // 5. Let the provider thread finish.
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while std::time::Instant::now() < deadline {
        let _events = session_mgr.tick().await.expect("tick");
        if session_mgr.agent_status(&agent_id).await.as_deref() == Some("idle") {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }

    let transcript = session_mgr
        .agent_transcript(&agent_id)
        .await
        .expect("transcript should exist");

    // Should have the continue instruction + mock assistant reply.
    let has_instruction = transcript.iter().any(|e| {
        matches!(
            e,
            agent_core::app::TranscriptEntry::User(text)
                if text.contains("continue finish all tasks")
        )
    });
    let has_reply = transcript.iter().any(|e| {
        matches!(
            e,
            agent_core::app::TranscriptEntry::Assistant(text)
                if text.contains("Mock reply")
        )
    });

    assert!(has_instruction, "transcript should contain continue instruction from decision action");
    assert!(has_reply, "transcript should contain assistant reply from provider thread");
}
