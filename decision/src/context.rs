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

    /// Metadata for engine state synchronization (e.g., reflection_round)
    pub metadata: HashMap<String, String>,
}

impl Clone for DecisionContext {
    fn clone(&self) -> Self {
        Self {
            trigger_situation: self.trigger_situation.clone_boxed(),
            main_agent_id: self.main_agent_id.clone(),
            current_task_id: self.current_task_id.clone(),
            current_story_id: self.current_story_id.clone(),
            running_context: self.running_context.clone(),
            project_rules: self.project_rules.clone(),
            decision_history: self.decision_history.clone(),
            metadata: self.metadata.clone(),
        }
    }
}

impl DecisionContext {
    pub fn new(situation: Box<dyn DecisionSituation>, main_agent_id: impl Into<String>) -> Self {
        Self {
            trigger_situation: situation,
            main_agent_id: main_agent_id.into(),
            current_task_id: None,
            current_story_id: None,
            running_context: RunningContextCache::default(),
            project_rules: ProjectRules::default(),
            decision_history: Vec::new(),
            metadata: HashMap::new(),
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

    /// Set reflection round in metadata for engine synchronization
    pub fn with_reflection_round(self, round: u8) -> Self {
        let mut metadata = self.metadata;
        metadata.insert("reflection_round".to_string(), round.to_string());
        Self { metadata, ..self }
    }

    /// Set metadata value
    pub fn with_metadata(self, key: impl Into<String>, value: impl Into<String>) -> Self {
        let mut metadata = self.metadata;
        metadata.insert(key.into(), value.into());
        Self { metadata, ..self }
    }

    /// Get reflection round from metadata
    pub fn reflection_round(&self) -> u8 {
        self.metadata
            .get("reflection_round")
            .and_then(|s| s.parse::<u8>().ok())
            .unwrap_or(0)
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

/// Running context cache - collects execution history with size limits
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

    /// Maximum total bytes
    max_total_bytes: usize,

    /// Current estimated size in bytes
    current_size: usize,
}

impl RunningContextCache {
    pub fn new(max_entries: usize) -> Self {
        Self {
            tool_calls: VecDeque::with_capacity(max_entries),
            file_changes: VecDeque::with_capacity(max_entries),
            thinking_summary: None,
            key_outputs: VecDeque::with_capacity(max_entries),
            max_entries,
            max_total_bytes: 10240, // 10KB default
            current_size: 0,
        }
    }

    pub fn with_max_bytes(self, max_bytes: usize) -> Self {
        Self {
            max_total_bytes: max_bytes,
            ..self
        }
    }

    /// Estimate tool call size
    fn estimate_tool_call_size(&self, record: &ToolCallRecord) -> usize {
        record.name.len()
            + record.input_preview.as_ref().map(|s| s.len()).unwrap_or(0)
            + record.output_preview.as_ref().map(|s| s.len()).unwrap_or(0)
            + 50 // Fixed overhead
    }

    /// Estimate file change size
    fn estimate_file_change_size(&self, record: &FileChangeRecord) -> usize {
        record.path.len() + record.diff_preview.as_ref().map(|s| s.len()).unwrap_or(0) + 20 // Fixed overhead
    }

    /// Recalculate current size
    fn recalculate_size(&mut self) {
        self.current_size = 0;
        for tc in &self.tool_calls {
            self.current_size += self.estimate_tool_call_size(tc);
        }
        for fc in &self.file_changes {
            self.current_size += self.estimate_file_change_size(fc);
        }
        self.current_size += self.thinking_summary.as_ref().map(|s| s.len()).unwrap_or(0);
        for ko in &self.key_outputs {
            self.current_size += ko.len();
        }
    }

    /// Add tool call record
    pub fn add_tool_call(&mut self, record: ToolCallRecord) {
        let entry_size = self.estimate_tool_call_size(&record);

        // If single entry exceeds limit, truncate it
        if entry_size > self.max_total_bytes {
            // Skip adding oversized entries
            return;
        }

        // Remove oldest if at limit
        while self.tool_calls.len() >= self.max_entries
            || self.current_size + entry_size > self.max_total_bytes
        {
            if let Some(old) = self.tool_calls.pop_front() {
                self.current_size -= self.estimate_tool_call_size(&old);
            } else {
                break; // No more to remove
            }
        }

        self.tool_calls.push_back(record);
        self.current_size += entry_size;
    }

    /// Add file change record
    pub fn add_file_change(&mut self, record: FileChangeRecord) {
        let entry_size = self.estimate_file_change_size(&record);

        // If single entry exceeds limit, truncate it
        if entry_size > self.max_total_bytes {
            // Remove diff preview and re-estimate
            let truncated = FileChangeRecord::new(record.path.clone(), record.change_type);
            let truncated_size = self.estimate_file_change_size(&truncated);
            if truncated_size > self.max_total_bytes {
                return; // Skip oversized
            }
            self.file_changes.push_back(truncated);
            self.current_size += truncated_size;
            return;
        }

        while self.file_changes.len() >= self.max_entries
            || self.current_size + entry_size > self.max_total_bytes
        {
            if let Some(old) = self.file_changes.pop_front() {
                self.current_size -= self.estimate_file_change_size(&old);
            } else {
                break;
            }
        }

        self.file_changes.push_back(record);
        self.current_size += entry_size;
    }

    /// Add key output
    pub fn add_key_output(&mut self, output: impl Into<String>) {
        let output_str = output.into();
        let entry_size = output_str.len();

        // If single entry exceeds limit, truncate it
        if entry_size > self.max_total_bytes {
            let truncated_len = self.max_total_bytes - 10;
            let truncated: String = output_str.chars().take(truncated_len).collect();
            let truncated_size = truncated.len();
            self.key_outputs.push_back(truncated);
            self.current_size = truncated_size;
            return;
        }

        while self.key_outputs.len() >= self.max_entries
            || self.current_size + entry_size > self.max_total_bytes
        {
            if let Some(old) = self.key_outputs.pop_front() {
                self.current_size -= old.len();
            } else {
                break;
            }
        }

        self.key_outputs.push_back(output_str);
        self.current_size += entry_size;
    }

    /// Update thinking summary
    pub fn update_thinking_summary(&mut self, summary: impl Into<String>) {
        let new_summary = summary.into();
        let old_size = self.thinking_summary.as_ref().map(|s| s.len()).unwrap_or(0);

        // Truncate if exceeds half of max bytes
        let max_thinking_len = self.max_total_bytes / 4;
        let truncated = if new_summary.len() > max_thinking_len {
            new_summary
                .chars()
                .skip(new_summary.len() - max_thinking_len)
                .collect::<String>()
        } else {
            new_summary
        };

        let new_size = truncated.len();
        self.thinking_summary = Some(truncated);
        self.current_size = self.current_size - old_size + new_size;
    }

    /// Clear cache
    pub fn clear(&mut self) {
        self.tool_calls.clear();
        self.file_changes.clear();
        self.thinking_summary = None;
        self.key_outputs.clear();
        self.current_size = 0;
    }

    /// Compress cache to fit size limit
    pub fn compress(&mut self) {
        while self.current_size > self.max_total_bytes {
            // Remove excess tool calls (keep last 10)
            while self.tool_calls.len() > 10 && self.current_size > self.max_total_bytes {
                if let Some(old) = self.tool_calls.pop_front() {
                    self.current_size -= self.estimate_tool_call_size(&old);
                }
            }

            // Compress thinking
            if self.current_size > self.max_total_bytes {
                if let Some(thinking) = &self.thinking_summary {
                    if thinking.len() > 100 {
                        let compressed = thinking.chars().skip(thinking.len() - 100).collect();
                        self.thinking_summary = Some(compressed);
                        self.recalculate_size();
                    }
                }
            }

            // Remove diff previews from file changes
            if self.current_size > self.max_total_bytes {
                for fc in &mut self.file_changes {
                    if let Some(diff) = &fc.diff_preview {
                        self.current_size -= diff.len();
                        fc.diff_preview = None;
                    }
                }
            }

            // Remove oldest key outputs
            while self.key_outputs.len() > 5 && self.current_size > self.max_total_bytes {
                if let Some(old) = self.key_outputs.pop_front() {
                    self.current_size -= old.len();
                }
            }
        }
    }

    /// Generate compact summary for LLM prompt
    pub fn generate_summary(&self) -> String {
        let files = self
            .file_changes
            .iter()
            .map(|fc| format!("{} ({})", fc.path, fc.change_type))
            .collect::<Vec<_>>();

        let tool_stats: HashMap<&str, usize> =
            self.tool_calls.iter().fold(HashMap::new(), |mut acc, tc| {
                *acc.entry(tc.name.as_str()).or_insert(0) += 1;
                acc
            });

        let tool_summary = tool_stats
            .iter()
            .map(|(name, count)| format!("{}: {} calls", name, count))
            .collect::<Vec<_>>();

        let recent_keys = self.key_outputs.iter().rev().take(3).collect::<Vec<_>>();

        format!(
            "Files: {}\n\
             Tools: {}\n\
             Recent: {}",
            files.join(", "),
            tool_summary.join(", "),
            recent_keys
                .iter()
                .map(|s| s.as_str())
                .collect::<Vec<_>>()
                .join("\n  ")
        )
    }

    /// Summary alias for generate_summary
    pub fn summary(&self) -> String {
        self.generate_summary()
    }

    /// Get current estimated size
    pub fn size_estimate(&self) -> usize {
        self.current_size
    }

    /// Check if size is within limits
    pub fn is_within_limits(&self) -> bool {
        self.current_size <= self.max_total_bytes
    }

    /// Get tool calls
    pub fn tool_calls(&self) -> &VecDeque<ToolCallRecord> {
        &self.tool_calls
    }

    /// Get file changes
    pub fn file_changes(&self) -> &VecDeque<FileChangeRecord> {
        &self.file_changes
    }

    /// Get thinking summary
    pub fn thinking_summary(&self) -> Option<&String> {
        self.thinking_summary.as_ref()
    }

    /// Get key outputs
    pub fn key_outputs(&self) -> &VecDeque<String> {
        &self.key_outputs
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

impl std::fmt::Display for ChangeType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ChangeType::Created => write!(f, "created"),
            ChangeType::Modified => write!(f, "modified"),
            ChangeType::Deleted => write!(f, "deleted"),
        }
    }
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
        Self {
            requires_human_rules,
            ..self
        }
    }

    pub fn contains_keyword(&self, keyword: &str) -> bool {
        self.keywords.contains(keyword)
    }

    /// Get a summary of project rules
    pub fn summary(&self) -> String {
        if self.rules.is_empty() {
            "No project rules defined".to_string()
        } else {
            self.rules
                .iter()
                .map(|(k, v)| format!("{}: {}", k, v))
                .collect::<Vec<_>>()
                .join("\n")
        }
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

    #[test]
    fn test_decision_context_new() {
        let situation = WaitingForChoiceSituation::default();
        let ctx = DecisionContext::new(Box::new(situation), "agent-1");
        assert_eq!(ctx.main_agent_id, "agent-1");
        assert_eq!(
            ctx.situation_type(),
            SituationType::new("waiting_for_choice")
        );
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

    #[test]
    fn test_running_context_cache_size_estimate() {
        let mut cache = RunningContextCache::new(100);
        cache.add_tool_call(ToolCallRecord::new("Bash", true).with_input_preview("ls -la"));
        assert!(cache.size_estimate() > 0);
        assert!(cache.is_within_limits());
    }

    #[test]
    fn test_running_context_cache_byte_limit() {
        let mut cache = RunningContextCache::new(100).with_max_bytes(100);
        // Add large output - should be truncated
        cache.add_key_output("x".repeat(200));
        // Size should be within limits
        assert!(cache.size_estimate() <= 100);
    }

    #[test]
    fn test_running_context_cache_compress() {
        let mut cache = RunningContextCache::new(100).with_max_bytes(1000);
        // Add many tool calls - size should stay within limit during addition
        for i in 0..50 {
            cache.add_tool_call(
                ToolCallRecord::new(format!("tool-{}", i), true)
                    .with_input_preview("x".repeat(100)),
            );
        }
        // Size should be limited during addition
        assert!(cache.is_within_limits());
        // Compress should still work if somehow size exceeded
        cache.compress();
        assert!(cache.is_within_limits());
    }

    #[test]
    fn test_running_context_cache_generate_summary() {
        let mut cache = RunningContextCache::new(100);
        cache.add_file_change(FileChangeRecord::new("/src/main.rs", ChangeType::Modified));
        cache.add_tool_call(ToolCallRecord::new("Bash", true));
        cache.add_tool_call(ToolCallRecord::new("Bash", true));
        cache.add_key_output("completed successfully");

        let summary = cache.generate_summary();
        assert!(summary.contains("/src/main.rs"));
        assert!(summary.contains("Bash: 2 calls"));
    }

    #[test]
    fn test_change_type_display() {
        assert_eq!(format!("{}", ChangeType::Created), "created");
        assert_eq!(format!("{}", ChangeType::Modified), "modified");
        assert_eq!(format!("{}", ChangeType::Deleted), "deleted");
    }

    #[test]
    fn test_running_context_cache_thinking_truncate() {
        let mut cache = RunningContextCache::new(100).with_max_bytes(1000);
        cache.update_thinking_summary("x".repeat(500));
        cache.update_thinking_summary("y".repeat(500));
        // Should truncate to max_total_bytes/4 = 250
        let len = cache.thinking_summary().map(|s| s.len()).unwrap_or(0);
        assert!(len <= 250);
    }
}
