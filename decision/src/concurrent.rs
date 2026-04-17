//! Concurrent processing support for multi-agent scenarios
//!
//! Sprint 8.6: Provides session pooling, rate limiting, and human decision
//! arbitration for concurrent multi-agent decision handling.

use crate::blocking::{HumanDecisionQueue, HumanDecisionRequest, HumanDecisionResponse};
use crate::engine::SessionHandle;
use crate::lifecycle::AgentId;
use crate::provider_kind::ProviderKind;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// Session pool configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionPoolConfig {
    /// Max sessions per provider (default: 3)
    pub max_per_provider: usize,

    /// Session idle timeout in milliseconds (default: 30 minutes)
    pub idle_timeout_ms: u64,

    /// Enable session reuse
    pub reuse_enabled: bool,
}

impl Default for SessionPoolConfig {
    fn default() -> Self {
        Self {
            max_per_provider: 3,
            idle_timeout_ms: 1800000, // 30 minutes
            reuse_enabled: true,
        }
    }
}

/// Session pool statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionPoolStats {
    /// Total active sessions
    pub total_active: usize,

    /// Available sessions by provider
    pub by_provider: HashMap<ProviderKind, usize>,
}

impl SessionPoolStats {
    pub fn new() -> Self {
        Self {
            total_active: 0,
            by_provider: HashMap::new(),
        }
    }
}

impl Default for SessionPoolStats {
    fn default() -> Self {
        Self::new()
    }
}

/// Pooled session entry
#[derive(Debug, Clone)]
pub struct PooledSession {
    /// Session handle
    pub handle: SessionHandle,

    /// Provider kind
    pub provider: ProviderKind,

    /// Agent ID using this session
    pub assigned_to: Option<AgentId>,

    /// Last activity time
    pub last_activity: Instant,

    /// Creation time
    pub created_at: Instant,
}

impl PooledSession {
    pub fn new(handle: SessionHandle, provider: ProviderKind) -> Self {
        Self {
            handle,
            provider,
            assigned_to: None,
            last_activity: Instant::now(),
            created_at: Instant::now(),
        }
    }

    pub fn assign(&mut self, agent_id: AgentId) {
        self.assigned_to = Some(agent_id);
        self.last_activity = Instant::now();
    }

    pub fn release(&mut self) {
        self.assigned_to = None;
        self.last_activity = Instant::now();
    }

    pub fn is_expired(&self, timeout_ms: u64) -> bool {
        self.last_activity.elapsed() > Duration::from_millis(timeout_ms)
    }

    pub fn is_assigned(&self) -> bool {
        self.assigned_to.is_some()
    }
}

/// Decision session pool - reuse sessions across agents
#[derive(Debug)]
pub struct DecisionSessionPool {
    /// Pool configuration
    config: SessionPoolConfig,

    /// Available sessions per provider
    available: HashMap<ProviderKind, VecDeque<PooledSession>>,

    /// Active sessions (agent_id -> session)
    active: HashMap<AgentId, PooledSession>,
}

impl DecisionSessionPool {
    pub fn new(config: SessionPoolConfig) -> Self {
        Self {
            config,
            available: HashMap::new(),
            active: HashMap::new(),
        }
    }

    /// Check if agent already has session
    pub fn has_session(&self, agent_id: &AgentId) -> bool {
        self.active.contains_key(agent_id)
    }

    /// Get session for agent if exists
    pub fn get_session(&self, agent_id: &AgentId) -> Option<&PooledSession> {
        self.active.get(agent_id)
    }

    /// Acquire session for agent (simulated - returns mock)
    pub fn acquire(
        &mut self,
        provider: ProviderKind,
        agent_id: AgentId,
    ) -> crate::error::Result<PooledSession> {
        // Check if agent already has session
        if let Some(session) = self.active.get(&agent_id) {
            return Ok(session.clone());
        }

        // Check available pool
        if self.config.reuse_enabled {
            if let Some(pool) = self.available.get_mut(&provider) {
                // Remove expired sessions
                pool.retain(|s| !s.is_expired(self.config.idle_timeout_ms));

                if let Some(session) = pool.pop_front() {
                    let mut session = session;
                    session.assign(agent_id.clone());
                    self.active.insert(agent_id.clone(), session.clone());
                    return Ok(session);
                }
            }
        }

        // Check active count for provider
        let active_count = self
            .active
            .values()
            .filter(|s| s.provider == provider)
            .count();

        if active_count < self.config.max_per_provider {
            // Create new session
            let handle = SessionHandle::new(format!("session-{}", uuid::Uuid::new_v4()), provider);
            let mut session = PooledSession::new(handle, provider);
            session.assign(agent_id.clone());
            self.active.insert(agent_id.clone(), session.clone());
            Ok(session)
        } else {
            // Pool exhausted
            Err(crate::error::DecisionError::EngineError(format!(
                "Session pool exhausted for provider: {}",
                provider
            )))
        }
    }

    /// Release session back to pool
    pub fn release(&mut self, agent_id: &AgentId) {
        if let Some(mut session) = self.active.remove(agent_id) {
            session.release();

            if self.config.reuse_enabled {
                // Return to available pool
                let pool = self
                    .available
                    .entry(session.provider)
                    .or_insert_with(VecDeque::new);

                // Don't exceed pool size
                if pool.len() < self.config.max_per_provider {
                    pool.push_back(session);
                }
            }
        }
    }

    /// Cleanup expired sessions
    pub fn cleanup_expired(&mut self) {
        for pool in self.available.values_mut() {
            pool.retain(|s| !s.is_expired(self.config.idle_timeout_ms));
        }
    }

    /// Get pool statistics
    pub fn stats(&self) -> SessionPoolStats {
        let mut by_provider = HashMap::new();

        // Count active by provider
        for session in self.active.values() {
            *by_provider.entry(session.provider).or_insert(0) += 1;
        }

        // Count available by provider
        for (provider, pool) in &self.available {
            *by_provider.entry(*provider).or_insert(0) += pool.len();
        }

        SessionPoolStats {
            total_active: self.active.len(),
            by_provider,
        }
    }

    /// Clear all sessions
    pub fn clear(&mut self) {
        self.available.clear();
        self.active.clear();
    }
}

impl Default for DecisionSessionPool {
    fn default() -> Self {
        Self::new(SessionPoolConfig::default())
    }
}

/// Rate limit result
#[derive(Debug, Clone)]
pub enum RateLimitResult {
    /// Request allowed
    Allowed { agent_id: AgentId },

    /// Request queued, waiting
    Waiting { position: usize },

    /// Dequeued from waiting queue (minute reset, now allowed)
    /// Bug fix: Separate result type to distinguish from normal allowed
    DequeuedAllowed {
        agent_id: AgentId,
        original_position: usize,
    },
}

/// Rate limit status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateLimitStatus {
    /// Current request count
    pub current_count: u32,

    /// Request limit per minute
    pub limit: u32,

    /// Number waiting in queue
    pub waiting_count: usize,

    /// Remaining seconds in current minute
    pub remaining_in_minute: u64,
}

impl RateLimitStatus {
    pub fn is_at_limit(&self) -> bool {
        self.current_count >= self.limit
    }

    pub fn remaining_requests(&self) -> u32 {
        self.limit.saturating_sub(self.current_count)
    }
}

/// Decision rate limiter - prevents API overload
#[derive(Debug)]
pub struct DecisionRateLimiter {
    /// Requests per minute limit
    requests_per_minute: u32,

    /// Current minute counter
    current_count: u32,

    /// Minute start time
    minute_start: Instant,

    /// Waiting queue
    waiting: VecDeque<AgentId>,
}

impl DecisionRateLimiter {
    pub fn new(requests_per_minute: u32) -> Self {
        Self {
            requests_per_minute,
            current_count: 0,
            minute_start: Instant::now(),
            waiting: VecDeque::new(),
        }
    }

    /// Check if request allowed
    ///
    /// Bug fix: Properly handle minute reset and waiting queue.
    /// When a minute resets, we process waiting agents but don't return
    /// their IDs to unrelated callers.
    pub fn check(&mut self, agent_id: AgentId) -> RateLimitResult {
        // Check if minute passed - need to reset
        let minute_passed = self.minute_start.elapsed() > Duration::from_secs(60);

        if minute_passed {
            // Reset counter for new minute
            self.current_count = 0;
            self.minute_start = Instant::now();
        }

        // Check if this agent is already in waiting queue
        let in_waiting = self.waiting.iter().position(|id| id == &agent_id);

        // If minute passed and there are waiting agents, process them first
        if minute_passed && !self.waiting.is_empty() {
            // Process waiting queue up to limit
            while self.current_count < self.requests_per_minute && !self.waiting.is_empty() {
                let waiting_id = self.waiting.pop_front().unwrap();

                // If the waiting agent is the current caller, allow it
                if waiting_id == agent_id {
                    self.current_count += 1;
                    return RateLimitResult::DequeuedAllowed {
                        agent_id,
                        original_position: 1,
                    };
                }

                // Otherwise, this waiting agent gets a slot but we need to
                // track this separately. For now, we increment the count
                // and the waiting agent will be notified separately.
                // In a real implementation, we'd have a callback mechanism.
                self.current_count += 1;
            }
        }

        // Now check if current agent can be allowed
        if self.current_count < self.requests_per_minute {
            // If agent was in waiting but we didn't process it above (limit reached)
            // remove it from waiting since it's now allowed
            if let Some(pos) = in_waiting {
                self.waiting.remove(pos);
            }
            self.current_count += 1;
            RateLimitResult::Allowed { agent_id }
        } else {
            // At limit - add to waiting queue if not already there
            if in_waiting.is_none() {
                self.waiting.push_back(agent_id.clone());
            }
            let position = self
                .waiting
                .iter()
                .position(|id| id == &agent_id)
                .map(|p| p + 1)
                .unwrap_or(self.waiting.len());
            RateLimitResult::Waiting { position }
        }
    }

    /// Get current status
    pub fn status(&self) -> RateLimitStatus {
        let elapsed = self.minute_start.elapsed().as_secs();
        let remaining = 60 - elapsed.min(60);

        RateLimitStatus {
            current_count: self.current_count,
            limit: self.requests_per_minute,
            waiting_count: self.waiting.len(),
            remaining_in_minute: remaining,
        }
    }

    /// Clear waiting queue
    pub fn clear_waiting(&mut self) {
        self.waiting.clear();
    }

    /// Get waiting queue position for agent
    pub fn waiting_position(&self, agent_id: &AgentId) -> Option<usize> {
        self.waiting
            .iter()
            .position(|id| id == agent_id)
            .map(|p| p + 1)
    }

    /// Remove agent from waiting queue
    pub fn cancel_waiting(&mut self, agent_id: &AgentId) -> bool {
        if let Some(pos) = self.waiting.iter().position(|id| id == agent_id) {
            self.waiting.remove(pos);
            true
        } else {
            false
        }
    }
}

impl Default for DecisionRateLimiter {
    fn default() -> Self {
        Self::new(20) // Default: 20 requests per minute
    }
}

/// Arbitration strategy for human decisions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ArbitrationStrategy {
    /// Handle one at a time, block others
    Sequential,

    /// Batch similar requests, handle together
    BatchSimilar { similarity_threshold: f64 },

    /// Parallel handling (multiple TUI modals - experimental)
    Parallel { max_concurrent: usize },
}

impl Default for ArbitrationStrategy {
    fn default() -> Self {
        ArbitrationStrategy::Sequential
    }
}

/// Arbitration result
#[derive(Debug, Clone)]
pub enum ArbitrationResult {
    /// Request handled immediately
    Immediate { request: HumanDecisionRequest },

    /// Request queued
    Queued { position: usize },

    /// Request batched with existing
    Batched { with_request_id: String },
}

/// Human decision arbitrator - handle multiple human requests
#[derive(Debug)]
pub struct HumanDecisionArbitrator {
    /// Pending requests queue
    queue: HumanDecisionQueue,

    /// Current request being handled
    current: Option<HumanDecisionRequest>,

    /// Arbitration strategy
    strategy: ArbitrationStrategy,

    /// Active concurrent requests (for parallel mode)
    active_parallel: Vec<HumanDecisionRequest>,
}

impl HumanDecisionArbitrator {
    pub fn new(strategy: ArbitrationStrategy, queue: HumanDecisionQueue) -> Self {
        Self {
            queue,
            current: None,
            strategy,
            active_parallel: Vec::new(),
        }
    }

    /// Submit new request
    pub fn submit(&mut self, request: HumanDecisionRequest) -> ArbitrationResult {
        match &self.strategy {
            ArbitrationStrategy::Sequential => {
                if self.current.is_some() {
                    // Add to queue, wait for current to complete
                    self.queue.push(request);
                    ArbitrationResult::Queued {
                        position: self.queue.total_pending(),
                    }
                } else {
                    // Handle immediately
                    self.current = Some(request.clone());
                    ArbitrationResult::Immediate { request }
                }
            }

            ArbitrationStrategy::BatchSimilar {
                similarity_threshold,
            } => {
                // Check if similar to current
                if let Some(current) = &self.current {
                    if self.is_similar(&request, current, *similarity_threshold) {
                        // Batch with current
                        self.queue.push(request);
                        ArbitrationResult::Batched {
                            with_request_id: current.id.clone(),
                        }
                    } else {
                        self.queue.push(request);
                        ArbitrationResult::Queued {
                            position: self.queue.total_pending(),
                        }
                    }
                } else {
                    self.current = Some(request.clone());
                    ArbitrationResult::Immediate { request }
                }
            }

            ArbitrationStrategy::Parallel { max_concurrent } => {
                // Allow multiple concurrent (experimental)
                if self.active_parallel.len() < *max_concurrent {
                    self.active_parallel.push(request.clone());
                    ArbitrationResult::Immediate { request }
                } else {
                    self.queue.push(request);
                    ArbitrationResult::Queued {
                        position: self.queue.total_pending(),
                    }
                }
            }
        }
    }

    /// Complete current request
    pub fn complete(&mut self, response: HumanDecisionResponse) -> Option<HumanDecisionRequest> {
        // Get the request before completing (using find logic from queue)
        let request_id = &response.request_id;
        let request = self.find_request(request_id);

        // Complete in queue
        self.queue.complete(response.clone());

        match &self.strategy {
            ArbitrationStrategy::Sequential => {
                // Move to next in queue
                self.current = self.queue.pop();
            }

            ArbitrationStrategy::BatchSimilar { .. } => {
                // Move to next in queue
                self.current = self.queue.pop();
            }

            ArbitrationStrategy::Parallel { .. } => {
                // Remove from active parallel
                if let Some(ref req) = request {
                    self.active_parallel.retain(|r| r.id != req.id);
                }

                // Fill from queue if space available
                if let Some(next) = self.queue.pop() {
                    self.active_parallel.push(next);
                }
            }
        }

        request
    }

    /// Find request by ID
    fn find_request(&self, id: &str) -> Option<HumanDecisionRequest> {
        // Check current first
        if let Some(ref current) = self.current {
            if current.id == id {
                return Some(current.clone());
            }
        }

        // Check in parallel active
        for req in &self.active_parallel {
            if req.id == id {
                return Some(req.clone());
            }
        }

        // Not found in current active - would need to check queue
        // For simplicity, return None (queue stores requests privately)
        None
    }

    /// Check similarity between two requests
    fn is_similar(
        &self,
        a: &HumanDecisionRequest,
        b: &HumanDecisionRequest,
        threshold: f64,
    ) -> bool {
        a.situation_type == b.situation_type
            && self.options_similarity(&a.options, &b.options) > threshold
    }

    /// Calculate options similarity
    fn options_similarity(
        &self,
        a: &[crate::situation::ChoiceOption],
        b: &[crate::situation::ChoiceOption],
    ) -> f64 {
        if a.is_empty() || b.is_empty() {
            return 0.0;
        }

        let matching = a
            .iter()
            .filter(|opt_a| b.iter().any(|opt_b| opt_a.id == opt_b.id))
            .count();

        matching as f64 / a.len().max(b.len()) as f64
    }

    /// Get current request count
    pub fn active_count(&self) -> usize {
        match &self.strategy {
            ArbitrationStrategy::Sequential => {
                if self.current.is_some() {
                    1
                } else {
                    0
                }
            }
            ArbitrationStrategy::BatchSimilar { .. } => {
                if self.current.is_some() {
                    1
                } else {
                    0
                }
            }
            ArbitrationStrategy::Parallel { .. } => self.active_parallel.len(),
        }
    }

    /// Get total pending count
    pub fn total_pending(&self) -> usize {
        self.active_count() + self.queue.total_pending()
    }

    /// Get strategy type
    pub fn strategy_type(&self) -> &ArbitrationStrategy {
        &self.strategy
    }
}

impl Default for HumanDecisionArbitrator {
    fn default() -> Self {
        Self::new(
            ArbitrationStrategy::default(),
            HumanDecisionQueue::default(),
        )
    }
}

/// Thread-safe session pool wrapper
pub struct ThreadSafeSessionPool {
    pool: Arc<Mutex<DecisionSessionPool>>,
}

impl ThreadSafeSessionPool {
    pub fn new(config: SessionPoolConfig) -> Self {
        Self {
            pool: Arc::new(Mutex::new(DecisionSessionPool::new(config))),
        }
    }

    pub fn acquire(
        &self,
        provider: ProviderKind,
        agent_id: AgentId,
    ) -> crate::error::Result<PooledSession> {
        let mut pool = self.pool.lock().unwrap();
        pool.acquire(provider, agent_id)
    }

    pub fn release(&self, agent_id: &AgentId) {
        let mut pool = self.pool.lock().unwrap();
        pool.release(agent_id);
    }

    pub fn stats(&self) -> SessionPoolStats {
        let pool = self.pool.lock().unwrap();
        pool.stats()
    }

    pub fn cleanup_expired(&self) {
        let mut pool = self.pool.lock().unwrap();
        pool.cleanup_expired();
    }
}

impl Clone for ThreadSafeSessionPool {
    fn clone(&self) -> Self {
        Self {
            pool: self.pool.clone(),
        }
    }
}

/// Thread-safe rate limiter wrapper
pub struct ThreadSafeRateLimiter {
    limiter: Arc<Mutex<DecisionRateLimiter>>,
}

impl ThreadSafeRateLimiter {
    pub fn new(requests_per_minute: u32) -> Self {
        Self {
            limiter: Arc::new(Mutex::new(DecisionRateLimiter::new(requests_per_minute))),
        }
    }

    pub fn check(&self, agent_id: AgentId) -> RateLimitResult {
        let mut limiter = self.limiter.lock().unwrap();
        limiter.check(agent_id)
    }

    pub fn status(&self) -> RateLimitStatus {
        let limiter = self.limiter.lock().unwrap();
        limiter.status()
    }

    pub fn cancel_waiting(&self, agent_id: &AgentId) -> bool {
        let mut limiter = self.limiter.lock().unwrap();
        limiter.cancel_waiting(agent_id)
    }
}

impl Clone for ThreadSafeRateLimiter {
    fn clone(&self) -> Self {
        Self {
            limiter: self.limiter.clone(),
        }
    }
}

/// Thread-safe arbitrator wrapper
pub struct ThreadSafeArbitrator {
    arbitrator: Arc<Mutex<HumanDecisionArbitrator>>,
}

impl ThreadSafeArbitrator {
    pub fn new(strategy: ArbitrationStrategy) -> Self {
        Self {
            arbitrator: Arc::new(Mutex::new(HumanDecisionArbitrator::new(
                strategy,
                HumanDecisionQueue::default(),
            ))),
        }
    }

    pub fn submit(&self, request: HumanDecisionRequest) -> ArbitrationResult {
        let mut arb = self.arbitrator.lock().unwrap();
        arb.submit(request)
    }

    pub fn complete(&self, response: HumanDecisionResponse) -> Option<HumanDecisionRequest> {
        let mut arb = self.arbitrator.lock().unwrap();
        arb.complete(response)
    }

    pub fn total_pending(&self) -> usize {
        let arb = self.arbitrator.lock().unwrap();
        arb.total_pending()
    }
}

impl Clone for ThreadSafeArbitrator {
    fn clone(&self) -> Self {
        Self {
            arbitrator: self.arbitrator.clone(),
        }
    }
}

impl Default for ThreadSafeSessionPool {
    fn default() -> Self {
        Self::new(SessionPoolConfig::default())
    }
}

impl Default for ThreadSafeRateLimiter {
    fn default() -> Self {
        Self::new(20)
    }
}

impl Default for ThreadSafeArbitrator {
    fn default() -> Self {
        Self::new(ArbitrationStrategy::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::situation::ChoiceOption;
    use crate::types::{SituationType, UrgencyLevel};

    // Helper function to create HumanDecisionRequest for tests
    fn create_test_request(
        id: &str,
        agent_id: &str,
        options: Vec<ChoiceOption>,
    ) -> HumanDecisionRequest {
        HumanDecisionRequest::new(
            id.to_string(),
            agent_id.to_string(),
            SituationType::new("test"),
            options,
            UrgencyLevel::Medium,
            60000,
        )
    }

    #[test]
    fn test_session_pool_config_default() {
        let config = SessionPoolConfig::default();
        assert_eq!(config.max_per_provider, 3);
        assert_eq!(config.idle_timeout_ms, 1800000);
        assert!(config.reuse_enabled);
    }

    #[test]
    fn test_session_pool_config_serde() {
        let config = SessionPoolConfig::default();
        let json = serde_json::to_string(&config).unwrap();
        let parsed: SessionPoolConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(config.max_per_provider, parsed.max_per_provider);
    }

    #[test]
    fn test_session_pool_stats_new() {
        let stats = SessionPoolStats::new();
        assert_eq!(stats.total_active, 0);
        assert!(stats.by_provider.is_empty());
    }

    #[test]
    fn test_pooled_session_new() {
        let handle = SessionHandle::new("test-session", ProviderKind::Claude);
        let session = PooledSession::new(handle, ProviderKind::Claude);

        assert_eq!(session.provider, ProviderKind::Claude);
        assert!(!session.is_assigned());
    }

    #[test]
    fn test_pooled_session_assign_release() {
        let handle = SessionHandle::new("test-session", ProviderKind::Claude);
        let mut session = PooledSession::new(handle, ProviderKind::Claude);

        session.assign(AgentId::new("agent-1"));
        assert!(session.is_assigned());
        assert_eq!(session.assigned_to, Some(AgentId::new("agent-1")));

        session.release();
        assert!(!session.is_assigned());
    }

    #[test]
    fn test_pooled_session_expired() {
        let handle = SessionHandle::new("test-session", ProviderKind::Claude);
        let session = PooledSession::new(handle, ProviderKind::Claude);

        // Fresh session should not be expired
        assert!(!session.is_expired(60000));
    }

    #[test]
    fn test_decision_session_pool_new() {
        let pool = DecisionSessionPool::new(SessionPoolConfig::default());
        assert!(!pool.has_session(&AgentId::new("agent-1")));
    }

    #[test]
    fn test_decision_session_pool_acquire() {
        let mut pool = DecisionSessionPool::new(SessionPoolConfig::default());
        let result = pool.acquire(ProviderKind::Claude, AgentId::new("agent-1"));

        assert!(result.is_ok());
        let session = result.unwrap();
        assert!(session.is_assigned());
        assert!(pool.has_session(&AgentId::new("agent-1")));
    }

    #[test]
    fn test_decision_session_pool_acquire_same_agent() {
        let mut pool = DecisionSessionPool::new(SessionPoolConfig::default());

        // First acquire
        let session1 = pool
            .acquire(ProviderKind::Claude, AgentId::new("agent-1"))
            .unwrap();

        // Same agent - should return same session
        let session2 = pool
            .acquire(ProviderKind::Claude, AgentId::new("agent-1"))
            .unwrap();

        assert_eq!(session1.handle.session_id, session2.handle.session_id);
    }

    #[test]
    fn test_decision_session_pool_release() {
        let mut pool = DecisionSessionPool::new(SessionPoolConfig::default());
        pool.acquire(ProviderKind::Claude, AgentId::new("agent-1"))
            .unwrap();

        pool.release(&AgentId::new("agent-1"));

        assert!(!pool.has_session(&AgentId::new("agent-1")));
    }

    #[test]
    fn test_decision_session_pool_exhausted() {
        let mut pool = DecisionSessionPool::new(SessionPoolConfig {
            max_per_provider: 2,
            idle_timeout_ms: 1800000,
            reuse_enabled: false,
        });

        // Acquire max sessions
        pool.acquire(ProviderKind::Claude, AgentId::new("agent-1"))
            .unwrap();
        pool.acquire(ProviderKind::Claude, AgentId::new("agent-2"))
            .unwrap();

        // Should fail for third agent
        let result = pool.acquire(ProviderKind::Claude, AgentId::new("agent-3"));
        assert!(result.is_err());
    }

    #[test]
    fn test_decision_session_pool_stats() {
        let mut pool = DecisionSessionPool::new(SessionPoolConfig::default());
        pool.acquire(ProviderKind::Claude, AgentId::new("agent-1"))
            .unwrap();
        pool.acquire(ProviderKind::Codex, AgentId::new("agent-2"))
            .unwrap();

        let stats = pool.stats();

        assert_eq!(stats.total_active, 2);
        assert_eq!(stats.by_provider.get(&ProviderKind::Claude), Some(&1));
        assert_eq!(stats.by_provider.get(&ProviderKind::Codex), Some(&1));
    }

    #[test]
    fn test_decision_session_pool_cleanup_expired() {
        let mut pool = DecisionSessionPool::new(SessionPoolConfig {
            max_per_provider: 3,
            idle_timeout_ms: 100, // Very short timeout for testing
            reuse_enabled: true,
        });

        // Acquire and release
        pool.acquire(ProviderKind::Claude, AgentId::new("agent-1"))
            .unwrap();
        pool.release(&AgentId::new("agent-1"));

        // Wait for timeout
        std::thread::sleep(Duration::from_millis(150));

        pool.cleanup_expired();

        // Available pool should be empty after cleanup
        let available = pool.available.get(&ProviderKind::Claude);
        assert!(available.is_none() || available.unwrap().is_empty());
    }

    #[test]
    fn test_decision_session_pool_clear() {
        let mut pool = DecisionSessionPool::new(SessionPoolConfig::default());
        pool.acquire(ProviderKind::Claude, AgentId::new("agent-1"))
            .unwrap();
        pool.clear();

        assert!(pool.active.is_empty());
        assert!(pool.available.is_empty());
    }

    #[test]
    fn test_rate_limit_result() {
        let allowed = RateLimitResult::Allowed {
            agent_id: AgentId::new("agent-1"),
        };
        let waiting = RateLimitResult::Waiting { position: 5 };

        assert!(matches!(allowed, RateLimitResult::Allowed { .. }));
        assert!(matches!(waiting, RateLimitResult::Waiting { position: 5 }));
    }

    #[test]
    fn test_rate_limit_status() {
        let status = RateLimitStatus {
            current_count: 15,
            limit: 20,
            waiting_count: 3,
            remaining_in_minute: 30,
        };

        assert!(!status.is_at_limit());
        assert_eq!(status.remaining_requests(), 5);
    }

    #[test]
    fn test_rate_limit_status_at_limit() {
        let status = RateLimitStatus {
            current_count: 20,
            limit: 20,
            waiting_count: 5,
            remaining_in_minute: 30,
        };

        assert!(status.is_at_limit());
        assert_eq!(status.remaining_requests(), 0);
    }

    #[test]
    fn test_decision_rate_limiter_new() {
        let limiter = DecisionRateLimiter::new(10);
        let status = limiter.status();

        assert_eq!(status.limit, 10);
        assert_eq!(status.current_count, 0);
    }

    #[test]
    fn test_decision_rate_limiter_allowed() {
        let mut limiter = DecisionRateLimiter::new(5);

        let result = limiter.check(AgentId::new("agent-1"));
        assert!(matches!(result, RateLimitResult::Allowed { .. }));

        let status = limiter.status();
        assert_eq!(status.current_count, 1);
    }

    #[test]
    fn test_decision_rate_limiter_waiting() {
        let mut limiter = DecisionRateLimiter::new(2);

        // Fill up limit
        limiter.check(AgentId::new("agent-1"));
        limiter.check(AgentId::new("agent-2"));

        // Third request should wait
        let result = limiter.check(AgentId::new("agent-3"));
        assert!(matches!(result, RateLimitResult::Waiting { position: 1 }));

        let status = limiter.status();
        assert_eq!(status.waiting_count, 1);
    }

    #[test]
    fn test_decision_rate_limiter_cancel_waiting() {
        let mut limiter = DecisionRateLimiter::new(2);

        limiter.check(AgentId::new("agent-1"));
        limiter.check(AgentId::new("agent-2"));
        limiter.check(AgentId::new("agent-3"));

        assert!(limiter.cancel_waiting(&AgentId::new("agent-3")));
        assert_eq!(limiter.status().waiting_count, 0);
    }

    #[test]
    fn test_decision_rate_limiter_waiting_position() {
        let mut limiter = DecisionRateLimiter::new(1);

        limiter.check(AgentId::new("agent-1"));
        limiter.check(AgentId::new("agent-2"));
        limiter.check(AgentId::new("agent-3"));

        assert_eq!(limiter.waiting_position(&AgentId::new("agent-2")), Some(1));
        assert_eq!(limiter.waiting_position(&AgentId::new("agent-3")), Some(2));
    }

    #[test]
    fn test_decision_rate_limiter_clear_waiting() {
        let mut limiter = DecisionRateLimiter::new(1);

        limiter.check(AgentId::new("agent-1"));
        limiter.check(AgentId::new("agent-2"));

        limiter.clear_waiting();
        assert_eq!(limiter.status().waiting_count, 0);
    }

    #[test]
    fn test_arbitration_strategy_default() {
        let strategy = ArbitrationStrategy::default();
        assert!(matches!(strategy, ArbitrationStrategy::Sequential));
    }

    #[test]
    fn test_arbitration_result() {
        let immediate = ArbitrationResult::Immediate {
            request: create_test_request("req-1", "agent-1", vec![]),
        };
        let queued = ArbitrationResult::Queued { position: 3 };
        let batched = ArbitrationResult::Batched {
            with_request_id: "req-1".to_string(),
        };

        assert!(matches!(immediate, ArbitrationResult::Immediate { .. }));
        assert!(matches!(queued, ArbitrationResult::Queued { position: 3 }));
        assert!(matches!(batched, ArbitrationResult::Batched { .. }));
    }

    #[test]
    fn test_human_decision_arbitrator_new() {
        let arb = HumanDecisionArbitrator::default();
        assert_eq!(arb.active_count(), 0);
        assert_eq!(arb.total_pending(), 0);
    }

    #[test]
    fn test_human_decision_arbitrator_submit_immediate() {
        let mut arb = HumanDecisionArbitrator::default();
        let request = create_test_request(
            "req-1",
            "agent-1",
            vec![ChoiceOption::new("opt-1", "Option 1")],
        );

        let result = arb.submit(request);

        assert!(matches!(result, ArbitrationResult::Immediate { .. }));
        assert_eq!(arb.active_count(), 1);
    }

    #[test]
    fn test_human_decision_arbitrator_submit_queued() {
        let mut arb = HumanDecisionArbitrator::default();

        // Submit first request
        let request1 = create_test_request(
            "req-1",
            "agent-1",
            vec![ChoiceOption::new("opt-1", "Option 1")],
        );
        arb.submit(request1);

        // Submit second request - should queue in sequential mode
        let request2 = create_test_request(
            "req-2",
            "agent-2",
            vec![ChoiceOption::new("opt-1", "Option 1")],
        );
        let result = arb.submit(request2);

        assert!(matches!(result, ArbitrationResult::Queued { .. }));
        assert_eq!(arb.total_pending(), 2);
    }

    #[test]
    fn test_human_decision_arbitrator_complete() {
        let mut arb = HumanDecisionArbitrator::default();

        let request = create_test_request(
            "req-1",
            "agent-1",
            vec![ChoiceOption::new("opt-1", "Option 1")],
        );
        arb.submit(request);

        let response = HumanDecisionResponse::new(
            "req-1".to_string(),
            crate::blocking::HumanSelection::Selected {
                option_id: "opt-1".to_string(),
            },
        );

        let completed = arb.complete(response);

        assert!(completed.is_some());
        assert_eq!(arb.active_count(), 0);
    }

    #[test]
    fn test_human_decision_arbitrator_parallel() {
        let mut arb = HumanDecisionArbitrator::new(
            ArbitrationStrategy::Parallel { max_concurrent: 2 },
            HumanDecisionQueue::default(),
        );

        // Submit two requests - should both be immediate
        let request1 = create_test_request(
            "req-1",
            "agent-1",
            vec![ChoiceOption::new("opt-1", "Option 1")],
        );
        let result1 = arb.submit(request1);
        assert!(matches!(result1, ArbitrationResult::Immediate { .. }));

        let request2 = create_test_request(
            "req-2",
            "agent-2",
            vec![ChoiceOption::new("opt-2", "Option 2")],
        );
        let result2 = arb.submit(request2);
        assert!(matches!(result2, ArbitrationResult::Immediate { .. }));

        // Third request should queue
        let request3 = create_test_request(
            "req-3",
            "agent-3",
            vec![ChoiceOption::new("opt-3", "Option 3")],
        );
        let result3 = arb.submit(request3);
        assert!(matches!(result3, ArbitrationResult::Queued { .. }));

        assert_eq!(arb.active_count(), 2);
    }

    #[test]
    fn test_human_decision_arbitrator_options_similarity() {
        let arb = HumanDecisionArbitrator::default();

        let options_a = vec![
            ChoiceOption::new("opt-1", "Option 1"),
            ChoiceOption::new("opt-2", "Option 2"),
        ];

        let options_b = vec![
            ChoiceOption::new("opt-1", "Option 1"),
            ChoiceOption::new("opt-3", "Option 3"),
        ];

        let options_c = vec![
            ChoiceOption::new("opt-4", "Option 4"),
            ChoiceOption::new("opt-5", "Option 5"),
        ];

        // Similarity between a and b: 1 matching / 2 max = 0.5
        let sim_ab = arb.options_similarity(&options_a, &options_b);
        assert_eq!(sim_ab, 0.5);

        // Similarity between a and c: 0 matching / 2 max = 0.0
        let sim_ac = arb.options_similarity(&options_a, &options_c);
        assert_eq!(sim_ac, 0.0);

        // Similarity between empty and non-empty: 0.0
        let sim_empty = arb.options_similarity(&[], &options_a);
        assert_eq!(sim_empty, 0.0);
    }

    #[test]
    fn test_thread_safe_session_pool() {
        let pool = ThreadSafeSessionPool::new(SessionPoolConfig::default());

        let result = pool.acquire(ProviderKind::Claude, AgentId::new("agent-1"));
        assert!(result.is_ok());

        pool.release(&AgentId::new("agent-1"));

        let stats = pool.stats();
        assert_eq!(stats.total_active, 0);
    }

    #[test]
    fn test_thread_safe_session_pool_clone() {
        let pool1 = ThreadSafeSessionPool::new(SessionPoolConfig::default());
        let pool2 = pool1.clone();

        pool1
            .acquire(ProviderKind::Claude, AgentId::new("agent-1"))
            .unwrap();

        // Both pools share the same underlying pool
        let stats = pool2.stats();
        assert_eq!(stats.total_active, 1);
    }

    #[test]
    fn test_thread_safe_rate_limiter() {
        let limiter = ThreadSafeRateLimiter::new(5);

        let result = limiter.check(AgentId::new("agent-1"));
        assert!(matches!(result, RateLimitResult::Allowed { .. }));

        let status = limiter.status();
        assert_eq!(status.current_count, 1);
    }

    #[test]
    fn test_thread_safe_rate_limiter_clone() {
        let limiter1 = ThreadSafeRateLimiter::new(5);
        let limiter2 = limiter1.clone();

        limiter1.check(AgentId::new("agent-1"));

        // Both share the same underlying limiter
        let status = limiter2.status();
        assert_eq!(status.current_count, 1);
    }

    #[test]
    fn test_thread_safe_arbitrator() {
        let arb = ThreadSafeArbitrator::new(ArbitrationStrategy::Sequential);

        let request = create_test_request(
            "req-1",
            "agent-1",
            vec![ChoiceOption::new("opt-1", "Option 1")],
        );

        let result = arb.submit(request);
        assert!(matches!(result, ArbitrationResult::Immediate { .. }));

        assert_eq!(arb.total_pending(), 1);
    }

    #[test]
    fn test_thread_safe_arbitrator_clone() {
        let arb1 = ThreadSafeArbitrator::new(ArbitrationStrategy::Sequential);
        let arb2 = arb1.clone();

        let request = create_test_request(
            "req-1",
            "agent-1",
            vec![ChoiceOption::new("opt-1", "Option 1")],
        );

        arb1.submit(request);

        // Both share the same underlying arbitrator
        assert_eq!(arb2.total_pending(), 1);
    }

    #[test]
    fn test_arbitration_strategy_serde() {
        let strategy = ArbitrationStrategy::BatchSimilar {
            similarity_threshold: 0.8,
        };
        let json = serde_json::to_string(&strategy).unwrap();
        let parsed: ArbitrationStrategy = serde_json::from_str(&json).unwrap();

        assert!(matches!(
            parsed,
            ArbitrationStrategy::BatchSimilar {
                similarity_threshold: 0.8
            }
        ));
    }

    #[test]
    fn test_rate_limit_status_serde() {
        let status = RateLimitStatus {
            current_count: 10,
            limit: 20,
            waiting_count: 5,
            remaining_in_minute: 30,
        };

        let json = serde_json::to_string(&status).unwrap();
        let parsed: RateLimitStatus = serde_json::from_str(&json).unwrap();

        assert_eq!(status.current_count, parsed.current_count);
        assert_eq!(status.limit, parsed.limit);
    }

    #[test]
    fn test_session_pool_stats_serde() {
        let mut stats = SessionPoolStats::new();
        stats.total_active = 5;
        stats.by_provider.insert(ProviderKind::Claude, 3);

        let json = serde_json::to_string(&stats).unwrap();
        let parsed: SessionPoolStats = serde_json::from_str(&json).unwrap();

        assert_eq!(stats.total_active, parsed.total_active);
    }
}
