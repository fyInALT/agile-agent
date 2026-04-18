//! Task Decision Engine (Sprint 13)
//!
//! Integrates Task, Workflow, Automation, and Persistence into a cohesive engine.

use std::collections::HashMap;

use crate::automation::{AutoChecker, AutoCheckResult, DecisionFilter, SimulatedOutput};
use crate::persistence::{ExecutionRecord, TaskRegistry, TaskUpdate};
use crate::task::{Task, TaskId, TaskStatus};
use crate::workflow::{Condition, DecisionProcess, DecisionStage, StageId, WorkflowAction};

/// Decision action returned by engine
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DecisionAction {
    /// Continue execution without intervention
    Continue,
    /// Reflect on output with reason
    Reflect { reason: String },
    /// Confirm task completion
    ConfirmCompletion,
    /// Request human decision
    RequestHuman { question: String },
    /// Advance to specific stage
    AdvanceTo { stage: StageId },
    /// Return to previous stage
    ReturnTo { stage: StageId },
    /// Cancel task
    Cancel { reason: String },
    /// Retry last action
    Retry,
    /// Wait for more output
    Wait,
}

impl std::fmt::Display for DecisionAction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Continue => write!(f, "Continue"),
            Self::Reflect { reason } => write!(f, "Reflect: {}", reason),
            Self::ConfirmCompletion => write!(f, "ConfirmCompletion"),
            Self::RequestHuman { question } => write!(f, "RequestHuman: {}", question),
            Self::AdvanceTo { stage } => write!(f, "AdvanceTo: {}", stage),
            Self::ReturnTo { stage } => write!(f, "ReturnTo: {}", stage),
            Self::Cancel { reason } => write!(f, "Cancel: {}", reason),
            Self::Retry => write!(f, "Retry"),
            Self::Wait => write!(f, "Wait"),
        }
    }
}

/// Human response types
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HumanResponse {
    /// Approve and continue
    Approve,
    /// Deny with reason
    Deny { reason: String },
    /// Provide custom feedback
    Custom { feedback: String },
    /// Cancel task
    Cancel,
}

/// Simulated agent output for testing
#[derive(Debug, Clone)]
pub struct AgentOutput {
    /// Output content
    pub content: String,
    /// Whether output has syntax errors
    pub has_syntax_errors: bool,
    /// Whether tests pass
    pub tests_pass: bool,
    /// Whether there are compile errors
    pub has_compile_errors: bool,
    /// Whether there are style issues
    pub has_style_issues: bool,
    /// Modified files
    pub modified_files: Vec<String>,
    /// Risky operations detected
    pub risky_operations: Vec<String>,
    /// Has dependency conflict
    pub has_dependency_conflict: bool,
    /// Has multiple valid solutions
    pub has_multiple_solutions: bool,
}

impl AgentOutput {
    /// Create a clean output (no errors, tests pass)
    pub fn clean(content: String) -> Self {
        Self {
            content,
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

    /// Create output with syntax errors
    pub fn with_syntax_errors(content: String) -> Self {
        Self {
            content,
            has_syntax_errors: true,
            tests_pass: false,
            has_compile_errors: false,
            has_style_issues: false,
            modified_files: Vec::new(),
            risky_operations: Vec::new(),
            has_dependency_conflict: false,
            has_multiple_solutions: false,
        }
    }

    /// Create output with test failures
    pub fn with_test_failures(content: String) -> Self {
        Self {
            content,
            has_syntax_errors: false,
            tests_pass: false,
            has_compile_errors: false,
            has_style_issues: false,
            modified_files: Vec::new(),
            risky_operations: Vec::new(),
            has_dependency_conflict: false,
            has_multiple_solutions: false,
        }
    }

    /// Create output with risky operations
    pub fn with_risky_operations(content: String, operations: Vec<String>) -> Self {
        Self {
            content,
            has_syntax_errors: false,
            tests_pass: true,
            has_compile_errors: false,
            has_style_issues: false,
            modified_files: Vec::new(),
            risky_operations: operations,
            has_dependency_conflict: false,
            has_multiple_solutions: false,
        }
    }

    /// Convert to SimulatedOutput for checker/filter
    fn to_simulated(&self) -> SimulatedOutput {
        SimulatedOutput {
            has_syntax_errors: self.has_syntax_errors,
            tests_pass: self.tests_pass,
            has_compile_errors: self.has_compile_errors,
            has_style_issues: self.has_style_issues,
            modified_files: self.modified_files.clone(),
            risky_operations: self.risky_operations.clone(),
            has_dependency_conflict: self.has_dependency_conflict,
            has_multiple_solutions: self.has_multiple_solutions,
        }
    }
}

/// Task Decision Engine
pub struct TaskDecisionEngine {
    /// Decision process (workflow)
    process: DecisionProcess,
    /// Current stage
    current_stage: StageId,
    /// Current task
    task: Task,
    /// Prompt templates
    templates: HashMap<String, String>,
    /// Auto checker
    checker: AutoChecker,
    /// Decision filter
    filter: DecisionFilter,
    /// Task registry for persistence
    registry: TaskRegistry,
}

impl TaskDecisionEngine {
    /// Create a new task decision engine
    pub fn new(
        process: DecisionProcess,
        task: Task,
        registry: TaskRegistry,
    ) -> Self {
        let initial_stage = process.stages.first()
            .map(|s| s.id.clone())
            .unwrap_or_else(|| StageId::new("start"));

        Self {
            process,
            current_stage: initial_stage,
            task,
            templates: default_templates(),
            checker: AutoChecker::default(),
            filter: DecisionFilter::default(),
            registry,
        }
    }

    /// Set custom templates
    pub fn with_templates(mut self, templates: HashMap<String, String>) -> Self {
        self.templates = templates;
        self
    }

    /// Set custom checker
    pub fn with_checker(mut self, checker: AutoChecker) -> Self {
        self.checker = checker;
        self
    }

    /// Set custom filter
    pub fn with_filter(mut self, filter: DecisionFilter) -> Self {
        self.filter = filter;
        self
    }

    /// Get current stage
    pub fn get_current_stage(&self) -> &StageId {
        &self.current_stage
    }

    /// Get task status
    pub fn get_status(&self) -> TaskStatus {
        self.task.status
    }

    /// Get reflection count
    pub fn reflection_count(&self) -> usize {
        self.task.reflection_count
    }

    /// Check if task is complete
    pub fn is_complete(&self) -> bool {
        self.task.is_complete()
    }

    /// Get task ID
    pub fn task_id(&self) -> &TaskId {
        &self.task.id
    }

    /// Get current stage definition (cloned to avoid borrow conflicts)
    fn stage_cloned(&self) -> Option<DecisionStage> {
        self.process.stages.iter().find(|s| s.id == self.current_stage).cloned()
    }

    /// Process agent output and make decision
    pub fn process_output(&mut self, output: AgentOutput) -> DecisionAction {
        // Get current stage (cloned to avoid borrow conflicts)
        let stage = self.stage_cloned();

        // Auto-check the output
        let simulated = output.to_simulated();
        let check_result = self.checker.check(&self.task, &simulated);

        // Determine decision based on check result
        let decision = match &check_result {
            AutoCheckResult::Pass => {
                // Check if transition condition met
                self.evaluate_transition(&output, &stage)
            }
            AutoCheckResult::NeedsReflection { reason } => {
                if self.task.reflection_count < self.task.max_reflection_rounds {
                    self.task.reflection_count += 1;
                    DecisionAction::Reflect { reason: reason.clone() }
                } else {
                    DecisionAction::RequestHuman { question: format!("Max reflections reached: {}", reason) }
                }
            }
            AutoCheckResult::NeedsHuman { reason } => {
                // Check if filter agrees
                let needs_human = self.filter.needs_human_decision(&self.task, &simulated);
                if needs_human.is_some() {
                    DecisionAction::RequestHuman { question: reason.clone() }
                } else {
                    // Filter disagrees, convert WorkflowAction to DecisionAction
                    let workflow_action = self.filter.auto_decide(&self.task, &simulated);
                    workflow_to_decision(workflow_action)
                }
            }
        };

        // Log the decision
        self.log_decision(&decision, &check_result);

        // Execute decision effects
        self.execute_decision(&decision);

        decision
    }

    /// Evaluate stage transitions
    fn evaluate_transition(&mut self, output: &AgentOutput, stage: &Option<DecisionStage>) -> DecisionAction {
        if let Some(stage) = stage {
            // Check exit conditions
            for transition in &stage.transitions {
                if self.check_transition_condition(&transition.condition, output) {
                    self.current_stage = transition.target.clone();
                    return DecisionAction::AdvanceTo { stage: transition.target.clone() };
                }
            }
        }

        // No transition matched, continue
        DecisionAction::Continue
    }

    /// Check if transition condition is met
    fn check_transition_condition(&self, condition: &Condition, output: &AgentOutput) -> bool {
        match condition {
            Condition::TestsPass => output.tests_pass && !output.has_compile_errors,
            Condition::NoCompileErrors => !output.has_compile_errors && !output.has_syntax_errors,
            Condition::NoSyntaxErrors => !output.has_syntax_errors,
            Condition::StyleConformant => !output.has_style_issues,
            Condition::GoalsAchieved => output.tests_pass && !output.has_syntax_errors && !output.has_compile_errors && output.risky_operations.is_empty(),
            Condition::MaxReflectionsReached => self.task.reflection_count >= self.task.max_reflection_rounds,
            Condition::HumanApproved => self.task.status == TaskStatus::InProgress,
            Condition::TimeoutExceeded => false,
            Condition::All(conditions) => conditions.iter().all(|c| self.check_transition_condition(c, output)),
            Condition::Any(conditions) => conditions.iter().any(|c| self.check_transition_condition(c, output)),
            Condition::Not(cond) => !self.check_transition_condition(cond, output),
            Condition::Custom(_) => false,
        }
    }

    /// Execute decision effects on task
    fn execute_decision(&mut self, decision: &DecisionAction) {
        match decision {
            DecisionAction::Reflect { .. } => {
                let _ = self.task.transition_to(TaskStatus::Reflecting);
            }
            DecisionAction::ConfirmCompletion => {
                self.task.confirmation_count += 1;
                let _ = self.task.transition_to(TaskStatus::PendingConfirmation);
            }
            DecisionAction::RequestHuman { .. } => {
                let _ = self.task.transition_to(TaskStatus::NeedsHumanDecision);
            }
            DecisionAction::AdvanceTo { stage } => {
                self.current_stage = stage.clone();
                self.update_status_for_stage(stage);
            }
            DecisionAction::Cancel { .. } => {
                let _ = self.task.transition_to(TaskStatus::Cancelled);
            }
            DecisionAction::ReturnTo { stage } => {
                self.current_stage = stage.clone();
            }
            _ => {}
        }

        // Persist task state
        self.persist_task();
    }

    /// Update task status based on stage
    fn update_status_for_stage(&mut self, stage: &StageId) {
        // Map stage names to status
        let status = match stage.as_str() {
            "start" => TaskStatus::Pending,
            "developing" => TaskStatus::InProgress,
            "reflecting" => TaskStatus::Reflecting,
            "confirming" | "check_completion" => TaskStatus::PendingConfirmation,
            "completed" => TaskStatus::Completed,
            _ => self.task.status,
        };

        if status != self.task.status {
            let _ = self.task.transition_to(status);
        }
    }

    /// Persist task to registry
    fn persist_task(&mut self) {
        let _ = self.registry.update(&self.task.id, TaskUpdate::status(self.task.status));
        let _ = self.registry.update(&self.task.id, TaskUpdate::reflection_count(self.task.reflection_count));
        let _ = self.registry.update(&self.task.id, TaskUpdate::confirmation_count(self.task.confirmation_count));
    }

    /// Log decision to execution history
    fn log_decision(&mut self, decision: &DecisionAction, check_result: &AutoCheckResult) {
        let action = decision_to_workflow(decision);
        let record = ExecutionRecord::with_auto_check(action, self.current_stage.clone(), check_result);
        self.task.execution_history.push(record);
    }

    /// Generate prompt for current stage
    pub fn generate_prompt(&self) -> String {
        let template_key = self.current_stage.as_str();
        let template = match self.templates.get(template_key) {
            Some(t) => t.clone(),
            None => self.templates.get("default").cloned().unwrap_or_default(),
        };

        self.render_template(&template)
    }

    /// Render template with values
    fn render_template(&self, template: &str) -> String {
        let mut result = template.to_string();

        result = result.replace("{{task_description}}", &self.task.description);
        result = result.replace("{{task_constraints}}", &self.task.constraints.join(", "));
        result = result.replace("{{reflection_count}}", &self.task.reflection_count.to_string());
        result = result.replace("{{max_reflections}}", &self.task.max_reflection_rounds.to_string());
        result = result.replace("{{current_stage}}", self.current_stage.as_str());

        result
    }

    /// Handle human response
    pub fn handle_human_response(&mut self, response: HumanResponse) -> DecisionAction {
        let decision = match &response {
            HumanResponse::Approve => {
                let _ = self.task.transition_to(TaskStatus::InProgress);
                DecisionAction::Continue
            }
            HumanResponse::Deny { reason } => {
                if self.task.reflection_count < self.task.max_reflection_rounds {
                    self.task.reflection_count += 1;
                    let _ = self.task.transition_to(TaskStatus::Reflecting);
                    DecisionAction::Reflect { reason: reason.clone() }
                } else {
                    let _ = self.task.transition_to(TaskStatus::Cancelled);
                    DecisionAction::Cancel { reason: "Human denied after max reflections".into() }
                }
            }
            HumanResponse::Custom { feedback } => {
                self.task.reflection_count += 1;
                let _ = self.task.transition_to(TaskStatus::Reflecting);
                DecisionAction::Reflect { reason: feedback.clone() }
            }
            HumanResponse::Cancel => {
                let _ = self.task.transition_to(TaskStatus::Cancelled);
                DecisionAction::Cancel { reason: "Human cancelled".into() }
            }
        };

        // Log human response
        let action = decision_to_workflow(&decision);
        let response_str = format!("{:?}", response);
        let mut record = ExecutionRecord::new(action, self.current_stage.clone());
        record.human_requested = true;
        record.human_response = Some(response_str);

        self.task.execution_history.push(record);

        // Persist
        self.persist_task();

        decision
    }
}

/// Convert DecisionAction to WorkflowAction
fn decision_to_workflow(decision: &DecisionAction) -> WorkflowAction {
    match decision {
        DecisionAction::Continue => WorkflowAction::Continue,
        DecisionAction::Reflect { reason } => WorkflowAction::Reflect { reason: reason.clone() },
        DecisionAction::ConfirmCompletion => WorkflowAction::ConfirmCompletion,
        DecisionAction::RequestHuman { question } => WorkflowAction::RequestHuman { question: question.clone() },
        DecisionAction::AdvanceTo { stage } => WorkflowAction::AdvanceTo { stage: stage.clone() },
        DecisionAction::ReturnTo { stage } => WorkflowAction::ReturnTo { stage: stage.clone() },
        DecisionAction::Cancel { reason } => WorkflowAction::Cancel { reason: reason.clone() },
        DecisionAction::Retry => WorkflowAction::Retry,
        DecisionAction::Wait => WorkflowAction::Wait { reason: "Waiting".to_string() },
    }
}

/// Convert WorkflowAction to DecisionAction
fn workflow_to_decision(action: WorkflowAction) -> DecisionAction {
    match action {
        WorkflowAction::Continue => DecisionAction::Continue,
        WorkflowAction::Reflect { reason } => DecisionAction::Reflect { reason },
        WorkflowAction::ConfirmCompletion => DecisionAction::ConfirmCompletion,
        WorkflowAction::RequestHuman { question } => DecisionAction::RequestHuman { question },
        WorkflowAction::AdvanceTo { stage } => DecisionAction::AdvanceTo { stage },
        WorkflowAction::ReturnTo { stage } => DecisionAction::ReturnTo { stage },
        WorkflowAction::Cancel { reason } => DecisionAction::Cancel { reason },
        WorkflowAction::Retry => DecisionAction::Retry,
        WorkflowAction::Wait { reason: _ } => DecisionAction::Wait,
    }
}

/// Default prompt templates
fn default_templates() -> HashMap<String, String> {
    let mut templates = HashMap::new();

    templates.insert("start".to_string(), "Starting task: {{task_description}}\nConstraints: {{task_constraints}}".to_string());
    templates.insert("developing".to_string(), "Working on: {{task_description}}\nConstraints: {{task_constraints}}".to_string());
    templates.insert("reflecting".to_string(), "Reflection round {{reflection_count}}/{{max_reflections}}\nTask: {{task_description}}\nPlease review and improve.".to_string());
    templates.insert("confirming".to_string(), "Task {{task_description}} appears complete.\nPlease confirm completion.".to_string());
    templates.insert("check_completion".to_string(), "Verifying task: {{task_description}}\nConstraints: {{task_constraints}}".to_string());
    templates.insert("default".to_string(), "Task: {{task_description}}\nStage: {{current_stage}}\nReflections: {{reflection_count}}/{{max_reflections}}".to_string());

    templates
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workflow::default_process;
    use tempfile::TempDir;

    fn create_test_registry() -> TaskRegistry {
        let temp = TempDir::new().expect("tempdir");
        let store = Box::new(crate::persistence::FileTaskStore::new(temp.path().to_path_buf()));
        TaskRegistry::new(store)
    }

    // Story 13.1 Tests: Decision Engine Core

    #[test]
    fn t13_1_t1_engine_created_with_default_process() {
        let registry = create_test_registry();
        let task = Task::new("Test task".to_string(), vec![]);
        let process = default_process();

        let engine = TaskDecisionEngine::new(process, task, registry);

        assert_eq!(engine.get_current_stage().as_str(), "start");
    }

    #[test]
    fn t13_1_t2_engine_created_with_custom_process() {
        let registry = create_test_registry();
        let task = Task::new("Test task".to_string(), vec![]);

        let init_stage = DecisionStage::new(
            StageId::new("init"),
            "Init".to_string(),
            "Initial stage".to_string(),
        );
        let init_id = init_stage.id.clone();

        let process = DecisionProcess::new(
            "custom_process".to_string(),
            "Custom process".to_string(),
            vec![init_stage],
            init_id.clone(),
            init_id,
        );

        let engine = TaskDecisionEngine::new(process, task, registry);

        assert_eq!(engine.get_current_stage().as_str(), "init");
    }

    #[test]
    fn t13_1_t3_initial_stage_set_correctly() {
        let registry = create_test_registry();
        let task = Task::new("Test task".to_string(), vec![]);
        let process = default_process();

        let engine = TaskDecisionEngine::new(process, task, registry);

        assert_eq!(engine.get_status(), TaskStatus::Pending);
    }

    #[test]
    fn t13_1_t4_components_integrated() {
        let registry = create_test_registry();
        let task = Task::new("Test task".to_string(), vec![]);
        let process = default_process();

        let engine = TaskDecisionEngine::new(process, task, registry)
            .with_templates(HashMap::new())
            .with_checker(AutoChecker::default())
            .with_filter(DecisionFilter::default());

        // All components should be set
        assert_eq!(engine.get_status(), TaskStatus::Pending);
    }

    // Story 13.2 Tests: Output Processing

    #[test]
    fn t13_2_t1_clean_output_continue() {
        let registry = create_test_registry();
        let task = Task::new("Test task".to_string(), vec![]);
        let process = default_process();

        let mut engine = TaskDecisionEngine::new(process, task, registry);
        engine.task.transition_to(TaskStatus::InProgress).expect("transition");

        let output = AgentOutput::clean("All done".to_string());
        let decision = engine.process_output(output);

        assert!(matches!(decision, DecisionAction::Continue | DecisionAction::AdvanceTo { .. }));
    }

    #[test]
    fn t13_2_t2_syntax_error_reflect() {
        let registry = create_test_registry();
        let task = Task::new("Test task".to_string(), vec![]);
        let process = default_process();

        let mut engine = TaskDecisionEngine::new(process, task, registry);
        engine.task.transition_to(TaskStatus::InProgress).expect("transition");

        let output = AgentOutput::with_syntax_errors("Bad code".to_string());
        let decision = engine.process_output(output);

        assert!(matches!(decision, DecisionAction::Reflect { .. }));
    }

    #[test]
    fn t13_2_t3_tests_fail_reflect() {
        let registry = create_test_registry();
        let task = Task::new("Test task".to_string(), vec![]);
        let process = default_process();

        let mut engine = TaskDecisionEngine::new(process, task, registry);
        engine.task.transition_to(TaskStatus::InProgress).expect("transition");

        let output = AgentOutput::with_test_failures("Tests failed".to_string());
        let decision = engine.process_output(output);

        assert!(matches!(decision, DecisionAction::Reflect { .. }));
    }

    #[test]
    fn t13_2_t4_boundary_violation_request_human() {
        let registry = create_test_registry();
        let task = Task::new("Test task".to_string(), vec!["only_modify_login".to_string()]);
        let process = default_process();

        let mut engine = TaskDecisionEngine::new(process, task, registry);
        engine.task.transition_to(TaskStatus::InProgress).expect("transition");

        // Create output that violates boundary (modifies admin.rs instead of login.rs)
        let output = AgentOutput {
            content: "Modified admin.rs".to_string(),
            has_syntax_errors: false,
            tests_pass: true,
            has_compile_errors: false,
            has_style_issues: false,
            modified_files: vec!["admin.rs".to_string()],
            risky_operations: Vec::new(),
            has_dependency_conflict: false,
            has_multiple_solutions: false,
        };

        let decision = engine.process_output(output);

        assert!(matches!(decision, DecisionAction::RequestHuman { .. } | DecisionAction::Reflect { .. }));
    }

    #[test]
    fn t13_2_t5_goals_achieved_advance() {
        let registry = create_test_registry();
        let task = Task::new("Test task".to_string(), vec![]);
        let process = default_process();

        let mut engine = TaskDecisionEngine::new(process, task, registry);
        engine.task.transition_to(TaskStatus::InProgress).expect("transition");

        let output = AgentOutput::clean("All goals achieved".to_string());
        let decision = engine.process_output(output);

        // Should either continue or advance
        assert!(matches!(decision, DecisionAction::Continue | DecisionAction::AdvanceTo { .. }));
    }

    #[test]
    fn t13_2_t6_max_reflections_request_human() {
        let registry = create_test_registry();
        let mut task = Task::new("Test task".to_string(), vec![]);
        task.reflection_count = 2;
        task.max_reflection_rounds = 2;
        let process = default_process();

        let mut engine = TaskDecisionEngine::new(process, task, registry);
        engine.task.transition_to(TaskStatus::InProgress).expect("transition");

        let output = AgentOutput::with_syntax_errors("Still bad".to_string());
        let decision = engine.process_output(output);

        assert!(matches!(decision, DecisionAction::RequestHuman { .. }));
    }

    #[test]
    fn t13_2_t7_decision_logged() {
        let registry = create_test_registry();
        let task = Task::new("Test task".to_string(), vec![]);
        let process = default_process();

        let mut engine = TaskDecisionEngine::new(process, task, registry);
        engine.task.transition_to(TaskStatus::InProgress).expect("transition");

        let output = AgentOutput::clean("Done".to_string());
        engine.process_output(output);

        // Execution history should have at least one record
        assert!(!engine.task.execution_history.is_empty());
    }

    // Story 13.3 Tests: Prompt Generation

    #[test]
    fn t13_3_t1_prompt_generated_for_start_stage() {
        let registry = create_test_registry();
        let task = Task::new("Test task".to_string(), vec!["constraint1".to_string()]);
        let process = default_process();

        let engine = TaskDecisionEngine::new(process, task, registry);

        let prompt = engine.generate_prompt();

        assert!(prompt.contains("Test task"));
        assert!(prompt.contains("constraint1"));
    }

    #[test]
    fn t13_3_t2_prompt_generated_for_reflecting_stage() {
        let registry = create_test_registry();
        let task = Task::new("Test task".to_string(), vec![]);
        let process = default_process();

        let mut engine = TaskDecisionEngine::new(process, task, registry);
        engine.current_stage = StageId::new("reflecting");

        let prompt = engine.generate_prompt();

        assert!(prompt.contains("Reflection"));
        assert!(prompt.contains("Test task"));
    }

    #[test]
    fn t13_3_t3_variables_substituted_correctly() {
        let registry = create_test_registry();
        let mut task = Task::new("Test task".to_string(), vec![]);
        task.reflection_count = 1;
        task.max_reflection_rounds = 3;
        let process = default_process();

        let mut engine = TaskDecisionEngine::new(process, task, registry);
        engine.current_stage = StageId::new("reflecting");

        let prompt = engine.generate_prompt();

        assert!(prompt.contains("1"));
        assert!(prompt.contains("3"));
    }

    #[test]
    fn t13_3_t4_missing_variable_handled() {
        let registry = create_test_registry();
        let task = Task::new("Test".to_string(), vec![]);
        let process = default_process();

        let mut engine = TaskDecisionEngine::new(process, task, registry);
        engine.templates = HashMap::new();

        // Should handle gracefully with empty or default
        let prompt = engine.generate_prompt();
        assert!(!prompt.contains("{{")); // No unreplaced variables in default case
    }

    // Story 13.4 Tests: Human Response Handling

    #[test]
    fn t13_4_t1_approve_continue() {
        let registry = create_test_registry();
        let mut task = Task::new("Test task".to_string(), vec![]);
        // Transition through proper workflow: Pending → InProgress → Reflecting → NeedsHumanDecision
        task.transition_to(TaskStatus::InProgress).expect("transition to InProgress");
        task.transition_to(TaskStatus::Reflecting).expect("transition to Reflecting");
        task.transition_to(TaskStatus::NeedsHumanDecision).expect("transition to NeedsHumanDecision");
        let process = default_process();

        let mut engine = TaskDecisionEngine::new(process, task, registry);

        let decision = engine.handle_human_response(HumanResponse::Approve);

        assert!(matches!(decision, DecisionAction::Continue));
        assert_eq!(engine.get_status(), TaskStatus::InProgress);
    }

    #[test]
    fn t13_4_t2_deny_reflect_or_cancel() {
        let registry = create_test_registry();
        let mut task = Task::new("Test task".to_string(), vec![]);
        // Transition through proper workflow
        task.transition_to(TaskStatus::InProgress).expect("transition to InProgress");
        task.transition_to(TaskStatus::Reflecting).expect("transition to Reflecting");
        task.transition_to(TaskStatus::NeedsHumanDecision).expect("transition to NeedsHumanDecision");
        let process = default_process();

        let mut engine = TaskDecisionEngine::new(process, task, registry);

        let decision = engine.handle_human_response(HumanResponse::Deny { reason: "Not good".into() });

        assert!(matches!(decision, DecisionAction::Reflect { .. } | DecisionAction::Cancel { .. }));
    }

    #[test]
    fn t13_4_t3_custom_feedback_reflect() {
        let registry = create_test_registry();
        let mut task = Task::new("Test task".to_string(), vec![]);
        // Transition through proper workflow
        task.transition_to(TaskStatus::InProgress).expect("transition to InProgress");
        task.transition_to(TaskStatus::Reflecting).expect("transition to Reflecting");
        task.transition_to(TaskStatus::NeedsHumanDecision).expect("transition to NeedsHumanDecision");
        let process = default_process();

        let mut engine = TaskDecisionEngine::new(process, task, registry);

        let decision = engine.handle_human_response(HumanResponse::Custom { feedback: "Try this instead".into() });

        assert!(matches!(decision, DecisionAction::Reflect { .. }));
        assert!(engine.task.execution_history.iter().any(|r| r.human_response.is_some()));
    }

    #[test]
    fn t13_4_t4_response_logged() {
        let registry = create_test_registry();
        let mut task = Task::new("Test task".to_string(), vec![]);
        // Transition through proper workflow
        task.transition_to(TaskStatus::InProgress).expect("transition to InProgress");
        task.transition_to(TaskStatus::Reflecting).expect("transition to Reflecting");
        task.transition_to(TaskStatus::NeedsHumanDecision).expect("transition to NeedsHumanDecision");
        let process = default_process();

        let mut engine = TaskDecisionEngine::new(process, task, registry);

        engine.handle_human_response(HumanResponse::Approve);

        // History should have a record with human_requested
        assert!(engine.task.execution_history.iter().any(|r| r.human_requested));
    }

    // Story 13.5 Tests: Status Queries

    #[test]
    fn t13_5_t1_get_status_returns_task_status() {
        let registry = create_test_registry();
        let task = Task::new("Test task".to_string(), vec![]);
        let process = default_process();

        let engine = TaskDecisionEngine::new(process, task, registry);

        assert_eq!(engine.get_status(), TaskStatus::Pending);
    }

    #[test]
    fn t13_5_t2_reflection_count_returns_count() {
        let registry = create_test_registry();
        let mut task = Task::new("Test task".to_string(), vec![]);
        task.reflection_count = 3;
        let process = default_process();

        let engine = TaskDecisionEngine::new(process, task, registry);

        assert_eq!(engine.reflection_count(), 3);
    }

    #[test]
    fn t13_5_t3_is_complete_returns_true_when_completed() {
        let registry = create_test_registry();
        let mut task = Task::new("Test task".to_string(), vec![]);
        task.transition_to(TaskStatus::InProgress).expect("transition");
        task.transition_to(TaskStatus::PendingConfirmation).expect("transition");
        task.transition_to(TaskStatus::Completed).expect("transition");
        let process = default_process();

        let engine = TaskDecisionEngine::new(process, task, registry);

        assert!(engine.is_complete());
    }
}