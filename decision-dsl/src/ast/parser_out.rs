use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::ext::blackboard::BlackboardValue;
use crate::ext::command::DecisionCommand;
use crate::ext::error::{ParseError, RuntimeError};

/// OutputParser enum with all built-in variants.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "payload")]
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

impl OutputParser {
    /// Parse raw LLM output into a map of blackboard values.
    pub fn parse(&self, raw: &str) -> Result<HashMap<String, BlackboardValue>, RuntimeError> {
        match self {
            OutputParser::Enum {
                values,
                case_sensitive,
            } => {
                let trimmed = raw.trim();
                let matched = if *case_sensitive {
                    values.iter().any(|v| v == trimmed)
                } else {
                    let lower = trimmed.to_lowercase();
                    values.iter().any(|v| v.to_lowercase() == lower)
                };
                match matched {
                    true => {
                        let mut map = HashMap::new();
                        map.insert("value".into(), BlackboardValue::String(trimmed.into()));
                        Ok(map)
                    }
                    false => Err(RuntimeError::FilterError(format!(
                        "input '{}' did not match any enum value",
                        trimmed
                    ))),
                }
            }
            OutputParser::Structured { pattern, fields } => {
                let re = Regex::new(pattern).map_err(|e| {
                    RuntimeError::FilterError(format!("invalid regex pattern: {e}"))
                })?;
                let caps = re.captures(raw).ok_or_else(|| {
                    RuntimeError::FilterError(
                        "input did not match structured pattern".into(),
                    )
                })?;
                let mut map = HashMap::new();
                for field in fields {
                    let group_val = caps.get(field.group).ok_or_else(|| {
                        RuntimeError::FilterError(format!(
                            "capture group {} not found for field '{}'",
                            field.group, field.name
                        ))
                    })?;
                    let value = match field.field_type {
                        FieldType::String => {
                            BlackboardValue::String(group_val.as_str().into())
                        }
                        FieldType::Integer => {
                            let s = group_val.as_str();
                            let n = s.parse::<i64>().map_err(|_| {
                                RuntimeError::FilterError(format!(
                                    "cannot parse integer from '{}' for field '{}'",
                                    s, field.name
                                ))
                            })?;
                            BlackboardValue::Integer(n)
                        }
                        FieldType::Float => {
                            let s = group_val.as_str();
                            let n = s.parse::<f64>().map_err(|_| {
                                RuntimeError::FilterError(format!(
                                    "cannot parse float from '{}' for field '{}'",
                                    s, field.name
                                ))
                            })?;
                            BlackboardValue::Float(n)
                        }
                        FieldType::Boolean => {
                            let s = group_val.as_str();
                            let b = s.parse::<bool>().map_err(|_| {
                                RuntimeError::FilterError(format!(
                                    "cannot parse boolean from '{}' for field '{}'",
                                    s, field.name
                                ))
                            })?;
                            BlackboardValue::Boolean(b)
                        }
                    };
                    map.insert(field.name.clone(), value);
                }
                Ok(map)
            }
            OutputParser::Json { schema: _ } => {
                let json_val: serde_json::Value =
                    serde_json::from_str(raw).map_err(|e| {
                        RuntimeError::FilterError(format!("invalid JSON: {e}"))
                    })?;
                let map = json_value_to_blackboard_map(json_val)?;
                Ok(map)
            }
            OutputParser::Command { mapping } => {
                let trimmed = raw.trim();
                let cmd = mapping.get(trimmed).ok_or_else(|| {
                    RuntimeError::FilterError(format!(
                        "unknown command key '{}'",
                        trimmed
                    ))
                })?;
                let mut map = HashMap::new();
                map.insert("__command".into(), BlackboardValue::Command(cmd.clone()));
                Ok(map)
            }
            OutputParser::Custom { name, .. } => Err(RuntimeError::FilterError(format!(
                "custom parser '{}' cannot be evaluated without registry",
                name
            ))),
        }
    }
}

/// Convert a serde_json::Value into a HashMap<String, BlackboardValue>.
/// Expects the top-level value to be a JSON object.
fn json_value_to_blackboard_map(
    value: serde_json::Value,
) -> Result<HashMap<String, BlackboardValue>, RuntimeError> {
    match value {
        serde_json::Value::Object(obj) => {
            let mut map = HashMap::new();
            for (k, v) in obj {
                map.insert(k, json_to_blackboard(v));
            }
            Ok(map)
        }
        _ => Err(RuntimeError::FilterError(
            "JSON parser expects a top-level object".into(),
        )),
    }
}

fn json_to_blackboard(value: serde_json::Value) -> BlackboardValue {
    match value {
        serde_json::Value::String(s) => BlackboardValue::String(s),
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                BlackboardValue::Integer(i)
            } else {
                BlackboardValue::Float(n.as_f64().unwrap_or(0.0))
            }
        }
        serde_json::Value::Bool(b) => BlackboardValue::Boolean(b),
        serde_json::Value::Array(arr) => {
            BlackboardValue::List(arr.into_iter().map(json_to_blackboard).collect())
        }
        serde_json::Value::Object(obj) => {
            let mut map = HashMap::new();
            for (k, v) in obj {
                map.insert(k, json_to_blackboard(v));
            }
            BlackboardValue::Map(map)
        }
        serde_json::Value::Null => BlackboardValue::String("".into()),
    }
}

// ── OutputParserRegistry ────────────────────────────────────────────────────

pub struct OutputParserRegistry {
    creators: HashMap<
        String,
        Box<
            dyn Fn(&HashMap<String, BlackboardValue>) -> Result<OutputParser, ParseError>
                + Send
                + Sync,
        >,
    >,
}

impl Default for OutputParserRegistry {
    fn default() -> Self {
        Self::with_builtins()
    }
}

impl OutputParserRegistry {
    pub fn new() -> Self {
        Self::with_builtins()
    }

    pub fn with_builtins() -> Self {
        let mut reg = Self {
            creators: HashMap::new(),
        };

        reg.register("Enum", |params| {
            let values = params
                .get("values")
                .and_then(|v| match v {
                    BlackboardValue::List(l) => Some(
                        l.iter()
                            .filter_map(|item| match item {
                                BlackboardValue::String(s) => Some(s.clone()),
                                _ => None,
                            })
                            .collect::<Vec<_>>(),
                    ),
                    _ => None,
                })
                .unwrap_or_default();
            let case_sensitive = params
                .get("caseSensitive")
                .and_then(|v| match v {
                    BlackboardValue::Boolean(b) => Some(*b),
                    _ => None,
                })
                .unwrap_or(true);
            Ok(OutputParser::Enum {
                values,
                case_sensitive,
            })
        });

        reg.register("Structured", |params| {
            let pattern = params
                .get("pattern")
                .and_then(|v| match v {
                    BlackboardValue::String(s) => Some(s.clone()),
                    _ => None,
                })
                .unwrap_or_default();
            let fields = params
                .get("fields")
                .and_then(|v| match v {
                    BlackboardValue::List(l) => Some(
                        l.iter()
                            .filter_map(|item| match item {
                                BlackboardValue::Map(m) => {
                                    let name = m
                                        .get("name")
                                        .and_then(|v| match v {
                                            BlackboardValue::String(s) => Some(s.clone()),
                                            _ => None,
                                        })
                                        .unwrap_or_default();
                                    let group = m
                                        .get("group")
                                        .and_then(|v| match v {
                                            BlackboardValue::Integer(i) => {
                                                Some(*i as usize)
                                            }
                                            _ => None,
                                        })
                                        .unwrap_or(0);
                                    let field_type = m
                                        .get("type")
                                        .and_then(|v| match v {
                                            BlackboardValue::String(s) => {
                                                match s.as_str() {
                                                    "string" | "String" => {
                                                        Some(FieldType::String)
                                                    }
                                                    "integer" | "Integer" => {
                                                        Some(FieldType::Integer)
                                                    }
                                                    "float" | "Float" => Some(FieldType::Float),
                                                    "boolean" | "Boolean" => {
                                                        Some(FieldType::Boolean)
                                                    }
                                                    _ => None,
                                                }
                                            }
                                            _ => None,
                                        })
                                        .unwrap_or(FieldType::String);
                                    Some(StructuredField {
                                        name,
                                        field_type,
                                        group,
                                    })
                                }
                                _ => None,
                            })
                            .collect::<Vec<_>>(),
                    ),
                    _ => None,
                })
                .unwrap_or_default();
            Ok(OutputParser::Structured { pattern, fields })
        });

        reg.register("Json", |_params| Ok(OutputParser::Json { schema: None }));

        reg.register("Command", |params| {
            // For builtin creation, we expect an empty or minimal params.
            // Real command mappings come from deserialized YAML.
            let mapping = params
                .get("mapping")
                .and_then(|v| match v {
                    BlackboardValue::Map(_m) => {
                        // We can't deserialize DecisionCommand from BlackboardValue::Map
                        // without serde, so this path is limited.
                        Some(HashMap::new())
                    }
                    _ => None,
                })
                .unwrap_or_default();
            Ok(OutputParser::Command { mapping })
        });

        reg
    }

    pub fn register<F>(&mut self, name: &str, creator: F)
    where
        F: Fn(&HashMap<String, BlackboardValue>) -> Result<OutputParser, ParseError>
            + Send
            + Sync
            + 'static,
    {
        self.creators
            .insert(name.to_string(), Box::new(creator));
    }

    pub fn create(
        &self,
        name: &str,
        params: &[(String, BlackboardValue)],
    ) -> Result<OutputParser, ParseError> {
        let creator = self.creators.get(name).ok_or_else(|| {
            ParseError::UnknownParserKind {
                kind: name.into(),
            }
        })?;
        let param_map: HashMap<String, BlackboardValue> = params.iter().cloned().collect();
        creator(&param_map)
    }
}
