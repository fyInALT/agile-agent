use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::ext::blackboard::BlackboardValue;

/// Evaluator placeholder — full implementation in Sprint 3.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Evaluator {
    OutputContains {
        pattern: String,
        #[serde(default, rename = "caseSensitive")]
        case_sensitive: bool,
    },
    SituationIs {
        #[serde(rename = "situationType")]
        situation_type: String,
    },
    ReflectionRoundUnder {
        max: u8,
    },
    VariableIs {
        key: String,
        expected: BlackboardValue,
    },
    RegexMatch {
        pattern: String,
    },
    Script {
        expression: String,
    },
    Or {
        conditions: Vec<Evaluator>,
    },
    And {
        conditions: Vec<Evaluator>,
    },
    Not {
        condition: Box<Evaluator>,
    },
    Custom {
        name: String,
        params: HashMap<String, BlackboardValue>,
    },
}

/// EvaluatorRegistry placeholder — full implementation in Sprint 3.
#[derive(Debug, Clone, Default)]
pub struct EvaluatorRegistry;

impl EvaluatorRegistry {
    pub fn new() -> Self {
        Self
    }
}
