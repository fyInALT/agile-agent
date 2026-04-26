use std::time::Duration;

use crate::ast::node::{
    ActionNode, ConditionNode, CooldownNode, ForceHumanNode, InverterNode, Node,
    NodeStatus, ParallelNode, ParallelPolicy, PromptNode, RepeaterNode, ReflectionGuardNode,
    SelectorNode, SequenceNode, SetVarNode, SubTreeNode, WhenNode,
};
use crate::ast::template::{render_command_templates, BlackboardExt};
use crate::ext::blackboard::Blackboard;
use crate::ext::command::{DecisionCommand, HumanCommand};
use crate::ext::error::RuntimeError;
use crate::ext::traits::{Clock, Logger, Session};

// ── TraceEntry ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub enum TraceEntry {
    Enter { name: String, child_index: usize, depth: usize },
    Exit { name: String, status: NodeStatus, duration_ms: u64 },
    Eval { node_name: String, evaluator: String, result: bool },
    Action { node_name: String, command: String },
    PromptSent { node_name: String },
    PromptSuccess { node_name: String, response: String },
    PromptFailure { node_name: String, error: String },
    EnterSubTree { name: String, ref_name: String },
    ExitSubTree { name: String, ref_name: String, status: NodeStatus },
    RuleMatched { rule_name: String, priority: u32 },
    RuleSkipped { rule_name: String, reason: String },
}

// ── Tracer ──────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Default)]
pub struct Tracer {
    entries: Vec<TraceEntry>,
    path_stack: Vec<usize>,
    running_path: Vec<usize>,
    depth: usize,
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
            depth: self.depth,
        });
        self.depth += 1;
    }

    pub fn exit(&mut self, name: &str, _child_index: usize, status: NodeStatus) {
        self.depth = self.depth.saturating_sub(1);
        self.path_stack.pop();
        self.entries.push(TraceEntry::Exit {
            name: name.into(),
            status,
            duration_ms: 0,
        });
        if status == NodeStatus::Running {
            self.running_path = self.path_stack.clone();
        }
    }

    pub fn record_action(&mut self, name: &str, command: &DecisionCommand) {
        self.entries.push(TraceEntry::Action {
            node_name: name.into(),
            command: format!("{command:?}"),
        });
    }

    pub fn record_eval(&mut self, node_name: &str, evaluator: &str, result: bool) {
        self.entries.push(TraceEntry::Eval {
            node_name: node_name.into(),
            evaluator: evaluator.into(),
            result,
        });
    }

    pub fn record_prompt_sent(&mut self, node_name: &str) {
        self.entries.push(TraceEntry::PromptSent {
            node_name: node_name.into(),
        });
    }

    pub fn record_prompt_success(&mut self, node_name: &str, response: &str) {
        self.entries.push(TraceEntry::PromptSuccess {
            node_name: node_name.into(),
            response: response.into(),
        });
    }

    pub fn record_prompt_failure(&mut self, node_name: &str, error: &str) {
        self.entries.push(TraceEntry::PromptFailure {
            node_name: node_name.into(),
            error: error.into(),
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

    pub fn record_rule_matched(&mut self, rule_name: &str, priority: u32) {
        self.entries.push(TraceEntry::RuleMatched {
            rule_name: rule_name.into(),
            priority,
        });
    }

    pub fn record_rule_skipped(&mut self, rule_name: &str, reason: &str) {
        self.entries.push(TraceEntry::RuleSkipped {
            rule_name: rule_name.into(),
            reason: reason.into(),
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
                if let Some(ref rule_name) = node.rule_name {
                    if !node.matched {
                        tracer.record_rule_matched(rule_name, node.rule_priority.unwrap_or(0));
                        node.matched = true;
                    }
                    // Record skipped rules without executing them
                    for _j in (i + 1)..node.children.len() {
                        tracer.record_rule_skipped(rule_name, "lower priority rule not evaluated");
                    }
                }
                return Ok(NodeStatus::Success);
            }
            NodeStatus::Running => {
                node.active_child = Some(i);
                return Ok(NodeStatus::Running);
            }
            NodeStatus::Failure => {
                continue;
            }
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
            if let Some(ref rule_name) = node.rule_name {
                if !node.matched {
                    tracer.record_rule_matched(rule_name, node.rule_priority.unwrap_or(0));
                    node.matched = true;
                }
                // Record skipped for remaining children that were never tried
                for _j in (child_idx + 1)..node.children.len() {
                    tracer.record_rule_skipped(rule_name, "lower priority rule not evaluated");
                }
            }
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
                        if let Some(ref rule_name) = node.rule_name {
                            if !node.matched {
                                tracer.record_rule_matched(rule_name, node.rule_priority.unwrap_or(0));
                                node.matched = true;
                            }
                            for _j in (i + 1)..node.children.len() {
                                tracer.record_rule_skipped(rule_name, "lower priority rule not evaluated");
                            }
                        }
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
    // Edge case: empty children
    // - AllSuccess: Success (vacuously true - all zero children succeeded)
    // - AnySuccess: Failure (no child to succeed)
    // - Majority: Failure (no majority can be achieved)
    let total = node.children.len();
    if total == 0 {
        node.active_child = None;
        return match node.policy {
            ParallelPolicy::AllSuccess => Ok(NodeStatus::Success),
            ParallelPolicy::AnySuccess => Ok(NodeStatus::Failure),
            ParallelPolicy::Majority => Ok(NodeStatus::Failure),
        };
    }

    // In a single-threaded tick model, we tick all children each cycle.
    // For true concurrency, all children would tick simultaneously.
    // Here we tick sequentially but treat results as concurrent.
    let mut successes = 0;
    let mut failures = 0;
    let mut running = 0;

    for i in 0..total {
        tracer.enter(&node.name, i);
        let status = node.children[i].tick(ctx, tracer)?;
        tracer.exit(&node.name, i, status);
        match status {
            NodeStatus::Success => successes += 1,
            NodeStatus::Failure => failures += 1,
            NodeStatus::Running => running += 1,
        }
    }

    node.active_child = None;

    match node.policy {
        ParallelPolicy::AllSuccess => {
            if successes == total {
                Ok(NodeStatus::Success)
            } else if failures > 0 {
                // Any failure means total failure for AllSuccess policy
                Ok(NodeStatus::Failure)
            } else if running > 0 {
                Ok(NodeStatus::Running)
            } else {
                // All finished but not all success
                Ok(NodeStatus::Failure)
            }
        }
        ParallelPolicy::AnySuccess => {
            if successes > 0 {
                Ok(NodeStatus::Success)
            } else if failures == total {
                Ok(NodeStatus::Failure)
            } else if running > 0 {
                Ok(NodeStatus::Running)
            } else {
                // No success, no running, but not all failures
                Ok(NodeStatus::Failure)
            }
        }
        ParallelPolicy::Majority => {
            let threshold = total / 2 + 1;
            if successes >= threshold {
                Ok(NodeStatus::Success)
            } else if failures > total / 2 {
                Ok(NodeStatus::Failure)
            } else if running > 0 {
                Ok(NodeStatus::Running)
            } else {
                // Not enough success or failure, and nothing running
                Ok(NodeStatus::Failure)
            }
        }
    }
}

fn resume_parallel(
    node: &mut ParallelNode,
    path: &[usize],
    depth: usize,
    ctx: &mut TickContext,
    tracer: &mut Tracer,
) -> Result<NodeStatus, RuntimeError> {
    // Edge case: empty children
    let total = node.children.len();
    if total == 0 {
        node.active_child = None;
        return match node.policy {
            ParallelPolicy::AllSuccess => Ok(NodeStatus::Success),
            ParallelPolicy::AnySuccess => Ok(NodeStatus::Failure),
            ParallelPolicy::Majority => Ok(NodeStatus::Failure),
        };
    }

    // Resume by ticking all children. The path indicates which child
    // was deepest in the running state, but for concurrent semantics,
    // we tick all children.
    let mut successes = 0;
    let mut failures = 0;
    let mut running = 0;

    for i in 0..total {
        tracer.enter(&node.name, i);
        let status = if i == path.get(depth).copied().unwrap_or(0) && depth + 1 < path.len() {
            // Resume the specific child that was running
            node.children[i].resume_at(path, depth + 1, ctx, tracer)?
        } else {
            node.children[i].tick(ctx, tracer)?
        };
        tracer.exit(&node.name, i, status);
        match status {
            NodeStatus::Success => successes += 1,
            NodeStatus::Failure => failures += 1,
            NodeStatus::Running => running += 1,
        }
    }

    node.active_child = None;

    match node.policy {
        ParallelPolicy::AllSuccess => {
            if successes == total {
                Ok(NodeStatus::Success)
            } else if failures > 0 {
                Ok(NodeStatus::Failure)
            } else if running > 0 {
                Ok(NodeStatus::Running)
            } else {
                Ok(NodeStatus::Failure)
            }
        }
        ParallelPolicy::AnySuccess => {
            if successes > 0 {
                Ok(NodeStatus::Success)
            } else if failures == total {
                Ok(NodeStatus::Failure)
            } else if running > 0 {
                Ok(NodeStatus::Running)
            } else {
                Ok(NodeStatus::Failure)
            }
        }
        ParallelPolicy::Majority => {
            let threshold = total / 2 + 1;
            if successes >= threshold {
                Ok(NodeStatus::Success)
            } else if failures > total / 2 {
                Ok(NodeStatus::Failure)
            } else if running > 0 {
                Ok(NodeStatus::Running)
            } else {
                Ok(NodeStatus::Failure)
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
    // Edge case: max_attempts=0 means no attempts, immediate failure
    if node.max_attempts == 0 {
        return Ok(NodeStatus::Failure);
    }

    while node.current < node.max_attempts {
        let status = node.child.tick(ctx, tracer)?;
        match status {
            NodeStatus::Success => {
                node.current += 1;
                if node.current >= node.max_attempts {
                    node.current = 0;
                    return Ok(NodeStatus::Success);
                }
                // Continue retrying
            }
            NodeStatus::Running => return Ok(NodeStatus::Running),
            NodeStatus::Failure => {
                node.current += 1;
                if node.current >= node.max_attempts {
                    node.current = 0;
                    return Ok(NodeStatus::Failure);
                }
                // Continue retrying
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
    // Edge case: max_attempts=0 means no attempts, immediate failure
    if node.max_attempts == 0 {
        return Ok(NodeStatus::Failure);
    }

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
            node.current += 1;
            if node.current >= node.max_attempts {
                node.current = 0;
                Ok(NodeStatus::Failure)
            } else {
                // Continue retrying
                tick_repeater(node, ctx, tracer)
            }
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
    // Re-check condition on resume - if it changed, abort
    if !node.condition.evaluate(ctx.blackboard)? {
        return Ok(NodeStatus::Failure);
    }
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
        tracer.record_prompt_success(&node.name, &reply);

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
        tracer.record_prompt_sent(&node.name);
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
    ctx.blackboard.push_scope().map_err(|_| RuntimeError::ScopeDepthExceeded)?;
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
    ctx.blackboard.push_scope().map_err(|_| RuntimeError::ScopeDepthExceeded)?;
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


// ── Trace ASCII rendering ───────────────────────────────────────────────────

pub fn render_trace_ascii(entries: &[TraceEntry]) -> String {
    let mut lines = Vec::new();
    let mut depth = 0usize;

    for entry in entries {
        match entry {
            TraceEntry::Enter { name, child_index, depth: d } => {
                depth = *d;
                lines.push(format!("{:indent$}[{child_index}] Enter: {name}", "", indent = depth * 2));
            }
            TraceEntry::Exit { name, status, .. } => {
                lines.push(format!("{:indent$}Exit: {name} -> {status:?}", "", indent = depth * 2));
            }
            TraceEntry::Eval { node_name, evaluator, result } => {
                lines.push(format!("{:indent$}Eval: {node_name} ({evaluator}) = {result}", "", indent = depth * 2));
            }
            TraceEntry::Action { node_name, command } => {
                lines.push(format!("{:indent$}Action: {node_name} -> {command}", "", indent = depth * 2));
            }
            TraceEntry::PromptSent { node_name } => {
                lines.push(format!("{:indent$}Prompt sent: {node_name}", "", indent = depth * 2));
            }
            TraceEntry::PromptSuccess { node_name, response } => {
                lines.push(format!("{:indent$}Prompt success: {node_name} -> {response}", "", indent = depth * 2));
            }
            TraceEntry::PromptFailure { node_name, error } => {
                lines.push(format!("{:indent$}Prompt failure: {node_name} -> {error}", "", indent = depth * 2));
            }
            TraceEntry::EnterSubTree { name, ref_name } => {
                lines.push(format!("{:indent$}--> SubTree: {name} ({ref_name})", "", indent = depth * 2));
            }
            TraceEntry::ExitSubTree { name, ref_name, status } => {
                lines.push(format!("{:indent$}<-- SubTree: {name} ({ref_name}) -> {status:?}", "", indent = depth * 2));
            }
            TraceEntry::RuleMatched { rule_name, priority } => {
                lines.push(format!("{:indent$}Rule matched: {rule_name} (priority {priority})", "", indent = depth * 2));
            }
            TraceEntry::RuleSkipped { rule_name, reason } => {
                lines.push(format!("{:indent$}Rule skipped: {rule_name} ({reason})", "", indent = depth * 2));
            }
        }
    }

    lines.join("\n")
}
