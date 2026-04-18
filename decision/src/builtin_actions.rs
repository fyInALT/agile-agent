//! Built-in action implementations

use crate::action::DecisionAction;
use crate::action_registry::ActionRegistry;
use crate::types::ActionType;
use serde::{Deserialize, Serialize};

// Built-in action type getters (functions instead of const)
pub fn select_option() -> ActionType {
    ActionType::new("select_option")
}

pub fn select_first() -> ActionType {
    ActionType::new("select_first")
}

pub fn reject_all() -> ActionType {
    ActionType::new("reject_all")
}

pub fn reflect() -> ActionType {
    ActionType::new("reflect")
}

pub fn confirm_completion() -> ActionType {
    ActionType::new("confirm_completion")
}

pub fn continue_action() -> ActionType {
    ActionType::new("continue")
}

pub fn retry() -> ActionType {
    ActionType::new("retry")
}

pub fn request_human() -> ActionType {
    ActionType::new("request_human")
}

pub fn abort() -> ActionType {
    ActionType::new("abort")
}

pub fn custom_instruction() -> ActionType {
    ActionType::new("custom_instruction")
}

pub fn continue_all_tasks() -> ActionType {
    ActionType::new("continue_all_tasks")
}

pub fn stop_if_complete() -> ActionType {
    ActionType::new("stop_if_complete")
}

pub fn wake_up() -> ActionType {
    ActionType::new("wake_up")
}

/// Action: Wake up from resting state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WakeUpAction;

impl WakeUpAction {
    pub fn new() -> Self {
        Self
    }
}

impl Default for WakeUpAction {
    fn default() -> Self {
        Self
    }
}

impl DecisionAction for WakeUpAction {
    fn action_type(&self) -> ActionType {
        wake_up()
    }

    fn implementation_type(&self) -> &'static str {
        "WakeUpAction"
    }

    fn to_prompt_format(&self) -> String {
        "WakeUp".to_string()
    }

    fn serialize_params(&self) -> String {
        "{}".to_string()
    }

    fn clone_boxed(&self) -> Box<dyn DecisionAction> {
        Box::new(self.clone())
    }
}

/// Action: Select option
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct SelectOptionAction {
    #[serde(default = "default_option_id")]
    pub option_id: String,
    #[serde(default)]
    pub reason: String,
}

fn default_option_id() -> String {
    "A".to_string()
}

impl SelectOptionAction {
    pub fn new(option_id: impl Into<String>, reason: impl Into<String>) -> Self {
        Self {
            option_id: option_id.into(),
            reason: reason.into(),
        }
    }

    pub fn parse(output: &str) -> Option<Box<dyn DecisionAction>> {
        // Parse from format: "Selection: [A]\nReason: ..."
        let lines: Vec<&str> = output.lines().collect();
        if lines.len() < 2 {
            return None;
        }

        let option_line = lines.iter().find(|l| l.starts_with("Selection:"))?;
        let reason_line = lines.iter().find(|l| l.starts_with("Reason:"))?;

        let option_id = option_line
            .split(':')
            .nth(1)
            .map(|s| s.trim().replace('[', "").replace(']', ""))
            .unwrap_or_default();

        let reason = reason_line
            .split(':')
            .nth(1)
            .map(|s| s.trim())
            .unwrap_or_default()
            .to_string();

        Some(Box::new(Self::new(option_id, reason)))
    }
}

impl Default for SelectOptionAction {
    fn default() -> Self {
        Self::new("A", "Default selection")
    }
}

impl DecisionAction for SelectOptionAction {
    fn action_type(&self) -> ActionType {
        select_option()
    }

    fn implementation_type(&self) -> &'static str {
        "SelectOptionAction"
    }

    fn to_prompt_format(&self) -> String {
        "Selection: [Option ID]\nReason: [Brief explanation]".to_string()
    }

    fn serialize_params(&self) -> String {
        serde_json::to_string(&self).unwrap_or_default()
    }

    fn clone_boxed(&self) -> Box<dyn DecisionAction> {
        Box::new(self.clone())
    }
}

/// Action: Reflect
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ReflectAction {
    #[serde(default)]
    pub prompt: String,
}

impl ReflectAction {
    pub fn new(prompt: impl Into<String>) -> Self {
        Self {
            prompt: prompt.into(),
        }
    }
}

impl Default for ReflectAction {
    fn default() -> Self {
        Self::new("Please reflect on your work and verify completion.")
    }
}

impl DecisionAction for ReflectAction {
    fn action_type(&self) -> ActionType {
        reflect()
    }

    fn implementation_type(&self) -> &'static str {
        "ReflectAction"
    }

    fn to_prompt_format(&self) -> String {
        "Reflect: [Reflection prompt for verification]".to_string()
    }

    fn serialize_params(&self) -> String {
        serde_json::to_string(&self).unwrap_or_default()
    }

    fn clone_boxed(&self) -> Box<dyn DecisionAction> {
        Box::new(self.clone())
    }
}

/// Action: Confirm completion
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ConfirmCompletionAction {
    #[serde(default)]
    pub submit_pr: bool,
    #[serde(default)]
    pub next_task_id: Option<String>,
}

impl ConfirmCompletionAction {
    pub fn new(submit_pr: bool) -> Self {
        Self {
            submit_pr,
            next_task_id: None,
        }
    }

    pub fn with_next_task(self, task_id: impl Into<String>) -> Self {
        Self {
            next_task_id: Some(task_id.into()),
            ..self
        }
    }
}

impl Default for ConfirmCompletionAction {
    fn default() -> Self {
        Self::new(false)
    }
}

impl DecisionAction for ConfirmCompletionAction {
    fn action_type(&self) -> ActionType {
        confirm_completion()
    }

    fn implementation_type(&self) -> &'static str {
        "ConfirmCompletionAction"
    }

    fn to_prompt_format(&self) -> String {
        "Confirm: [yes/no]\nSubmitPR: [yes/no]".to_string()
    }

    fn serialize_params(&self) -> String {
        serde_json::to_string(&self).unwrap_or_default()
    }

    fn clone_boxed(&self) -> Box<dyn DecisionAction> {
        Box::new(self.clone())
    }
}

/// Action: Continue
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ContinueAction {
    #[serde(default)]
    pub prompt: String,
    #[serde(default)]
    pub focus_items: Vec<String>,
}

impl ContinueAction {
    pub fn new(prompt: impl Into<String>) -> Self {
        Self {
            prompt: prompt.into(),
            focus_items: Vec::new(),
        }
    }

    pub fn with_focus_items(self, items: Vec<String>) -> Self {
        Self {
            focus_items: items,
            ..self
        }
    }
}

impl Default for ContinueAction {
    fn default() -> Self {
        Self::new("Continue with the next steps.")
    }
}

impl DecisionAction for ContinueAction {
    fn action_type(&self) -> ActionType {
        continue_action()
    }

    fn implementation_type(&self) -> &'static str {
        "ContinueAction"
    }

    fn to_prompt_format(&self) -> String {
        "Continue: [Instruction to continue]\nFocus: [Items to focus on]".to_string()
    }

    fn serialize_params(&self) -> String {
        serde_json::to_string(&self).unwrap_or_default()
    }

    fn clone_boxed(&self) -> Box<dyn DecisionAction> {
        Box::new(self.clone())
    }
}

/// Action: Retry
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct RetryAction {
    #[serde(default)]
    pub prompt: String,
    #[serde(default = "default_cooldown_ms")]
    pub cooldown_ms: u64,
    #[serde(default)]
    pub adjusted: bool,
}

fn default_cooldown_ms() -> u64 {
    1000
}

impl RetryAction {
    pub fn new(prompt: impl Into<String>) -> Self {
        Self {
            prompt: prompt.into(),
            cooldown_ms: 1000,
            adjusted: false,
        }
    }

    pub fn with_cooldown(self, cooldown_ms: u64) -> Self {
        Self {
            cooldown_ms,
            ..self
        }
    }

    pub fn adjusted(self) -> Self {
        Self {
            adjusted: true,
            ..self
        }
    }
}

impl Default for RetryAction {
    fn default() -> Self {
        Self::new("Retry the previous action.")
    }
}

impl DecisionAction for RetryAction {
    fn action_type(&self) -> ActionType {
        retry()
    }

    fn implementation_type(&self) -> &'static str {
        "RetryAction"
    }

    fn to_prompt_format(&self) -> String {
        "Retry: [Retry instruction]\nCooldownMs: [milliseconds]".to_string()
    }

    fn serialize_params(&self) -> String {
        serde_json::to_string(&self).unwrap_or_default()
    }

    fn clone_boxed(&self) -> Box<dyn DecisionAction> {
        Box::new(self.clone())
    }
}

/// Action: Request human
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct RequestHumanAction {
    #[serde(default)]
    pub message: String,
}

impl RequestHumanAction {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl Default for RequestHumanAction {
    fn default() -> Self {
        Self::new("Please provide human input.")
    }
}

impl DecisionAction for RequestHumanAction {
    fn action_type(&self) -> ActionType {
        request_human()
    }

    fn implementation_type(&self) -> &'static str {
        "RequestHumanAction"
    }

    fn to_prompt_format(&self) -> String {
        "RequestHuman: [Message for human]".to_string()
    }

    fn serialize_params(&self) -> String {
        serde_json::to_string(&self).unwrap_or_default()
    }

    fn clone_boxed(&self) -> Box<dyn DecisionAction> {
        Box::new(self.clone())
    }
}

/// Action: Custom instruction
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct CustomInstructionAction {
    #[serde(default)]
    pub instruction: String,
}

impl CustomInstructionAction {
    pub fn new(instruction: impl Into<String>) -> Self {
        Self {
            instruction: instruction.into(),
        }
    }
}

impl Default for CustomInstructionAction {
    fn default() -> Self {
        Self::new("Custom instruction placeholder.")
    }
}

impl DecisionAction for CustomInstructionAction {
    fn action_type(&self) -> ActionType {
        custom_instruction()
    }

    fn implementation_type(&self) -> &'static str {
        "CustomInstructionAction"
    }

    fn to_prompt_format(&self) -> String {
        "Custom: [Free-form instruction text]".to_string()
    }

    fn serialize_params(&self) -> String {
        serde_json::to_string(&self).unwrap_or_default()
    }

    fn clone_boxed(&self) -> Box<dyn DecisionAction> {
        Box::new(self.clone())
    }
}

/// Action: Continue All Tasks
///
/// Sends instruction to agent to continue working on all pending tasks.
/// Used when decision layer determines there are still tasks to complete.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ContinueAllTasksAction {
    #[serde(default)]
    pub instruction: String,
}

impl ContinueAllTasksAction {
    pub fn new(instruction: impl Into<String>) -> Self {
        Self {
            instruction: instruction.into(),
        }
    }
}

impl Default for ContinueAllTasksAction {
    fn default() -> Self {
        Self::new("continue finish all tasks")
    }
}

impl DecisionAction for ContinueAllTasksAction {
    fn action_type(&self) -> ActionType {
        continue_all_tasks()
    }

    fn implementation_type(&self) -> &'static str {
        "ContinueAllTasksAction"
    }

    fn to_prompt_format(&self) -> String {
        "ContinueAllTasks: [Instruction to continue working]".to_string()
    }

    fn serialize_params(&self) -> String {
        serde_json::to_string(&self).unwrap_or_default()
    }

    fn clone_boxed(&self) -> Box<dyn DecisionAction> {
        Box::new(self.clone())
    }
}

/// Action: Stop If Complete
///
/// Instructs agent to stop if all tasks are confirmed complete.
/// Decision layer checks Kanban/Backlog before choosing this action.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct StopIfCompleteAction {
    #[serde(default)]
    pub reason: String,
}

impl StopIfCompleteAction {
    pub fn new(reason: impl Into<String>) -> Self {
        Self {
            reason: reason.into(),
        }
    }
}

impl Default for StopIfCompleteAction {
    fn default() -> Self {
        Self::new("All tasks complete")
    }
}

impl DecisionAction for StopIfCompleteAction {
    fn action_type(&self) -> ActionType {
        stop_if_complete()
    }

    fn implementation_type(&self) -> &'static str {
        "StopIfCompleteAction"
    }

    fn to_prompt_format(&self) -> String {
        "StopIfComplete: [Reason for stopping]".to_string()
    }

    fn serialize_params(&self) -> String {
        serde_json::to_string(&self).unwrap_or_default()
    }

    fn clone_boxed(&self) -> Box<dyn DecisionAction> {
        Box::new(self.clone())
    }
}

/// Initialize registry with built-in actions
// Helper deserializer functions
fn deserialize_select_option(params: &str) -> Option<Box<dyn DecisionAction>> {
    let action: SelectOptionAction = serde_json::from_str(params).ok()?;
    Some(Box::new(action))
}

fn deserialize_reflect(params: &str) -> Option<Box<dyn DecisionAction>> {
    let action: ReflectAction = serde_json::from_str(params).ok()?;
    Some(Box::new(action))
}

fn deserialize_confirm_completion(params: &str) -> Option<Box<dyn DecisionAction>> {
    let action: ConfirmCompletionAction = serde_json::from_str(params).ok()?;
    Some(Box::new(action))
}

fn deserialize_continue(params: &str) -> Option<Box<dyn DecisionAction>> {
    let action: ContinueAction = serde_json::from_str(params).ok()?;
    Some(Box::new(action))
}

fn deserialize_retry(params: &str) -> Option<Box<dyn DecisionAction>> {
    let action: RetryAction = serde_json::from_str(params).ok()?;
    Some(Box::new(action))
}

fn deserialize_request_human(params: &str) -> Option<Box<dyn DecisionAction>> {
    let action: RequestHumanAction = serde_json::from_str(params).ok()?;
    Some(Box::new(action))
}

fn deserialize_custom_instruction(params: &str) -> Option<Box<dyn DecisionAction>> {
    let action: CustomInstructionAction = serde_json::from_str(params).ok()?;
    Some(Box::new(action))
}

fn deserialize_continue_all_tasks(params: &str) -> Option<Box<dyn DecisionAction>> {
    let action: ContinueAllTasksAction = serde_json::from_str(params).ok()?;
    Some(Box::new(action))
}

fn deserialize_stop_if_complete(params: &str) -> Option<Box<dyn DecisionAction>> {
    let action: StopIfCompleteAction = serde_json::from_str(params).ok()?;
    Some(Box::new(action))
}

pub fn register_action_builtins(registry: &ActionRegistry) {
    registry.register(Box::new(SelectOptionAction::default()));
    registry.register(Box::new(ReflectAction::default()));
    registry.register(Box::new(ConfirmCompletionAction::default()));
    registry.register(Box::new(ContinueAction::default()));
    registry.register(Box::new(RetryAction::default()));
    registry.register(Box::new(RequestHumanAction::default()));
    registry.register(Box::new(CustomInstructionAction::default()));
    registry.register(Box::new(ContinueAllTasksAction::default()));
    registry.register(Box::new(StopIfCompleteAction::default()));

    // Register parsers
    registry.register_parser(select_option(), SelectOptionAction::parse);

    // Register deserializers
    registry.register_deserializer(select_option(), deserialize_select_option);
    registry.register_deserializer(reflect(), deserialize_reflect);
    registry.register_deserializer(confirm_completion(), deserialize_confirm_completion);
    registry.register_deserializer(continue_action(), deserialize_continue);
    registry.register_deserializer(retry(), deserialize_retry);
    registry.register_deserializer(request_human(), deserialize_request_human);
    registry.register_deserializer(custom_instruction(), deserialize_custom_instruction);
    registry.register_deserializer(continue_all_tasks(), deserialize_continue_all_tasks);
    registry.register_deserializer(stop_if_complete(), deserialize_stop_if_complete);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_select_option_action_type() {
        let action = SelectOptionAction::new("A", "test");
        assert_eq!(action.action_type(), select_option());
    }

    #[test]
    fn test_select_option_action_params() {
        let action = SelectOptionAction::new("A", "reason");
        assert_eq!(action.option_id, "A");
        assert_eq!(action.reason, "reason");
    }

    #[test]
    fn test_select_option_action_serialize() {
        let action = SelectOptionAction::new("A", "reason");
        let params = action.serialize_params();
        assert!(params.contains("A"));
        assert!(params.contains("reason"));
    }

    #[test]
    fn test_select_option_parse() {
        let output = "Selection: [B]\nReason: Best option";
        let parsed = SelectOptionAction::parse(output);
        assert!(parsed.is_some());
        let action = parsed.unwrap();
        assert_eq!(action.action_type(), select_option());
    }

    #[test]
    fn test_reflect_action_type() {
        let action = ReflectAction::new("test prompt");
        assert_eq!(action.action_type(), reflect());
    }

    #[test]
    fn test_reflect_action_prompt() {
        let action = ReflectAction::new("Please verify");
        assert_eq!(action.prompt, "Please verify");
    }

    #[test]
    fn test_confirm_completion_action_type() {
        let action = ConfirmCompletionAction::new(true);
        assert_eq!(action.action_type(), confirm_completion());
    }

    #[test]
    fn test_confirm_completion_submit_pr() {
        let action = ConfirmCompletionAction::new(true);
        assert!(action.submit_pr);
    }

    #[test]
    fn test_confirm_completion_with_next_task() {
        let action = ConfirmCompletionAction::new(false).with_next_task("task-123");
        assert_eq!(action.next_task_id, Some("task-123".to_string()));
    }

    #[test]
    fn test_continue_action_type() {
        let action = ContinueAction::new("keep going");
        assert_eq!(action.action_type(), continue_action());
    }

    #[test]
    fn test_continue_action_with_focus() {
        let action = ContinueAction::new("keep going")
            .with_focus_items(vec!["item1".to_string(), "item2".to_string()]);
        assert_eq!(action.focus_items.len(), 2);
    }

    #[test]
    fn test_retry_action_type() {
        let action = RetryAction::new("retry");
        assert_eq!(action.action_type(), retry());
    }

    #[test]
    fn test_retry_action_cooldown() {
        let action = RetryAction::new("retry").with_cooldown(2000);
        assert_eq!(action.cooldown_ms, 2000);
    }

    #[test]
    fn test_retry_action_adjusted() {
        let action = RetryAction::new("retry").adjusted();
        assert!(action.adjusted);
    }

    #[test]
    fn test_request_human_action_type() {
        let action = RequestHumanAction::new("help needed");
        assert_eq!(action.action_type(), request_human());
    }

    #[test]
    fn test_custom_instruction_action_type() {
        let action = CustomInstructionAction::new("do this");
        assert_eq!(action.action_type(), custom_instruction());
    }

    #[test]
    fn test_register_builtins() {
        let registry = ActionRegistry::new();
        register_action_builtins(&registry);

        assert!(registry.is_registered(&select_option()));
        assert!(registry.is_registered(&reflect()));
        assert!(registry.is_registered(&confirm_completion()));
        assert!(registry.is_registered(&continue_action()));
        assert!(registry.is_registered(&retry()));
        assert!(registry.is_registered(&request_human()));
        assert!(registry.is_registered(&custom_instruction()));
    }

    #[test]
    fn test_action_serde() {
        let action = SelectOptionAction::new("A", "test reason");
        let json = serde_json::to_string(&action).unwrap();
        let parsed: SelectOptionAction = serde_json::from_str(&json).unwrap();
        assert_eq!(action.option_id, parsed.option_id);
        assert_eq!(action.reason, parsed.reason);
    }

    #[test]
    fn test_prompt_format() {
        let action = SelectOptionAction::default();
        let format = action.to_prompt_format();
        assert!(format.contains("Selection:"));
        assert!(format.contains("Reason:"));
    }

    #[test]
    fn test_action_type_getters() {
        assert_eq!(select_option().name, "select_option");
        assert_eq!(reflect().name, "reflect");
        assert_eq!(confirm_completion().name, "confirm_completion");
        assert_eq!(wake_up().name, "wake_up");
    }

    #[test]
    fn test_wake_up_action_type() {
        let action = WakeUpAction::new();
        assert_eq!(action.action_type(), wake_up());
    }

    #[test]
    fn test_wake_up_action_impl() {
        let action = WakeUpAction;
        assert_eq!(action.implementation_type(), "WakeUpAction");
    }
}
