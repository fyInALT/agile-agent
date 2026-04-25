use std::time::Duration;

use crate::ast::node::{
    ActionNode, ConditionNode, CooldownNode, ForceHumanNode, InverterNode, Node, NodeStatus,
    ParallelNode, ParallelPolicy, PromptNode, RepeaterNode, ReflectionGuardNode, SelectorNode,
    SequenceNode, SetVarNode, SubTreeNode, WhenNode,
};
use crate::ast::template::{render_command_templates, BlackboardExt};
use crate::ext::blackboard::Blackboard;
use crate::ext::command::{DecisionCommand, HumanCommand};
use crate::ext::error::RuntimeError;
use crate::ext::traits::{Clock, Logger, Session};

// ── TraceEntry ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub enum TraceEntry {
    Enter { name: String, child_index: usize },
    Exit { name: String, child_index: usize, status: NodeStatus },
    Action { name: String, command: DecisionCommand },
    PromptSend { name: String, template: String },
    PromptReceive { name: String, reply: String },
    PromptFailure { name: String, reason: String },
    EnterSubTree { name: String, ref_name: String },
    ExitSubTree { name: String, ref_name: String, status: NodeStatus },
}

// ── Tracer ──────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Default)]
pub struct Tracer {
    entries: Vec<TraceEntry>,
    path_stack: Vec<usize>,
    running_path: Vec<usize>,
}

impl Tracer {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn enter(&mut self, name: &str, child_index: usize) {
        self.path_stack.push(child_index);
        self.entries.push(TraceEntry::Enter {
            name: name.into(),
            child_index,
        });
    }

    pub fn exit(&mut self, name: &str, child_index: usize, status: NodeStatus) {
        self.path_stack.pop();
        self.entries.push(TraceEntry::Exit {
            name: name.into(),
            child_index,
            status,
        });
        if status == NodeStatus::Running {
            self.running_path = self.path_stack.clone();
        }
    }

    pub fn record_action(&mut self, name: &str, command: &DecisionCommand) {
        self.entries.push(TraceEntry::Action {
            name: name.into(),
            command: command.clone(),
        });
    }

    pub fn record_prompt_send(&mut self, name: &str, template: &str) {
        self.entries.push(TraceEntry::PromptSend {
            name: name.into(),
            template: template.into(),
        });
    }

    pub fn record_prompt_receive(&mut self, name: &str, reply: &str) {
        self.entries.push(TraceEntry::PromptReceive {
            name: name.into(),
            reply: reply.into(),
        });
    }

    pub fn record_prompt_failure(&mut self, name: &str, reason: &str) {
        self.entries.push(TraceEntry::PromptFailure {
            name: name.into(),
            reason: reason.into(),
        });
    }

    pub fn enter_subtree(&mut self, name: &str, ref_name: &str) {
        self.entries.push(TraceEntry::EnterSubTree {
            name: name.into(),
            ref_name: ref_name.into(),
        });
    }

    pub fn exit_subtree(&mut self, name: &str, ref_name: &str, status: NodeStatus) {
        self.entries.push(TraceEntry::ExitSubTree {
            name: name.into(),
            ref_name: ref_name.into(),
            status,
        });
    }

    pub fn running_path(&self) -> &[usize] {
        &self.running_path
    }

    pub fn entries(&self) -> &[TraceEntry] {
        &self.entries
    }

    pub fn into_entries(self) -> Vec<TraceEntry> {
        self.entries
    }
}

// ── TickContext ─────────────────────────────────────────────────────────────

pub struct TickContext<'a> {
    pub blackboard: &'a mut Blackboard,
    pub session: &'a mut dyn Session,
    pub clock: &'a dyn Clock,
    pub logger: &'a dyn Logger,
}

// ── TickResult ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub struct TickResult {
    pub status: NodeStatus,
    pub commands: Vec<DecisionCommand>,
    pub trace: Vec<TraceEntry>,
}

// ── DslRunner trait ─────────────────────────────────────────────────────────

pub trait DslRunner {
    fn tick(
        &mut self,
        tree: &mut crate::ast::document::Tree,
        ctx: &mut TickContext,
    ) -> Result<TickResult, RuntimeError>;
    fn reset(&mut self);
}

// ── Executor ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Default)]
pub struct Executor {
    running_path: Vec<usize>,
    is_running: bool,
}

impl Executor {
    pub fn new() -> Self {
        Self::default()
    }
}

impl DslRunner for Executor {
    fn tick(
        &mut self,
        tree: &mut crate::ast::document::Tree,
        ctx: &mut TickContext,
    ) -> Result<TickResult, RuntimeError> {
        let mut tracer = Tracer::new();

        let status = if self.is_running {
            tree.spec
                .root
                .resume_at(&self.running_path, 0, ctx, &mut tracer)?
        } else {
            tree.spec.root.tick(ctx, &mut tracer)?
        };

        match status {
            NodeStatus::Running => {
                self.is_running = true;
                self.running_path = tracer.running_path().to_vec();
            }
            _ => {
                self.is_running = false;
                self.running_path.clear();
            }
        }

        let commands = ctx.blackboard.drain_commands();
        let trace = tracer.into_entries();

        Ok(TickResult {
            status,
            commands,
            trace,
        })
    }

    fn reset(&mut self) {
        self.is_running = false;
        self.running_path.clear();
    }
}

// ── Node tick & resume_at ───────────────────────────────────────────────────

impl Node {
    pub fn tick(
        &mut self,
        ctx: &mut TickContext,
        tracer: &mut Tracer,
    ) -> Result<NodeStatus, RuntimeError> {
        match self {
            Node::Selector(n) => tick_selector(n, ctx, tracer),
            Node::Sequence(n) => tick_sequence(n, ctx, tracer),
            Node::Parallel(n) => tick_parallel(n, ctx, tracer),
            Node::Inverter(n) => tick_inverter(n, ctx, tracer),
            Node::Repeater(n) => tick_repeater(n, ctx, tracer),
            Node::Cooldown(n) => tick_cooldown(n, ctx, tracer),
            Node::ReflectionGuard(n) => tick_reflection_guard(n, ctx, tracer),
            Node::ForceHuman(n) => tick_force_human(n, ctx, tracer),
            Node::When(n) => tick_when(n, ctx, tracer),
            Node::Condition(n) => tick_condition(n, ctx, tracer),
            Node::Action(n) => tick_action(n, ctx, tracer),
            Node::Prompt(n) => tick_prompt(n, ctx, tracer),
            Node::SetVar(n) => tick_set_var(n, ctx, tracer),
            Node::SubTree(n) => tick_subtree(n, ctx, tracer),
        }
    }

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

        match self {
            Node::Selector(n) => resume_selector(n, path, depth, ctx, tracer),
            Node::Sequence(n) => resume_sequence(n, path, depth, ctx, tracer),
            Node::Parallel(n) => resume_parallel(n, path, depth, ctx, tracer),
            Node::Inverter(n) => resume_inverter(n, path, depth, ctx, tracer),
            Node::Repeater(n) => resume_repeater(n, path, depth, ctx, tracer),
            Node::Cooldown(n) => resume_cooldown(n, path, depth, ctx, tracer),
            Node::ReflectionGuard(n) => resume_reflection_guard(n, path, depth, ctx, tracer),
            Node::ForceHuman(n) => resume_force_human(n, path, depth, ctx, tracer),
            Node::When(n) => resume_when(n, path, depth, ctx, tracer),
            Node::Condition(_) => self.tick(ctx, tracer),
            Node::Action(_) => self.tick(ctx, tracer),
            Node::Prompt(n) => resume_prompt(n, path, depth, ctx, tracer),
            Node::SetVar(_) => self.tick(ctx, tracer),
            Node::SubTree(n) => resume_subtree(n, path, depth, ctx, tracer),
        }
    }
}

// ── Composite: Selector ─────────────────────────────────────────────────────

fn tick_selector(
    node: &mut SelectorNode,
    ctx: &mut TickContext,
    tracer: &mut Tracer,
) -> Result<NodeStatus, RuntimeError> {
    let start = node.active_child.unwrap_or(0);
    for i in start..node.children.len() {
        tracer.enter(&node.name, i);
        let status = node.children[i].tick(ctx, tracer)?;
        tracer.exit(&node.name, i, status);
        match status {
            NodeStatus::Success => {
                node.active_child = None;
                return Ok(NodeStatus::Success);
            }
            NodeStatus::Running => {
                node.active_child = Some(i);
                return Ok(NodeStatus::Running);
            }
            NodeStatus::Failure => continue,
        }
    }
    node.active_child = None;
    Ok(NodeStatus::Failure)
}

fn resume_selector(
    node: &mut SelectorNode,
    path: &[usize],
    depth: usize,
    ctx: &mut TickContext,
    tracer: &mut Tracer,
) -> Result<NodeStatus, RuntimeError> {
    let child_idx = path[depth];
    tracer.enter(&node.name, child_idx);
    let status = if depth + 1 >= path.len() {
        node.children[child_idx].tick(ctx, tracer)?
    } else {
        node.children[child_idx].resume_at(path, depth + 1, ctx, tracer)?
    };
    tracer.exit(&node.name, child_idx, status);
    match status {
        NodeStatus::Success => {
            node.active_child = None;
            Ok(NodeStatus::Success)
        }
        NodeStatus::Running => {
            node.active_child = Some(child_idx);

            Ok(NodeStatus::Running)
        }
        NodeStatus::Failure => {
            // Continue with next siblings
            for i in (child_idx + 1)..node.children.len() {
                tracer.enter(&node.name, i);
                let s = node.children[i].tick(ctx, tracer)?;
                tracer.exit(&node.name, i, s);
                match s {
                    NodeStatus::Success => {
                        node.active_child = None;
                        return Ok(NodeStatus::Success);
                    }
                    NodeStatus::Running => {
                        node.active_child = Some(i);
            
                        return Ok(NodeStatus::Running);
                    }
                    NodeStatus::Failure => continue,
                }
            }
            node.active_child = None;
            Ok(NodeStatus::Failure)
        }
    }
}

// ── Composite: Sequence ─────────────────────────────────────────────────────

fn tick_sequence(
    node: &mut SequenceNode,
    ctx: &mut TickContext,
    tracer: &mut Tracer,
) -> Result<NodeStatus, RuntimeError> {
    let start = node.active_child.unwrap_or(0);
    for i in start..node.children.len() {
        tracer.enter(&node.name, i);
        let status = node.children[i].tick(ctx, tracer)?;
        tracer.exit(&node.name, i, status);
        match status {
            NodeStatus::Success => continue,
            NodeStatus::Running => {
                node.active_child = Some(i);
                return Ok(NodeStatus::Running);
            }
            NodeStatus::Failure => {
                node.active_child = None;
                return Ok(NodeStatus::Failure);
            }
        }
    }
    node.active_child = None;
    Ok(NodeStatus::Success)
}

fn resume_sequence(
    node: &mut SequenceNode,
    path: &[usize],
    depth: usize,
    ctx: &mut TickContext,
    tracer: &mut Tracer,
) -> Result<NodeStatus, RuntimeError> {
    let child_idx = path[depth];
    tracer.enter(&node.name, child_idx);
    let status = if depth + 1 >= path.len() {
        node.children[child_idx].tick(ctx, tracer)?
    } else {
        node.children[child_idx].resume_at(path, depth + 1, ctx, tracer)?
    };
    tracer.exit(&node.name, child_idx, status);
    match status {
        NodeStatus::Success => {
            // Continue with next siblings
            for i in (child_idx + 1)..node.children.len() {
                tracer.enter(&node.name, i);
                let s = node.children[i].tick(ctx, tracer)?;
                tracer.exit(&node.name, i, s);
                match s {
                    NodeStatus::Success => continue,
                    NodeStatus::Running => {
                        node.active_child = Some(i);
            
                        return Ok(NodeStatus::Running);
                    }
                    NodeStatus::Failure => {
                        node.active_child = None;
                        return Ok(NodeStatus::Failure);
                    }
                }
            }
            node.active_child = None;
            Ok(NodeStatus::Success)
        }
        NodeStatus::Running => {
            node.active_child = Some(child_idx);

            Ok(NodeStatus::Running)
        }
        NodeStatus::Failure => {
            node.active_child = None;
            Ok(NodeStatus::Failure)
        }
    }
}

// ── Composite: Parallel ─────────────────────────────────────────────────────

fn tick_parallel(
    node: &mut ParallelNode,
    ctx: &mut TickContext,
    tracer: &mut Tracer,
) -> Result<NodeStatus, RuntimeError> {
    let mut successes = 0;
    let mut failures = 0;
    let total = node.children.len();

    for (i, child) in node.children.iter_mut().enumerate() {
        tracer.enter(&node.name, i);
        let status = child.tick(ctx, tracer)?;
        tracer.exit(&node.name, i, status);
        match status {
            NodeStatus::Success => successes += 1,
            NodeStatus::Failure => failures += 1,
            NodeStatus::Running => {}
        }
    }

    match node.policy {
        ParallelPolicy::AllSuccess => {
            if successes == total {
                Ok(NodeStatus::Success)
            } else if failures > 0 {
                Ok(NodeStatus::Failure)
            } else {
                Ok(NodeStatus::Running)
            }
        }
        ParallelPolicy::AnySuccess => {
            if successes > 0 {
                Ok(NodeStatus::Success)
            } else if failures == total {
                Ok(NodeStatus::Failure)
            } else {
                Ok(NodeStatus::Running)
            }
        }
        ParallelPolicy::Majority => {
            let threshold = total / 2 + 1;
            if successes >= threshold {
                Ok(NodeStatus::Success)
            } else if failures > total / 2 {
                Ok(NodeStatus::Failure)
            } else {
                Ok(NodeStatus::Running)
            }
        }
    }
}

fn resume_parallel(
    _node: &mut ParallelNode,
    _path: &[usize],
    _depth: usize,
    ctx: &mut TickContext,
    tracer: &mut Tracer,
) -> Result<NodeStatus, RuntimeError> {
    // Parallel doesn't store running state per child, so we just re-tick all children
    // For simplicity in resume, we re-run the whole parallel node
    // This is acceptable because parallel children are independent
    let mut successes = 0;
    let mut failures = 0;
    let total = _node.children.len();

    for (i, child) in _node.children.iter_mut().enumerate() {
        tracer.enter(&_node.name, i);
        let status = child.tick(ctx, tracer)?;
        tracer.exit(&_node.name, i, status);
        match status {
            NodeStatus::Success => successes += 1,
            NodeStatus::Failure => failures += 1,
            NodeStatus::Running => {}
        }
    }

    match _node.policy {
        ParallelPolicy::AllSuccess => {
            if successes == total {
                Ok(NodeStatus::Success)
            } else if failures > 0 {
                Ok(NodeStatus::Failure)
            } else {
                Ok(NodeStatus::Running)
            }
        }
        ParallelPolicy::AnySuccess => {
            if successes > 0 {
                Ok(NodeStatus::Success)
            } else if failures == total {
                Ok(NodeStatus::Failure)
            } else {
                Ok(NodeStatus::Running)
            }
        }
        ParallelPolicy::Majority => {
            let threshold = total / 2 + 1;
            if successes >= threshold {
                Ok(NodeStatus::Success)
            } else if failures > total / 2 {
                Ok(NodeStatus::Failure)
            } else {
                Ok(NodeStatus::Running)
            }
        }
    }
}

// ── Decorator: Inverter ─────────────────────────────────────────────────────

fn tick_inverter(
    node: &mut InverterNode,
    ctx: &mut TickContext,
    tracer: &mut Tracer,
) -> Result<NodeStatus, RuntimeError> {
    let status = node.child.tick(ctx, tracer)?;
    match status {
        NodeStatus::Success => Ok(NodeStatus::Failure),
        NodeStatus::Failure => Ok(NodeStatus::Success),
        NodeStatus::Running => Ok(NodeStatus::Running),
    }
}

fn resume_inverter(
    node: &mut InverterNode,
    path: &[usize],
    depth: usize,
    ctx: &mut TickContext,
    tracer: &mut Tracer,
) -> Result<NodeStatus, RuntimeError> {
    let status = node.child.resume_at(path, depth, ctx, tracer)?;
    match status {
        NodeStatus::Success => Ok(NodeStatus::Failure),
        NodeStatus::Failure => Ok(NodeStatus::Success),
        NodeStatus::Running => Ok(NodeStatus::Running),
    }
}

// ── Decorator: Repeater ─────────────────────────────────────────────────────

fn tick_repeater(
    node: &mut RepeaterNode,
    ctx: &mut TickContext,
    tracer: &mut Tracer,
) -> Result<NodeStatus, RuntimeError> {
    while node.current < node.max_attempts {
        let status = node.child.tick(ctx, tracer)?;
        match status {
            NodeStatus::Success => {
                node.current += 1;
                if node.current >= node.max_attempts {
                    node.current = 0;
                    return Ok(NodeStatus::Success);
                }
            }
            NodeStatus::Running => return Ok(NodeStatus::Running),
            NodeStatus::Failure => {
                node.current = 0;
                return Ok(NodeStatus::Failure);
            }
        }
    }
    node.current = 0;
    Ok(NodeStatus::Success)
}

fn resume_repeater(
    node: &mut RepeaterNode,
    path: &[usize],
    depth: usize,
    ctx: &mut TickContext,
    tracer: &mut Tracer,
) -> Result<NodeStatus, RuntimeError> {
    let status = node.child.resume_at(path, depth, ctx, tracer)?;
    match status {
        NodeStatus::Success => {
            node.current += 1;
            if node.current >= node.max_attempts {
                node.current = 0;
                Ok(NodeStatus::Success)
            } else {
                // Continue looping
                tick_repeater(node, ctx, tracer)
            }
        }
        NodeStatus::Running => Ok(NodeStatus::Running),
        NodeStatus::Failure => {
            node.current = 0;
            Ok(NodeStatus::Failure)
        }
    }
}

// ── Decorator: Cooldown ─────────────────────────────────────────────────────

fn tick_cooldown(
    node: &mut CooldownNode,
    ctx: &mut TickContext,
    tracer: &mut Tracer,
) -> Result<NodeStatus, RuntimeError> {
    let duration = Duration::from_millis(node.duration_ms);
    if let Some(last) = node.last_success {
        if ctx.clock.now().duration_since(last) < duration {
            return Ok(NodeStatus::Failure);
        }
    }
    let status = node.child.tick(ctx, tracer)?;
    if status == NodeStatus::Success {
        node.last_success = Some(ctx.clock.now());
    }
    Ok(status)
}

fn resume_cooldown(
    node: &mut CooldownNode,
    path: &[usize],
    depth: usize,
    ctx: &mut TickContext,
    tracer: &mut Tracer,
) -> Result<NodeStatus, RuntimeError> {
    let duration = Duration::from_millis(node.duration_ms);
    if let Some(last) = node.last_success {
        if ctx.clock.now().duration_since(last) < duration {
            return Ok(NodeStatus::Failure);
        }
    }
    let status = node.child.resume_at(path, depth, ctx, tracer)?;
    if status == NodeStatus::Success {
        node.last_success = Some(ctx.clock.now());
    }
    Ok(status)
}

// ── Decorator: ReflectionGuard ──────────────────────────────────────────────

fn tick_reflection_guard(
    node: &mut ReflectionGuardNode,
    ctx: &mut TickContext,
    tracer: &mut Tracer,
) -> Result<NodeStatus, RuntimeError> {
    if ctx.blackboard.reflection_round >= node.max_rounds {
        return Ok(NodeStatus::Failure);
    }
    let status = node.child.tick(ctx, tracer)?;
    if status == NodeStatus::Success {
        ctx.blackboard.reflection_round += 1;
    }
    Ok(status)
}

fn resume_reflection_guard(
    node: &mut ReflectionGuardNode,
    path: &[usize],
    depth: usize,
    ctx: &mut TickContext,
    tracer: &mut Tracer,
) -> Result<NodeStatus, RuntimeError> {
    if ctx.blackboard.reflection_round >= node.max_rounds {
        return Ok(NodeStatus::Failure);
    }
    let status = node.child.resume_at(path, depth, ctx, tracer)?;
    if status == NodeStatus::Success {
        ctx.blackboard.reflection_round += 1;
    }
    Ok(status)
}

// ── Decorator: ForceHuman ───────────────────────────────────────────────────

fn tick_force_human(
    node: &mut ForceHumanNode,
    ctx: &mut TickContext,
    tracer: &mut Tracer,
) -> Result<NodeStatus, RuntimeError> {
    let status = node.child.tick(ctx, tracer)?;
    if status == NodeStatus::Success {
        ctx.blackboard.push_command(DecisionCommand::Human(HumanCommand::Escalate {
            reason: node.reason.clone(),
            context: None,
        }));
    }
    Ok(status)
}

fn resume_force_human(
    node: &mut ForceHumanNode,
    path: &[usize],
    depth: usize,
    ctx: &mut TickContext,
    tracer: &mut Tracer,
) -> Result<NodeStatus, RuntimeError> {
    let status = node.child.resume_at(path, depth, ctx, tracer)?;
    if status == NodeStatus::Success {
        ctx.blackboard.push_command(DecisionCommand::Human(HumanCommand::Escalate {
            reason: node.reason.clone(),
            context: None,
        }));
    }
    Ok(status)
}

// ── High-level: When ────────────────────────────────────────────────────────

fn tick_when(
    node: &mut WhenNode,
    ctx: &mut TickContext,
    tracer: &mut Tracer,
) -> Result<NodeStatus, RuntimeError> {
    if !node.condition.evaluate(ctx.blackboard)? {
        return Ok(NodeStatus::Failure);
    }
    node.action.tick(ctx, tracer)
}

fn resume_when(
    node: &mut WhenNode,
    path: &[usize],
    depth: usize,
    ctx: &mut TickContext,
    tracer: &mut Tracer,
) -> Result<NodeStatus, RuntimeError> {
    node.action.resume_at(path, depth, ctx, tracer)
}

// ── Leaf: Condition ─────────────────────────────────────────────────────────

fn tick_condition(
    node: &ConditionNode,
    ctx: &mut TickContext,
    _tracer: &mut Tracer,
) -> Result<NodeStatus, RuntimeError> {
    if node.evaluator.evaluate(ctx.blackboard)? {
        Ok(NodeStatus::Success)
    } else {
        Ok(NodeStatus::Failure)
    }
}

// ── Leaf: Action ────────────────────────────────────────────────────────────

fn tick_action(
    node: &ActionNode,
    ctx: &mut TickContext,
    tracer: &mut Tracer,
) -> Result<NodeStatus, RuntimeError> {
    if let Some(ref evaluator) = node.when {
        if !evaluator.evaluate(ctx.blackboard)? {
            return Ok(NodeStatus::Failure);
        }
    }
    let rendered = render_command_templates(&node.command, ctx.blackboard)?;
    ctx.blackboard.push_command(rendered.clone());
    tracer.record_action(&node.name, &rendered);
    Ok(NodeStatus::Success)
}

// ── Leaf: Prompt ────────────────────────────────────────────────────────────

fn tick_prompt(
    node: &mut PromptNode,
    ctx: &mut TickContext,
    tracer: &mut Tracer,
) -> Result<NodeStatus, RuntimeError> {
    if node.pending {
        // Timeout check
        if let Some(sent_at) = node.sent_at {
            let timeout = Duration::from_millis(node.timeout_ms);
            if ctx.clock.now().duration_since(sent_at) > timeout {
                node.pending = false;
                node.sent_at = None;
                tracer.record_prompt_failure(&node.name, "timeout");
                return Ok(NodeStatus::Failure);
            }
        }

        if !ctx.session.is_ready() {
            return Ok(NodeStatus::Running);
        }

        let reply = ctx.session.receive().map_err(|e| {
            RuntimeError::Session {
                kind: e.kind,
                message: e.message,
            }
        })?;
        ctx.blackboard.store_llm_response(&node.name, &reply);
        tracer.record_prompt_receive(&node.name, &reply);

        match node.parser.parse(&reply) {
            Ok(values) => {
                if let Some(crate::ext::blackboard::BlackboardValue::Command(cmd)) =
                    values.get("__command")
                {
                    ctx.blackboard.push_command(cmd.clone());
                }
                for mapping in &node.sets {
                    if let Some(value) = values.get(&mapping.field) {
                        ctx.blackboard.set(&mapping.key, value.clone());
                    }
                }
                node.pending = false;
                node.sent_at = None;
                Ok(NodeStatus::Success)
            }
            Err(e) => {
                node.pending = false;
                node.sent_at = None;
                tracer.record_prompt_failure(&node.name, &e.to_string());
                Ok(NodeStatus::Failure)
            }
        }
    } else {
        // First tick: render template and send
        let rendered = crate::ast::template::render_prompt_template(&node.template, &ctx.blackboard.to_template_context())?;
        tracer.record_prompt_send(&node.name, &rendered);
        if let Some(ref model) = node.model {
            ctx.session
                .send_with_hint(&rendered, model)
                .map_err(|e| RuntimeError::Session {
                    kind: e.kind,
                    message: e.message,
                })?;
        } else {
            ctx.session
                .send(&rendered)
                .map_err(|e| RuntimeError::Session {
                    kind: e.kind,
                    message: e.message,
                })?;
        }
        node.pending = true;
        node.sent_at = Some(ctx.clock.now());
        Ok(NodeStatus::Running)
    }
}

fn resume_prompt(
    node: &mut PromptNode,
    _path: &[usize],
    _depth: usize,
    ctx: &mut TickContext,
    tracer: &mut Tracer,
) -> Result<NodeStatus, RuntimeError> {
    tick_prompt(node, ctx, tracer)
}

// ── Leaf: SetVar ────────────────────────────────────────────────────────────

fn tick_set_var(
    node: &SetVarNode,
    ctx: &mut TickContext,
    _tracer: &mut Tracer,
) -> Result<NodeStatus, RuntimeError> {
    ctx.blackboard.set(&node.key, node.value.clone());
    Ok(NodeStatus::Success)
}

// ── Leaf: SubTree ───────────────────────────────────────────────────────────

fn tick_subtree(
    node: &mut SubTreeNode,
    ctx: &mut TickContext,
    tracer: &mut Tracer,
) -> Result<NodeStatus, RuntimeError> {
    tracer.enter_subtree(&node.name, &node.ref_name);
    ctx.blackboard.push_scope();
    let status = match &mut node.resolved_root {
        Some(root) => root.tick(ctx, tracer)?,
        None => {
            ctx.blackboard.pop_scope();
            return Err(RuntimeError::SubTreeNotResolved {
                name: node.ref_name.clone(),
            });
        }
    };
    ctx.blackboard.pop_scope();
    tracer.exit_subtree(&node.name, &node.ref_name, status);
    Ok(status)
}

fn resume_subtree(
    node: &mut SubTreeNode,
    path: &[usize],
    depth: usize,
    ctx: &mut TickContext,
    tracer: &mut Tracer,
) -> Result<NodeStatus, RuntimeError> {
    tracer.enter_subtree(&node.name, &node.ref_name);
    ctx.blackboard.push_scope();
    let status = match &mut node.resolved_root {
        Some(root) => root.resume_at(path, depth, ctx, tracer)?,
        None => {
            ctx.blackboard.pop_scope();
            return Err(RuntimeError::SubTreeNotResolved {
                name: node.ref_name.clone(),
            });
        }
    };
    ctx.blackboard.pop_scope();
    tracer.exit_subtree(&node.name, &node.ref_name, status);
    Ok(status)
}

