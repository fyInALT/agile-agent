//! Decision observability - metrics collection and logging
//!
//! Sprint 8.5: Provides metrics collection, success rate tracking,
//! and structured decision logging for observability.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Decision metrics collected for observability
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecisionMetrics {
    /// Total decisions made
    pub total_decisions: u64,

    /// Successful decisions
    pub successful_decisions: u64,

    /// Failed decisions
    pub failed_decisions: u64,

    /// Decisions by situation type
    pub by_situation_type: HashMap<String, u64>,

    /// Decisions by action type
    pub by_action_type: HashMap<String, u64>,

    /// Average decision duration in milliseconds
    pub avg_duration_ms: u64,

    /// Maximum decision duration in milliseconds
    pub max_duration_ms: u64,

    /// Minimum decision duration in milliseconds
    pub min_duration_ms: u64,

    /// Total human interventions requested
    pub human_interventions: u64,

    /// Human interventions accepted
    pub human_accepted: u64,

    /// Human interventions rejected
    pub human_rejected: u64,

    /// Human interventions timed out
    pub human_timed_out: u64,

    /// Timeout fallbacks
    pub timeout_fallbacks: u64,

    /// Engine switches
    pub engine_switches: u64,

    /// Retries
    pub retries: u64,

    /// Metrics collection start time
    pub start_time: DateTime<Utc>,

    /// Last update time
    pub last_update: DateTime<Utc>,
}

impl DecisionMetrics {
    pub fn new() -> Self {
        Self {
            total_decisions: 0,
            successful_decisions: 0,
            failed_decisions: 0,
            by_situation_type: HashMap::new(),
            by_action_type: HashMap::new(),
            avg_duration_ms: 0,
            max_duration_ms: 0,
            min_duration_ms: 0,
            human_interventions: 0,
            human_accepted: 0,
            human_rejected: 0,
            human_timed_out: 0,
            timeout_fallbacks: 0,
            engine_switches: 0,
            retries: 0,
            start_time: Utc::now(),
            last_update: Utc::now(),
        }
    }

    /// Calculate success rate
    pub fn success_rate(&self) -> f64 {
        if self.total_decisions == 0 {
            1.0
        } else {
            self.successful_decisions as f64 / self.total_decisions as f64
        }
    }

    /// Calculate failure rate
    pub fn failure_rate(&self) -> f64 {
        if self.total_decisions == 0 {
            0.0
        } else {
            self.failed_decisions as f64 / self.total_decisions as f64
        }
    }

    /// Calculate human intervention rate
    pub fn human_intervention_rate(&self) -> f64 {
        if self.total_decisions == 0 {
            0.0
        } else {
            self.human_interventions as f64 / self.total_decisions as f64
        }
    }

    /// Record a successful decision
    pub fn record_success(&mut self, situation_type: &str, action_type: &str, duration_ms: u64) {
        self.total_decisions += 1;
        self.successful_decisions += 1;
        self.record_situation_type(situation_type);
        self.record_action_type(action_type);
        self.update_duration_stats(duration_ms);
        self.last_update = Utc::now();
    }

    /// Record a failed decision
    pub fn record_failure(&mut self, situation_type: &str, duration_ms: u64) {
        self.total_decisions += 1;
        self.failed_decisions += 1;
        self.record_situation_type(situation_type);
        self.update_duration_stats(duration_ms);
        self.last_update = Utc::now();
    }

    /// Record human intervention request
    pub fn record_human_intervention(&mut self) {
        self.human_interventions += 1;
        self.last_update = Utc::now();
    }

    /// Record human response
    pub fn record_human_response(&mut self, accepted: bool) {
        if accepted {
            self.human_accepted += 1;
        } else {
            self.human_rejected += 1;
        }
        self.last_update = Utc::now();
    }

    /// Record human timeout
    pub fn record_human_timeout(&mut self) {
        self.human_timed_out += 1;
        self.last_update = Utc::now();
    }

    /// Record timeout fallback
    pub fn record_timeout_fallback(&mut self) {
        self.timeout_fallbacks += 1;
        self.last_update = Utc::now();
    }

    /// Record engine switch
    pub fn record_engine_switch(&mut self) {
        self.engine_switches += 1;
        self.last_update = Utc::now();
    }

    /// Record retry
    pub fn record_retry(&mut self) {
        self.retries += 1;
        self.last_update = Utc::now();
    }

    /// Record situation type
    fn record_situation_type(&mut self, situation_type: &str) {
        *self.by_situation_type.entry(situation_type.to_string()).or_insert(0) += 1;
    }

    /// Record action type
    fn record_action_type(&mut self, action_type: &str) {
        *self.by_action_type.entry(action_type.to_string()).or_insert(0) += 1;
    }

    /// Update duration statistics
    fn update_duration_stats(&mut self, duration_ms: u64) {
        if self.total_decisions == 1 {
            self.avg_duration_ms = duration_ms;
            self.max_duration_ms = duration_ms;
            self.min_duration_ms = duration_ms;
        } else {
            // Update average (approximation for rolling average)
            let total_duration = self.avg_duration_ms * (self.total_decisions - 1) + duration_ms;
            self.avg_duration_ms = total_duration / self.total_decisions;

            if duration_ms > self.max_duration_ms {
                self.max_duration_ms = duration_ms;
            }
            if duration_ms < self.min_duration_ms {
                self.min_duration_ms = duration_ms;
            }
        }
    }

    /// Get elapsed time since start in seconds
    pub fn elapsed_seconds(&self) -> i64 {
        (self.last_update - self.start_time).num_seconds()
    }

    /// Get decisions per second
    pub fn decisions_per_second(&self) -> f64 {
        let elapsed = self.elapsed_seconds();
        if elapsed <= 0 {
            0.0
        } else {
            self.total_decisions as f64 / elapsed as f64
        }
    }

    /// Reset metrics
    pub fn reset(&mut self) {
        self.total_decisions = 0;
        self.successful_decisions = 0;
        self.failed_decisions = 0;
        self.by_situation_type.clear();
        self.by_action_type.clear();
        self.avg_duration_ms = 0;
        self.max_duration_ms = 0;
        self.min_duration_ms = 0;
        self.human_interventions = 0;
        self.human_accepted = 0;
        self.human_rejected = 0;
        self.human_timed_out = 0;
        self.timeout_fallbacks = 0;
        self.engine_switches = 0;
        self.retries = 0;
        self.start_time = Utc::now();
        self.last_update = Utc::now();
    }

    /// Merge metrics from another instance
    pub fn merge(&mut self, other: &DecisionMetrics) {
        self.total_decisions += other.total_decisions;
        self.successful_decisions += other.successful_decisions;
        self.failed_decisions += other.failed_decisions;

        for (k, v) in &other.by_situation_type {
            *self.by_situation_type.entry(k.clone()).or_insert(0) += v;
        }

        for (k, v) in &other.by_action_type {
            *self.by_action_type.entry(k.clone()).or_insert(0) += v;
        }

        self.human_interventions += other.human_interventions;
        self.human_accepted += other.human_accepted;
        self.human_rejected += other.human_rejected;
        self.human_timed_out += other.human_timed_out;
        self.timeout_fallbacks += other.timeout_fallbacks;
        self.engine_switches += other.engine_switches;
        self.retries += other.retries;

        // Update max/min durations
        if other.max_duration_ms > self.max_duration_ms {
            self.max_duration_ms = other.max_duration_ms;
        }
        if other.min_duration_ms < self.min_duration_ms || self.min_duration_ms == 0 {
            self.min_duration_ms = other.min_duration_ms;
        }

        // Recalculate average
        if self.total_decisions > 0 {
            let total_avg = (self.avg_duration_ms * (self.total_decisions - other.total_decisions)
                + other.avg_duration_ms * other.total_decisions)
                / self.total_decisions;
            self.avg_duration_ms = total_avg;
        }

        self.last_update = Utc::now();
    }
}

impl Default for DecisionMetrics {
    fn default() -> Self {
        Self::new()
    }
}

/// Structured decision log entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecisionLogEntry {
    /// Decision ID
    pub decision_id: String,

    /// Timestamp
    pub timestamp: DateTime<Utc>,

    /// Agent ID
    pub agent_id: String,

    /// Situation type
    pub situation_type: String,

    /// Situation subtype (if applicable)
    pub situation_subtype: Option<String>,

    /// Action type chosen
    pub action_type: String,

    /// Decision duration in milliseconds
    pub duration_ms: u64,

    /// Engine type used
    pub engine_type: String,

    /// Confidence level (if applicable)
    pub confidence: Option<f64>,

    /// Whether human intervention was requested
    pub human_requested: bool,

    /// Retry count at time of decision
    pub retry_count: u8,

    /// Reflection rounds at time of decision
    pub reflection_rounds: u8,

    /// Outcome status
    pub outcome: DecisionOutcome,

    /// Additional context
    pub context: HashMap<String, String>,
}

impl DecisionLogEntry {
    pub fn new(
        decision_id: String,
        agent_id: String,
        situation_type: String,
        action_type: String,
    ) -> Self {
        Self {
            decision_id,
            timestamp: Utc::now(),
            agent_id,
            situation_type,
            situation_subtype: None,
            action_type,
            duration_ms: 0,
            engine_type: "unknown".to_string(),
            confidence: None,
            human_requested: false,
            retry_count: 0,
            reflection_rounds: 0,
            outcome: DecisionOutcome::Pending,
            context: HashMap::new(),
        }
    }

    pub fn with_duration(mut self, duration_ms: u64) -> Self {
        self.duration_ms = duration_ms;
        self
    }

    pub fn with_engine_type(mut self, engine_type: String) -> Self {
        self.engine_type = engine_type;
        self
    }

    pub fn with_confidence(mut self, confidence: f64) -> Self {
        self.confidence = Some(confidence);
        self
    }

    pub fn with_human_requested(mut self, human_requested: bool) -> Self {
        self.human_requested = human_requested;
        self
    }

    pub fn with_retry_count(mut self, retry_count: u8) -> Self {
        self.retry_count = retry_count;
        self
    }

    pub fn with_reflection_rounds(mut self, reflection_rounds: u8) -> Self {
        self.reflection_rounds = reflection_rounds;
        self
    }

    pub fn with_outcome(mut self, outcome: DecisionOutcome) -> Self {
        self.outcome = outcome;
        self
    }

    pub fn with_context(mut self, key: String, value: String) -> Self {
        self.context.insert(key, value);
        self
    }

    pub fn is_success(&self) -> bool {
        matches!(self.outcome, DecisionOutcome::Success)
    }

    pub fn is_failure(&self) -> bool {
        matches!(self.outcome, DecisionOutcome::Failed)
    }

    pub fn is_pending(&self) -> bool {
        matches!(self.outcome, DecisionOutcome::Pending)
    }

    /// Convert to JSON string
    pub fn to_json(&self) -> crate::error::Result<String> {
        serde_json::to_string(self).map_err(crate::error::DecisionError::JsonError)
    }
}

/// Decision outcome status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DecisionOutcome {
    /// Decision is pending execution
    Pending,

    /// Decision executed successfully
    Success,

    /// Decision execution failed
    Failed,

    /// Decision was skipped
    Skipped,

    /// Decision was overridden by human
    HumanOverride,
}

impl DecisionOutcome {
    pub fn is_final(&self) -> bool {
        matches!(
            self,
            DecisionOutcome::Success
                | DecisionOutcome::Failed
                | DecisionOutcome::Skipped
                | DecisionOutcome::HumanOverride
        )
    }
}

impl std::fmt::Display for DecisionOutcome {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DecisionOutcome::Pending => write!(f, "pending"),
            DecisionOutcome::Success => write!(f, "success"),
            DecisionOutcome::Failed => write!(f, "failed"),
            DecisionOutcome::Skipped => write!(f, "skipped"),
            DecisionOutcome::HumanOverride => write!(f, "human_override"),
        }
    }
}

/// Decision log storage
#[derive(Debug, Clone, Default)]
pub struct DecisionLog {
    /// Log entries
    entries: Vec<DecisionLogEntry>,

    /// Maximum entries to keep
    max_entries: usize,
}

impl DecisionLog {
    pub fn new(max_entries: usize) -> Self {
        Self {
            entries: Vec::new(),
            max_entries,
        }
    }

    /// Add entry
    pub fn add(&mut self, entry: DecisionLogEntry) {
        self.entries.push(entry);

        // Trim if exceeds max
        if self.entries.len() > self.max_entries {
            self.entries.remove(0);
        }
    }

    /// Get all entries
    pub fn entries(&self) -> &[DecisionLogEntry] {
        &self.entries
    }

    /// Get entries for agent
    pub fn for_agent(&self, agent_id: &str) -> Vec<&DecisionLogEntry> {
        self.entries
            .iter()
            .filter(|e| e.agent_id == agent_id)
            .collect()
    }

    /// Get entries by situation type
    pub fn by_situation(&self, situation_type: &str) -> Vec<&DecisionLogEntry> {
        self.entries
            .iter()
            .filter(|e| e.situation_type == situation_type)
            .collect()
    }

    /// Get entries by outcome
    pub fn by_outcome(&self, outcome: DecisionOutcome) -> Vec<&DecisionLogEntry> {
        self.entries
            .iter()
            .filter(|e| e.outcome == outcome)
            .collect()
    }

    /// Get recent entries
    pub fn recent(&self, count: usize) -> Vec<&DecisionLogEntry> {
        let start = self.entries.len().saturating_sub(count);
        self.entries[start..].iter().collect()
    }

    /// Clear log
    pub fn clear(&mut self) {
        self.entries.clear();
    }

    /// Export to JSON
    pub fn to_json(&self) -> crate::error::Result<String> {
        serde_json::to_string(&self.entries).map_err(crate::error::DecisionError::JsonError)
    }

    /// Count entries
    pub fn count(&self) -> usize {
        self.entries.len()
    }
}

/// CLI metrics summary for display
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricsSummary {
    /// Total decisions
    pub total_decisions: u64,

    /// Success rate percentage
    pub success_rate_pct: f64,

    /// Average duration
    pub avg_duration_ms: u64,

    /// Human intervention rate percentage
    pub human_intervention_rate_pct: f64,

    /// Top situation types
    pub top_situation_types: Vec<(String, u64)>,

    /// Top action types
    pub top_action_types: Vec<(String, u64)>,

    /// Collection period in seconds
    pub collection_period_seconds: i64,

    /// Decisions per second
    pub decisions_per_second: f64,
}

impl MetricsSummary {
    pub fn from_metrics(metrics: &DecisionMetrics) -> Self {
        let mut top_situation_types: Vec<(String, u64)> = metrics
            .by_situation_type
            .iter()
            .map(|(k, v)| (k.clone(), *v))
            .collect();
        top_situation_types.sort_by(|a, b| b.1.cmp(&a.1));
        top_situation_types.truncate(5);

        let mut top_action_types: Vec<(String, u64)> = metrics
            .by_action_type
            .iter()
            .map(|(k, v)| (k.clone(), *v))
            .collect();
        top_action_types.sort_by(|a, b| b.1.cmp(&a.1));
        top_action_types.truncate(5);

        Self {
            total_decisions: metrics.total_decisions,
            success_rate_pct: metrics.success_rate() * 100.0,
            avg_duration_ms: metrics.avg_duration_ms,
            human_intervention_rate_pct: metrics.human_intervention_rate() * 100.0,
            top_situation_types,
            top_action_types,
            collection_period_seconds: metrics.elapsed_seconds(),
            decisions_per_second: metrics.decisions_per_second(),
        }
    }

    /// Format for CLI display
    pub fn to_display(&self) -> String {
        format!(
            "Decision Metrics Summary:\n\
             ========================\n\
             Total Decisions: {}\n\
             Success Rate: {:.1}%\n\
             Avg Duration: {}ms\n\
             Human Intervention: {:.1}%\n\
             Throughput: {:.2} decisions/sec\n\
             Collection Period: {}s\n\
             \n\
             Top Situation Types:\n\
             {}\n\
             \n\
             Top Action Types:\n\
             {}",
            self.total_decisions,
            self.success_rate_pct,
            self.avg_duration_ms,
            self.human_intervention_rate_pct,
            self.decisions_per_second,
            self.collection_period_seconds,
            self.format_top_items(&self.top_situation_types),
            self.format_top_items(&self.top_action_types)
        )
    }

    fn format_top_items(&self, items: &[(String, u64)]) -> String {
        items
            .iter()
            .map(|(name, count)| format!("  - {}: {}", name, count))
            .collect::<Vec<String>>()
            .join("\n")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_decision_metrics_new() {
        let metrics = DecisionMetrics::new();
        assert_eq!(metrics.total_decisions, 0);
        assert_eq!(metrics.successful_decisions, 0);
        assert_eq!(metrics.failed_decisions, 0);
        assert_eq!(metrics.avg_duration_ms, 0);
        assert!(metrics.by_situation_type.is_empty());
        assert!(metrics.by_action_type.is_empty());
    }

    #[test]
    fn test_decision_metrics_success_rate() {
        let metrics = DecisionMetrics::new();
        assert_eq!(metrics.success_rate(), 1.0); // No decisions = 100%

        let mut metrics = DecisionMetrics::new();
        metrics.total_decisions = 10;
        metrics.successful_decisions = 7;
        assert_eq!(metrics.success_rate(), 0.7);
    }

    #[test]
    fn test_decision_metrics_failure_rate() {
        let metrics = DecisionMetrics::new();
        assert_eq!(metrics.failure_rate(), 0.0);

        let mut metrics = DecisionMetrics::new();
        metrics.total_decisions = 10;
        metrics.failed_decisions = 3;
        assert_eq!(metrics.failure_rate(), 0.3);
    }

    #[test]
    fn test_decision_metrics_record_success() {
        let mut metrics = DecisionMetrics::new();
        metrics.record_success("waiting_for_choice", "select_option", 100);

        assert_eq!(metrics.total_decisions, 1);
        assert_eq!(metrics.successful_decisions, 1);
        assert_eq!(metrics.avg_duration_ms, 100);
        assert_eq!(metrics.max_duration_ms, 100);
        assert_eq!(metrics.min_duration_ms, 100);
        assert_eq!(metrics.by_situation_type.get("waiting_for_choice"), Some(&1));
        assert_eq!(metrics.by_action_type.get("select_option"), Some(&1));
    }

    #[test]
    fn test_decision_metrics_record_failure() {
        let mut metrics = DecisionMetrics::new();
        metrics.record_failure("error", 200);

        assert_eq!(metrics.total_decisions, 1);
        assert_eq!(metrics.failed_decisions, 1);
        assert_eq!(metrics.by_situation_type.get("error"), Some(&1));
    }

    #[test]
    fn test_decision_metrics_human_intervention() {
        let mut metrics = DecisionMetrics::new();
        metrics.record_human_intervention();
        assert_eq!(metrics.human_interventions, 1);

        metrics.record_human_response(true);
        assert_eq!(metrics.human_accepted, 1);

        metrics.record_human_response(false);
        assert_eq!(metrics.human_rejected, 1);

        metrics.record_human_timeout();
        assert_eq!(metrics.human_timed_out, 1);
    }

    #[test]
    fn test_decision_metrics_timeout_fallback() {
        let mut metrics = DecisionMetrics::new();
        metrics.record_timeout_fallback();
        assert_eq!(metrics.timeout_fallbacks, 1);
    }

    #[test]
    fn test_decision_metrics_engine_switch() {
        let mut metrics = DecisionMetrics::new();
        metrics.record_engine_switch();
        assert_eq!(metrics.engine_switches, 1);
    }

    #[test]
    fn test_decision_metrics_retry() {
        let mut metrics = DecisionMetrics::new();
        metrics.record_retry();
        metrics.record_retry();
        assert_eq!(metrics.retries, 2);
    }

    #[test]
    fn test_decision_metrics_duration_stats() {
        let mut metrics = DecisionMetrics::new();
        metrics.record_success("s1", "a1", 100);
        metrics.record_success("s2", "a2", 200);
        metrics.record_success("s3", "a3", 150);

        assert_eq!(metrics.total_decisions, 3);
        assert_eq!(metrics.max_duration_ms, 200);
        assert_eq!(metrics.min_duration_ms, 100);
        // Average: (100 + 200 + 150) / 3 = 150
        assert_eq!(metrics.avg_duration_ms, 150);
    }

    #[test]
    fn test_decision_metrics_reset() {
        let mut metrics = DecisionMetrics::new();
        metrics.record_success("s1", "a1", 100);
        metrics.record_human_intervention();
        metrics.reset();

        assert_eq!(metrics.total_decisions, 0);
        assert_eq!(metrics.successful_decisions, 0);
        assert!(metrics.by_situation_type.is_empty());
        assert_eq!(metrics.human_interventions, 0);
    }

    #[test]
    fn test_decision_metrics_merge() {
        let mut metrics1 = DecisionMetrics::new();
        metrics1.record_success("s1", "a1", 100);

        let mut metrics2 = DecisionMetrics::new();
        metrics2.record_success("s2", "a2", 200);
        metrics2.record_human_intervention();

        metrics1.merge(&metrics2);

        assert_eq!(metrics1.total_decisions, 2);
        assert_eq!(metrics1.successful_decisions, 2);
        assert_eq!(metrics1.human_interventions, 1);
    }

    #[test]
    fn test_decision_metrics_human_intervention_rate() {
        let mut metrics = DecisionMetrics::new();
        metrics.total_decisions = 10;
        metrics.human_interventions = 2;
        assert_eq!(metrics.human_intervention_rate(), 0.2);
    }

    #[test]
    fn test_decision_log_entry_new() {
        let entry = DecisionLogEntry::new(
            "dec-1".to_string(),
            "agent-1".to_string(),
            "waiting_for_choice".to_string(),
            "select_option".to_string(),
        );

        assert_eq!(entry.decision_id, "dec-1");
        assert_eq!(entry.agent_id, "agent-1");
        assert_eq!(entry.situation_type, "waiting_for_choice");
        assert_eq!(entry.action_type, "select_option");
        assert!(entry.is_pending());
    }

    #[test]
    fn test_decision_log_entry_with_methods() {
        let entry = DecisionLogEntry::new(
            "dec-1".to_string(),
            "agent-1".to_string(),
            "waiting_for_choice".to_string(),
            "select_option".to_string(),
        )
        .with_duration(500)
        .with_engine_type("rule_based".to_string())
        .with_confidence(0.9)
        .with_human_requested(true)
        .with_retry_count(2)
        .with_reflection_rounds(1)
        .with_outcome(DecisionOutcome::Success);

        assert_eq!(entry.duration_ms, 500);
        assert_eq!(entry.engine_type, "rule_based");
        assert_eq!(entry.confidence, Some(0.9));
        assert!(entry.human_requested);
        assert_eq!(entry.retry_count, 2);
        assert_eq!(entry.reflection_rounds, 1);
        assert!(entry.is_success());
    }

    #[test]
    fn test_decision_log_entry_to_json() {
        let entry = DecisionLogEntry::new(
            "dec-1".to_string(),
            "agent-1".to_string(),
            "waiting_for_choice".to_string(),
            "select_option".to_string(),
        );

        let json = entry.to_json().unwrap();
        assert!(json.contains("dec-1"));
        assert!(json.contains("waiting_for_choice"));
    }

    #[test]
    fn test_decision_outcome_is_final() {
        assert!(!DecisionOutcome::Pending.is_final());
        assert!(DecisionOutcome::Success.is_final());
        assert!(DecisionOutcome::Failed.is_final());
        assert!(DecisionOutcome::Skipped.is_final());
        assert!(DecisionOutcome::HumanOverride.is_final());
    }

    #[test]
    fn test_decision_outcome_display() {
        assert_eq!(format!("{}", DecisionOutcome::Pending), "pending");
        assert_eq!(format!("{}", DecisionOutcome::Success), "success");
        assert_eq!(format!("{}", DecisionOutcome::Failed), "failed");
    }

    #[test]
    fn test_decision_log_new() {
        let log = DecisionLog::new(100);
        assert_eq!(log.count(), 0);
    }

    #[test]
    fn test_decision_log_add() {
        let mut log = DecisionLog::new(100);
        let entry = DecisionLogEntry::new(
            "dec-1".to_string(),
            "agent-1".to_string(),
            "s1".to_string(),
            "a1".to_string(),
        );
        log.add(entry);

        assert_eq!(log.count(), 1);
    }

    #[test]
    fn test_decision_log_trim() {
        let mut log = DecisionLog::new(3);

        for i in 0..5 {
            log.add(DecisionLogEntry::new(
                format!("dec-{}", i),
                "agent-1".to_string(),
                "s1".to_string(),
                "a1".to_string(),
            ));
        }

        assert_eq!(log.count(), 3);
        assert_eq!(log.entries()[0].decision_id, "dec-2");
        assert_eq!(log.entries()[2].decision_id, "dec-4");
    }

    #[test]
    fn test_decision_log_for_agent() {
        let mut log = DecisionLog::new(100);
        log.add(DecisionLogEntry::new(
            "dec-1".to_string(),
            "agent-1".to_string(),
            "s1".to_string(),
            "a1".to_string(),
        ));
        log.add(DecisionLogEntry::new(
            "dec-2".to_string(),
            "agent-2".to_string(),
            "s1".to_string(),
            "a1".to_string(),
        ));
        log.add(DecisionLogEntry::new(
            "dec-3".to_string(),
            "agent-1".to_string(),
            "s1".to_string(),
            "a1".to_string(),
        ));

        let agent1_entries = log.for_agent("agent-1");
        assert_eq!(agent1_entries.len(), 2);
    }

    #[test]
    fn test_decision_log_by_situation() {
        let mut log = DecisionLog::new(100);
        log.add(DecisionLogEntry::new(
            "dec-1".to_string(),
            "agent-1".to_string(),
            "waiting_for_choice".to_string(),
            "a1".to_string(),
        ));
        log.add(DecisionLogEntry::new(
            "dec-2".to_string(),
            "agent-1".to_string(),
            "error".to_string(),
            "a1".to_string(),
        ));

        let wfc_entries = log.by_situation("waiting_for_choice");
        assert_eq!(wfc_entries.len(), 1);
    }

    #[test]
    fn test_decision_log_by_outcome() {
        let mut log = DecisionLog::new(100);
        log.add(
            DecisionLogEntry::new(
                "dec-1".to_string(),
                "agent-1".to_string(),
                "s1".to_string(),
                "a1".to_string(),
            )
            .with_outcome(DecisionOutcome::Success),
        );
        log.add(
            DecisionLogEntry::new(
                "dec-2".to_string(),
                "agent-1".to_string(),
                "s1".to_string(),
                "a1".to_string(),
            )
            .with_outcome(DecisionOutcome::Failed),
        );

        let success_entries = log.by_outcome(DecisionOutcome::Success);
        assert_eq!(success_entries.len(), 1);
    }

    #[test]
    fn test_decision_log_recent() {
        let mut log = DecisionLog::new(100);
        for i in 0..10 {
            log.add(DecisionLogEntry::new(
                format!("dec-{}", i),
                "agent-1".to_string(),
                "s1".to_string(),
                "a1".to_string(),
            ));
        }

        let recent = log.recent(3);
        assert_eq!(recent.len(), 3);
        assert_eq!(recent[0].decision_id, "dec-7");
    }

    #[test]
    fn test_decision_log_clear() {
        let mut log = DecisionLog::new(100);
        log.add(DecisionLogEntry::new(
            "dec-1".to_string(),
            "agent-1".to_string(),
            "s1".to_string(),
            "a1".to_string(),
        ));
        log.clear();
        assert_eq!(log.count(), 0);
    }

    #[test]
    fn test_decision_log_to_json() {
        let mut log = DecisionLog::new(100);
        log.add(DecisionLogEntry::new(
            "dec-1".to_string(),
            "agent-1".to_string(),
            "s1".to_string(),
            "a1".to_string(),
        ));

        let json = log.to_json().unwrap();
        assert!(json.starts_with("["));
        assert!(json.contains("dec-1"));
    }

    #[test]
    fn test_metrics_summary_from_metrics() {
        let mut metrics = DecisionMetrics::new();
        metrics.record_success("waiting_for_choice", "select_option", 100);
        metrics.record_success("waiting_for_choice", "reflect", 200);
        metrics.record_success("error", "retry", 150);

        let summary = MetricsSummary::from_metrics(&metrics);

        assert_eq!(summary.total_decisions, 3);
        assert_eq!(summary.success_rate_pct, 100.0);
        assert_eq!(summary.avg_duration_ms, 150);
        assert_eq!(summary.top_situation_types.len(), 2);
    }

    #[test]
    fn test_metrics_summary_to_display() {
        let mut metrics = DecisionMetrics::new();
        metrics.record_success("s1", "a1", 100);

        let summary = MetricsSummary::from_metrics(&metrics);
        let display = summary.to_display();

        assert!(display.contains("Total Decisions: 1"));
        assert!(display.contains("Success Rate:"));
    }

    #[test]
    fn test_decision_metrics_serde() {
        let mut metrics = DecisionMetrics::new();
        metrics.record_success("s1", "a1", 100);

        let json = serde_json::to_string(&metrics).unwrap();
        let parsed: DecisionMetrics = serde_json::from_str(&json).unwrap();

        assert_eq!(metrics.total_decisions, parsed.total_decisions);
        assert_eq!(metrics.successful_decisions, parsed.successful_decisions);
    }

    #[test]
    fn test_decision_log_entry_serde() {
        let entry = DecisionLogEntry::new(
            "dec-1".to_string(),
            "agent-1".to_string(),
            "s1".to_string(),
            "a1".to_string(),
        )
        .with_duration(100);

        let json = serde_json::to_string(&entry).unwrap();
        let parsed: DecisionLogEntry = serde_json::from_str(&json).unwrap();

        assert_eq!(entry.decision_id, parsed.decision_id);
        assert_eq!(entry.duration_ms, parsed.duration_ms);
    }
}