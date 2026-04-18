//! Decision workflow types for task concept (Sprint 10)
//!
//! Provides structured decision workflow with stages, conditions, and transitions.

use serde::{Deserialize, Serialize};

/// Stage identifier within a decision process
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct StageId(String);

impl StageId {
    /// Create a new stage ID
    pub fn new(name: impl Into<String>) -> Self {
        Self(name.into())
    }

    /// Get string representation
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for StageId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

/// Condition for stage entry/exit and transitions
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", content = "value")]
pub enum Condition {
    // Built-in conditions
    TestsPass,
    NoCompileErrors,
    NoSyntaxErrors,
    StyleConformant,
    GoalsAchieved,
    MaxReflectionsReached,
    HumanApproved,
    TimeoutExceeded,

    // Composite conditions
    All(Vec<Condition>),
    Any(Vec<Condition>),
    Not(Box<Condition>),

    // Custom condition (name for lookup in registry)
    Custom(String),
}

impl Default for Condition {
    fn default() -> Self {
        Self::Custom("default".to_string())
    }
}

/// Transition from one stage to another
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StageTransition {
    /// Target stage
    pub target: StageId,
    /// Condition triggering this transition
    pub condition: Condition,
    /// Prompt to generate when transitioning
    pub prompt: String,
}

impl StageTransition {
    /// Create a new transition
    pub fn new(target: StageId, condition: Condition, prompt: String) -> Self {
        Self { target, condition, prompt }
    }
}

/// Decision action taken at a stage
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum WorkflowAction {
    /// Continue execution
    Continue,
    /// Reflect and fix issues
    Reflect { reason: String },
    /// Confirm task completion
    ConfirmCompletion,
    /// Request human intervention
    RequestHuman { question: String },
    /// Transition to next stage
    AdvanceTo { stage: StageId },
    /// Return to previous stage
    ReturnTo { stage: StageId },
    /// Cancel the task
    Cancel { reason: String },
    /// Retry after error
    Retry,
    /// Wait for condition
    Wait { reason: String },
}

impl WorkflowAction {
    /// Convert action to prompt text for AI
    pub fn to_prompt(&self) -> String {
        match self {
            Self::Continue => "Continue with current execution.".to_string(),
            Self::Reflect { reason } => format!("Reflect on and fix: {}", reason),
            Self::ConfirmCompletion => "Confirm task completion.".to_string(),
            Self::RequestHuman { question } => format!("Human decision needed: {}", question),
            Self::AdvanceTo { stage } => format!("Advance to stage: {}", stage),
            Self::ReturnTo { stage } => format!("Return to stage: {}", stage),
            Self::Cancel { reason } => format!("Cancel task: {}", reason),
            Self::Retry => "Retry the last operation.".to_string(),
            Self::Wait { reason } => format!("Wait: {}", reason),
        }
    }
}

impl Default for WorkflowAction {
    fn default() -> Self {
        Self::Continue
    }
}

/// Decision stage in a workflow
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecisionStage {
    /// Stage identifier
    pub id: StageId,
    /// Human-readable stage name
    pub name: String,
    /// Stage description
    pub description: String,
    /// Entry condition
    #[serde(default)]
    pub entry_condition: Condition,
    /// Exit condition
    #[serde(default)]
    pub exit_condition: Condition,
    /// Possible transitions
    #[serde(default)]
    pub transitions: Vec<StageTransition>,
    /// Available actions in this stage
    #[serde(default)]
    pub actions: Vec<WorkflowAction>,
}

impl DecisionStage {
    /// Create a new stage
    pub fn new(id: StageId, name: String, description: String) -> Self {
        Self {
            id,
            name,
            description,
            entry_condition: Condition::default(),
            exit_condition: Condition::default(),
            transitions: Vec::new(),
            actions: Vec::new(),
        }
    }

    /// Add a transition to this stage
    pub fn with_transition(mut self, transition: StageTransition) -> Self {
        self.transitions.push(transition);
        self
    }

    /// Add an action to this stage
    pub fn with_action(mut self, action: WorkflowAction) -> Self {
        self.actions.push(action);
        self
    }
}

/// Process configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessConfig {
    /// Maximum reflection rounds before escalation
    #[serde(default = "default_max_reflection_rounds")]
    pub max_reflection_rounds: usize,
    /// Whether verification is mandatory
    #[serde(default = "default_enforce_verification")]
    pub enforce_verification: bool,
    /// Task execution timeout in seconds
    #[serde(default = "default_timeout_seconds")]
    pub timeout_seconds: u64,
    /// Whether to log all decisions
    #[serde(default = "default_log_decisions")]
    pub log_decisions: bool,
}

fn default_max_reflection_rounds() -> usize { 2 }
fn default_enforce_verification() -> bool { true }
fn default_timeout_seconds() -> u64 { 1800 }
fn default_log_decisions() -> bool { true }

impl Default for ProcessConfig {
    fn default() -> Self {
        Self {
            max_reflection_rounds: default_max_reflection_rounds(),
            enforce_verification: default_enforce_verification(),
            timeout_seconds: default_timeout_seconds(),
            log_decisions: default_log_decisions(),
        }
    }
}

/// Error type for process validation
#[derive(Debug, Clone, thiserror::Error)]
pub enum ProcessValidationError {
    #[error("Initial stage '{0}' not found in process")]
    InvalidInitialStage(String),
    #[error("Final stage '{0}' not found in process")]
    InvalidFinalStage(String),
    #[error("Invalid transition from '{from}' to '{to}'")]
    InvalidTransition { from: String, to: String },
}

/// Decision process combining stages into a workflow
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecisionProcess {
    /// Process name
    pub name: String,
    /// Process description
    pub description: String,
    /// All stages in this process
    pub stages: Vec<DecisionStage>,
    /// Initial stage
    pub initial_stage: StageId,
    /// Final stage (completion)
    pub final_stage: StageId,
    /// Process configuration
    #[serde(default)]
    pub config: ProcessConfig,
}

impl DecisionProcess {
    /// Create a new process
    pub fn new(
        name: String,
        description: String,
        stages: Vec<DecisionStage>,
        initial_stage: StageId,
        final_stage: StageId,
    ) -> Self {
        Self {
            name,
            description,
            stages,
            initial_stage,
            final_stage,
            config: ProcessConfig::default(),
        }
    }

    /// Validate process integrity
    pub fn validate(&self) -> Result<(), ProcessValidationError> {
        // Check initial_stage exists
        if !self.stages.iter().any(|s| s.id == self.initial_stage) {
            return Err(ProcessValidationError::InvalidInitialStage(
                self.initial_stage.to_string(),
            ));
        }

        // Check final_stage exists
        if !self.stages.iter().any(|s| s.id == self.final_stage) {
            return Err(ProcessValidationError::InvalidFinalStage(
                self.final_stage.to_string(),
            ));
        }

        // Check all transitions target valid stages
        for stage in &self.stages {
            for transition in &stage.transitions {
                if !self.stages.iter().any(|s| s.id == transition.target) {
                    return Err(ProcessValidationError::InvalidTransition {
                        from: stage.id.to_string(),
                        to: transition.target.to_string(),
                    });
                }
            }
        }

        Ok(())
    }

    /// Get a stage by ID
    pub fn get_stage(&self, id: &StageId) -> Option<&DecisionStage> {
        self.stages.iter().find(|s| s.id == *id)
    }

    /// Check if stage exists
    pub fn has_stage(&self, id: &StageId) -> bool {
        self.stages.iter().any(|s| s.id == *id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Story 10.1 Tests: Decision Stage Definition

    #[test]
    fn t10_1_t1_stage_id_creation_and_comparison() {
        let id1 = StageId::new("start");
        let id2 = StageId::new("developing");
        let id3 = StageId::new("start");

        assert_ne!(id1, id2);
        assert_eq!(id1, id3);
        assert_eq!(id1.as_str(), "start");
    }

    #[test]
    fn t10_1_t2_stage_created_with_correct_defaults() {
        let stage = DecisionStage::new(
            StageId::new("start"),
            "Start Development".to_string(),
            "Begin task execution".to_string(),
        );

        assert_eq!(stage.id.as_str(), "start");
        assert_eq!(stage.name, "Start Development");
        assert_eq!(stage.description, "Begin task execution");
        assert!(stage.transitions.is_empty());
        assert!(stage.actions.is_empty());
    }

    #[test]
    fn t10_1_t3_transitions_stored_correctly() {
        let transition = StageTransition::new(
            StageId::new("developing"),
            Condition::Custom("ai_response".to_string()),
            "Begin implementing task".to_string(),
        );

        assert_eq!(transition.target.as_str(), "developing");
        assert_eq!(transition.condition, Condition::Custom("ai_response".to_string()));
        assert_eq!(transition.prompt, "Begin implementing task");

        let stage = DecisionStage::new(
            StageId::new("start"),
            "Start".to_string(),
            "Start stage".to_string(),
        ).with_transition(transition);

        assert_eq!(stage.transitions.len(), 1);
        assert_eq!(stage.transitions[0].target.as_str(), "developing");
    }

    #[test]
    fn t10_1_t4_stage_serialization_works() {
        let stage = DecisionStage::new(
            StageId::new("start"),
            "Start Development".to_string(),
            "Begin task execution".to_string(),
        ).with_transition(StageTransition::new(
            StageId::new("developing"),
            Condition::TestsPass,
            "Tests passed".to_string(),
        ));

        let json = serde_json::to_string(&stage).expect("Should serialize");
        let deserialized: DecisionStage =
            serde_json::from_str(&json).expect("Should deserialize");

        assert_eq!(deserialized.id, stage.id);
        assert_eq!(deserialized.name, stage.name);
        assert_eq!(deserialized.transitions.len(), 1);
    }

    // Story 10.2 Tests: Condition System

    #[test]
    fn t10_2_t1_tests_pass_condition_defined() {
        let cond = Condition::TestsPass;
        assert!(matches!(cond, Condition::TestsPass));
    }

    #[test]
    fn t10_2_t2_no_compile_errors_condition_defined() {
        let cond = Condition::NoCompileErrors;
        assert!(matches!(cond, Condition::NoCompileErrors));
    }

    #[test]
    fn t10_2_t3_goals_achieved_condition_defined() {
        let cond = Condition::GoalsAchieved;
        assert!(matches!(cond, Condition::GoalsAchieved));
    }

    #[test]
    fn t10_2_t4_max_reflections_reached_defined() {
        let cond = Condition::MaxReflectionsReached;
        assert!(matches!(cond, Condition::MaxReflectionsReached));
    }

    #[test]
    fn t10_2_t5_all_condition_composite() {
        let cond = Condition::All(vec![
            Condition::TestsPass,
            Condition::NoCompileErrors,
        ]);
        assert!(matches!(cond, Condition::All(_)));
    }

    #[test]
    fn t10_2_t6_any_condition_composite() {
        let cond = Condition::Any(vec![
            Condition::TestsPass,
            Condition::GoalsAchieved,
        ]);
        assert!(matches!(cond, Condition::Any(_)));
    }

    #[test]
    fn t10_2_t7_not_condition_composite() {
        let cond = Condition::Not(Box::new(Condition::TestsPass));
        assert!(matches!(cond, Condition::Not(_)));
    }

    #[test]
    fn t10_2_t8_custom_condition_placeholder() {
        let cond = Condition::Custom("problem_located".to_string());
        assert!(matches!(cond, Condition::Custom(_)));
    }

    #[test]
    fn t10_2_t9_condition_serialization_works() {
        let conditions = vec![
            Condition::TestsPass,
            Condition::All(vec![Condition::TestsPass, Condition::NoCompileErrors]),
            Condition::Custom("my_condition".to_string()),
        ];

        for cond in conditions {
            let json = serde_json::to_string(&cond).expect("Should serialize");
            let deserialized: Condition =
                serde_json::from_str(&json).expect("Should deserialize");
            assert_eq!(cond, deserialized);
        }
    }

    // Story 10.3 Tests: Decision Action Definition

    #[test]
    fn t10_3_t1_all_action_variants_defined() {
        let actions = [
            WorkflowAction::Continue,
            WorkflowAction::Reflect { reason: "test".to_string() },
            WorkflowAction::ConfirmCompletion,
            WorkflowAction::RequestHuman { question: "test?".to_string() },
            WorkflowAction::AdvanceTo { stage: StageId::new("next") },
            WorkflowAction::ReturnTo { stage: StageId::new("prev") },
            WorkflowAction::Cancel { reason: "test".to_string() },
            WorkflowAction::Retry,
            WorkflowAction::Wait { reason: "test".to_string() },
        ];

        // Verify distinct variants
        for (i, a1) in actions.iter().enumerate() {
            for (j, a2) in actions.iter().enumerate() {
                if i != j {
                    assert_ne!(a1, a2, "Actions should be distinct");
                }
            }
        }
    }

    #[test]
    fn t10_3_t2_reflect_action_has_reason_field() {
        let action = WorkflowAction::Reflect { reason: "Tests failed".to_string() };
        assert_eq!(action.to_prompt(), "Reflect on and fix: Tests failed");
    }

    #[test]
    fn t10_3_t3_request_human_action_has_question_field() {
        let action = WorkflowAction::RequestHuman { question: "Approve changes?".to_string() };
        assert_eq!(action.to_prompt(), "Human decision needed: Approve changes?");
    }

    #[test]
    fn t10_3_t4_actions_serialize_correctly() {
        let actions = vec![
            WorkflowAction::Continue,
            WorkflowAction::Reflect { reason: "test".to_string() },
            WorkflowAction::AdvanceTo { stage: StageId::new("next") },
        ];

        for action in actions {
            let json = serde_json::to_string(&action).expect("Should serialize");
            let deserialized: WorkflowAction =
                serde_json::from_str(&json).expect("Should deserialize");
            assert_eq!(action, deserialized);
        }
    }

    #[test]
    fn t10_3_t5_to_prompt_generates_appropriate_text() {
        assert_eq!(WorkflowAction::Continue.to_prompt(), "Continue with current execution.");
        assert_eq!(WorkflowAction::ConfirmCompletion.to_prompt(), "Confirm task completion.");
        assert_eq!(WorkflowAction::Retry.to_prompt(), "Retry the last operation.");
    }

    // Story 10.4 Tests: Decision Process Definition

    #[test]
    fn t10_4_t1_process_created_with_stages() {
        let stages = vec![
            DecisionStage::new(StageId::new("start"), "Start".to_string(), "Start".to_string()),
            DecisionStage::new(StageId::new("end"), "End".to_string(), "End".to_string()),
        ];

        let process = DecisionProcess::new(
            "Test Process".to_string(),
            "A test process".to_string(),
            stages,
            StageId::new("start"),
            StageId::new("end"),
        );

        assert_eq!(process.name, "Test Process");
        assert_eq!(process.stages.len(), 2);
        assert_eq!(process.initial_stage.as_str(), "start");
        assert_eq!(process.final_stage.as_str(), "end");
    }

    #[test]
    fn t10_4_t2_initial_final_stages_valid() {
        let stages = vec![
            DecisionStage::new(StageId::new("start"), "Start".to_string(), "Start".to_string()),
            DecisionStage::new(StageId::new("end"), "End".to_string(), "End".to_string()),
        ];

        let process = DecisionProcess::new(
            "Test".to_string(),
            "Test".to_string(),
            stages,
            StageId::new("start"),
            StageId::new("end"),
        );

        assert!(process.validate().is_ok());
    }

    #[test]
    fn t10_4_t3_process_validation_detects_issues() {
        // Invalid initial stage
        let process1 = DecisionProcess::new(
            "Test".to_string(),
            "Test".to_string(),
            vec![DecisionStage::new(StageId::new("a"), "A".to_string(), "A".to_string())],
            StageId::new("invalid"), // doesn't exist
            StageId::new("a"),
        );
        assert!(process1.validate().is_err());

        // Invalid final stage
        let process2 = DecisionProcess::new(
            "Test".to_string(),
            "Test".to_string(),
            vec![DecisionStage::new(StageId::new("a"), "A".to_string(), "A".to_string())],
            StageId::new("a"),
            StageId::new("invalid"), // doesn't exist
        );
        assert!(process2.validate().is_err());

        // Invalid transition target
        let process3 = DecisionProcess::new(
            "Test".to_string(),
            "Test".to_string(),
            vec![
                DecisionStage::new(StageId::new("a"), "A".to_string(), "A".to_string())
                    .with_transition(StageTransition::new(
                        StageId::new("invalid"), // doesn't exist
                        Condition::default(),
                        "go".to_string(),
                    )),
            ],
            StageId::new("a"),
            StageId::new("a"),
        );
        assert!(process3.validate().is_err());
    }

    #[test]
    fn t10_4_t4_process_serialization_works() {
        let process = DecisionProcess::new(
            "Test".to_string(),
            "Test process".to_string(),
            vec![
                DecisionStage::new(StageId::new("start"), "Start".to_string(), "Start".to_string()),
                DecisionStage::new(StageId::new("end"), "End".to_string(), "End".to_string()),
            ],
            StageId::new("start"),
            StageId::new("end"),
        );

        let json = serde_json::to_string(&process).expect("Should serialize");
        let deserialized: DecisionProcess =
            serde_json::from_str(&json).expect("Should deserialize");

        assert_eq!(deserialized.name, process.name);
        assert_eq!(deserialized.stages.len(), 2);
        assert_eq!(deserialized.initial_stage, process.initial_stage);
    }

    #[test]
    fn t10_4_t5_process_config_defaults() {
        let config = ProcessConfig::default();

        assert_eq!(config.max_reflection_rounds, 2);
        assert!(config.enforce_verification);
        assert_eq!(config.timeout_seconds, 1800);
        assert!(config.log_decisions);
    }

    // Story 10.5 Tests: Default Process Implementation
    // (Will be added after default_process implementation)

    #[test]
    fn t10_5_t1_default_process_creates_valid_process() {
        let process = default_process();

        assert!(process.validate().is_ok(), "Default process should be valid");
    }

    #[test]
    fn t10_5_t2_all_9_stages_defined() {
        let process = default_process();

        assert_eq!(process.stages.len(), 9, "Should have 9 stages");
    }

    #[test]
    fn t10_5_t3_all_transitions_valid() {
        let process = default_process();

        // All transitions should target existing stages
        for stage in &process.stages {
            for transition in &stage.transitions {
                assert!(
                    process.has_stage(&transition.target),
                    "Transition to {} should be valid",
                    transition.target
                );
            }
        }
    }

    #[test]
    fn t10_5_t4_initial_stage_is_start() {
        let process = default_process();

        assert_eq!(process.initial_stage.as_str(), "start");
    }

    #[test]
    fn t10_5_t5_final_stage_is_completed() {
        let process = default_process();

        assert_eq!(process.final_stage.as_str(), "completed");
    }
}

/// Create the default "Simple Agile" decision process
pub fn default_process() -> DecisionProcess {
    let stages = vec![
        // Stage 1: Start
        DecisionStage::new(
            StageId::new("start"),
            "Start Development".to_string(),
            "Task starts, AI begins execution".to_string(),
        ).with_transition(StageTransition::new(
            StageId::new("developing"),
            Condition::Custom("ai_response".to_string()),
            "Begin implementing task".to_string(),
        ))
        .with_action(WorkflowAction::Continue),

        // Stage 2: Developing
        DecisionStage::new(
            StageId::new("developing"),
            "Developing".to_string(),
            "AI developing, needs continuous decisions".to_string(),
        ).with_transition(StageTransition::new(
            StageId::new("check_quality"),
            Condition::Custom("ai_output".to_string()),
            "Check AI output for issues".to_string(),
        ))
        .with_action(WorkflowAction::Continue)
        .with_action(WorkflowAction::Reflect { reason: "Issue found".to_string() }),

        // Stage 3: Check Quality
        DecisionStage::new(
            StageId::new("check_quality"),
            "Quality Check".to_string(),
            "Check output quality".to_string(),
        ).with_transition(StageTransition::new(
            StageId::new("reflecting"),
            Condition::Custom("issue_found".to_string()),
            "Issue found, reflect and fix".to_string(),
        ))
        .with_transition(StageTransition::new(
            StageId::new("check_completion"),
            Condition::Custom("no_issue".to_string()),
            "No issues, check completion".to_string(),
        ))
        .with_action(WorkflowAction::Reflect { reason: "Quality issue".to_string() })
        .with_action(WorkflowAction::AdvanceTo { stage: StageId::new("check_completion") }),

        // Stage 4: Reflecting
        DecisionStage::new(
            StageId::new("reflecting"),
            "Reflecting".to_string(),
            "Reflect and fix issues".to_string(),
        ).with_transition(StageTransition::new(
            StageId::new("developing"),
            Condition::Custom("fixed".to_string()),
            "Issue fixed, continue development".to_string(),
        ))
        .with_transition(StageTransition::new(
            StageId::new("human_decision"),
            Condition::MaxReflectionsReached,
            "Reflection limit reached, need human".to_string(),
        ))
        .with_action(WorkflowAction::Reflect { reason: "Fix issue".to_string() })
        .with_action(WorkflowAction::RequestHuman { question: "Reflection limit".to_string() }),

        // Stage 5: Check Completion
        DecisionStage::new(
            StageId::new("check_completion"),
            "Completion Check".to_string(),
            "Check if task fully completed".to_string(),
        ).with_transition(StageTransition::new(
            StageId::new("developing"),
            Condition::Custom("not_complete".to_string()),
            "Not fully complete, continue".to_string(),
        ))
        .with_transition(StageTransition::new(
            StageId::new("confirming"),
            Condition::GoalsAchieved,
            "Goals achieved, confirm completion".to_string(),
        ))
        .with_action(WorkflowAction::Continue)
        .with_action(WorkflowAction::ConfirmCompletion),

        // Stage 6: Confirming
        DecisionStage::new(
            StageId::new("confirming"),
            "Confirming Completion".to_string(),
            "Final confirmation of completion".to_string(),
        ).with_transition(StageTransition::new(
            StageId::new("completed"),
            Condition::HumanApproved,
            "Completion confirmed".to_string(),
        ))
        .with_transition(StageTransition::new(
            StageId::new("reflecting"),
            Condition::Custom("rejected".to_string()),
            "Confirmation rejected, reflect".to_string(),
        ))
        .with_action(WorkflowAction::ConfirmCompletion)
        .with_action(WorkflowAction::RequestHuman { question: "Confirm completion?".to_string() }),

        // Stage 7: Human Decision
        DecisionStage::new(
            StageId::new("human_decision"),
            "Human Decision".to_string(),
            "Wait for human decision".to_string(),
        ).with_transition(StageTransition::new(
            StageId::new("developing"),
            Condition::HumanApproved,
            "Human approved, continue".to_string(),
        ))
        .with_transition(StageTransition::new(
            StageId::new("cancelled"),
            Condition::Custom("human_cancelled".to_string()),
            "Human cancelled".to_string(),
        ))
        .with_action(WorkflowAction::RequestHuman { question: "Decision needed".to_string() }),

        // Stage 8: Completed
        DecisionStage::new(
            StageId::new("completed"),
            "Completed".to_string(),
            "Task completed".to_string(),
        ),

        // Stage 9: Cancelled
        DecisionStage::new(
            StageId::new("cancelled"),
            "Cancelled".to_string(),
            "Task cancelled".to_string(),
        ),
    ];

    DecisionProcess::new(
        "Simple Agile".to_string(),
        "Default agile development workflow".to_string(),
        stages,
        StageId::new("start"),
        StageId::new("completed"),
    )
}