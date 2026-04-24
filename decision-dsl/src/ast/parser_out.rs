use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::ext::blackboard::BlackboardValue;
use crate::ext::command::DecisionCommand;

/// OutputParser placeholder — full implementation in Sprint 3.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum OutputParser {
    Enum {
        values: Vec<String>,
        #[serde(default, rename = "caseSensitive")]
        case_sensitive: bool,
    },
    Structured {
        pattern: String,
        fields: Vec<StructuredField>,
    },
    Json {
        schema: Option<serde_json::Value>,
    },
    Command {
        mapping: HashMap<String, DecisionCommand>,
    },
    Custom {
        name: String,
        params: HashMap<String, BlackboardValue>,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StructuredField {
    pub name: String,
    #[serde(rename = "type")]
    pub field_type: FieldType,
    pub group: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum FieldType {
    String,
    Integer,
    Float,
    Boolean,
}

/// OutputParserRegistry placeholder — full implementation in Sprint 3.
#[derive(Debug, Clone, Default)]
pub struct OutputParserRegistry;

impl OutputParserRegistry {
    pub fn new() -> Self {
        Self
    }
}
