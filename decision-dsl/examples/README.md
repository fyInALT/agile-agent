# Decision Flow Examples

This directory contains behavior tree templates for common decision workflows.

## Templates

### sprint-completion-flow.yaml

Multi-sprint verification flow that ensures all sprints are genuinely completed before marking a task done.

**Use case**: When Work Agent completes 2 of 4 sprints and stops early, this flow catches it.

**Flow**:
1. Initialize sprint counter
2. Loop through sprints with AI execution and reflection
3. Multi-angle verification after all sprints
4. Final confirmation or escalation

### error-recovery-flow.yaml

Error handling flow with AI-driven strategy adjustment.

**Use case**: When a tool call fails or approach doesn't work, this flow analyzes and retries.

**Flow**:
1. Analyze failure cause
2. Propose alternative strategy
3. Retry with adjusted approach (max 3 retries)
4. Escalate if all retries exhausted

### task-verification-flow.yaml

Task completion verification with multi-angle AI review.

**Use case**: Before marking a task complete, verify from requirements, implementation, and testing perspectives.

**Flow**:
1. Check if Work Agent reports completion
2. Verify requirements are met
3. Verify implementation quality
4. Verify testing coverage
5. Final decision: confirm, request review, or continue work

## Usage

These templates can be loaded into a `DecisionAgentSlot`:

```rust
use std::path::PathBuf;
use decision_dsl::ast::document::Tree;
use decision_dsl::parser::parse_tree_from_file;
use agent_daemon::decision_agent_slot::{DecisionAgentSlot, DecisionSlotConfig};

// Load template
let yaml_path = PathBuf::from("decision-dsl/examples/sprint-completion-flow.yaml");
let tree = parse_tree_from_file(&yaml_path)?;

// Configure slot
let config = DecisionSlotConfig {
    provider_kind: "claude".to_string(),
    cwd: PathBuf::from("/project"),
    max_reflection_rounds: 2,
    total_sprints: 4,
    sprint_goals: vec![
        SprintGoal::new(1, "Implement core feature"),
        SprintGoal::new(2, "Add tests"),
        SprintGoal::new(3, "Handle edge cases"),
        SprintGoal::new(4, "Documentation"),
    ],
};

// Create slot
let mut slot = DecisionAgentSlot::new(tree, config)?;
slot.set_work_agent_id("agent-work-1");
slot.set_task_description("Implement authentication");

// Execute decision cycle
let result = slot.tick()?;
```

## Template Format

Templates use the decision-dsl YAML format:

```yaml
apiVersion: decision.agile-agent.io/v1
kind: BehaviorTree
metadata:
  name: flow-name
  description: Flow description
spec:
  root:
    kind: Sequence  # or Selector, Repeater, Prompt, etc.
    payload:
      name: step-name
      children: [...]  # nested nodes
```

See `docs/research/behavior-tree-driven-ai-decision.md` for full design documentation.