use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;
use std::time::Instant;

use super::command::DecisionCommand;

/// Maximum depth for scope nesting to prevent stack overflow from bugs or malicious input.
const MAX_SCOPE_DEPTH: usize = 64;

/// Error type for scope operations.
#[derive(Debug, Clone, PartialEq)]
pub enum ScopeError {
    MaxDepthExceeded,
}

impl fmt::Display for ScopeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ScopeError::MaxDepthExceeded => write!(f, "maximum scope depth exceeded"),
        }
    }
}

impl std::error::Error for ScopeError {}

// ── BlackboardValue ─────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum BlackboardValue {
    String(String),
    Integer(i64),
    Float(f64),
    Boolean(bool),
    List(Vec<BlackboardValue>),
    Map(HashMap<String, BlackboardValue>),
    Command(DecisionCommand),
    Null,
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

// ── Decision Flow Types ─────────────────────────────────────────────────────

/// Entry in the reflection chain tracking AI judgment history.
#[derive(Debug, Clone)]
pub struct ReflectionEntry {
    /// Sprint number this reflection belongs to
    pub sprint: u8,
    /// AI decision result (proceed/retry/escalate)
    pub result: String,
    /// AI reasoning for the decision
    pub reasoning: String,
    /// When this reflection was made
    pub timestamp: Instant,
}

impl ReflectionEntry {
    pub fn new(sprint: u8, result: impl Into<String>, reasoning: impl Into<String>) -> Self {
        Self {
            sprint,
            result: result.into(),
            reasoning: reasoning.into(),
            timestamp: Instant::now(),
        }
    }
}

/// Entry in the decision chain tracking behavior tree node outcomes.
#[derive(Debug, Clone)]
pub struct DecisionEntry {
    /// Node name that made the decision
    pub node_name: String,
    /// AI decision output (from PromptNode JSON)
    pub decision: String,
    /// Outcome of executing this decision
    pub outcome: String,
}

impl DecisionEntry {
    pub fn new(node_name: impl Into<String>, decision: impl Into<String>, outcome: impl Into<String>) -> Self {
        Self {
            node_name: node_name.into(),
            decision: decision.into(),
            outcome: outcome.into(),
        }
    }
}

/// Sprint goal definition for multi-sprint workflows.
#[derive(Debug, Clone)]
pub struct SprintGoal {
    pub sprint_number: u8,
    pub description: String,
}

impl SprintGoal {
    pub fn new(sprint_number: u8, description: impl Into<String>) -> Self {
        Self {
            sprint_number,
            description: description.into(),
        }
    }
}

// ── Blackboard ──────────────────────────────────────────────────────────────

pub struct Blackboard {
    // --- Work Agent context ---
    pub task_description: String,
    pub provider_output: String,
    pub context_summary: String,
    pub last_tool_call: Option<ToolCallRecord>,
    pub file_changes: Vec<FileChangeRecord>,

    // --- Reflection tracking ---
    pub reflection_round: u8,
    pub max_reflection_rounds: u8,
    pub confidence_accumulator: f64,

    // --- Agent identification ---
    pub agent_id: String,
    pub work_agent_id: String,  // Target agent for SendInstruction commands
    pub current_task_id: String,
    pub current_story_id: String,

    // --- Sprint tracking ---
    pub current_sprint: u8,
    pub total_sprints: u8,
    pub sprint_goals: Vec<SprintGoal>,

    // --- Decision flow tracking ---
    pub reflection_chain: Vec<ReflectionEntry>,
    pub decision_chain: Vec<DecisionEntry>,

    // --- Project context ---
    pub project_rules: ProjectRules,
    pub decision_history: Vec<DecisionRecord>,

    // --- Variable storage ---
    scopes: Vec<HashMap<String, BlackboardValue>>,

    // --- Output ---
    pub commands: Vec<DecisionCommand>,
    pub llm_responses: HashMap<String, String>,
}

impl Default for Blackboard {
    fn default() -> Self {
        Self {
            task_description: String::new(),
            provider_output: String::new(),
            context_summary: String::new(),
            last_tool_call: None,
            file_changes: Vec::new(),
            reflection_round: 0,
            max_reflection_rounds: 2,
            confidence_accumulator: 0.0,
            agent_id: String::new(),
            work_agent_id: String::new(),
            current_task_id: String::new(),
            current_story_id: String::new(),
            current_sprint: 1,
            total_sprints: 1,
            sprint_goals: Vec::new(),
            reflection_chain: Vec::new(),
            decision_chain: Vec::new(),
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

    /// Push a new scope onto the stack.
    /// Returns an error if the maximum scope depth is exceeded.
    pub fn push_scope(&mut self) -> Result<(), ScopeError> {
        if self.scopes.len() >= MAX_SCOPE_DEPTH {
            return Err(ScopeError::MaxDepthExceeded);
        }
        self.scopes.push(HashMap::new());
        Ok(())
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
            // --- Work Agent context ---
            "task_description" => Some(BlackboardValue::String(self.task_description.clone())),
            "provider_output" => Some(BlackboardValue::String(self.provider_output.clone())),
            "context_summary" => Some(BlackboardValue::String(self.context_summary.clone())),
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

            // --- Reflection tracking ---
            "reflection_round" => Some(BlackboardValue::Integer(self.reflection_round as i64)),
            "max_reflection_rounds" => {
                Some(BlackboardValue::Integer(self.max_reflection_rounds as i64))
            }
            "confidence_accumulator" => Some(BlackboardValue::Float(self.confidence_accumulator)),

            // --- Agent identification ---
            "agent_id" => Some(BlackboardValue::String(self.agent_id.clone())),
            "work_agent_id" => Some(BlackboardValue::String(self.work_agent_id.clone())),
            "current_task_id" => Some(BlackboardValue::String(self.current_task_id.clone())),
            "current_story_id" => Some(BlackboardValue::String(self.current_story_id.clone())),

            // --- Sprint tracking ---
            "current_sprint" => Some(BlackboardValue::Integer(self.current_sprint as i64)),
            "total_sprints" => Some(BlackboardValue::Integer(self.total_sprints as i64)),
            "sprint_goals" => Some(BlackboardValue::List(
                self.sprint_goals
                    .iter()
                    .map(|sg| {
                        let mut m = HashMap::new();
                        m.insert("sprint_number".into(), BlackboardValue::Integer(sg.sprint_number as i64));
                        m.insert("description".into(), BlackboardValue::String(sg.description.clone()));
                        BlackboardValue::Map(m)
                    })
                    .collect(),
            )),
            "sprint_goal" => self.sprint_goals
                .iter()
                .find(|sg| sg.sprint_number == self.current_sprint)
                .map(|sg| BlackboardValue::String(sg.description.clone())),

            // --- Decision flow tracking ---
            "reflection_chain" => Some(BlackboardValue::List(
                self.reflection_chain
                    .iter()
                    .map(|r| {
                        let mut m = HashMap::new();
                        m.insert("sprint".into(), BlackboardValue::Integer(r.sprint as i64));
                        m.insert("result".into(), BlackboardValue::String(r.result.clone()));
                        m.insert("reasoning".into(), BlackboardValue::String(r.reasoning.clone()));
                        BlackboardValue::Map(m)
                    })
                    .collect(),
            )),
            "decision_chain" => Some(BlackboardValue::List(
                self.decision_chain
                    .iter()
                    .map(|d| {
                        let mut m = HashMap::new();
                        m.insert("node_name".into(), BlackboardValue::String(d.node_name.clone()));
                        m.insert("decision".into(), BlackboardValue::String(d.decision.clone()));
                        m.insert("outcome".into(), BlackboardValue::String(d.outcome.clone()));
                        BlackboardValue::Map(m)
                    })
                    .collect(),
            )),

            // --- LLM responses ---
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

    // --- Sprint management ---

    /// Set sprint configuration
    pub fn set_sprint_config(&mut self, total_sprints: u8, goals: Vec<SprintGoal>) {
        self.total_sprints = total_sprints;
        self.sprint_goals = goals;
    }

    /// Advance to next sprint
    pub fn advance_sprint(&mut self) {
        if self.current_sprint < self.total_sprints {
            self.current_sprint += 1;
        }
    }

    /// Check if all sprints completed
    pub fn is_all_sprints_completed(&self) -> bool {
        self.current_sprint > self.total_sprints
    }

    /// Get current sprint goal description
    pub fn current_sprint_goal(&self) -> Option<&str> {
        self.sprint_goals
            .iter()
            .find(|sg| sg.sprint_number == self.current_sprint)
            .map(|sg| sg.description.as_str())
    }

    // --- Reflection chain management ---

    /// Add reflection entry to the chain
    pub fn push_reflection(&mut self, entry: ReflectionEntry) {
        self.reflection_chain.push(entry);
    }

    /// Get reflections for a specific sprint
    pub fn reflections_for_sprint(&self, sprint: u8) -> Vec<&ReflectionEntry> {
        self.reflection_chain
            .iter()
            .filter(|r| r.sprint == sprint)
            .collect()
    }

    /// Get the most recent reflection
    pub fn last_reflection(&self) -> Option<&ReflectionEntry> {
        self.reflection_chain.last()
    }

    // --- Decision chain management ---

    /// Add decision entry to the chain
    pub fn push_decision(&mut self, entry: DecisionEntry) {
        self.decision_chain.push(entry);
    }

    /// Get decisions by node name
    pub fn decisions_by_node(&self, node_name: &str) -> Vec<&DecisionEntry> {
        self.decision_chain
            .iter()
            .filter(|d| d.node_name == node_name)
            .collect()
    }

    /// Get the most recent decision
    pub fn last_decision(&self) -> Option<&DecisionEntry> {
        self.decision_chain.last()
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

    /// Iterate over all scoped variables (outer to inner scope).
    pub fn iter_variables(&self) -> impl Iterator<Item = (&String, &BlackboardValue)> {
        self.scopes.iter().flat_map(|scope| scope.iter())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reflection_entry_new() {
        let entry = ReflectionEntry::new(1, "proceed", "all items completed");
        assert_eq!(entry.sprint, 1);
        assert_eq!(entry.result, "proceed");
        assert_eq!(entry.reasoning, "all items completed");
    }

    #[test]
    fn decision_entry_new() {
        let entry = DecisionEntry::new("reflect_node", "retry", "incomplete items");
        assert_eq!(entry.node_name, "reflect_node");
        assert_eq!(entry.decision, "retry");
        assert_eq!(entry.outcome, "incomplete items");
    }

    #[test]
    fn sprint_goal_new() {
        let goal = SprintGoal::new(1, "implement authentication");
        assert_eq!(goal.sprint_number, 1);
        assert_eq!(goal.description, "implement authentication");
    }

    #[test]
    fn blackboard_sprint_config() {
        let mut bb = Blackboard::new();
        let goals = vec![
            SprintGoal::new(1, "sprint 1 goal"),
            SprintGoal::new(2, "sprint 2 goal"),
        ];
        bb.set_sprint_config(2, goals);

        assert_eq!(bb.total_sprints, 2);
        assert_eq!(bb.sprint_goals.len(), 2);
        assert_eq!(bb.current_sprint, 1); // default
    }

    #[test]
    fn blackboard_advance_sprint() {
        let mut bb = Blackboard::new();
        bb.total_sprints = 3;
        bb.current_sprint = 1;

        bb.advance_sprint();
        assert_eq!(bb.current_sprint, 2);

        bb.advance_sprint();
        assert_eq!(bb.current_sprint, 3);

        // Should not exceed total
        bb.advance_sprint();
        assert_eq!(bb.current_sprint, 3);
    }

    #[test]
    fn blackboard_is_all_sprints_completed() {
        let mut bb = Blackboard::new();
        bb.total_sprints = 2;
        bb.current_sprint = 1;

        assert!(!bb.is_all_sprints_completed());

        bb.current_sprint = 2;
        assert!(!bb.is_all_sprints_completed());

        bb.current_sprint = 3;
        assert!(bb.is_all_sprints_completed());
    }

    #[test]
    fn blackboard_current_sprint_goal() {
        let mut bb = Blackboard::new();
        bb.sprint_goals = vec![
            SprintGoal::new(1, "first goal"),
            SprintGoal::new(2, "second goal"),
        ];
        bb.current_sprint = 1;

        assert_eq!(bb.current_sprint_goal(), Some("first goal"));

        bb.current_sprint = 2;
        assert_eq!(bb.current_sprint_goal(), Some("second goal"));

        bb.current_sprint = 3;
        assert_eq!(bb.current_sprint_goal(), None);
    }

    #[test]
    fn blackboard_push_reflection() {
        let mut bb = Blackboard::new();
        bb.push_reflection(ReflectionEntry::new(1, "proceed", "done"));
        bb.push_reflection(ReflectionEntry::new(2, "retry", "incomplete"));

        assert_eq!(bb.reflection_chain.len(), 2);
        assert_eq!(bb.last_reflection().unwrap().sprint, 2);
    }

    #[test]
    fn blackboard_reflections_for_sprint() {
        let mut bb = Blackboard::new();
        bb.push_reflection(ReflectionEntry::new(1, "proceed", "first"));
        bb.push_reflection(ReflectionEntry::new(1, "retry", "second"));
        bb.push_reflection(ReflectionEntry::new(2, "proceed", "third"));

        let sprint1 = bb.reflections_for_sprint(1);
        assert_eq!(sprint1.len(), 2);

        let sprint2 = bb.reflections_for_sprint(2);
        assert_eq!(sprint2.len(), 1);
    }

    #[test]
    fn blackboard_push_decision() {
        let mut bb = Blackboard::new();
        bb.push_decision(DecisionEntry::new("node1", "proceed", "success"));
        bb.push_decision(DecisionEntry::new("node2", "retry", "running"));

        assert_eq!(bb.decision_chain.len(), 2);
        assert_eq!(bb.last_decision().unwrap().node_name, "node2");
    }

    #[test]
    fn blackboard_decisions_by_node() {
        let mut bb = Blackboard::new();
        bb.push_decision(DecisionEntry::new("node1", "proceed", "first"));
        bb.push_decision(DecisionEntry::new("node2", "retry", "second"));
        bb.push_decision(DecisionEntry::new("node1", "proceed", "third"));

        let node1_decisions = bb.decisions_by_node("node1");
        assert_eq!(node1_decisions.len(), 2);

        let node2_decisions = bb.decisions_by_node("node2");
        assert_eq!(node2_decisions.len(), 1);
    }

    #[test]
    fn blackboard_get_path_sprint_fields() {
        let mut bb = Blackboard::new();
        bb.current_sprint = 2;
        bb.total_sprints = 4;
        bb.sprint_goals = vec![SprintGoal::new(2, "test goal")];

        assert_eq!(bb.get_u8("current_sprint"), Some(2));
        assert_eq!(bb.get_u8("total_sprints"), Some(4));

        let goal = bb.get_string("sprint_goal");
        assert_eq!(goal, Some("test goal".to_string()));
    }

    #[test]
    fn blackboard_get_path_reflection_chain() {
        let mut bb = Blackboard::new();
        bb.push_reflection(ReflectionEntry::new(1, "proceed", "test reasoning"));

        let chain = bb.get_path("reflection_chain");
        assert!(chain.is_some());

        let chain_list = chain.unwrap();
        if let BlackboardValue::List(list) = chain_list {
            assert_eq!(list.len(), 1);
        } else {
            panic!("expected list");
        }
    }

    #[test]
    fn blackboard_get_path_decision_chain() {
        let mut bb = Blackboard::new();
        bb.push_decision(DecisionEntry::new("test_node", "proceed", "success"));

        let chain = bb.get_path("decision_chain");
        assert!(chain.is_some());

        let chain_list = chain.unwrap();
        if let BlackboardValue::List(list) = chain_list {
            assert_eq!(list.len(), 1);
        } else {
            panic!("expected list");
        }
    }

    #[test]
    fn blackboard_work_agent_id() {
        let mut bb = Blackboard::new();
        bb.work_agent_id = "agent-123".to_string();

        assert_eq!(bb.get_string("work_agent_id"), Some("agent-123".to_string()));
    }
}
