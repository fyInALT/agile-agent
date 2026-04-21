//! Integration test: daemon agent lifecycle with snapshot persistence
//!
//! Scenario:
//! 1. Bootstrap SessionManager in a temp directory
//! 2. Spawn two agents (A and B)
//! 3. Simulate agent A completing work (inject transcript, create file)
//! 4. Stop agent A
//! 5. Snapshot should only contain agent B
//! 6. Write snapshot to disk, read it back, verify consistency

use agent_daemon::session_mgr::SessionManager;
use agent_types::WorkplaceId;

/// Full lifecycle: spawn, work, stop, snapshot, restore.
#[tokio::test]
#[serial_test::serial]
async fn daemon_agent_lifecycle_and_snapshot_roundtrip() {
    let temp = tempfile::TempDir::new().expect("temp dir");
    let workdir = temp.path().to_path_buf();
    let workplace_id = WorkplaceId::new("test-daemon-lifecycle");

    // 1. Bootstrap daemon session
    let session_mgr = SessionManager::bootstrap(workdir.clone(), workplace_id.clone())
        .await
        .expect("bootstrap session manager");

    // 2. Spawn agent A and agent B
    let agent_a = session_mgr
        .spawn_agent(agent_types::ProviderKind::Mock)
        .await
        .expect("spawn agent A");
    let agent_b = session_mgr
        .spawn_agent(agent_types::ProviderKind::Mock)
        .await
        .expect("spawn agent B");

    let agent_a_id = agent_a.id.clone();
    let agent_b_id = agent_b.id.clone();

    assert_eq!(session_mgr.agent_count().await, 2);
    assert!(session_mgr.agent_exists(&agent_a_id).await);
    assert!(session_mgr.agent_exists(&agent_b_id).await);

    // 3. Simulate agent A completing work:
    //    - Inject a transcript entry showing the assistant wrote Go hello world
    //    - Create the actual Go file on disk
    session_mgr
        .inject_transcript_entry(
            &agent_a_id,
            agent_core::app::TranscriptEntry::Assistant(
                "I've written a Go hello world program.".to_string(),
            ),
        )
        .await
        .expect("inject transcript");

    let go_file = workdir.join("hello.go");
    std::fs::write(
        &go_file,
        r#"package main

import "fmt"

func main() {
    fmt.Println("Hello, World!")
}
"#,
    )
    .expect("write go file");
    assert!(go_file.exists(), "Go file should exist on disk");

    // 4. Stop agent A
    session_mgr
        .stop_agent(&agent_a_id)
        .await
        .expect("stop agent A");

    // Verify agent A is stopped, agent B is still present
    let active_agents = session_mgr.list_agents(false).await;
    let active_ids: Vec<_> = active_agents.iter().map(|a| a.id.clone()).collect();
    assert!(
        !active_ids.contains(&agent_a_id),
        "stopped agent A should no longer appear in active list"
    );
    assert!(active_ids.contains(&agent_b_id));
    assert_eq!(active_ids.len(), 1);

    // Verify the Go file is still on disk
    assert!(go_file.exists(), "Go file should persist after agent A is stopped");

    // 5. Generate snapshot — contains both agents, but agent A is stopped
    let snapshot = session_mgr.snapshot().await.expect("generate snapshot");
    let agent_a_in_snapshot = snapshot.agents.iter().find(|a| a.id == agent_a_id);
    let agent_b_in_snapshot = snapshot.agents.iter().find(|a| a.id == agent_b_id);
    assert!(agent_a_in_snapshot.is_some(), "snapshot should contain agent A");
    assert!(
        agent_b_in_snapshot.is_some(),
        "snapshot should contain agent B"
    );
    assert_eq!(snapshot.agents.len(), 2);
    // Agent A status in snapshot should be stopped
    assert_eq!(
        agent_a_in_snapshot.unwrap().status,
        agent_protocol::state::AgentSlotStatus::Stopped
    );

    // 6. Write snapshot to disk
    let snapshot_path = temp.path().join("test_snapshot.json");
    session_mgr
        .write_snapshot(&snapshot_path)
        .await
        .expect("write snapshot");
    assert!(snapshot_path.exists(), "snapshot file should be written");

    // 7. Read snapshot back
    let restored_file = SessionManager::read_snapshot(&snapshot_path)
        .await
        .expect("read snapshot");

    // Verify checksum and round-trip consistency
    assert!(restored_file.checksum.is_some(), "snapshot should have checksum");
    assert_eq!(restored_file.state.agents.len(), 2, "restored snapshot should contain both agents");
    let restored_a = restored_file.state.agents.iter().find(|a| a.id == agent_a_id);
    let restored_b = restored_file.state.agents.iter().find(|a| a.id == agent_b_id);
    assert!(restored_a.is_some());
    assert!(restored_b.is_some());
    assert_eq!(restored_a.unwrap().status, agent_protocol::state::AgentSlotStatus::Stopped);

    // 8. Verify Go file content
    let content = std::fs::read_to_string(&go_file).expect("read go file");
    assert!(content.contains("Hello, World!"));
}

/// Snapshot gracefully handles empty state.
#[tokio::test]
#[serial_test::serial]
async fn snapshot_empty_state_roundtrip() {
    let temp = tempfile::TempDir::new().expect("temp dir");
    let workdir = temp.path().to_path_buf();
    let workplace_id = WorkplaceId::new("test-empty");

    let session_mgr = SessionManager::bootstrap(workdir, workplace_id)
        .await
        .expect("bootstrap");

    let snapshot = session_mgr.snapshot().await.expect("snapshot");
    assert!(snapshot.agents.is_empty());

    let path = temp.path().join("empty_snapshot.json");
    session_mgr.write_snapshot(&path).await.expect("write");

    let file = SessionManager::read_snapshot(&path).await.expect("read");
    assert!(file.state.agents.is_empty());
}

/// Full shutdown and resume roundtrip using core-format shutdown snapshot.
///
/// Scenario:
/// 1. Bootstrap SessionManager
/// 2. Spawn agent A and agent B
/// 3. Agent A writes a Go hello world file
/// 4. Stop agent A
/// 5. Save shutdown snapshot
/// 6. Drop SessionManager (simulate shutdown)
/// 7. Restore from shutdown snapshot
/// 8. Assert only agent B remains active
/// 9. Assert the Go file still exists
#[tokio::test]
#[serial_test::serial]
async fn daemon_shutdown_and_resume_roundtrip() {
    let temp = tempfile::TempDir::new().expect("temp dir");
    let workdir = temp.path().to_path_buf();
    let workplace_id = WorkplaceId::new("test-daemon-resume");

    // 1. Bootstrap daemon session
    let session_mgr = SessionManager::bootstrap(workdir.clone(), workplace_id.clone())
        .await
        .expect("bootstrap session manager");

    // 2. Spawn agent A and agent B
    let agent_a = session_mgr
        .spawn_agent(agent_types::ProviderKind::Mock)
        .await
        .expect("spawn agent A");
    let agent_b = session_mgr
        .spawn_agent(agent_types::ProviderKind::Mock)
        .await
        .expect("spawn agent B");

    let agent_a_id = agent_a.id.clone();
    let agent_b_id = agent_b.id.clone();

    assert_eq!(session_mgr.agent_count().await, 2);

    // 3. Simulate agent A completing work
    session_mgr
        .inject_transcript_entry(
            &agent_a_id,
            agent_core::app::TranscriptEntry::Assistant(
                "I've written a Go hello world program.".to_string(),
            ),
        )
        .await
        .expect("inject transcript");

    let go_file = workdir.join("hello.go");
    std::fs::write(
        &go_file,
        r#"package main

import "fmt"

func main() {
    fmt.Println("Hello, World!")
}
"#,
    )
    .expect("write go file");
    assert!(go_file.exists(), "Go file should exist on disk");

    // 4. Stop agent A
    session_mgr
        .stop_agent(&agent_a_id)
        .await
        .expect("stop agent A");

    // Verify agent A is stopped, agent B is still active
    let active_before = session_mgr.list_agents(false).await;
    assert_eq!(active_before.len(), 1, "only agent B should be active before shutdown");
    assert_eq!(active_before[0].id, agent_b_id);

    // 5. Save shutdown snapshot in core format
    let snapshot_path = session_mgr
        .save_shutdown_snapshot(agent_core::shutdown_snapshot::ShutdownReason::UserQuit)
        .await
        .expect("save shutdown snapshot");
    assert!(
        snapshot_path.exists(),
        "shutdown snapshot should be written to disk"
    );

    // Verify snapshot format version and transcript content before shutdown
    let snapshot_json = std::fs::read_to_string(&snapshot_path).expect("read snapshot");
    let snapshot: agent_core::shutdown_snapshot::ShutdownSnapshot =
        serde_json::from_str(&snapshot_json).expect("parse snapshot");
    assert_eq!(snapshot.format_version, 1, "snapshot should be version 1");
    let agent_a_snapshot = snapshot
        .agents
        .iter()
        .find(|a| a.meta.agent_id.as_str() == agent_a_id)
        .expect("agent A in snapshot");
    assert_eq!(
        agent_a_snapshot.transcript.len(),
        1,
        "agent A's transcript should be preserved in snapshot"
    );
    assert_eq!(
        agent_a_snapshot.role,
        agent_types::AgentRole::Developer,
        "agent A's role should be Developer in snapshot"
    );

    // 6. Drop SessionManager to simulate shutdown
    drop(session_mgr);

    // 7. Restore from shutdown snapshot
    let restored_mgr = SessionManager::restore_from_shutdown_snapshot(
        workdir.clone(),
        workplace_id.clone(),
        agent_types::ProviderKind::Mock,
        4,
    )
    .await
    .expect("restore from shutdown snapshot");

    // 8. Assert only agent B remains active AND IDs are preserved
    assert_eq!(
        restored_mgr.agent_count().await,
        2,
        "both agents should exist in pool after resume"
    );

    let active_after = restored_mgr.list_agents(false).await;
    assert_eq!(
        active_after.len(),
        1,
        "only one agent should remain active after resume"
    );
    assert_eq!(
        active_after[0].id, agent_b_id,
        "agent B's original ID should be preserved after resume"
    );
    assert_ne!(
        active_after[0].status,
        agent_protocol::state::AgentSlotStatus::Stopped,
        "remaining agent should not be stopped"
    );

    // Verify stopped agent A is also present with its original ID
    let all_after = restored_mgr.list_agents(true).await;
    assert_eq!(
        all_after.len(),
        2,
        "both agents should exist in pool (one active, one stopped)"
    );
    let stopped = all_after
        .iter()
        .find(|a| a.status == agent_protocol::state::AgentSlotStatus::Stopped);
    assert!(
        stopped.is_some(),
        "stopped agent should be present in full list"
    );
    assert_eq!(
        stopped.unwrap().id,
        agent_a_id,
        "agent A's original ID should be preserved after resume"
    );

    // Verify transcript was restored for agent A
    let restored_transcript = restored_mgr
        .agent_transcript(&agent_a_id)
        .await
        .expect("agent A transcript should exist after restore");
    assert_eq!(
        restored_transcript.len(),
        1,
        "agent A's transcript should survive restore"
    );
    assert_eq!(
        restored_transcript[0],
        agent_core::app::TranscriptEntry::Assistant(
            "I've written a Go hello world program.".to_string(),
        ),
        "agent A's transcript content should match"
    );

    // Verify role was restored for agent B
    assert_eq!(
        active_after[0].role, "Developer",
        "agent B's role should be preserved as Developer"
    );

    // Verify shutdown snapshot was consumed (cleared) by RuntimeSession::bootstrap
    assert!(
        !snapshot_path.exists(),
        "shutdown snapshot should be cleared after successful restore"
    );

    // 9. Verify Go file still exists and content is intact
    assert!(
        go_file.exists(),
        "Go file should persist after shutdown and resume"
    );
    let content = std::fs::read_to_string(&go_file).expect("read go file");
    assert!(content.contains("Hello, World!"));
}

/// Restore without an existing snapshot falls back to clean bootstrap.
#[tokio::test]
#[serial_test::serial]
async fn restore_without_snapshot_falls_back_to_bootstrap() {
    let temp = tempfile::TempDir::new().expect("temp dir");
    let workdir = temp.path().to_path_buf();
    let workplace_id = WorkplaceId::new("test-daemon-no-snapshot");

    // Ensure no shutdown snapshot exists
    let workplace = agent_core::workplace_store::WorkplaceStore::for_cwd(&workdir)
        .expect("resolve workplace");
    let _ = workplace.clear_shutdown_snapshot();

    // Restore should bootstrap a fresh session
    let restored = SessionManager::restore_from_shutdown_snapshot(
        workdir.clone(),
        workplace_id.clone(),
        agent_types::ProviderKind::Mock,
        4,
    )
    .await
    .expect("restore should fall back to bootstrap");

    assert_eq!(
        restored.agent_count().await,
        0,
        "fresh bootstrap should have no agents"
    );
    assert!(
        restored.list_agents(true).await.is_empty(),
        "fresh bootstrap should have empty agent list"
    );
}
