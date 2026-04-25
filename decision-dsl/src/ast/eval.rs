use std::collections::HashMap;

use regex::Regex;
use serde::{Deserialize, Serialize};

use crate::ext::blackboard::{Blackboard, BlackboardValue};
use crate::ext::error::RuntimeError;

/// Evaluator enum with all built-in variants.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "payload")]
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

impl Evaluator {
    pub fn evaluate(&self, blackboard: &Blackboard) -> Result<bool, RuntimeError> {
        match self {
            Evaluator::OutputContains {
                pattern,
                case_sensitive,
            } => {
                if *case_sensitive {
                    Ok(blackboard.provider_output.contains(pattern))
                } else {
                    Ok(blackboard.provider_output.to_lowercase().contains(&pattern.to_lowercase()))
                }
            }
            Evaluator::SituationIs { situation_type } => {
                Ok(blackboard.task_description == *situation_type)
            }
            Evaluator::ReflectionRoundUnder { max } => {
                Ok(blackboard.reflection_round < *max)
            }
            Evaluator::VariableIs { key, expected } => {
                let actual = blackboard
                    .get_path(key)
                    .ok_or_else(|| RuntimeError::MissingVariable {
                        key: key.clone(),
                    })?;
                Ok(actual == *expected)
            }
            Evaluator::RegexMatch { pattern } => {
                let re = Regex::new(pattern).map_err(|e| RuntimeError::FilterError(
                    format!("RegexMatch: {e}"),
                ))?;
                Ok(re.is_match(&blackboard.provider_output))
            }
            Evaluator::Script { expression } => {
                evaluate_script(expression, blackboard)
            }
            Evaluator::Or { conditions } => {
                for cond in conditions {
                    if cond.evaluate(blackboard)? {
                        return Ok(true);
                    }
                }
                Ok(false)
            }
            Evaluator::And { conditions } => {
                for cond in conditions {
                    if !cond.evaluate(blackboard)? {
                        return Ok(false);
                    }
                }
                Ok(true)
            }
            Evaluator::Not { condition } => {
                Ok(!condition.evaluate(blackboard)?)
            }
            Evaluator::Custom { name, .. } => {
                Err(RuntimeError::FilterError(
                    format!("custom evaluator '{name}' not registered"),
                ))
            }
        }
    }
}

// ── Script Evaluator ────────────────────────────────────────────────────────

fn evaluate_script(expr: &str, bb: &Blackboard) -> Result<bool, RuntimeError> {
    let mut parser = ScriptParser::new(expr);
    parser.parse_expr(bb)
}

struct ScriptParser<'a> {
    input: &'a str,
    pos: usize,
}

impl<'a> ScriptParser<'a> {
    fn new(input: &'a str) -> Self {
        Self { input, pos: 0 }
    }

    fn parse_expr(&mut self, bb: &Blackboard) -> Result<bool, RuntimeError> {
        let mut result = self.parse_comparison(bb)?;
        loop {
            self.skip_ws();
            if self.consume("&&") {
                let rhs = self.parse_comparison(bb)?;
                result = result && rhs;
            } else if self.consume("||") {
                let rhs = self.parse_comparison(bb)?;
                result = result || rhs;
            } else {
                break;
            }
        }
        Ok(result)
    }

    fn parse_comparison(&mut self, bb: &Blackboard) -> Result<bool, RuntimeError> {
        self.skip_ws();

        if self.consume("is_dangerous") {
            self.skip_ws();
            if !self.consume("(") {
                return Err(RuntimeError::FilterError(
                    "expected '(' after is_dangerous".into(),
                ));
            }
            self.skip_ws();
            let path = self.parse_path()?;
            self.skip_ws();
            if !self.consume(")") {
                return Err(RuntimeError::FilterError(
                    "expected ')' after is_dangerous argument".into(),
                ));
            }
            let value = get_bb_value(bb, &path)?;
            let s = value_as_string(&value)?;
            return Ok(is_dangerous(&s));
        }

        let mut path = vec![self.parse_identifier()?];
        self.skip_ws();
        loop {
            if !self.consume(".") {
                break;
            }
            self.skip_ws();
            let ident = self.parse_identifier()?;
            self.skip_ws();
            if ident == "contains" {
                if !self.consume("(") {
                    return Err(RuntimeError::FilterError(
                        "expected '(' after contains".into(),
                    ));
                }
                self.skip_ws();
                let needle = self.parse_string()?;
                self.skip_ws();
                if !self.consume(")") {
                    return Err(RuntimeError::FilterError(
                        "expected ')' after contains argument".into(),
                    ));
                }
                let value = get_bb_value(bb, &path)?;
                let s = value_as_string(&value)?;
                return Ok(s.contains(&needle));
            }
            path.push(ident);
        }

        let op = if self.consume("==") {
            "=="
        } else if self.consume("!=") {
            "!="
        } else if self.consume("<=") {
            "<="
        } else if self.consume(">=") {
            ">="
        } else if self.consume("<") {
            "<"
        } else if self.consume(">") {
            ">"
        } else {
            return Err(RuntimeError::FilterError(
                format!("expected operator, got: {}", &self.input[self.pos..]),
            ));
        };

        self.skip_ws();
        let literal = self.parse_literal()?;
        let value = get_bb_value(bb, &path)?;
        compare_values(&value, op, &literal)
    }

    fn parse_path(&mut self) -> Result<Vec<String>, RuntimeError> {
        let mut parts = Vec::new();
        loop {
            self.skip_ws();
            let ident = self.parse_identifier()?;
            parts.push(ident);
            self.skip_ws();
            if !self.consume(".") {
                break;
            }
        }
        Ok(parts)
    }

    fn parse_identifier(&mut self) -> Result<String, RuntimeError> {
        self.skip_ws();
        let start = self.pos;
        if let Some(ch) = self.peek() {
            if ch.is_alphabetic() || ch == '_' {
                self.advance();
                while let Some(ch) = self.peek() {
                    if ch.is_alphanumeric() || ch == '_' {
                        self.advance();
                    } else {
                        break;
                    }
                }
                Ok(self.input[start..self.pos].to_string())
            } else {
                Err(RuntimeError::FilterError(
                    format!("expected identifier at: {}", &self.input[self.pos..]),
                ))
            }
        } else {
            Err(RuntimeError::FilterError(
                "unexpected end of input, expected identifier".into(),
            ))
        }
    }

    fn parse_literal(&mut self) -> Result<BlackboardValue, RuntimeError> {
        self.skip_ws();
        if self.consume("true") {
            Ok(BlackboardValue::Boolean(true))
        } else if self.consume("false") {
            Ok(BlackboardValue::Boolean(false))
        } else if let Some(ch) = self.peek() {
            if ch == '"' {
                Ok(BlackboardValue::String(self.parse_string()?))
            } else if ch.is_ascii_digit() || ch == '-' {
                self.parse_number()
            } else {
                Err(RuntimeError::FilterError(
                    format!("expected literal at: {}", &self.input[self.pos..]),
                ))
            }
        } else {
            Err(RuntimeError::FilterError(
                "unexpected end of input, expected literal".into(),
            ))
        }
    }

    fn parse_string(&mut self) -> Result<String, RuntimeError> {
        self.skip_ws();
        if !self.consume("\"") {
            return Err(RuntimeError::FilterError(
                "expected string literal".into(),
            ));
        }
        let mut result = String::new();
        while let Some(ch) = self.peek() {
            if ch == '"' {
                break;
            }
            if ch == '\\' {
                self.advance(); // consume backslash
                match self.peek() {
                    Some('n') => result.push('\n'),
                    Some('t') => result.push('\t'),
                    Some('r') => result.push('\r'),
                    Some('"') => result.push('"'),
                    Some('\\') => result.push('\\'),
                    Some('/') => result.push('/'),
                    Some('u') => {
                        // Unicode escape: \uXXXX
                        self.advance();
                        let hex = &self.input[self.pos..self.pos.min(self.pos + 4)];
                        if hex.len() < 4 {
                            return Err(RuntimeError::FilterError(
                                "invalid unicode escape: expected 4 hex digits".into(),
                            ));
                        }
                        let code = u32::from_str_radix(hex, 16).map_err(|_| {
                            RuntimeError::FilterError("invalid unicode escape: bad hex digits".into())
                        })?;
                        let c = char::from_u32(code).ok_or_else(|| {
                            RuntimeError::FilterError("invalid unicode escape: invalid code point".into())
                        })?;
                        result.push(c);
                        self.pos += 4;
                        continue;
                    }
                    Some(c) => {
                        // Unknown escape, keep the character literally
                        result.push(c);
                    }
                    None => {
                        return Err(RuntimeError::FilterError(
                            "unterminated escape sequence".into(),
                        ));
                    }
                }
                self.advance();
            } else {
                result.push(ch);
                self.advance();
            }
        }
        if !self.consume("\"") {
            return Err(RuntimeError::FilterError(
                "unterminated string literal".into(),
            ));
        }
        Ok(result)
    }

    fn parse_number(&mut self) -> Result<BlackboardValue, RuntimeError> {
        self.skip_ws();
        let start = self.pos;
        if self.consume("-") {
            // negative number
        }
        while let Some(ch) = self.peek() {
            if ch.is_ascii_digit() || ch == '.' {
                self.advance();
            } else {
                break;
            }
        }
        let s = &self.input[start..self.pos];
        if s.contains('.') {
            s.parse::<f64>()
                .map(BlackboardValue::Float)
                .map_err(|e| RuntimeError::FilterError(format!("invalid float: {e}")))
        } else {
            s.parse::<i64>()
                .map(BlackboardValue::Integer)
                .map_err(|e| RuntimeError::FilterError(format!("invalid integer: {e}")))
        }
    }

    fn skip_ws(&mut self) {
        while let Some(ch) = self.peek() {
            if ch.is_whitespace() {
                self.advance();
            } else {
                break;
            }
        }
    }

    fn peek(&self) -> Option<char> {
        self.input[self.pos..].chars().next()
    }

    fn advance(&mut self) {
        if let Some(ch) = self.peek() {
            self.pos += ch.len_utf8();
        }
    }

    fn consume(&mut self, s: &str) -> bool {
        self.skip_ws();
        if self.input[self.pos..].starts_with(s) {
            if s.chars().all(|c| c.is_alphanumeric() || c == '_') {
                let after = self.pos + s.len();
                if let Some(ch) = self.input[after..].chars().next() {
                    if ch.is_alphanumeric() || ch == '_' {
                        return false;
                    }
                }
            }
            self.pos += s.len();
            true
        } else {
            false
        }
    }
}

fn get_bb_value(bb: &Blackboard, path: &[String]) -> Result<BlackboardValue, RuntimeError> {
    let path_str = path.join(".");
    bb.get_path(&path_str).ok_or_else(|| RuntimeError::MissingVariable {
        key: path_str,
    })
}

fn value_as_string(value: &BlackboardValue) -> Result<String, RuntimeError> {
    match value {
        BlackboardValue::String(s) => Ok(s.clone()),
        BlackboardValue::Integer(i) => Ok(i.to_string()),
        BlackboardValue::Float(f) => Ok(f.to_string()),
        BlackboardValue::Boolean(b) => Ok(b.to_string()),
        BlackboardValue::Null => Ok("".into()),
        _ => Err(RuntimeError::FilterError(
            "expected scalar value".into(),
        )),
    }
}

fn compare_values(
    left: &BlackboardValue,
    op: &str,
    right: &BlackboardValue,
) -> Result<bool, RuntimeError> {
    match (left, right) {
        (BlackboardValue::String(a), BlackboardValue::String(b)) => match op {
            "==" => Ok(a == b),
            "!=" => Ok(a != b),
            "<" => Ok(a < b),
            "<=" => Ok(a <= b),
            ">" => Ok(a > b),
            ">=" => Ok(a >= b),
            _ => Err(RuntimeError::FilterError(format!("unknown operator: {op}"))),
        },
        (BlackboardValue::Integer(a), BlackboardValue::Integer(b)) => match op {
            "==" => Ok(a == b),
            "!=" => Ok(a != b),
            "<" => Ok(a < b),
            "<=" => Ok(a <= b),
            ">" => Ok(a > b),
            ">=" => Ok(a >= b),
            _ => Err(RuntimeError::FilterError(format!("unknown operator: {op}"))),
        },
        (BlackboardValue::Float(a), BlackboardValue::Float(b)) => {
            // Handle NaN: NaN comparisons are always false except !=
            // NaN is not equal to anything, including itself
            let a_is_nan = a.is_nan();
            let b_is_nan = b.is_nan();
            match op {
                "==" => {
                    if a_is_nan || b_is_nan {
                        Ok(false) // NaN never equals anything
                    } else {
                        Ok((a - b).abs() < f64::EPSILON)
                    }
                }
                "!=" => {
                    if a_is_nan || b_is_nan {
                        Ok(true) // NaN is not equal to anything
                    } else {
                        Ok((a - b).abs() >= f64::EPSILON)
                    }
                }
                "<" => {
                    if a_is_nan || b_is_nan {
                        Ok(false) // NaN comparisons are false
                    } else {
                        Ok(a < b)
                    }
                }
                "<=" => {
                    if a_is_nan || b_is_nan {
                        Ok(false)
                    } else {
                        Ok(a <= b)
                    }
                }
                ">" => {
                    if a_is_nan || b_is_nan {
                        Ok(false)
                    } else {
                        Ok(a > b)
                    }
                }
                ">=" => {
                    if a_is_nan || b_is_nan {
                        Ok(false)
                    } else {
                        Ok(a >= b)
                    }
                }
                _ => Err(RuntimeError::FilterError(format!("unknown operator: {op}"))),
            }
        }
        (BlackboardValue::Integer(a), BlackboardValue::Float(_b)) => {
            compare_values(&BlackboardValue::Float(*a as f64), op, right)
        }
        (BlackboardValue::Float(_a), BlackboardValue::Integer(b)) => {
            compare_values(left, op, &BlackboardValue::Float(*b as f64))
        }
        (BlackboardValue::Boolean(a), BlackboardValue::Boolean(b)) => match op {
            "==" => Ok(a == b),
            "!=" => Ok(a != b),
            _ => Err(RuntimeError::FilterError(
                format!("operator {op} not supported for booleans"),
            )),
        },
        _ => Err(RuntimeError::FilterError(
            format!("type mismatch in comparison: {left:?} {op} {right:?}"),
        )),
    }
}

fn is_dangerous(s: &str) -> bool {
    let lower = s.to_lowercase();
    let keywords = [
        "delete", "drop", "rm -rf", "truncate table", "drop table",
        "drop database", "shutdown", "kill", "format", "destroy",
    ];
    keywords.iter().any(|&kw| lower.contains(kw))
}

// ── EvaluatorRegistry ───────────────────────────────────────────────────────

pub struct EvaluatorRegistry {
    builtins: HashMap<String, Box<dyn Fn(&HashMap<String, BlackboardValue>) -> Result<Evaluator, RuntimeError> + Send + Sync>>,
}

impl Default for EvaluatorRegistry {
    fn default() -> Self {
        Self { builtins: HashMap::new() }
    }
}

impl EvaluatorRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_builtins() -> Self {
        let mut reg = Self::new();
        reg.register("OutputContains", |params| {
            let pattern = match params.get("pattern") {
                Some(BlackboardValue::String(s)) => s.clone(),
                _ => return Err(RuntimeError::FilterError(
                    "OutputContains requires 'pattern' param".into(),
                )),
            };
            let case_sensitive = match params.get("caseSensitive") {
                Some(BlackboardValue::Boolean(b)) => *b,
                _ => false,
            };
            Ok(Evaluator::OutputContains { pattern, case_sensitive })
        });
        reg.register("SituationIs", |params| {
            let situation_type = match params.get("situationType") {
                Some(BlackboardValue::String(s)) => s.clone(),
                _ => return Err(RuntimeError::FilterError(
                    "SituationIs requires 'situationType' param".into(),
                )),
            };
            Ok(Evaluator::SituationIs { situation_type })
        });
        reg.register("ReflectionRoundUnder", |params| {
            let max = match params.get("max") {
                Some(BlackboardValue::Integer(i)) => *i as u8,
                _ => return Err(RuntimeError::FilterError(
                    "ReflectionRoundUnder requires 'max' param".into(),
                )),
            };
            Ok(Evaluator::ReflectionRoundUnder { max })
        });
        reg.register("VariableIs", |params| {
            let key = match params.get("key") {
                Some(BlackboardValue::String(s)) => s.clone(),
                _ => return Err(RuntimeError::FilterError(
                    "VariableIs requires 'key' param".into(),
                )),
            };
            let expected = match params.get("expected") {
                Some(v) => v.clone(),
                _ => return Err(RuntimeError::FilterError(
                    "VariableIs requires 'expected' param".into(),
                )),
            };
            Ok(Evaluator::VariableIs { key, expected })
        });
        reg.register("RegexMatch", |params| {
            let pattern = match params.get("pattern") {
                Some(BlackboardValue::String(s)) => s.clone(),
                _ => return Err(RuntimeError::FilterError(
                    "RegexMatch requires 'pattern' param".into(),
                )),
            };
            Ok(Evaluator::RegexMatch { pattern })
        });
        reg
    }

    pub fn register<F>(&mut self, name: &str, factory: F)
    where
        F: Fn(&HashMap<String, BlackboardValue>) -> Result<Evaluator, RuntimeError> + Send + Sync + 'static,
    {
        self.builtins.insert(name.into(), Box::new(factory));
    }

    pub fn create(
        &self,
        name: &str,
        params: &[(String, BlackboardValue)],
    ) -> Option<Evaluator> {
        let factory = self.builtins.get(name)?;
        let map: HashMap<String, BlackboardValue> = params.iter().cloned().collect();
        factory(&map).ok()
    }
}
