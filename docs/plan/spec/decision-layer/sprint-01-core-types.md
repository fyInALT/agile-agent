# Sprint 1: Core Types

## Metadata

- Sprint ID: `decision-sprint-001`
- Title: `Core Types`
- Duration: 2 weeks
- Priority: P0 (Critical)
- Status: `Backlog`
- Created: 2026-04-14

## TDD Reference

See [Test Specification](test-specification.md) for detailed TDD test tasks:
- Sprint 1 Tests: T1.1.T1-T1.4.T6 (30 tests)
- Test execution: Write failing tests first, implement minimum, refactor

## Sprint Goal

Establish core domain types for decision layer: ProviderStatus, ProviderOutputType, DecisionOutput, DecisionContext, DecisionAgentConfig, and human intervention types.

## Stories

### Story 1.1: ProviderStatus and ProviderOutputType Enums

**Priority**: P0
**Effort**: 3 points
**Status**: Backlog

Define enums for classifying provider output status.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T1.1.1 | Create `ProviderOutputType` enum (Running, Finished) | Todo | - |
| T1.1.2 | Create `ProviderStatus` enum with four variants | Todo | - |
| T1.1.3 | Create `ChoiceOption` struct | Todo | - |
| T1.1.4 | Create `CompletionProgress` struct | Todo | - |
| T1.1.5 | Create `ErrorType` enum with three variants | Todo | - |
| T1.1.6 | Add serde derives for JSON serialization | Todo | - |
| T1.1.7 | Write unit tests for enum variants | Todo | - |

#### Acceptance Criteria

- All enums serialize/deserialize correctly
- ProviderStatus covers four decision situations
- ChoiceOption has id and label fields

#### Technical Notes

```rust
/// Provider output classification
pub enum ProviderOutputType {
    /// Running output (no action needed)
    Running {
        event: ProviderEvent,
    },
    
    /// Finished output (needs decision)
    Finished {
        status: ProviderStatus,
    },
}

/// Four decision situations
pub enum ProviderStatus {
    /// Situation 1: Provider waiting for user choice
    WaitingForChoice {
        options: Vec<ChoiceOption>,
    },
    
    /// Situation 2: Provider claims completion
    ClaimsCompletion {
        summary: String,
        reflection_rounds: u8,
    },
    
    /// Situation 3: Provider partially completed
    PartialCompletion {
        progress: CompletionProgress,
    },
    
    /// Situation 4: Provider error
    Error {
        error_type: ErrorType,
    },
}

pub struct ChoiceOption {
    /// Option identifier (e.g., "A", "B", "once", "always")
    pub id: String,
    
    /// Option display label
    pub label: String,
}

pub struct CompletionProgress {
    /// Completed functionality items
    pub completed_items: Vec<String>,
    
    /// Remaining functionality items
    pub remaining_items: Vec<String>,
}

pub enum ErrorType {
    /// Explicit failure with message
    Failure { message: String },
    
    /// Gibberish (nonsensical output)
    Gibberish,
    
    /// Repetition of previous output
    Repetition { previous_output_hash: String },
}
```

---

### Story 1.2: DecisionOutput and DecisionContext Structs

**Priority**: P0
**Effort**: 4 points
**Status**: Backlog

Define structs for decision output and context.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T1.2.1 | Create `DecisionOutput` enum with five variants | Todo | - |
| T1.2.2 | Create `DecisionContext` struct | Todo | - |
| T1.2.3 | Create `RunningContextCache` struct | Todo | - |
| T1.2.4 | Create `ToolCallRecord` struct | Todo | - |
| T1.2.5 | Create `FileChangeRecord` struct | Todo | - |
| T1.2.6 | Create `DecisionRecord` struct | Todo | - |
| T1.2.7 | Add helper methods for context building | Todo | - |
| T1.2.8 | Write unit tests for decision output variants | Todo | - |

#### Acceptance Criteria

- DecisionOutput covers all feedback types
- DecisionContext aggregates all needed information
- RunningContextCache has proper field structure

#### Technical Notes

```rust
/// Decision layer output (feedback to provider)
pub enum DecisionOutput {
    /// Choice selection (Situation 1)
    Choice {
        selected: String,
        reason: String,
    },
    
    /// Reflection request (Situation 2, rounds 1-2)
    ReflectionRequest {
        prompt: String,
    },
    
    /// Completion confirmation (Situation 2, round 3)
    CompletionConfirm {
        submit_pr: bool,
        next_task: Option<TaskId>,
    },
    
    /// Continue instruction (Situation 3)
    ContinueInstruction {
        prompt: String,
        focus_items: Vec<String>,
    },
    
    /// Retry instruction (Situation 4)
    RetryInstruction {
        prompt: String,
        cooldown_ms: u64,
    },
}

/// Context for making a decision
pub struct DecisionContext {
    /// Project rules (from CLAUDE.md/AGENTS.md)
    pub project_rules: ProjectRules,
    
    /// Current story definition
    pub current_story: Option<StoryDefinition>,
    
    /// Current task definition
    pub current_task: Option<TaskDefinition>,
    
    /// Running context cache
    pub running_context: RunningContextCache,
    
    /// Decision history for this session
    pub decision_history: Vec<DecisionRecord>,
    
    /// Reflection rounds count
    pub reflection_rounds: u8,
    
    /// Retry count
    pub retry_count: u8,
    
    /// Provider status triggering this decision
    pub trigger_status: ProviderStatus,
}

/// Cache of running context for decision reference
pub struct RunningContextCache {
    /// Tool call records
    pub tool_calls: Vec<ToolCallRecord>,
    
    /// File modification records
    pub file_changes: Vec<FileChangeRecord>,
    
    /// Thinking summary
    pub thinking_summary: Option<String>,
    
    /// Key output excerpts
    pub key_outputs: Vec<String>,
}

pub struct ToolCallRecord {
    pub name: String,
    pub call_id: Option<String>,
    pub input_preview: Option<String>,
    pub output_preview: Option<String>,
    pub success: bool,
    pub timestamp: DateTime<Utc>,
}

pub struct FileChangeRecord {
    pub path: String,
    pub change_type: FileChangeType,
    pub diff_preview: Option<String>,
    pub timestamp: DateTime<Utc>,
}

pub enum FileChangeType {
    Read,
    Write,
    Edit,
    Delete,
}

pub struct DecisionRecord {
    pub decision_id: DecisionId,
    pub timestamp: DateTime<Utc>,
    pub trigger_status: ProviderStatus,
    pub output: DecisionOutput,
    pub engine_type: DecisionEngineType,
}
```

---

### Story 1.3: DecisionAgentConfig and DecisionEngine Enums

**Priority**: P0
**Effort**: 3 points
**Status**: Backlog

Define configuration types for decision agent.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T1.3.1 | Create `DecisionEngineType` enum | Todo | - |
| T1.3.2 | Create `DecisionAgentConfig` struct | Todo | - |
| T1.3.3 | Create `DecisionAgentCreationPolicy` enum | Todo | - |
| T1.3.4 | Create default configuration values | Todo | - |
| T1.3.5 | Create TOML configuration parsing | Todo | - |
| T1.3.6 | Write unit tests for config validation | Todo | - |
| T1.3.7 | Write unit tests for default values | Todo | - |

#### Acceptance Criteria

- Configuration supports all engine types
- Default values are sensible
- TOML parsing works correctly

#### Technical Notes

```rust
/// Decision engine type selection
pub enum DecisionEngineType {
    /// Use LLM for decision (e.g., Claude API)
    LLM {
        provider: ProviderKind,
    },
    
    /// Use CLI provider (independent session)
    CLI {
        provider: ProviderKind,
    },
    
    /// Use rule-based engine
    RuleBased,
    
    /// Mock engine for testing
    Mock,
}

/// Decision agent creation timing
pub enum DecisionAgentCreationPolicy {
    /// Create immediately when Main Agent spawns
    Eager,
    
    /// Create on first blocked event
    Lazy,
    
    /// Follow configuration setting
    Configured,
}

/// Decision agent configuration
pub struct DecisionAgentConfig {
    /// Decision engine type
    pub engine_type: DecisionEngineType,
    
    /// Maximum reflection rounds (default: 2)
    pub max_reflection_rounds: u8,
    
    /// Retry cooldown in milliseconds (default: 10000)
    pub retry_cooldown_ms: u64,
    
    /// Maximum retries (default: 3)
    pub max_retries: u8,
    
    /// Decision timeout in milliseconds (default: 60000)
    pub decision_timeout_ms: u64,
    
    /// Creation policy (default: Lazy)
    pub creation_policy: DecisionAgentCreationPolicy,
    
    /// Enable human intervention (default: true)
    pub enable_human_intervention: bool,
    
    /// Criticality threshold for human escalation (default: 3)
    pub criticality_threshold: u8,
    
    /// Context cache size limit in bytes (default: 10240)
    pub context_cache_max_bytes: usize,
}

impl Default for DecisionAgentConfig {
    fn default() -> Self {
        Self {
            engine_type: DecisionEngineType::LLM { provider: ProviderKind::Claude },
            max_reflection_rounds: 2,
            retry_cooldown_ms: 10000,
            max_retries: 3,
            decision_timeout_ms: 60000,
            creation_policy: DecisionAgentCreationPolicy::Lazy,
            enable_human_intervention: true,
            criticality_threshold: 3,
            context_cache_max_bytes: 10240,
        }
    }
}
```

**Configuration File Format**:

```toml
[decision_layer]
engine = "llm"
provider = "claude"
max_reflection_rounds = 2
retry_cooldown_ms = 10000
max_retries = 3
decision_timeout_ms = 60000
creation_policy = "lazy"
enable_human_intervention = true
criticality_threshold = 3

[decision_layer.context_cache]
max_bytes = 10240
max_tool_calls = 50
max_file_changes = 30
max_key_outputs = 20
```

---

### Story 1.4: CriticalDecisionCriteria and HumanDecisionRequest

**Priority**: P0
**Effort**: 4 points
**Status**: Backlog

Define types for human intervention system.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T1.4.1 | Create `CriticalDecisionCriteria` struct | Todo | - |
| T1.4.2 | Create `CriticalDecisionReason` enum | Todo | - |
| T1.4.3 | Create `HumanDecisionRequest` struct | Todo | - |
| T1.4.4 | Create `HumanSelection` enum | Todo | - |
| T1.4.5 | Create `HumanDecisionResponse` struct | Todo | - |
| T1.4.6 | Create `Recommendation` struct | Todo | - |
| T1.4.7 | Create `DecisionAnalysis` struct | Todo | - |
| T1.4.8 | Implement `criticality_score()` method | Todo | - |
| T1.4.9 | Write unit tests for criteria evaluation | Todo | - |

#### Acceptance Criteria

- Criteria covers all escalation dimensions
- HumanDecisionRequest has complete fields
- criticality_score calculation works correctly

#### Technical Notes

```rust
/// Criteria for determining if decision requires human escalation
pub struct CriticalDecisionCriteria {
    /// Affects multiple agents
    pub multi_agent_impact: bool,
    
    /// Irreversible operation
    pub irreversible: bool,
    
    /// High risk operation
    pub high_risk: bool,
    
    /// Decision agent confidence below threshold
    pub low_confidence: bool,
    
    /// Cost exceeds threshold
    pub high_cost: bool,
    
    /// Project rule requires human confirmation
    pub requires_human: bool,
}

impl CriticalDecisionCriteria {
    /// Check if decision is critical (requires human)
    pub fn is_critical(&self) -> bool {
        self.multi_agent_impact ||
        self.irreversible ||
        self.high_risk ||
        self.low_confidence ||
        self.high_cost ||
        self.requires_human
    }
    
    /// Calculate criticality score (higher = more critical)
    pub fn criticality_score(&self) -> u8 {
        let mut score = 0;
        if self.multi_agent_impact { score += 2; }
        if self.irreversible { score += 3; }
        if self.high_risk { score += 3; }
        if self.low_confidence { score += 1; }
        if self.high_cost { score += 1; }
        if self.requires_human { score += 2; }
        score
    }
}

/// Reason for critical decision escalation
pub enum CriticalDecisionReason {
    /// Impacts multiple agents
    MultiAgentImpact { affected_agents: Vec<AgentId> },
    
    /// Irreversible operation
    IrreversibleOperation { operation: String },
    
    /// High risk operation
    HighRiskOperation { risk_description: String },
    
    /// Confidence below threshold
    LowConfidence { confidence: f64 },
    
    /// Project rule requires human
    ProjectRuleRequiresHuman { rule: String },
    
    /// High cost consideration
    HighCost { estimated_cost: String },
}

/// Request sent to human for decision
pub struct HumanDecisionRequest {
    /// Unique request ID
    pub id: DecisionRequestId,
    
    /// Source main agent
    pub agent_id: AgentId,
    
    /// Source decision agent
    pub decision_agent_id: AgentId,
    
    /// Associated task (if any)
    pub task_id: Option<TaskId>,
    
    /// Decision type
    pub decision_type: DecisionType,
    
    /// Criticality reason
    pub reason: CriticalDecisionReason,
    
    /// Available options
    pub options: Vec<ChoiceOption>,
    
    /// Decision agent recommendation
    pub recommendation: Option<Recommendation>,
    
    /// Detailed analysis from decision agent
    pub analysis: DecisionAnalysis,
    
    /// Request timestamp
    pub requested_at: DateTime<Utc>,
    
    /// Expiration timestamp
    pub expires_at: DateTime<Utc>,
    
    /// Priority level
    pub priority: DecisionPriority,
}

pub struct Recommendation {
    /// Recommended option
    pub selected: String,
    
    /// Reason for recommendation
    pub reason: String,
    
    /// Confidence level
    pub confidence: f64,
}

pub struct DecisionAnalysis {
    /// Analysis for each option
    pub option_analysis: HashMap<String, OptionAnalysis>,
    
    /// Context summary
    pub context_summary: String,
    
    /// Risk assessment
    pub risk_assessment: String,
    
    /// Impact analysis
    pub impact_analysis: String,
}

pub struct OptionAnalysis {
    /// Advantages
    pub pros: Vec<String>,
    
    /// Disadvantages
    pub cons: Vec<String>,
    
    /// Risks
    pub risks: Vec<String>,
    
    /// Recommendation score
    pub score: f64,
}

/// Human's response to decision request
pub enum HumanSelection {
    /// Select specific option
    Select { option_id: String },
    
    /// Accept decision agent recommendation
    AcceptRecommendation,
    
    /// Provide custom instruction
    CustomInstruction { instruction: String },
    
    /// Cancel current operation
    CancelOperation,
    
    /// Skip current task
    SkipTask,
    
    /// Pause the agent
    PauseAgent,
}

pub struct HumanDecisionResponse {
    /// Original request ID
    pub request_id: DecisionRequestId,
    
    /// User's selection
    pub selection: HumanSelection,
    
    /// User note (optional)
    pub note: Option<String>,
    
    /// Response timestamp
    pub responded_at: DateTime<Utc>,
    
    /// Response source
    pub source: ResponseSource,
}

pub enum ResponseSource {
    Tui,
    CliCommand,
    ExternalWebhook,
    TimeoutDefault,
}

pub enum DecisionPriority {
    High,
    Medium,
    Low,
}

pub enum DecisionType {
    Architecture,
    TechnologySelection,
    Deployment,
    DatabaseOperation,
    PrMerge,
    Custom,
}
```

---

## Sprint Risks

| Risk | Probability | Impact | Mitigation |
|------|-------------|--------|------------|
| Type definition complexity | Medium | Medium | Incremental design with reviews |
| Serialization edge cases | Low | Medium | Comprehensive serde tests |
| Configuration validation gaps | Low | Low | Unit tests for all config fields |

## Sprint Deliverables

- `decision/src/types.rs` - Core type definitions
- `decision/src/config.rs` - Configuration types
- `decision/src/human_types.rs` - Human intervention types
- Unit tests for all type definitions

## Dependencies

None (foundation sprint).

## Next Sprint

After completing this sprint, proceed to [Sprint 2: Output Classifier](./sprint-02-output-classifier.md) for provider-specific output classification.