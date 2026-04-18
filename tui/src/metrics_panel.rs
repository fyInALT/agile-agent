//! Metrics panel widget for TUI dashboard (Sprint 14, Story 14.4)
//!
//! Displays automation statistics from TaskMetrics.
//!
//! NOTE: This widget is designed for future integration with the app loop.
//! Currently not connected to the runtime - suppress dead_code warnings.

#![allow(dead_code)]

use agent_decision::TaskMetrics;

/// Metrics panel state
#[derive(Debug, Clone)]
pub struct MetricsPanel {
    /// Current metrics
    metrics: TaskMetrics,
}

impl Default for MetricsPanel {
    fn default() -> Self {
        Self::new()
    }
}

impl MetricsPanel {
    /// Create a new empty metrics panel
    pub fn new() -> Self {
        Self {
            metrics: TaskMetrics::new(),
        }
    }

    /// Create panel with metrics
    pub fn with_metrics(metrics: TaskMetrics) -> Self {
        Self { metrics }
    }

    /// Update metrics
    pub fn update_metrics(&mut self, metrics: TaskMetrics) {
        self.metrics = metrics;
    }

    /// Get automation rate percentage
    pub fn automation_rate_percent(&self) -> f64 {
        self.metrics.automation_rate() * 100.0
    }

    /// Get completion rate percentage
    pub fn completion_rate_percent(&self) -> f64 {
        self.metrics.completion_rate() * 100.0
    }

    /// Get total tasks (completed + cancelled)
    pub fn total_tasks(&self) -> usize {
        self.metrics.completed_tasks + self.metrics.cancelled_tasks
    }

    /// Get total decisions
    pub fn total_decisions(&self) -> usize {
        self.metrics.auto_decisions + self.metrics.human_decisions
    }

    /// Check if automation target met (>80%)
    pub fn automation_target_met(&self) -> bool {
        self.metrics.automation_rate() >= 0.8
    }

    /// Format automation rate for display
    pub fn format_automation_rate(&self) -> String {
        let rate = self.automation_rate_percent();
        let target_marker = if self.automation_target_met() {
            "✓"
        } else {
            "○"
        };
        format!(
            "{} Automation Rate: {:.1}% (>80% target)",
            target_marker, rate
        )
    }

    /// Format completion rate for display
    pub fn format_completion_rate(&self) -> String {
        let rate = self.completion_rate_percent();
        format!("Completion Rate: {:.1}%", rate)
    }

    /// Format average reflections
    pub fn format_avg_reflections(&self) -> String {
        format!("Avg Reflections: {:.1}", self.metrics.avg_reflections())
    }

    /// Format average confirmations
    pub fn format_avg_confirmations(&self) -> String {
        format!("Avg Confirmations: {:.1}", self.metrics.avg_confirmations())
    }

    /// Format total tasks
    pub fn format_total_tasks(&self) -> String {
        format!("Total Tasks: {}", self.total_tasks())
    }

    /// Render lines for display
    pub fn render_lines(&self) -> Vec<String> {
        vec![
            "=== Task Metrics ===".to_string(),
            self.format_automation_rate(),
            self.format_completion_rate(),
            self.format_avg_reflections(),
            self.format_avg_confirmations(),
            self.format_total_tasks(),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Story 14.4 Tests: Metrics Panel Widget

    #[test]
    fn t14_4_t1_metrics_panel_renders() {
        let panel = MetricsPanel::new();
        let lines = panel.render_lines();

        assert_eq!(lines.len(), 6);
        assert!(lines[0].contains("Task Metrics"));
    }

    #[test]
    fn t14_4_t2_automation_rate_shown() {
        let mut metrics = TaskMetrics::new();
        metrics.auto_decisions = 8;
        metrics.human_decisions = 2;
        metrics.completed_tasks = 5;
        metrics.cancelled_tasks = 5;

        let panel = MetricsPanel::with_metrics(metrics);

        let rate = panel.automation_rate_percent();
        assert_eq!(rate, 80.0);

        let formatted = panel.format_automation_rate();
        assert!(formatted.contains("80.0%"));
        assert!(formatted.contains("✓")); // Target met
    }

    #[test]
    fn t14_4_t3_average_reflections_shown() {
        let mut metrics = TaskMetrics::new();
        metrics.completed_tasks = 5;
        metrics.total_reflections = 10;

        let avg = metrics.avg_reflections();
        let panel = MetricsPanel::with_metrics(metrics);

        assert_eq!(avg, 2.0);

        let formatted = panel.format_avg_reflections();
        assert!(formatted.contains("2.0"));
    }

    #[test]
    fn test_automation_target_met() {
        let mut metrics = TaskMetrics::new();
        metrics.auto_decisions = 8;
        metrics.human_decisions = 2;

        let panel = MetricsPanel::with_metrics(metrics);
        assert!(panel.automation_target_met());

        // Below target
        let mut metrics2 = TaskMetrics::new();
        metrics2.auto_decisions = 7;
        metrics2.human_decisions = 3;
        let panel2 = MetricsPanel::with_metrics(metrics2);
        assert!(!panel2.automation_target_met());
    }

    #[test]
    fn test_empty_metrics() {
        let panel = MetricsPanel::new();

        assert_eq!(panel.automation_rate_percent(), 0.0);
        assert_eq!(panel.completion_rate_percent(), 0.0);
        assert_eq!(panel.metrics.avg_reflections(), 0.0);
        assert_eq!(panel.metrics.avg_confirmations(), 0.0);
    }

    #[test]
    fn test_completion_rate() {
        let mut metrics = TaskMetrics::new();
        metrics.completed_tasks = 7;
        metrics.cancelled_tasks = 3;

        let panel = MetricsPanel::with_metrics(metrics);

        assert_eq!(panel.completion_rate_percent(), 70.0);
    }
}
