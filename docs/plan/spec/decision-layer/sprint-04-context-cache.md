# Sprint 4: Context Cache

## Metadata

- Sprint ID: `decision-sprint-004`
- Title: `Context Cache`
- Duration: 1 week
- Priority: P1 (High)
- Status: `Backlog`
- Created: 2026-04-14

## TDD Reference

See [Test Specification](test-specification.md) for detailed TDD test tasks:
- Sprint 4 Tests: T4.1.T1-T4.3.T4 (16 tests)
- Test execution: Write failing tests first, implement minimum, refactor

## Sprint Goal

Implement RunningContextCache with size limits, compression strategies, and priority retention to control prompt size for decision engines.

## Stories

### Story 4.1: RunningContextCache with Size Limits

**Priority**: P1
**Effort**: 3 points
**Status**: Backlog

Implement context cache with configurable size limits.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T4.1.1 | Create `RunningContextCache` with VecDeque fields | Todo | - |
| T4.1.2 | Implement size limit configuration | Todo | - |
| T4.1.3 | Implement `add_tool_call()` with overflow handling | Todo | - |
| T4.1.4 | Implement `add_file_change()` with overflow handling | Todo | - |
| T4.1.5 | Implement `add_thinking()` with rolling summary | Todo | - |
| T4.1.6 | Implement `add_key_output()` with overflow handling | Todo | - |
| T4.1.7 | Implement `estimate_size()` method | Todo | - |
| T4.1.8 | Write unit tests for size limits | Todo | - |

#### Acceptance Criteria

- Cache respects size limits
- Overflow removes oldest entries
- Size estimation accurate

#### Technical Notes

```rust
/// Running context cache with size limits
pub struct RunningContextCache {
    /// Tool call records (max N entries)
    tool_calls: VecDeque<ToolCallRecord>,
    max_tool_calls: usize,  // Default: 50
    
    /// File change records (max N entries)
    file_changes: VecDeque<FileChangeRecord>,
    max_file_changes: usize,  // Default: 30
    
    /// Thinking summary (rolling)
    thinking_summary: Option<String>,
    max_thinking_length: usize,  // Default: 1000
    
    /// Key outputs (max N entries)
    key_outputs: VecDeque<String>,
    max_key_outputs: usize,  // Default: 20
    
    /// Total size limit in bytes
    max_total_bytes: usize,  // Default: 10240 (10KB)
    
    /// Current estimated size
    current_size: usize,
}

impl RunningContextCache {
    pub fn new(config: &DecisionAgentConfig) -> Self {
        Self {
            tool_calls: VecDeque::with_capacity(50),
            max_tool_calls: 50,
            file_changes: VecDeque::with_capacity(30),
            max_file_changes: 30,
            thinking_summary: None,
            max_thinking_length: 1000,
            key_outputs: VecDeque::with_capacity(20),
            max_key_outputs: 20,
            max_total_bytes: config.context_cache_max_bytes,
            current_size: 0,
        }
    }
    
    pub fn add_tool_call(&mut self, record: ToolCallRecord) {
        // Estimate entry size
        let entry_size = self.estimate_tool_call_size(&record);
        
        // Remove oldest if at limit
        while self.tool_calls.len() >= self.max_tool_calls || 
              self.current_size + entry_size > self.max_total_bytes {
            if let Some(old) = self.tool_calls.pop_front() {
                self.current_size -= self.estimate_tool_call_size(&old);
            }
        }
        
        self.tool_calls.push_back(record);
        self.current_size += entry_size;
    }
    
    pub fn add_file_change(&mut self, record: FileChangeRecord) {
        let entry_size = self.estimate_file_change_size(&record);
        
        while self.file_changes.len() >= self.max_file_changes ||
              self.current_size + entry_size > self.max_total_bytes {
            if let Some(old) = self.file_changes.pop_front() {
                self.current_size -= self.estimate_file_change_size(&old);
            }
        }
        
        self.file_changes.push_back(record);
        self.current_size += entry_size;
    }
    
    pub fn add_thinking(&mut self, thinking: String) {
        // Rolling summary: append new thinking, truncate if over limit
        let current = self.thinking_summary.take().unwrap_or_default();
        let combined = if current.is_empty() {
            thinking
        } else {
            format!("{} | {}", current, thinking)
        };
        
        if combined.len() > self.max_thinking_length {
            // Truncate to last N characters
            self.thinking_summary = Some(combined.chars()
                .skip(combined.len() - self.max_thinking_length)
                .collect());
        } else {
            self.thinking_summary = Some(combined);
        }
        
        // Recalculate current size
        self.recalculate_size();
    }
    
    pub fn add_key_output(&mut self, output: String) {
        let entry_size = output.len();
        
        while self.key_outputs.len() >= self.max_key_outputs ||
              self.current_size + entry_size > self.max_total_bytes {
            if let Some(old) = self.key_outputs.pop_front() {
                self.current_size -= old.len();
            }
        }
        
        self.key_outputs.push_back(output);
        self.current_size += entry_size;
    }
    
    fn estimate_tool_call_size(&self, record: &ToolCallRecord) -> usize {
        // Rough estimate: name + input_preview + output_preview
        record.name.len() + 
        record.input_preview.map(|s| s.len()).unwrap_or(0) +
        record.output_preview.map(|s| s.len()).unwrap_or(0) +
        50 // Fixed overhead
    }
    
    fn estimate_file_change_size(&self, record: &FileChangeRecord) -> usize {
        record.path.len() +
        record.diff_preview.map(|s| s.len()).unwrap_or(0) +
        20 // Fixed overhead
    }
    
    fn recalculate_size(&mut self) {
        self.current_size = 0;
        
        for tc in &self.tool_calls {
            self.current_size += self.estimate_tool_call_size(tc);
        }
        
        for fc in &self.file_changes {
            self.current_size += self.estimate_file_change_size(fc);
        }
        
        self.current_size += self.thinking_summary.map(|s| s.len()).unwrap_or(0);
        
        for ko in &self.key_outputs {
            self.current_size += ko.len();
        }
    }
}
```

---

### Story 4.2: Context Compression and Priority Retention

**Priority**: P1
**Effort**: 3 points
**Status**: Backlog

Implement compression strategy when cache exceeds limits.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T4.2.1 | Create `InfoPriority` enum | Todo | - |
| T4.2.2 | Implement `compress()` method | Todo | - |
| T4.2.3 | Implement `is_key_info()` detection | Todo | - |
| T4.2.4 | Implement `generate_summary()` method | Todo | - |
| T4.2.5 | Implement priority-based retention | Todo | - |
| T4.2.6 | Write unit tests for compression | Todo | - |

#### Acceptance Criteria

- Compression reduces size below limit
- High-priority info retained
- Summary generation useful for decisions

#### Technical Notes

```rust
/// Information priority for retention decisions
pub enum InfoPriority {
    /// High: Keep all or compress to summary
    High,
    
    /// Medium: Rolling summary
    Medium,
    
    /// Low: Keep last N entries
    Low,
}

impl RunningContextCache {
    /// Compress cache to fit size limit
    pub fn compress(&mut self) {
        while self.current_size > self.max_total_bytes {
            // Strategy: remove low priority first, then medium, then compress high
            
            // 1. Remove excess low priority entries (older tool calls)
            while self.tool_calls.len() > 10 && self.current_size > self.max_total_bytes {
                if let Some(old) = self.tool_calls.pop_front() {
                    self.current_size -= self.estimate_tool_call_size(&old);
                }
            }
            
            // 2. Compress medium priority (thinking summary)
            if self.current_size > self.max_total_bytes {
                self.compress_thinking();
            }
            
            // 3. Compress high priority (file changes to diff summary)
            if self.current_size > self.max_total_bytes {
                self.compress_file_changes();
            }
        }
    }
    
    fn compress_thinking(&mut self) {
        // Truncate thinking to half of max length
        if let Some(thinking) = &self.thinking_summary {
            if thinking.len() > self.max_thinking_length / 2 {
                self.thinking_summary = Some(thinking.chars()
                    .skip(thinking.len() - self.max_thinking_length / 2)
                    .collect());
                self.recalculate_size();
            }
        }
    }
    
    fn compress_file_changes(&mut self) {
        // Keep file paths, remove diff previews
        for fc in &mut self.file_changes {
            fc.diff_preview = None;
        }
        self.recalculate_size();
    }
    
    /// Generate compact summary for LLM prompt
    pub fn generate_summary(&self) -> String {
        // Format for LLM consumption
        let files = self.file_changes.iter()
            .map(|fc| format!("{} ({})", fc.path, fc.change_type))
            .collect::<Vec<_>>();
        
        let tool_stats = self.tool_calls.iter()
            .group_by(|tc| tc.name.as_str())
            .map(|(name, group)| format!("{}: {} calls", name, group.count()))
            .collect::<Vec<_>>();
        
        let recent_keys = self.key_outputs.iter().rev().take(3)
            .collect::<Vec<_>>();
        
        format!(
            "File changes: {}\n\
             Tool call stats: {}\n\
             Recent key outputs: {}",
            files.join(", "),
            tool_stats.join(", "),
            recent_keys.join("\n  ")
        )
    }
    
    /// Determine if event is key information
    pub fn is_key_info(event: &ProviderEvent) -> bool {
        match event {
            // File modification = key
            ProviderEvent::PatchApplyFinished { .. } => true,
            
            // Error = key
            ProviderEvent::ExecCommandFinished { exit_code, .. } if exit_code != 0 => true,
            
            // Decision-related text = key
            ProviderEvent::AssistantChunk { text } 
                if text.contains("选择") || 
                   text.contains("完成") || 
                   text.contains("choice") ||
                   text.contains("complete") => true,
            
            // Other = ordinary
            _ => false,
        }
    }
}
```

**Priority Retention Matrix**:

| Info Type | Priority | Retention Strategy |
|-----------|----------|-------------------|
| File changes | High | Keep all paths, optional diff preview |
| Errors | High | Keep last 3 with full content |
| Decision points | High | Keep all |
| Thinking | Medium | Rolling summary |
| Tool calls | Low | Keep last 10, stats summary |
| Regular output | Low | Summary only |

---

### Story 4.3: Context Persistence and Recovery

**Priority**: P1
**Effort**: 2 points
**Status**: Backlog

Implement context persistence for recovery after restart.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T4.3.1 | Create `ContextCacheFile` struct for persistence | Todo | - |
| T4.3.2 | Implement `persist()` method | Todo | - |
| T4.3.3 | Implement `restore()` method | Todo | - |
| T4.3.4 | Handle truncation on restore if limits changed | Todo | - |
| T4.3.5 | Write unit tests for persistence | Todo | - |

#### Acceptance Criteria

- Context persists to JSON file
- Context restores correctly
- Truncation handles limit changes

#### Technical Notes

```rust
/// Context cache persistence format
#[derive(Serialize, Deserialize)]
pub struct ContextCacheFile {
    pub tool_calls: Vec<ToolCallRecord>,
    pub file_changes: Vec<FileChangeRecord>,
    pub thinking_summary: Option<String>,
    pub key_outputs: Vec<String>,
}

impl RunningContextCache {
    pub fn persist(&self, path: &Path) -> Result<()> {
        let file = ContextCacheFile {
            tool_calls: self.tool_calls.iter().cloned().collect(),
            file_changes: self.file_changes.iter().cloned().collect(),
            thinking_summary: self.thinking_summary.clone(),
            key_outputs: self.key_outputs.iter().cloned().collect(),
        };
        
        let json = serde_json::to_string(&file)?;
        std::fs::write(path, json)?;
        
        Ok(())
    }
    
    pub fn restore(path: &Path, config: &DecisionAgentConfig) -> Result<Self> {
        let json = std::fs::read_to_string(path)?;
        let file: ContextCacheFile = serde_json::from_str(&json)?;
        
        let mut cache = Self::new(config);
        
        // Restore entries, respecting current limits
        for tc in file.tool_calls {
            cache.add_tool_call(tc);
        }
        
        for fc in file.file_changes {
            cache.add_file_change(fc);
        }
        
        cache.thinking_summary = file.thinking_summary;
        
        for ko in file.key_outputs {
            cache.add_key_output(ko);
        }
        
        cache.recalculate_size();
        
        Ok(cache)
    }
}
```

**Persistence Path**:

```
~/.agile-agent/workplaces/{workplace_id}/agents/{agent_id}/decision/
└── context_cache.json    # RunningContextCache persistence
```

---

## Sprint Risks

| Risk | Probability | Impact | Mitigation |
|------|-------------|--------|------------|
| Size estimation inaccuracy | Medium | Low | Conservative estimates, recalculate |
| Compression losing key info | Low | Medium | Priority retention, key info detection |
| Persistence file corruption | Low | Medium | Backup on write, validation on restore |

## Sprint Deliverables

- `decision/src/context_cache.rs` - RunningContextCache implementation
- Unit tests for size limits, compression, persistence

## Dependencies

- Sprint 1: Core Types (ToolCallRecord, FileChangeRecord)
- Sprint 3: Decision Engine (uses context in prompts)

## Next Sprint

After completing this sprint, proceed to [Sprint 5: Lifecycle](./sprint-05-lifecycle.md) for Decision Agent creation and destruction.