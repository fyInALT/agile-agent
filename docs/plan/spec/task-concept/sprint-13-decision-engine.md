# Sprint 13: Decision Engine Integration

## Metadata

- Sprint ID: `decision-sprint-013`
- Title: `Decision Engine Integration`
- Duration: 2 weeks
- Priority: P1 (High)
- Status: `Completed`
- Created: 2026-04-18
- Depends on: `decision-sprint-009`, `decision-sprint-010`, `decision-sprint-011`, `decision-sprint-012`

## Reference

See [Task Concept Requirements](../../decision-layer-task-concept-requirements.md) for detailed requirements:
- FR-015: Decision Engine Implementation

## Sprint Goal

Integrate all previous sprint components into a working DecisionEngine that processes AI output and makes decisions.

## Context

With Task entities, workflows, automation, and persistence implemented, we now need to combine them into a cohesive engine that:
1. Processes AI output
2. Evaluates conditions
3. Makes decisions (auto or human)
4. Executes transitions
5. Logs all actions

This sprint is the integration point where all components work together.

## Stories

### Story 13.1: Decision Engine Core

**Priority**: P1
**Effort**: 5 points
**Status**: Backlog

Implement the core DecisionEngine struct.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T13.1.1 | Create `DecisionEngine` struct | Todo | - |
| T13.1.2 | Store process, stage, task | Todo | - |
| T13.1.3 | Integrate templates, checker, filter | Todo | - |
| T13.1.4 | Implement `new()` constructor | Todo | - |
| T13.1.5 | Write unit tests for engine creation | Todo | - |

#### TDD Test Tasks

| Test ID | Definition |
|---------|------------|
| T13.1.T1 | Engine created with default process |
| T13.1.T2 | Engine created with custom process |
| T13.1.T3 | Initial stage set correctly |
| T13.1.T4 | Components integrated |

#### Acceptance Criteria

- DecisionEngine matches FR-015 specification
- All components properly integrated
- Engine initialized correctly

#### Technical Notes

```rust
pub struct DecisionEngine {
    process: DecisionProcess,
    current_stage: StageId,
    task: Task,
    templates: HashMap<String, PromptTemplate>,
    checker: AutoChecker,
    filter: DecisionFilter,
    registry: TaskRegistry,
}

impl DecisionEngine {
    pub fn new(
        process: DecisionProcess,
        task: Task,
        registry: TaskRegistry,
    ) -> Self {
        Self {
            process,
            current_stage: process.initial_stage.clone(),
            task,
            templates: default_templates(),
            checker: AutoChecker::default(),
            filter: DecisionFilter::default(),
            registry,
        }
    }
    
    pub fn with_templates(mut self, templates: HashMap<String, PromptTemplate>) -> Self {
        self.templates = templates;
        self
    }
    
    pub fn with_checker(mut self, checker: AutoChecker) -> Self {
        self.checker = checker;
        self
    }
    
    pub fn with_filter(mut self, filter: DecisionFilter) -> Self {
        self.filter = filter;
        self
    }
}
```

### Story 13.2: Output Processing

**Priority**: P1
**Effort**: 5 points
**Status**: Backlog

Implement `process_output()` to evaluate AI output and make decisions.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T13.2.1 | Implement `process_output()` method | Todo | - |
| T13.2.2 | Evaluate exit condition | Todo | - |
| T13.2.3 | Check transitions | Todo | - |
| T13.2.4 | Evaluate auto-check | Todo | - |
| T13.2.5 | Apply filter for human decisions | Todo | - |
| T13.2.6 | Execute stage actions | Todo | - |
| T13.2.7 | Log decision in history | Todo | - |
| T13.2.8 | Write unit tests for processing | Todo | - |

#### TDD Test Tasks

| Test ID | Definition |
|---------|------------|
| T13.2.T1 | Clean output → Continue |
| T13.2.T2 | Syntax error → Reflect |
| T13.2.T3 | Tests fail → Reflect |
| T13.2.T4 | Boundary violation → RequestHuman |
| T13.2.T5 | Goals achieved → AdvanceTo confirming |
| T13.2.T6 | Max reflections → RequestHuman |
| T13.2.T7 | Decision logged |

#### Acceptance Criteria

- `process_output()` correctly processes all cases
- Decisions routed correctly
- All decisions logged

#### Technical Notes

```rust
impl DecisionEngine {
    pub fn process_output(&mut self, output: AgentOutput) -> DecisionAction {
        // Get current stage
        let stage = self.get_current_stage();
        
        // Auto-check the output
        let check_result = self.checker.check(&self.task, &output);
        
        // Determine decision
        let decision = match check_result {
            AutoCheckResult::Pass => {
                // Check if transition condition met
                self.evaluate_transition(&output)
            }
            AutoCheckResult::NeedsReflection { reason } => {
                if self.task.reflection_count < self.task.max_reflection_rounds {
                    self.task.reflection_count += 1;
                    DecisionAction::Reflect { reason }
                } else {
                    DecisionAction::RequestHuman { question: reason }
                }
            }
            AutoCheckResult::NeedsHuman { reason } => {
                // Check if filter agrees
                if let Some(filtered_reason) = self.filter.needs_human_decision(&self.task, &output) {
                    DecisionAction::RequestHuman { question: filtered_reason }
                } else {
                    // Filter disagrees, auto-decide
                    self.filter.auto_decide(&self.task, &output)
                }
            }
        };
        
        // Log the decision
        self.log_decision(decision.clone(), check_result, Some(output));
        
        // Execute decision effects
        self.execute_decision(&decision);
        
        decision
    }
    
    fn evaluate_transition(&mut self, output: &AgentOutput) -> DecisionAction {
        let stage = self.get_current_stage();
        let ctx = EvaluationContext {
            task: self.task.clone(),
            output: Some(output.clone()),
            test_results: None,
            reflection_count: self.task.reflection_count,
            config: self.process.config.clone(),
        };
        
        for transition in &stage.transitions {
            if transition.condition.evaluate(&ctx) {
                self.current_stage = transition.target.clone();
                return DecisionAction::AdvanceTo { stage: transition.target };
            }
        }
        
        // No transition, auto-decide
        self.filter.auto_decide(&self.task, output)
    }
    
    fn execute_decision(&mut self, decision: &DecisionAction) {
        match decision {
            DecisionAction::Reflect { .. } => {
                self.task.transition_to(TaskStatus::Reflecting);
            }
            DecisionAction::ConfirmCompletion => {
                self.task.confirmation_count += 1;
                self.task.transition_to(TaskStatus::PendingConfirmation);
            }
            DecisionAction::RequestHuman { .. } => {
                self.task.transition_to(TaskStatus::NeedsHumanDecision);
            }
            DecisionAction::AdvanceTo { stage } => {
                self.current_stage = stage.clone();
                // Update task status based on stage
                self.update_status_for_stage(stage);
            }
            DecisionAction::Cancel { .. } => {
                self.task.transition_to(TaskStatus::Cancelled);
            }
            _ => {}
        }
        
        // Save task state
        self.registry.update(&self.task.id, TaskUpdate::from_task(&self.task));
    }
}
```

### Story 13.3: Prompt Generation

**Priority**: P1
**Effort**: 3 points
**Status**: Backlog

Implement `generate_prompt()` for each stage.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T13.3.1 | Implement `generate_prompt()` method | Todo | - |
| T13.3.2 | Collect variable values | Todo | - |
| T13.3.3 | Render template | Todo | - |
| T13.3.4 | Handle missing variables | Todo | - |
| T13.3.5 | Write unit tests for prompts | Todo | - |

#### TDD Test Tasks

| Test ID | Definition |
|---------|------------|
| T13.3.T1 | Prompt generated for start stage |
| T13.3.T2 | Prompt generated for reflecting stage |
| T13.3.T3 | Variables substituted correctly |
| T13.3.T4 | Missing variable handled |

#### Acceptance Criteria

- Prompts generated correctly for all stages
- Variables substituted from context

#### Technical Notes

```rust
impl DecisionEngine {
    pub fn generate_prompt(&self) -> String {
        let template = self.templates.get(&self.current_stage.to_string())
            .unwrap_or_else(|| self.templates.get("default").unwrap());
        
        let values = self.collect_prompt_values();
        template.render(&values).unwrap_or_else(|_| template.content.clone())
    }
    
    fn collect_prompt_values(&self) -> HashMap<String, String> {
        let mut values = HashMap::new();
        
        values.insert("task_description", self.task.description.clone());
        values.insert("task_constraints", self.task.constraints.join(", "));
        values.insert("reflection_count", self.task.reflection_count.to_string());
        values.insert("max_reflections", self.task.max_reflection_rounds.to_string());
        values.insert("task_goals", self.task.description.clone()); // Simplified
        
        // Add stage-specific values
        // ...
        
        values
    }
}
```

### Story 13.4: Human Response Handling

**Priority**: P1
**Effort**: 4 points
**Status**: Backlog

Implement handling of human decisions and feedback.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T13.4.1 | Define `HumanResponse` type | Todo | - |
| T13.4.2 | Implement `handle_human_response()` | Todo | - |
| T13.4.3 | Process approve/deny/custom | Todo | - |
| T13.4.4 | Log human response | Todo | - |
| T13.4.5 | Write unit tests for responses | Todo | - |

#### TDD Test Tasks

| Test ID | Definition |
|---------|------------|
| T13.4.T1 | Approve → Continue |
| T13.4.T2 | Deny → Reflect or Cancel |
| T13.4.T3 | Custom feedback → Reflect with feedback |
| T13.4.T4 | Response logged |

#### Acceptance Criteria

- Human responses handled correctly
- All response types supported
- Response logged in history

#### Technical Notes

```rust
#[derive(Debug, Clone)]
pub enum HumanResponse {
    Approve,
    Deny { reason: String },
    Custom { feedback: String },
    Cancel,
}

impl DecisionEngine {
    pub fn handle_human_response(&mut self, response: HumanResponse) -> DecisionAction {
        let decision = match response {
            HumanResponse::Approve => {
                self.task.transition_to(TaskStatus::InProgress);
                DecisionAction::Continue
            }
            HumanResponse::Deny { reason } => {
                if self.task.reflection_count < self.task.max_reflection_rounds {
                    self.task.reflection_count += 1;
                    self.task.transition_to(TaskStatus::Reflecting);
                    DecisionAction::Reflect { reason }
                } else {
                    self.task.transition_to(TaskStatus::Cancelled);
                    DecisionAction::Cancel { reason: "Human denied after max reflections".into() }
                }
            }
            HumanResponse::Custom { feedback } => {
                self.task.reflection_count += 1;
                self.task.transition_to(TaskStatus::Reflecting);
                DecisionAction::Reflect { reason: feedback }
            }
            HumanResponse::Cancel => {
                self.task.transition_to(TaskStatus::Cancelled);
                DecisionAction::Cancel { reason: "Human cancelled".into() }
            }
        };
        
        // Log human response
        self.log_decision(decision.clone(), None, None);
        
        decision
    }
}
```

### Story 13.5: Status Queries

**Priority**: P2
**Effort**: 2 points
**Status**: Backlog

Implement status query methods for TUI.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T13.5.1 | Implement `get_status()` | Todo | - |
| T13.5.2 | Implement `reflection_count()` | Todo | - |
| T13.5.3 | Implement `is_complete()` | Todo | - |
| T13.5.4 | Implement `get_current_stage()` | Todo | - |
| T13.5.5 | Write unit tests for queries | Todo | - |

#### TDD Test Tasks

| Test ID | Definition |
|---------|------------|
| T13.5.T1 | get_status() returns task status |
| T13.5.T2 | reflection_count() returns count |
| T13.5.T3 | is_complete() returns true when completed |

#### Acceptance Criteria

- All query methods implemented
- Methods used by TUI in Sprint 14

## Sprint Deliverables

1. `DecisionEngine` struct integrating all components
2. `process_output()` decision logic
3. `generate_prompt()` for stages
4. `HumanResponse` handling
5. Status query methods
6. Integration tests for full flow

## Sprint Review Checklist

- [x] All tasks completed
- [x] All tests passing
- [x] Engine processes output correctly
- [x] Decisions routed correctly
- [x] Human responses handled
- [x] Code reviewed and merged

## Risks and Mitigation

| Risk | Mitigation |
|------|------------|
| Complex decision logic | Break into clear functions, document |
| Integration issues | Thorough integration tests |
| Performance concerns | Profile and optimize hot paths |

## Next Sprint Preview

Sprint 14 will add TUI integration:
- Task panel in dashboard
- History detail view
- Human decision overlay