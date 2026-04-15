//! Decision context and running context cache

use crate::output::DecisionRecord;
use crate::situation::DecisionSituation;
use crate::types::{ActionType, SituationType};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet, VecDeque};

/// Decision context - input to decision engine
pub struct DecisionContext {
    /// Current situation (trait reference)
    pub trigger_situation: Box<dyn DecisionSituation>,

    /// Parent main agent ID
    pub main_agent_id: String,

    /// Current task ID (if assigned)
    pub current_task_id: Option<String>,

    /// Current story ID (if assigned)
    pub current_story_id: Option<String>,

    /// Running context cache
    pub running_context: RunningContextCache,

    /// Project rules
    pub project_rules: ProjectRules,

    /// Decision history for this session
    pub decision_history: Vec<DecisionRecord>,
}

impl DecisionContext {
    pub fn new(
        situation: Box<dyn DecisionSituation>,
        main_agent_id: impl Into<String>,
    ) -> Self {
        Self {
            trigger_situation: situation,
            main_agent_id: main_agent_id.into(),
            current_task_id: None,
            current_story_id: None,
            running_context: RunningContextCache::default(),
            project_rules: ProjectRules::default(),
            decision_history: Vec::new(),
        }
    }

    pub fn with_task(self, task_id: impl Into<String>) -> Self {
        Self {
            current_task_id: Some(task_id.into()),
            ..self
        }
    }

    pub fn with_story(self, story_id: impl Into<String>) -> Self {
        Self {
            current_story_id: Some(story_id.into()),
            ..self
        }
    }

    /// Get situation type
    pub fn situation_type(&self) -> SituationType {
        self.trigger_situation.situation_type()
    }

    /// Check if project rule keyword is present
    pub fn contains_project_keyword(&self, keyword: &str) -> bool {
        self.project_rules.contains_keyword(keyword)
    }

    /// Add decision to history
    pub fn record_decision(&mut self, record: DecisionRecord) {
        self.decision_history.push(record);
    }
}

/// Running context cache - collects execution history
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunningContextCache {
    /// Tool call records (max N entries)
    pub tool_calls: VecDeque<ToolCallRecord>,

    /// File change records (max N entries)
    pub file_changes: VecDeque<FileChangeRecord>,

    /// Thinking summary (rolling)
    pub thinking_summary: Option<String>,

    /// Key outputs (max N entries)
    pub key_outputs: VecDeque<String>,

    /// Maximum entries per deque
    max_entries: usize,
}

impl RunningContextCache {
    pub fn new(max_entries: usize) -> Self {
        Self {
            tool_calls: VecDeque::with_capacity(max_entries),
            file_changes: VecDeque::with_capacity(max_entries),
            thinking_summary: None,
            key_outputs: VecDeque::with_capacity(max_entries),
            max_entries,
        }
    }

    /// Add tool call record
    pub fn add_tool_call(&mut self, record: ToolCallRecord) {
        if self.tool_calls.len() >= self.max_entries {
            self.tool_calls.pop_front();
        }
        self.tool_calls.push_back(record);
    }

    /// Add file change record
    pub fn add_file_change(&mut self, record: FileChangeRecord) {
        if self.file_changes.len() >= self.max_entries {
            self.file_changes.pop_front();
        }
        self.file_changes.push_back(record);
    }

    /// Add key output
    pub fn add_key_output(&mut self, output: impl Into<String>) {
        if self.key_outputs.len() >= self.max_entries {
            self.key_outputs.pop_front();
        }
        self.key_outputs.push_back(output.into());
    }

    /// Update thinking summary
    pub fn update_thinking_summary(&mut self, summary: impl Into<String>) {
        self.thinking_summary = Some(summary.into());
    }

    /// Clear cache
    pub fn clear(&mut self) {
        self.tool_calls.clear();
        self.file_changes.clear();
        self.thinking_summary = None;
        self.key_outputs.clear();
    }
}

impl Default for RunningContextCache {
    fn default() -> Self {
        Self::new(100)
    }
}

/// Tool call record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCallRecord {
    pub name: String,
    pub input_preview: Option<String>,
    pub output_preview: Option<String>,
    pub timestamp: DateTime<Utc>,
    pub success: bool,
}

impl ToolCallRecord {
    pub fn new(name: impl Into<String>, success: bool) -> Self {
        Self {
            name: name.into(),
            input_preview: None,
            output_preview: None,
            timestamp: Utc::now(),
            success,
        }
    }

    pub fn with_input_preview(self, preview: impl Into<String>) -> Self {
        Self {
            input_preview: Some(preview.into()),
            ..self
        }
    }

    pub fn with_output_preview(self, preview: impl Into<String>) -> Self {
        Self {
            output_preview: Some(preview.into()),
            ..self
        }
    }
}

/// File change record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileChangeRecord {
    pub path: String,
    pub change_type: ChangeType,
    pub diff_preview: Option<String>,
}

impl FileChangeRecord {
    pub fn new(path: impl Into<String>, change_type: ChangeType) -> Self {
        Self {
            path: path.into(),
            change_type,
            diff_preview: None,
        }
    }

    pub fn with_diff_preview(self, preview: impl Into<String>) -> Self {
        Self {
            diff_preview: Some(preview.into()),
            ..self
        }
    }
}

/// Change type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ChangeType {
    Created,
    Modified,
    Deleted,
}

/// Project rules from CLAUDE.md
#[derive(Debug, Clone, Default)]
pub struct ProjectRules {
    /// Raw content
    pub content: String,

    /// Extracted rules (key-value)
    pub rules: HashMap<String, String>,

    /// Keywords for rule matching
    pub keywords: HashSet<String>,

    /// Rules that require human confirmation
    pub requires_human_rules: Vec<String>,
}

impl ProjectRules {
    pub fn new(content: impl Into<String>) -> Self {
        Self {
            content: content.into(),
            rules: HashMap::new(),
            keywords: HashSet::new(),
            requires_human_rules: Vec::new(),
        }
    }

    pub fn with_rule(self, key: impl Into<String>, value: impl Into<String>) -> Self {
        let mut rules = self.rules;
        rules.insert(key.into(), value.into());
        Self { rules, ..self }
    }

    pub fn with_keyword(self, keyword: impl Into<String>) -> Self {
        let mut keywords = self.keywords;
        keywords.insert(keyword.into());
        Self { keywords, ..self }
    }

    pub fn with_requires_human_rule(self, rule: impl Into<String>) -> Self {
        let mut requires_human_rules = self.requires_human_rules;
        requires_human_rules.push(rule.into());
        Self { requires_human_rules, ..self }
    }

    pub fn contains_keyword(&self, keyword: &str) -> bool {
        self.keywords.contains(keyword)
    }

    pub fn requires_human_for(&self, action_type: &ActionType) -> bool {
        self.requires_human_rules
            .iter()
            .any(|r| r.contains(&action_type.name))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::builtin_situations::WaitingForChoiceSituation;
    use crate::situation::ChoiceOption;

    #[test]
    fn test_decision_context_new() {
        let situation = WaitingForChoiceSituation::default();
        let ctx = DecisionContext::new(Box::new(situation), "agent-1");
        assert_eq!(ctx.main_agent_id, "agent-1");
        assert_eq!(ctx.situation_type(), SituationType::new("waiting_for_choice"));
    }

    #[test]
    fn test_decision_context_with_task() {
        let situation = WaitingForChoiceSituation::default();
        let ctx = DecisionContext::new(Box::new(situation), "agent-1").with_task("task-1");
        assert_eq!(ctx.current_task_id, Some("task-1".to_string()));
    }

    #[test]
    fn test_running_context_cache_default() {
        let cache = RunningContextCache::default();
        assert!(cache.tool_calls.is_empty());
        assert!(cache.file_changes.is_empty());
        assert!(cache.key_outputs.is_empty());
    }

    #[test]
    fn test_running_context_cache_add_tool_call() {
        let mut cache = RunningContextCache::new(5);
        for i in 0..10 {
            cache.add_tool_call(ToolCallRecord::new(format!("tool-{}", i), true));
        }
        // Should only keep last 5
        assert_eq!(cache.tool_calls.len(), 5);
    }

    #[test]
    fn test_tool_call_record_timestamp() {
        let record = ToolCallRecord::new("test", true);
        assert!(record.timestamp <= Utc::now());
    }

    #[test]
    fn test_tool_call_record_with_preview() {
        let record = ToolCallRecord::new("test", true)
            .with_input_preview("input")
            .with_output_preview("output");
        assert_eq!(record.input_preview, Some("input".to_string()));
        assert_eq!(record.output_preview, Some("output".to_string()));
    }

    #[test]
    fn test_file_change_record() {
        let record = FileChangeRecord::new("/src/main.rs", ChangeType::Modified);
        assert_eq!(record.path, "/src/main.rs");
        assert_eq!(record.change_type, ChangeType::Modified);
    }

    #[test]
    fn test_project_rules_keyword() {
        let rules = ProjectRules::new("").with_keyword("TDD");
        assert!(rules.contains_keyword("TDD"));
        assert!(!rules.contains_keyword("unknown"));
    }

    #[test]
    fn test_project_rules_requires_human() {
        let rules = ProjectRules::new("").with_requires_human_rule("submit_pr");
        assert!(rules.requires_human_for(&ActionType::new("submit_pr")));
        assert!(!rules.requires_human_for(&ActionType::new("select_option")));
    }

    #[test]
    fn test_running_context_cache_serde() {
        let mut cache = RunningContextCache::new(10);
        cache.add_tool_call(ToolCallRecord::new("test", true));
        cache.add_key_output("output");

        let json = serde_json::to_string(&cache).unwrap();
        let parsed: RunningContextCache = serde_json::from_str(&json).unwrap();
        assert_eq!(cache.tool_calls.len(), parsed.tool_calls.len());
    }

    #[test]
    fn test_decision_context_record_decision() {
        let situation = WaitingForChoiceSituation::default();
        let mut ctx = DecisionContext::new(Box::new(situation), "agent-1");
        assert!(ctx.decision_history.is_empty());

        let record = DecisionRecord::new(
            "dec-1",
            SituationType::new("test"),
            &crate::output::DecisionOutput::new(vec![], "test"),
            crate::types::DecisionEngineType::Mock,
        );
        ctx.record_decision(record);
        assert_eq!(ctx.decision_history.len(), 1);
    }
}