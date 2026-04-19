//! CLI decision engine
//!
//! Sprint 3.3: Uses independent CLI session for human input decisions.

use crate::model::action::DecisionAction;
use crate::model::action::action_registry::ActionRegistry;
use crate::core::context::DecisionContext;
use crate::engine::engine::{DecisionEngine, SessionHandle};
use crate::core::error::DecisionError;
use crate::core::output::DecisionOutput;
use crate::provider::provider_kind::ProviderKind;
use crate::model::situation::DecisionSituation;
use crate::core::types::DecisionEngineType;
use serde::{Deserialize, Serialize};
use std::path::Path;

/// CLI decision engine configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CLIEngineConfig {
    /// Timeout for human input (default: 60 seconds)
    pub timeout_seconds: u64,

    /// Enable prompt confirmation
    pub confirm_prompt: bool,

    /// Show reasoning before asking
    pub show_reasoning: bool,
}

impl Default for CLIEngineConfig {
    fn default() -> Self {
        Self {
            timeout_seconds: 60,
            confirm_prompt: true,
            show_reasoning: true,
        }
    }
}

/// CLI decision engine - prompts human for decisions via CLI
pub struct CLIDecisionEngine {
    /// Provider type for display
    provider: ProviderKind,

    /// Session handle
    session: Option<SessionHandle>,

    /// Engine configuration
    config: CLIEngineConfig,

    /// Healthy flag
    healthy: bool,

    /// Pending requests awaiting response
    pending_requests: Vec<PendingCLIRequest>,
}

/// Pending CLI request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PendingCLIRequest {
    /// Request ID
    pub id: String,

    /// Situation description
    pub situation_description: String,

    /// Available options
    pub options: Vec<CLIOption>,

    /// Timestamp
    pub timestamp: String,
}

/// CLI option for display
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CLIOption {
    /// Option ID
    pub id: String,

    /// Option label
    pub label: String,

    /// Option description
    pub description: String,
}

impl CLIDecisionEngine {
    /// Create new CLI decision engine
    pub fn new(provider: ProviderKind) -> Self {
        Self {
            provider,
            session: None,
            config: CLIEngineConfig::default(),
            healthy: true,
            pending_requests: Vec::new(),
        }
    }

    /// Create with configuration
    pub fn with_config(provider: ProviderKind, config: CLIEngineConfig) -> Self {
        Self {
            provider,
            session: None,
            config,
            healthy: true,
            pending_requests: Vec::new(),
        }
    }

    /// Create with session
    pub fn with_session(provider: ProviderKind, session: SessionHandle) -> Self {
        Self {
            provider,
            session: Some(session),
            config: CLIEngineConfig::default(),
            healthy: true,
            pending_requests: Vec::new(),
        }
    }

    /// Build prompt for human
    fn build_human_prompt(
        &self,
        context: &DecisionContext,
        action_registry: &ActionRegistry,
    ) -> String {
        let situation_text = context.trigger_situation.to_prompt_text();
        let available_actions = context.trigger_situation.available_actions();

        let action_labels: Vec<String> = available_actions
            .iter()
            .enumerate()
            .filter_map(|(i, action_type)| {
                action_registry
                    .get(&action_type)
                    .map(|action| format!("[{}] {}", i + 1, action.to_prompt_format()))
            })
            .collect();

        let task_info = context
            .current_task_id
            .as_ref()
            .map(|id| format!("Task: {}", id))
            .unwrap_or_else(|| "No task assigned".to_string());

        format!(
            "\n╔══════════════════════════════════════════════════╗\n\
             ║          DECISION REQUIRED                        ║\n\
             ╠══════════════════════════════════════════════════╣\n\
             ║ Agent: {}                                     ║\n\
             ║ {}                                            ║\n\
             ╠══════════════════════════════════════════════════╣\n\
             ║ Situation:                                        ║\n\
             ║ {}                                                ║\n\
             ╠══════════════════════════════════════════════════╣\n\
             ║ Available Actions:                                ║\n\
             ║ {}                                                ║\n\
             ╠══════════════════════════════════════════════════╣\n\
             ║ [Enter number] Select action                      ║\n\
             ║ [c] Custom instruction                            ║\n\
             ║ [s] Skip this decision                            ║\n\
             ║ [h] Request human intervention                    ║\n\
             ╚══════════════════════════════════════════════════╝\n\
             \n\
             Your choice: ",
            context.main_agent_id,
            task_info,
            situation_text,
            action_labels.join("\n║ ")
        )
    }

    /// Simulate CLI input (placeholder - actual implementation would read from stdin)
    fn get_cli_input(&mut self, _prompt: &str) -> crate::error::Result<String> {
        // This is a placeholder - in production, this would:
        // 1. Display prompt to console
        // 2. Read input with timeout
        // 3. Return user's choice

        // For testing, simulate default response based on situation
        // Default: select first available action
        Ok("1".to_string())
    }

    /// Parse CLI input to action
    fn parse_cli_input(
        &self,
        input: &str,
        situation: &dyn DecisionSituation,
        action_registry: &ActionRegistry,
    ) -> crate::error::Result<Vec<Box<dyn DecisionAction>>> {
        use crate::model::action::builtin_actions::{CustomInstructionAction, RequestHumanAction};

        let trimmed = input.trim().to_lowercase();
        let available_actions = situation.available_actions();

        // Handle special inputs
        if trimmed == "c" || trimmed.starts_with("c:") {
            let instruction = if trimmed.starts_with("c:") {
                trimmed[2..].trim().to_string()
            } else {
                "Custom instruction provided via CLI".to_string()
            };
            return Ok(vec![Box::new(CustomInstructionAction::new(instruction))]);
        }

        if trimmed == "s" {
            // Skip - return empty actions
            return Ok(Vec::new());
        }

        if trimmed == "h" {
            return Ok(vec![Box::new(RequestHumanAction::new("Requested via CLI"))]);
        }

        // Try parsing as number
        if let Ok(num) = trimmed.parse::<usize>() {
            if num > 0 && num <= available_actions.len() {
                let action_type = available_actions[num - 1].clone();
                if let Some(action) = action_registry.get(&action_type) {
                    return Ok(vec![action.clone_boxed()]);
                }
            }
        }

        // Fallback: try parsing directly
        for action_type in &available_actions {
            if let Some(action) = action_registry.parse(action_type.clone(), input) {
                return Ok(vec![action]);
            }
        }

        Err(DecisionError::ParseError(format!(
            "Invalid CLI input: {}",
            input
        )))
    }

    /// Add pending request
    pub fn add_pending_request(&mut self, request: PendingCLIRequest) {
        self.pending_requests.push(request);
    }

    /// Get pending requests
    pub fn pending_requests(&self) -> &[PendingCLIRequest] {
        &self.pending_requests
    }

    /// Clear pending request
    pub fn clear_pending_request(&mut self, id: &str) {
        self.pending_requests.retain(|r| r.id != id);
    }

    /// Persist session
    pub fn persist_session(&self, path: &Path) -> crate::error::Result<()> {
        let state = CLISessionState {
            provider: self.provider,
            config: self.config.clone(),
            pending_requests: self.pending_requests.clone(),
        };

        let json = serde_json::to_string_pretty(&state).map_err(DecisionError::JsonError)?;

        std::fs::write(path, json).map_err(|e| DecisionError::PersistenceError(e.to_string()))?;

        Ok(())
    }

    /// Restore session
    pub fn restore_session(&mut self, path: &Path) -> crate::error::Result<()> {
        if !path.exists() {
            return Ok(());
        }

        let json = std::fs::read_to_string(path)
            .map_err(|e| DecisionError::PersistenceError(e.to_string()))?;

        let state: CLISessionState =
            serde_json::from_str(&json).map_err(DecisionError::JsonError)?;

        self.provider = state.provider;
        self.config = state.config;
        self.pending_requests = state.pending_requests;

        Ok(())
    }
}

/// CLI session state for persistence
#[derive(Debug, Clone, Serialize, Deserialize)]
struct CLISessionState {
    provider: ProviderKind,
    config: CLIEngineConfig,
    pending_requests: Vec<PendingCLIRequest>,
}

impl DecisionEngine for CLIDecisionEngine {
    fn engine_type(&self) -> DecisionEngineType {
        DecisionEngineType::CLI {
            provider: self.provider,
        }
    }

    fn decide(
        &mut self,
        context: DecisionContext,
        action_registry: &ActionRegistry,
    ) -> crate::error::Result<DecisionOutput> {
        // 1. Build human prompt
        let prompt = self.build_human_prompt(&context, action_registry);

        // 2. Get CLI input
        let input = self.get_cli_input(&prompt)?;

        // 3. Parse to actions
        let actions =
            self.parse_cli_input(&input, context.trigger_situation.as_ref(), action_registry)?;

        // 4. Build reasoning
        let reasoning = format!("CLI input: {}", input);
        let confidence = if actions.is_empty() { 0.0 } else { 1.0 };

        Ok(DecisionOutput::new(actions, reasoning).with_confidence(confidence))
    }

    fn build_prompt(&self, context: &DecisionContext, action_registry: &ActionRegistry) -> String {
        self.build_human_prompt(context, action_registry)
    }

    fn parse_response(
        &self,
        response: &str,
        situation: &dyn DecisionSituation,
        action_registry: &ActionRegistry,
    ) -> crate::error::Result<Vec<Box<dyn DecisionAction>>> {
        self.parse_cli_input(response, situation, action_registry)
    }

    fn session_handle(&self) -> Option<&str> {
        self.session.as_ref().map(|s| s.session_id.as_str())
    }

    fn is_healthy(&self) -> bool {
        self.healthy
    }

    fn reset(&mut self) -> crate::error::Result<()> {
        self.pending_requests.clear();
        self.session = None;
        self.healthy = true;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::action::action_registry::ActionRegistry;
    use crate::model::action::builtin_actions::register_action_builtins;
    use crate::model::situation::builtin_situations::WaitingForChoiceSituation;
    use crate::core::context::DecisionContext;
    use crate::model::situation::ChoiceOption;
    use tempfile::TempDir;

    fn make_test_context() -> DecisionContext {
        let situation: Box<dyn crate::situation::DecisionSituation> =
            Box::new(WaitingForChoiceSituation::new(vec![
                ChoiceOption::new("A", "Option A"),
                ChoiceOption::new("B", "Option B"),
            ]));
        DecisionContext::new(situation, "test-agent")
    }

    fn make_test_registry() -> ActionRegistry {
        let registry = ActionRegistry::new();
        register_action_builtins(&registry);
        registry
    }

    #[test]
    fn test_cli_engine_new() {
        let engine = CLIDecisionEngine::new(ProviderKind::Claude);
        assert!(matches!(
            engine.engine_type(),
            DecisionEngineType::CLI { .. }
        ));
    }

    #[test]
    fn test_cli_engine_config_default() {
        let config = CLIEngineConfig::default();
        assert_eq!(config.timeout_seconds, 60);
        assert!(config.confirm_prompt);
    }

    #[test]
    fn test_cli_build_prompt() {
        let engine = CLIDecisionEngine::new(ProviderKind::Claude);
        let context = make_test_context();
        let registry = make_test_registry();

        let prompt = engine.build_prompt(&context, &registry);

        assert!(prompt.contains("DECISION REQUIRED"));
        assert!(prompt.contains("test-agent"));
    }

    #[test]
    fn test_cli_parse_input_number() {
        let engine = CLIDecisionEngine::new(ProviderKind::Claude);
        let registry = make_test_registry();
        let situation: Box<dyn crate::situation::DecisionSituation> = Box::new(
            WaitingForChoiceSituation::new(vec![ChoiceOption::new("A", "Option A")]),
        );

        let actions = engine
            .parse_cli_input("1", situation.as_ref(), &registry)
            .unwrap();
        assert_eq!(actions.len(), 1);
        assert_eq!(actions[0].action_type().name, "select_option");
    }

    #[test]
    fn test_cli_parse_input_custom() {
        let engine = CLIDecisionEngine::new(ProviderKind::Claude);
        let registry = make_test_registry();
        let situation: Box<dyn crate::situation::DecisionSituation> = Box::new(
            WaitingForChoiceSituation::new(vec![ChoiceOption::new("A", "Option A")]),
        );

        let actions = engine
            .parse_cli_input("c:do something", situation.as_ref(), &registry)
            .unwrap();
        assert_eq!(actions.len(), 1);
        assert_eq!(actions[0].action_type().name, "custom_instruction");
    }

    #[test]
    fn test_cli_parse_input_skip() {
        let engine = CLIDecisionEngine::new(ProviderKind::Claude);
        let registry = make_test_registry();
        let situation: Box<dyn crate::situation::DecisionSituation> = Box::new(
            WaitingForChoiceSituation::new(vec![ChoiceOption::new("A", "Option A")]),
        );

        let actions = engine
            .parse_cli_input("s", situation.as_ref(), &registry)
            .unwrap();
        assert!(actions.is_empty());
    }

    #[test]
    fn test_cli_parse_input_human() {
        let engine = CLIDecisionEngine::new(ProviderKind::Claude);
        let registry = make_test_registry();
        let situation: Box<dyn crate::situation::DecisionSituation> = Box::new(
            WaitingForChoiceSituation::new(vec![ChoiceOption::new("A", "Option A")]),
        );

        let actions = engine
            .parse_cli_input("h", situation.as_ref(), &registry)
            .unwrap();
        assert_eq!(actions.len(), 1);
        assert_eq!(actions[0].action_type().name, "request_human");
    }

    #[test]
    fn test_cli_decide() {
        let mut engine = CLIDecisionEngine::new(ProviderKind::Claude);
        let context = make_test_context();
        let registry = make_test_registry();

        let output = engine.decide(context, &registry).unwrap();
        assert!(output.has_actions());
    }

    #[test]
    fn test_cli_pending_request() {
        let mut engine = CLIDecisionEngine::new(ProviderKind::Claude);

        let request = PendingCLIRequest {
            id: "req-001".to_string(),
            situation_description: "Test situation".to_string(),
            options: vec![CLIOption {
                id: "A".to_string(),
                label: "Option A".to_string(),
                description: "First option".to_string(),
            }],
            timestamp: "2026-04-15T10:00:00Z".to_string(),
        };

        engine.add_pending_request(request);
        assert_eq!(engine.pending_requests().len(), 1);

        engine.clear_pending_request("req-001");
        assert!(engine.pending_requests().is_empty());
    }

    #[test]
    fn test_cli_session_persistence() {
        let mut engine = CLIDecisionEngine::new(ProviderKind::Claude);
        engine.add_pending_request(PendingCLIRequest {
            id: "req-001".to_string(),
            situation_description: "Test".to_string(),
            options: vec![],
            timestamp: "2026-04-15T10:00:00Z".to_string(),
        });

        let temp = TempDir::new().unwrap();
        let path = temp.path().join("session.json");

        engine.persist_session(&path).unwrap();

        let mut restored = CLIDecisionEngine::new(ProviderKind::Claude);
        restored.restore_session(&path).unwrap();

        assert_eq!(restored.pending_requests().len(), 1);
    }

    #[test]
    fn test_cli_reset() {
        let mut engine = CLIDecisionEngine::new(ProviderKind::Claude);
        engine.add_pending_request(PendingCLIRequest {
            id: "req-001".to_string(),
            situation_description: "Test".to_string(),
            options: vec![],
            timestamp: "2026-04-15T10:00:00Z".to_string(),
        });

        engine.reset().unwrap();
        assert!(engine.pending_requests().is_empty());
    }

    #[test]
    fn test_cli_config_serde() {
        let config = CLIEngineConfig {
            timeout_seconds: 120,
            confirm_prompt: false,
            show_reasoning: false,
        };

        let json = serde_json::to_string(&config).unwrap();
        let parsed: CLIEngineConfig = serde_json::from_str(&json).unwrap();

        assert_eq!(config.timeout_seconds, parsed.timeout_seconds);
        assert_eq!(config.confirm_prompt, parsed.confirm_prompt);
    }

    #[test]
    fn test_cli_option() {
        let option = CLIOption {
            id: "A".to_string(),
            label: "Option A".to_string(),
            description: "First option".to_string(),
        };

        assert_eq!(option.id, "A");
        assert_eq!(option.label, "Option A");
    }
}
