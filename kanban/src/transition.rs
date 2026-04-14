//! Transition rules and registry for status transitions
//!
//! Extensible status transition rules replacing hardcoded valid_transitions().

use crate::types::StatusType;
use std::sync::RwLock;

/// TransitionRule trait - extensible status transition validation
///
/// Replaces hardcoded Status::valid_transitions() method,
/// enabling custom transition rules without modifying core code.
pub trait TransitionRule: Send + Sync + 'static {
    /// Get the from status type
    fn from_status(&self) -> StatusType;

    /// Get the to status type
    fn to_status(&self) -> StatusType;

    /// Check if transition is valid for given element context (optional)
    fn is_valid(&self) -> bool {
        true
    }

    /// Clone into boxed trait object
    fn clone_boxed(&self) -> Box<dyn TransitionRule>;
}

/// BuiltinTransitionRule - simple rule for predefined transitions
pub struct BuiltinTransitionRule {
    from: StatusType,
    to: StatusType,
}

impl BuiltinTransitionRule {
    pub fn new(from: StatusType, to: StatusType) -> Self {
        Self { from, to }
    }
}

impl TransitionRule for BuiltinTransitionRule {
    fn from_status(&self) -> StatusType {
        self.from.clone()
    }

    fn to_status(&self) -> StatusType {
        self.to.clone()
    }

    fn clone_boxed(&self) -> Box<dyn TransitionRule> {
        Box::new(BuiltinTransitionRule {
            from: self.from.clone(),
            to: self.to.clone(),
        })
    }
}

/// TransitionRegistry - thread-safe registry for transition rules
///
/// Uses RwLock for concurrent registration and query.
pub struct TransitionRegistry {
    rules: RwLock<Vec<Box<dyn TransitionRule>>>,
}

impl TransitionRegistry {
    /// Create a new empty registry
    pub fn new() -> Self {
        Self {
            rules: RwLock::new(Vec::new()),
        }
    }

    /// Register a transition rule (thread-safe)
    pub fn register(&self, rule: Box<dyn TransitionRule>) {
        self.rules.write().unwrap().push(rule);
    }

    /// Register all builtin transition rules
    pub fn register_builtin_rules(&self) {
        // Plan -> Backlog
        self.register(Box::new(BuiltinTransitionRule::new(
            StatusType::new("plan"),
            StatusType::new("backlog"),
        )));

        // Backlog -> Blocked, Ready, Todo, Plan
        self.register(Box::new(BuiltinTransitionRule::new(
            StatusType::new("backlog"),
            StatusType::new("blocked"),
        )));
        self.register(Box::new(BuiltinTransitionRule::new(
            StatusType::new("backlog"),
            StatusType::new("ready"),
        )));
        self.register(Box::new(BuiltinTransitionRule::new(
            StatusType::new("backlog"),
            StatusType::new("todo"),
        )));
        self.register(Box::new(BuiltinTransitionRule::new(
            StatusType::new("backlog"),
            StatusType::new("plan"),
        )));

        // Blocked -> Backlog
        self.register(Box::new(BuiltinTransitionRule::new(
            StatusType::new("blocked"),
            StatusType::new("backlog"),
        )));

        // Ready -> Todo, Backlog
        self.register(Box::new(BuiltinTransitionRule::new(
            StatusType::new("ready"),
            StatusType::new("todo"),
        )));
        self.register(Box::new(BuiltinTransitionRule::new(
            StatusType::new("ready"),
            StatusType::new("backlog"),
        )));

        // Todo -> InProgress, Ready
        self.register(Box::new(BuiltinTransitionRule::new(
            StatusType::new("todo"),
            StatusType::new("in_progress"),
        )));
        self.register(Box::new(BuiltinTransitionRule::new(
            StatusType::new("todo"),
            StatusType::new("ready"),
        )));

        // InProgress -> Done, Todo
        self.register(Box::new(BuiltinTransitionRule::new(
            StatusType::new("in_progress"),
            StatusType::new("done"),
        )));
        self.register(Box::new(BuiltinTransitionRule::new(
            StatusType::new("in_progress"),
            StatusType::new("todo"),
        )));

        // Done -> Verified, Todo
        self.register(Box::new(BuiltinTransitionRule::new(
            StatusType::new("done"),
            StatusType::new("verified"),
        )));
        self.register(Box::new(BuiltinTransitionRule::new(
            StatusType::new("done"),
            StatusType::new("todo"),
        )));
    }

    /// Check if a transition is valid (thread-safe)
    pub fn can_transition(&self, from: &StatusType, to: &StatusType) -> bool {
        let rules = self.rules.read().unwrap();
        rules.iter().any(|r| {
            r.from_status().name() == from.name() && r.to_status().name() == to.name()
        })
    }

    /// Get all valid transitions from a status (thread-safe)
    pub fn valid_transitions(&self, from: &StatusType) -> Vec<StatusType> {
        let rules = self.rules.read().unwrap();
        rules
            .iter()
            .filter(|r| r.from_status().name() == from.name())
            .map(|r| r.to_status())
            .collect()
    }

    /// Check if registry is empty
    pub fn is_empty(&self) -> bool {
        self.rules.read().unwrap().is_empty()
    }

    /// Get the number of registered rules
    pub fn len(&self) -> usize {
        self.rules.read().unwrap().len()
    }
}

impl Default for TransitionRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_builtin_rule() {
        let rule = BuiltinTransitionRule::new(StatusType::new("plan"), StatusType::new("backlog"));
        assert_eq!(rule.from_status().name(), "plan");
        assert_eq!(rule.to_status().name(), "backlog");
    }

    #[test]
    fn test_registry_builtin_rules() {
        let registry = TransitionRegistry::new();
        registry.register_builtin_rules();
        assert!(registry.can_transition(&StatusType::new("plan"), &StatusType::new("backlog")));
        assert!(!registry.can_transition(&StatusType::new("plan"), &StatusType::new("done")));
    }
}