use std::collections::HashMap;

use super::command::DecisionCommand;

// ── BlackboardValue ─────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub enum BlackboardValue {
    String(String),
    Integer(i64),
    Float(f64),
    Boolean(bool),
    List(Vec<BlackboardValue>),
    Map(HashMap<String, BlackboardValue>),
}

// ── Supporting types ────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct ToolCallRecord {
    pub name: String,
    pub input: String,
    pub output: String,
}

#[derive(Debug, Clone)]
pub struct FileChangeRecord {
    pub path: String,
    pub change_type: String,
}

#[derive(Debug, Clone, Default)]
pub struct ProjectRules {
    pub rules: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct DecisionRecord {
    pub situation: String,
    pub command: DecisionCommand,
    pub timestamp: String,
}

// ── Blackboard ──────────────────────────────────────────────────────────────

pub struct Blackboard {
    pub task_description: String,
    pub provider_output: String,
    pub context_summary: String,
    pub reflection_round: u8,
    pub max_reflection_rounds: u8,
    pub confidence_accumulator: f64,
    pub agent_id: String,
    pub current_task_id: String,
    pub current_story_id: String,
    pub last_tool_call: Option<ToolCallRecord>,
    pub file_changes: Vec<FileChangeRecord>,
    pub project_rules: ProjectRules,
    pub decision_history: Vec<DecisionRecord>,
    scopes: Vec<HashMap<String, BlackboardValue>>,
    pub commands: Vec<DecisionCommand>,
    pub llm_responses: HashMap<String, String>,
}

impl Default for Blackboard {
    fn default() -> Self {
        Self {
            task_description: String::new(),
            provider_output: String::new(),
            context_summary: String::new(),
            reflection_round: 0,
            max_reflection_rounds: 2,
            confidence_accumulator: 0.0,
            agent_id: String::new(),
            current_task_id: String::new(),
            current_story_id: String::new(),
            last_tool_call: None,
            file_changes: Vec::new(),
            project_rules: ProjectRules::default(),
            decision_history: Vec::new(),
            scopes: vec![HashMap::new()], // root scope
            commands: Vec::new(),
            llm_responses: HashMap::new(),
        }
    }
}

impl Blackboard {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_capacity(n: usize) -> Self {
        Self {
            scopes: vec![HashMap::with_capacity(n)],
            commands: Vec::with_capacity(8),
            ..Default::default()
        }
    }

    // --- Scope management ---

    pub fn push_scope(&mut self) {
        self.scopes.push(HashMap::new());
    }

    pub fn pop_scope(&mut self) {
        if self.scopes.len() > 1 {
            self.scopes.pop();
        }
    }

    // --- Variable access ---

    pub fn set(&mut self, key: &str, value: BlackboardValue) {
        if let Some(scope) = self.scopes.last_mut() {
            scope.insert(key.to_string(), value);
        }
    }

    pub fn get(&self, key: &str) -> Option<&BlackboardValue> {
        for scope in self.scopes.iter().rev() {
            if let Some(v) = scope.get(key) {
                return Some(v);
            }
        }
        None
    }

    // --- Path access ---

    pub fn get_path(&self, path: &str) -> Option<BlackboardValue> {
        let mut parts = path.split('.');
        let first = parts.next()?;

        let mut current = match first {
            "task_description" => Some(BlackboardValue::String(self.task_description.clone())),
            "provider_output" => Some(BlackboardValue::String(self.provider_output.clone())),
            "context_summary" => Some(BlackboardValue::String(self.context_summary.clone())),
            "reflection_round" => Some(BlackboardValue::Integer(self.reflection_round as i64)),
            "max_reflection_rounds" => {
                Some(BlackboardValue::Integer(self.max_reflection_rounds as i64))
            }
            "confidence_accumulator" => Some(BlackboardValue::Float(self.confidence_accumulator)),
            "agent_id" => Some(BlackboardValue::String(self.agent_id.clone())),
            "current_task_id" => Some(BlackboardValue::String(self.current_task_id.clone())),
            "current_story_id" => Some(BlackboardValue::String(self.current_story_id.clone())),
            "last_tool_call" => self.last_tool_call.as_ref().map(|t| {
                let mut m = HashMap::new();
                m.insert("name".into(), BlackboardValue::String(t.name.clone()));
                m.insert("input".into(), BlackboardValue::String(t.input.clone()));
                m.insert("output".into(), BlackboardValue::String(t.output.clone()));
                BlackboardValue::Map(m)
            }),
            "file_changes" => Some(BlackboardValue::List(
                self.file_changes
                    .iter()
                    .map(|fc| {
                        let mut m = HashMap::new();
                        m.insert("path".into(), BlackboardValue::String(fc.path.clone()));
                        m.insert(
                            "change_type".into(),
                            BlackboardValue::String(fc.change_type.clone()),
                        );
                        BlackboardValue::Map(m)
                    })
                    .collect(),
            )),
            "llm_responses" => Some(BlackboardValue::Map(
                self.llm_responses
                    .iter()
                    .map(|(k, v)| (k.clone(), BlackboardValue::String(v.clone())))
                    .collect(),
            )),
            _ => self.get(first).cloned(),
        };

        for part in parts {
            current = match current? {
                BlackboardValue::Map(m) => m.get(part).cloned(),
                BlackboardValue::List(l) => {
                    let idx: usize = part.parse().ok()?;
                    l.get(idx).cloned()
                }
                _ => None,
            };
        }

        current
    }

    // --- Typed getters ---

    pub fn get_string(&self, key: &str) -> Option<String> {
        self.get_path(key).and_then(|v| match v {
            BlackboardValue::String(s) => Some(s),
            _ => None,
        })
    }

    pub fn get_bool(&self, key: &str) -> Option<bool> {
        self.get_path(key).and_then(|v| match v {
            BlackboardValue::Boolean(b) => Some(b),
            _ => None,
        })
    }

    pub fn get_u8(&self, key: &str) -> Option<u8> {
        self.get_path(key).and_then(|v| match v {
            BlackboardValue::Integer(i) => i.try_into().ok(),
            _ => None,
        })
    }

    pub fn get_f64(&self, key: &str) -> Option<f64> {
        self.get_path(key).and_then(|v| match v {
            BlackboardValue::Float(f) => Some(f),
            BlackboardValue::Integer(i) => Some(i as f64),
            _ => None,
        })
    }

    // --- Typed setters ---

    pub fn set_string(&mut self, key: &str, value: String) {
        self.set(key, BlackboardValue::String(value));
    }

    pub fn set_u8(&mut self, key: &str, value: u8) {
        self.set(key, BlackboardValue::Integer(value as i64));
    }

    pub fn set_f64(&mut self, key: &str, value: f64) {
        self.set(key, BlackboardValue::Float(value));
    }

    pub fn set_bool(&mut self, key: &str, value: bool) {
        self.set(key, BlackboardValue::Boolean(value));
    }

    // --- Command management ---

    pub fn push_command(&mut self, cmd: DecisionCommand) {
        self.commands.push(cmd);
    }

    pub fn drain_commands(&mut self) -> Vec<DecisionCommand> {
        std::mem::take(&mut self.commands)
    }

    // --- LLM responses ---

    pub fn store_llm_response(&mut self, key: &str, value: &str) {
        self.llm_responses.insert(key.to_string(), value.to_string());
    }
}
