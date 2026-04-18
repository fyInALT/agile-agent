# Sprint 10: Decision Workflow Core

## Metadata

- Sprint ID: `decision-sprint-010`
- Title: `Decision Workflow Core`
- Duration: 2 weeks
- Priority: P0 (Critical)
- Status: `Backlog`
- Created: 2026-04-18
- Depends on: `decision-sprint-009` (Task Entity)

## Reference

See [Task Concept Requirements](../../decision-layer-task-concept-requirements.md) for detailed requirements:
- FR-004: Decision Stage Definition
- FR-005: Condition System
- FR-006: Decision Action Definition
- FR-007: Decision Process Definition
- FR-008: Default Process - Simple Agile

## Sprint Goal

Establish the structured decision workflow system with stages, conditions, actions, and process definitions, enabling customizable decision flows.

## Context

Building on the Task entity from Sprint 09, this sprint introduces the workflow abstraction. A decision process is composed of stages, each with entry/exit conditions and transitions to other stages. This structure enables:

1. Clear separation of decision phases
2. Customizable workflows per task type
3. Explicit condition evaluation
4. Future user-defined workflows

## Stories

### Story 10.1: Decision Stage Definition

**Priority**: P0
**Effort**: 4 points
**Status**: Backlog

Define the DecisionStage struct with all workflow components.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T10.1.1 | Define `StageId` type | Todo | - |
| T10.1.2 | Create `DecisionStage` struct | Todo | - |
| T10.1.3 | Define `StageTransition` struct | Todo | - |
| T10.1.4 | Add entry/exit condition fields | Todo | - |
| T10.1.5 | Implement `Serialize/Deserialize` | Todo | - |
| T10.1.6 | Write unit tests for stage creation | Todo | - |

#### TDD Test Tasks

| Test ID | Definition |
|---------|------------|
| T10.1.T1 | StageId creation and comparison |
| T10.1.T2 | Stage created with correct defaults |
| T10.1.T3 | Transitions stored correctly |
| T10.1.T4 | Stage serialization works |

#### Acceptance Criteria

- DecisionStage struct matches FR-004 specification
- StageId is unique within a process
- Transitions are directed edges

#### Technical Notes

```rust
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct StageId(String);

impl StageId {
    pub fn new(name: impl Into<String>) -> Self {
        Self(name.into())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecisionStage {
    pub id: StageId,
    pub name: String,
    pub description: String,
    pub entry_condition: Condition,
    pub exit_condition: Condition,
    pub transitions: Vec<StageTransition>,
    pub actions: Vec<DecisionAction>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StageTransition {
    pub target: StageId,
    pub condition: Condition,
    pub prompt: String,
}
```

### Story 10.2: Condition System

**Priority**: P0
**Effort**: 5 points
**Status**: Backlog

Implement the condition evaluation system with built-in and composite conditions.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T10.2.1 | Define `Condition` enum with built-in types | Todo | - |
| T10.2.2 | Define composite conditions (All, Any, Not) | Todo | - |
| T10.2.3 | Create `EvaluationContext` struct | Todo | - |
| T10.2.4 | Implement `Condition::evaluate()` | Todo | - |
| T10.2.5 | Implement composite condition evaluation | Todo | - |
| T10.2.6 | Add Custom condition placeholder | Todo | - |
| T10.2.7 | Write unit tests for each condition type | Todo | - |

#### TDD Test Tasks

| Test ID | Definition |
|---------|------------|
| T10.2.T1 | TestsPass condition evaluates correctly |
| T10.2.T2 | NoCompileErrors condition evaluates |
| T10.2.T3 | GoalsAchieved condition evaluates |
| T10.2.T4 | MaxReflectionsReached evaluates |
| T10.2.T5 | All(conditions) evaluates all must pass |
| T10.2.T6 | Any(conditions) evaluates one must pass |
| T10.2.T7 | Not(condition) negates result |
| T10.2.T8 | Custom condition placeholder works |

#### Acceptance Criteria

- All FR-005 condition types implemented
- Composite conditions work correctly
- EvaluationContext provides required data

#### Technical Notes

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
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
    
    // Custom condition
    Custom(String),
}

#[derive(Debug, Clone)]
pub struct EvaluationContext {
    pub task: Task,
    pub output: Option<AgentOutput>,
    pub test_results: Option<TestResults>,
    pub reflection_count: usize,
    pub config: ProcessConfig,
}

impl Condition {
    pub fn evaluate(&self, ctx: &EvaluationContext) -> bool {
        match self {
            Self::TestsPass => ctx.test_results.as_ref().map(|r| r.all_pass()).unwrap_or(false),
            Self::NoCompileErrors => ctx.output.as_ref().map(|o| !o.has_compile_errors()).unwrap_or(true),
            Self::GoalsAchieved => ctx.task.status == TaskStatus::PendingConfirmation,
            Self::MaxReflectionsReached => ctx.reflection_count >= ctx.config.max_reflection_rounds,
            Self::HumanApproved => false, // Set by human response
            Self::TimeoutExceeded => false, // Set by timer
            Self::All(conditions) => conditions.iter().all(|c| c.evaluate(ctx)),
            Self::Any(conditions) => conditions.iter().any(|c| c.evaluate(ctx)),
            Self::Not(condition) => !condition.evaluate(ctx),
            Self::Custom(name) => false, // Placeholder for custom evaluators
        }
    }
}
```

### Story 10.3: Decision Action Definition

**Priority**: P0
**Effort**: 3 points
**Status**: Backlog

Define all decision actions that can be taken at each stage.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T10.3.1 | Define `DecisionAction` enum | Todo | - |
| T10.3.2 | Add action-specific fields (reason, question) | Todo | - |
| T10.3.3 | Implement `Serialize/Deserialize` | Todo | - |
| T10.3.4 | Add `to_prompt()` method for AI | Todo | - |
| T10.3.5 | Write unit tests for all actions | Todo | - |

#### TDD Test Tasks

| Test ID | Definition |
|---------|------------|
| T10.3.T1 | All action variants defined |
| T10.3.T2 | Reflect action has reason field |
| T10.3.T3 | RequestHuman action has question field |
| T10.3.T4 | Actions serialize correctly |
| T10.3.T5 | to_prompt() generates appropriate text |

#### Acceptance Criteria

- DecisionAction enum matches FR-006 specification
- Each action can be converted to prompt
- Actions are serializable for logging

#### Technical Notes

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DecisionAction {
    Continue,
    Reflect { reason: String },
    ConfirmCompletion,
    RequestHuman { question: String },
    AdvanceTo { stage: StageId },
    ReturnTo { stage: StageId },
    Cancel { reason: String },
    Retry,
    Wait { reason: String },
}

impl DecisionAction {
    pub fn to_prompt(&self) -> String {
        match self {
            Self::Continue => "Continue with current execution.",
            Self::Reflect { reason } => format!("Reflect on and fix: {}", reason),
            Self::ConfirmCompletion => "Confirm task completion.",
            Self::RequestHuman { question } => format!("Human decision needed: {}", question),
            Self::AdvanceTo { stage } => format!("Advance to stage: {}", stage),
            Self::ReturnTo { stage } => format!("Return to stage: {}", stage),
            Self::Cancel { reason } => format!("Cancel task: {}", reason),
            Self::Retry => "Retry the last operation.",
            Self::Wait { reason } => format!("Wait: {}", reason),
        }
    }
}
```

### Story 10.4: Decision Process Definition

**Priority**: P0
**Effort**: 3 points
**Status**: Backlog

Define the DecisionProcess struct that combines stages into a workflow.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T10.4.1 | Create `DecisionProcess` struct | Todo | - |
| T10.4.2 | Create `ProcessConfig` struct | Todo | - |
| T10.4.3 | Add validation for process integrity | Todo | - |
| T10.4.4 | Implement `Serialize/Deserialize` | Todo | - |
| T10.4.5 | Write unit tests for process creation | Todo | - |

#### TDD Test Tasks

| Test ID | Definition |
|---------|------------|
| T10.4.T1 | Process created with stages |
| T10.4.T2 | Initial/final stages are valid |
| T10.4.T3 | Process validation detects issues |
| T10.4.T4 | Process serialization works |

#### Acceptance Criteria

- DecisionProcess struct matches FR-007 specification
- Process validates: no unreachable stages, valid initial/final
- ProcessConfig with all required settings

#### Technical Notes

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecisionProcess {
    pub name: String,
    pub description: String,
    pub stages: Vec<DecisionStage>,
    pub initial_stage: StageId,
    pub final_stage: StageId,
    pub config: ProcessConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessConfig {
    pub max_reflection_rounds: usize,
    pub enforce_verification: bool,
    pub timeout_seconds: u64,
    pub log_decisions: bool,
}

impl Default for ProcessConfig {
    fn default() -> Self {
        Self {
            max_reflection_rounds: 2,
            enforce_verification: true,
            timeout_seconds: 1800, // 30 minutes
            log_decisions: true,
        }
    }
}

impl DecisionProcess {
    pub fn validate(&self) -> Result<(), ProcessValidationError> {
        // Check initial_stage exists
        if !self.stages.iter().any(|s| s.id == self.initial_stage) {
            return Err(ProcessValidationError::InvalidInitialStage);
        }
        // Check final_stage exists
        if !self.stages.iter().any(|s| s.id == self.final_stage) {
            return Err(ProcessValidationError::InvalidFinalStage);
        }
        // Check all transitions target valid stages
        for stage in &self.stages {
            for transition in &stage.transitions {
                if !self.stages.iter().any(|s| s.id == transition.target) {
                    return Err(ProcessValidationError::InvalidTransition {
                        from: stage.id.clone(),
                        to: transition.target.clone(),
                    });
                }
            }
        }
        Ok(())
    }
}
```

### Story 10.5: Default Process Implementation

**Priority**: P0
**Effort**: 4 points
**Status**: Backlog

Implement the default "Simple Agile" decision process.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T10.5.1 | Define 9 default stages | Todo | - |
| T10.5.2 | Implement stage transitions | Todo | - |
| T10.5.3 | Create `default_process()` function | Todo | - |
| T10.5.4 | Validate default process integrity | Todo | - |
| T10.5.5 | Write unit tests for default process | Todo | - |

#### TDD Test Tasks

| Test ID | Definition |
|---------|------------|
| T10.5.T1 | default_process() creates valid process |
| T10.5.T2 | All 9 stages defined |
| T10.5.T3 | All transitions valid |
| T10.5.T4 | Initial stage is "start" |
| T10.5.T5 | Final stage is "completed" |

#### Acceptance Criteria

- Default process matches FR-008 specification
- All 9 stages correctly defined
- All transitions correctly defined
- Process passes validation

#### Technical Notes

```rust
pub fn default_process() -> DecisionProcess {
    let stages = vec![
        DecisionStage {
            id: StageId::new("start"),
            name: "Start Development".into(),
            description: "Task starts, AI begins execution".into(),
            entry_condition: Condition::Custom("task_assigned"),
            exit_condition: Condition::Custom("ai_output"),
            transitions: vec![StageTransition {
                target: StageId::new("developing"),
                condition: Condition::Custom("ai_response"),
                prompt: "Begin implementing task".into(),
            }],
            actions: vec![DecisionAction::Continue],
        },
        // ... (9 stages total)
    ];
    
    DecisionProcess {
        name: "Simple Agile".into(),
        description: "Default agile development workflow".into(),
        stages,
        initial_stage: StageId::new("start"),
        final_stage: StageId::new("completed"),
        config: ProcessConfig::default(),
    }
}
```

See full stage definitions in FR-008.

## Sprint Deliverables

1. `StageId` and `DecisionStage` types
2. `StageTransition` with condition and prompt
3. `Condition` enum with 12+ condition types
4. `EvaluationContext` for condition evaluation
5. `DecisionAction` enum with 9 action types
6. `DecisionProcess` with validation
7. `ProcessConfig` with defaults
8. `default_process()` implementation (9 stages)
9. Unit tests with >90% coverage

## Sprint Review Checklist

- [ ] All tasks completed
- [ ] All tests passing
- [ ] Workflow types defined correctly
- [ ] Default process passes validation
- [ ] Code reviewed and merged
- [ ] Documentation updated

## Risks and Mitigation

| Risk | Mitigation |
|------|------------|
| Condition evaluation too complex | Start with simple conditions, add composite later |
| Default process too rigid | Process is configurable, can be replaced |
| Missing transition paths | Validate process, add missing transitions |

## Next Sprint Preview

Sprint 11 will build on workflow types to implement:
- Prompt templates for each stage
- Auto-check system for quality verification
- Human escalation filter for intelligent routing