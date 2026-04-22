# Decision Layer Task Concept - Requirements Specification

## Document Information

- **Author**: Development Team
- **Date**: 2026-04-18
- **Status**: Draft
- **Version**: 1.0

---

## 1. Executive Summary

This specification defines the requirements for introducing a **Task concept** into the decision layer of agile-agent. The current decision layer lacks a structured task lifecycle management, making it difficult to track reflection cycles, completion confirmations, and execution history. This enhancement will enable the decision layer to:

1. Automate routine decisions (target: 80% automation rate)
2. Escalate only genuinely human-required decisions to the programmer
3. Enforce mandatory reflection and verification cycles to prevent AI errors
4. Provide structured, customizable decision workflows

---

## 2. Context and Background

### 2.1 Current System State

The decision layer currently provides basic decision capabilities:

- `DecisionSituation` trait defines decision contexts
- `ActionRegistry` manages available actions
- Built-in actions: `reflect`, `confirm_completion`, `continue`, `retry`, `request_human`
- Default configuration: `max_reflection_rounds = 2`, followed by verification

**What's Missing**:

- No explicit **Task entity** to track execution state
- No structured recording of reflection count and confirmation count per task
- No task lifecycle management (pending → in-progress → completed)
- No structured decision workflow (stages, transitions, conditions)
- No prompt template system for different decision stages

### 2.2 Design Philosophy

#### AI Errors Are Inevitable

AI assistants (Claude, Codex) frequently make mistakes:
- Missing test cases
- Missing edge cases
- Code style violations
- Scope creep (making changes beyond task boundaries)
- Misunderstanding requirements

**Therefore**: Mandatory reflection and verification cycles are essential, not optional optimizations.

#### Decision Layer Purpose: Automate Routine Decisions

When an experienced programmer works with AI assistants, most interactions are mechanical:
- View output → No issues → Confirm continue
- View output → Issues found → Reflect and fix
- View output → Task done → Confirm completion

These 80% of routine decisions should be automated by the decision layer, leaving only genuinely complex decisions for human intervention.

#### Agile Development Context

Tasks delivered to AI have already been:
- Split during Sprint Planning
- Reviewed by Product Owner
- Thought through by Developer

This means task boundaries are typically clear, enabling the decision layer to operate autonomously within those boundaries.

### 2.3 Problem Statement

Without a Task concept, the decision layer cannot:

| Problem | Impact |
|---------|--------|
| No task state tracking | Cannot determine if a task is progressing or stuck |
| No reflection count tracking | Cannot detect excessive reflection loops |
| No confirmation count tracking | Cannot detect repeated completion failures |
| No execution history | Cannot analyze what went wrong |
| No structured workflow | Current logic is ad-hoc, hard to customize |

---

## 3. Requirements Overview

### 3.1 Feature Categories

| Category | Priority | Description |
|----------|----------|-------------|
| Task Entity | P0 | Core task structure and lifecycle |
| Decision Workflow | P0 | Structured stages and transitions |
| Prompt Templates | P0 | Stage-specific prompt generation |
| Auto-Check System | P0 | Automated quality/completion verification |
| Task Registry | P1 | Task storage and retrieval |
| Execution History | P1 | Action logging and replay |
| Metrics Collection | P2 | Automation rate, completion statistics |
| Custom Workflows | P2 | User-defined decision processes |

### 3.2 Stakeholders

| Stakeholder | Concerns |
|-------------|----------|
| Developer (User) | Wants minimal intervention, clear task status, control when needed |
| AI Agent | Needs clear prompts, structured feedback, task context |
| Decision Layer | Needs task state, workflow definition, condition evaluation |
| Scrum Master Agent | Needs task progress, blocker detection |

---

## 4. Functional Requirements

### 4.1 Task Entity [P0]

#### FR-001: Task Structure Definition

**Requirement**: Define a Task entity with complete lifecycle tracking.

**Specification**:

```rust
pub struct Task {
    /// Unique task identifier
    pub id: TaskId,

    /// Task description from Sprint Backlog
    pub description: String,

    /// Task boundary constraints (optional)
    pub constraints: Vec<String>,

    /// Current task status
    pub status: TaskStatus,

    /// Creation timestamp
    pub created_at: DateTime<Utc>,

    /// Last update timestamp
    pub updated_at: DateTime<Utc>,

    /// Number of reflection cycles executed
    pub reflection_count: usize,

    /// Number of completion confirmation attempts
    pub confirmation_count: usize,

    /// Maximum allowed reflection rounds (configurable)
    pub max_reflection_rounds: usize,

    /// Retry count for error recovery
    pub retry_count: usize,
}
```

**Acceptance Criteria**:
- [x] Task struct defined with all required fields
- [x] TaskId type defined (UUID or string-based)
- [x] Task can be serialized/deserialized for persistence

#### FR-002: Task Status Enumeration

**Requirement**: Define all possible task states.

**Specification**:

```rust
pub enum TaskStatus {
    /// Task is waiting to start
    Pending,

    /// Task is actively being executed by AI
    InProgress,

    /// Task is in reflection cycle (fixing issues)
    Reflecting,

    /// Task passed verification, waiting for final confirmation
    PendingConfirmation,

    /// Task blocked, requires human decision
    NeedsHumanDecision,

    /// Task execution paused (timeout, system error)
    Paused,

    /// Task completed and confirmed
    Completed,

    /// Task cancelled by user or system
    Cancelled,
}
```

**Acceptance Criteria**:
- [x] All statuses defined covering complete lifecycle
- [x] Status transitions are valid (defined in FR-003)
- [x] Status can be displayed in TUI

#### FR-003: Task Status Transitions

**Requirement**: Define valid state transitions.

**Specification**:

| From | To | Condition |
|------|----|-----------| `Pending` | `InProgress` | Task assigned to agent |
| `InProgress` | `Reflecting` | Issue detected by auto-check |
| `InProgress` | `PendingConfirmation` | Verification passed |
| `Reflecting` | `InProgress` | Issue fixed, continue execution |
| `Reflecting` | `NeedsHumanDecision` | Reflection count exceeded |
| `PendingConfirmation` | `Completed` | Human confirms completion |
| `PendingConfirmation` | `Reflecting` | Human rejects completion |
| `NeedsHumanDecision` | `InProgress` | Human approves continuation |
| `NeedsHumanDecision` | `Cancelled` | Human cancels task |
| `InProgress` | `Paused` | Timeout or system error |
| `Paused` | `InProgress` | System recovered |
| `Paused` | `Cancelled` | Timeout exceeded |
| Any | `Cancelled` | User cancels |

**Acceptance Criteria**:
- [x] Transition function validates allowed state changes
- [x] Invalid transitions return error
- [x] Each transition is logged in execution history

### 4.2 Decision Workflow [P0]

#### FR-004: Decision Stage Definition

**Requirement**: Define a structured decision stage abstraction.

**Specification**:

```rust
pub struct DecisionStage {
    /// Stage identifier
    pub id: StageId,

    /// Human-readable stage name
    pub name: String,

    /// Stage description (used in prompts)
    pub description: String,

    /// Entry condition (what must be true to enter this stage)
    pub entry_condition: Condition,

    /// Exit condition (what must be true to leave this stage)
    pub exit_condition: Condition,

    /// Possible transitions to other stages
    pub transitions: Vec<StageTransition>,

    /// Actions available in this stage
    pub actions: Vec<DecisionAction>,
}

pub struct StageTransition {
    /// Target stage
    pub target: StageId,

    /// Condition triggering this transition
    pub condition: Condition,

    /// Prompt to generate when transitioning
    pub prompt: String,
}
```

**Acceptance Criteria**:
- [x] Stage struct defined with all fields
- [x] StageId type defined
- [x] Transitions are directed edges (no cycles at definition level, cycles form at runtime)

#### FR-005: Condition System

**Requirement**: Define a flexible condition evaluation system.

**Specification**:

```rust
pub enum Condition {
    // Built-in conditions
    TestsPass,
    NoCompileErrors,
    NoSyntaxErrors,
    StyleConformant,
    GoalsAchieved,
    MaxReflectionsReached,
    HumanApproved,
    TimeoutExceeded,

    // Composite conditions
    All(Vec<Condition>),
    Any(Vec<Condition>),
    Not(Condition),

    // Custom condition (script/rule)
    Custom(String),
}

impl Condition {
    pub fn evaluate(&self, context: &EvaluationContext) -> bool;
}
```

**Evaluation Context**:

```rust
pub struct EvaluationContext {
    pub task: &Task,
    pub output: &AgentOutput,
    pub test_results: Option<TestResults>,
    pub reflection_count: usize,
    pub config: &ProcessConfig,
}
```

**Acceptance Criteria**:
- [x] All condition types implemented
- [x] evaluate() function correctly evaluates each condition
- [x] Composite conditions (All, Any, Not) work correctly
- [x] Custom conditions can be registered

#### FR-006: Decision Action Definition

**Requirement**: Define all possible decision actions.

**Specification**:

```rust
pub enum DecisionAction {
    /// Continue execution (no intervention needed)
    Continue,

    /// Request AI to reflect and fix issues
    Reflect { reason: String },

    /// Confirm task completion
    ConfirmCompletion,

    /// Request human intervention
    RequestHuman { question: String },

    /// Transition to next stage
    AdvanceTo { stage: StageId },

    /// Return to previous stage
    ReturnTo { stage: StageId },

    /// Cancel the task
    Cancel { reason: String },

    /// Retry after error
    Retry,

    /// Wait for condition
    Wait { reason: String },
}
```

**Acceptance Criteria**:
- [x] All actions defined
- [x] Each action can be converted to prompt for AI
- [x] Actions are serializable for logging

#### FR-007: Decision Process Definition

**Requirement**: Define a complete decision process as a collection of stages.

**Specification**:

```rust
pub struct DecisionProcess {
    /// Process name
    pub name: String,

    /// Process description
    pub description: String,

    /// All stages in this process
    pub stages: Vec<DecisionStage>,

    /// Initial stage when process starts
    pub initial_stage: StageId,

    /// Final stage when process completes
    pub final_stage: StageId,

    /// Global configuration
    pub config: ProcessConfig,
}

pub struct ProcessConfig {
    /// Maximum reflection rounds before escalation
    pub max_reflection_rounds: usize,

    /// Whether verification is mandatory
    pub enforce_verification: bool,

    /// Task execution timeout
    pub timeout: Duration,

    /// Whether to log all decisions
    pub log_decisions: bool,
}
```

**Acceptance Criteria**:
- [x] Process struct defined
- [x] Process can be validated (no unreachable stages, valid initial/final)
- [x] Process can be serialized for persistence

#### FR-008: Default Process - Simple Agile

**Requirement**: Provide a default decision process matching current behavior.

**Specification**:

The default process should have 9 stages:

| Stage ID | Name | Purpose |
|----------|------|---------|
| `start` | Start Development | Initialize task execution |
| `developing` | Developing | AI executing the task |
| `check_quality` | Quality Check | Verify output quality |
| `reflecting` | Reflecting | Fix detected issues |
| `check_completion` | Completion Check | Verify all goals achieved |
| `confirming` | Confirming Completion | Final human confirmation |
| `human_decision` | Human Decision | Wait for human input |
| `completed` | Completed | Task successfully finished |
| `cancelled` | Cancelled | Task cancelled |

**Stage Flow**:

```
start → developing → check_quality
                       ├─→ reflecting → developing (loop)
                       │              └→ human_decision
                       └→ check_completion
                              ├─→ developing (continue)
                              └→ confirming → completed
                                             └→ reflecting (reject)

human_decision → developing (approve) or cancelled (reject)
```

**Acceptance Criteria**:
- [x] Default process defined with all 9 stages
- [x] All transitions correctly defined
- [x] Default config: max_reflection_rounds=2, enforce_verification=true, timeout=30min

### 4.3 Prompt Templates [P0]

#### FR-009: Prompt Template Structure

**Requirement**: Define structured prompt templates for each stage.

**Specification**:

```rust
pub struct PromptTemplate {
    /// Template identifier
    pub id: String,

    /// Template content with {{variable}} placeholders
    pub content: String,

    /// List of required variables
    pub variables: Vec<String>,
}

impl PromptTemplate {
    /// Render template with variable values
    pub fn render(&self, values: &HashMap<String, String>) -> String;
}
```

**Acceptance Criteria**:
- [x] Template struct defined
- [x] render() replaces all {{var}} placeholders
- [x] Missing variables return error or use default

#### FR-010: Default Prompt Templates

**Requirement**: Define default templates for each stage in the default process.

**Specification**:

| Stage | Template Variables | Template Content |
|-------|-------------------|------------------|
| `start` | `task_description` | "Begin implementing task: {{task_description}}" |
| `check_quality` | `ai_output`, `task_constraints` | "Check AI output:\n1. Syntax errors?\n2. Tests pass?\n3. Within boundaries?\n4. Code quality?\n\nOutput: {{ai_output}}\nConstraints: {{task_constraints}}" |
| `reflecting` | `problem_description`, `reflection_count`, `max_reflections` | "Issue found: {{problem_description}}\n\nAnalyze and fix.\nCurrent round: {{reflection_count}}/{{max_reflections}}" |
| `check_completion` | `task_goals`, `current_status` | "Check task completion:\nGoals: {{task_goals}}\nStatus: {{current_status}}\n\nAll goals achieved?" |
| `confirming` | `task_description`, `changes_summary`, `test_results` | "Task ready for confirmation:\nTask: {{task_description}}\nChanges: {{changes_summary}}\nTests: {{test_results}}\n\nConfirm completion?" |
| `human_decision` | `question`, `context` | "Human decision required:\nQuestion: {{question}}\nContext: {{context}}" |

**Acceptance Criteria**:
- [x] Templates defined for all decision-requiring stages
- [x] All variables documented
- [x] Templates are clear and actionable

### 4.4 Auto-Check System [P0]

#### FR-011: Automatic Quality Verification

**Requirement**: Automatically check AI output quality without human intervention.

**Specification**:

```rust
pub enum AutoCheckResult {
    /// Output passes all checks
    Pass,

    /// Output has issues requiring reflection
    NeedsReflection { reason: String },

    /// Output has issues requiring human decision
    NeedsHuman { reason: String },
}

pub struct AutoChecker {
    /// Quality check rules
    rules: Vec<CheckRule>,
}

impl AutoChecker {
    /// Evaluate output against all rules
    pub fn check(&self, task: &Task, output: &AgentOutput) -> AutoCheckResult;
}
```

**Check Rules**:

| Rule | Condition | Result |
|------|-----------|--------|
| Syntax Check | Has syntax errors | NeedsReflection |
| Test Check | Tests fail | NeedsReflection |
| Compilation Check | Compile errors | NeedsReflection |
| Boundary Check | Exceeds task constraints | NeedsHuman |
| Risk Check | Contains risky operations | NeedsHuman |
| Style Check | Style violations | NeedsReflection (if configured) |
| Completion Check | Goals not achieved | NeedsReflection |

**Acceptance Criteria**:
- [x] AutoChecker implemented with all rules
- [x] check() returns correct result type
- [x] Each rule is independently configurable

#### FR-012: Intelligent Human-Escalation Filter

**Requirement**: Filter decisions to only escalate genuinely human-required issues.

**Specification**:

**Should Escalate to Human**:

| Condition | Example |
|-----------|----------|
| Exceeds task boundary | AI modifies files outside task scope |
| Design decision required | Multiple valid solutions exist |
| High-risk operation | Data deletion, permission changes |
| Dependency conflict | Changes conflict with other tasks |
| Cannot auto-verify | UI/UX decisions, subjective quality |
| Reflection limit reached | Issue persists after max rounds |

**Should NOT Escalate (Auto-handle)**:

| Condition | Action |
|-----------|--------|
| Syntax error | Auto-reflect |
| Test failure | Auto-reflect |
| Code style issue | Auto-reflect (or auto-fix) |
| Normal iteration | Auto-continue |
| Verification pass | Auto-confirm (pending human final approval) |

**Acceptance Criteria**:
- [x] DecisionFilter implemented
- [x] Filter correctly classifies issues
- [x] Filter is configurable (add/remove rules)

### 4.5 Task Registry [P1]

#### FR-013: Task Storage and Retrieval

**Requirement**: Store and retrieve tasks with persistence.

**Specification**:

```rust
pub struct TaskRegistry {
    /// Active tasks currently being executed
    active_tasks: HashMap<TaskId, Task>,

    /// Completed tasks for history
    completed_tasks: Vec<Task>,

    /// Storage backend
    store: TaskStore,
}

impl TaskRegistry {
    /// Create a new task
    pub fn create(&mut self, description: String, constraints: Vec<String>) -> TaskId;

    /// Get a task by ID
    pub fn get(&self, id: TaskId) -> Option<&Task>;

    /// Update a task
    pub fn update(&mut self, id: TaskId, update: TaskUpdate) -> Result<(), Error>;

    /// Complete a task
    pub fn complete(&mut self, id: TaskId) -> Result<(), Error>;

    /// Cancel a task
    pub fn cancel(&mut self, id: TaskId, reason: String) -> Result<(), Error>;

    /// List all active tasks
    pub fn list_active(&self) -> Vec<&Task>;

    /// Load tasks from storage
    pub fn load(&mut self) -> Result<(), Error>;

    /// Save all tasks to storage
    pub fn save(&self) -> Result<(), Error>;
}
```

**Acceptance Criteria**:
- [x] TaskRegistry implemented
- [x] Tasks persist across session restarts
- [x] CRUD operations work correctly
- [x] Concurrent access is safe

### 4.6 Execution History [P1]

#### FR-014: Action Logging

**Requirement**: Log all actions taken during task execution.

**Specification**:

```rust
pub struct ExecutionRecord {
    /// Action taken
    pub action: DecisionAction,

    /// Timestamp of action
    pub timestamp: DateTime<Utc>,

    /// Stage when action was taken
    pub stage: StageId,

    /// Auto-check result (if applicable)
    pub auto_check: Option<AutoCheckResult>,

    /// Whether human was requested
    pub human_requested: bool,

    /// Human response (if provided)
    pub human_response: Option<String>,

    /// AI output that triggered this action
    pub triggering_output: Option<String>,
}

impl Task {
    /// Add execution record
    pub fn add_record(&mut self, record: ExecutionRecord);

    /// Get all records
    pub fn get_history(&self) -> &Vec<ExecutionRecord>;

    /// Get records from specific stage
    pub fn get_stage_history(&self, stage: StageId) -> Vec<&ExecutionRecord>;
}
```

**Acceptance Criteria**:
- [x] ExecutionRecord defined
- [x] Records added for every decision
- [x] History can be queried by stage, time, action type

### 4.7 Decision Engine [P1]

#### FR-015: Decision Engine Implementation

**Requirement**: Implement the core decision-making engine.

**Specification**:

```rust
pub struct DecisionEngine {
    /// Current decision process
    process: DecisionProcess,

    /// Current stage
    current_stage: StageId,

    /// Task being processed
    task: Task,

    /// Prompt templates
    templates: HashMap<String, PromptTemplate>,

    /// Auto-checker
    checker: AutoChecker,

    /// Human escalation filter
    filter: DecisionFilter,
}

impl DecisionEngine {
    /// Create engine with process and task
    pub fn new(process: DecisionProcess, task: Task) -> Self;

    /// Process AI output and make decision
    pub fn process_output(&mut self, output: AgentOutput) -> DecisionAction;

    /// Generate prompt for current stage
    pub fn generate_prompt(&self) -> String;

    /// Handle human response
    pub fn handle_human_response(&mut self, response: HumanResponse) -> DecisionAction;

    /// Get current task status
    pub fn get_status(&self) -> TaskStatus;

    /// Get reflection count
    pub fn reflection_count(&self) -> usize;

    /// Check if task is complete
    pub fn is_complete(&self) -> bool;
}
```

**Decision Logic**:

```
AI Output → Auto-Check → Filter
                         ├─ Auto-handle → Execute action
                         └─ Needs human → RequestHuman
```

**Acceptance Criteria**:
- [x] DecisionEngine implemented
- [x] process_output() correctly evaluates conditions
- [x] Transitions follow defined workflow
- [x] Prompts generated correctly

### 4.8 Metrics Collection [P2]

#### FR-016: Decision Metrics

**Requirement**: Collect metrics to evaluate decision layer effectiveness.

**Specification**:

```rust
pub struct DecisionMetrics {
    /// Number of automated decisions
    auto_decisions: usize,

    /// Number of human decisions
    human_decisions: usize,

    /// Total reflection cycles
    total_reflections: usize,

    /// Total confirmation attempts
    total_confirmations: usize,

    /// Successfully completed tasks
    completed_tasks: usize,

    /// Cancelled tasks
    cancelled_tasks: usize,

    /// Average task duration
    avg_duration: Duration,

    /// Average reflection count per task
    avg_reflections: f64,
}

impl DecisionMetrics {
    /// Calculate automation rate
    pub fn automation_rate(&self) -> f64 {
        self.auto_decisions / (self.auto_decisions + self.human_decisions)
    }

    /// Calculate completion rate
    pub fn completion_rate(&self) -> f64 {
        self.completed_tasks / (self.completed_tasks + self.cancelled_tasks)
    }
}
```

**Target Metrics**:

| Metric | Target |
|--------|--------|
| Automation rate | > 80% |
| Human intervention rate | < 20% |
| Average reflections | 1-2 per task |
| Completion rate | > 90% |

**Acceptance Criteria**:
- [x] Metrics collected for all decisions
- [x] Metrics persisted for analysis
- [x] Metrics viewable in TUI

### 4.9 Custom Workflows [P2]

#### FR-017: User-Defined Decision Processes

**Requirement**: Allow users to define custom decision workflows.

**Specification**:

```rust
/// Load process from configuration file
pub fn load_process(path: &Path) -> Result<DecisionProcess, Error>;

/// Register custom condition evaluator
pub fn register_condition(name: String, evaluator: fn(&EvaluationContext) -> bool);

/// Register custom check rule
pub fn register_check_rule(rule: CheckRule);
```

**Configuration Format (YAML)**:

```yaml
process:
  name: "Bug Fix Workflow"
  description: "Structured bug fixing process"
  stages:
    - id: locate
      name: "Locate Problem"
      transitions:
        - target: understand
          condition: "problem_located"
    - id: understand
      name: "Understand Cause"
      transitions:
        - target: design_fix
          condition: "cause_understood"
    - id: design_fix
      name: "Design Fix"
      transitions:
        - target: implement
          condition: "fix_designed"
    - id: implement
      name: "Implement Fix"
      transitions:
        - target: verify
          condition: "fix_implemented"
    - id: verify
      name: "Verify Fix"
      transitions:
        - target: check_side_effects
          condition: "tests_pass"
        - target: implement
          condition: "tests_fail"
    - id: check_side_effects
      name: "Check Side Effects"
      transitions:
        - target: completed
          condition: "no_side_effects"
        - target: human_decision
          condition: "has_side_effects"
  initial_stage: locate
  final_stage: completed
  config:
    max_reflection_rounds: 3
    enforce_verification: true
    timeout: 20min
```

**Acceptance Criteria**:
- [x] YAML configuration supported
- [x] Custom processes can be loaded
- [x] Custom conditions can be registered
- [x] Invalid configurations return clear errors

---

## 5. Non-Functional Requirements

### 5.1 Performance

| Requirement | Specification |
|-------------|---------------|
| Auto-check latency | < 100ms per evaluation |
| Decision latency | < 50ms after AI output |
| Task persistence | < 10ms per write |
| History query | < 50ms for 100 records |

### 5.2 Reliability

| Requirement | Specification |
|-------------|---------------|
| Task recovery | Tasks recoverable after crash |
| Decision logging | All decisions logged before execution |
| Graceful degradation | If auto-check fails, escalate to human |

### 5.3 Maintainability

| Requirement | Specification |
|-------------|---------------|
| Workflow modification | Workflows editable without code changes |
| Rule addition | New check rules addable via API |
| Template modification | Templates editable in config files |

### 5.4 Usability

| Requirement | Specification |
|-------------|---------------|
| Task visibility | All task states visible in TUI |
| History access | Execution history queryable in TUI |
| Human intervention | Clear prompts for human decisions |
| Metrics view | Automation metrics visible in TUI |

---

## 6. Integration Requirements

### 6.1 TUI Integration

**Requirement**: Display task status in TUI dashboard.

**Specification**:

- Task panel showing active tasks with status, reflection count
- Detail view showing execution history
- Human decision overlay for escalation events
- Metrics panel showing automation statistics

### 6.2 Agent Slot Integration

**Requirement**: Associate tasks with agent slots.

**Specification**:

```rust
pub struct AgentSlot {
    /// Current task (if any)
    pub current_task: Option<TaskId>,
    /// Decision engine for this slot
    pub decision_engine: Option<DecisionEngine>,
}
```

### 6.3 Kanban Integration

**Requirement**: Sync task status with Kanban ticket status.

**Specification**:

| Decision Task Status | Kanban Ticket Status |
|---------------------|---------------------|
| InProgress | IN_PROGRESS |
| Completed | DONE |
| Cancelled | DONE (with note) |
| NeedsHumanDecision | BLOCKED |

---

## 7. Testing Requirements

### 7.1 Unit Tests

- [x] Task creation, update, completion
- [x] All status transitions valid
- [x] Condition evaluation correctness
- [x] Auto-check rule accuracy
- [x] Prompt template rendering
- [x] Metrics calculation

### 7.2 Integration Tests

- [x] Decision engine full workflow execution
- [x] TUI task display
- [x] Persistence and recovery
- [x] Human intervention flow

### 7.3 Acceptance Tests

- [x] Automation rate > 80% with sample tasks
- [x] All escalation cases correctly identified
- [x] Task completes after successful AI execution
- [x] Task correctly handles repeated failures

---

## 8. Implementation Phases

### Phase 1: Core Entities (P0)

1. Task struct with status enum
2. DecisionStage, Condition, DecisionAction
3. DecisionProcess definition
4. Default process implementation

### Phase 2: Decision Logic (P0)

1. Prompt templates
2. Auto-check system
3. Human escalation filter
4. Decision engine

### Phase 3: Persistence (P1)

1. Task registry with storage
2. Execution history logging
3. Recovery mechanism

### Phase 4: TUI Integration (P1)

1. Task panel in dashboard
2. History detail view
3. Human decision overlay integration

### Phase 5: Metrics & Customization (P2)

1. Metrics collection
2. Custom workflow support
3. Configuration files

---

## 9. Dependencies

| Dependency | Purpose |
|------------|---------|
| `serde` | Task/process serialization |
| `chrono` | Timestamp handling |
| `uuid` | Task ID generation |
| Existing decision layer | DecisionSituation, ActionRegistry |

---

## 10. Glossary

| Term | Definition |
|------|------------|
| Task | A unit of work assigned to AI, tracked through lifecycle |
| Decision Stage | A discrete decision point in the workflow |
| Condition | A boolean predicate evaluated to determine transitions |
| Transition | A directed edge from one stage to another |
| Reflection | AI self-correction cycle to fix issues |
| Confirmation | Human approval of task completion |
| Auto-check | Automatic quality/completion verification |
| Escalation | Routing a decision to human intervention |
| Process | A complete decision workflow definition |

---

## 11. References

- Brainstorming document: `docs/决策层任务概念设计思考.md`
- Decision layer architecture: `docs/plan/spec/decision-layer/README.md`
- Current decision actions: `decision/src/builtin_actions.rs`
- Prompt templates: `decision/src/prompts/mod.rs`