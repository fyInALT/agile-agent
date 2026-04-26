# Sprint 2: AST & Desugaring

## Metadata

- Sprint ID: `dsl-sprint-02`
- Title: `AST & Desugaring`
- Duration: 2 weeks
- Priority: P0 (Critical)
- Status: `Done`
- Created: 2026-04-20

## Sprint Goal

Build the complete AST data model, YAML parser, and desugaring pass. DecisionRules YAML compiles to BehaviorTree AST; BehaviorTree and SubTree YAML parse directly. All documents are validated at load time.

## Dependencies

- **Sprint 1** (`dsl-sprint-01`): Blackboard, Command enum, Error types, External traits.

## Non-goals

- No evaluator or parser implementation (Sprint 3).
- No template engine (Sprint 3).
- No execution engine (Sprint 4).
- No hot reload (Sprint 5).

---

## Stories

### Story 2.1: AST Data Model

**Priority**: P0
**Effort**: 5 points
**Status**: Done

Implement the complete AST type hierarchy using `enum_dispatch` for zero-cost node behavior abstraction.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T2.1.1 | Define `Tree`, `Metadata`, `Spec`, `Bundle` structs | Done | - |
| T2.1.2 | Define `TreeKind` enum (`BehaviorTree`, `SubTree`) | Done | - |
| T2.1.3 | Define `NodeBehavior` trait; `Node` enum annotated with `enum_dispatch` (manual impl used due to serde conflict) | Done | - |
| T2.1.4 | Define `Node` enum with 14 variants | Done | - |
| T2.1.5 | Define all node-specific structs with serde attributes | Done | - |
| T2.1.6 | Mark runtime-state fields with `#[serde(skip)]` | Done | - |
| T2.1.7 | Add `#[serde(rename = "...")]` for camelCase YAML fields | Done | - |
| T2.1.8 | Define `SetMapping` struct | Done | - |
| T2.1.9 | Define `ParallelPolicy` enum | Done | - |
| T2.1.10 | Define `NodeStatus` enum (`Success`, `Failure`, `Running`) | Done | - |
| T2.1.11 | Write unit tests for Node serialization round-trip | Done | - |

#### Acceptance Criteria

- `Node` enum derives `Clone`, `Debug`, and serde traits.
- `enum_dispatch` annotation present on `Node` enum; due to 0.3.x incompatibility with serde derive, a manual `impl NodeBehavior for Node` provides equivalent behavior.
- Runtime fields (`active_child`, `pending`, `sent_at`, `last_success`, `current`, `resolved_root`) are skipped during serde.

#### Technical Notes

```rust
// enum_dispatch 0.3.x + serde derive cannot coexist on same enum.
// Manual impl provides identical behavior to enum_dispatch-generated code.

#[enum_dispatch(NodeBehavior)]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", content = "payload")]
pub(crate) enum Node {
    Selector(SelectorNode),
    Sequence(SequenceNode),
    Parallel(ParallelNode),
    Inverter(InverterNode),
    Repeater(RepeaterNode),
    Cooldown(CooldownNode),
    ReflectionGuard(ReflectionGuardNode),
    ForceHuman(ForceHumanNode),
    When(WhenNode),
    Condition(ConditionNode),
    Action(ActionNode),
    Prompt(PromptNode),
    SetVar(SetVarNode),
    SubTree(SubTreeNode),
}

// Manual impl (same behavior as enum_dispatch would generate):
impl NodeBehavior for Node {
    fn reset(&mut self) { /* delegates to each variant */ }
    fn name(&self) -> &str { /* delegates to each variant */ }
    fn children(&self) -> Vec<&Node> { /* delegates to each variant */ }
    fn children_mut(&mut self) -> Vec<&mut Node> { /* delegates to each variant */ }
}
```

---

### Story 2.2: YAML Parser & DslDocument

**Priority**: P0
**Effort**: 5 points
**Status**: Done

Implement the YAML parser that produces `DslDocument` (a parse-time representation mirroring raw YAML).

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T2.2.1 | Define `DslParser` trait (`parse_document`, `parse_bundle`) | Done | - |
| T2.2.2 | Define `DslDocument` enum (`DecisionRules`, `BehaviorTree`, `SubTree`) | Done | - |
| T2.2.3 | Define `RuleSpec` struct with serde renames | Done | - |
| T2.2.4 | Define `ThenSpec` enum (`InlineCommand`, `Switch`, `When`, `Pipeline`, `SubTree`) | Done | - |
| T2.2.5 | Define `SwitchSpec`, `SwitchOn`, `WhenSpec`, `PipelineSpec` | Done | - |
| T2.2.6 | Define `OnError` enum (`Skip`, `Escalate`, `Retry`) | Done | - |
| T2.2.7 | Implement `YamlParser` struct with evaluator/parser registries | Done | - |
| T2.2.8 | Implement `parse_document` using `serde_yaml` | Done | - |
| T2.2.9 | Implement `parse_bundle` using `Fs` trait | Done | - |
| T2.2.10 | Write unit tests for DecisionRules parsing | Done | - |
| T2.2.11 | Write unit tests for BehaviorTree parsing | Done | - |
| T2.2.12 | Write unit tests for SubTree parsing | Done | - |

#### Acceptance Criteria

- `YamlParser::parse_document` handles all three `kind` values.
- `serde(rename = "...")` correctly maps camelCase YAML to snake_case Rust.
- `parse_bundle` recursively reads `rules.d/`, `trees/`, `subtrees/` directories.

#### Technical Notes

```rust
pub trait DslParser {
    fn parse_document(&self, yaml: &str) -> Result<DslDocument, ParseError>;
    fn parse_bundle(&self, dir: &Path, fs: &dyn Fs) -> Result<Bundle, ParseError>;
}

pub struct YamlParser {
    pub evaluator_registry: EvaluatorRegistry,
    pub parser_registry: OutputParserRegistry,
}
```

---

### Story 2.3: Desugaring Pass

**Priority**: P0
**Effort**: 5 points
**Status**: Done

Implement the desugaring pass that compiles high-level constructs to low-level BehaviorTree AST nodes.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T2.3.1 | Implement `DslDocument::desugar()` entry point | Done | - |
| T2.3.2 | Implement `desugar_rule` (Sequence + Condition + decorators + on_error) | Done | - |
| T2.3.3 | Implement `desugar_then` (InlineCommand, Switch, When, Pipeline, SubTree) | Done | - |
| T2.3.4 | Implement `desugar_switch` for `SwitchOn::Prompt` (Sequence + Prompt + Selector) | Done | - |
| T2.3.5 | Implement `desugar_switch` for `SwitchOn::Variable` (Selector + When) | Done | - |
| T2.3.6 | Handle `result_key` configuration in Switch prompt desugaring | Done | - |
| T2.3.7 | Handle `default` case in Switch (any ThenSpec, not just command) | Done | - |
| T2.3.8 | Implement `desugar_when` (without on_error) | Done | - |
| T2.3.8a | Handle `WhenSpec.on_error` wrapping: `skip` → no-op, `escalate` → Selector with EscalateToHuman fallback, `retry` → Repeater(2) | Done | - |
| T2.3.9 | Implement `desugar_pipeline` | Done | - |
| T2.3.10 | Add automatic `NoMatchFallback` (`ApproveAndContinue`) to DecisionRules | Done | - |
| T2.3.11 | Write unit tests for DecisionRules → Selector desugaring | Done | - |
| T2.3.12 | Write unit tests for Switch prompt desugaring | Done | - |
| T2.3.13 | Write unit tests for on_error wrapping (Escalate, Retry, Skip) on Rule | Done | - |
| T2.3.13a | Write unit tests for `When.on_error` wrapping (Escalate, Retry) | Done | - |
| T2.3.14 | Write unit tests for Switch `result_key` configuration (cases match against specified key, default `decision`) | Done | - |
| T2.3.15 | Write unit tests for `_default` case supporting nested Switch/When/Pipeline in ThenSpec | Done | - |

#### Acceptance Criteria

- `DecisionRules` desugars to `Selector(children..., NoMatchFallback)`.
- Decorator wrapping order: `Cooldown` (outer) → `ReflectionGuard` → `on_error` (inner).
- `Switch on Prompt` produces `Sequence(Prompt, Selector(When...))`.
- `_default` supports nested Switch / When / Pipeline.
- `When { if, then, on_error }` applies the same error-handling wrapping as `Rule.on_error`:
  - `skip` → no extra wrapping (When already returns Failure on false condition)
  - `escalate` → `Selector(When(...), Action(EscalateToHuman))`
  - `retry` → `Repeater(2, When(...))`
- `result_key` in Switch on Prompt correctly stores parser result in specified blackboard key; cases match against this key.

#### Technical Notes

Desugaring table:

| High-Level Construct | Desugars To |
|---------------------|-------------|
| `DecisionRules { rules }` | `Selector(rule[1], ..., rule[n], NoMatchFallback)` |
| `Rule { if, then }` | `Sequence(Condition(if), then.desugar())` |
| `Rule { ..., cooldownMs }` | `Cooldown(...)` |
| `Rule { ..., reflectionMaxRounds }` | `ReflectionGuard(...)` |
| `Rule { ..., on_error: escalate }` | `Selector(Rule.desugar(), Action(EscalateToHuman))` |
| `Rule { ..., on_error: retry }` | `Repeater(2, Rule.desugar())` |
| `Switch on Prompt` | `Sequence(Prompt, Selector(When..., Default))` |
| `Switch on Variable` | `Selector(When..., Default)` |
| `When { if, then }` | `WhenNode(condition, then.desugar())` |
| `When { if, then, on_error: escalate }` | `Selector(WhenNode(...), Action(EscalateToHuman))` |
| `When { if, then, on_error: retry }` | `Repeater(2, WhenNode(...))` |
| `Pipeline { steps }` | `Sequence(Condition/Action...)` |

---

### Story 2.4: Validation

**Priority**: P1
**Effort**: 3 points
**Status**: Done

Implement load-time validation to reject invalid DSL before execution.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T2.4.1 | Implement `validate_api_version` (format: `decision.agile-agent.io/v{N}`) | Done | - |
| T2.4.2 | Implement `validate_unique_names` within a tree | Done | - |
| T2.4.3 | Implement `validate_unique_priorities` for DecisionRules | Done | - |
| T2.4.4 | Implement `validate_evaluators` (all `kind` values registered) | Done | - |
| T2.4.5 | Implement `validate_parsers` (all `kind` values registered) | Done | - |
| T2.4.6 | Implement `validate_subtree_refs` (all refs resolve in Bundle) | Done | - |
| T2.4.7 | Implement `detect_circular_subtree_refs` | Done | - |
| T2.4.8 | Integrate validation into `parse_bundle` pipeline | Done | - |
| T2.4.9 | Write unit tests for each validation rule | Done | - |

#### Acceptance Criteria

- Duplicate node names produce `ParseError::DuplicateName`.
- Circular SubTree references produce `ParseError::CircularSubTreeRef`.
- Unknown evaluator kind produces `ParseError::UnknownEvaluatorKind`.
- Validation runs before desugaring in `parse_bundle`.

#### Technical Notes

Validation order:
1. YAML syntax check
2. apiVersion check
3. Name uniqueness check
4. Priority uniqueness check (DecisionRules)
5. Evaluator kind registration check
6. Parser kind registration check
7. SubTree reference resolution check
8. Circular reference detection

---

## Sprint Completion Criteria

- [x] `cargo check` passes for the `decision-dsl` crate.
- [x] `cargo test --lib` passes with ≥90% coverage on parser and AST modules.
- [x] All desugaring paths have unit tests with structural assertions.
- [x] Invalid DSL documents are rejected with clear `ParseError` messages.
