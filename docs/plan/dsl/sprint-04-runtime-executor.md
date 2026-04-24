# Sprint 4: Runtime Executor

## Metadata

- Sprint ID: `dsl-sprint-04`
- Title: `Runtime Executor`
- Duration: 2 weeks
- Priority: P0 (Critical)
- Status: `Backlog`
- Created: 2026-04-20

## Sprint Goal

Build the complete executor tick loop and all node behavior implementations. The executor runs BehaviorTree AST against a Blackboard, producing `Vec<DecisionCommand>` and `TraceEntry` output. The Prompt node's async same-session lifecycle is fully supported with resume.

## Dependencies

- **Sprint 1** (`dsl-sprint-01`): Blackboard, External traits, Error types.
- **Sprint 2** (`dsl-sprint-02`): AST nodes, enum_dispatch.
- **Sprint 3** (`dsl-sprint-03`): Evaluators, Parsers, Templates.

## Non-goals

- No hot reload (Sprint 5).
- No metrics collection (Sprint 5).
- No host integration bridges (Sprint 5).

---

## Stories

### Story 4.1: Executor & Tick Loop

**Priority**: P0
**Effort**: 5 points
**Status**: Backlog

Implement the `DslRunner` trait, `Executor` struct, and `TickContext`.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T4.1.1 | Define `DslRunner` trait (`tick`, `reset`) | Todo | - |
| T4.1.2 | Define `TickResult` struct (`status`, `commands`, `trace`) | Todo | - |
| T4.1.3 | Define `TickContext` struct (`blackboard`, `session`, `clock`, `logger`) | Todo | - |
| T4.1.4 | Implement `Executor` struct (`running_path`, `is_running`) | Todo | - |
| T4.1.5 | Implement `Executor::tick` with running-path management | Todo | - |
| T4.1.6 | Implement `Executor::reset` | Todo | - |
| T4.1.7 | Implement command draining from blackboard after tick | Todo | - |
| T4.1.8 | Implement `Node::resume_at` for async Prompt continuation | Todo | - |
| T4.1.9 | Write unit tests for basic tick loop | Todo | - |
| T4.1.10 | Write unit tests for Running → Success resume | Todo | - |
| T4.1.11 | Write unit tests for Running → Failure resume | Todo | - |

#### Acceptance Criteria

- `tick` returns `TickResult` with drained commands and trace entries.
- `Running` status stores the path via `Tracer::running_path()`.
- On next tick, `resume_at` continues from the stored path.
- `reset` clears `running_path` and all node `active_child` states.

#### Technical Notes

```rust
pub trait DslRunner {
    fn tick(&mut self, tree: &mut Tree, ctx: &mut TickContext) -> Result<TickResult, RuntimeError>;
    fn reset(&mut self);
}

pub struct Executor {
    running_path: Vec<usize>,
    is_running: bool,
}

impl DslRunner for Executor {
    fn tick(&mut self, tree: &mut Tree, ctx: &mut TickContext) -> Result<TickResult, RuntimeError> {
        let mut tracer = Tracer::new();
        let status = if self.is_running {
            tree.spec.root.resume_at(&self.running_path, 0, ctx, &mut tracer)?
        } else {
            tree.spec.root.tick(ctx, &mut tracer)?
        };
        // running path management, command draining
    }
}
```

---

### Story 4.2: Composite Nodes

**Priority**: P0
**Effort**: 3 points
**Status**: Backlog

Implement Selector, Sequence, and Parallel node behaviors.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T4.2.1 | Implement `SelectorNode::tick` (first Success wins, Failure falls through) | Todo | - |
| T4.2.2 | Implement `SelectorNode::reset` | Todo | - |
| T4.2.3 | Implement `SequenceNode::tick` (all must succeed, first Failure aborts) | Todo | - |
| T4.2.4 | Implement `SequenceNode::reset` | Todo | - |
| T4.2.5 | Implement `ParallelNode::tick` with `ParallelPolicy` | Todo | - |
| T4.2.6 | Implement `ParallelNode::reset` | Todo | - |
| T4.2.7 | Write unit tests for Selector | Todo | - |
| T4.2.8 | Write unit tests for Sequence | Todo | - |
| T4.2.9 | Write unit tests for Parallel (allSuccess, anySuccess, majority) | Todo | - |

#### Acceptance Criteria

- Selector: `Success` on first child success; `Failure` if all fail.
- Sequence: `Success` if all succeed; `Failure` on first child failure.
- Parallel: `allSuccess` → all must succeed; `anySuccess` → at least one; `majority` → >50%.
- `active_child` is correctly set on `Running` and cleared on terminal status.

#### Technical Notes

```rust
impl NodeBehavior for SelectorNode {
    fn tick(&mut self, ctx: &mut TickContext, tracer: &mut Tracer) -> Result<NodeStatus, RuntimeError> {
        let start = self.active_child.unwrap_or(0);
        for i in start..self.children.len() {
            tracer.enter(&self.name, i);
            let status = self.children[i].tick(ctx, tracer)?;
            tracer.exit(&self.name, i, status, duration);
            match status {
                NodeStatus::Success => { self.active_child = None; return Ok(Success); }
                NodeStatus::Running => { self.active_child = Some(i); return Ok(Running); }
                NodeStatus::Failure => continue,
            }
        }
        self.active_child = None;
        Ok(Failure)
    }
}
```

---

### Story 4.3: Decorator Nodes

**Priority**: P0
**Effort**: 3 points
**Status**: Backlog

Implement Inverter, Repeater, Cooldown, ReflectionGuard, and ForceHuman.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T4.3.1 | Implement `InverterNode` (Success ↔ Failure) | Todo | - |
| T4.3.2 | Implement `RepeaterNode` (loop up to `max_attempts`) | Todo | - |
| T4.3.3 | Implement `CooldownNode` (clock-based gate) | Todo | - |
| T4.3.4 | Implement `ReflectionGuardNode` (round counter) | Todo | - |
| T4.3.5 | Implement `ForceHumanNode` (auto-escalate on success) | Todo | - |
| T4.3.6 | Write unit tests for each decorator | Todo | - |
| T4.3.7 | Write unit tests for Cooldown with MockClock | Todo | - |
| T4.3.8 | Write unit tests for ReflectionGuard round counting | Todo | - |

#### Acceptance Criteria

- Inverter: `Success` → `Failure`, `Failure` → `Success`, `Running` passes through.
- Repeater: loops child until `max_attempts` successes; returns `Failure` on child failure.
- Cooldown: returns `Failure` if `now - last_success < duration_ms`.
- ReflectionGuard: returns `Failure` if `reflection_round >= max_rounds`; increments on success.
- ForceHuman: pushes `EscalateToHuman` command when child succeeds.

#### Technical Notes

```rust
impl NodeBehavior for CooldownNode {
    fn tick(&mut self, ctx: &mut TickContext, tracer: &mut Tracer) -> Result<NodeStatus, RuntimeError> {
        let duration = Duration::from_millis(self.duration_ms);
        if let Some(last) = self.last_success {
            if ctx.clock.now().duration_since(last) < duration {
                return Ok(NodeStatus::Failure);
            }
        }
        let status = self.child.tick(ctx, tracer)?;
        if status == NodeStatus::Success {
            self.last_success = Some(ctx.clock.now());
        }
        Ok(status)
    }
}
```

---

### Story 4.4: Leaf Nodes

**Priority**: P0
**Effort**: 3 points
**Status**: Backlog

Implement Condition, Action, and SetVar.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T4.4.1 | Implement `ConditionNode::tick` (evaluator → Success/Failure) | Todo | - |
| T4.4.2 | Implement `ActionNode::tick` (render templates + push command) | Todo | - |
| T4.4.3 | Implement `ActionNode` optional `when` guard | Todo | - |
| T4.4.4 | Implement `SetVarNode::tick` (write to blackboard scope) | Todo | - |
| T4.4.5 | Write unit tests for Condition with all evaluator types | Todo | - |
| T4.4.6 | Write unit tests for Action with template rendering | Todo | - |
| T4.4.7 | Write unit tests for Action `when` guard | Todo | - |
| T4.4.8 | Write unit tests for SetVar with all BlackboardValue types (string, integer, float, boolean, list) | Todo | - |

#### Acceptance Criteria

- Condition: `Success` if evaluator returns `true`, else `Failure`.
- Action: renders command templates, pushes to `blackboard.commands`, returns `Success`.
- Action with `when`: returns `Failure` if guard evaluator is `false`.
- SetVar: writes to innermost scope, returns `Success`.

#### Technical Notes

```rust
impl NodeBehavior for ActionNode {
    fn tick(&mut self, ctx: &mut TickContext, tracer: &mut Tracer) -> Result<NodeStatus, RuntimeError> {
        if let Some(ref evaluator) = self.when {
            if !evaluator.evaluate(ctx.blackboard)? {
                return Ok(NodeStatus::Failure);
            }
        }
        let rendered = render_command_templates(&self.command, ctx.blackboard)?;
        ctx.blackboard.push_command(rendered.clone());
        tracer.record_action(&self.name, &rendered);
        Ok(NodeStatus::Success)
    }
}
```

---

### Story 4.5: Prompt Node Async Lifecycle

**Priority**: P0
**Effort**: 5 points
**Status**: Backlog

Implement the Prompt node's two-tick async lifecycle. This is the most complex leaf node.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T4.5.1 | Implement first-tick path: render template + `session.send_with_hint` | Todo | - |
| T4.5.2 | Set `pending = true` and `sent_at = Some(now)` on first tick | Todo | - |
| T4.5.3 | Implement second-tick path: check `session.is_ready()` | Todo | - |
| T4.5.4 | Implement timeout check using `clock.now() - sent_at` | Todo | - |
| T4.5.5 | Receive reply, parse with `OutputParser`, store `llm_responses` | Todo | - |
| T4.5.6 | Handle `__command` magic key for CommandParser | Todo | - |
| T4.5.7 | Apply `sets` mappings to blackboard | Todo | - |
| T4.5.8 | Implement `PromptNode::reset` (clear pending + sent_at) | Todo | - |
| T4.5.9 | Write unit tests for full Prompt lifecycle (tick 1 → tick 2 → Success) | Todo | - |
| T4.5.10 | Write unit tests for timeout | Todo | - |
| T4.5.11 | Write unit tests for parse failure | Todo | - |
| T4.5.12 | Write unit tests for CommandParser | Todo | - |

#### Acceptance Criteria

- First tick: sends message, returns `Running`.
- Second tick (ready): parses reply, stores values, returns `Success`.
- Second tick (not ready): returns `Running`.
- Timeout: returns `Failure`, clears pending state.
- Parse failure: returns `Failure`, clears pending state.

#### Technical Notes

```rust
impl NodeBehavior for PromptNode {
    fn tick(&mut self, ctx: &mut TickContext, tracer: &mut Tracer) -> Result<NodeStatus, RuntimeError> {
        if self.pending {
            // timeout check
            if let Some(sent_at) = self.sent_at {
                let timeout = Duration::from_millis(self.timeout_ms);
                if ctx.clock.now().duration_since(sent_at) > timeout {
                    self.pending = false; self.sent_at = None;
                    tracer.record_prompt_failure(&self.name, "timeout");
                    return Ok(NodeStatus::Failure);
                }
            }
            if !ctx.session.is_ready() { return Ok(NodeStatus::Running); }
            let reply = ctx.session.receive()?;
            ctx.blackboard.store_llm_response(&self.name, reply.clone());
            match self.parser.parse(&reply) {
                Ok(values) => { /* handle __command or sets */ }
                Err(e) => { /* failure */ }
            }
        } else {
            // first tick: render and send
        }
    }
}
```

---

### Story 4.6: SubTree Node & Scope Isolation

**Priority**: P1
**Effort**: 3 points
**Status**: Backlog

Implement SubTree execution with identity-preserving traces and scoped variable isolation.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T4.6.1 | Implement `SubTreeNode::tick` with `push_scope` / `pop_scope` | Todo | - |
| T4.6.2 | Emit `EnterSubTree` / `ExitSubTree` trace entries | Todo | - |
| T4.6.3 | Handle unresolved SubTree (`resolved_root: None`) as error | Todo | - |
| T4.6.4 | Implement `SubTreeNode::reset` (delegate to resolved root) | Todo | - |
| T4.6.5 | Write unit tests for scope isolation | Todo | - |
| T4.6.6 | Write unit tests for identity-preserving traces | Todo | - |
| T4.6.7 | Write unit tests for unresolved SubTree error | Todo | - |

#### Acceptance Criteria

- SubTree tick creates a new scope; variables written inside do not leak out.
- Parent variables are visible (read-only) to child scopes.
- Traces contain `EnterSubTree { name, ref_name }` and `ExitSubTree` entries.
- Unresolved SubTree returns `RuntimeError::SubTreeNotResolved`.

#### Technical Notes

```rust
impl NodeBehavior for SubTreeNode {
    fn tick(&mut self, ctx: &mut TickContext, tracer: &mut Tracer) -> Result<NodeStatus, RuntimeError> {
        tracer.enter_subtree(&self.name, &self.ref_name);
        ctx.blackboard.push_scope();
        let status = match &mut self.resolved_root {
            Some(root) => root.tick(ctx, tracer)?,
            None => return Err(RuntimeError::SubTreeNotResolved { name: self.ref_name.clone() }),
        };
        ctx.blackboard.pop_scope();
        tracer.exit_subtree(&self.name, &self.ref_name, status);
        Ok(status)
    }
}
```

---

## Sprint Completion Criteria

- [ ] `cargo check` passes for the `decision-dsl` crate.
- [ ] `cargo test --lib` passes with ≥95% coverage on runtime and node modules.
- [ ] All node types have dedicated unit tests.
- [ ] Prompt node async lifecycle is tested with `MockSession`.
- [ ] SubTree scope isolation is tested with nested variable reads/writes.
