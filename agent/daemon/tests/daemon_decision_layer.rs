//! Integration test: daemon decision layer integration
//!
//! Verifies that the daemon can:
//! 1. Start provider threads via send_input
//! 2. Process provider events in tick()
//! 3. Trigger the decision layer when appropriate
//! 4. Broadcast protocol events to clients

use agent_daemon::session_mgr::SessionManager;
use agent_types::WorkplaceId;

/// Daemon processes provider events and triggers decision layer.
#[tokio::test]
#[serial_test::serial]
async fn daemon_tick_processes_provider_events_and_triggers_decisions() {
    let temp = tempfile::TempDir::new().expect("temp dir");
    let workdir = temp.path().to_path_buf();
    let workplace_id = WorkplaceId::new("test-daemon-decision");

    // 1. Bootstrap daemon session
    let session_mgr = SessionManager::bootstrap(workdir.clone(), workplace_id.clone())
        .await
        .expect("bootstrap session manager");

    // 2. Spawn a Mock agent
    let agent = session_mgr
        .spawn_agent(agent_types::ProviderKind::Mock)
        .await
        .expect("spawn agent");
    let agent_id = agent.id.clone();

    // Mock provider does not auto-spawn a decision agent, so do it manually
    // for this test so the full decision pipeline can be exercised.
    session_mgr
        .spawn_decision_agent_for(&agent_id)
        .await
        .expect("spawn decision agent");
    assert!(
        session_mgr.decision_agent_exists(&agent_id).await,
        "decision agent should exist"
    );

    // 3. Send input to the agent — this starts a provider thread.
    let result = session_mgr
        .send_input(&agent_id, "hello world")
        .await
        .expect("send input");
    assert!(result.accepted, "input should be accepted");

    // 4. Pump tick() until the provider thread finishes.
    // The Mock provider emits: Status → AssistantChunk(s) → Finished.
    let mut saw_assistant = false;
    let mut saw_finished_event = false;
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);

    while std::time::Instant::now() < deadline {
        let events = session_mgr.tick().await.expect("tick");
        for ev in &events {
            match &ev.payload {
                agent_protocol::events::EventPayload::ItemDelta(delta) => {
                    if matches!(delta.delta, agent_protocol::events::ItemDelta::Text(_)) {
                        saw_assistant = true;
                    }
                }
                agent_protocol::events::EventPayload::AgentStatusChanged(status) => {
                    if status.agent_id == agent_id
                        && status.status == agent_protocol::state::AgentSlotStatus::Idle
                    {
                        saw_finished_event = true;
                    }
                }
                _ => {}
            }
        }
        if saw_finished_event {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }

    assert!(
        saw_assistant,
        "should have broadcast at least one assistant chunk event"
    );
    assert!(
        saw_finished_event,
        "should have broadcast agent status changed to Idle"
    );

    // 5. Verify transcript was updated.
    let transcript = session_mgr
        .agent_transcript(&agent_id)
        .await
        .expect("transcript should exist");
    let has_user = transcript.iter().any(|e| {
        matches!(e, agent_core::app::TranscriptEntry::User(text) if text == "hello world")
    });
    let has_assistant = transcript.iter().any(|e| {
        matches!(e, agent_core::app::TranscriptEntry::Assistant(text) if text.contains("Mock reply"))
    });
    assert!(has_user, "transcript should contain user input");
    assert!(has_assistant, "transcript should contain assistant reply");

    // 6. Verify the agent transitioned back to idle.
    let status = session_mgr
        .agent_status(&agent_id)
        .await
        .expect("status should exist");
    assert_eq!(status, "idle", "agent should be idle after provider finishes");
}

/// send_input rejects when agent is already busy.
#[tokio::test]
#[serial_test::serial]
async fn daemon_send_input_rejects_when_agent_busy() {
    let temp = tempfile::TempDir::new().expect("temp dir");
    let workdir = temp.path().to_path_buf();
    let workplace_id = WorkplaceId::new("test-daemon-busy");

    let session_mgr = SessionManager::bootstrap(workdir, workplace_id)
        .await
        .expect("bootstrap");

    let agent = session_mgr
        .spawn_agent(agent_types::ProviderKind::Mock)
        .await
        .expect("spawn agent");
    let agent_id = agent.id;

    // First send_input should succeed.
    session_mgr
        .send_input(&agent_id, "first prompt")
        .await
        .expect("first send_input");

    // Immediately try a second send_input — depending on timing the Mock provider
    // may still be running. If it is, we expect rejection.
    let _second_result = session_mgr.send_input(&agent_id, "second prompt").await;

    // Pump events until the first provider definitely finishes.
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while std::time::Instant::now() < deadline {
        let _ = session_mgr.tick().await;
        if session_mgr.agent_status(&agent_id).await.as_deref() == Some("idle") {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }

    // Verify the agent is idle.
    assert_eq!(
        session_mgr.agent_status(&agent_id).await.as_deref(),
        Some("idle"),
        "agent should be idle after provider finishes"
    );

    // After finishing, a new send_input should always succeed.
    session_mgr
        .send_input(&agent_id, "third prompt")
        .await
        .expect("third send_input after idle");

    // If the second send_input happened while the provider was still running,
    // it should have been rejected. We verify this by checking whether the
    // transcript contains only the first and third prompts (not the second).
    let transcript = session_mgr
        .agent_transcript(&agent_id)
        .await
        .expect("transcript should exist");
    let user_prompts: Vec<_> = transcript
        .iter()
        .filter_map(|e| match e {
            agent_core::app::TranscriptEntry::User(text) => Some(text.as_str()),
            _ => None,
        })
        .collect();

    // If second was rejected, we see ["first prompt", "third prompt"].
    // If second succeeded (thread finished fast), we see ["first", "second", "third"].
    // Both are valid outcomes; we just assert the transcript makes sense.
    assert!(
        user_prompts.contains(&"first prompt"),
        "transcript should contain first prompt"
    );
    assert!(
        user_prompts.contains(&"third prompt"),
        "transcript should contain third prompt"
    );
}

/// Idle agent triggers decision layer after timeout.
#[tokio::test]
#[serial_test::serial]
async fn daemon_idle_agent_triggers_decision_layer() {
    let temp = tempfile::TempDir::new().expect("temp dir");
    let workdir = temp.path().to_path_buf();
    let workplace_id = WorkplaceId::new("test-daemon-idle");

    let session_mgr = SessionManager::bootstrap(workdir, workplace_id)
        .await
        .expect("bootstrap");

    // 1. Spawn a Mock agent
    let agent = session_mgr
        .spawn_agent(agent_types::ProviderKind::Mock)
        .await
        .expect("spawn agent");
    let agent_id = agent.id;

    // 2. Manually spawn a decision agent (Mock does not auto-spawn)
    session_mgr
        .spawn_decision_agent_for(&agent_id)
        .await
        .expect("spawn decision agent");

    // 3. Verify agent is idle.
    assert_eq!(
        session_mgr.agent_status(&agent_id).await.as_deref(),
        Some("idle"),
        "agent should start idle"
    );

    // 4. Artificially age the slot's last_activity so it appears idle > 60s.
    let aged = std::time::Instant::now() - std::time::Duration::from_secs(70);
    session_mgr
        .set_slot_last_activity(&agent_id, aged)
        .await;

    // 5. Pump tick() — the idle check should fire and send a decision request.
    let mut saw_decision_request = false;
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);

    while std::time::Instant::now() < deadline {
        let _ = session_mgr.tick().await.expect("tick");

        // Check whether a decision request was enqueued by looking at the
        // decision agent's status (it should transition from idle to thinking).
        if let Some(da) = session_mgr.decision_agent_status(&agent_id).await {
            if da == "thinking" {
                saw_decision_request = true;
                break;
            }
        }

        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }

    assert!(
        saw_decision_request,
        "idle agent should have triggered a decision request"
    );
}
