# Decision DSL: Evaluators & Output Parsers

> Expression evaluation and output parsing specification for the decision DSL engine. Covers Condition evaluators (used by Condition nodes and Action `when` guards) and output parsers (used by Prompt nodes to structure LLM replies).
>
> This document is a chapter of the [Decision DSL Implementation](decision-dsl-implementation.md).

## Evaluator System

Condition nodes and Action `when` guards both use the same `Evaluator` trait.

###1 YAML Property Helpers

These helpers extract typed values from `serde_yaml::Mapping` (the raw YAML properties of a node).
They perform the camelCase → snake_case field mapping at parse time.

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

fn get_u32(props: &serde_yaml::Mapping, key: &str) -> Result<u32, ParseError> {
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
        .map(|v| v.as_str().map(|s| s.to_string()).ok_or_else(|| ParseError::Custom(format!("expected string array in {}", key))))
        .collect()
}

fn get_structured_fields(props: &serde_yaml::Mapping) -> Result<Vec<StructuredField>, ParseError> {
    let fields_seq = props.get("fields")
        .and_then(|v| v.as_sequence())
        .ok_or_else(|| ParseError::MissingProperty("fields"))?;
    let mut result = Vec::new();
    for item in fields_seq {
        let map = item.as_mapping().ok_or_else(|| ParseError::Custom("expected mapping in fields".into()))?;
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

/// Convert snake_case key to camelCase for YAML property lookup.
/// Example: "case_sensitive" → "caseSensitive"
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

###2 Trait Definition

```rust
pub(crate) trait Evaluator: std::fmt::Debug + Send + Sync {
    fn evaluate(&self, blackboard: &Blackboard) -> Result<bool, RuntimeError>;
}

/// Registry of evaluator factories.
pub(crate) struct EvaluatorRegistry {
    factories: HashMap<String, EvaluatorFactory>,
}

type EvaluatorFactory = fn(properties: &serde_yaml::Mapping) -> Result<Box<dyn Evaluator>, ParseError>;

impl EvaluatorRegistry {
    pub fn with_builtins() -> Self {
        let mut reg = Self { factories: HashMap::new() };
        reg.register("outputContains", OutputContains::from_yaml);
        reg.register("situationIs", SituationIs::from_yaml);
        reg.register("reflectionRoundUnder", ReflectionRoundUnder::from_yaml);
        reg.register("variableIs", VariableIs::from_yaml);
        reg.register("regex", RegexMatch::from_yaml);
        reg.register("script", ScriptEvaluator::from_yaml);
        reg.register("or", OrEvaluator::from_yaml);
        reg.register("and", AndEvaluator::from_yaml);
        reg
    }

    pub fn register(&mut self, kind: &str, factory: EvaluatorFactory) {
        self.factories.insert(kind.to_string(), factory);
    }

    pub fn create(&self, kind: &str, properties: &serde_yaml::Mapping) -> Result<Box<dyn Evaluator>, ParseError> {
        let factory = self.factories.get(kind)
            .ok_or_else(|| ParseError::UnknownEvaluatorKind { kind: kind.to_string() })?;
        factory(properties)
    }
}
```

###2 Built-in Evaluators

```rust
// --- outputContains ---
#[derive(Debug)]
pub struct OutputContains {
    pub pattern: String,
    pub case_sensitive: bool,
}

impl OutputContains {
    fn from_yaml(props: &serde_yaml::Mapping) -> Result<Box<dyn Evaluator>, ParseError> {
        let pattern = get_string(props, "pattern")?;
        let case_sensitive = get_bool(props, "caseSensitive").unwrap_or(false);
        Ok(Box::new(Self { pattern, case_sensitive }))
    }
}

impl Evaluator for OutputContains {
    fn evaluate(&self, bb: &Blackboard) -> Result<bool, RuntimeError> {
        let output = bb.get_string("provider_output").unwrap_or("");
        Ok(if self.case_sensitive {
            output.contains(&self.pattern)
        } else {
            output.to_lowercase().contains(&self.pattern.to_lowercase())
        })
    }
}

// --- situationIs ---
#[derive(Debug)]
pub struct SituationIs {
    pub situation_type: String,
}

impl Evaluator for SituationIs {
    fn evaluate(&self, bb: &Blackboard) -> Result<bool, RuntimeError> {
        let output = bb.get_string("provider_output").unwrap_or("");
        let summary = bb.get_string("context_summary").unwrap_or("");
        Ok(output.contains(&self.situation_type) || summary.contains(&self.situation_type))
    }
}

// --- reflectionRoundUnder ---
#[derive(Debug)]
pub struct ReflectionRoundUnder {
    pub max: u8,
}

impl Evaluator for ReflectionRoundUnder {
    fn evaluate(&self, bb: &Blackboard) -> Result<bool, RuntimeError> {
        let round = bb.get_u8("reflection_round").unwrap_or(0);
        Ok(round < self.max)
    }
}

// --- variableIs ---
#[derive(Debug)]
pub struct VariableIs {
    pub key: String,
    pub expected: BlackboardValue,
}

impl Evaluator for VariableIs {
    fn evaluate(&self, bb: &Blackboard) -> Result<bool, RuntimeError> {
        match bb.get_path(&self.key) {
            Some(value) => Ok(&value == &self.expected),
            None => Ok(false),
        }
    }
}

// --- regex ---
#[derive(Debug)]
pub struct RegexMatch {
    pub re: regex::Regex,
}

impl Evaluator for RegexMatch {
    fn evaluate(&self, bb: &Blackboard) -> Result<bool, RuntimeError> {
        let output = bb.get_string("provider_output").unwrap_or("");
        Ok(self.re.is_match(output))
    }
}

// --- script (Rhai) ---
// Note: If Rhai is too heavy for a zero-dependency crate, we implement
// a minimal expression language instead.
#[derive(Debug)]
pub struct ScriptEvaluator {
    pub script: String,
}

impl Evaluator for ScriptEvaluator {
    fn evaluate(&self, bb: &Blackboard) -> Result<bool, RuntimeError> {
        // V1: Evaluate a minimal expression against blackboard variables.
        // Full Rhai integration can be added as an optional feature.
        evaluate_minimal_script(&self.script, bb)
    }
}

// --- or ---
#[derive(Debug)]
pub struct OrEvaluator {
    pub conditions: Vec<Box<dyn Evaluator>>,
}

impl Evaluator for OrEvaluator {
    fn evaluate(&self, bb: &Blackboard) -> Result<bool, RuntimeError> {
        for cond in &self.conditions {
            if cond.evaluate(bb)? {
                return Ok(true);
            }
        }
        Ok(false)
    }
}

// --- and ---
#[derive(Debug)]
pub struct AndEvaluator {
    pub conditions: Vec<Box<dyn Evaluator>>,
}

impl Evaluator for AndEvaluator {
    fn evaluate(&self, bb: &Blackboard) -> Result<bool, RuntimeError> {
        for cond in &self.conditions {
            if !cond.evaluate(bb)? {
                return Ok(false);
            }
        }
        Ok(true)
    }
}
```

###3 Minimal Script Engine (V1)

For zero-dependency compliance, the `script` evaluator in V1 supports a tiny expression language:

```rust
/// Supported syntax:
///   blackboard.key < 2
///   blackboard.provider_output.contains("claims_completion")
///   blackboard.reflection_round < 2 && blackboard.confidence > 0.8
fn evaluate_minimal_script(script: &str, bb: &Blackboard) -> Result<bool, RuntimeError> {
    // Design: recursive descent parser for a tiny expression language.
    //
    // Grammar:
    //   expr       := term (('&&' | '||') term)*
    //   term       := comparison | '(' expr ')'
    //   comparison := path op literal
    //   op         := '==' | '!=' | '<' | '<=' | '>' | '>='
    //   path       := 'blackboard.' identifier ('.' identifier)*
    //   literal    := string | number | bool
    //
    // Implementation sketch:
    //   1. Tokenize the script into tokens (identifier, operator, literal, punctuation).
    //   2. Parse into an AST using recursive descent.
    //   3. Evaluate the AST against the Blackboard:
    //      - Resolve paths via bb.get_path()
    //      - Compare values using the specified operator.
    //      - Short-circuit && and ||.
    //
    // For V1, script evaluators can also be implemented as a pre-registered
    // custom evaluator if the host provides a full scripting engine (e.g. Rhai).
    Err(RuntimeError::Custom("script evaluator not yet implemented".into()))
}
```

---

## Output Parser System

Prompt nodes use `OutputParser` to turn raw LLM text into structured Blackboard values.

###1 Trait Definition

```rust
pub(crate) trait OutputParser: std::fmt::Debug + Send + Sync {
    fn parse(&self, raw: &str) -> Result<HashMap<String, BlackboardValue>, ParseError>;
}

pub(crate) struct OutputParserRegistry {
    factories: HashMap<String, ParserFactory>,
}

type ParserFactory = fn(properties: &serde_yaml::Mapping) -> Result<Box<dyn OutputParser>, ParseError>;

impl OutputParserRegistry {
    pub fn with_builtins() -> Self {
        let mut reg = Self { factories: HashMap::new() };
        reg.register("enum", EnumParser::from_yaml);
        reg.register("structured", StructuredParser::from_yaml);
        reg.register("json", JsonParser::from_yaml);
        reg.register("command", CommandParser::from_yaml);
        reg
    }

    pub fn register(&mut self, kind: &str, factory: ParserFactory) {
        self.factories.insert(kind.to_string(), factory);
    }

    pub fn create(&self, kind: &str, props: &serde_yaml::Mapping) -> Result<Box<dyn OutputParser>, ParseError> {
        let factory = self.factories.get(kind)
            .ok_or_else(|| ParseError::UnknownParserKind { kind: kind.to_string() })?;
        factory(props)
    }
}
```

###2 Enum Parser

```rust
#[derive(Debug)]
pub struct EnumParser {
    pub allowed_values: Vec<String>,
    pub case_sensitive: bool,
}

impl EnumParser {
    fn from_yaml(props: &serde_yaml::Mapping) -> Result<Box<dyn OutputParser>, ParseError> {
        let values = get_string_array(props, "values")?;
        let case_sensitive = get_bool(props, "caseSensitive").unwrap_or(false);
        Ok(Box::new(Self { allowed_values: values, case_sensitive }))
    }
}

impl OutputParser for EnumParser {
    fn parse(&self, raw: &str) -> Result<HashMap<String, BlackboardValue>, ParseError> {
        let trimmed = raw.trim();
        for value in &self.allowed_values {
            let matches = if self.case_sensitive {
                trimmed == value
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
            expected: self.allowed_values.clone(),
        })
    }
}
```

###3 Structured Parser (Regex with Typed Groups)

```rust
#[derive(Debug)]
pub struct StructuredField {
    pub name: String,
    pub group: usize,
    pub ty: FieldType,   // optional type conversion
}

#[derive(Debug, Clone, Copy)]
pub enum FieldType {
    String,
    Integer,
    Float,
    Boolean,
}

#[derive(Debug)]
pub struct StructuredParser {
    pub pattern: regex::Regex,
    pub fields: Vec<StructuredField>,
}

impl StructuredParser {
    fn from_yaml(props: &serde_yaml::Mapping) -> Result<Box<dyn OutputParser>, ParseError> {
        let pattern_str = get_string(props, "pattern")?;
        let pattern = regex::Regex::new(&pattern_str)
            .map_err(|e| ParseError::InvalidProperty {
                key: "pattern".into(),
                value: pattern_str,
                reason: e.to_string(),
            })?;

        let fields = get_structured_fields(props)?;
        Ok(Box::new(Self { pattern, fields }))
    }
}

impl OutputParser for StructuredParser {
    fn parse(&self, raw: &str) -> Result<HashMap<String, BlackboardValue>, ParseError> {
        let caps = self.pattern.captures(raw)
            .ok_or_else(|| ParseError::NoMatch { pattern: self.pattern.as_str().to_string() })?;

        let mut result = HashMap::new();
        for field in &self.fields {
            let m = caps.get(field.group)
                .ok_or_else(|| ParseError::MissingCaptureGroup {
                    group: field.group,
                    pattern: self.pattern.as_str().to_string(),
                })?;
            let value = match field.ty {
                FieldType::String => BlackboardValue::String(m.as_str().to_string()),
                FieldType::Integer => m.as_str().parse::<i64>()
                    .map(BlackboardValue::Integer)
                    .map_err(|_| ParseError::TypeMismatch {
                        field: field.name.clone(), expected: "integer", got: m.as_str().to_string(),
                    })?,
                FieldType::Float => m.as_str().parse::<f64>()
                    .map(BlackboardValue::Float)
                    .map_err(|_| ParseError::TypeMismatch {
                        field: field.name.clone(), expected: "float", got: m.as_str().to_string(),
                    })?,
                FieldType::Boolean => match m.as_str().to_lowercase().as_str() {
                    "true" | "yes" | "1" => BlackboardValue::Boolean(true),
                    "false" | "no" | "0" => BlackboardValue::Boolean(false),
                    _ => return Err(ParseError::TypeMismatch {
                        field: field.name.clone(), expected: "boolean", got: m.as_str().to_string(),
                    }),
                },
            };
            result.insert(field.name.clone(), value);
        }
        Ok(result)
    }
}
```

###4 JSON Parser

```rust
#[derive(Debug)]
pub struct JsonParser {
    pub schema: Option<serde_json::Value>, // Optional JSON Schema
}

impl OutputParser for JsonParser {
    fn parse(&self, raw: &str) -> Result<HashMap<String, BlackboardValue>, ParseError> {
        let json: serde_json::Value = serde_json::from_str(raw)
            .map_err(|e| ParseError::JsonSyntax(e.to_string()))?;

        // Optional schema validation
        if let Some(schema) = &self.schema {
            // jsonschema validation (optional feature)
            // validate_json_schema(&json, schema)?;
        }

        let mut result = HashMap::new();
        if let serde_json::Value::Object(map) = json {
            for (k, v) in map {
                result.insert(k, json_to_blackboard(v)?);
            }
        }
        Ok(result)
    }
}

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
        serde_json::Value::Null => Ok(BlackboardValue::String("".to_string())),
    }
}
```

###5 Command Parser

```rust
/// Parses LLM output directly into a Command, bypassing the Blackboard.
#[derive(Debug)]
pub struct CommandParser {
    pub mapping: HashMap<String, CommandMapping>,
}

#[derive(Debug, Clone)]
pub struct CommandMapping {
    pub command: Command,
}

impl OutputParser for CommandParser {
    fn parse(&self, raw: &str) -> Result<HashMap<String, BlackboardValue>, ParseError> {
        let trimmed = raw.trim();
        for (key, mapping) in &self.mapping {
            if trimmed.eq_ignore_ascii_case(key) {
                // Command parsers do NOT return Blackboard values.
                // Instead, they store the command in a special marker.
                // The Prompt node detects this marker and pushes the command directly.
                let mut result = HashMap::new();
                result.insert("__command".to_string(), BlackboardValue::String(
                    serde_json::to_string(&mapping.command).unwrap()
                ));
                return Ok(result);
            }
        }
        Err(ParseError::UnexpectedValue {
            got: trimmed.to_string(),
            expected: self.mapping.keys().cloned().collect(),
        })
    }
}
```

---

