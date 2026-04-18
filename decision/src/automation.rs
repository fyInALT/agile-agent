//! Automation layer for task concept (Sprint 11)
//!
//! Provides prompt templates, auto-check system, and decision filtering
//! to automate 80% of routine decisions.

use std::collections::HashMap;
use serde::{Deserialize, Serialize};

use crate::task::{Task, TaskStatus};

// ============================================================================
// Story 11.1: Prompt Template Structure
// ============================================================================

/// Error type for template rendering
#[derive(Debug, Clone, thiserror::Error)]
pub enum RenderError {
    #[error("Missing variable: {0}")]
    MissingVariable(String),
}

/// Prompt template with variable substitution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptTemplate {
    /// Template identifier
    pub id: String,
    /// Template content with {{variable}} placeholders
    pub content: String,
    /// List of required variables
    pub variables: Vec<String>,
}

impl PromptTemplate {
    /// Create a new prompt template
    pub fn new(id: String, content: String, variables: Vec<String>) -> Self {
        Self { id, content, variables }
    }

    /// Render template with variable values
    pub fn render(&self, values: &HashMap<String, String>) -> Result<String, RenderError> {
        let mut result = self.content.clone();
        for var in &self.variables {
            let value = values
                .get(var)
                .ok_or_else(|| RenderError::MissingVariable(var.clone()))?;
            result = result.replace(&format!("{{{{{}}}}}", var), value);
        }
        Ok(result)
    }

    /// Render template with default values for missing variables
    pub fn render_with_defaults(&self, values: &HashMap<String, String>) -> String {
        let mut result = self.content.clone();
        for var in &self.variables {
            let value = values.get(var).map(|s| s.as_str()).unwrap_or("");
            result = result.replace(&format!("{{{{{}}}}}", var), value);
        }
        result
    }
}

// ============================================================================
// Story 11.3: Auto-Check System
// ============================================================================

/// Result of automatic quality check
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AutoCheckResult {
    /// Output passes all checks
    Pass,
    /// Output has issues requiring reflection
    NeedsReflection { reason: String },
    /// Output has issues requiring human decision
    NeedsHuman { reason: String },
}

/// Simulated AI output for checking
#[derive(Debug, Clone, Default)]
pub struct SimulatedOutput {
    /// Has syntax errors
    pub has_syntax_errors: bool,
    /// Tests pass
    pub tests_pass: bool,
    /// Has compile errors
    pub has_compile_errors: bool,
    /// Has style issues
    pub has_style_issues: bool,
    /// Modified files (for boundary check)
    pub modified_files: Vec<String>,
    /// Contains risky operations
    pub risky_operations: Vec<String>,
    /// Has dependency conflict
    pub has_dependency_conflict: bool,
    /// Has multiple valid solutions
    pub has_multiple_solutions: bool,
}

impl SimulatedOutput {
    /// Create output with syntax errors
    pub fn with_syntax_errors() -> Self {
        Self { has_syntax_errors: true, ..Default::default() }
    }

    /// Create output with test failures
    pub fn with_test_failures() -> Self {
        Self { tests_pass: false, ..Default::default() }
    }

    /// Create output with compile errors
    pub fn with_compile_errors() -> Self {
        Self { has_compile_errors: true, ..Default::default() }
    }

    /// Create clean output (all pass)
    pub fn clean() -> Self {
        Self {
            has_syntax_errors: false,
            tests_pass: true,
            has_compile_errors: false,
            has_style_issues: false,
            modified_files: Vec::new(),
            risky_operations: Vec::new(),
            has_dependency_conflict: false,
            has_multiple_solutions: false,
        }
    }

    /// Create output with boundary violation
    pub fn with_boundary_violation(modified_files: Vec<String>) -> Self {
        Self { modified_files, ..Self::clean() }
    }

    /// Create output with risky operation
    pub fn with_risky_operation(op: String) -> Self {
        Self { risky_operations: vec![op], ..Self::clean() }
    }

    /// Check if contains specific file modification
    pub fn contains_file(&self, file: &str) -> bool {
        self.modified_files.iter().any(|f| f.contains(file))
    }

    /// Check if contains risky operation
    pub fn contains_operation(&self, op: &str) -> bool {
        self.risky_operations.iter().any(|o| o.contains(op))
    }
}

/// Check rule trait for extensible rules
pub trait CheckRule: Send + Sync {
    /// Rule name
    fn name(&self) -> &str;
    /// Check task and output, return result if issue found
    fn check(&self, task: &Task, output: &SimulatedOutput) -> Option<AutoCheckResult>;
}

/// Syntax error check rule
pub struct SyntaxCheckRule;

impl CheckRule for SyntaxCheckRule {
    fn name(&self) -> &str {
        "syntax_check"
    }

    fn check(&self, _task: &Task, output: &SimulatedOutput) -> Option<AutoCheckResult> {
        if output.has_syntax_errors {
            Some(AutoCheckResult::NeedsReflection {
                reason: "Syntax errors found".to_string(),
            })
        } else {
            None
        }
    }
}

/// Test pass check rule
pub struct TestCheckRule;

impl CheckRule for TestCheckRule {
    fn name(&self) -> &str {
        "test_check"
    }

    fn check(&self, _task: &Task, output: &SimulatedOutput) -> Option<AutoCheckResult> {
        if !output.tests_pass {
            Some(AutoCheckResult::NeedsReflection {
                reason: "Tests failed".to_string(),
            })
        } else {
            None
        }
    }
}

/// Compile error check rule
pub struct CompileCheckRule;

impl CheckRule for CompileCheckRule {
    fn name(&self) -> &str {
        "compile_check"
    }

    fn check(&self, _task: &Task, output: &SimulatedOutput) -> Option<AutoCheckResult> {
        if output.has_compile_errors {
            Some(AutoCheckResult::NeedsReflection {
                reason: "Compilation errors".to_string(),
            })
        } else {
            None
        }
    }
}

/// Boundary check rule
pub struct BoundaryCheckRule {
    /// Allowed files/patterns
    allowed_files: Vec<String>,
}

impl BoundaryCheckRule {
    /// Create with allowed file patterns
    pub fn new(allowed_files: Vec<String>) -> Self {
        Self { allowed_files }
    }

    /// Check if output violates boundaries
    #[allow(dead_code)]
    fn is_violated(&self, output: &SimulatedOutput, allowed: &[String]) -> bool {
        if allowed.is_empty() {
            return false; // No boundaries defined, nothing to violate
        }
        for file in &output.modified_files {
            // Check if file matches any allowed pattern (substring or exact match)
            let is_allowed = allowed.iter().any(|pattern| {
                // Exact match
                if file == pattern {
                    return true;
                }
                // Pattern is substring of file (e.g., "login.rs" matches "src/login.rs")
                if file.contains(pattern) {
                    return true;
                }
                // File is substring of pattern (e.g., "login" matches "login.rs")
                if pattern.contains(file) {
                    return true;
                }
                false
            });
            if !is_allowed {
                return true;
            }
        }
        false
    }
}

impl CheckRule for BoundaryCheckRule {
    fn name(&self) -> &str {
        "boundary_check"
    }

    fn check(&self, task: &Task, output: &SimulatedOutput) -> Option<AutoCheckResult> {
        // Use task constraints as allowed files
        let allowed = if self.allowed_files.is_empty() {
            task.constraints.clone()
        } else {
            self.allowed_files.clone()
        };

        for file in &output.modified_files {
            let is_allowed = allowed.iter().any(|pattern| file.contains(pattern) || pattern.contains(file));
            if !is_allowed && !allowed.is_empty() {
                return Some(AutoCheckResult::NeedsHuman {
                    reason: format!("Boundary violation: modified {}", file),
                });
            }
        }
        None
    }
}

/// Risk check rule
pub struct RiskCheckRule {
    /// High-risk operation patterns
    risky_patterns: Vec<String>,
}

impl RiskCheckRule {
    /// Create with risky patterns
    pub fn new(risky_patterns: Vec<String>) -> Self {
        Self { risky_patterns }
    }

    /// Default risky patterns
    pub fn default_patterns() -> Vec<String> {
        vec![
            "DROP TABLE".to_string(),
            "DELETE FROM".to_string(),
            "rm -rf".to_string(),
            "chmod 777".to_string(),
            "sudo".to_string(),
        ]
    }
}

impl Default for RiskCheckRule {
    fn default() -> Self {
        Self::new(Self::default_patterns())
    }
}

impl CheckRule for RiskCheckRule {
    fn name(&self) -> &str {
        "risk_check"
    }

    fn check(&self, _task: &Task, output: &SimulatedOutput) -> Option<AutoCheckResult> {
        for pattern in &self.risky_patterns {
            if output.contains_operation(pattern) {
                return Some(AutoCheckResult::NeedsHuman {
                    reason: format!("High-risk operation: {}", pattern),
                });
            }
        }
        // Also check output's risky_operations field
        for op in &output.risky_operations {
            return Some(AutoCheckResult::NeedsHuman {
                reason: format!("High-risk operation: {}", op),
            });
        }
        None
    }
}

/// Auto-checker combining all rules
pub struct AutoChecker {
    rules: Vec<Box<dyn CheckRule>>,
}

impl AutoChecker {
    /// Create with rules
    pub fn new(rules: Vec<Box<dyn CheckRule>>) -> Self {
        Self { rules }
    }

    /// Create with default rules
    pub fn with_defaults() -> Self {
        Self::new(vec![
            Box::new(SyntaxCheckRule),
            Box::new(TestCheckRule),
            Box::new(CompileCheckRule),
            Box::new(BoundaryCheckRule::new(vec![])),
            Box::new(RiskCheckRule::default()),
        ])
    }

    /// Add a rule
    pub fn add_rule(&mut self, rule: Box<dyn CheckRule>) {
        self.rules.push(rule);
    }

    /// Check output against all rules
    pub fn check(&self, task: &Task, output: &SimulatedOutput) -> AutoCheckResult {
        for rule in &self.rules {
            if let Some(result) = rule.check(task, output) {
                return result;
            }
        }
        AutoCheckResult::Pass
    }
}

impl Default for AutoChecker {
    fn default() -> Self {
        Self::with_defaults()
    }
}

// ============================================================================
// Story 11.4: Human Escalation Filter
// ============================================================================

use crate::workflow::WorkflowAction;

/// Boundary rule for filter
#[derive(Debug, Clone)]
pub struct BoundaryRule {
    pattern: String,
    description: String,
}

impl BoundaryRule {
    /// Create a boundary rule
    pub fn new(pattern: String, description: String) -> Self {
        Self { pattern, description }
    }

    /// Get description
    pub fn description(&self) -> &str {
        &self.description
    }

    /// Check if violated (output file not matching pattern)
    pub fn is_violated(&self, output: &SimulatedOutput, task_constraints: &[String]) -> bool {
        // If task has explicit constraints, use them
        if !task_constraints.is_empty() {
            for file in &output.modified_files {
                let is_allowed = task_constraints.iter().any(|c| {
                    file.contains(c) || c.contains(file) || file == c
                });
                if !is_allowed {
                    return true;
                }
            }
            return false;
        }
        // Otherwise use this rule's pattern
        for file in &output.modified_files {
            if !file.contains(&self.pattern) && &self.pattern != file {
                return true;
            }
        }
        false
    }
}

/// Decision filter for human escalation
#[derive(Debug, Clone)]
pub struct DecisionFilter {
    /// High-risk operations
    risky_operations: Vec<String>,
    /// Boundary rules
    boundary_rules: Vec<BoundaryRule>,
}

impl DecisionFilter {
    /// Create new filter
    pub fn new(risky_operations: Vec<String>, boundary_rules: Vec<BoundaryRule>) -> Self {
        Self { risky_operations, boundary_rules }
    }

    /// Check if human decision is needed
    pub fn needs_human_decision(&self, task: &Task, output: &SimulatedOutput) -> Option<String> {
        // Check boundary violation using task constraints
        for rule in &self.boundary_rules {
            if rule.is_violated(output, &task.constraints) {
                return Some(format!("Boundary violation: {}", rule.description()));
            }
        }

        // Also check if any modified file is outside task constraints
        if !task.constraints.is_empty() {
            for file in &output.modified_files {
                let is_allowed = task.constraints.iter().any(|c| {
                    file.contains(c) || c.contains(file) || file == c
                });
                if !is_allowed {
                    return Some(format!("Boundary violation: modified {}", file));
                }
            }
        }

        // Check risky operations
        for op in &self.risky_operations {
            if output.contains_operation(op) {
                return Some(format!("High-risk: {}", op));
            }
        }
        for op in &output.risky_operations {
            return Some(format!("High-risk: {}", op));
        }

        // Check design decision needed
        if output.has_multiple_solutions {
            return Some("Design decision required".to_string());
        }

        // Check dependency conflict
        if output.has_dependency_conflict {
            return Some("Dependency conflict".to_string());
        }

        // Check max reflections reached
        if task.reflection_count >= task.max_reflection_rounds {
            return Some("Reflection limit reached".to_string());
        }

        None
    }

    /// Auto-decide for routine cases
    pub fn auto_decide(&self, task: &Task, output: &SimulatedOutput) -> WorkflowAction {
        // Check if human needed first
        if self.needs_human_decision(task, output).is_some() {
            return WorkflowAction::RequestHuman {
                question: "Human decision required".to_string(),
            };
        }

        // Test failure → Reflect
        if !output.tests_pass {
            return WorkflowAction::Reflect {
                reason: "Tests failed".to_string(),
            };
        }

        // Syntax error → Reflect
        if output.has_syntax_errors {
            return WorkflowAction::Reflect {
                reason: "Syntax errors".to_string(),
            };
        }

        // Compile error → Reflect
        if output.has_compile_errors {
            return WorkflowAction::Reflect {
                reason: "Compile errors".to_string(),
            };
        }

        // Style issue → Reflect (if configured, but we skip for now)
        if output.has_style_issues {
            return WorkflowAction::Reflect {
                reason: "Style issues".to_string(),
            };
        }

        // Task pending confirmation → Confirm
        if task.status == TaskStatus::PendingConfirmation {
            return WorkflowAction::ConfirmCompletion;
        }

        // Default → Continue
        WorkflowAction::Continue
    }
}

impl Default for DecisionFilter {
    fn default() -> Self {
        Self::new(
            RiskCheckRule::default_patterns(),
            vec![BoundaryRule::new("allowed".to_string(), "Task boundary".to_string())],
        )
    }
}

// ============================================================================
// Story 11.2: Default Prompt Templates
// ============================================================================

/// Get default prompt templates
pub fn default_templates() -> HashMap<String, PromptTemplate> {
    let mut templates = HashMap::new();

    templates.insert(
        "start".to_string(),
        PromptTemplate::new(
            "start".to_string(),
            "Begin implementing task: {{task_description}}".to_string(),
            vec!["task_description".to_string()],
        ),
    );

    templates.insert(
        "check_quality".to_string(),
        PromptTemplate::new(
            "check_quality".to_string(),
            "Check AI output:\n1. Syntax errors?\n2. Tests pass?\n3. Within boundaries?\n4. Code quality?\n\nOutput: {{ai_output}}\nConstraints: {{task_constraints}}".to_string(),
            vec!["ai_output".to_string(), "task_constraints".to_string()],
        ),
    );

    templates.insert(
        "reflecting".to_string(),
        PromptTemplate::new(
            "reflecting".to_string(),
            "Issue found: {{problem_description}}\n\nAnalyze and fix.\nCurrent round: {{reflection_count}}/{{max_reflections}}".to_string(),
            vec!["problem_description".to_string(), "reflection_count".to_string(), "max_reflections".to_string()],
        ),
    );

    templates.insert(
        "check_completion".to_string(),
        PromptTemplate::new(
            "check_completion".to_string(),
            "Check task completion:\nGoals: {{task_goals}}\nStatus: {{current_status}}\n\nAll goals achieved?".to_string(),
            vec!["task_goals".to_string(), "current_status".to_string()],
        ),
    );

    templates.insert(
        "confirming".to_string(),
        PromptTemplate::new(
            "confirming".to_string(),
            "Task ready for confirmation:\nTask: {{task_description}}\nChanges: {{changes_summary}}\nTests: {{test_results}}\n\nConfirm completion?".to_string(),
            vec!["task_description".to_string(), "changes_summary".to_string(), "test_results".to_string()],
        ),
    );

    templates.insert(
        "human_decision".to_string(),
        PromptTemplate::new(
            "human_decision".to_string(),
            "Human decision required:\nQuestion: {{question}}\nContext: {{context}}".to_string(),
            vec!["question".to_string(), "context".to_string()],
        ),
    );

    templates
}

#[cfg(test)]
mod tests {
    use super::*;

    // Story 11.1 Tests: Prompt Template Structure

    #[test]
    fn t11_1_t1_template_renders_with_all_variables() {
        let template = PromptTemplate::new(
            "test".to_string(),
            "Task: {{task_name}}, Status: {{status}}".to_string(),
            vec!["task_name".to_string(), "status".to_string()],
        );

        let values = HashMap::from([
            ("task_name".to_string(), "Fix bug".to_string()),
            ("status".to_string(), "InProgress".to_string()),
        ]);

        let result = template.render(&values).expect("Should render");
        assert_eq!(result, "Task: Fix bug, Status: InProgress");
    }

    #[test]
    fn t11_1_t2_missing_variable_returns_error() {
        let template = PromptTemplate::new(
            "test".to_string(),
            "Task: {{task_name}}".to_string(),
            vec!["task_name".to_string()],
        );

        let values = HashMap::new();

        let result = template.render(&values);
        assert!(result.is_err(), "Should return error for missing variable");
        assert!(matches!(result.unwrap_err(), RenderError::MissingVariable(_)));
    }

    #[test]
    fn t11_1_t3_multiple_variables_replaced_correctly() {
        let template = PromptTemplate::new(
            "test".to_string(),
            "{{a}} {{b}} {{c}}".to_string(),
            vec!["a".to_string(), "b".to_string(), "c".to_string()],
        );

        let values = HashMap::from([
            ("a".to_string(), "1".to_string()),
            ("b".to_string(), "2".to_string()),
            ("c".to_string(), "3".to_string()),
        ]);

        let result = template.render(&values).expect("Should render");
        assert_eq!(result, "1 2 3");
    }

    #[test]
    fn t11_1_t4_empty_template_returns_empty_string() {
        let template = PromptTemplate::new(
            "empty".to_string(),
            "".to_string(),
            vec![],
        );

        let result = template.render(&HashMap::new()).expect("Should render");
        assert_eq!(result, "");
    }

    #[test]
    fn t11_1_t5_render_with_defaults_handles_missing() {
        let template = PromptTemplate::new(
            "test".to_string(),
            "Task: {{task_name}}".to_string(),
            vec!["task_name".to_string()],
        );

        let result = template.render_with_defaults(&HashMap::new());
        assert_eq!(result, "Task: ");
    }

    // Story 11.2 Tests: Default Prompt Templates

    #[test]
    fn t11_2_t1_start_template_has_task_description() {
        let templates = default_templates();
        let start = templates.get("start").expect("Should have start template");

        assert_eq!(start.id, "start");
        assert!(start.content.contains("{{task_description}}"));
        assert!(start.variables.contains(&"task_description".to_string()));
    }

    #[test]
    fn t11_2_t2_check_quality_template_has_ai_output_constraints() {
        let templates = default_templates();
        let quality = templates.get("check_quality").expect("Should have check_quality template");

        assert!(quality.variables.contains(&"ai_output".to_string()));
        assert!(quality.variables.contains(&"task_constraints".to_string()));
    }

    #[test]
    fn t11_2_t3_reflecting_template_has_problem_count_max() {
        let templates = default_templates();
        let reflecting = templates.get("reflecting").expect("Should have reflecting template");

        assert!(reflecting.variables.contains(&"problem_description".to_string()));
        assert!(reflecting.variables.contains(&"reflection_count".to_string()));
        assert!(reflecting.variables.contains(&"max_reflections".to_string()));
    }

    #[test]
    fn t11_2_t4_check_completion_template_has_goals_status() {
        let templates = default_templates();
        let completion = templates.get("check_completion").expect("Should have check_completion template");

        assert!(completion.variables.contains(&"task_goals".to_string()));
        assert!(completion.variables.contains(&"current_status".to_string()));
    }

    #[test]
    fn t11_2_t5_confirming_template_has_task_changes_tests() {
        let templates = default_templates();
        let confirming = templates.get("confirming").expect("Should have confirming template");

        assert!(confirming.variables.contains(&"task_description".to_string()));
        assert!(confirming.variables.contains(&"changes_summary".to_string()));
        assert!(confirming.variables.contains(&"test_results".to_string()));
    }

    #[test]
    fn t11_2_t6_all_templates_in_registry() {
        let templates = default_templates();

        assert!(templates.contains_key("start"));
        assert!(templates.contains_key("check_quality"));
        assert!(templates.contains_key("reflecting"));
        assert!(templates.contains_key("check_completion"));
        assert!(templates.contains_key("confirming"));
        assert!(templates.contains_key("human_decision"));
        assert_eq!(templates.len(), 6);
    }

    // Story 11.3 Tests: Auto-Check System

    #[test]
    fn t11_3_t1_syntax_check_detects_errors() {
        let rule = SyntaxCheckRule;
        let task = Task::new("Test".to_string(), vec![]);
        let output = SimulatedOutput::with_syntax_errors();

        let result = rule.check(&task, &output);
        assert!(result.is_some());
        let check_result = result.unwrap();
        assert_eq!(check_result, AutoCheckResult::NeedsReflection {
            reason: "Syntax errors found".to_string(),
        });
    }

    #[test]
    fn t11_3_t2_test_check_detects_failures() {
        let rule = TestCheckRule;
        let task = Task::new("Test".to_string(), vec![]);
        let output = SimulatedOutput::with_test_failures();

        let result = rule.check(&task, &output);
        assert!(result.is_some());
        assert_eq!(result.unwrap(), AutoCheckResult::NeedsReflection {
            reason: "Tests failed".to_string(),
        });
    }

    #[test]
    fn t11_3_t3_compilation_check_detects_errors() {
        let rule = CompileCheckRule;
        let task = Task::new("Test".to_string(), vec![]);
        let output = SimulatedOutput::with_compile_errors();

        let result = rule.check(&task, &output);
        assert!(result.is_some());
        assert_eq!(result.unwrap(), AutoCheckResult::NeedsReflection {
            reason: "Compilation errors".to_string(),
        });
    }

    #[test]
    fn t11_3_t4_boundary_check_detects_violations() {
        let rule = BoundaryCheckRule::new(vec!["src/auth.rs".to_string()]);
        let task = Task::new("Fix auth".to_string(), vec!["src/auth.rs".to_string()]);
        let output = SimulatedOutput::with_boundary_violation(vec!["src/db.rs".to_string(), "src/auth.rs".to_string()]);

        let result = rule.check(&task, &output);
        assert!(result.is_some());
        let check_result = result.unwrap();
        assert!(matches!(check_result, AutoCheckResult::NeedsHuman { .. }));
    }

    #[test]
    fn t11_3_t5_risk_check_detects_high_risk_ops() {
        let rule = RiskCheckRule::default();
        let task = Task::new("Test".to_string(), vec![]);
        let output = SimulatedOutput::with_risky_operation("DROP TABLE users".to_string());

        let result = rule.check(&task, &output);
        assert!(result.is_some());
        assert!(matches!(result.unwrap(), AutoCheckResult::NeedsHuman { .. }));
    }

    #[test]
    fn t11_3_t6_auto_checker_combines_all_rules() {
        let checker = AutoChecker::with_defaults();
        let task = Task::new("Test".to_string(), vec![]);
        let output = SimulatedOutput::with_syntax_errors();

        let result = checker.check(&task, &output);
        assert_eq!(result, AutoCheckResult::NeedsReflection {
            reason: "Syntax errors found".to_string(),
        });
    }

    #[test]
    fn t11_3_t7_pass_when_all_rules_pass() {
        let checker = AutoChecker::with_defaults();
        let task = Task::new("Test".to_string(), vec![]);
        let output = SimulatedOutput::clean();

        let result = checker.check(&task, &output);
        assert_eq!(result, AutoCheckResult::Pass);
    }

    #[test]
    fn t11_3_t8_needs_reflection_for_syntax() {
        let checker = AutoChecker::with_defaults();
        let task = Task::new("Test".to_string(), vec![]);
        let output = SimulatedOutput::with_syntax_errors();

        let result = checker.check(&task, &output);
        assert!(matches!(result, AutoCheckResult::NeedsReflection { .. }));
    }

    #[test]
    fn t11_3_t9_needs_human_for_boundary() {
        let checker = AutoChecker::with_defaults();
        let task = Task::new("Fix login".to_string(), vec!["login.rs".to_string()]);
        let output = SimulatedOutput {
            modified_files: vec!["admin.rs".to_string()],
            ..SimulatedOutput::clean()
        };

        let result = checker.check(&task, &output);
        assert!(matches!(result, AutoCheckResult::NeedsHuman { .. }));
    }

    // Story 11.4 Tests: Human Escalation Filter

    #[test]
    fn t11_4_t1_boundary_violation_needs_human() {
        let filter = DecisionFilter::default();
        // Task constraint: only modify login.rs
        let task = Task::new("Fix login".to_string(), vec!["login.rs".to_string()]);
        // Output: modified admin.rs (outside boundary)
        let output = SimulatedOutput {
            modified_files: vec!["admin.rs".to_string()],
            ..SimulatedOutput::clean()
        };

        let result = filter.needs_human_decision(&task, &output);
        assert!(result.is_some(), "Should detect boundary violation");
        assert!(result.unwrap().contains("Boundary violation"));
    }

    #[test]
    fn t11_4_t2_high_risk_operation_needs_human() {
        let filter = DecisionFilter::default();
        let task = Task::new("Test".to_string(), vec![]);
        let output = SimulatedOutput::with_risky_operation("DROP TABLE".to_string());

        let result = filter.needs_human_decision(&task, &output);
        assert!(result.is_some());
    }

    #[test]
    fn t11_4_t3_design_decision_needs_human() {
        let filter = DecisionFilter::default();
        let task = Task::new("Test".to_string(), vec![]);
        let output = SimulatedOutput {
            has_multiple_solutions: true,
            ..SimulatedOutput::clean()
        };

        let result = filter.needs_human_decision(&task, &output);
        assert!(result.is_some());
        assert!(result.unwrap().contains("Design decision"));
    }

    #[test]
    fn t11_4_t4_dependency_conflict_needs_human() {
        let filter = DecisionFilter::default();
        let task = Task::new("Test".to_string(), vec![]);
        let output = SimulatedOutput {
            has_dependency_conflict: true,
            ..SimulatedOutput::clean()
        };

        let result = filter.needs_human_decision(&task, &output);
        assert!(result.is_some());
        assert!(result.unwrap().contains("Dependency conflict"));
    }

    #[test]
    fn t11_4_t5_syntax_error_auto_reflect() {
        let filter = DecisionFilter::default();
        let task = Task::new("Test".to_string(), vec![]);
        let output = SimulatedOutput::with_syntax_errors();

        let action = filter.auto_decide(&task, &output);
        assert!(matches!(action, WorkflowAction::Reflect { .. }));
    }

    #[test]
    fn t11_4_t6_test_failure_auto_reflect() {
        let filter = DecisionFilter::default();
        let task = Task::new("Test".to_string(), vec![]);
        let output = SimulatedOutput::with_test_failures();

        let action = filter.auto_decide(&task, &output);
        assert!(matches!(action, WorkflowAction::Reflect { .. }));
    }

    #[test]
    fn t11_4_t7_normal_output_auto_continue() {
        let filter = DecisionFilter::default();
        let task = Task::new("Test".to_string(), vec![]);
        let output = SimulatedOutput::clean();

        let action = filter.auto_decide(&task, &output);
        assert_eq!(action, WorkflowAction::Continue);
    }

    #[test]
    fn t11_4_t8_reflection_limit_needs_human() {
        let filter = DecisionFilter::default();
        let mut task = Task::new("Test".to_string(), vec![]);
        task.reflection_count = 2;
        task.max_reflection_rounds = 2;
        let output = SimulatedOutput::with_syntax_errors();

        let result = filter.needs_human_decision(&task, &output);
        assert!(result.is_some());
        assert!(result.unwrap().contains("Reflection limit"));
    }

    // Story 11.5 Tests: Integration Tests for Automation

    #[test]
    fn t11_5_t1_clean_output_pass_continue() {
        let checker = AutoChecker::with_defaults();
        let filter = DecisionFilter::default();
        let task = Task::new("Fix bug".to_string(), vec!["bug.rs".to_string()]);
        let output = SimulatedOutput::clean();

        // Check passes
        let check_result = checker.check(&task, &output);
        assert_eq!(check_result, AutoCheckResult::Pass);

        // Auto-decide continues
        let action = filter.auto_decide(&task, &output);
        assert_eq!(action, WorkflowAction::Continue);
    }

    #[test]
    fn t11_5_t2_syntax_error_reflect() {
        let checker = AutoChecker::with_defaults();
        let filter = DecisionFilter::default();
        let task = Task::new("Fix bug".to_string(), vec![]);
        let output = SimulatedOutput::with_syntax_errors();

        // Check finds reflection need
        let check_result = checker.check(&task, &output);
        assert!(matches!(check_result, AutoCheckResult::NeedsReflection { .. }));

        // Auto-decide reflects
        let action = filter.auto_decide(&task, &output);
        assert!(matches!(action, WorkflowAction::Reflect { .. }));
    }

    #[test]
    fn t11_5_t3_boundary_violation_request_human() {
        let checker = AutoChecker::with_defaults();
        let filter = DecisionFilter::default();
        let task = Task::new("Fix login".to_string(), vec!["login.rs".to_string()]);
        let output = SimulatedOutput {
            modified_files: vec!["admin.rs".to_string()],
            ..SimulatedOutput::clean()
        };

        // Check finds human need
        let check_result = checker.check(&task, &output);
        assert!(matches!(check_result, AutoCheckResult::NeedsHuman { .. }));

        // Auto-decide requests human
        let action = filter.auto_decide(&task, &output);
        assert!(matches!(action, WorkflowAction::RequestHuman { .. }));
    }

    #[test]
    fn t11_5_t4_template_matches_decision_context() {
        let templates = default_templates();
        let reflecting = templates.get("reflecting").expect("template");

        let values = HashMap::from([
            ("problem_description".to_string(), "Tests failed".to_string()),
            ("reflection_count".to_string(), "1".to_string()),
            ("max_reflections".to_string(), "2".to_string()),
        ]);

        let prompt = reflecting.render(&values).expect("Should render");
        assert!(prompt.contains("Tests failed"));
        assert!(prompt.contains("1/2"));
    }
}