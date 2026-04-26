#![allow(dead_code)]

//! Preset LLM response library for integration testing.
//!
//! Provides realistic output patterns mimicking Codex, Claude, and agent
//! decision-layer outputs. Each preset is a function that returns a raw
//! string suitable for `OutputParser` consumption.

use std::collections::HashMap;

/// Identifier for a preset response pattern.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Preset {
    // ── Codex-style outputs ──────────────────────────────────────────────────
    /// Simple approval text: "yes"
    CodexApprove,
    /// Simple rejection text: "no"
    CodexReject,
    /// Code block response with language tag.
    CodexCodeBlock { language: &'static str, code: &'static str },
    /// JSON tool call as Codex might emit.
    CodexJsonToolCall { tool: &'static str, args: &'static str },
    /// Natural language with thinking/reasoning tags.
    CodexThinking { content: &'static str },
    /// Plain text containing a specific keyword.
    CodexKeyword(&'static str),

    // ── Claude-style outputs ─────────────────────────────────────────────────
    /// Structured XML-like output with decision and reasoning fields.
    ClaudeStructured { decision: &'static str, reasoning: &'static str },
    /// Pure JSON object response.
    ClaudeJson { fields: HashMap<&'static str, &'static str> },
    /// Single command keyword.
    ClaudeCommand(&'static str),
    /// Escalation request with reason.
    ClaudeEscalate { reason: &'static str },
    /// Reflection/self-correction prompt.
    ClaudeReflect { observation: &'static str },

    // ── Agent decision-layer outputs ─────────────────────────────────────────
    /// Agent emits "wake" command.
    AgentWakeUp,
    /// Agent requests reflection with a prompt.
    AgentReflect { prompt: &'static str },
    /// Agent suggests a git commit.
    AgentGitCommit { message: &'static str },
    /// Agent requests tool retry.
    AgentToolRetry { tool: &'static str, reason: &'static str },
    /// Agent terminates session.
    AgentTerminate { reason: &'static str },
    /// Agent approves and continues.
    AgentApprove,
}

impl Preset {
    /// Render the preset into a raw string for LLM parsing.
    pub fn render(&self) -> String {
        match self {
            // ── Codex ──────────────────────────────────────────────────────
            Preset::CodexApprove => "yes".into(),
            Preset::CodexReject => "no".into(),
            Preset::CodexCodeBlock { language, code } => {
                format!("```{}\n{}\n```", language, code)
            }
            Preset::CodexJsonToolCall { tool, args } => {
                format!(r#"{{"tool": "{}", "args": {}}}"#, tool, args)
            }
            Preset::CodexThinking { content } => {
                format!(
                    "<thinking>\nLet me think through this...\n</thinking>\n\n{}",
                    content
                )
            }
            Preset::CodexKeyword(kw) => kw.to_string(),

            // ── Claude ─────────────────────────────────────────────────────
            Preset::ClaudeStructured { decision, reasoning } => {
                format!(
                    "<decision>{}</decision>\n<reasoning>{}</reasoning>",
                    decision, reasoning
                )
            }
            Preset::ClaudeJson { fields } => {
                let pairs: Vec<String> = fields
                    .iter()
                    .map(|(k, v)| format!(r#""{}": "{}""#, k, v))
                    .collect();
                format!("{{\n  {}\n}}", pairs.join(",\n  "))
            }
            Preset::ClaudeCommand(cmd) => cmd.to_string(),
            Preset::ClaudeEscalate { reason } => {
                format!(
                    "I need human assistance. Reason: {}",
                    reason
                )
            }
            Preset::ClaudeReflect { observation } => {
                format!(
                    "Upon reflection: {}. I should reconsider my approach.",
                    observation
                )
            }

            // ── Agent ──────────────────────────────────────────────────────
            Preset::AgentWakeUp => "wake".into(),
            Preset::AgentReflect { prompt } => prompt.to_string(),
            Preset::AgentGitCommit { message } => {
                format!("suggest_commit: {}", message)
            }
            Preset::AgentToolRetry { tool, reason } => {
                format!(
                    "retry_tool: {} (reason: {})",
                    tool, reason
                )
            }
            Preset::AgentTerminate { reason } => {
                format!("terminate: {}", reason)
            }
            Preset::AgentApprove => "approve".into(),
        }
    }
}

/// Convenience constructors for common preset combinations.
impl Preset {
    /// Codex approves with a simple "yes".
    pub fn codex_yes() -> Self {
        Preset::CodexApprove
    }

    /// Codex rejects with a simple "no".
    pub fn codex_no() -> Self {
        Preset::CodexReject
    }

    /// Claude decides to escalate to human.
    pub fn claude_escalate(reason: &'static str) -> Self {
        Preset::ClaudeEscalate { reason }
    }

    /// Agent decides to wake up another agent.
    pub fn agent_wakeup() -> Self {
        Preset::AgentWakeUp
    }

    /// Agent decides to reflect on its work.
    pub fn agent_reflect(prompt: &'static str) -> Self {
        Preset::AgentReflect { prompt }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preset_codex_approve() {
        assert_eq!(Preset::CodexApprove.render(), "yes");
    }

    #[test]
    fn preset_codex_code_block() {
        let out = Preset::CodexCodeBlock {
            language: "rust",
            code: "fn main() {}",
        }
        .render();
        assert!(out.contains("```rust"));
        assert!(out.contains("fn main() {}"));
    }

    #[test]
    fn preset_claude_structured() {
        let out = Preset::ClaudeStructured {
            decision: "escalate",
            reasoning: "too complex",
        }
        .render();
        assert!(out.contains("<decision>escalate</decision>"));
        assert!(out.contains("<reasoning>too complex</reasoning>"));
    }

    #[test]
    fn preset_claude_json() {
        let mut fields = HashMap::new();
        fields.insert("action", "commit");
        fields.insert("message", "fix bug");
        let out = Preset::ClaudeJson { fields }.render();
        assert!(out.contains("\"action\": \"commit\""));
        assert!(out.contains("\"message\": \"fix bug\""));
    }

    #[test]
    fn preset_agent_wakeup() {
        assert_eq!(Preset::AgentWakeUp.render(), "wake");
    }

    #[test]
    fn preset_agent_terminate() {
        let out = Preset::AgentTerminate {
            reason: "task complete",
        }
        .render();
        assert!(out.contains("terminate: task complete"));
    }
}
