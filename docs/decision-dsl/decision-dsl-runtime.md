# Decision DSL: Runtime Execution

> Runtime execution specification for the decision DSL engine. Covers the Executor tick loop, composite/decorator/leaf node implementations, and the Prompt node — the async leaf that communicates with the ongoing LLM session.
>
> This document is a chapter of the [Decision DSL Implementation](decision-dsl-implementation.md).

## Executor / Tick Loop

###1 Composite Tick Implementation

```rust
impl SelectorNode {
    fn tick(&mut self, ctx: &mut TickContext, tracer: &mut Tracer) -> Result<NodeStatus, RuntimeError> {
        let start = self.active_child.unwrap_or(0);

        for i in start..self.children.len() {
            tracer.enter(self.name(), i);
            let t0 = ctx.clock.now();
            let status = self.children[i].tick(ctx, tracer)?;
            let duration = t0.elapsed();
            tracer.exit(self.name(), i, status, duration, i);

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
}

impl SequenceNode {
    fn tick(&mut self, ctx: &mut TickContext, tracer: &mut Tracer) -> Result<NodeStatus, RuntimeError> {
        let start = self.active_child.unwrap_or(0);

        for i in start..self.children.len() {
            tracer.enter(self.name(), i);
            let t0 = ctx.clock.now();
            let status = self.children[i].tick(ctx, tracer)?;
            let duration = t0.elapsed();
            tracer.exit(self.name(), i, status, duration, i);

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
}

impl ParallelNode {
    fn tick(&mut self, ctx: &mut TickContext, tracer: &mut Tracer) -> Result<NodeStatus, RuntimeError> {
        let mut successes = 0;
        let mut failures = 0;

        for (i, child) in self.children.iter_mut().enumerate() {
            tracer.enter(self.name(), i);
            let t0 = ctx.clock.now();
            let status = child.tick(ctx, tracer)?;
            let duration = t0.elapsed();
            tracer.exit(self.name(), i, status, duration, i);

            match status {
                NodeStatus::Success => successes += 1,
                NodeStatus::Failure => failures += 1,
                NodeStatus::Running => return Ok(NodeStatus::Running),
            }
        }

        let total = self.children.len();
        let result = match self.policy {
            ParallelPolicy::AllSuccess => {
                if successes == total { NodeStatus::Success } else { NodeStatus::Failure }
            }
            ParallelPolicy::AnySuccess => {
                if successes > 0 { NodeStatus::Success } else { NodeStatus::Failure }
            }
            ParallelPolicy::Majority => {
                if successes > total / 2 { NodeStatus::Success } else { NodeStatus::Failure }
            }
        };
        Ok(result)
    }

    fn reset(&mut self) {
        for child in &mut self.children { child.reset(); }
    }
}
```

###2 Decorator Tick Implementation

```rust
impl InverterNode {
    fn tick(&mut self, ctx: &mut TickContext, tracer: &mut Tracer) -> Result<NodeStatus, RuntimeError> {
        let status = self.child.tick(ctx, tracer)?;
        Ok(match status {
            NodeStatus::Success => NodeStatus::Failure,
            NodeStatus::Failure => NodeStatus::Success,
            NodeStatus::Running => NodeStatus::Running,
        })
    }
    fn reset(&mut self) { self.child.reset(); }
}

impl RepeaterNode {
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
}

impl CooldownNode {
    fn tick(&mut self, ctx: &mut TickContext, tracer: &mut Tracer) -> Result<NodeStatus, RuntimeError> {
        if let Some(last) = self.last_success {
            if ctx.clock.now().duration_since(last) < self.duration {
                ctx.logger.log(LogLevel::Debug, "Cooldown",
                    &format!("{}: still on cooldown", self.name));
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
}

impl ReflectionGuardNode {
    fn tick(&mut self, ctx: &mut TickContext, tracer: &mut Tracer) -> Result<NodeStatus, RuntimeError> {
        let count = ctx.blackboard.get_u8("reflection_round").unwrap_or(0);
        if count >= self.max_rounds {
            ctx.logger.log(LogLevel::Info, "ReflectionGuard",
                &format!("{}: max rounds ({}) reached", self.name, self.max_rounds));
            return Ok(NodeStatus::Failure);
        }
        let status = self.child.tick(ctx, tracer)?;
        if status == NodeStatus::Success {
            ctx.blackboard.set_u8("reflection_round", count + 1);
        }
        Ok(status)
    }
    fn reset(&mut self) { self.child.reset(); }
}

impl ForceHumanNode {
    fn tick(&mut self, ctx: &mut TickContext, tracer: &mut Tracer) -> Result<NodeStatus, RuntimeError> {
        let status = self.child.tick(ctx, tracer)?;
        if status == NodeStatus::Success {
            ctx.blackboard.push_command(Command::EscalateToHuman {
                reason: self.reason.clone(),
                context: Some(format!("Forced by decorator after {} succeeded", self.child.name())),
            });
        }
        Ok(status)
    }
    fn reset(&mut self) { self.child.reset(); }
}
```

###3 Leaf Tick Implementation

```rust
impl ConditionNode {
    fn tick(&mut self, ctx: &mut TickContext, tracer: &mut Tracer) -> Result<NodeStatus, RuntimeError> {
        let result = self.evaluator.evaluate(&ctx.blackboard)?;
        tracer.record_eval(self.name(), &self.evaluator, result);
        Ok(if result { NodeStatus::Success } else { NodeStatus::Failure })
    }
    fn reset(&mut self) {}
}

impl ActionNode {
    fn tick(&mut self, ctx: &mut TickContext, tracer: &mut Tracer) -> Result<NodeStatus, RuntimeError> {
        // Check precondition
        if let Some(ref evaluator) = self.when {
            if !evaluator.evaluate(&ctx.blackboard)? {
                ctx.logger.log(LogLevel::Debug, "Action",
                    &format!("{}: precondition failed", self.name));
                return Ok(NodeStatus::Failure);
            }
        }

        // Render command fields that contain templates
        let rendered_cmd = render_command_templates(&self.command, &ctx.blackboard)?;

        ctx.blackboard.push_command(rendered_cmd.clone());
        tracer.record_action(self.name(), &rendered_cmd);
        Ok(NodeStatus::Success)
    }
    fn reset(&mut self) {}
}

impl SetVarNode {
    fn tick(&mut self, ctx: &mut TickContext, _tracer: &mut Tracer) -> Result<NodeStatus, RuntimeError> {
        ctx.blackboard.set(&self.key, self.value.clone());
        Ok(NodeStatus::Success)
    }
    fn reset(&mut self) {}
}

impl SubTreeRefNode {
    fn tick(&mut self, _ctx: &mut TickContext, _tracer: &mut Tracer) -> Result<NodeStatus, RuntimeError> {
        // SubTreeRef is resolved at parse time; this should never be called.
        Err(RuntimeError::Custom("SubTreeRef not resolved".into()))
    }
    fn reset(&mut self) {}
}
```

---

## Prompt Node Implementation

The Prompt node is the most complex leaf. It implements the same-session invariant.

###1 Lifecycle

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

###2 Implementation

```rust
impl PromptNode {
    fn tick(&mut self, ctx: &mut TickContext, tracer: &mut Tracer) -> Result<NodeStatus, RuntimeError> {
        if self.pending {
            // Async continuation
            if !ctx.session.is_ready() {
                ctx.logger.log(LogLevel::Debug, "Prompt",
                    &format!("{}: waiting for reply", self.name));
                return Ok(NodeStatus::Running);
            }

            let reply = ctx.session.send("POLL")?;
            ctx.blackboard.store_llm_response(&self.name, reply.clone());

            match self.parser.parse(&reply) {
                Ok(values) => {
                    // Handle CommandParser special case
                    if let Some(BlackboardValue::String(cmd_json)) = values.get("__command") {
                        let cmd: Command = serde_json::from_str(cmd_json)
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
                    tracer.record_prompt_success(&self.name, &reply);
                    return Ok(NodeStatus::Success);
                }
                Err(e) => {
                    ctx.logger.log(LogLevel::Warn, "Prompt",
                        &format!("{}: parse error: {}", self.name, e));
                    self.pending = false;
                    tracer.record_prompt_failure(&self.name, &e.to_string());
                    return Ok(NodeStatus::Failure);
                }
            }
        }

        // First tick — render and send
        let rendered = TemplateEngine::render(&self.template, &ctx.blackboard)?;
        ctx.logger.log(LogLevel::Debug, "Prompt",
            &format!("{}: sending prompt ({} chars)", self.name, rendered.len()));

        ctx.session.send(&rendered)?;
        self.pending = true;
        tracer.record_prompt_sent(&self.name);

        Ok(NodeStatus::Running)
    }

    fn reset(&mut self) {
        self.pending = false;
    }
}
```

###3 Model Selection

The `model` field is stored but not directly used by the DSL engine. The engine passes it through to the host via the `Session` trait if needed:

```rust
// Alternative: extend Session trait with model hint
pub trait Session {
    fn send(&mut self, message: &str) -> Result<String, SessionError>;
    fn send_with_model(&mut self, message: &str, model: &str) -> Result<String, SessionError>;
    fn is_ready(&self) -> bool;
}
```

For V1, we keep `model` as metadata on the node and let the host's `Session` implementation decide whether to honor it.

---

