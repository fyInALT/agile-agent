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
