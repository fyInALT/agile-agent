# Decision DSL: Runtime Execution

> Runtime execution specification for the decision DSL engine. Covers the Executor tick loop, node implementations (via `enum_dispatch`), the scoped SubTree execution model, and the Prompt node's async lifecycle.

---

## 1. Executor / Tick Loop

### 1.1 TickContext

```rust
pub struct TickContext<'a> {
    pub blackboard: &'a mut Blackboard,
    pub session: &'a mut dyn Session,
    pub clock: &'a dyn Clock,
    pub logger: &'a dyn Logger,
}
```

### 1.2 Executor

```rust
pub struct Executor {
    running_path: Vec<usize>,
    is_running: bool,
}

impl Executor {
    pub fn new() -> Self {
        Self { running_path: Vec::new(), is_running: false }
    }
}

impl DslRunner for Executor {
    fn tick(&mut self, tree: &mut Tree, ctx: &mut TickContext) -> Result<TickResult, RuntimeError> {
        let mut tracer = Tracer::new();

        let status = if self.is_running {
            tree.spec.root.resume_at(&self.running_path, 0, ctx, &mut tracer)?
        } else {
            tree.spec.root.tick(ctx, &mut tracer)?
        };

        if status == NodeStatus::Running {
            self.is_running = true;
            self.running_path = tracer.running_path().to_vec();
        } else {
            self.is_running = false;
            self.running_path.clear();
        }

        let commands = ctx.blackboard.drain_commands();
        Ok(TickResult { status, commands, trace: tracer.into_entries() })
    }

    fn reset(&mut self) {
        self.is_running = false;
        self.running_path.clear();
    }
}
```

### 1.3 Decision Cycle Lifecycle

Each decision cycle follows a 6-phase lifecycle, matching the architecture in `decision-layer-design.md` §6.3:

```
1. OBSERVE
   └── Collect task_description, provider_output, context_summary
       from the work agent's state.

2. BUILD BLACKBOARD
   └── Populate Blackboard with inputs and persistent state
       (reflection_round, decision_history, etc.).

3. RESET
   └── Call executor.reset() to clear internal node state
       and running_path.

4. TICK
   └── Call executor.tick(&mut tree, &mut ctx).
       └── If Running: store executor state, return empty commands.
           Poll again later.
       └── If Success/Failure: collect commands from blackboard.

5. PERSIST
   └── Write changed state (reflection_round, variables, etc.)
       back to the agent's persistent state store.

6. RETURN
   └── Return Vec<DecisionCommand> to the runtime host.
```

This lifecycle is driven by the host (e.g., `agent-decision`'s `DslDecisionEngine`). The host calls `build_blackboard()` → `executor.reset()` → `executor.tick()` → persists state → returns commands. See the Host Integration example in `README.md` §5.

### 1.4 `enum_dispatch` Node Trait

All node structs implement the `NodeBehavior` trait. `enum_dispatch` auto-generates the `Node::tick()` and `Node::reset()` match arms:

```rust
#[enum_dispatch]
pub(crate) trait NodeBehavior {
    fn tick(&mut self, ctx: &mut TickContext, tracer: &mut Tracer) -> Result<NodeStatus, RuntimeError>;
    fn reset(&mut self);
    fn name(&self) -> &str;
    fn children(&self) -> Vec<&Node>;
    fn children_mut(&mut self) -> Vec<&mut Node>;
}
```

Adding a new node type requires: (1) define the struct, (2) impl `NodeBehavior`, (3) add to the `Node` enum. No other code changes needed.

---

## 2. Composite Node Implementations

### 2.1 Selector

```rust
impl NodeBehavior for SelectorNode {
    fn tick(&mut self, ctx: &mut TickContext, tracer: &mut Tracer) -> Result<NodeStatus, RuntimeError> {
        let start = self.active_child.unwrap_or(0);

        for i in start..self.children.len() {
            tracer.enter(&self.name, i);
            let t0 = ctx.clock.now();
            let status = self.children[i].tick(ctx, tracer)?;
            let duration = t0.elapsed();
            tracer.exit(&self.name, i, status, duration);

            match status {
                NodeStatus::Success => {
                    self.active_child = None;
                    return Ok(NodeStatus::Success);
                }
                NodeStatus::Running => {
                    self.active_child = Some(i);
                    return Ok(NodeStatus::Running);
                }
                NodeStatus::Failure => continue,
            }
        }

        self.active_child = None;
        Ok(NodeStatus::Failure)
    }

    fn reset(&mut self) {
        self.active_child = None;
        for child in &mut self.children { child.reset(); }
    }

    fn name(&self) -> &str { &self.name }
    fn children(&self) -> Vec<&Node> { self.children.iter().collect() }
    fn children_mut(&mut self) -> Vec<&mut Node> { self.children.iter_mut().collect() }
}
```

### 2.2 Sequence

```rust
impl NodeBehavior for SequenceNode {
    fn tick(&mut self, ctx: &mut TickContext, tracer: &mut Tracer) -> Result<NodeStatus, RuntimeError> {
        let start = self.active_child.unwrap_or(0);

        for i in start..self.children.len() {
            tracer.enter(&self.name, i);
            let t0 = ctx.clock.now();
            let status = self.children[i].tick(ctx, tracer)?;
            let duration = t0.elapsed();
            tracer.exit(&self.name, i, status, duration);

            match status {
                NodeStatus::Success => continue,
                NodeStatus::Running => {
                    self.active_child = Some(i);
                    return Ok(NodeStatus::Running);
                }
                NodeStatus::Failure => {
                    self.active_child = None;
                    return Ok(NodeStatus::Failure);
                }
            }
        }

        self.active_child = None;
        Ok(NodeStatus::Success)
    }

    fn reset(&mut self) {
        self.active_child = None;
        for child in &mut self.children { child.reset(); }
    }

    fn name(&self) -> &str { &self.name }
    fn children(&self) -> Vec<&Node> { self.children.iter().collect() }
    fn children_mut(&mut self) -> Vec<&mut Node> { self.children.iter_mut().collect() }
}
```

### 2.3 Parallel

```rust
impl NodeBehavior for ParallelNode {
    fn tick(&mut self, ctx: &mut TickContext, tracer: &mut Tracer) -> Result<NodeStatus, RuntimeError> {
        let mut successes = 0;
        let mut failures = 0;

        for (i, child) in self.children.iter_mut().enumerate() {
            tracer.enter(&self.name, i);
            let t0 = ctx.clock.now();
            let status = child.tick(ctx, tracer)?;
            let duration = t0.elapsed();
            tracer.exit(&self.name, i, status, duration);

            match status {
                NodeStatus::Success => successes += 1,
                NodeStatus::Failure => failures += 1,
                NodeStatus::Running => return Ok(NodeStatus::Running),
            }
        }

        let total = self.children.len();
        Ok(match self.policy {
            ParallelPolicy::AllSuccess if successes == total => NodeStatus::Success,
            ParallelPolicy::AllSuccess => NodeStatus::Failure,
            ParallelPolicy::AnySuccess if successes > 0 => NodeStatus::Success,
            ParallelPolicy::AnySuccess => NodeStatus::Failure,
            ParallelPolicy::Majority if successes > total / 2 => NodeStatus::Success,
            ParallelPolicy::Majority => NodeStatus::Failure,
        })
    }

    fn reset(&mut self) {
        for child in &mut self.children { child.reset(); }
    }

    fn name(&self) -> &str { &self.name }
    fn children(&self) -> Vec<&Node> { self.children.iter().collect() }
    fn children_mut(&mut self) -> Vec<&mut Node> { self.children.iter_mut().collect() }
}
```

---

## 3. Decorator Node Implementations

### 3.1 Inverter

```rust
impl NodeBehavior for InverterNode {
    fn tick(&mut self, ctx: &mut TickContext, tracer: &mut Tracer) -> Result<NodeStatus, RuntimeError> {
        let status = self.child.tick(ctx, tracer)?;
        Ok(match status {
            NodeStatus::Success => NodeStatus::Failure,
            NodeStatus::Failure => NodeStatus::Success,
            NodeStatus::Running => NodeStatus::Running,
        })
    }
    fn reset(&mut self) { self.child.reset(); }
    fn name(&self) -> &str { &self.name }
    fn children(&self) -> Vec<&Node> { vec![self.child.as_ref()] }
    fn children_mut(&mut self) -> Vec<&mut Node> { vec![self.child.as_mut()] }
}
```

### 3.2 Repeater

```rust
impl NodeBehavior for RepeaterNode {
    fn tick(&mut self, ctx: &mut TickContext, tracer: &mut Tracer) -> Result<NodeStatus, RuntimeError> {
        while self.current < self.max_attempts {
            match self.child.tick(ctx, tracer)? {
                NodeStatus::Success => {
                    self.current += 1;
                    if self.current >= self.max_attempts {
                        return Ok(NodeStatus::Success);
                    }
                }
                NodeStatus::Failure => return Ok(NodeStatus::Failure),
                NodeStatus::Running => return Ok(NodeStatus::Running),
            }
        }
        Ok(NodeStatus::Success)
    }
    fn reset(&mut self) { self.current = 0; self.child.reset(); }
    fn name(&self) -> &str { &self.name }
    fn children(&self) -> Vec<&Node> { vec![self.child.as_ref()] }
    fn children_mut(&mut self) -> Vec<&mut Node> { vec![self.child.as_mut()] }
}
```

### 3.3 Cooldown

```rust
impl NodeBehavior for CooldownNode {
    fn tick(&mut self, ctx: &mut TickContext, tracer: &mut Tracer) -> Result<NodeStatus, RuntimeError> {
        let duration = std::time::Duration::from_millis(self.duration_ms);
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
    fn reset(&mut self) { self.last_success = None; self.child.reset(); }
    fn name(&self) -> &str { &self.name }
    fn children(&self) -> Vec<&Node> { vec![self.child.as_ref()] }
    fn children_mut(&mut self) -> Vec<&mut Node> { vec![self.child.as_mut()] }
}
```

### 3.4 ReflectionGuard

```rust
impl NodeBehavior for ReflectionGuardNode {
    fn tick(&mut self, ctx: &mut TickContext, tracer: &mut Tracer) -> Result<NodeStatus, RuntimeError> {
        let count = ctx.blackboard.reflection_round;
        if count >= self.max_rounds {
            return Ok(NodeStatus::Failure);
        }
        let status = self.child.tick(ctx, tracer)?;
        if status == NodeStatus::Success {
            ctx.blackboard.reflection_round = count + 1;
        }
        Ok(status)
    }
    fn reset(&mut self) { self.child.reset(); }
    fn name(&self) -> &str { &self.name }
    fn children(&self) -> Vec<&Node> { vec![self.child.as_ref()] }
    fn children_mut(&mut self) -> Vec<&mut Node> { vec![self.child.as_mut()] }
}
```

### 3.5 ForceHuman

```rust
impl NodeBehavior for ForceHumanNode {
    fn tick(&mut self, ctx: &mut TickContext, tracer: &mut Tracer) -> Result<NodeStatus, RuntimeError> {
        let status = self.child.tick(ctx, tracer)?;
        if status == NodeStatus::Success {
            ctx.blackboard.push_command(DecisionCommand::Human(HumanCommand::Escalate {
                reason: self.reason.clone(),
                context: Some(format!("Forced by ForceHuman decorator after {} succeeded", self.child.name())),
            }));
        }
        Ok(status)
    }
    fn reset(&mut self) { self.child.reset(); }
    fn name(&self) -> &str { &self.name }
    fn children(&self) -> Vec<&Node> { vec![self.child.as_ref()] }
    fn children_mut(&mut self) -> Vec<&mut Node> { vec![self.child.as_mut()] }
}
```

---

## 4. High-Level Node Implementations

### 4.1 When (Guarded Action)

`When` is a desugared shorthand for `Sequence(Condition, Action)`. At runtime it evaluates the condition and, if true, executes the action.

```rust
impl NodeBehavior for WhenNode {
    fn tick(&mut self, ctx: &mut TickContext, tracer: &mut Tracer) -> Result<NodeStatus, RuntimeError> {
        // Evaluate condition
        if !self.condition.evaluate(ctx.blackboard)? {
            return Ok(NodeStatus::Failure);
        }
        // Execute action
        self.action.tick(ctx, tracer)
    }
    fn reset(&mut self) { self.action.reset(); }
    fn name(&self) -> &str { &self.name }
    fn children(&self) -> Vec<&Node> { vec![self.action.as_ref()] }
    fn children_mut(&mut self) -> Vec<&mut Node> { vec![self.action.as_mut()] }
}
```

---

## 5. Leaf Node Implementations

### 5.1 Condition

```rust
impl NodeBehavior for ConditionNode {
    fn tick(&mut self, ctx: &mut TickContext, tracer: &mut Tracer) -> Result<NodeStatus, RuntimeError> {
        let result = self.evaluator.evaluate(ctx.blackboard)?;
        tracer.record_eval(&self.name, &self.evaluator, result);
        Ok(if result { NodeStatus::Success } else { NodeStatus::Failure })
    }
    fn reset(&mut self) {}
    fn name(&self) -> &str { &self.name }
    fn children(&self) -> Vec<&Node> { vec![] }
    fn children_mut(&mut self) -> Vec<&mut Node> { vec![] }
}
```

### 5.2 Action

```rust
impl NodeBehavior for ActionNode {
    fn tick(&mut self, ctx: &mut TickContext, tracer: &mut Tracer) -> Result<NodeStatus, RuntimeError> {
        // Check optional precondition
        if let Some(ref evaluator) = self.when {
            if !evaluator.evaluate(ctx.blackboard)? {
                return Ok(NodeStatus::Failure);
            }
        }

        // Render template strings in command fields
        let rendered_cmd = render_command_templates(&self.command, ctx.blackboard)?;

        ctx.blackboard.push_command(rendered_cmd.clone());
        tracer.record_action(&self.name, &rendered_cmd);
        Ok(NodeStatus::Success)
    }
    fn reset(&mut self) {}
    fn name(&self) -> &str { &self.name }
    fn children(&self) -> Vec<&Node> { vec![] }
    fn children_mut(&mut self) -> Vec<&mut Node> { vec![] }
}
```

### 5.3 SetVar

```rust
impl NodeBehavior for SetVarNode {
    fn tick(&mut self, ctx: &mut TickContext, _tracer: &mut Tracer) -> Result<NodeStatus, RuntimeError> {
        ctx.blackboard.set(&self.key, self.value.clone());
        Ok(NodeStatus::Success)
    }
    fn reset(&mut self) {}
    fn name(&self) -> &str { &self.name }
    fn children(&self) -> Vec<&Node> { vec![] }
    fn children_mut(&mut self) -> Vec<&mut Node> { vec![] }
}
```

---

## 6. SubTree Node (Identity-Preserving)

SubTree nodes preserve their identity in traces. They create a new Blackboard scope, execute the resolved subtree within it, then pop the scope.

```rust
impl NodeBehavior for SubTreeNode {
    fn tick(&mut self, ctx: &mut TickContext, tracer: &mut Tracer) -> Result<NodeStatus, RuntimeError> {
        tracer.enter_subtree(&self.name, &self.ref_name);

        // Create a new variable scope
        ctx.blackboard.push_scope();

        // Execute the resolved subtree root
        let status = match &mut self.resolved_root {
            Some(root) => root.tick(ctx, tracer)?,
            None => return Err(RuntimeError::Custom(
                format!("SubTree '{}' not resolved", self.ref_name)
            )),
        };

        // Pop scope — discard any variables set inside the subtree
        ctx.blackboard.pop_scope();

        tracer.exit_subtree(&self.name, &self.ref_name, status);
        Ok(status)
    }

    fn reset(&mut self) {
        if let Some(root) = &mut self.resolved_root {
            root.reset();
        }
    }

    fn name(&self) -> &str { &self.name }
    fn children(&self) -> Vec<&Node> {
        self.resolved_root.as_ref().map(|r| vec![r.as_ref()]).unwrap_or_default()
    }
    fn children_mut(&mut self) -> Vec<&mut Node> {
        self.resolved_root.as_mut().map(|r| vec![r.as_mut()]).unwrap_or_default()
    }
}
```

**Key difference from old design**: SubTrees are NOT inlined at parse time. The `resolved_root` is resolved at load time but the `SubTreeNode` wrapper remains in the AST. Traces show `enter_subtree("use_reflect_loop", "reflect")` and `exit_subtree(...)` boundaries.

---

## 7. Prompt Node (Async Same-Session)

The Prompt node implements the same-session invariant. It uses `model` as a hint passed to the host's `Session` implementation.

### 7.1 Lifecycle

```
Tick 1: Prompt node renders template → sends to session → returns Running
  ↓
[Host polls; session receives reply]
  ↓
Tick 2: Prompt node checks session.is_ready() → receives reply
        Parses reply into Blackboard values
        Stores raw response in blackboard.llm_responses
        Returns Success or Failure
```

### 7.2 Implementation

```rust
impl NodeBehavior for PromptNode {
    fn tick(&mut self, ctx: &mut TickContext, tracer: &mut Tracer) -> Result<NodeStatus, RuntimeError> {
        if self.pending {
            // Check timeout first
            if let Some(sent_at) = self.sent_at {
                let timeout = std::time::Duration::from_millis(self.timeout_ms);
                if ctx.clock.now().duration_since(sent_at) > timeout {
                    self.pending = false;
                    self.sent_at = None;
                    tracer.record_prompt_failure(&self.name, "timeout");
                    return Ok(NodeStatus::Failure);
                }
            }

            // Async continuation: check for reply
            if !ctx.session.is_ready() {
                return Ok(NodeStatus::Running);
            }

            let reply = ctx.session.receive()?;
            ctx.blackboard.store_llm_response(&self.name, reply.clone());

            match self.parser.parse(&reply) {
                Ok(values) => {
                    // Handle CommandParser special case (__command magic key)
                    if let Some(BlackboardValue::String(cmd_json)) = values.get("__command") {
                        let cmd: DecisionCommand = serde_json::from_str(cmd_json)
                            .map_err(|e| RuntimeError::FilterError(e.to_string()))?;
                        ctx.blackboard.push_command(cmd);
                    } else {
                        // Normal set mapping
                        for mapping in &self.sets {
                            if let Some(value) = values.get(&mapping.field) {
                                ctx.blackboard.set(&mapping.key, value.clone());
                            }
                        }
                    }

                    self.pending = false;
                    self.sent_at = None;
                    tracer.record_prompt_success(&self.name, &reply);
                    Ok(NodeStatus::Success)
                }
                Err(e) => {
                    self.pending = false;
                    self.sent_at = None;
                    tracer.record_prompt_failure(&self.name, &e.to_string());
                    Ok(NodeStatus::Failure)
                }
            }
        } else {
            // First tick: render and send
            let context = ctx.blackboard.to_template_context();
            let rendered = render_prompt_template(&self.template, &context)
                .map_err(|e| RuntimeError::FilterError(e.to_string()))?;

            ctx.session.send_with_hint(
                &rendered,
                self.model.as_deref().unwrap_or("standard"),
            )?;

            self.pending = true;
            self.sent_at = Some(ctx.clock.now());
            tracer.record_prompt_sent(&self.name);
            Ok(NodeStatus::Running)
        }
    }

    fn reset(&mut self) {
        self.pending = false;
        self.sent_at = None;
    }

    fn name(&self) -> &str { &self.name }
    fn children(&self) -> Vec<&Node> { vec![] }
    fn children_mut(&mut self) -> Vec<&mut Node> { vec![] }
}
```

### 7.3 Session Trait (Updated)

```rust
pub trait Session {
    /// Send a message to the LLM session. Returns immediately.
    fn send(&mut self, message: &str) -> Result<(), SessionError>;

    /// Send with a model hint (e.g., "thinking" or "standard").
    fn send_with_hint(&mut self, message: &str, model: &str) -> Result<(), SessionError>;

    /// Check if a reply is available.
    fn is_ready(&self) -> bool;

    /// Receive the reply. Call only after is_ready() returns true.
    fn receive(&mut self) -> Result<String, SessionError>;
}
```

---

## 8. Tree Resume (Async Prompt Continuation)

When a Prompt node returns `Running`, the executor stores the path and resumes from there on the next tick.

```rust
impl Node {
    pub fn resume_at(
        &mut self,
        path: &[usize],
        depth: usize,
        ctx: &mut TickContext,
        tracer: &mut Tracer,
    ) -> Result<NodeStatus, RuntimeError> {
        if depth >= path.len() {
            return self.tick(ctx, tracer);
        }

        let child_idx = path[depth];

        // Use children_mut() to get mutable references by index
        let children = self.children_mut();
        let child = &mut children[child_idx];

        ctx.logger.log(
            LogLevel::Trace,
            "decision-dsl",
            &format!("resume_at: depth={}, child_idx={}, node={}", depth, child_idx, child.name()),
        );

        let status = child.resume_at(path, depth + 1, ctx, tracer)?;

        // Propagate Running; on terminal status, let the composite handle it
        // by re-ticking from the current position
        Ok(status)
    }
}
```

---

## 9. Command Template Rendering

Action nodes and `Switch` cases support template interpolation in command string fields. This uses `minijinja`:

```rust
fn render_command_templates(cmd: &DecisionCommand, bb: &Blackboard) -> Result<DecisionCommand, RuntimeError> {
    let ctx = bb.to_template_context();
    let render = |s: &str| -> Result<String, RuntimeError> {
        render_prompt_template(s, &ctx)
            .map_err(|e| RuntimeError::FilterError(e.to_string()))
    };

    match cmd {
        // --- Agent commands ---
        DecisionCommand::Agent(AgentCommand::Reflect { prompt }) => {
            Ok(DecisionCommand::Agent(AgentCommand::Reflect { prompt: render(prompt)? }))
        }
        DecisionCommand::Agent(AgentCommand::SendInstruction { prompt, target_agent }) => {
            Ok(DecisionCommand::Agent(AgentCommand::SendInstruction {
                prompt: render(prompt)?,
                target_agent: render(target_agent)?,
            }))
        }
        DecisionCommand::Agent(AgentCommand::Terminate { reason }) => {
            Ok(DecisionCommand::Agent(AgentCommand::Terminate {
                reason: render(reason)?,
            }))
        }
        DecisionCommand::Agent(AgentCommand::ApproveAndContinue) |
        DecisionCommand::Agent(AgentCommand::WakeUp) => Ok(cmd.clone()),

        // --- Git commands ---
        DecisionCommand::Git(GitCommand::Commit { message, wip }, wt) => {
            Ok(DecisionCommand::Git(GitCommand::Commit {
                message: render(message)?,
                wip: *wip,
            }, wt.clone()))
        }
        DecisionCommand::Git(GitCommand::CreateBranch { name, base }, wt) => {
            Ok(DecisionCommand::Git(GitCommand::CreateBranch {
                name: render(name)?,
                base: render(base)?,
            }, wt.clone()))
        }
        DecisionCommand::Git(GitCommand::Stash { description, include_untracked }, wt) => {
            Ok(DecisionCommand::Git(GitCommand::Stash {
                description: render(description)?,
                include_untracked: *include_untracked,
            }, wt.clone()))
        }
        DecisionCommand::Git(GitCommand::Discard, wt) => {
            Ok(DecisionCommand::Git(GitCommand::Discard, wt.clone()))
        }
        DecisionCommand::Git(GitCommand::Rebase { base }, wt) => {
            Ok(DecisionCommand::Git(GitCommand::Rebase {
                base: render(base)?,
            }, wt.clone()))
        }

        // --- Task commands ---
        DecisionCommand::Task(TaskCommand::StopIfComplete { reason }) => {
            Ok(DecisionCommand::Task(TaskCommand::StopIfComplete {
                reason: render(reason)?,
            }))
        }
        DecisionCommand::Task(TaskCommand::PrepareStart { task_id, description }) => {
            Ok(DecisionCommand::Task(TaskCommand::PrepareStart {
                task_id: render(task_id)?,
                description: render(description)?,
            }))
        }
        DecisionCommand::Task(TaskCommand::ConfirmCompletion) => Ok(cmd.clone()),

        // --- Human commands ---
        DecisionCommand::Human(HumanCommand::Escalate { reason, context }) => {
            Ok(DecisionCommand::Human(HumanCommand::Escalate {
                reason: render(reason)?,
                context: context.as_ref().map(|c| render(c)).transpose()?,
            }))
        }
        DecisionCommand::Human(HumanCommand::SelectOption { option_id }) => {
            Ok(DecisionCommand::Human(HumanCommand::SelectOption {
                option_id: render(option_id)?,
            }))
        }
        DecisionCommand::Human(HumanCommand::SkipDecision) => Ok(cmd.clone()),

        // --- Provider commands ---
        DecisionCommand::Provider(ProviderCommand::RetryTool { tool_name, args, max_attempts }) => {
            Ok(DecisionCommand::Provider(ProviderCommand::RetryTool {
                tool_name: render(tool_name)?,
                args: args.as_ref().map(|a| render(a)).transpose()?,
                max_attempts: *max_attempts,
            }))
        }
        DecisionCommand::Provider(ProviderCommand::SwitchProvider { provider_type }) => {
            Ok(DecisionCommand::Provider(ProviderCommand::SwitchProvider {
                provider_type: render(provider_type)?,
            }))
        }
        DecisionCommand::Provider(ProviderCommand::SuggestCommit { message, mandatory, reason }) => {
            Ok(DecisionCommand::Provider(ProviderCommand::SuggestCommit {
                message: render(message)?,
                mandatory: *mandatory,
                reason: render(reason)?,
            }))
        }
        DecisionCommand::Provider(ProviderCommand::PreparePr { title, description, base, draft }) => {
            Ok(DecisionCommand::Provider(ProviderCommand::PreparePr {
                title: render(title)?,
                description: render(description)?,
                base: render(base)?,
                draft: *draft,
            }))
        }
    }
}
```

---

*Document version: 2.0*
*Last updated: 2026-04-24*
