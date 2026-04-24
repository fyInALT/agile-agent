use std::fmt;

// ── SessionError / SessionErrorKind ─────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionErrorKind {
    Unavailable,
    Timeout,
    UnexpectedFormat,
    SendFailed,
}

#[derive(Debug, Clone)]
pub struct SessionError {
    pub kind: SessionErrorKind,
    pub message: String,
}

impl fmt::Display for SessionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "session error ({:?}): {}", self.kind, self.message)
    }
}

impl std::error::Error for SessionError {}

// ── ParseError ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub enum ParseError {
    InvalidYaml { detail: String },
    MissingField { field: String, node: String },
    UnknownNodeType { kind: String },
    DuplicateTreeId { id: String },
    InvalidTemplate { detail: String },
    InvalidRegex { detail: String },
    InvalidTimeout { value: String },
    InvalidEvaluator { name: String },
    MissingSubtree { id: String },
    CycleDetected { ids: Vec<String> },
    InvalidPath { path: String },
    InvalidCaseKey { key: String },
    MixedCaseTypes,
    InvalidEnumCase { case: String, allowed: Vec<String> },
    MissingDefaultCase,
    InvalidBundleFormat { detail: String },
    InvalidApiVersion { version: String },
    InvalidDesugaring { detail: String },
    MissingOnErrorHandler { node: String },
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ParseError::InvalidYaml { detail } => write!(f, "invalid YAML: {detail}"),
            ParseError::MissingField { field, node } => {
                write!(f, "missing field '{field}' in node '{node}'")
            }
            ParseError::UnknownNodeType { kind } => write!(f, "unknown node type: {kind}"),
            ParseError::DuplicateTreeId { id } => write!(f, "duplicate tree id: {id}"),
            ParseError::InvalidTemplate { detail } => write!(f, "invalid template: {detail}"),
            ParseError::InvalidRegex { detail } => write!(f, "invalid regex: {detail}"),
            ParseError::InvalidTimeout { value } => write!(f, "invalid timeout value: {value}"),
            ParseError::InvalidEvaluator { name } => write!(f, "invalid evaluator: {name}"),
            ParseError::MissingSubtree { id } => write!(f, "missing subtree definition: {id}"),
            ParseError::CycleDetected { ids } => {
                let joined = ids.iter().map(|s| s.as_str()).collect::<Vec<_>>().join(", ");
                write!(f, "cycle detected in tree references: [{joined}]")
            }
            ParseError::InvalidPath { path } => write!(f, "invalid blackboard path: {path}"),
            ParseError::InvalidCaseKey { key } => write!(f, "invalid case key: '{key}'"),
            ParseError::MixedCaseTypes => {
                write!(f, "mixed case types in switch (string/int)")
            }
            ParseError::InvalidEnumCase { case, allowed } => {
                let joined = allowed.iter().map(|s| s.as_str()).collect::<Vec<_>>().join(", ");
                write!(f, "invalid enum case '{case}', allowed: [{joined}]")
            }
            ParseError::MissingDefaultCase => write!(f, "switch missing default case"),
            ParseError::InvalidBundleFormat { detail } => {
                write!(f, "invalid bundle format: {detail}")
            }
            ParseError::InvalidApiVersion { version } => write!(f, "invalid API version: {version}"),
            ParseError::InvalidDesugaring { detail } => write!(f, "invalid desugaring: {detail}"),
            ParseError::MissingOnErrorHandler { node } => {
                write!(f, "missing on_error handler in node '{node}'")
            }
        }
    }
}

impl std::error::Error for ParseError {}

impl From<serde_yaml::Error> for ParseError {
    fn from(err: serde_yaml::Error) -> Self {
        ParseError::InvalidYaml {
            detail: err.to_string(),
        }
    }
}

// ── RuntimeError ────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub enum RuntimeError {
    NodeNotFound { path: Vec<usize> },
    InvalidBlackboardAccess { path: String },
    EvaluatorFailure { name: String, detail: String },
    TemplateRenderFailure { detail: String },
    PromptTimeout { node: String, timeout_ms: u64 },
    SessionError { kind: SessionErrorKind, message: String },
    MaxReflectionExceeded,
    CooldownActive { node: String, remaining_ms: u64 },
}

impl fmt::Display for RuntimeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RuntimeError::NodeNotFound { path } => write!(f, "node not found at path {path:?}"),
            RuntimeError::InvalidBlackboardAccess { path } => {
                write!(f, "invalid blackboard access: {path}")
            }
            RuntimeError::EvaluatorFailure { name, detail } => {
                write!(f, "evaluator '{name}' failed: {detail}")
            }
            RuntimeError::TemplateRenderFailure { detail } => {
                write!(f, "template render failure: {detail}")
            }
            RuntimeError::PromptTimeout { node, timeout_ms } => {
                write!(f, "prompt node '{node}' timed out after {timeout_ms}ms")
            }
            RuntimeError::SessionError { kind, message } => {
                write!(f, "session error ({kind:?}): {message}")
            }
            RuntimeError::MaxReflectionExceeded => write!(f, "max reflection rounds exceeded"),
            RuntimeError::CooldownActive { node, remaining_ms } => {
                write!(f, "cooldown active on node '{node}', {remaining_ms}ms remaining")
            }
        }
    }
}

impl std::error::Error for RuntimeError {}

// ── DslError ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub enum DslError {
    Parse(ParseError),
    Runtime(RuntimeError),
}

impl fmt::Display for DslError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DslError::Parse(p) => write!(f, "parse error: {p}"),
            DslError::Runtime(r) => write!(f, "runtime error: {r}"),
        }
    }
}

impl std::error::Error for DslError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        None
    }
}

impl From<ParseError> for DslError {
    fn from(err: ParseError) -> Self {
        DslError::Parse(err)
    }
}

impl From<RuntimeError> for DslError {
    fn from(err: RuntimeError) -> Self {
        DslError::Runtime(err)
    }
}

impl From<SessionError> for RuntimeError {
    fn from(err: SessionError) -> Self {
        RuntimeError::SessionError {
            kind: err.kind,
            message: err.message,
        }
    }
}

impl From<SessionError> for DslError {
    fn from(err: SessionError) -> Self {
        DslError::Runtime(err.into())
    }
}

impl From<serde_yaml::Error> for DslError {
    fn from(err: serde_yaml::Error) -> Self {
        DslError::Parse(err.into())
    }
}

impl From<serde_json::Error> for DslError {
    fn from(err: serde_json::Error) -> Self {
        DslError::Parse(ParseError::InvalidYaml {
            detail: err.to_string(),
        })
    }
}
