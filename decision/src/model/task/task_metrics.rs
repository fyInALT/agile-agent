//! Task metrics for automation analysis (Sprint 15)
//!
//! Tracks task-specific automation effectiveness and completion statistics.

use std::time::Duration;

use serde::{Deserialize, Serialize};

/// Task metrics for tracking automation effectiveness
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TaskMetrics {
    /// Number of automated decisions (no human intervention)
    pub auto_decisions: usize,
    /// Number of decisions requiring human intervention
    pub human_decisions: usize,
    /// Total reflection rounds across all tasks
    pub total_reflections: usize,
    /// Total confirmation attempts across all tasks
    pub total_confirmations: usize,
    /// Number of successfully completed tasks
    pub completed_tasks: usize,
    /// Number of cancelled tasks
    pub cancelled_tasks: usize,
    /// Total execution duration in seconds
    pub total_duration_seconds: u64,
}

impl TaskMetrics {
    /// Create empty metrics
    pub fn new() -> Self {
        Self::default()
    }

    /// Calculate automation rate (auto / total decisions)
    ///
    /// Returns 0.0 if no decisions recorded.
    /// Target: > 80%
    pub fn automation_rate(&self) -> f64 {
        let total = self.auto_decisions + self.human_decisions;
        if total == 0 {
            return 0.0;
        }
        self.auto_decisions as f64 / total as f64
    }

    /// Calculate completion rate (completed / total outcomes)
    ///
    /// Returns 0.0 if no outcomes recorded.
    /// Target: > 90%
    pub fn completion_rate(&self) -> f64 {
        let total = self.completed_tasks + self.cancelled_tasks;
        if total == 0 {
            return 0.0;
        }
        self.completed_tasks as f64 / total as f64
    }

    /// Calculate average reflections per completed task
    ///
    /// Returns 0.0 if no tasks completed.
    /// Target: 1-2 reflections
    pub fn avg_reflections(&self) -> f64 {
        if self.completed_tasks == 0 {
            return 0.0;
        }
        self.total_reflections as f64 / self.completed_tasks as f64
    }

    /// Calculate average confirmation attempts per completed task
    pub fn avg_confirmations(&self) -> f64 {
        if self.completed_tasks == 0 {
            return 0.0;
        }
        self.total_confirmations as f64 / self.completed_tasks as f64
    }

    /// Calculate average duration per completed task
    pub fn avg_duration(&self) -> Duration {
        if self.completed_tasks == 0 {
            return Duration::ZERO;
        }
        Duration::from_secs(self.total_duration_seconds / self.completed_tasks as u64)
    }

    /// Calculate human intervention rate
    ///
    /// Returns 0.0 if no decisions recorded.
    /// Target: < 20%
    pub fn human_intervention_rate(&self) -> f64 {
        1.0 - self.automation_rate()
    }

    /// Get total decisions count
    pub fn total_decisions(&self) -> usize {
        self.auto_decisions + self.human_decisions
    }

    /// Get total tasks processed
    pub fn total_tasks(&self) -> usize {
        self.completed_tasks + self.cancelled_tasks
    }

    /// Record an automated decision
    pub fn record_auto_decision(&mut self) {
        self.auto_decisions += 1;
    }

    /// Record a human decision
    pub fn record_human_decision(&mut self) {
        self.human_decisions += 1;
    }

    /// Record a reflection round
    pub fn record_reflection(&mut self) {
        self.total_reflections += 1;
    }

    /// Record a confirmation attempt
    pub fn record_confirmation(&mut self) {
        self.total_confirmations += 1;
    }

    /// Record task completion with statistics
    pub fn record_task_completion(
        &mut self,
        reflections: usize,
        confirmations: usize,
        duration: Duration,
    ) {
        self.completed_tasks += 1;
        self.total_reflections += reflections;
        self.total_confirmations += confirmations;
        self.total_duration_seconds += duration.as_secs();
    }

    /// Record task cancellation
    pub fn record_task_cancellation(&mut self) {
        self.cancelled_tasks += 1;
    }

    /// Merge metrics from another instance
    pub fn merge(&mut self, other: &TaskMetrics) {
        self.auto_decisions += other.auto_decisions;
        self.human_decisions += other.human_decisions;
        self.total_reflections += other.total_reflections;
        self.total_confirmations += other.total_confirmations;
        self.completed_tasks += other.completed_tasks;
        self.cancelled_tasks += other.cancelled_tasks;
        self.total_duration_seconds += other.total_duration_seconds;
    }

    /// Reset all metrics
    pub fn reset(&mut self) {
        *self = Self::default();
    }

    /// Check if automation rate meets target (> 80%)
    pub fn meets_automation_target(&self) -> bool {
        self.automation_rate() >= 0.80
    }

    /// Check if human intervention rate meets target (< 20%)
    pub fn meets_human_intervention_target(&self) -> bool {
        self.human_intervention_rate() <= 0.20
    }

    /// Check if completion rate meets target (> 90%)
    pub fn meets_completion_target(&self) -> bool {
        self.completion_rate() >= 0.90
    }

    /// Format metrics for display
    pub fn format_summary(&self) -> String {
        format!(
            "Automation: {:.1}% ({}/{}) | Completion: {:.1}% ({}/{}) | Avg Reflections: {:.1}",
            self.automation_rate() * 100.0,
            self.auto_decisions,
            self.total_decisions(),
            self.completion_rate() * 100.0,
            self.completed_tasks,
            self.total_tasks(),
            self.avg_reflections()
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Story 15.1 Tests: Metrics Collection

    #[test]
    fn t15_1_t1_metrics_counts_auto_decisions() {
        let mut metrics = TaskMetrics::new();

        metrics.record_auto_decision();
        metrics.record_auto_decision();
        metrics.record_auto_decision();

        assert_eq!(metrics.auto_decisions, 3);
    }

    #[test]
    fn t15_1_t2_metrics_counts_human_decisions() {
        let mut metrics = TaskMetrics::new();

        metrics.record_human_decision();
        metrics.record_human_decision();

        assert_eq!(metrics.human_decisions, 2);
    }

    #[test]
    fn t15_1_t3_automation_rate_calculated_correctly() {
        let mut metrics = TaskMetrics::new();

        // 8 auto, 2 human = 80% automation
        for _ in 0..8 {
            metrics.record_auto_decision();
        }
        for _ in 0..2 {
            metrics.record_human_decision();
        }

        assert_eq!(metrics.automation_rate(), 0.8);
    }

    #[test]
    fn t15_1_t4_completion_rate_calculated_correctly() {
        let mut metrics = TaskMetrics::new();

        // 9 completed, 1 cancelled = 90% completion
        for _ in 0..9 {
            metrics.record_task_completion(1, 1, Duration::from_secs(60));
        }
        metrics.record_task_cancellation();

        assert_eq!(metrics.completion_rate(), 0.9);
    }

    #[test]
    fn t15_1_t5_metrics_persist_across_sessions() {
        // Test serialization/deserialization
        let mut metrics = TaskMetrics::new();
        metrics.record_auto_decision();
        metrics.record_task_completion(2, 1, Duration::from_secs(120));

        let json = serde_json::to_string(&metrics).expect("serialize");
        let loaded: TaskMetrics = serde_json::from_str(&json).expect("deserialize");

        assert_eq!(loaded.auto_decisions, 1);
        assert_eq!(loaded.completed_tasks, 1);
        assert_eq!(loaded.total_reflections, 2);
    }

    #[test]
    fn test_automation_rate_empty() {
        let metrics = TaskMetrics::new();
        assert_eq!(metrics.automation_rate(), 0.0);
    }

    #[test]
    fn test_completion_rate_empty() {
        let metrics = TaskMetrics::new();
        assert_eq!(metrics.completion_rate(), 0.0);
    }

    #[test]
    fn test_avg_reflections() {
        let mut metrics = TaskMetrics::new();

        // 3 tasks with 1, 2, 3 reflections = avg 2
        metrics.record_task_completion(1, 1, Duration::from_secs(60));
        metrics.record_task_completion(2, 1, Duration::from_secs(60));
        metrics.record_task_completion(3, 1, Duration::from_secs(60));

        assert_eq!(metrics.avg_reflections(), 2.0);
    }

    #[test]
    fn test_avg_duration() {
        let mut metrics = TaskMetrics::new();

        // 2 tasks with 60s and 120s = avg 90s
        metrics.record_task_completion(1, 1, Duration::from_secs(60));
        metrics.record_task_completion(1, 1, Duration::from_secs(120));

        assert_eq!(metrics.avg_duration(), Duration::from_secs(90));
    }

    #[test]
    fn test_meets_targets() {
        let mut metrics = TaskMetrics::new();

        // 9 auto, 1 human = 90% automation (meets target)
        for _ in 0..9 {
            metrics.record_auto_decision();
        }
        metrics.record_human_decision();

        // 10 completed, 0 cancelled = 100% completion (meets target)
        for _ in 0..10 {
            metrics.record_task_completion(1, 1, Duration::from_secs(60));
        }

        assert!(metrics.meets_automation_target());
        assert!(metrics.meets_completion_target());
        assert!(metrics.meets_human_intervention_target());
    }

    #[test]
    fn test_merge_metrics() {
        let mut m1 = TaskMetrics::new();
        m1.record_auto_decision();
        m1.record_task_completion(2, 1, Duration::from_secs(100));

        let mut m2 = TaskMetrics::new();
        m2.record_auto_decision();
        m2.record_human_decision();
        m2.record_task_completion(1, 1, Duration::from_secs(50));

        m1.merge(&m2);

        assert_eq!(m1.auto_decisions, 2);
        assert_eq!(m1.human_decisions, 1);
        assert_eq!(m1.completed_tasks, 2);
        assert_eq!(m1.total_duration_seconds, 150);
    }

    #[test]
    fn test_format_summary() {
        let mut metrics = TaskMetrics::new();
        metrics.record_auto_decision();
        metrics.record_task_completion(1, 1, Duration::from_secs(60));

        let summary = metrics.format_summary();
        assert!(summary.contains("Automation"));
        assert!(summary.contains("100.0%"));
    }

    #[test]
    fn test_reset_metrics() {
        let mut metrics = TaskMetrics::new();
        metrics.record_auto_decision();
        metrics.record_task_completion(1, 1, Duration::from_secs(60));

        metrics.reset();

        assert_eq!(metrics.auto_decisions, 0);
        assert_eq!(metrics.completed_tasks, 0);
    }
}
