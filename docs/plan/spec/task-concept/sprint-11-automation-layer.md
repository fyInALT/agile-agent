# Sprint 11: Automation Layer

## Metadata

- Sprint ID: `decision-sprint-011`
- Title: `Automation Layer`
- Duration: 2 weeks
- Priority: P0 (Critical)
- Status: `Backlog`
- Created: 2026-04-18
- Depends on: `decision-sprint-010` (Decision Workflow)

## Reference

See [Task Concept Requirements](../../decision-layer-task-concept-requirements.md) for detailed requirements:
- FR-009: Prompt Template Structure
- FR-010: Default Prompt Templates
- FR-011: Automatic Quality Verification
- FR-012: Intelligent Human-Escalation Filter

## Sprint Goal

Implement the automation layer that enables the decision layer to make routine decisions without human intervention, achieving the target 80% automation rate.

## Context

With Task entities (Sprint 09) and Workflow definitions (Sprint 10), this sprint adds the intelligence to:
1. Generate appropriate prompts for each stage
2. Automatically verify AI output quality
3. Filter decisions to only escalate genuinely human-required issues

This is the core value proposition: automate 80% of routine decisions.

## Stories

### Story 11.1: Prompt Template Structure

**Priority**: P0
**Effort**: 3 points
**Status**: Backlog

Define the prompt template system with variable substitution.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T11.1.1 | Create `PromptTemplate` struct | Todo | - |
| T11.1.2 | Define variable placeholder syntax | Todo | - |
| T11.1.3 | Implement `render()` method | Todo | - |
| T11.1.4 | Handle missing variables gracefully | Todo | - |
| T11.1.5 | Write unit tests for template rendering | Todo | - |

#### TDD Test Tasks

| Test ID | Definition |
|---------|------------|
| T11.1.T1 | Template renders with all variables |
| T11.1.T2 | Missing variable returns error or default |
| T11.1.T3 | Multiple variables replaced correctly |
| T11.1.T4 | Empty template returns empty string |

#### Acceptance Criteria

- PromptTemplate matches FR-009 specification
- {{variable}} syntax works correctly
- render() handles edge cases

#### Technical Notes

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptTemplate {
    pub id: String,
    pub content: String,
    pub variables: Vec<String>,
}

impl PromptTemplate {
    pub fn render(&self, values: &HashMap<String, String>) -> Result<String, RenderError> {
        let mut result = self.content.clone();
        for var in &self.variables {
            let value = values.get(var)
                .ok_or_else(|| RenderError::MissingVariable(var.clone()))?;
            result = result.replace(&format!("{{{{{}}}}}", var), value);
        }
        Ok(result)
    }
}
```

### Story 11.2: Default Prompt Templates

**Priority**: P0
**Effort**: 4 points
**Status**: Backlog

Implement default templates for each stage in the default process.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T11.2.1 | Create template for "start" stage | Todo | - |
| T11.2.2 | Create template for "check_quality" stage | Todo | - |
| T11.2.3 | Create template for "reflecting" stage | Todo | - |
| T11.2.4 | Create template for "check_completion" stage | Todo | - |
| T11.2.5 | Create template for "confirming" stage | Todo | - |
| T11.2.6 | Create template for "human_decision" stage | Todo | - |
| T11.2.7 | Create `default_templates()` registry | Todo | - |
| T11.2.8 | Write unit tests for each template | Todo | - |

#### TDD Test Tasks

| Test ID | Definition |
|---------|------------|
| T11.2.T1 | start template has task_description |
| T11.2.T2 | check_quality template has ai_output, constraints |
| T11.2.T3 | reflecting template has problem, count, max |
| T11.2.T4 | check_completion template has goals, status |
| T11.2.T5 | confirming template has task, changes, tests |
| T11.2.T6 | All templates in registry |

#### Acceptance Criteria

- Templates match FR-010 specification
- All required variables defined
- Templates clear and actionable

#### Technical Notes

```rust
pub fn default_templates() -> HashMap<String, PromptTemplate> {
    let mut templates = HashMap::new();
    
    templates.insert("start", PromptTemplate {
        id: "start".into(),
        content: "Begin implementing task: {{task_description}}".into(),
        variables: vec!["task_description".into()],
    });
    
    templates.insert("check_quality", PromptTemplate {
        id: "check_quality".into(),
        content: "Check AI output:\n1. Syntax errors?\n2. Tests pass?\n3. Within boundaries?\n4. Code quality?\n\nOutput: {{ai_output}}\nConstraints: {{task_constraints}}".into(),
        variables: vec!["ai_output".into(), "task_constraints".into()],
    });
    
    // ... (6 templates total per FR-010)
    
    templates
}
```

### Story 11.3: Auto-Check System

**Priority**: P0
**Effort**: 5 points
**Status**: Backlog

Implement automatic quality verification of AI output.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T11.3.1 | Define `AutoCheckResult` enum | Todo | - |
| T11.3.2 | Define `CheckRule` trait | Todo | - |
| T11.3.3 | Implement syntax check rule | Todo | - |
| T11.3.4 | Implement test check rule | Todo | - |
| T11.3.5 | Implement compilation check rule | Todo | - |
| T11.3.6 | Implement boundary check rule | Todo | - |
| T11.3.7 | Implement risk check rule | Todo | - |
| T11.3.8 | Create `AutoChecker` with rule registry | Todo | - |
| T11.3.9 | Write unit tests for each rule | Todo | - |

#### TDD Test Tasks

| Test ID | Definition |
|---------|------------|
| T11.3.T1 | Syntax check detects errors |
| T11.3.T2 | Test check detects failures |
| T11.3.T3 | Compilation check detects errors |
| T11.3.T4 | Boundary check detects violations |
| T11.3.T5 | Risk check detects high-risk ops |
| T11.3.T6 | AutoChecker combines all rules |
| T11.3.T7 | Pass → Continue |
| T11.3.T8 | NeedsReflection → Reflect |
| T11.3.T9 | NeedsHuman → RequestHuman |

#### Acceptance Criteria

- AutoChecker matches FR-011 specification
- All check rules implemented
- Correct result classification

#### Technical Notes

```rust
#[derive(Debug, Clone)]
pub enum AutoCheckResult {
    Pass,
    NeedsReflection { reason: String },
    NeedsHuman { reason: String },
}

pub trait CheckRule: Send + Sync {
    fn name(&self) -> &str;
    fn check(&self, task: &Task, output: &AgentOutput) -> Option<AutoCheckResult>;
}

pub struct AutoChecker {
    rules: Vec<Box<dyn CheckRule>>,
}

impl AutoChecker {
    pub fn check(&self, task: &Task, output: &AgentOutput) -> AutoCheckResult {
        for rule in &self.rules {
            if let Some(result) = rule.check(task, output) {
                return result;
            }
        }
        AutoCheckResult::Pass
    }
}

// Built-in rules
pub struct SyntaxCheckRule;
pub struct TestCheckRule;
pub struct CompileCheckRule;
pub struct BoundaryCheckRule;
pub struct RiskCheckRule;
```

### Story 11.4: Human Escalation Filter

**Priority**: P0
**Effort**: 4 points
**Status**: Backlog

Implement intelligent filtering for human-required decisions.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T11.4.1 | Define escalation criteria | Todo | - |
| T11.4.2 | Create `DecisionFilter` struct | Todo | - |
| T11.4.3 | Implement `needs_human_decision()` | Todo | - |
| T11.4.4 | Implement `auto_decide()` | Todo | - |
| T11.4.5 | Configure filter thresholds | Todo | - |
| T11.4.6 | Write unit tests for filtering | Todo | - |

#### TDD Test Tasks

| Test ID | Definition |
|---------|------------|
| T11.4.T1 | Boundary violation → NeedsHuman |
| T11.4.T2 | High-risk operation → NeedsHuman |
| T11.4.T3 | Design decision → NeedsHuman |
| T11.4.T4 | Dependency conflict → NeedsHuman |
| T11.4.T5 | Syntax error → Auto Reflect |
| T11.4.T6 | Test failure → Auto Reflect |
| T11.4.T7 | Normal output → Auto Continue |

#### Acceptance Criteria

- DecisionFilter matches FR-012 specification
- Correct classification of issues
- Configurable filter rules

#### Technical Notes

```rust
pub struct DecisionFilter {
    risky_operations: Vec<String>,
    boundary_rules: Vec<BoundaryRule>,
}

impl DecisionFilter {
    pub fn needs_human_decision(&self, task: &Task, output: &AgentOutput) -> Option<String> {
        // Check boundary violation
        for rule in &self.boundary_rules {
            if rule.is_violated(output) {
                return Some(format!("Boundary violation: {}", rule.description()));
            }
        }
        
        // Check risky operations
        for op in &self.risky_operations {
            if output.contains_operation(op) {
                return Some(format!("High-risk: {}", op));
            }
        }
        
        // Check design decision needed
        if output.has_multiple_valid_solutions() {
            return Some("Design decision required");
        }
        
        // Check dependency conflict
        if output.has_dependency_conflict() {
            return Some("Dependency conflict");
        }
        
        None
    }
    
    pub fn auto_decide(&self, task: &Task, output: &AgentOutput) -> DecisionAction {
        // Test failure → Reflect
        if !output.tests_pass() {
            return DecisionAction::Reflect { reason: "Tests failed".into() };
        }
        
        // Syntax error → Reflect
        if output.has_syntax_errors() {
            return DecisionAction::Reflect { reason: "Syntax errors".into() };
        }
        
        // Style issue → Reflect (if configured)
        if output.has_style_issues() {
            return DecisionAction::Reflect { reason: "Style issues".into() };
        }
        
        // Complete verified → Confirm
        if task.status == TaskStatus::PendingConfirmation {
            return DecisionAction::ConfirmCompletion;
        }
        
        // Default → Continue
        DecisionAction::Continue
    }
}
```

### Story 11.5: Integration Tests for Automation

**Priority**: P1
**Effort**: 3 points
**Status**: Backlog

Write integration tests combining templates, auto-check, and filter.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T11.5.1 | Test full automation flow (pass case) | Todo | - |
| T11.5.2 | Test reflection flow (issue found) | Todo | - |
| T11.5.3 | Test human escalation flow | Todo | - |
| T11.5.4 | Test template + decision integration | Todo | - |

#### TDD Test Tasks

| Test ID | Definition |
|---------|------------|
| T11.5.T1 | Clean output → Pass → Continue |
| T11.5.T2 | Syntax error → NeedsReflection → Reflect |
| T11.5.T3 | Boundary violation → NeedsHuman → RequestHuman |
| T11.5.T4 | Template matches decision context |

#### Acceptance Criteria

- Integration tests cover all automation paths
- Tests verify correct decision routing

## Sprint Deliverables

1. `PromptTemplate` with variable substitution
2. 6 default templates for key stages
3. `AutoCheckResult` and `CheckRule` trait
4. 5 built-in check rules
5. `AutoChecker` combining all rules
6. `DecisionFilter` for human escalation
7. Integration tests for automation flow

## Sprint Review Checklist

- [ ] All tasks completed
- [ ] All tests passing
- [ ] Templates render correctly
- [ ] Auto-check detects all issues
- [ ] Filter correctly routes decisions
- [ ] Code reviewed and merged

## Risks and Mitigation

| Risk | Mitigation |
|------|------------|
| Check rules too strict | Configurable thresholds, start conservative |
| False positives in filter | Refine rules based on metrics |
| Missing check rules | Extensible CheckRule trait, add later |

## Next Sprint Preview

Sprint 12 will add persistence:
- TaskRegistry for storage and retrieval
- ExecutionHistory for action logging
- Recovery mechanism for crash handling