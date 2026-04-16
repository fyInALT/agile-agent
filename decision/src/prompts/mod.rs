//! Configurable Decision Prompts
//!
//! Provides prompt templates for guiding LLM decision making in various situations.
//! Templates can be customized via the global configuration system.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::error::{DecisionError, Result};

// Import DecisionContext for conversion (avoid circular dependency by not importing full module)
// The conversion function will be implemented in a separate extension

// ============================================================================
// Default Prompt Templates
// ============================================================================

/// Default system prompt explaining the decision agent role
pub const DEFAULT_SYSTEM_PROMPT: &str = "\
You are a decision assistant for autonomous development agents.

Your role is to analyze agent outputs and make decisions to keep development flowing.
You help the main development agent (Claude/Codex/etc.) continue its work by:
1. Selecting appropriate options when the agent waits for user choice
2. Verifying completion claims through reflection
3. Providing continuation instructions for partial progress
4. Initiating recovery actions when errors occur

Key principles:
- Prioritize project rules and guidelines (CLAUDE.md, AGENTS.md)
- Make decisions that advance the current story/task
- Be conservative with high-impact decisions (escalate to human if uncertain)
- Learn from recent decision history to improve consistency";

/// Default prompt for waiting_for_choice situations
pub const DEFAULT_CHOICE_PROMPT: &str = "\
## Decision Task: Select an Option

The development agent is waiting for your selection. Choose the most appropriate option based on the context.

## Current Situation
{situation_text}

## Available Options
{options_text}

## Project Rules
{project_rules}

## Current Task
{task_info}

## Running Context
{running_context}

## Decision History (Recent)
{decision_history}

## Instructions
1. Analyze each option against project rules and task requirements
2. Consider which option best advances the current task
3. If uncertain about impact, consider escalating to human
4. Select ONE option and explain your reasoning

## Output Format
ACTION: select_option
PARAMETERS: {\"option_id\": \"<selected_option_id>\"}
REASONING: <brief explanation of why this option was selected>
CONFIDENCE: <number between 0.0 and 1.0>

Respond in this exact format.";

/// Default reflection prompt for round 1 (first completion claim)
pub const DEFAULT_REFLECTION_PROMPT_1: &str = "\
## Decision Task: Reflection Round 1

The development agent claims to have completed the task. Before confirming, request reflection.

## Completion Claim
{completion_summary}

## Original Task Requirements
{task_requirements}

## Project Rules
{project_rules}

## Running Context Summary
{running_context}

## Work Done
{work_summary}

## Instructions
The agent claims completion but we need verification. Request reflection with these focus areas:
1. Are all required files modified correctly?
2. Are edge cases handled properly?
3. Does the implementation follow project rules?
4. Are tests added or updated as needed?

If you see obvious gaps, point them out. If the claim seems reasonable, ask for brief reflection.

## Output Format
ACTION: reflect
PARAMETERS: {\"prompt\": \"<reflection prompt for the agent>\"}
REASONING: <why reflection is needed or if completion seems solid>
CONFIDENCE: <number between 0.0 and 1.0>

Respond in this exact format.";

/// Default reflection prompt for round 2 (second completion claim)
pub const DEFAULT_REFLECTION_PROMPT_2: &str = "\
## Decision Task: Reflection Round 2

The agent has reflected once and still claims completion. Request deeper verification.

## Completion Claim (After Reflection)
{completion_summary}

## Original Task Requirements
{task_requirements}

## First Reflection Summary
{reflection_summary}

## Project Rules
{project_rules}

## Files Modified
{file_changes}

## Instructions
This is the second completion claim. Request deeper reflection on:
1. Are there hidden bugs or edge cases?
2. Is documentation updated if needed?
3. Are there integration concerns with other components?
4. Could the implementation be simplified or improved?

Be more thorough this round. If truly complete, prepare to confirm.

## Output Format
ACTION: reflect
PARAMETERS: {\"prompt\": \"<deeper reflection prompt>\"}
REASONING: <assessment of completion thoroughness>
CONFIDENCE: <number between 0.0 and 1.0>

Respond in this exact format.";

/// Default prompt for partial_completion situations
pub const DEFAULT_PARTIAL_PROMPT: &str = "\
## Decision Task: Continue Development

The development agent has made partial progress. Determine what remains and guide continuation.

## Progress Summary
{progress_summary}

## Completed Items
{completed_items}

## Remaining Items (Estimated)
{remaining_items}

## Current Task
{task_info}

## Running Context
{running_context}

## Project Rules
{project_rules}

## Instructions
Analyze the partial progress and provide clear continuation instructions:
1. Identify what has been completed
2. Identify what remains to be done
3. Provide specific next steps
4. Consider dependencies between items

## Output Format
ACTION: continue
PARAMETERS: {\"instruction\": \"<specific continuation instruction>\"}
REASONING: <analysis of progress and next steps>
CONFIDENCE: <number between 0.0 and 1.0>

Respond in this exact format.";

/// Default prompt for error situations
pub const DEFAULT_ERROR_PROMPT: &str = "\
## Decision Task: Error Recovery

The development agent encountered an error. Analyze and determine recovery action.

## Error Type
{error_type}

## Error Details
{error_details}

## Error Context
{error_context}

## Recent History
{decision_history}

## Retry Count
{retry_count}

## Max Retries Allowed
{max_retries}

## Instructions
Analyze the error and determine appropriate recovery:
1. If retryable and retry_count < max_retries → initiate retry with adjustment
2. If retry exhausted → consider alternative approaches
3. If critical/unrecoverable → escalate to human intervention
4. Provide helpful context for recovery attempt

## Output Format
For retry:
ACTION: retry
PARAMETERS: {\"prompt\": \"<recovery instruction>\"}
REASONING: <why retry might succeed with adjustment>
CONFIDENCE: <number between 0.0 and 1.0>

For human escalation:
ACTION: request_human
PARAMETERS: {\"reason\": \"<explanation of why human needed>\"}
REASONING: <why automatic recovery is insufficient>
CONFIDENCE: <number between 0.0 and 1.0>

Respond in this exact format.";

/// Default prompt for critical decisions requiring human intervention
pub const DEFAULT_HUMAN_ESCALATION_PROMPT: &str = "\
## Decision Task: Escalate to Human

This decision requires human intervention due to critical factors.

## Decision Request ID
{decision_id}

## Criticality Score
{criticality_score}

## Critical Factors
{critical_factors}

## Available Options
{options_text}

## Decision Agent Analysis
{preliminary_analysis}

## Recommendation
{recommendation}

## Impact Assessment
{impact_assessment}

## Instructions
Prepare a clear escalation request for the human operator:
1. Summarize why this decision is critical
2. Present all options with analysis
3. Provide your recommendation with reasoning
4. Note the confidence level

The human will make the final decision. Your role is to inform, not decide.

## Output Format
ACTION: request_human
PARAMETERS: {\"request\": {\"reason\": \"<critical factors summary>\", \"options\": [<option list>], \"recommendation\": \"<your recommendation>\", \"confidence\": <your confidence>}}
REASONING: <why this decision must be made by human>
CONFIDENCE: <number between 0.0 and 1.0 for your recommendation confidence>

Respond in this exact format.";

/// Default prompt for verifying final completion
pub const DEFAULT_VERIFY_PROMPT: &str = "\
## Decision Task: Final Completion Verification

This is the final verification before confirming task completion.

## Completion Summary
{completion_summary}

## Task Definition of Done
{definition_of_done}

## Files Modified Summary
{files_modified}

## Tests Status
{tests_status}

## Integration Check
{integration_check}

## Project Rules Compliance
{rules_compliance}

## Instructions
Perform final verification:
1. Check each Definition of Done item
2. Verify file modifications are appropriate
3. Confirm tests are passing or added
4. Check for integration concerns
5. Verify project rules compliance

If all criteria met, confirm completion. If gaps remain, list them.

## Output Format
For confirmed completion:
ACTION: confirm_completion
PARAMETERS: {\"verified\": true, \"summary\": \"<verification summary>\"}
REASONING: <why completion is verified>
CONFIDENCE: <number between 0.0 and 1.0>

For incomplete:
ACTION: continue
PARAMETERS: {\"instruction\": \"<list of remaining items>\"}
REASONING: <what gaps were found>
CONFIDENCE: <number between 0.0 and 1.0>

Respond in this exact format.";

// ============================================================================
// Prompt Configuration
// ============================================================================

/// Prompt configuration for decision layer
///
/// Can be loaded from global config and customized per deployment.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptConfig {
    /// System prompt (role explanation)
    #[serde(default = "default_system_prompt")]
    pub system_prompt: String,

    /// Prompt for waiting_for_choice situations
    #[serde(default = "default_choice_prompt")]
    pub choice_prompt: String,

    /// First reflection prompt (round 1)
    #[serde(default = "default_reflection_prompt_1")]
    pub reflection_prompt_1: String,

    /// Second reflection prompt (round 2)
    #[serde(default = "default_reflection_prompt_2")]
    pub reflection_prompt_2: String,

    /// Prompt for partial completion situations
    #[serde(default = "default_partial_prompt")]
    pub partial_prompt: String,

    /// Prompt for error recovery situations
    #[serde(default = "default_error_prompt")]
    pub error_prompt: String,

    /// Prompt for human escalation
    #[serde(default = "default_human_escalation_prompt")]
    pub human_escalation_prompt: String,

    /// Prompt for final completion verification
    #[serde(default = "default_verify_prompt")]
    pub verify_prompt: String,

    /// Maximum reflection rounds (default: 2)
    #[serde(default = "default_max_reflection_rounds")]
    pub max_reflection_rounds: u8,

    /// Custom situation prompts (situation_type -> prompt template)
    #[serde(default)]
    pub custom_prompts: HashMap<String, String>,
}

fn default_system_prompt() -> String {
    DEFAULT_SYSTEM_PROMPT.to_string()
}

fn default_choice_prompt() -> String {
    DEFAULT_CHOICE_PROMPT.to_string()
}

fn default_reflection_prompt_1() -> String {
    DEFAULT_REFLECTION_PROMPT_1.to_string()
}

fn default_reflection_prompt_2() -> String {
    DEFAULT_REFLECTION_PROMPT_2.to_string()
}

fn default_partial_prompt() -> String {
    DEFAULT_PARTIAL_PROMPT.to_string()
}

fn default_error_prompt() -> String {
    DEFAULT_ERROR_PROMPT.to_string()
}

fn default_human_escalation_prompt() -> String {
    DEFAULT_HUMAN_ESCALATION_PROMPT.to_string()
}

fn default_verify_prompt() -> String {
    DEFAULT_VERIFY_PROMPT.to_string()
}

fn default_max_reflection_rounds() -> u8 {
    2
}

/// Minimum valid reflection rounds
const MIN_REFLECTION_ROUNDS: u8 = 1;

/// Maximum valid reflection rounds
const MAX_REFLECTION_ROUNDS: u8 = 5;

impl Default for PromptConfig {
    fn default() -> Self {
        Self {
            system_prompt: default_system_prompt(),
            choice_prompt: default_choice_prompt(),
            reflection_prompt_1: default_reflection_prompt_1(),
            reflection_prompt_2: default_reflection_prompt_2(),
            partial_prompt: default_partial_prompt(),
            error_prompt: default_error_prompt(),
            human_escalation_prompt: default_human_escalation_prompt(),
            verify_prompt: default_verify_prompt(),
            max_reflection_rounds: default_max_reflection_rounds(),
            custom_prompts: HashMap::new(),
        }
    }
}

impl PromptConfig {
    /// Create new prompt config with defaults
    pub fn new() -> Self {
        Self::default()
    }

    /// Load from file path
    pub fn from_file(path: &std::path::Path) -> Result<Self> {
        if !path.exists() {
            return Ok(Self::default());
        }
        let content = std::fs::read_to_string(path)
            .map_err(|e| DecisionError::PersistenceError(e.to_string()))?;
        let config: Self = serde_json::from_str(&content)
            .map_err(DecisionError::JsonError)?;
        Ok(config)
    }

    /// Save to file path
    pub fn to_file(&self, path: &std::path::Path) -> Result<()> {
        let content = serde_json::to_string_pretty(self)
            .map_err(DecisionError::JsonError)?;
        std::fs::write(path, content)
            .map_err(|e| DecisionError::PersistenceError(e.to_string()))?;
        Ok(())
    }

    /// Get prompt for a specific situation type
    pub fn get_prompt_for_situation(&self, situation_type: &str) -> &str {
        // Check custom prompts first
        if let Some(custom) = self.custom_prompts.get(situation_type) {
            return custom;
        }

        // Fall back to standard prompts
        match situation_type {
            "waiting_for_choice" => &self.choice_prompt,
            "claims_completion" => &self.reflection_prompt_1, // Default to round 1
            "partial_completion" => &self.partial_prompt,
            "error" => &self.error_prompt,
            _ => &self.choice_prompt, // Default fallback
        }
    }

    /// Add a custom prompt for a situation type
    pub fn add_custom_prompt(&mut self, situation_type: String, prompt: String) {
        self.custom_prompts.insert(situation_type, prompt);
    }

    /// Validate configuration
    ///
    /// Returns error if configuration values are invalid.
    pub fn validate(&self) -> Result<()> {
        // Validate max_reflection_rounds
        if self.max_reflection_rounds < MIN_REFLECTION_ROUNDS {
            return Err(DecisionError::ConfigError(format!(
                "max_reflection_rounds ({}) must be at least {}",
                self.max_reflection_rounds, MIN_REFLECTION_ROUNDS
            )));
        }
        if self.max_reflection_rounds > MAX_REFLECTION_ROUNDS {
            return Err(DecisionError::ConfigError(format!(
                "max_reflection_rounds ({}) must be at most {}",
                self.max_reflection_rounds, MAX_REFLECTION_ROUNDS
            )));
        }

        // Validate that custom prompts contain at least one placeholder
        for (situation_type, prompt) in &self.custom_prompts {
            if !prompt.contains('{') || !prompt.contains('}') {
                return Err(DecisionError::ConfigError(format!(
                    "Custom prompt for '{}' must contain at least one placeholder like {{situation_text}}",
                    situation_type
                )));
            }
        }

        Ok(())
    }

    /// Load from file with validation
    pub fn from_file_validated(path: &std::path::Path) -> Result<Self> {
        let config = Self::from_file(path)?;
        config.validate()?;
        Ok(config)
    }
}

// ============================================================================
// Prompt Builder
// ============================================================================

/// Context variables for prompt construction
#[derive(Debug, Clone)]
pub struct PromptVariables {
    /// Situation description text
    pub situation_text: String,

    /// Available options text
    pub options_text: String,

    /// Project rules summary
    pub project_rules: String,

    /// Current task information
    pub task_info: String,

    /// Running context summary
    pub running_context: String,

    /// Decision history summary
    pub decision_history: String,

    /// Completion summary (for reflection)
    pub completion_summary: Option<String>,

    /// Task requirements (for reflection)
    pub task_requirements: Option<String>,

    /// Work summary (for reflection)
    pub work_summary: Option<String>,

    /// Reflection summary from previous round
    pub reflection_summary: Option<String>,

    /// File changes summary
    pub file_changes: Option<String>,

    /// Progress summary (for partial)
    pub progress_summary: Option<String>,

    /// Completed items (for partial)
    pub completed_items: Option<String>,

    /// Remaining items (for partial)
    pub remaining_items: Option<String>,

    /// Error type
    pub error_type: Option<String>,

    /// Error details
    pub error_details: Option<String>,

    /// Error context
    pub error_context: Option<String>,

    /// Retry count
    pub retry_count: Option<u32>,

    /// Max retries
    pub max_retries: Option<u32>,

    /// Decision ID (for human escalation)
    pub decision_id: Option<String>,

    /// Criticality score (for human escalation)
    pub criticality_score: Option<u8>,

    /// Critical factors (for human escalation)
    pub critical_factors: Option<String>,

    /// Preliminary analysis (for human escalation)
    pub preliminary_analysis: Option<String>,

    /// Recommendation (for human escalation)
    pub recommendation: Option<String>,

    /// Impact assessment (for human escalation)
    pub impact_assessment: Option<String>,

    /// Definition of Done (for verification)
    pub definition_of_done: Option<String>,

    /// Files modified (for verification)
    pub files_modified: Option<String>,

    /// Tests status (for verification)
    pub tests_status: Option<String>,

    /// Integration check (for verification)
    pub integration_check: Option<String>,

    /// Rules compliance (for verification)
    pub rules_compliance: Option<String>,

    /// Reflection round number
    pub reflection_round: Option<u8>,
}

impl Default for PromptVariables {
    fn default() -> Self {
        Self {
            situation_text: "No situation description".to_string(),
            options_text: "No options available".to_string(),
            project_rules: "No project rules specified".to_string(),
            task_info: "No task assigned".to_string(),
            running_context: "No running context".to_string(),
            decision_history: "No previous decisions".to_string(),
            completion_summary: None,
            task_requirements: None,
            work_summary: None,
            reflection_summary: None,
            file_changes: None,
            progress_summary: None,
            completed_items: None,
            remaining_items: None,
            error_type: None,
            error_details: None,
            error_context: None,
            retry_count: None,
            max_retries: None,
            decision_id: None,
            criticality_score: None,
            critical_factors: None,
            preliminary_analysis: None,
            recommendation: None,
            impact_assessment: None,
            definition_of_done: None,
            files_modified: None,
            tests_status: None,
            integration_check: None,
            rules_compliance: None,
            reflection_round: None,
        }
    }
}

impl PromptVariables {
    /// Create new empty variables
    pub fn new() -> Self {
        Self::default()
    }

    /// Set situation text
    pub fn with_situation(mut self, text: String) -> Self {
        self.situation_text = text;
        self
    }

    /// Set options text
    pub fn with_options(mut self, text: String) -> Self {
        self.options_text = text;
        self
    }

    /// Set project rules
    pub fn with_project_rules(mut self, rules: String) -> Self {
        self.project_rules = rules;
        self
    }

    /// Set task info
    pub fn with_task_info(mut self, info: String) -> Self {
        self.task_info = info;
        self
    }

    /// Set running context
    pub fn with_running_context(mut self, context: String) -> Self {
        self.running_context = context;
        self
    }

    /// Set decision history
    pub fn with_decision_history(mut self, history: String) -> Self {
        self.decision_history = history;
        self
    }

    /// Set completion summary
    pub fn with_completion_summary(mut self, summary: String) -> Self {
        self.completion_summary = Some(summary);
        self
    }

    /// Set reflection round
    pub fn with_reflection_round(mut self, round: u8) -> Self {
        self.reflection_round = Some(round);
        self
    }

    /// Set error details
    pub fn with_error(mut self, error_type: String, details: String) -> Self {
        self.error_type = Some(error_type);
        self.error_details = Some(details);
        self
    }

    /// Set retry info
    pub fn with_retry_info(mut self, count: u32, max: u32) -> Self {
        self.retry_count = Some(count);
        self.max_retries = Some(max);
        self
    }

    /// Set file changes summary
    pub fn with_file_changes(mut self, changes: String) -> Self {
        self.file_changes = Some(changes);
        self
    }

    /// Set task requirements
    pub fn with_task_requirements(mut self, requirements: String) -> Self {
        self.task_requirements = Some(requirements);
        self
    }

    /// Set work summary
    pub fn with_work_summary(mut self, summary: String) -> Self {
        self.work_summary = Some(summary);
        self
    }

    /// Format decision history from records
    ///
    /// Takes a list of (situation_name, action_name, confidence) tuples
    /// and formats them for the prompt.
    pub fn with_formatted_history(mut self, records: &[(String, String, f64)]) -> Self {
        if records.is_empty() {
            self.decision_history = "No previous decisions.".to_string();
        } else {
            let entries: Vec<String> = records
                .iter()
                .rev()
                .take(5)
                .map(|(sit, act, conf)| format!("- {} -> {} (confidence: {:.2})", sit, act, conf))
                .collect();
            self.decision_history = entries.join("\n");
        }
        self
    }
}

/// Builder for constructing prompts from templates
pub struct PromptBuilder {
    /// Prompt configuration
    config: PromptConfig,
}

impl PromptBuilder {
    /// Create new builder with default config
    pub fn new() -> Self {
        Self {
            config: PromptConfig::default(),
        }
    }

    /// Create builder with custom config
    pub fn with_config(config: PromptConfig) -> Self {
        Self { config }
    }

    /// Get the configuration
    pub fn config(&self) -> &PromptConfig {
        &self.config
    }

    /// Build the full decision prompt
    pub fn build(
        &self,
        situation_type: &str,
        variables: &PromptVariables,
    ) -> String {
        // Start with system prompt
        let system_prompt = &self.config.system_prompt;

        // Get situation-specific prompt template
        let template = self.get_template_for_situation(situation_type, variables);

        // Interpolate variables
        let interpolated = self.interpolate(&template, variables);

        // Combine system + situation
        format!(
            "{}\n\n---\n\n{}",
            system_prompt,
            interpolated
        )
    }

    /// Get template for situation, considering reflection round
    fn get_template_for_situation(
        &self,
        situation_type: &str,
        variables: &PromptVariables,
    ) -> String {
        // Check custom prompts FIRST (including claims_completion custom)
        if let Some(custom) = self.config.custom_prompts.get(situation_type) {
            return custom.clone();
        }

        // Special handling for claims_completion with reflection rounds
        if situation_type == "claims_completion" {
            let round = variables.reflection_round.unwrap_or(1);
            let max_rounds = self.config.max_reflection_rounds;

            if round <= max_rounds {
                // Within configured reflection rounds
                if round == 1 {
                    return self.config.reflection_prompt_1.clone();
                } else {
                    // Round 2+ use reflection_prompt_2
                    return self.config.reflection_prompt_2.clone();
                }
            } else {
                // Exceeded max reflection rounds -> verify
                return self.config.verify_prompt.clone();
            }
        }

        // Standard situation prompts
        match situation_type {
            "waiting_for_choice" => self.config.choice_prompt.clone(),
            "partial_completion" => self.config.partial_prompt.clone(),
            "error" => self.config.error_prompt.clone(),
            "human_escalation" => self.config.human_escalation_prompt.clone(),
            "verify_completion" => self.config.verify_prompt.clone(),
            _ => self.config.choice_prompt.clone(),
        }
    }

    /// Interpolate variables into template
    fn interpolate(&self, template: &str, variables: &PromptVariables) -> String {
        let mut result = template.to_string();

        // Standard variables
        result = result.replace("{situation_text}", &variables.situation_text);
        result = result.replace("{options_text}", &variables.options_text);
        result = result.replace("{project_rules}", &variables.project_rules);
        result = result.replace("{task_info}", &variables.task_info);
        result = result.replace("{running_context}", &variables.running_context);
        result = result.replace("{decision_history}", &variables.decision_history);

        // Optional variables - replace with empty string or default if not set
        result = result.replace(
            "{completion_summary}",
            variables.completion_summary.as_deref().unwrap_or("Not available"),
        );
        result = result.replace(
            "{task_requirements}",
            variables.task_requirements.as_deref().unwrap_or("Not specified"),
        );
        result = result.replace(
            "{work_summary}",
            variables.work_summary.as_deref().unwrap_or("No work summary available"),
        );
        result = result.replace(
            "{reflection_summary}",
            variables.reflection_summary.as_deref().unwrap_or("No previous reflection"),
        );
        result = result.replace(
            "{file_changes}",
            variables.file_changes.as_deref().unwrap_or("No file changes recorded"),
        );
        result = result.replace(
            "{progress_summary}",
            variables.progress_summary.as_deref().unwrap_or("No progress summary"),
        );
        result = result.replace(
            "{completed_items}",
            variables.completed_items.as_deref().unwrap_or("None identified"),
        );
        result = result.replace(
            "{remaining_items}",
            variables.remaining_items.as_deref().unwrap_or("None identified"),
        );
        result = result.replace(
            "{error_type}",
            variables.error_type.as_deref().unwrap_or("Unknown"),
        );
        result = result.replace(
            "{error_details}",
            variables.error_details.as_deref().unwrap_or("No details available"),
        );
        result = result.replace(
            "{error_context}",
            variables.error_context.as_deref().unwrap_or("No context available"),
        );
        result = result.replace(
            "{retry_count}",
            &variables.retry_count.map(|c| c.to_string()).unwrap_or_else(|| "0".to_string()),
        );
        result = result.replace(
            "{max_retries}",
            &variables.max_retries.map(|m| m.to_string()).unwrap_or_else(|| "3".to_string()),
        );
        result = result.replace(
            "{decision_id}",
            variables.decision_id.as_deref().unwrap_or("unknown"),
        );
        result = result.replace(
            "{criticality_score}",
            &variables.criticality_score.map(|s| s.to_string()).unwrap_or_else(|| "0".to_string()),
        );
        result = result.replace(
            "{critical_factors}",
            variables.critical_factors.as_deref().unwrap_or("None identified"),
        );
        result = result.replace(
            "{preliminary_analysis}",
            variables.preliminary_analysis.as_deref().unwrap_or("No analysis available"),
        );
        result = result.replace(
            "{recommendation}",
            variables.recommendation.as_deref().unwrap_or("No recommendation"),
        );
        result = result.replace(
            "{impact_assessment}",
            variables.impact_assessment.as_deref().unwrap_or("Not assessed"),
        );
        result = result.replace(
            "{definition_of_done}",
            variables.definition_of_done.as_deref().unwrap_or("Not specified"),
        );
        result = result.replace(
            "{files_modified}",
            variables.files_modified.as_deref().unwrap_or("No files recorded"),
        );
        result = result.replace(
            "{tests_status}",
            variables.tests_status.as_deref().unwrap_or("Unknown"),
        );
        result = result.replace(
            "{integration_check}",
            variables.integration_check.as_deref().unwrap_or("Not checked"),
        );
        result = result.replace(
            "{rules_compliance}",
            variables.rules_compliance.as_deref().unwrap_or("Not verified"),
        );

        result
    }
}

impl Default for PromptBuilder {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Criticality Assessment
// ============================================================================

/// Critical decision criteria for human escalation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CriticalityCriteria {
    /// Impact affects multiple agents
    pub multi_agent_impact: bool,

    /// Operation is irreversible
    pub irreversible: bool,

    /// High risk operation
    pub high_risk: bool,

    /// Confidence is below threshold
    pub low_confidence: bool,

    /// Cost exceeds threshold
    pub high_cost: bool,

    /// Project rules require human confirmation
    pub requires_human: bool,

    /// Always report types (e.g., pr_merge, deploy)
    pub always_report_types: Vec<String>,
}

impl Default for CriticalityCriteria {
    fn default() -> Self {
        Self {
            multi_agent_impact: false,
            irreversible: false,
            high_risk: false,
            low_confidence: false,
            high_cost: false,
            requires_human: false,
            always_report_types: vec![
                "pr_merge".to_string(),
                "deploy".to_string(),
                "database_migration".to_string(),
                "architectural_change".to_string(),
                "delete_files".to_string(),
            ],
        }
    }
}

impl CriticalityCriteria {
    /// Calculate criticality score (0-12)
    pub fn calculate_score(&self) -> u8 {
        let mut score = 0u8;
        if self.multi_agent_impact { score += 2; }
        if self.irreversible { score += 3; }
        if self.high_risk { score += 3; }
        if self.low_confidence { score += 1; }
        if self.high_cost { score += 1; }
        if self.requires_human { score += 2; }
        score
    }

    /// Check if decision type is in the always-report list
    pub fn is_always_report_type(&self, decision_type: &str) -> bool {
        self.always_report_types.iter().any(|t| t == decision_type)
    }

    /// Check if decision should be escalated to human
    pub fn should_escalate(&self, threshold: u8) -> bool {
        self.calculate_score() >= threshold
    }

    /// Check if decision should be escalated, considering decision type
    pub fn should_escalate_with_type(&self, threshold: u8, decision_type: &str) -> bool {
        // Always escalate if decision type is in always_report_types
        if self.is_always_report_type(decision_type) {
            return true;
        }
        self.should_escalate(threshold)
    }

    /// Get description of critical factors
    pub fn describe_factors(&self) -> String {
        let mut factors = Vec::new();
        if self.multi_agent_impact {
            factors.push("Affects multiple agents");
        }
        if self.irreversible {
            factors.push("Irreversible operation");
        }
        if self.high_risk {
            factors.push("High risk operation");
        }
        if self.low_confidence {
            factors.push("Decision confidence below threshold");
        }
        if self.high_cost {
            factors.push("High cost operation");
        }
        if self.requires_human {
            factors.push("Project rules require human confirmation");
        }
        factors.join(", ")
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_prompt_config_defaults() {
        let config = PromptConfig::default();
        assert!(!config.system_prompt.is_empty());
        assert!(!config.choice_prompt.is_empty());
        assert_eq!(config.max_reflection_rounds, 2);
    }

    #[test]
    fn test_prompt_config_save_load() {
        let temp = TempDir::new().unwrap();
        let path = temp.path().join("prompts.json");

        let config = PromptConfig {
            max_reflection_rounds: 3,
            custom_prompts: HashMap::from([
                ("custom_situation".to_string(), "Custom prompt".to_string()),
            ]),
            ..Default::default()
        };

        config.to_file(&path).unwrap();
        let loaded = PromptConfig::from_file(&path).unwrap();

        assert_eq!(loaded.max_reflection_rounds, 3);
        assert!(loaded.custom_prompts.contains_key("custom_situation"));
    }

    #[test]
    fn test_prompt_config_from_missing_file() {
        let temp = TempDir::new().unwrap();
        let path = temp.path().join("nonexistent.json");

        let config = PromptConfig::from_file(&path).unwrap();
        assert_eq!(config.max_reflection_rounds, 2); // Default
    }

    #[test]
    fn test_prompt_builder_basic() {
        let builder = PromptBuilder::new();
        let variables = PromptVariables::new()
            .with_situation("Agent waiting for option selection".to_string())
            .with_options("[A] Option 1\n[B] Option 2".to_string())
            .with_task_info("Implement feature X".to_string());

        let prompt = builder.build("waiting_for_choice", &variables);

        assert!(prompt.contains("Agent waiting for option selection"));
        assert!(prompt.contains("[A] Option 1"));
        assert!(prompt.contains("Implement feature X"));
        assert!(prompt.contains("You are a decision assistant"));
    }

    #[test]
    fn test_prompt_builder_reflection_rounds() {
        let builder = PromptBuilder::new();

        // Round 1
        let vars1 = PromptVariables::new().with_reflection_round(1);
        let prompt1 = builder.build("claims_completion", &vars1);
        assert!(prompt1.contains("Reflection Round 1"));

        // Round 2
        let vars2 = PromptVariables::new().with_reflection_round(2);
        let prompt2 = builder.build("claims_completion", &vars2);
        assert!(prompt2.contains("Reflection Round 2"));
    }

    #[test]
    fn test_prompt_builder_error() {
        let builder = PromptBuilder::new();
        let variables = PromptVariables::new()
            .with_error("command_failed".to_string(), "Exit code 1".to_string())
            .with_retry_info(1, 3);

        let prompt = builder.build("error", &variables);

        assert!(prompt.contains("command_failed"));
        assert!(prompt.contains("Exit code 1"));
        assert!(prompt.contains("retry_count")); // Template has this placeholder
    }

    #[test]
    fn test_prompt_variables_optional() {
        let vars = PromptVariables::new()
            .with_completion_summary("Done".to_string())
            .with_reflection_round(2);

        assert_eq!(vars.completion_summary, Some("Done".to_string()));
        assert_eq!(vars.reflection_round, Some(2));
        assert_eq!(vars.error_type, None);
    }

    #[test]
    fn test_criticality_criteria() {
        let criteria = CriticalityCriteria {
            irreversible: true,
            high_risk: true,
            ..Default::default()
        };

        assert_eq!(criteria.calculate_score(), 6); // 3 + 3
        assert!(criteria.should_escalate(5));
        assert!(!criteria.should_escalate(7));
    }

    #[test]
    fn test_criticality_factors_description() {
        let criteria = CriticalityCriteria {
            multi_agent_impact: true,
            irreversible: true,
            ..Default::default()
        };

        let desc = criteria.describe_factors();
        assert!(desc.contains("multiple agents"));
        assert!(desc.contains("Irreversible"));
    }

    #[test]
    fn test_custom_prompt() {
        let mut config = PromptConfig::default();
        config.add_custom_prompt(
            "custom_situation".to_string(),
            "This is a custom prompt template with {situation_text}".to_string(),
        );

        let builder = PromptBuilder::with_config(config);
        let variables = PromptVariables::new()
            .with_situation("Custom situation".to_string());

        let prompt = builder.build("custom_situation", &variables);
        assert!(prompt.contains("Custom situation"));
    }

    #[test]
    fn test_default_templates_not_empty() {
        assert!(!DEFAULT_SYSTEM_PROMPT.is_empty());
        assert!(!DEFAULT_CHOICE_PROMPT.is_empty());
        assert!(!DEFAULT_REFLECTION_PROMPT_1.is_empty());
        assert!(!DEFAULT_REFLECTION_PROMPT_2.is_empty());
        assert!(!DEFAULT_PARTIAL_PROMPT.is_empty());
        assert!(!DEFAULT_ERROR_PROMPT.is_empty());
        assert!(!DEFAULT_HUMAN_ESCALATION_PROMPT.is_empty());
        assert!(!DEFAULT_VERIFY_PROMPT.is_empty());
    }

    #[test]
    fn test_interpolation_missing_vars() {
        let builder = PromptBuilder::new();
        let variables = PromptVariables::new(); // All defaults

        let prompt = builder.build("waiting_for_choice", &variables);

        // Should have placeholders filled with defaults
        assert!(!prompt.contains("{situation_text}")); // All placeholders should be resolved
        assert!(!prompt.contains("{options_text}"));
    }

    #[test]
    fn test_custom_prompts_override_claims_completion() {
        // Test that custom prompts work for claims_completion
        let mut config = PromptConfig::default();
        config.add_custom_prompt(
            "claims_completion".to_string(),
            "Custom claims completion prompt with {completion_summary}".to_string(),
        );

        let builder = PromptBuilder::with_config(config);
        let variables = PromptVariables::new()
            .with_completion_summary("Task done".to_string())
            .with_reflection_round(1);

        let prompt = builder.build("claims_completion", &variables);
        // Should use custom prompt, not the default round-based logic
        assert!(prompt.contains("Custom claims completion prompt"));
    }

    #[test]
    fn test_max_reflection_rounds_config() {
        // Test max_reflection_rounds = 1
        let config = PromptConfig {
            max_reflection_rounds: 1,
            ..Default::default()
        };
        let builder = PromptBuilder::with_config(config);

        // Round 1 should use reflection_prompt_1
        let vars1 = PromptVariables::new().with_reflection_round(1);
        let prompt1 = builder.build("claims_completion", &vars1);
        assert!(prompt1.contains("Reflection Round 1"));

        // Round 2 should jump to verify (since max is 1)
        let vars2 = PromptVariables::new().with_reflection_round(2);
        let prompt2 = builder.build("claims_completion", &vars2);
        assert!(prompt2.contains("Final Completion Verification"));
    }

    #[test]
    fn test_always_report_types() {
        let criteria = CriticalityCriteria::default();

        // Default types should be in always_report_types
        assert!(criteria.is_always_report_type("pr_merge"));
        assert!(criteria.is_always_report_type("deploy"));
        assert!(criteria.is_always_report_type("database_migration"));

        // Non-always types
        assert!(!criteria.is_always_report_type("file_read"));
        assert!(!criteria.is_always_report_type("test_run"));
    }

    #[test]
    fn test_should_escalate_with_type() {
        let criteria = CriticalityCriteria {
            irreversible: false,
            high_risk: false,
            ..Default::default()
        };

        // Score is 0, threshold is 5 - normally wouldn't escalate
        assert!(!criteria.should_escalate(5));

        // But pr_merge is in always_report_types, so should escalate
        assert!(criteria.should_escalate_with_type(5, "pr_merge"));
        assert!(criteria.should_escalate_with_type(5, "deploy"));
    }

    #[test]
    fn test_config_validation_valid() {
        let config = PromptConfig::default();
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_config_validation_min_reflection_rounds() {
        let config = PromptConfig {
            max_reflection_rounds: 0,
            ..Default::default()
        };
        let result = config.validate();
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("must be at least"));
    }

    #[test]
    fn test_config_validation_max_reflection_rounds() {
        let config = PromptConfig {
            max_reflection_rounds: 10,
            ..Default::default()
        };
        let result = config.validate();
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("must be at most"));
    }

    #[test]
    fn test_config_validation_custom_prompt_no_placeholder() {
        let mut config = PromptConfig::default();
        config.add_custom_prompt(
            "test_situation".to_string(),
            "This prompt has no placeholders".to_string(),
        );
        let result = config.validate();
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("must contain at least one placeholder"));
    }

    #[test]
    fn test_config_validation_custom_prompt_valid() {
        let mut config = PromptConfig::default();
        config.add_custom_prompt(
            "test_situation".to_string(),
            "Prompt with {situation_text} placeholder".to_string(),
        );
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_prompt_variables_with_formatted_history() {
        let records = vec![
            ("waiting_for_choice".to_string(), "select_option".to_string(), 0.85),
            ("claims_completion".to_string(), "confirm_completion".to_string(), 0.90),
        ];
        let vars = PromptVariables::new().with_formatted_history(&records);
        assert!(vars.decision_history.contains("waiting_for_choice"));
        assert!(vars.decision_history.contains("select_option"));
        assert!(vars.decision_history.contains("0.85"));
    }

    #[test]
    fn test_prompt_variables_with_formatted_history_empty() {
        let records: Vec<(String, String, f64)> = vec![];
        let vars = PromptVariables::new().with_formatted_history(&records);
        assert_eq!(vars.decision_history, "No previous decisions.");
    }
}
