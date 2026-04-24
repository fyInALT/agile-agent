# Decision DSL Reflection: Toward a More Elegant Design

> An architectural critique of the **pre-`55cd8a7` behavior-tree-based DSL**, drawing on patterns from Temporal, AWS Step Functions, Celery, XState, Zig, and Kubernetes controllers.
>
> **Audience**: authors of `decision-dsl.md` and `decision-dsl-implementation.md`.
> **Status**: Historical — most proposals herein have been adopted in the `implementation/` documents.
>
> **Note**: The `implementation/` documents (`decision-dsl-ast.md`, `decision-dsl-runtime.md`, etc.) already incorporate the redesign proposed in this document: enum-based `Evaluator`/`OutputParser`, `enum_dispatch` for `Node`, scoped Blackboard, grouped `DecisionCommand`, `minijinja` templates, and high-level `DecisionRules` syntax. This document is retained as design rationale.

---

## 1. The Core Mismatch: Behavior Trees for a Rules Engine

The current DSL inherits from **game-AI behavior trees** (Unreal Engine, BehaviorTree.CPP). That model assumes:

- **High-frequency ticks** (every frame, 60 Hz)
- **Deeply nested compositions** (hundreds of small behaviors)
- **Priority-based fallback** (Selector short-circuits: try this, else this, else this)
- **Continuous state** (agents navigate a world)

An AI-agent decision layer has **none** of these properties:

| Property | Game AI BT | AI Agent Decision Layer |
|----------|-----------|------------------------|
| Tick frequency | 60 Hz | ~0.05 Hz (once per agent output) |
| Tree depth | 10–50 levels | 2–4 levels |
| Primary operation | `if (see_enemy) fight()` | `ask LLM → branch → emit command` |
| Branching factor | Dozens of children | 2–4 branches |
| State management | Blackboard (100s of keys) | ~10 well-known variables |
| Failure semantics | Fall through to next child | Escalate or retry |

The current DSL forces behavior-tree structure onto a problem that is better modeled as **workflow orchestration** (Temporal, AWS Step Functions) or a **rules engine** (Drools).

### Concrete evidence

The "Prompt + Branch" pattern — which accounts for ~80% of real decisions — requires **40 lines of YAML** and **6 nodes** (Sequence, Prompt, Selector, 2× Sequence, 2× Condition, 2× Action) to express what is logically:

> "Ask the LLM for a choice; if REFLECT, emit Reflect; if CONFIRM, emit ConfirmCompletion."

This is what Alan Kay meant by "simple things should be simple." The BT model fails this test.

---

## 2. What the Current Design Gets Right

Before critiquing, credit where it's due:

1. **Trait-based dependency injection** (Session, Clock, Fs, Logger) — clean, testable, Lua-inspired. This is excellent.
2. **YAML as the authoring format** — correct choice for a declarative language that non-Rust-developers will write.
3. **Hot reload with atomic swap** — in-flight decisions use old tree; new decisions use new tree. Correct semantics.
4. **apiVersion for migration** — forward-compatible. Critical for an evolving language.
5. **Same-session LLM integration** — sending prompts into the existing agent session (not making new API calls) is the right architecture.
6. **Registry pattern for extensibility** — NodeRegistry, EvaluatorRegistry, OutputParserRegistry, FilterRegistry. Consistent, clean.
7. **Fail-fast at load time** — validation before execution. Correct.
8. **Prompt node's Running status** — cleanly models async LLM within a synchronous tick loop.

These are solid foundations. The redesign should preserve them.

---

## 3. Problems, Ranked by Severity

### P0 — YAML Verbosity: The "Branch Boilerplate" Problem

Every LLM-driven branch requires this pattern:

```yaml
# 6 nodes, 35 lines. That's ~30 lines of ceremony for 2 lines of logic.
- kind: Sequence
  name: handle_decision
  children:
    - kind: Prompt
      name: ask
      template: "REFLECT or CONFIRM?"
      parser: { kind: enum, values: [REFLECT, CONFIRM] }
      sets:
        - key: next_action
          field: decision
    - kind: Selector
      name: branch
      children:
        - kind: Sequence
          name: do_reflect
          children:
            - kind: Condition
              name: is_reflect
              eval: { kind: variableIs, key: next_action, value: REFLECT }
            - kind: Action
              name: emit_reflect
              command: { Reflect: { prompt: "..." } }
        - kind: Sequence
          name: do_confirm
          children:
            - kind: Condition
              name: is_confirm
              eval: { kind: variableIs, key: next_action, value: CONFIRM }
            - kind: Action
              name: emit_confirm
              command: ConfirmCompletion
```

**Root cause**: The DSL forces the user to manually wire up: parse LLM output → store each field as a blackboard variable → create a condition for each branch value → wrap each condition+action in a sequence → wrap all branches in a selector. This is five layers of indirection.

**How others solve this**:

- **AWS Step Functions Choice state**: `"Choices": [{"Variable": "$.decision", "StringEquals": "REFLECT", "Next": "ReflectState"}, ...]` — **6 lines** for a 2-branch decision.
- **Celery canvas**: `chain(prompt.s("..."), switch(on_result={"REFLECT": reflect_action, "CONFIRM": confirm_action}))` — **4 lines**.
- **Temporal workflow**: `switch (await askLLM("REFLECT or CONFIRM?")) { case REFLECT → await reflect(); case CONFIRM → await confirm(); }` — code, **8 lines**.

### P1 — The Blackboard is a Global `HashMap<String, Value>`

```rust
pub struct Blackboard {
    pub variables: HashMap<String, BlackboardValue>,
    // ...
}
```

Every variable read/write is a string-keyed HashMap lookup. This means:

- **No type safety**: `bb.set("retry_count", BlackboardValue::Integer(3))` and `bb.get_string("retry_count")` is a runtime mismatch.
- **No scoping**: All variables are global. SubTrees can accidentally clobber each other.
- **No IDE support**: No autocomplete on variable names.
- **No documentation embedded in the code**: What variables does a SubTree expect? What does it produce? You have to read the YAML.

**How others solve this**:

- **Zig's comptime**: Struct fields are known at compile time. The equivalent would be a typed `DecisionContext` struct with named fields.
- **Temporal's typed signals/queries**: Each workflow declares its signal types. The SDK generates type-safe wrappers.
- **XState's context + assign**: `context: { retryCount: 0 }` — still dynamic but at least declared in one place with defaults.

### P2 — The `Node` Enum is a Manual Visitor (Expression Problem)

The `Node` enum has 13 variants. Every traversal (tick, reset, resolve_subtrees, detect_cycles, validate_unique_names, validate_evaluators, validate_parsers, validate_subtree_refs, resume_at) requires matching all 13 variants — **9 × 13 = 117 match arms** just to walk a tree.

Every new node type requires adding one variant and updating all 9 functions. This is the Expression Problem: adding a new *operation* is easy (one function), adding a new *node type* is expensive (N functions).

```rust
// 9 functions × 13 variants = boilerplate explosion
impl Node {
    pub fn tick(&mut self, ...) { match self { Node::Selector(n) => ..., Node::Sequence(n) => ..., /* ... 11 more */ } }
    pub fn reset(&mut self) { match self { /* ... 13 arms */ } }
    pub fn resolve_subtrees(&mut self, ...) { match self { /* ... 13 arms */ } }
    pub fn detect_cycles(&self, ...) { match self { /* ... 13 arms */ } }
    pub fn validate_unique_names_recursive(&self, ...) { match self { /* ... 13 arms */ } }
    pub fn validate_evaluators_recursive(&self, ...) { match self { /* ... 13 arms */ } }
    pub fn validate_parsers_recursive(&self, ...) { match self { /* ... 13 arms */ } }
    pub fn resume_at(&mut self, ...) { match self { /* ... 13 arms */ } }
    pub fn name(&self) -> &str { match self { /* ... 13 arms */ } }
}
```

**How others solve this**:

- **Rust's `enum_dispatch` crate**: Auto-generates the match arms from a trait. `#[enum_dispatch] trait NodeBehavior { fn tick(...); fn reset(...); ... }` — one trait, N impls, 0 manual matches.
- **TypeScript's discriminated unions + visitor**: `node.accept(visitor)` — the visitor pattern. Still O(N×M) but at least centralized.
- **Kubernetes' unstructured + scheme**: `Unstructured` for unknown kinds; typed wrappers for known kinds. A hybrid.

### P3 — The Prompt Node Violates Single Responsibility

The Prompt node currently does **five** things:

1. Render a Jinja2 template from Blackboard variables
2. Send the rendered text to an LLM session
3. Receive and parse the LLM's reply (enum/structured/json/command)
4. Store parsed fields as Blackboard variables
5. Optionally emit a DecisionCommand directly (CommandParser)

Each of these is a separate concern. The CommandParser hack (`__command` magic key) is a particularly strong signal that the abstraction is wrong — a parser that doesn't parse but instead directly emits commands.

### P4 — Template Engine is a Stub

The spec declares Jinja2 compatibility but the implementation is a 300-line toy:

- `eval_condition` supports `variable > 0` but not `a > 0 && b < 5`
- `eval_expr` does path lookup via `bb.get_path()` — no arithmetic, no function calls
- No proper whitespace control (`{%- -%}` is declared but not implemented)
- No escaping (`{% raw %}`, `| safe`)
- No macro support

Building a real Jinja2-compatible engine from scratch is **months of work**. Tera (the closest Rust equivalent) is ~15K LOC.

**Recommendation**: Depend on `tera` (or `minijinja`), or narrow the spec to "Jinja2-subset" and define exactly what's supported.

### P5 — The Command Enum Has Too Many Variants

`DecisionCommand` has 22 variants, and the DSL's `Command` enum has 21 variants — with slight differences. Many are isomorphic:

```
CommitChanges { message, is_wip, worktree_path }
StashChanges { description, include_untracked, worktree_path }
DiscardChanges { worktree_path }
```

These three are all "git state mutations." They could be:

```rust
enum GitCommand {
    Commit { message: String, wip: bool },
    Stash { description: String, include_untracked: bool },
    Discard,
}

enum Command {
    // ...
    Git(GitCommand, worktree_path: Option<String>),
}
```

Grouping reduces the flat namespace from 22 to ~10 meaningful categories.

### P6 — No First-Class Error Boundary

In the current design, if any node fails:

- **Selector**: falls through to the next child (by design)
- **Sequence**: immediate failure, entire sequence aborts
- **Parallel**: depends on policy

This is behavior-tree semantics, but it means error handling is conflated with normal fallback. There's no `try/catch` equivalent, no standard `on_error` handler, and no way to attach cleanup actions to a subtree.

**How Temporal solves this**: `Workflow.continueAsNew` for retry; `Activity.retryPolicy` for automatic backoff; `try/catch` blocks in workflow code.

### P7 — SubTree Inlining Destroys Identity

```rust
Node::SubTreeRef(n) => {
    *self = subtree.spec.root.clone();  // Inline at parse time
}
```

After this, the subtree node is gone. Traces show the inlined nodes as if they were always part of the parent tree. You can't tell from a trace that a subtree was used, and you can't get aggregate metrics per subtree ("rate_limit_handler took 5s across all invocations").

---

## 4. What Excellent Projects Teach Us

### Temporal / Cadence (Uber)
**Lesson**: Workflows are code, not config. Deterministic execution with replay. Activities are side effects.

**Applicable insight**: The DSL doesn't need to model "execute arbitrary logic" — that's what Rust code is for. The DSL should model **decision routing** only: "given state S, which command should we emit?" The evaluation logic lives in Rust `Evaluator` implementations.

### AWS Step Functions ASL
**Lesson**: The Amazon States Language has a clean separation between **flow control** (Choice, Parallel, Map, Wait) and **task execution** (Task states invoke Lambda/ECS/etc.). The `Choice` state with `Choices` array is particularly elegant:

```json
"ValidateInput": {
  "Type": "Choice",
  "Choices": [
    { "Variable": "$.age", "NumericGreaterThanEquals": 18, "Next": "AdultPath" },
    { "Variable": "$.age", "NumericLessThan": 18, "Next": "MinorPath" }
  ],
  "Default": "ErrorPath"
}
```

**Applicable insight**: Replace the Selector+Sequence+Condition+Action pattern with a single `Switch` node.

### Celery Canvas (Python)
**Lesson**: Small composable primitives: `chain(a, b, c)` (sequential), `group(a, b, c)` (parallel), `chord(group, callback)` (parallel then join), `switch(task, cases)` (branch). Each primitive is 1–2 lines.

**Applicable insight**: The DSL needs a `chain` shorthand and a `switch` shorthand.

### XState (Stately)
**Lesson**: State machines with `on: { EVENT: { target, cond, actions } }`. Events trigger transitions; guards gate them; actions are side effects. Very concise:

```js
states: {
  idle: { on: { START: { target: 'running', actions: 'logStart' } } },
  running: { on: { COMPLETE: 'done', ERROR: 'error' } }
}
```

**Applicable insight**: Define decisions as transitions: "when claims_completion detected → ask LLM → if REFLECT → emit Reflect; if CONFIRM → emit ConfirmCompletion."

### Kubernetes Controllers (Google)
**Lesson**: The reconcile loop: `observe current state → diff against desired state → act to close the gap`. No tree structure needed — just a single loop with pluggable handlers.

**Applicable insight**: The decision layer's core loop is already a reconcile loop: "observe agent output → determine response → emit command." The DSL should make writing handlers for this loop easy, not force a tree structure onto it.

---

## 5. Proposed Redesign: Decision Rules + Shorthands

### 5.1 Keep the Low-Level BT, Add High-Level Shorthands

The BT model stays as the **compilation target** — the AST that the executor actually runs. But users author in a **higher-level syntax** that desugars to BT nodes.

```
┌─────────────────────┐
│  High-Level YAML    │  ← User writes this (concise, domain-specific)
│  (Decision Rules)   │
└────────┬────────────┘
         │ desugar (at parse time)
         ▼
┌─────────────────────┐
│  Low-Level BT AST   │  ← Executor runs this (unchanged from current design)
│  (Selector, etc.)   │
└────────┬────────────┘
         │ tick
         ▼
┌─────────────────────┐
│  Vec<DecisionCmd>   │
└─────────────────────┘
```

This is analogous to how:
- **React JSX** desugars to `React.createElement()` — JSX is sugar, the runtime is unchanged.
- **Celery canvas signatures** desugar to message chains — the `|` operator is sugar.
- **AWS Step Functions Workflow Studio** generates ASL JSON — the visual editor is a skin.

### 5.2 The `Switch` Node: One-to-Many Branching

Replace the 6-node "Prompt + Branch" pattern with a single `Switch`:

```yaml
# BEFORE: 40 lines, 6 nodes
kind: Sequence
name: handle_decision
children:
  - kind: Prompt
    name: ask
    template: "REFLECT or CONFIRM?"
    parser: { kind: enum, values: [REFLECT, CONFIRM] }
    sets: [{ key: next_action, field: decision }]
  - kind: Selector
    name: branch
    children:
      - kind: Sequence
        name: do_reflect
        children:
          - kind: Condition
            name: is_reflect
            eval: { kind: variableIs, key: next_action, value: REFLECT }
          - kind: Action
            name: emit_reflect
            command: { Reflect: { prompt: "Review your work" } }
      - kind: Sequence
        name: do_confirm
        children:
          - kind: Condition
            name: is_confirm
            eval: { kind: variableIs, key: next_action, value: CONFIRM }
          - kind: Action
            name: emit_confirm
            command: ConfirmCompletion

# AFTER: 12 lines, 1 node
kind: Switch
name: completion_decision
on:
  kind: Prompt
  template: "REFLECT or CONFIRM?"
  parser: { kind: enum, values: [REFLECT, CONFIRM] }
cases:
  REFLECT:
    command: { Reflect: { prompt: "Review your work" } }
  CONFIRM:
    command: ConfirmCompletion
  _default:
    command: ApproveAndContinue
```

**Desugaring**: The parser compiles `Switch` into `Sequence(Prompt, Selector(Sequence(Condition, Action)...))` — exactly the BT nodes the executor already understands.

`Switch` can also branch on a Blackboard variable (no LLM call):

```yaml
kind: Switch
name: route_by_strategy
on:
  kind: variable
  key: error_strategy
cases:
  RETRY:
    command: { RetryTool: { tool_name: "{{ last_tool_call.name }}", max_attempts: 3 } }
  FIX:
    command: { SendCustomInstruction: { prompt: "Fix the {{ error_type }}", target_agent: "{{ agent_id }}" } }
  ESCALATE:
    command: { EscalateToHuman: { reason: "Error recovery chose escalation" } }
```

### 5.3 The `When` Rule: The Simplest Case

For decisions that are purely condition-based (no LLM), introduce `When` as a top-level rule:

```yaml
# BEFORE: 8 lines, 2 nodes
kind: Sequence
name: handle_rate_limit
children:
  - kind: Condition
    name: is_rate_limit
    eval: { kind: outputContains, pattern: "429" }
  - kind: Action
    name: retry
    command: { RetryTool: { tool_name: "...", max_attempts: 3 } }

# AFTER: 5 lines, 1 node
kind: When
name: handle_rate_limit
if:
  kind: outputContains
  pattern: "429"
then:
  command: { RetryTool: { tool_name: "{{ last_tool_call.name }}", max_attempts: 3 } }
```

`When` desugars to `Sequence(Condition, Action)`.

### 5.4 The `Pipeline` Node: Sequential Chains

For multi-step sequences, the `children` array is fine. But for 3+ steps, a chain shorthand is clearer:

```yaml
# BEFORE
kind: Sequence
name: safe_commit
children:
  - kind: Condition
    name: tests_pass
    eval: { kind: outputContains, pattern: "all tests passing" }
  - kind: Condition
    name: no_dangerous_changes
    eval: { kind: script, script: "blackboard.file_changes.len() < 10" }
  - kind: Action
    name: commit
    command: { SuggestCommit: { message: "Safe checkpoint", mandatory: false } }

# AFTER
kind: Pipeline
name: safe_commit
steps:
  - if: { kind: outputContains, pattern: "all tests passing" }
  - if: { kind: script, script: "blackboard.file_changes.len() < 10" }
  - then: { command: { SuggestCommit: { message: "Safe checkpoint", mandatory: false } } }
```

### 5.5 Decision File as a Priority-Ordered Rule List

The root tree's Selector pattern (try first handler, else second handler, ...) is conceptually a **priority-ordered rule list**. Make this explicit:

```yaml
apiVersion: decision.agile-agent.io/v1
kind: DecisionRules
metadata:
  name: default_decisions
spec:
  rules:
    - priority: 1
      name: rate_limit
      if: { kind: regex, pattern: "(429|rate.?limit)" }
      then: { command: { RetryTool: { tool_name: "{{ last_tool_call.name }}", max_attempts: 3 } } }
      cooldownMs: 5000

    - priority: 2
      name: dangerous_action
      if: { kind: script, script: "is_dangerous(blackboard.provider_output)" }
      then:
        command: { EscalateToHuman: { reason: "Dangerous action detected" } }

    - priority: 3
      name: claims_completion
      if: { kind: outputContains, pattern: "claims_completion" }
      then:
        kind: Switch
        on:
          kind: Prompt
          template: "..."
        cases:
          REFLECT: { command: { Reflect: { prompt: "..." } } }
          CONFIRM: { command: ConfirmCompletion }
      reflectionMaxRounds: 2

    - priority: 4
      name: error_recovery
      if: { kind: outputContains, pattern: "error" }
      then:
        kind: Switch
        on:
          kind: Prompt
          model: thinking
          template: "..."  # error classification prompt
          parser:
            kind: structured
            pattern: "CLASS:\\s*(\\w+)\\s*RECOMMEND:\\s*(\\w+)"
            fields: [...]
        cases:
          RETRY: { command: { RetryTool: ... } }
          FIX: { command: { SendCustomInstruction: ... } }
          ESCALATE: { command: { EscalateToHuman: ... } }

    - priority: 99
      name: default_continue
      then: { command: ApproveAndContinue }
```

This **120-line file** replaces the current **~800 lines** of YAML across the default_tree, reflect_loop, error_recovery, and rate_limit files.

Key observations:
- **`if`/`then` replaces Sequence(Condition, Action)** — eliminating 50% of nodes.
- **`Switch` with `cases` replaces the Prompt+Selector+Sequence+Condition+Action pattern** — eliminating another 30%.
- **`cooldownMs` as a rule property** (not a decorator node) — `Cooldown` is only used for `RetryTool`; make it a first-class concept.
- **`reflectionMaxRounds` as a rule property** — same pattern. The ReflectionGuard decorator is only used around Prompt nodes.
- **Priority order replaces the root Selector** — no need for a `root_handler` Selector wrapping everything.

### 5.6 Desugaring Table

| High-Level Construct | Desugars To |
|---------------------|-------------|
| `DecisionRules` | `Selector(rule[1].desugar(), rule[2].desugar(), ..., rule[N].desugar())` |
| `Rule { if, then }` | `Sequence(Condition(if), then.desugar())` |
| `When { if, then }` | `Sequence(Condition(if), Action(then.command))` |
| `Switch { on: Prompt, cases }` | `Sequence(Prompt(on), Selector(for each case: Sequence(Condition(var==case), Action(case.command)), DefaultCase))` |
| `Switch { on: variable, cases }` | `Selector(for each case: Sequence(Condition(var==case), Action(case.command)), DefaultCase)` |
| `Pipeline { steps }` | `Sequence(for each step: if → Condition; then → Action)` |

The executor still runs the same BT AST. All high-level constructs compile to the existing low-level nodes.

---

## 6. AST & Runtime Improvements

### 6.1 Replace `Box<dyn Trait>` with Enums

The current design uses trait objects for Evaluator and OutputParser:

```rust
pub(crate) struct ConditionNode {
    pub evaluator: Box<dyn Evaluator>,  // allocation + no Clone
}
```

Switch to enums:

```rust
pub(crate) enum Evaluator {
    OutputContains { pattern: String, case_sensitive: bool },
    SituationIs { situation_type: String },
    ReflectionRoundUnder { max: u8 },
    VariableIs { key: String, expected: BlackboardValue },
    RegexMatch { re: Regex },
    Script { script: String },
    Or { conditions: Vec<Evaluator> },
    And { conditions: Vec<Evaluator> },
    Custom { name: String, params: HashMap<String, BlackboardValue> },
}

impl Evaluator {
    fn evaluate(&self, bb: &Blackboard) -> Result<bool, RuntimeError> {
        match self {
            Evaluator::OutputContains { pattern, case_sensitive } => { /* ... */ }
            Evaluator::VariableIs { key, expected } => { /* ... */ }
            // ...
        }
    }
}
```

Benefits:
- No heap allocation per evaluator
- Derives `Clone`, `Debug`, `PartialEq` for free
- Pattern-matching is faster than dynamic dispatch (no vtable lookup)
- Custom evaluators go through the `Custom` variant, not `Box<dyn Evaluator>`

Same treatment for `OutputParser`:

```rust
pub(crate) enum OutputParser {
    Enum { values: Vec<String>, case_sensitive: bool },
    Structured { pattern: Regex, fields: Vec<StructuredField> },
    Json { schema: Option<serde_json::Value> },
    Command { mapping: HashMap<String, Command> },
    Custom { name: String, params: HashMap<String, BlackboardValue> },
}
```

### 6.2 Use `enum_dispatch` to Eliminate the Manual Visitor

```rust
use enum_dispatch::enum_dispatch;

#[enum_dispatch]
pub(crate) trait NodeBehavior {
    fn tick(&mut self, ctx: &mut TickContext, tracer: &mut Tracer) -> Result<NodeStatus, RuntimeError>;
    fn reset(&mut self);
    fn name(&self) -> &str;
    fn children_mut(&mut self) -> Vec<&mut Node>;
    fn children(&self) -> Vec<&Node>;
}

#[enum_dispatch(NodeBehavior)]
pub(crate) enum Node {
    Selector(SelectorNode),
    Sequence(SequenceNode),
    // ...
}
```

This auto-generates all match arms. Adding a new node type = add the struct + impl the trait + add to the enum — O(1) changes, not O(N).

### 6.3 Typed Blackboard with Scoped Overlays

Replace the global `HashMap<String, BlackboardValue>` with a layered approach:

```rust
pub struct Blackboard {
    // Built-in fields (strongly typed, always present)
    pub task_description: String,
    pub provider_output: String,
    pub context_summary: String,
    pub reflection_round: u8,
    pub max_reflection_rounds: u8,
    pub confidence_accumulator: f64,
    pub agent_id: String,
    pub current_task_id: String,
    pub last_tool_call: Option<ToolCallRecord>,
    pub file_changes: Vec<FileChangeRecord>,

    // Scoped overlays for custom variables
    scopes: Vec<HashMap<String, BlackboardValue>>,
}

impl Blackboard {
    /// Start a new scope (e.g., entering a sub-tree).
    pub fn push_scope(&mut self) {
        self.scopes.push(HashMap::new());
    }

    /// End a scope, discarding all variables set within it.
    pub fn pop_scope(&mut self) {
        self.scopes.pop();
    }

    /// Set a variable in the current scope.
    pub fn set(&mut self, key: &str, value: BlackboardValue) {
        if let Some(scope) = self.scopes.last_mut() {
            scope.insert(key.to_string(), value);
        }
    }

    /// Get a variable, searching scopes from innermost to outermost.
    pub fn get(&self, key: &str) -> Option<&BlackboardValue> {
        for scope in self.scopes.iter().rev() {
            if let Some(v) = scope.get(key) { return Some(v); }
        }
        None
    }
}
```

SubTree execution automatically creates a scope. Variables from parent trees are visible (read-only), but writes don't leak outward.

### 6.4 Smaller, Grouped Command Enum

```rust
pub enum DecisionCommand {
    // Agent control
    Agent(AgentCommand),

    // Git operations
    Git(GitCommand, Option<String> /* worktree_path */),

    // Task management
    Task(TaskCommand),

    // Human interaction
    Human(HumanCommand),

    // Provider control
    Provider(ProviderCommand),
}

pub enum AgentCommand {
    ApproveAndContinue,
    Reflect { prompt: String },
    SendInstruction { prompt: String, target_agent: String },
    Terminate { reason: String },
    WakeUp,
}

pub enum GitCommand {
    Commit { message: String, wip: bool },
    Stash { description: String, include_untracked: bool },
    Discard,
    CreateBranch { name: String, base: String },
    Rebase { base: String },
}

pub enum TaskCommand {
    ConfirmCompletion,
    StopIfComplete { reason: String },
    PrepareStart { task_id: String, description: String },
}

pub enum HumanCommand {
    Escalate { reason: String, context: Option<String> },
    SelectOption { option_id: String },
    SkipDecision,
}

pub enum ProviderCommand {
    RetryTool { tool_name: String, args: Option<String>, max_attempts: u32 },
    Switch { provider_type: String },
    SuggestCommit { message: String, mandatory: bool, reason: String },
    PreparePr { title: String, description: String, base: String, draft: bool },
}
```

This groups 22 flat variants into 5 categories × 4 variants each = same information, cleaner namespace.

---

## 7. Migration Strategy

### Phase 1: Desugaring Layer (non-breaking)
- Add `Switch`, `When`, `Pipeline`, `DecisionRules` as YAML-level constructs
- Implement desugaring to existing BT AST
- Old YAML continues to work unchanged
- Both syntaxes produce the same executor traces

### Phase 2: AST Refactor (minor breaking)
- Switch `Box<dyn Evaluator>` → `Evaluator` enum
- Switch `Box<dyn OutputParser>` → `OutputParser` enum
- Adopt `enum_dispatch` for `Node`
- Ensure serialization compatibility

### Phase 3: Blackboard Scope (non-breaking)
- Add `push_scope`/`pop_scope` — opt-in
- SubTree nodes auto-create scopes behind a feature flag

### Phase 4: Command Grouping (breaking, apiVersion v2)
- Restructure `DecisionCommand` into grouped enums
- Add Display impls for each group
- Bump `apiVersion` to v2
- Provide v1→v2 migration tool

### Phase 5: Template Engine (non-breaking)
- Replace hand-rolled template engine with `minijinja` or `tera`
- Add template validation at load time
- Extend test coverage for all Jinja2 features in use

### Phase 6: Deprecate Old YAML Syntax
- Mark `Selector`/`Sequence`-based patterns as deprecated
- Auto-migrate old YAML to new syntax with a CLI tool
- Remove old syntax after one release cycle

---

## 8. Summary: What Changes

| Dimension | Current | Proposed |
|-----------|---------|-----------|
| **Authoring syntax** | 6 nodes for a branch | 1 `Switch` node |
| **Root structure** | Selector wrapping handlers | `DecisionRules` with priority |
| **Condition+Action** | Sequence(Condition, Action) | `When { if, then }` |
| **Cool down** | Cooldown decorator node | `cooldownMs` property on rule |
| **Reflection limit** | ReflectionGuard decorator node | `reflectionMaxRounds` property on rule |
| **Blackboard** | Global flat HashMap | Layered scopes |
| **Evaluator** | `Box<dyn Evaluator>` | `Evaluator` enum |
| **Parser** | `Box<dyn OutputParser>` | `OutputParser` enum |
| **Node walking** | Manual match per operation | `enum_dispatch` |
| **Command enum** | 22 flat variants | 5 groups × ~4 variants each |
| **Template engine** | Hand-rolled 300-line stub | `minijinja` or `tera` |
| **SubTree identity** | Inlined, lost at parse time | Preserved in traces |
| **Error handling** | Ad-hoc via Selector fallback | `on_error` property on rules |

### What Stays the Same

- YAML as the authoring format
- Trait-based DI (Session, Clock, Fs, Logger)
- Hot reload with atomic swap
- `apiVersion` for migration
- Same-session LLM integration
- Registry pattern for extensibility
- Fail-fast at load time
- Prompt node's Running status
- The BT executor as the compilation target (all shorthands desugar to BT nodes)

---

*Document version: 1.0*
*Last updated: 2026-04-24*
