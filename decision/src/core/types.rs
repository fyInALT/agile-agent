//! Core type identifiers for decision layer

use serde::{Deserialize, Serialize};
use std::fmt;

/// Situation type identifier - extensible string-based
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SituationType {
    /// Type name (e.g., "waiting_for_choice", "claude_finished")
    pub name: String,

    /// Optional subtype for provider-specific variants
    pub subtype: Option<String>,
}

impl SituationType {
    /// Create a new situation type
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            subtype: None,
        }
    }

    /// Create a situation type with subtype
    pub fn with_subtype(name: impl Into<String>, subtype: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            subtype: Some(subtype.into()),
        }
    }

    /// Get the base type (without subtype)
    pub fn base_type(&self) -> SituationType {
        if self.subtype.is_some() {
            SituationType::new(&self.name)
        } else {
            self.clone()
        }
    }

    /// Check if this type matches (exact or base)
    pub fn matches(&self, other: &SituationType) -> bool {
        self == other || self.base_type() == other.base_type()
    }
}

impl fmt::Display for SituationType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.subtype {
            Some(subtype) => write!(f, "{}.{}", self.name, subtype),
            None => write!(f, "{}", self.name),
        }
    }
}

impl Default for SituationType {
    fn default() -> Self {
        Self::new("unknown")
    }
}

/// Action type identifier
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ActionType {
    pub name: String,
}

impl ActionType {
    pub fn new(name: impl Into<String>) -> Self {
        Self { name: name.into() }
    }
}

impl fmt::Display for ActionType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.name)
    }
}

impl Default for ActionType {
    fn default() -> Self {
        Self::new("unknown")
    }
}

/// Urgency level for human intervention
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum UrgencyLevel {
    #[default]
    Low,
    Medium,
    High,
    Critical,
}

impl fmt::Display for UrgencyLevel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            UrgencyLevel::Low => write!(f, "low"),
            UrgencyLevel::Medium => write!(f, "medium"),
            UrgencyLevel::High => write!(f, "high"),
            UrgencyLevel::Critical => write!(f, "critical"),
        }
    }
}

/// Decision engine type identifier
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum DecisionEngineType {
    /// LLM-based engine with provider
    LLM {
        provider: crate::provider::ProviderKind,
    },
    /// CLI-based engine with provider
    CLI {
        provider: crate::provider::ProviderKind,
    },
    /// Rule-based engine
    RuleBased,
    /// Mock engine for testing
    Mock,
    /// Custom engine
    Custom { name: String },
}

impl DecisionEngineType {
    /// Check if this is an LLM engine
    pub fn is_llm(&self) -> bool {
        matches!(self, DecisionEngineType::LLM { .. })
    }

    /// Check if this is a CLI engine
    pub fn is_cli(&self) -> bool {
        matches!(self, DecisionEngineType::CLI { .. })
    }

    /// Check if this is a mock engine
    pub fn is_mock(&self) -> bool {
        matches!(self, DecisionEngineType::Mock)
    }
}

impl fmt::Display for DecisionEngineType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DecisionEngineType::LLM { provider } => write!(f, "llm:{}", provider),
            DecisionEngineType::CLI { provider } => write!(f, "cli:{}", provider),
            DecisionEngineType::RuleBased => write!(f, "rule_based"),
            DecisionEngineType::Mock => write!(f, "mock"),
            DecisionEngineType::Custom { name } => write!(f, "custom:{}", name),
        }
    }
}

/// Generate a unique ID with prefix
pub fn generate_id(prefix: &str) -> String {
    format!("{}-{}", prefix, chrono::Utc::now().timestamp_millis())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_situation_type_creation() {
        let st = SituationType::new("waiting_for_choice");
        assert_eq!(st.name, "waiting_for_choice");
        assert_eq!(st.subtype, None);
    }

    #[test]
    fn test_situation_type_with_subtype() {
        let st = SituationType::with_subtype("finished", "claude");
        assert_eq!(st.name, "finished");
        assert_eq!(st.subtype, Some("claude".to_string()));
    }

    #[test]
    fn test_situation_type_display() {
        let st1 = SituationType::new("error");
        assert_eq!(format!("{}", st1), "error");

        let st2 = SituationType::with_subtype("waiting_for_choice", "codex");
        assert_eq!(format!("{}", st2), "waiting_for_choice.codex");
    }

    #[test]
    fn test_situation_type_base_type() {
        let st = SituationType::with_subtype("waiting_for_choice", "codex");
        let base = st.base_type();
        assert_eq!(base.name, "waiting_for_choice");
        assert_eq!(base.subtype, None);
    }

    #[test]
    fn test_situation_type_matches() {
        let st1 = SituationType::with_subtype("waiting_for_choice", "codex");
        let st2 = SituationType::new("waiting_for_choice");
        let st3 = SituationType::new("error");

        assert!(st1.matches(&st2));
        assert!(st2.matches(&st1));
        assert!(!st1.matches(&st3));
    }

    #[test]
    fn test_action_type_creation() {
        let at = ActionType::new("select_option");
        assert_eq!(at.name, "select_option");
    }

    #[test]
    fn test_urgency_level_default() {
        let u = UrgencyLevel::default();
        assert_eq!(u, UrgencyLevel::Low);
    }

    #[test]
    fn test_urgency_level_display() {
        assert_eq!(format!("{}", UrgencyLevel::Low), "low");
        assert_eq!(format!("{}", UrgencyLevel::Critical), "critical");
    }

    #[test]
    fn test_situation_type_serde() {
        let st = SituationType::with_subtype("finished", "claude");
        let json = serde_json::to_string(&st).unwrap();
        let parsed: SituationType = serde_json::from_str(&json).unwrap();
        assert_eq!(st, parsed);
    }

    #[test]
    fn test_action_type_serde() {
        let at = ActionType::new("select_option");
        let json = serde_json::to_string(&at).unwrap();
        let parsed: ActionType = serde_json::from_str(&json).unwrap();
        assert_eq!(at, parsed);
    }
}
