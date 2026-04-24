# Decision DSL: Evaluators & Output Parsers

> Expression evaluation and output parsing specification for the decision DSL engine. Condition evaluators (used by Condition nodes, `When` guards, and `if` fields) and output parsers (used by Prompt nodes and `Switch` prompts) are defined as **enums** — no trait objects, no heap allocations.

---

## Evaluator Enum

Condition nodes, `When` guards, and rule `if` fields all use the same `Evaluator` enum.

```rust
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub(crate) enum Evaluator {
    OutputContains {
        pattern: String,
        #[serde(default)]
        case_sensitive: bool,
    },
    SituationIs {
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
        #[serde(with = "serde_regex")]
        pattern: Regex,
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
    /// Extension point: host-registered custom evaluators.
    Custom {
        name: String,
        params: HashMap<String, BlackboardValue>,
    },
}

impl Evaluator {
    pub fn evaluate(&self, bb: &Blackboard) -> Result<bool, RuntimeError> {
        match self {
            Self::OutputContains { pattern, case_sensitive } => {
                let output = bb.provider_output.as_str();
                Ok(if *case_sensitive {
                    output.contains(pattern.as_str())
                } else {
                    output.to_lowercase().contains(&pattern.to_lowercase())
                })
            }
            Self::SituationIs { situation_type } => {
                let output = bb.provider_output.as_str();
                let summary = bb.context_summary.as_str();
                Ok(output.contains(situation_type.as_str())
                    || summary.contains(situation_type.as_str()))
            }
            Self::ReflectionRoundUnder { max } => {
                Ok(bb.reflection_round < *max)
            }
            Self::VariableIs { key, expected } => {
                match bb.get_path(key) {
                    Some(value) => Ok(value == *expected),
                    None => Ok(false),
                }
            }
            Self::RegexMatch { pattern } => {
                Ok(pattern.is_match(&bb.provider_output))
            }
            Self::Script { expression } => {
                evaluate_minimal_script(expression, bb)
            }
            Self::Or { conditions } => {
                for cond in conditions {
                    if cond.evaluate(bb)? {
                        return Ok(true);
                    }
                }
                Ok(false)
            }
            Self::And { conditions } => {
                for cond in conditions {
                    if !cond.evaluate(bb)? {
                        return Ok(false);
                    }
                }
                Ok(true)
            }
            Self::Not { condition } => {
                Ok(!condition.evaluate(bb)?)
            }
            Self::Custom { name, params } => {
                // Delegated to host-provided evaluator
                EVALUATOR_DISPATCH.with(|dispatch| {
                    dispatch.evaluate(name, params, bb)
                })
            }
        }
    }
}
```

### Benefits Over Trait Objects

| Aspect | `Box<dyn Evaluator>` (old) | `Evaluator` enum (new) |
|--------|---------------------------|----------------------|
| Allocation | Heap allocation per evaluator | Stack-allocated |
| Clone | Requires `dyn_clone` crate | Derives `Clone` |
| Debug | `{:?}` shows type name only | `{:?}` shows all fields |
| Serialization | Manual impl | `#[derive(Serialize, Deserialize)]` |
| Pattern matching | Not possible | Exhaustive match |
| Custom evaluators | `Box<dyn Evaluator>` from registry | `Custom { name, params }` variant + dispatch |

### Built-in Evaluators

#### outputContains

```yaml
eval:
  kind: outputContains
  pattern: "429"
  caseSensitive: false        # Optional, default: false
```

Checks if `provider_output` contains the pattern string.

#### situationIs

```yaml
eval:
  kind: situationIs
  type: claims_completion
```

Checks if the situation type appears in `provider_output` or `context_summary`.

#### reflectionRoundUnder

```yaml
eval:
  kind: reflectionRoundUnder
  max: 2
```

Checks if `reflection_round < max`.

#### variableIs

```yaml
eval:
  kind: variableIs
  key: variables.next_action
  value: REFLECT
```

Checks if a blackboard variable equals the expected value. Supports dot-notation paths.

#### regex

```yaml
eval:
  kind: regex
  pattern: "(429|rate.?limit|quota.?exceeded)"
```

Matches `provider_output` against a regex pattern.

#### script

```yaml
eval:
  kind: script
  expression: "reflection_round < 2 && provider_output.contains(\"claims_completion\")"
```

Evaluates a minimal expression against blackboard variables.

**V1 expression grammar**:
```
expr       := comparison (('&&' | '||') comparison)*
comparison := path op literal | 'is_dangerous' '(' path ')' | path '.' 'contains' '(' string ')'
path       := identifier ('.' identifier)*
op         := '==' | '!=' | '<' | '<=' | '>' | '>='
literal    := string | number | 'true' | 'false'
```

#### or / and / not

```yaml
eval:
  kind: or
  conditions:
    - { kind: variableIs, key: error_recommendation, value: ESCALATE }
    - { kind: variableIs, key: error_recommendation, value: SKIP }

eval:
  kind: and
  conditions:
    - { kind: outputContains, pattern: "error" }
    - { kind: reflectionRoundUnder, max: 3 }

eval:
  kind: not
  condition:
    kind: outputContains
    pattern: "success"
```

### EvaluatorRegistry

```rust
/// Registry for built-in and custom evaluators.
pub(crate) struct EvaluatorRegistry {
    custom_factories: HashMap<String, CustomEvaluatorFactory>,
}

type CustomEvaluatorFactory = fn(params: &HashMap<String, BlackboardValue>) -> Result<Box<dyn CustomEval>, ParseError>;

/// Trait for host-provided custom evaluators.
pub trait CustomEval: Send + Sync {
    fn evaluate(&self, params: &HashMap<String, BlackboardValue>, bb: &Blackboard) -> Result<bool, RuntimeError>;
}

impl EvaluatorRegistry {
    pub fn new() -> Self {
        Self { custom_factories: HashMap::new() }
    }

    /// Register a custom evaluator factory by name.
    pub fn register(&mut self, name: &str, factory: CustomEvaluatorFactory) {
        self.custom_factories.insert(name.to_string(), factory);
    }

    /// Create an Evaluator from its YAML representation.
    pub fn create(&self, kind: &str, props: &serde_yaml::Mapping) -> Result<Evaluator, ParseError> {
        match kind {
            "outputContains" => Ok(Evaluator::OutputContains {
                pattern: get_string(props, "pattern")?,
                case_sensitive: get_bool(props, "caseSensitive").unwrap_or(false),
            }),
            "situationIs" => Ok(Evaluator::SituationIs {
                situation_type: get_string(props, "type")?,
            }),
            "reflectionRoundUnder" => Ok(Evaluator::ReflectionRoundUnder {
                max: get_u8(props, "max")?,
            }),
            "variableIs" => Ok(Evaluator::VariableIs {
                key: get_string(props, "key")?,
                expected: get_blackboard_value(props, "value")?,
            }),
            "regex" => {
                let pattern_str = get_string(props, "pattern")?;
                let re = Regex::new(&pattern_str)
                    .map_err(|e| ParseError::InvalidProperty {
                        key: "pattern".into(),
                        value: pattern_str,
                        reason: e.to_string(),
                    })?;
                Ok(Evaluator::RegexMatch { pattern: re })
            }
            "script" => Ok(Evaluator::Script {
                expression: get_string(props, "expression")?,
            }),
            "or" => {
                let conds = parse_evaluator_array(props, "conditions")?;
                Ok(Evaluator::Or { conditions: conds })
            }
            "and" => {
                let conds = parse_evaluator_array(props, "conditions")?;
                Ok(Evaluator::And { conditions: conds })
            }
            "not" => {
                let cond = self.create_from_mapping(props, "condition")?;
                Ok(Evaluator::Not { condition: Box::new(cond) })
            }
            _ => {
                // Check custom registry
                if self.custom_factories.contains_key(kind) {
                    let params = parse_params(props)?;
                    Ok(Evaluator::Custom { name: kind.to_string(), params })
                } else {
                    Err(ParseError::UnknownEvaluatorKind { kind: kind.to_string() })
                }
            }
        }
    }
}
```

### Minimal Script Engine (V1)

```rust
/// Supported syntax:
///   reflection_round < 2
///   provider_output.contains("claims_completion")
///   reflection_round < 2 && confidence > 0.8
///   is_dangerous(provider_output)
fn evaluate_minimal_script(expression: &str, bb: &Blackboard) -> Result<bool, RuntimeError> {
    // Recursive descent parser for the expression grammar defined above.
    // Short-circuits && and ||.
    // Resolves paths via bb.get_path().
    // Built-in functions: is_dangerous(path) checks for known dangerous keywords.
    todo!("implement minimal script evaluator")
}
```

---

## Output Parser Enum

Prompt nodes and Switch prompts use `OutputParser` to turn raw LLM text into structured Blackboard values.

```rust
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub(crate) enum OutputParser {
    Enum {
        values: Vec<String>,
        #[serde(default)]
        case_sensitive: bool,
    },
    Structured {
        pattern: String,           // Stored as string for serde; compiled at creation time
        fields: Vec<StructuredField>,
    },
    Json {
        schema: Option<serde_json::Value>,
    },
    Command {
        mapping: HashMap<String, DecisionCommand>,
    },
    /// Extension point: host-registered custom parsers.
    Custom {
        name: String,
        params: HashMap<String, BlackboardValue>,
    },
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub(crate) struct StructuredField {
    pub name: String,
    pub group: usize,
    #[serde(default)]
    pub ty: FieldType,
}

#[derive(Debug, Clone, Copy, PartialEq, Default, serde::Serialize, serde::Deserialize)]
pub(crate) enum FieldType {
    #[default]
    String,
    Integer,
    Float,
    Boolean,
}

impl OutputParser {
    pub fn parse(&self, raw: &str) -> Result<HashMap<String, BlackboardValue>, ParseError> {
        match self {
            Self::Enum { values, case_sensitive } => {
                let trimmed = raw.trim();
                for value in values {
                    let matches = if *case_sensitive {
                        trimmed == value.as_str()
                    } else {
                        trimmed.eq_ignore_ascii_case(value)
                    };
                    if matches {
                        let mut result = HashMap::new();
                        result.insert("decision".to_string(), BlackboardValue::String(value.clone()));
                        return Ok(result);
                    }
                }
                Err(ParseError::UnexpectedValue {
                    got: trimmed.to_string(),
                    expected: values.clone(),
                })
            }
            Self::Structured { pattern, fields } => {
                let re = Regex::new(pattern)
                    .map_err(|e| ParseError::InvalidProperty {
                        key: "pattern".into(),
                        value: pattern.clone(),
                        reason: e.to_string(),
                    })?;
                let caps = re.captures(raw)
                    .ok_or_else(|| ParseError::NoMatch { pattern: pattern.clone() })?;

                let mut result = HashMap::new();
                for field in fields {
                    let m = caps.get(field.group)
                        .ok_or_else(|| ParseError::MissingCaptureGroup {
                            group: field.group,
                            pattern: pattern.clone(),
                        })?;
                    let value = match field.ty {
                        FieldType::String => BlackboardValue::String(m.as_str().to_string()),
                        FieldType::Integer => m.as_str().parse::<i64>()
                            .map(BlackboardValue::Integer)
                            .map_err(|_| ParseError::TypeMismatch {
                                field: field.name.clone(),
                                expected: "integer",
                                got: m.as_str().to_string(),
                            })?,
                        FieldType::Float => m.as_str().parse::<f64>()
                            .map(BlackboardValue::Float)
                            .map_err(|_| ParseError::TypeMismatch {
                                field: field.name.clone(),
                                expected: "float",
                                got: m.as_str().to_string(),
                            })?,
                        FieldType::Boolean => match m.as_str().to_lowercase().as_str() {
                            "true" | "yes" | "1" => BlackboardValue::Boolean(true),
                            "false" | "no" | "0" => BlackboardValue::Boolean(false),
                            _ => return Err(ParseError::TypeMismatch {
                                field: field.name.clone(),
                                expected: "boolean",
                                got: m.as_str().to_string(),
                            }),
                        },
                    };
                    result.insert(field.name.clone(), value);
                }
                Ok(result)
            }
            Self::Json { schema } => {
                let json: serde_json::Value = serde_json::from_str(raw)
                    .map_err(|e| ParseError::JsonSyntax(e.to_string()))?;

                if let Some(schema) = schema {
                    // Optional: validate against JSON Schema
                }

                let mut result = HashMap::new();
                if let serde_json::Value::Object(map) = json {
                    for (k, v) in map {
                        result.insert(k, json_to_blackboard(v)?);
                    }
                }
                Ok(result)
            }
            Self::Command { mapping } => {
                let trimmed = raw.trim();
                for (key, cmd) in mapping {
                    if trimmed.eq_ignore_ascii_case(key) {
                        let mut result = HashMap::new();
                        result.insert(
                            "__command".to_string(),
                            BlackboardValue::String(serde_json::to_string(cmd).unwrap()),
                        );
                        return Ok(result);
                    }
                }
                Err(ParseError::UnexpectedValue {
                    got: trimmed.to_string(),
                    expected: mapping.keys().cloned().collect(),
                })
            }
            Self::Custom { name, params } => {
                PARSER_DISPATCH.with(|dispatch| {
                    dispatch.parse(name, params, raw)
                })
            }
        }
    }
}
```

### Enum Parser

Parse a single value from a constrained set:

```yaml
parser:
  kind: enum
  values: [REFLECT, CONFIRM, ESCALATE]
  caseSensitive: false
```

**Input**: `"  reflect  "` → **Parsed**: `{ decision: "REFLECT" }`
**Input**: `"maybe"` → **Failure**: `UnexpectedValue`

### Structured Parser

Parse fields from text using regex capture groups:

```yaml
parser:
  kind: structured
  pattern: "CLASS:\\s*(\\w+)\\s*RECOMMEND:\\s*(\\w+)\\s*REASON:\\s*(.*)"
  fields:
    - { name: classification, group: 1 }
    - { name: recommendation, group: 2 }
    - { name: reason, group: 3, type: string }
```

**Input**: `CLASS: SYNTAX RECOMMEND: FIX REASON: Missing semicolon`
→ **Parsed**: `{ classification: "SYNTAX", recommendation: "FIX", reason: "Missing semicolon" }`

### JSON Parser

Parse JSON responses with optional schema validation:

```yaml
parser:
  kind: json
  schema:                         # Optional JSON Schema
    type: object
    properties:
      decision:
        type: string
        enum: [reflect, confirm]
      confidence:
        type: number
    required: [decision]
```

**Input**: `{"decision": "reflect", "confidence": 0.82}`
→ **Parsed**: `{ decision: "reflect", confidence: 0.82 }`

### Command Parser

Parse directly into a DecisionCommand (for low-level Prompt nodes only; Switch nodes handle this implicitly):

```yaml
parser:
  kind: command
  mapping:
    REFLECT:
      command: Reflect
      params:
        prompt: "Review your work"
    CONFIRM:
      command: ConfirmCompletion
    ESCALATE:
      command: EscalateToHuman
      params:
        reason: "LLM chose escalation"
```

### OutputParserRegistry

```rust
pub(crate) struct OutputParserRegistry {
    custom_factories: HashMap<String, CustomParserFactory>,
}

type CustomParserFactory = fn(params: &HashMap<String, BlackboardValue>) -> Result<Box<dyn CustomParse>, ParseError>;

pub trait CustomParse: Send + Sync {
    fn parse(&self, params: &HashMap<String, BlackboardValue>, raw: &str) -> Result<HashMap<String, BlackboardValue>, ParseError>;
}

impl OutputParserRegistry {
    pub fn new() -> Self {
        Self { custom_factories: HashMap::new() }
    }

    pub fn register(&mut self, name: &str, factory: CustomParserFactory) {
        self.custom_factories.insert(name.to_string(), factory);
    }

    pub fn create(&self, kind: &str, props: &serde_yaml::Mapping) -> Result<OutputParser, ParseError> {
        match kind {
            "enum" => Ok(OutputParser::Enum {
                values: get_string_array(props, "values")?,
                case_sensitive: get_bool(props, "caseSensitive").unwrap_or(false),
            }),
            "structured" => {
                let pattern_str = get_string(props, "pattern")?;
                // Validate regex at parse time (stored as string for serde compatibility)
                Regex::new(&pattern_str).map_err(|e| ParseError::InvalidProperty {
                    key: "pattern".into(),
                    value: pattern_str.clone(),
                    reason: e.to_string(),
                })?;
                let fields = get_structured_fields(props)?;
                Ok(OutputParser::Structured { pattern: pattern_str, fields })
            }
            "json" => {
                let schema = props.get("schema").cloned();
                Ok(OutputParser::Json { schema })
            }
            "command" => {
                let mapping = parse_command_mapping(props)?;
                Ok(OutputParser::Command { mapping })
            }
            _ => {
                if self.custom_factories.contains_key(kind) {
                    let params = parse_params(props)?;
                    Ok(OutputParser::Custom { name: kind.to_string(), params })
                } else {
                    Err(ParseError::UnknownParserKind { kind: kind.to_string() })
                }
            }
        }
    }
}
```

### Helper: JSON to Blackboard Value

```rust
fn json_to_blackboard(v: serde_json::Value) -> Result<BlackboardValue, ParseError> {
    match v {
        serde_json::Value::String(s) => Ok(BlackboardValue::String(s)),
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                Ok(BlackboardValue::Integer(i))
            } else {
                Ok(BlackboardValue::Float(n.as_f64().unwrap_or(0.0)))
            }
        }
        serde_json::Value::Bool(b) => Ok(BlackboardValue::Boolean(b)),
        serde_json::Value::Array(arr) => {
            let items: Result<Vec<_>, _> = arr.into_iter().map(json_to_blackboard).collect();
            Ok(BlackboardValue::List(items?))
        }
        serde_json::Value::Object(map) => {
            let mut result = HashMap::new();
            for (k, v) in map {
                result.insert(k, json_to_blackboard(v)?);
            }
            Ok(BlackboardValue::Map(result))
        }
        serde_json::Value::Null => Ok(BlackboardValue::String(String::new())),
    }
}
```

---

## YAML Property Helpers

These helpers extract typed values from `serde_yaml::Mapping`. They support both `snake_case` and `camelCase` keys.

```rust
fn get_string(props: &serde_yaml::Mapping, key: &str) -> Result<String, ParseError> {
    props.get(serde_yaml::Value::String(key.to_string()))
        .or_else(|| props.get(&serde_yaml::Value::String(to_camel_case(key))))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| ParseError::MissingProperty(key))
}

fn get_bool(props: &serde_yaml::Mapping, key: &str) -> Option<bool> {
    props.get(serde_yaml::Value::String(key.to_string()))
        .or_else(|| props.get(&serde_yaml::Value::String(to_camel_case(key))))
        .and_then(|v| v.as_bool())
}

fn get_u8(props: &serde_yaml::Mapping, key: &str) -> Result<u8, ParseError> {
    props.get(serde_yaml::Value::String(key.to_string()))
        .or_else(|| props.get(&serde_yaml::Value::String(to_camel_case(key))))
        .and_then(|v| v.as_u64())
        .and_then(|n| n.try_into().ok())
        .ok_or_else(|| ParseError::MissingProperty(key))
}

fn get_string_array(props: &serde_yaml::Mapping, key: &str) -> Result<Vec<String>, ParseError> {
    let arr = props.get(serde_yaml::Value::String(key.to_string()))
        .or_else(|| props.get(&serde_yaml::Value::String(to_camel_case(key))))
        .and_then(|v| v.as_sequence())
        .ok_or_else(|| ParseError::MissingProperty(key))?;
    arr.iter()
        .map(|v| v.as_str().map(|s| s.to_string())
            .ok_or_else(|| ParseError::Custom(format!("expected string array in {}", key))))
        .collect()
}

fn get_structured_fields(props: &serde_yaml::Mapping) -> Result<Vec<StructuredField>, ParseError> {
    let fields_seq = props.get("fields")
        .and_then(|v| v.as_sequence())
        .ok_or_else(|| ParseError::MissingProperty("fields"))?;
    let mut result = Vec::new();
    for item in fields_seq {
        let map = item.as_mapping()
            .ok_or_else(|| ParseError::Custom("expected mapping in fields".into()))?;
        let name = get_string(map, "name")?;
        let group = map.get("group")
            .and_then(|v| v.as_u64())
            .and_then(|n| n.try_into().ok())
            .ok_or_else(|| ParseError::MissingProperty("group"))?;
        let ty = map.get("type")
            .and_then(|v| v.as_str())
            .map(|s| match s {
                "string" => FieldType::String,
                "integer" => FieldType::Integer,
                "float" => FieldType::Float,
                "boolean" => FieldType::Boolean,
                _ => FieldType::String,
            })
            .unwrap_or(FieldType::String);
        result.push(StructuredField { name, group, ty });
    }
    Ok(result)
}

fn to_camel_case(s: &str) -> String {
    let mut result = String::new();
    let mut capitalize_next = false;
    for ch in s.chars() {
        if ch == '_' {
            capitalize_next = true;
        } else if capitalize_next {
            result.push(ch.to_ascii_uppercase());
            capitalize_next = false;
        } else {
            result.push(ch);
        }
    }
    result
}
```

---

*Document version: 2.0*
*Last updated: 2026-04-24*
