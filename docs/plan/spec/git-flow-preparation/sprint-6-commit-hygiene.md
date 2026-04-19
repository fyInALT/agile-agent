# Sprint 6: Commit Hygiene

## Goal

Encourage proper commit practices during agent work by detecting commit boundaries and prompting agents.

## Duration

2-3 days

## Stories

### Story 6.1: CommitBoundaryDetector

**Description**: Detect good points to commit based on agent activity patterns.

**Acceptance Criteria**:
- Detects after file save completion
- Detects after test pass
- Detects after logical feature implementation
- Avoids noisy detection (too frequent prompts)

**Implementation**:
```rust
// In decision/src/commit_boundary.rs
#[derive(Debug, Clone)]
pub struct CommitBoundaryDetector {
    /// Minimum changes threshold for suggesting commit
    min_changes_threshold: usize,
    /// Time window to batch changes (avoid noisy prompts)
    batch_window_secs: u64,
    /// Last suggestion timestamp (to avoid spam)
    last_suggestion: Option<DateTime<Utc>>,
    /// Minimum interval between suggestions
    min_suggestion_interval_secs: u64,
}

impl CommitBoundaryDetector {
    /// Check if current state is a good commit boundary
    pub fn check_boundary(&mut self, activity: &AgentActivitySnapshot) -> Option<CommitBoundarySignal> {
        // Skip if too soon since last suggestion
        if self.too_soon_for_suggestion() {
            return None;
        }
        
        // Check for commit-worthy changes
        if activity.uncommitted_files.len() >= self.min_changes_threshold {
            // Check if agent appears to be at a natural pause
            if activity.is_paused || activity.just_finished_task {
                return Some(CommitBoundarySignal {
                    files: activity.uncommitted_files.clone(),
                    reason: BoundaryReason::NaturalPause,
                    suggested_message: self.generate_suggestion(&activity),
                });
            }
        }
        
        None
    }
    
    fn generate_suggestion(&self, activity: &AgentActivitySnapshot) -> String {
        // Generate based on recent activity
        if activity.last_action.contains("test") {
            "Tests updated, consider committing".to_string()
        } else if activity.last_action.contains("implement") {
            "Implementation complete, consider committing".to_string()
        } else {
            "Multiple files changed, consider committing".to_string()
        }
    }
}

#[derive(Debug, Clone)]
pub struct CommitBoundarySignal {
    pub files: Vec<FileStatus>,
    pub reason: BoundaryReason,
    pub suggested_message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BoundaryReason {
    NaturalPause,
    TestPassed,
    FeatureComplete,
    MultipleFilesChanged,
    UserRequested,
}
```

---

### Story 6.2: SuggestCommitAction

**Description**: Create action to suggest committing to the agent.

**Acceptance Criteria**:
- Registered as new action type
- Provides gentle, helpful prompt
- Includes suggested commit message
- Non-blocking (agent can ignore)

**Implementation**:
```rust
// In decision/src/builtin_actions.rs
pub fn suggest_commit() -> ActionType {
    ActionType::new("suggest_commit")
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SuggestCommitAction {
    /// Suggested commit message
    pub suggested_message: String,
    /// Files that would be committed
    pub files: Vec<FileStatus>,
    /// Whether this is mandatory (vs optional suggestion)
    pub mandatory: bool,
}

impl DecisionAction for SuggestCommitAction {
    fn action_type(&self) -> ActionType {
        suggest_commit()
    }
    
    fn to_prompt_format(&self) -> String {
        format!(
            "SuggestCommit: {}\nFiles: {}\n{}",
            self.suggested_message,
            self.files.len(),
            if self.mandatory { "MANDATORY" } else { "OPTIONAL" }
        )
    }
}
```

---

### Story 6.3: CommitBoundary Situation

**Description**: Create situation triggered at commit boundaries.

**Acceptance Criteria**:
- Registered as new situation type
- Low priority (not urgent)
- Contains commit suggestion

**Implementation**:
```rust
pub fn commit_boundary() -> SituationType {
    SituationType::new("commit_boundary")
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommitBoundarySituation {
    /// Signal from detector
    pub signal: CommitBoundarySignal,
    /// Agent activity context
    pub activity: AgentActivitySnapshot,
}

impl DecisionSituation for CommitBoundarySituation {
    fn situation_type(&self) -> SituationType {
        commit_boundary()
    }
    
    fn requires_human(&self) -> bool {
        false  // Never requires human, just a suggestion
    }
    
    fn human_urgency(&self) -> UrgencyLevel {
        UrgencyLevel::Low
    }
    
    fn available_actions(&self) -> Vec<ActionType> {
        vec![
            ActionType::new("suggest_commit"),
            ActionType::new("continue"),  // Ignore suggestion
        ]
    }
    
    fn to_prompt_text(&self) -> String {
        format!(
            "Commit boundary detected:\nFiles changed: {}\nReason: {}\nSuggestion: {}",
            self.signal.files.len(),
            self.signal.reason,
            self.signal.suggested_message,
        )
    }
}
```

---

### Story 6.4: Pre-Commit Validation

**Description**: Validate commit content before allowing commit.

**Acceptance Criteria**:
- Check for sensitive files
- Validate commit message format
- Warn about large commits
- Suggest splitting oversized commits

**Implementation**:
```rust
pub struct PreCommitValidator {
    /// Sensitive file patterns
    sensitive_patterns: Vec<String>,
    /// Maximum recommended commit size (files)
    max_commit_size: usize,
    /// Commit message format regex
    message_format: String,
}

impl PreCommitValidator {
    pub fn validate(&self, files: &[FileStatus], message: &str) -> ValidationResult {
        let mut warnings = Vec::new();
        let mut errors = Vec::new();
        
        // Check sensitive files
        for file in files {
            if self.is_sensitive(&file.path) {
                warnings.push(format!(
                    "Warning: '{}' may contain sensitive data. Review before committing.",
                    file.path
                ));
            }
        }
        
        // Check commit size
        if files.len() > self.max_commit_size {
            warnings.push(format!(
                "Large commit ({}) files. Consider splitting into multiple commits.",
                files.len()
            ));
        }
        
        // Validate message format
        if !self.is_valid_message_format(message) {
            errors.push("Commit message should follow conventional format: type(scope): description");
        }
        
        ValidationResult {
            valid: errors.is_empty(),
            warnings,
            errors,
        }
    }
    
    fn is_valid_message_format(&self, message: &str) -> bool {
        // Check conventional commits format
        // feat|fix|refactor|docs|test|chore(scope?): description
        let pattern = Regex::new(r"^(feat|fix|refactor|docs|test|chore)(\([^)]+\))?:\s+.+").unwrap();
        pattern.is_match(message)
    }
}

#[derive(Debug, Clone)]
pub struct ValidationResult {
    pub valid: bool,
    pub warnings: Vec<String>,
    pub errors: Vec<String>,
}
```

---

### Story 6.5: Integration with Agent Activity Tracking

**Description**: Track agent activity for boundary detection.

**Acceptance Criteria**:
- Monitor provider events for activity
- Track file modifications
- Track test results
- Integrate with existing event flow

**Implementation**:
```rust
// In core/src/agent_pool.rs or app_loop.rs
impl AgentPool {
    /// Update activity tracking from provider event
    fn update_activity_tracking(&mut self, agent_id: &AgentId, event: &ProviderEvent) {
        if let Some(tracker) = self.activity_trackers.get_mut(agent_id) {
            match event {
                ProviderEvent::GenericToolCallFinished { name, success, .. } => {
                    if name == "test" && *success {
                        tracker.record_test_pass();
                    }
                    if name == "write_file" || name == "edit_file" {
                        tracker.record_file_change();
                    }
                }
                ProviderEvent::Finished => {
                    tracker.record_pause();
                }
                _ => {}
            }
            
            // Check for commit boundary
            if let Some(signal) = self.commit_detector.check_boundary(&tracker.snapshot()) {
                self.trigger_commit_boundary_decision(agent_id, signal);
            }
        }
    }
}
```

---

### Story 6.6: Non-Blocking Suggestion Execution

**Description**: Execute commit suggestion without blocking agent.

**Acceptance Criteria**:
- Suggestion added to transcript
- Agent continues working
- No state transition required
- Agent can choose to commit or ignore

**Implementation**:
```rust
// In core/src/agent_pool.rs
impl AgentPool {
    fn execute_suggest_commit(&mut self, agent_id: &AgentId, action: &SuggestCommitAction) {
        let slot = self.get_slot_mut_by_id(agent_id)?;
        
        // Add suggestion to transcript (non-blocking)
        if action.mandatory {
            slot.append_transcript(TranscriptEntry::User(format!(
                "MANDATORY: Commit your changes now.\n{}\nFiles: {}",
                action.suggested_message,
                action.files.iter().map(|f| f.path.as_str()).join(", ")
            )));
        } else {
            slot.append_transcript(TranscriptEntry::System(format!(
                "[Git Reminder] Consider committing: {}",
                action.suggested_message
            )));
        }
        
        // No state transition - agent continues
    }
}
```

---

### Story 6.7: Unit Tests

**Description**: Comprehensive unit tests.

**Test Cases**:
```rust
#[test]
fn test_boundary_detection_after_test_pass() {
    let detector = CommitBoundaryDetector::default();
    let activity = AgentActivitySnapshot {
        uncommitted_files: vec![FileStatus::new("src/test.rs", Modified)],
        just_finished_task: true,
        last_action: "run_tests".to_string(),
    };
    
    let signal = detector.check_boundary(&activity);
    assert!(signal.is_some());
}

#[test]
fn test_no_boundary_too_soon() {
    let detector = CommitBoundaryDetector::default();
    // First call returns suggestion
    let _ = detector.check_boundary(&activity_with_changes);
    
    // Immediate second call returns None (too soon)
    let signal = detector.check_boundary(&activity_with_changes);
    assert!(signal.is_none());
}

#[test]
fn test_message_format_validation() {
    let validator = PreCommitValidator::default();
    
    assert!(validator.is_valid_message_format("feat(auth): add login"));
    assert!(validator.is_valid_message_format("fix: resolve timeout"));
    assert!(!validator.is_valid_message_format("added some stuff"));
}

#[test]
fn test_sensitive_file_warning() {
    let validator = PreCommitValidator::default();
    let files = vec![FileStatus::new(".env", Modified)];
    
    let result = validator.validate(&files, "feat: config update");
    assert!(result.warnings.iter().any(|w| w.contains("sensitive")));
}
```

---

## Integration Points

- `decision/src/commit_boundary.rs`: New module
- `decision/src/builtin_actions.rs`: Register action
- `decision/src/builtin_situations.rs`: Register situation
- `core/src/agent_pool.rs`: Activity tracking and execution
- `tui/src/app_loop.rs`: Event processing

## Dependencies

- Sprint 1-5 completed (basic git infrastructure)
- Existing event flow

## Risks

- Too many prompts annoying agents (mitigate: configurable thresholds, debouncing)
- Missing good commit moments (mitigate: tune detection logic)

## Definition of Done

- [ ] CommitBoundaryDetector implemented
- [ ] SuggestCommitAction registered
- [ ] CommitBoundarySituation defined
- [ ] PreCommitValidator implemented
- [ ] Activity tracking integrated
- [ ] Non-blocking execution working
- [ ] Unit tests passing
- [ ] Code reviewed
