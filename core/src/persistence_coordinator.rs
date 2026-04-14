//! Agent Persistence Coordinator
//!
//! Coordinates persistence operations across multiple agents.
//! Provides batch persistence and periodic flush capabilities.

use std::collections::VecDeque;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use anyhow::Result;

use crate::agent_memory::AgentMemory;
use crate::agent_messages::AgentMessages;
use crate::agent_runtime::AgentId;
use crate::agent_runtime::AgentMeta;
use crate::agent_state::AgentState;
use crate::agent_transcript::AgentTranscript;
use crate::workplace_store::WorkplaceStore;

/// Persistence operation types
#[derive(Debug, Clone)]
pub enum PersistenceOp {
    /// Save agent metadata
    SaveMeta {
        agent_id: AgentId,
        meta: AgentMeta,
    },
    /// Save agent transcript
    SaveTranscript {
        agent_id: AgentId,
        transcript: AgentTranscript,
    },
    /// Save agent state
    SaveState {
        agent_id: AgentId,
        state: AgentState,
    },
    /// Save agent messages
    SaveMessages {
        agent_id: AgentId,
        messages: AgentMessages,
    },
    /// Save agent memory
    SaveMemory {
        agent_id: AgentId,
        memory: AgentMemory,
    },
}

/// Coordinator for managing persistence operations across agents
///
/// Queues persistence operations and flushes them periodically.
/// Ensures concurrent persistence doesn't corrupt state.
#[derive(Debug)]
pub struct AgentPersistenceCoordinator {
    /// Workplace store reference
    workplace: WorkplaceStore,
    /// Pending persistence operations queue
    pending_ops: VecDeque<PersistenceOp>,
    /// Last flush timestamp
    last_flush: Instant,
    /// Minimum interval between flushes
    flush_interval: Duration,
    /// Number of operations processed
    ops_processed: usize,
}

impl AgentPersistenceCoordinator {
    /// Create a new persistence coordinator
    pub fn new(workplace: WorkplaceStore, flush_interval: Duration) -> Self {
        Self {
            workplace,
            pending_ops: VecDeque::new(),
            last_flush: Instant::now(),
            flush_interval,
            ops_processed: 0,
        }
    }

    /// Create with default flush interval (5 seconds)
    pub fn with_default_interval(workplace: WorkplaceStore) -> Self {
        Self::new(workplace, Duration::from_secs(5))
    }

    /// Queue a persistence operation
    pub fn queue(&mut self, op: PersistenceOp) {
        self.pending_ops.push_back(op);
    }

    /// Queue multiple operations
    pub fn queue_batch(&mut self, ops: Vec<PersistenceOp>) {
        for op in ops {
            self.pending_ops.push_back(op);
        }
    }

    /// Check if flush is needed based on interval
    pub fn needs_flush(&self) -> bool {
        Instant::now().saturating_duration_since(self.last_flush) >= self.flush_interval
    }

    /// Check if there are pending operations
    pub fn has_pending_ops(&self) -> bool {
        !self.pending_ops.is_empty()
    }

    /// Get number of pending operations
    pub fn pending_count(&self) -> usize {
        self.pending_ops.len()
    }

    /// Flush all pending operations
    ///
    /// Returns the paths of saved files.
    pub fn flush(&mut self) -> Result<Vec<PathBuf>> {
        let mut saved_paths = Vec::new();

        while let Some(op) = self.pending_ops.pop_front() {
            let path = self.execute_op(&op)?;
            saved_paths.push(path);
            self.ops_processed += 1;
        }

        self.last_flush = Instant::now();
        Ok(saved_paths)
    }

    /// Force immediate save for a specific agent
    ///
    /// Flushes all pending operations for the given agent.
    pub fn force_save(&mut self, agent_id: &AgentId) -> Result<Vec<PathBuf>> {
        let mut saved_paths = Vec::new();

        // Drain all ops and partition into matching vs non-matching
        let all_ops: Vec<PersistenceOp> = self.pending_ops.drain(..).collect();
        let (matching, non_matching): (Vec<_>, Vec<_>) = all_ops.into_iter().partition(|op| {
            match op {
                PersistenceOp::SaveMeta { agent_id: id, .. } => id == agent_id,
                PersistenceOp::SaveTranscript { agent_id: id, .. } => id == agent_id,
                PersistenceOp::SaveState { agent_id: id, .. } => id == agent_id,
                PersistenceOp::SaveMessages { agent_id: id, .. } => id == agent_id,
                PersistenceOp::SaveMemory { agent_id: id, .. } => id == agent_id,
            }
        });

        // Re-queue non-matching ops
        for op in non_matching {
            self.pending_ops.push_back(op);
        }

        // Execute matching ops
        for op in matching {
            let path = self.execute_op(&op)?;
            saved_paths.push(path);
            self.ops_processed += 1;
        }

        Ok(saved_paths)
    }

    /// Execute a single persistence operation
    fn execute_op(&self, op: &PersistenceOp) -> Result<PathBuf> {
        match op {
            PersistenceOp::SaveMeta { agent_id, meta } => {
                self.save_meta(agent_id, meta)
            }
            PersistenceOp::SaveTranscript { agent_id, transcript } => {
                self.save_transcript(agent_id, transcript)
            }
            PersistenceOp::SaveState { agent_id, state } => {
                self.save_state(agent_id, state)
            }
            PersistenceOp::SaveMessages { agent_id, messages } => {
                self.save_messages(agent_id, messages)
            }
            PersistenceOp::SaveMemory { agent_id, memory } => {
                self.save_memory(agent_id, memory)
            }
        }
    }

    /// Get agent directory path
    fn agent_dir(&self, agent_id: &AgentId) -> PathBuf {
        self.workplace.agents_dir().join(agent_id.as_str())
    }

    /// Save agent metadata
    fn save_meta(&self, agent_id: &AgentId, meta: &AgentMeta) -> Result<PathBuf> {
        let agent_dir = self.agent_dir(agent_id);
        std::fs::create_dir_all(&agent_dir)?;
        let path = agent_dir.join("meta.json");
        let payload = serde_json::to_string_pretty(meta)?;
        std::fs::write(&path, payload)?;
        Ok(path)
    }

    /// Save agent transcript
    fn save_transcript(&self, agent_id: &AgentId, transcript: &AgentTranscript) -> Result<PathBuf> {
        let agent_dir = self.agent_dir(agent_id);
        std::fs::create_dir_all(&agent_dir)?;
        let path = agent_dir.join("transcript.json");
        let payload = serde_json::to_string_pretty(transcript)?;
        std::fs::write(&path, payload)?;
        Ok(path)
    }

    /// Save agent state
    fn save_state(&self, agent_id: &AgentId, state: &AgentState) -> Result<PathBuf> {
        let agent_dir = self.agent_dir(agent_id);
        std::fs::create_dir_all(&agent_dir)?;
        let path = agent_dir.join("state.json");
        let payload = serde_json::to_string_pretty(state)?;
        std::fs::write(&path, payload)?;
        Ok(path)
    }

    /// Save agent messages
    fn save_messages(&self, agent_id: &AgentId, messages: &AgentMessages) -> Result<PathBuf> {
        let agent_dir = self.agent_dir(agent_id);
        std::fs::create_dir_all(&agent_dir)?;
        let path = agent_dir.join("messages.json");
        let payload = serde_json::to_string_pretty(messages)?;
        std::fs::write(&path, payload)?;
        Ok(path)
    }

    /// Save agent memory
    fn save_memory(&self, agent_id: &AgentId, memory: &AgentMemory) -> Result<PathBuf> {
        let agent_dir = self.agent_dir(agent_id);
        std::fs::create_dir_all(&agent_dir)?;
        let path = agent_dir.join("memory.json");
        let payload = serde_json::to_string_pretty(memory)?;
        std::fs::write(&path, payload)?;
        Ok(path)
    }

    /// Get number of operations processed
    pub fn ops_processed(&self) -> usize {
        self.ops_processed
    }

    /// Reset statistics
    pub fn reset_stats(&mut self) {
        self.ops_processed = 0;
    }

    /// Get workplace reference
    pub fn workplace(&self) -> &WorkplaceStore {
        &self.workplace
    }
}

#[cfg(test)]
mod tests {
    use super::AgentPersistenceCoordinator;
    use super::PersistenceOp;
    use crate::agent_runtime::AgentCodename;
    use crate::agent_runtime::AgentId;
    use crate::agent_runtime::AgentMeta;
    use crate::agent_runtime::AgentStatus;
    use crate::agent_runtime::ProviderType;
    use crate::agent_runtime::WorkplaceId;
    use crate::agent_state::AgentState;
    use crate::app::LoopPhase;
    use crate::workplace_store::WorkplaceStore;
    use std::time::Duration;
    use tempfile::TempDir;

    fn make_test_meta(agent_id: &AgentId) -> AgentMeta {
        AgentMeta {
            agent_id: agent_id.clone(),
            codename: AgentCodename::new("alpha"),
            workplace_id: WorkplaceId::new("workplace-001"),
            provider_type: ProviderType::Mock,
            provider_session_id: None,
            created_at: "2025-01-01T00:00:00Z".to_string(),
            updated_at: "2025-01-01T00:00:00Z".to_string(),
            status: AgentStatus::Idle,
        }
    }

    fn make_test_state() -> AgentState {
        AgentState {
            cwd: "/tmp".to_string(),
            draft_input: "".to_string(),
            enabled_skill_names: vec![],
            active_task_id: None,
            active_task_had_error: false,
            continuation_attempts: 0,
            loop_phase: LoopPhase::Idle,
            loop_run_active: false,
            remaining_loop_iterations: 0,
        }
    }

    fn make_coordinator() -> (AgentPersistenceCoordinator, TempDir) {
        let temp = TempDir::new().expect("tempdir");
        let workplace = WorkplaceStore::for_cwd(temp.path()).expect("workplace");
        workplace.ensure().expect("ensure");
        let coord = AgentPersistenceCoordinator::with_default_interval(workplace);
        (coord, temp)
    }

    #[test]
    fn new_coordinator_has_no_pending_ops() {
        let (coord, _temp) = make_coordinator();
        assert!(!coord.has_pending_ops());
        assert_eq!(coord.pending_count(), 0);
    }

    #[test]
    fn queue_adds_operation() {
        let (mut coord, _temp) = make_coordinator();
        let agent_id = AgentId::new("agent_001");
        let meta = make_test_meta(&agent_id);

        coord.queue(PersistenceOp::SaveMeta { agent_id, meta });

        assert!(coord.has_pending_ops());
        assert_eq!(coord.pending_count(), 1);
    }

    #[test]
    fn queue_batch_adds_multiple_operations() {
        let (mut coord, _temp) = make_coordinator();
        let agent_id = AgentId::new("agent_001");

        coord.queue_batch(vec![
            PersistenceOp::SaveMeta { agent_id: agent_id.clone(), meta: make_test_meta(&agent_id) },
            PersistenceOp::SaveState { agent_id: agent_id.clone(), state: make_test_state() },
        ]);

        assert_eq!(coord.pending_count(), 2);
    }

    #[test]
    fn flush_executes_all_operations() {
        let (mut coord, _temp) = make_coordinator();
        let agent_id = AgentId::new("agent_001");

        coord.queue(PersistenceOp::SaveMeta {
            agent_id: agent_id.clone(),
            meta: make_test_meta(&agent_id),
        });

        let paths = coord.flush().expect("flush");
        assert_eq!(paths.len(), 1);
        assert!(!coord.has_pending_ops());
        assert_eq!(coord.ops_processed(), 1);
    }

    #[test]
    fn needs_flush_after_interval() {
        let (mut coord, _temp) = make_coordinator();
        coord.flush_interval = Duration::from_millis(10);

        // Initially doesn't need flush
        assert!(!coord.needs_flush());

        // After interval, needs flush
        std::thread::sleep(Duration::from_millis(15));
        assert!(coord.needs_flush());
    }

    #[test]
    fn flush_creates_agent_directory() {
        let (mut coord, _temp) = make_coordinator();
        let agent_id = AgentId::new("agent_001");

        // Get workplace path before mutable operations
        let workplace_path = coord.workplace().path().to_path_buf();

        coord.queue(PersistenceOp::SaveMeta {
            agent_id: agent_id.clone(),
            meta: make_test_meta(&agent_id),
        });

        coord.flush().expect("flush");

        let agent_dir = workplace_path.join("agents").join("agent_001");
        assert!(agent_dir.exists());
        assert!(agent_dir.join("meta.json").exists());
    }

    #[test]
    fn force_save_only_processes_matching_agent() {
        let (mut coord, _temp) = make_coordinator();
        let agent1 = AgentId::new("agent_001");
        let agent2 = AgentId::new("agent_002");

        coord.queue(PersistenceOp::SaveMeta {
            agent_id: agent1.clone(),
            meta: make_test_meta(&agent1),
        });
        coord.queue(PersistenceOp::SaveMeta {
            agent_id: agent2.clone(),
            meta: make_test_meta(&agent2),
        });

        let paths = coord.force_save(&agent1).expect("force_save");
        assert_eq!(paths.len(), 1);
        assert_eq!(coord.pending_count(), 1); // agent2 op still pending
    }
}