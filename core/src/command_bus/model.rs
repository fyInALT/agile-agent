use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandNamespace {
    Local,
    Agent,
    Provider,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommandTargetSpec {
    AgentName(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandInvocation {
    pub namespace: CommandNamespace,
    pub target: Option<CommandTargetSpec>,
    pub path: Vec<String>,
    pub args: Vec<String>,
    pub raw_tail: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParsedSlashCommand {
    Invocation(CommandInvocation),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandParseError {
    message: String,
}

impl CommandParseError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl fmt::Display for CommandParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for CommandParseError {}
