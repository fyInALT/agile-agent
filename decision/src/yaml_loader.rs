//! YAML configuration loader for custom decision processes (Sprint 15, FR-17)
//!
//! Allows users to define custom workflows via YAML files.

use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::workflow::{
    Condition, DecisionProcess, DecisionStage, ProcessConfig, ProcessValidationError,
    StageId, StageTransition, WorkflowAction,
};

/// Error type for YAML loading
#[derive(Debug, Clone, thiserror::Error)]
pub enum YamlLoadError {
    #[error("IO error: {0}")]
    Io(String),
    #[error("YAML parsing error: {0}")]
    Parse(String),
    #[error("Process validation error: {0}")]
    Validation(String),
}

impl From<std::io::Error> for YamlLoadError {
    fn from(e: std::io::Error) -> Self {
        YamlLoadError::Io(e.to_string())
    }
}

impl From<serde_yaml::Error> for YamlLoadError {
    fn from(e: serde_yaml::Error) -> Self {
        YamlLoadError::Parse(e.to_string())
    }
}

impl From<ProcessValidationError> for YamlLoadError {
    fn from(e: ProcessValidationError) -> Self {
        YamlLoadError::Validation(e.to_string())
    }
}

/// YAML representation of a process
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ProcessYaml {
    pub process: ProcessConfigYaml,
}

/// YAML representation of process configuration
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ProcessConfigYaml {
    pub name: String,
    pub description: String,
    pub stages: Vec<StageYaml>,
    pub initial_stage: String,
    pub final_stage: String,
    #[serde(default)]
    pub config: Option<ConfigYaml>,
}

/// YAML representation of a stage
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct StageYaml {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub entry_condition: Option<String>,
    #[serde(default)]
    pub exit_condition: Option<String>,
    #[serde(default)]
    pub transitions: Vec<TransitionYaml>,
    #[serde(default)]
    pub actions: Vec<String>,
}

/// YAML representation of a transition
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TransitionYaml {
    pub target: String,
    #[serde(default)]
    pub condition: Option<String>,
    #[serde(default)]
    pub prompt: Option<String>,
}

/// YAML representation of process settings
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ConfigYaml {
    #[serde(default = "default_max_reflection_rounds")]
    pub max_reflection_rounds: usize,
    #[serde(default = "default_enforce_verification")]
    pub enforce_verification: bool,
    #[serde(default = "default_timeout_seconds")]
    pub timeout_seconds: u64,
    #[serde(default = "default_log_decisions")]
    pub log_decisions: bool,
}

fn default_max_reflection_rounds() -> usize { 2 }
fn default_enforce_verification() -> bool { true }
fn default_timeout_seconds() -> u64 { 1800 }
fn default_log_decisions() -> bool { true }

impl Default for ConfigYaml {
    fn default() -> Self {
        Self {
            max_reflection_rounds: default_max_reflection_rounds(),
            enforce_verification: default_enforce_verification(),
            timeout_seconds: default_timeout_seconds(),
            log_decisions: default_log_decisions(),
        }
    }
}

impl ProcessYaml {
    /// Convert YAML representation to DecisionProcess
    pub fn to_process(&self) -> Result<DecisionProcess, YamlLoadError> {
        let stages = self.process.stages.iter()
            .map(|s| s.to_stage())
            .collect::<Result<Vec<_>, YamlLoadError>>()?;

        let config = self.process.config
            .as_ref()
            .map(|c| c.to_config())
            .unwrap_or_default();

        let process = DecisionProcess {
            name: self.process.name.clone(),
            description: self.process.description.clone(),
            stages,
            initial_stage: StageId::new(&self.process.initial_stage),
            final_stage: StageId::new(&self.process.final_stage),
            config,
        };

        process.validate()?;
        Ok(process)
    }
}

impl StageYaml {
    /// Convert YAML stage to DecisionStage
    pub fn to_stage(&self) -> Result<DecisionStage, YamlLoadError> {
        let transitions = self.transitions.iter()
            .map(|t| t.to_transition())
            .collect::<Result<Vec<_>, YamlLoadError>>()?;

        let actions = self.actions.iter()
            .map(|a| parse_action(a))
            .collect::<Result<Vec<_>, YamlLoadError>>()?;

        Ok(DecisionStage {
            id: StageId::new(&self.id),
            name: self.name.clone(),
            description: self.description.clone().unwrap_or_default(),
            entry_condition: parse_condition(self.entry_condition.as_deref()),
            exit_condition: parse_condition(self.exit_condition.as_deref()),
            transitions,
            actions,
        })
    }
}

impl TransitionYaml {
    /// Convert YAML transition to StageTransition
    pub fn to_transition(&self) -> Result<StageTransition, YamlLoadError> {
        Ok(StageTransition {
            target: StageId::new(&self.target),
            condition: parse_condition(self.condition.as_deref()),
            prompt: self.prompt.clone().unwrap_or_default(),
        })
    }
}

impl ConfigYaml {
    /// Convert YAML config to ProcessConfig
    pub fn to_config(&self) -> ProcessConfig {
        ProcessConfig {
            max_reflection_rounds: self.max_reflection_rounds,
            enforce_verification: self.enforce_verification,
            timeout_seconds: self.timeout_seconds,
            log_decisions: self.log_decisions,
        }
    }
}

/// Parse a condition string to Condition enum
fn parse_condition(s: Option<&str>) -> Condition {
    match s {
        None => Condition::default(),
        Some("TestsPass") => Condition::TestsPass,
        Some("NoCompileErrors") => Condition::NoCompileErrors,
        Some("NoSyntaxErrors") => Condition::NoSyntaxErrors,
        Some("StyleConformant") => Condition::StyleConformant,
        Some("GoalsAchieved") => Condition::GoalsAchieved,
        Some("MaxReflectionsReached") => Condition::MaxReflectionsReached,
        Some("HumanApproved") => Condition::HumanApproved,
        Some("TimeoutExceeded") => Condition::TimeoutExceeded,
        Some(name) => Condition::Custom(name.to_string()),
    }
}

/// Parse an action string to WorkflowAction
fn parse_action(s: &str) -> Result<WorkflowAction, YamlLoadError> {
    match s.trim() {
        "Continue" => Ok(WorkflowAction::Continue),
        "ConfirmCompletion" => Ok(WorkflowAction::ConfirmCompletion),
        "Retry" => Ok(WorkflowAction::Retry),
        "Reflect" => Ok(WorkflowAction::Reflect { reason: "Issue found".to_string() }),
        "RequestHuman" => Ok(WorkflowAction::RequestHuman { question: "Decision needed".to_string() }),
        other => Err(YamlLoadError::Parse(format!("Unknown action: {}", other))),
    }
}

/// Load a decision process from a YAML file
pub fn load_process_from_yaml(path: &Path) -> Result<DecisionProcess, YamlLoadError> {
    let content = fs::read_to_string(path)?;
    let yaml: ProcessYaml = serde_yaml::from_str(&content)?;
    yaml.to_process()
}

/// Save a decision process to a YAML file
pub fn save_process_to_yaml(process: &DecisionProcess, path: &Path) -> Result<(), YamlLoadError> {
    let yaml = ProcessYaml::from_process(process);
    let content = serde_yaml::to_string(&yaml)?;
    fs::write(path, content)?;
    Ok(())
}

impl ProcessYaml {
    /// Create YAML representation from DecisionProcess
    pub fn from_process(process: &DecisionProcess) -> Self {
        let stages = process.stages.iter()
            .map(|s| StageYaml::from_stage(s))
            .collect();

        let config = Some(ConfigYaml::from_config(&process.config));

        Self {
            process: ProcessConfigYaml {
                name: process.name.clone(),
                description: process.description.clone(),
                stages,
                initial_stage: process.initial_stage.as_str().to_string(),
                final_stage: process.final_stage.as_str().to_string(),
                config,
            },
        }
    }
}

impl StageYaml {
    /// Create YAML representation from DecisionStage
    pub fn from_stage(stage: &DecisionStage) -> Self {
        let transitions = stage.transitions.iter()
            .map(|t| TransitionYaml::from_transition(t))
            .collect();

        let actions = stage.actions.iter()
            .map(|a| format_action(a))
            .collect();

        Self {
            id: stage.id.as_str(),
            name: stage.name.clone(),
            description: Some(stage.description.clone()),
            entry_condition: Some(format_condition(&stage.entry_condition)),
            exit_condition: Some(format_condition(&stage.exit_condition)),
            transitions,
            actions,
        }
    }
}

impl TransitionYaml {
    /// Create YAML representation from StageTransition
    pub fn from_transition(t: &StageTransition) -> Self {
        Self {
            target: t.target.as_str(),
            condition: Some(format_condition(&t.condition)),
            prompt: Some(t.prompt.clone()),
        }
    }
}

impl ConfigYaml {
    /// Create YAML representation from ProcessConfig
    pub fn from_config(config: &ProcessConfig) -> Self {
        Self {
            max_reflection_rounds: config.max_reflection_rounds,
            enforce_verification: config.enforce_verification,
            timeout_seconds: config.timeout_seconds,
            log_decisions: config.log_decisions,
        }
    }
}

/// Format condition to string
fn format_condition(c: &Condition) -> String {
    match c {
        Condition::TestsPass => "TestsPass",
        Condition::NoCompileErrors => "NoCompileErrors",
        Condition::NoSyntaxErrors => "NoSyntaxErrors",
        Condition::StyleConformant => "StyleConformant",
        Condition::GoalsAchieved => "GoalsAchieved",
        Condition::MaxReflectionsReached => "MaxReflectionsReached",
        Condition::HumanApproved => "HumanApproved",
        Condition::TimeoutExceeded => "TimeoutExceeded",
        Condition::All(_) => "All",
        Condition::Any(_) => "Any",
        Condition::Not(_) => "Not",
        Condition::Custom(name) => name,
    }.to_string()
}

/// Format action to string
fn format_action(a: &WorkflowAction) -> String {
    match a {
        WorkflowAction::Continue => "Continue",
        WorkflowAction::Reflect { .. } => "Reflect",
        WorkflowAction::ConfirmCompletion => "ConfirmCompletion",
        WorkflowAction::RequestHuman { .. } => "RequestHuman",
        WorkflowAction::AdvanceTo { .. } => "AdvanceTo",
        WorkflowAction::ReturnTo { .. } => "ReturnTo",
        WorkflowAction::Cancel { .. } => "Cancel",
        WorkflowAction::Retry => "Retry",
        WorkflowAction::Wait { .. } => "Wait",
    }.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    // Story 15.3 Tests: YAML Process Configuration

    #[test]
    fn t15_3_t1_simple_process_loaded_from_yaml() {
        let yaml_content = r"
process:
  name: Test Process
  description: A simple test process
  stages:
    - id: start
      name: Start
      transitions:
        - target: end
          condition: GoalsAchieved
  initial_stage: start
  final_stage: end
";

        let yaml: ProcessYaml = serde_yaml::from_str(yaml_content).expect("parse");
        let process = yaml.to_process().expect("convert");

        assert_eq!(process.name, "Test Process");
        assert_eq!(process.stages.len(), 1);
    }

    #[test]
    fn t15_3_t2_stages_parsed_correctly() {
        let yaml_content = r"
process:
  name: Test
  description: Test
  stages:
    - id: stage1
      name: Stage One
      description: First stage
      transitions:
        - target: stage2
          condition: Custom(my_condition)
          prompt: Go to stage 2
    - id: stage2
      name: Stage Two
  initial_stage: stage1
  final_stage: stage2
";

        let yaml: ProcessYaml = serde_yaml::from_str(yaml_content).expect("parse");
        let process = yaml.to_process().expect("convert");

        assert_eq!(process.stages.len(), 2);
        assert_eq!(process.stages[0].id.as_str(), "stage1");
        assert_eq!(process.stages[0].transitions.len(), 1);
    }

    #[test]
    fn t15_3_t3_transitions_parsed_correctly() {
        let yaml_content = r"
process:
  name: Test
  description: Test
  stages:
    - id: a
      name: A
      transitions:
        - target: b
          condition: TestsPass
          prompt: Tests passed
        - target: c
          condition: MaxReflectionsReached
          prompt: Max reached
    - id: b
      name: B
    - id: c
      name: C
  initial_stage: a
  final_stage: c
";

        let yaml: ProcessYaml = serde_yaml::from_str(yaml_content).expect("parse");
        let process = yaml.to_process().expect("convert");

        let stage_a = process.stages.iter().find(|s| s.id.as_str() == "a").expect("stage a");
        assert_eq!(stage_a.transitions.len(), 2);
        assert_eq!(stage_a.transitions[0].condition, Condition::TestsPass);
        assert_eq!(stage_a.transitions[1].condition, Condition::MaxReflectionsReached);
    }

    #[test]
    fn t15_3_t4_invalid_yaml_returns_error() {
        let yaml_content = "not valid yaml at all {";

        let result: Result<ProcessYaml, _> = serde_yaml::from_str(yaml_content);
        assert!(result.is_err());
    }

    #[test]
    fn t15_3_t5_missing_fields_handled() {
        // Minimal YAML - missing optional fields should use defaults
        let yaml_content = r"
process:
  name: Minimal
  description: Minimal process
  stages:
    - id: only
      name: Only Stage
  initial_stage: only
  final_stage: only
";

        let yaml: ProcessYaml = serde_yaml::from_str(yaml_content).expect("parse");
        let process = yaml.to_process().expect("convert");

        assert_eq!(process.stages[0].transitions.len(), 0); // Default empty
        assert_eq!(process.config.max_reflection_rounds, 2); // Default
    }

    #[test]
    fn t15_3_t6_config_defaults_applied() {
        let yaml_content = r"
process:
  name: Test
  description: Test
  stages:
    - id: s
      name: S
  initial_stage: s
  final_stage: s
  config:
    max_reflection_rounds: 5
    enforce_verification: false
    timeout_seconds: 600
";

        let yaml: ProcessYaml = serde_yaml::from_str(yaml_content).expect("parse");
        let process = yaml.to_process().expect("convert");

        assert_eq!(process.config.max_reflection_rounds, 5);
        assert!(!process.config.enforce_verification);
        assert_eq!(process.config.timeout_seconds, 600);
    }

    #[test]
    fn t15_3_t7_load_from_file() {
        let temp = TempDir::new().expect("tempdir");
        let path = temp.path().join("process.yaml");

        let yaml_content = r"
process:
  name: File Process
  description: Loaded from file
  stages:
    - id: start
      name: Start
      transitions:
        - target: end
  initial_stage: start
  final_stage: end
";

        fs::write(&path, yaml_content).expect("write");
        let process = load_process_from_yaml(&path).expect("load");

        assert_eq!(process.name, "File Process");
    }

    #[test]
    fn t15_3_t8_save_to_file() {
        let temp = TempDir::new().expect("tempdir");
        let path = temp.path().join("saved.yaml");

        let process = crate::workflow::default_process();
        save_process_to_yaml(&process, &path).expect("save");

        assert!(path.exists());

        // Reload and verify
        let loaded = load_process_from_yaml(&path).expect("load");
        assert_eq!(loaded.name, process.name);
    }

    #[test]
    fn t15_3_t9_invalid_transition_target_error() {
        let yaml_content = r"
process:
  name: Invalid
  description: Invalid process
  stages:
    - id: start
      name: Start
      transitions:
        - target: nonexistent
  initial_stage: start
  final_stage: end
";

        let yaml: ProcessYaml = serde_yaml::from_str(yaml_content).expect("parse");
        let result = yaml.to_process();
        assert!(result.is_err());
    }
}
