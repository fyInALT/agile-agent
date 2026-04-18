# Sprint 15: Metrics and Customization

## Metadata

- Sprint ID: `decision-sprint-015`
- Title: `Metrics and Customization`
- Duration: 1-2 weeks
- Priority: P2 (Enhancement)
- Status: `Completed`
- Created: 2026-04-18
- Depends on: `decision-sprint-013` (Decision Engine), `decision-sprint-014` (TUI Integration)

## Reference

See [Task Concept Requirements](../../decision-layer-task-concept-requirements.md) for detailed requirements:
- FR-016: Decision Metrics
- FR-017: User-Defined Decision Processes

## Sprint Goal

Implement metrics collection for automation analysis and custom workflow support for user personalization.

## Context

With the core system complete, this sprint adds:
1. Metrics to measure automation effectiveness
2. Custom workflow configuration for different development styles

These features enable continuous improvement and user adaptation.

## Stories

### Story 15.1: Metrics Collection

**Priority**: P2
**Effort**: 3 points
**Status**: Backlog

Implement `DecisionMetrics` for tracking automation statistics.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T15.1.1 | Create `DecisionMetrics` struct | Todo | - |
| T15.1.2 | Track auto vs human decisions | Todo | - |
| T15.1.3 | Track reflection/confirmation counts | Todo | - |
| T15.1.4 | Track completed/cancelled tasks | Todo | - |
| T15.1.5 | Implement `automation_rate()` | Todo | - |
| T15.1.6 | Implement `completion_rate()` | Todo | - |
| T15.1.7 | Write unit tests for metrics | Todo | - |

#### TDD Test Tasks

| Test ID | Definition |
|---------|------------|
| T15.1.T1 | Metrics counts auto decisions |
| T15.1.T2 | Metrics counts human decisions |
| T15.1.T3 | automation_rate() calculated correctly |
| T15.1.T4 | completion_rate() calculated correctly |
| T15.1.T5 | Metrics persist across sessions |

#### Acceptance Criteria

- Metrics match FR-016 specification
- automation_rate() > 80% target visible
- Metrics persisted for analysis

#### Technical Notes

```rust
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DecisionMetrics {
    pub auto_decisions: usize,
    pub human_decisions: usize,
    pub total_reflections: usize,
    pub total_confirmations: usize,
    pub completed_tasks: usize,
    pub cancelled_tasks: usize,
    pub total_duration_seconds: u64,
}

impl DecisionMetrics {
    pub fn automation_rate(&self) -> f64 {
        let total = self.auto_decisions + self.human_decisions;
        if total == 0 { return 0.0; }
        self.auto_decisions as f64 / total as f64
    }
    
    pub fn completion_rate(&self) -> f64 {
        let total = self.completed_tasks + self.cancelled_tasks;
        if total == 0 { return 0.0; }
        self.completed_tasks as f64 / total as f64
    }
    
    pub fn avg_reflections(&self) -> f64 {
        if self.completed_tasks == 0 { return 0.0; }
        self.total_reflections as f64 / self.completed_tasks as f64
    }
    
    pub fn avg_duration(&self) -> Duration {
        if self.completed_tasks == 0 { return Duration::ZERO; }
        Duration::from_secs(self.total_duration_seconds / self.completed_tasks as u64)
    }
    
    pub fn record_auto_decision(&mut self) {
        self.auto_decisions += 1;
    }
    
    pub fn record_human_decision(&mut self) {
        self.human_decisions += 1;
    }
    
    pub fn record_task_completion(&mut self, reflections: usize, duration: Duration) {
        self.completed_tasks += 1;
        self.total_reflections += reflections;
        self.total_duration_seconds += duration.as_secs();
    }
    
    pub fn record_task_cancellation(&mut self) {
        self.cancelled_tasks += 1;
    }
}
```

### Story 15.2: Metrics Integration

**Priority**: P2
**Effort**: 2 points
**Status**: Backlog

Integrate metrics collection into DecisionEngine.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T15.2.1 | Add metrics field to DecisionEngine | Todo | - |
| T15.2.2 | Record on auto decision | Todo | - |
| T15.2.3 | Record on human decision | Todo | - |
| T15.2.4 | Record on task completion/cancellation | Todo | - |
| T15.2.5 | Persist metrics with registry | Todo | - |
| T15.2.6 | Write integration tests | Todo | - |

#### TDD Test Tasks

| Test ID | Definition |
|---------|------------|
| T15.2.T1 | Auto decision increments auto_decisions |
| T15.2.T2 | Human decision increments human_decisions |
| T15.2.T3 | Task completion updates metrics |
| T15.2.T4 | Metrics saved to storage |

#### Acceptance Criteria

- Metrics collected for every decision
- Metrics persisted across sessions

### Story 15.3: YAML Process Configuration

**Priority**: P2
**Effort**: 4 points
**Status**: Backlog

Implement YAML-based custom process configuration.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T15.3.1 | Define YAML schema for process | Todo | - |
| T15.3.2 | Implement `load_process()` from YAML | Todo | - |
| T15.3.3 | Parse stages and transitions | Todo | - |
| T15.3.4 | Parse conditions (built-in and custom) | Todo | - |
| T15.3.5 | Validate loaded process | Todo | - |
| T15.3.6 | Handle parse errors gracefully | Todo | - |
| T15.3.7 | Write unit tests for YAML parsing | Todo | - |

#### TDD Test Tasks

| Test ID | Definition |
|---------|------------|
| T15.3.T1 | Simple process loaded from YAML |
| T15.3.T2 | Stages parsed correctly |
| T15.3.T3 | Transitions parsed correctly |
| T15.3.T4 | Invalid YAML returns error |
| T15.3.T5 | Missing fields handled |

#### Acceptance Criteria

- YAML configuration matches FR-017 specification
- Custom processes can be loaded
- Invalid configs return clear errors

#### Technical Notes

```yaml
# Example: bug-fix-workflow.yaml
process:
  name: "Bug Fix Workflow"
  description: "Structured bug fixing process"
  stages:
    - id: locate
      name: "Locate Problem"
      description: "Find the bug location"
      entry_condition: "task_assigned"
      exit_condition: "problem_located"
      transitions:
        - target: understand
          condition: "problem_located"
          prompt: "Problem located, analyze cause"
      actions:
        - Continue
    
    - id: understand
      name: "Understand Cause"
      transitions:
        - target: design_fix
          condition: "cause_understood"
          prompt: "Cause understood, design fix"
    
    - id: design_fix
      name: "Design Fix"
      transitions:
        - target: implement
          condition: "fix_designed"
          prompt: "Fix designed, implement"
    
    - id: implement
      name: "Implement Fix"
      transitions:
        - target: verify
          condition: "fix_implemented"
          prompt: "Fix implemented, verify"
        - target: design_fix
          condition: "implementation_failed"
          prompt: "Implementation failed, redesign"
    
    - id: verify
      name: "Verify Fix"
      transitions:
        - target: check_side_effects
          condition: TestsPass
          prompt: "Tests pass, check side effects"
        - target: implement
          condition: TestsFail
          prompt: "Tests fail, re-implement"
    
    - id: check_side_effects
      name: "Check Side Effects"
      transitions:
        - target: completed
          condition: "no_side_effects"
          prompt: "No side effects, complete"
        - target: human_decision
          condition: "has_side_effects"
          prompt: "Side effects detected, need decision"
    
    - id: completed
      name: "Completed"
      transitions: []
      actions: []
    
    - id: human_decision
      name: "Human Decision"
      transitions:
        - target: implement
          condition: HumanApproved
          prompt: "Human approved, continue"
        - target: cancelled
          condition: "human_cancelled"
          prompt: "Task cancelled"
    
    - id: cancelled
      name: "Cancelled"
      transitions: []
      actions: []
  
  initial_stage: locate
  final_stage: completed
  config:
    max_reflection_rounds: 3
    enforce_verification: true
    timeout_seconds: 1200
```

```rust
// In decision/src/config/yaml_loader.rs

use serde_yaml;

pub fn load_process(path: &Path) -> Result<DecisionProcess, ConfigError> {
    let content = fs::read_to_string(path)?;
    let config: ProcessYaml = serde_yaml::from_str(&content)?;
    
    let process = config.to_process()?;
    process.validate()?;
    
    Ok(process)
}

#[derive(Debug, Deserialize)]
struct ProcessYaml {
    process: ProcessConfigYaml,
}

#[derive(Debug, Deserialize)]
struct ProcessConfigYaml {
    name: String,
    description: String,
    stages: Vec<StageYaml>,
    initial_stage: String,
    final_stage: String,
    config: Option<ConfigYaml>,
}

#[derive(Debug, Deserialize)]
struct StageYaml {
    id: String,
    name: String,
    description: Option<String>,
    entry_condition: Option<String>,
    exit_condition: Option<String>,
    transitions: Vec<TransitionYaml>,
    actions: Option<Vec<String>>,
}

impl ProcessYaml {
    fn to_process(&self) -> Result<DecisionProcess, ConfigError> {
        let stages = self.process.stages.iter()
            .map(|s| s.to_stage())
            .collect::<Result<Vec<_>, _>>()?;
        
        Ok(DecisionProcess {
            name: self.process.name.clone(),
            description: self.process.description.clone(),
            stages,
            initial_stage: StageId::new(&self.process.initial_stage),
            final_stage: StageId::new(&self.process.final_stage),
            config: self.process.config.map(|c| c.to_config()).unwrap_or_default(),
        })
    }
}
```

### Story 15.4: Custom Condition Registration

**Priority**: P2
**Effort**: 3 points
**Status**: Backlog

Allow registration of custom condition evaluators.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T15.4.1 | Create `ConditionRegistry` | Todo | - |
| T15.4.2 | Implement `register_condition()` | Todo | - |
| T15.4.3 | Evaluate custom conditions | Todo | - |
| T15.4.4 | Handle missing custom conditions | Todo | - |
| T15.4.5 | Write unit tests for custom conditions | Todo | - |

#### TDD Test Tasks

| Test ID | Definition |
|---------|------------|
| T15.4.T1 | Custom condition registered |
| T15.4.T2 | Custom condition evaluated |
| T15.4.T3 | Missing custom condition returns false |

#### Acceptance Criteria

- Custom conditions can be registered
- YAML can reference custom conditions

#### Technical Notes

```rust
pub struct ConditionRegistry {
    evaluators: HashMap<String, fn(&EvaluationContext) -> bool>,
}

impl ConditionRegistry {
    pub fn new() -> Self {
        Self { evaluators: HashMap::new() }
    }
    
    pub fn register(&mut self, name: String, evaluator: fn(&EvaluationContext) -> bool) {
        self.evaluators.insert(name, evaluator);
    }
    
    pub fn evaluate(&self, name: &str, ctx: &EvaluationContext) -> bool {
        self.evaluators.get(name)
            .map(|f| f(ctx))
            .unwrap_or(false)
    }
}

// Usage
let mut registry = ConditionRegistry::new();
registry.register("problem_located", |ctx| {
    ctx.output.as_ref().map(|o| o.contains("located")).unwrap_or(false)
});
```

### Story 15.5: Check Rule Extension

**Priority**: P2
**Effort**: 2 points
**Status**: Backlog

Allow registration of custom check rules.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T15.5.1 | Add `register_rule()` to AutoChecker | Todo | - |
| T15.5.2 | Support configurable rule order | Todo | - |
| T15.5.3 | Write unit tests for rule extension | Todo | - |

#### TDD Test Tasks

| Test ID | Definition |
|---------|------------|
| T15.5.T1 | Custom rule registered |
| T15.5.T2 | Custom rule evaluated in order |

#### Acceptance Criteria

- Users can add custom check rules
- Rules evaluated in configurable order

## Sprint Deliverables

1. `DecisionMetrics` with calculation methods
2. Metrics integration in DecisionEngine
3. YAML process loader
4. `ConditionRegistry` for custom conditions
5. Check rule extension support
6. Unit tests for all features

## Sprint Review Checklist

- [x] All tasks completed
- [x] All tests passing
- [x] Metrics collected correctly
- [x] Custom process loads from YAML
- [x] Custom conditions work
- [x] Code reviewed and merged

## Risks and Mitigation

| Risk | Mitigation |
|------|------------|
| YAML schema complexity | Start simple, expand gradually |
| Custom condition security | Restrict to safe evaluators |
| Metrics accuracy | Comprehensive testing |

## Final Notes

This sprint completes the Task Concept implementation. The decision layer now has:
- Task entity with lifecycle (Sprint 09)
- Structured workflow (Sprint 10)
- Automation intelligence (Sprint 11)
- Persistence (Sprint 12)
- Decision engine (Sprint 13)
- TUI integration (Sprint 14)
- Metrics and customization (Sprint 15)

**Total: 7 sprints, ~10-12 weeks effort**

Target metrics to verify:
- Automation rate > 80%
- Human intervention rate < 20%
- Average reflections: 1-2
- Completion rate > 90%