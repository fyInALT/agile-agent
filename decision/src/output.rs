//! Decision output - action sequence

use crate::action::DecisionAction;
use crate::action_registry::ActionRegistry;
use crate::types::{ActionType, DecisionEngineType, SituationType};
use serde::{Deserialize, Serialize};

/// Decision output - sequence of actions
pub struct DecisionOutput {
    /// Actions to execute
    pub actions: Vec<Box<dyn DecisionAction>>,

    /// Reasoning for the decision
    pub reasoning: String,

    /// Confidence level (0.0-1.0)
    pub confidence: f64,

    /// Whether human was requested
    pub human_requested: bool,
}

impl DecisionOutput {
    pub fn new(actions: Vec<Box<dyn DecisionAction>>, reasoning: impl Into<String>) -> Self {
        Self {
            actions,
            reasoning: reasoning.into(),
            confidence: 0.8,
            human_requested: false,
        }
    }

    pub fn with_confidence(self, confidence: f64) -> Self {
        Self { confidence, ..self }
    }

    pub fn with_human_requested(self) -> Self {
        Self {
            human_requested: true,
            ..self
        }
    }

    /// Check if output has actions
    pub fn has_actions(&self) -> bool {
        !self.actions.is_empty()
    }

    /// Get first action type
    pub fn first_action_type(&self) -> Option<ActionType> {
        self.actions.first().map(|a| a.action_type())
    }

    /// Serialize to serde format for persistence
    pub fn to_serde(&self) -> DecisionOutputSerde {
        DecisionOutputSerde {
            action_types: self.actions.iter().map(|a| a.action_type().name).collect(),
            action_params: self.actions.iter().map(|a| a.serialize_params()).collect(),
            reasoning: self.reasoning.clone(),
            confidence: self.confidence,
            human_requested: self.human_requested,
        }
    }

    /// Deserialize from serde format using registry
    pub fn from_serde(serde: DecisionOutputSerde, registry: &ActionRegistry) -> Option<Self> {
        let actions: Vec<Box<dyn DecisionAction>> = serde
            .action_types
            .iter()
            .zip(serde.action_params.iter())
            .filter_map(|(type_name, params)| {
                let action_type = ActionType::new(type_name);
                registry.deserialize(&action_type, params)
            })
            .collect();

        if actions.len() != serde.action_types.len() {
            return None; // Failed to deserialize some actions
        }

        Some(Self {
            actions,
            reasoning: serde.reasoning,
            confidence: serde.confidence,
            human_requested: serde.human_requested,
        })
    }
}

/// Decision output serde format (for persistence)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecisionOutputSerde {
    /// Action types
    pub action_types: Vec<String>,

    /// Serialized action parameters
    pub action_params: Vec<String>,

    /// Reasoning
    pub reasoning: String,

    /// Confidence
    pub confidence: f64,

    /// Human requested
    pub human_requested: bool,
}

/// Decision record - history entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecisionRecord {
    pub decision_id: String,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub situation_type: SituationType,
    pub action_types: Vec<ActionType>,
    pub reasoning: String,
    pub confidence: f64,
    pub engine_type: DecisionEngineType,
}

impl DecisionRecord {
    pub fn new(
        decision_id: impl Into<String>,
        situation_type: SituationType,
        output: &DecisionOutput,
        engine_type: DecisionEngineType,
    ) -> Self {
        Self {
            decision_id: decision_id.into(),
            timestamp: chrono::Utc::now(),
            situation_type,
            action_types: output.actions.iter().map(|a| a.action_type()).collect(),
            reasoning: output.reasoning.clone(),
            confidence: output.confidence,
            engine_type,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::builtin_actions::{register_action_builtins, SelectOptionAction};

    #[test]
    fn test_decision_output_new() {
        let output = DecisionOutput::new(
            vec![Box::new(SelectOptionAction::new("A", "test"))],
            "Selected option A",
        );
        assert!(output.has_actions());
        assert_eq!(output.reasoning, "Selected option A");
    }

    #[test]
    fn test_decision_output_with_confidence() {
        let output = DecisionOutput::new(vec![], "test").with_confidence(0.95);
        assert_eq!(output.confidence, 0.95);
    }

    #[test]
    fn test_decision_output_with_human_requested() {
        let output = DecisionOutput::new(vec![], "test").with_human_requested();
        assert!(output.human_requested);
    }

    #[test]
    fn test_decision_output_first_action_type() {
        let output = DecisionOutput::new(
            vec![Box::new(SelectOptionAction::new("A", "test"))],
            "test",
        );
        assert_eq!(
            output.first_action_type(),
            Some(ActionType::new("select_option"))
        );
    }

    #[test]
    fn test_decision_output_no_actions() {
        let output = DecisionOutput::new(vec![], "test");
        assert!(!output.has_actions());
        assert_eq!(output.first_action_type(), None);
    }

    #[test]
    fn test_decision_output_serde_roundtrip() {
        let registry = ActionRegistry::new();
        register_action_builtins(&registry);

        let output = DecisionOutput::new(
            vec![Box::new(SelectOptionAction::new("A", "reason"))],
            "Selected A",
        ).with_confidence(0.9);

        let serde = output.to_serde();
        let restored = DecisionOutput::from_serde(serde, &registry);

        assert!(restored.is_some());
        let restored = restored.unwrap();
        assert_eq!(output.reasoning, restored.reasoning);
        assert_eq!(output.confidence, restored.confidence);
        assert_eq!(output.human_requested, restored.human_requested);
    }

    #[test]
    fn test_decision_output_serde_json() {
        let serde = DecisionOutputSerde {
            action_types: vec!["select_option".to_string()],
            action_params: vec![serde_json::to_string(&SelectOptionAction::new("A", "r")).unwrap()],
            reasoning: "test".to_string(),
            confidence: 0.8,
            human_requested: false,
        };

        let json = serde_json::to_string(&serde).unwrap();
        let parsed: DecisionOutputSerde = serde_json::from_str(&json).unwrap();
        assert_eq!(serde.action_types, parsed.action_types);
    }

    #[test]
    fn test_decision_record_new() {
        let output = DecisionOutput::new(
            vec![Box::new(SelectOptionAction::new("A", "test"))],
            "test",
        );
        let record = DecisionRecord::new(
            "dec-1",
            SituationType::new("waiting_for_choice"),
            &output,
            DecisionEngineType::Mock,
        );

        assert_eq!(record.decision_id, "dec-1");
        assert_eq!(record.situation_type, SituationType::new("waiting_for_choice"));
        assert!(record.engine_type.is_mock());
    }
}