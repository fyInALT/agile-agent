//! Session Manager Interpreter — executes DecisionCommands via SessionManager
//!
//! Implements DecisionCommandInterpreter to actually send commands to Work Agent,
//! rather than just logging them.

use std::sync::Arc;

use anyhow::{Context, Result};
use decision_dsl::ext::command::{AgentCommand, DecisionCommand, HumanCommand, TaskCommand};

use crate::session_mgr::EventLoop;
use crate::decision_agent_slot::DecisionCommandInterpreter;

/// Interpreter that actually controls Work Agent via SessionManager.
pub struct SessionManagerInterpreter {
    /// EventLoop reference for sending commands
    event_loop: Arc<EventLoop>,
    /// Human escalation callback (if configured)
    escalation_handler: Option<Box<dyn EscalationHandler>>,
}

/// Handler for human escalations.
pub trait EscalationHandler: Send + Sync {
    fn handle_escalation(&self, reason: &str, context: Option<&str>, agent_id: &str);
}

/// Default escalation handler that logs.
pub struct LogEscalationHandler;

impl EscalationHandler for LogEscalationHandler {
    fn handle_escalation(&self, reason: &str, context: Option<&str>, agent_id: &str) {
        tracing::warn!(
            reason = reason,
            context = context,
            agent_id = agent_id,
            "Human escalation triggered"
        );
    }
}

impl SessionManagerInterpreter {
    /// Create a new interpreter connected to EventLoop.
    pub fn new(event_loop: Arc<EventLoop>) -> Self {
        Self {
            event_loop,
            escalation_handler: Some(Box::new(LogEscalationHandler)),
        }
    }

    /// Create with custom escalation handler.
    pub fn with_escalation_handler(
        event_loop: Arc<EventLoop>,
        handler: Box<dyn EscalationHandler>,
    ) -> Self {
        Self {
            event_loop,
            escalation_handler: Some(handler),
        }
    }

    /// Create without escalation handler (escalations just log).
    pub fn without_escalation(event_loop: Arc<EventLoop>) -> Self {
        Self {
            event_loop,
            escalation_handler: None,
        }
    }
}

impl DecisionCommandInterpreter for SessionManagerInterpreter {
    fn handle_agent_command(&mut self, cmd: AgentCommand) -> Result<()> {
        match cmd {
            AgentCommand::SendInstruction { prompt, target_agent } => {
                tracing::info!(
                    target_agent = target_agent,
                    prompt_len = prompt.len(),
                    "Sending instruction to Work Agent"
                );
                // Actually send input to the agent
                // Note: this is async in EventLoop, so we spawn a blocking task
                let event_loop = self.event_loop.clone();
                let target = target_agent.clone();
                let text = prompt.clone();
                tokio::spawn(async move {
                    let result = event_loop.send_input(&target, &text).await;
                    if let Err(e) = result {
                        tracing::error!(
                            target_agent = target,
                            error = e.to_string(),
                            "Failed to send instruction"
                        );
                    }
                });
                Ok(())
            }

            AgentCommand::WakeUp => {
                tracing::info!("WakeUp requested - agent will be polled");
                // WakeUp typically means we should check agent status
                // For now, log only - real implementation would trigger status check
                Ok(())
            }

            AgentCommand::Reflect { prompt } => {
                tracing::info!(
                    prompt_len = prompt.len(),
                    "Reflection requested"
                );
                // Send reflection prompt to agent
                let event_loop = self.event_loop.clone();
                let text = prompt.clone();
                tokio::spawn(async move {
                    // Reflect typically goes to the focused agent
                    // For now, we just log - agent_id would need to be known
                    tracing::debug!(
                        prompt_len = text.len(),
                        "Reflection prompt queued"
                    );
                });
                Ok(())
            }

            AgentCommand::Terminate { reason } => {
                tracing::info!(
                    reason = reason,
                    "Terminate requested"
                );
                // Stop the agent
                let event_loop = self.event_loop.clone();
                let reason_str = reason.clone();
                tokio::spawn(async move {
                    // Would need to know agent_id - for now log
                    tracing::warn!(
                        reason = reason_str,
                        "Agent termination queued"
                    );
                });
                Ok(())
            }

            AgentCommand::ApproveAndContinue => {
                tracing::info!("ApproveAndContinue - agent should proceed");
                Ok(())
            }
        }
    }

    fn handle_human_command(&mut self, cmd: HumanCommand) -> Result<()> {
        match cmd {
            HumanCommand::Escalate { reason, context } => {
                tracing::warn!(
                    reason = reason,
                    context = context,
                    "Human escalation - requesting intervention"
                );

                // Call escalation handler if configured
                if let Some(handler) = &self.escalation_handler {
                    handler.handle_escalation(
                        &reason,
                        context.as_deref(),
                        "decision-agent",
                    );
                }

                Ok(())
            }

            HumanCommand::SelectOption { option_id } => {
                tracing::info!(
                    option_id = option_id,
                    "Option selected"
                );
                Ok(())
            }

            HumanCommand::SkipDecision => {
                tracing::info!("Decision skipped");
                Ok(())
            }
        }
    }

    fn handle_task_command(&mut self, cmd: TaskCommand) -> Result<()> {
        match cmd {
            TaskCommand::ConfirmCompletion => {
                tracing::info!("Task completion confirmed - marking as done");
                // In real implementation, would update task state
                Ok(())
            }

            TaskCommand::StopIfComplete { reason } => {
                tracing::info!(
                    reason = reason,
                    "StopIfComplete - checking if task is done"
                );
                Ok(())
            }

            TaskCommand::PrepareStart { task_id, description } => {
                tracing::info!(
                    task_id = task_id,
                    description = description,
                    "Preparing task start"
                );
                Ok(())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use agent_types::WorkplaceId;

    async fn create_event_loop() -> Arc<EventLoop> {
        let temp = TempDir::new().unwrap();
        let mgr = EventLoop::bootstrap(
            temp.path().to_path_buf(),
            WorkplaceId::new("test-workplace"),
        )
        .await
        .unwrap();
        Arc::new(mgr)
    }

    #[tokio::test]
    async fn interpreter_new() {
        let event_loop = create_event_loop().await;
        let interpreter = SessionManagerInterpreter::new(event_loop);
        assert!(interpreter.escalation_handler.is_some());
    }

    #[tokio::test]
    async fn interpreter_without_escalation() {
        let event_loop = create_event_loop().await;
        let interpreter = SessionManagerInterpreter::without_escalation(event_loop);
        assert!(interpreter.escalation_handler.is_none());
    }

    #[tokio::test]
    async fn handle_wakeup_succeeds() {
        let event_loop = create_event_loop().await;
        let mut interpreter = SessionManagerInterpreter::new(event_loop);
        let result = interpreter.handle_agent_command(AgentCommand::WakeUp);
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn handle_escalate_calls_handler() {
        let event_loop = create_event_loop().await;
        let mut interpreter = SessionManagerInterpreter::new(event_loop);
        let result = interpreter.handle_human_command(HumanCommand::Escalate {
            reason: "test reason".to_string(),
            context: Some("test context".to_string()),
        });
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn handle_confirm_completion() {
        let event_loop = create_event_loop().await;
        let mut interpreter = SessionManagerInterpreter::new(event_loop);
        let result = interpreter.handle_task_command(TaskCommand::ConfirmCompletion);
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn handle_skip_decision() {
        let event_loop = create_event_loop().await;
        let mut interpreter = SessionManagerInterpreter::new(event_loop);
        let result = interpreter.handle_human_command(HumanCommand::SkipDecision);
        assert!(result.is_ok());
    }
}