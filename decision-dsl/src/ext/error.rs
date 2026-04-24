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
    YamlSyntax(String),
    UnknownNodeKind { kind: String },
    UnknownEvaluatorKind { kind: String },
    UnknownParserKind { kind: String },
    MissingProperty(&'static str),
    MissingRules,
    DuplicatePriority { priority: u32 },
    InvalidProperty { key: String, value: String, reason: String },
    UnresolvedSubTree { name: String },
    CircularSubTreeRef { name: String },
    DuplicateName { name: String },
    UnexpectedValue { got: String, expected: Vec<String> },
    NoMatch { pattern: String },
    MissingCaptureGroup { group: usize, pattern: String },
    TypeMismatch { field: String, expected: &'static str, got: String },
    JsonSyntax(String),
    UnsupportedVersion(String),
    Custom(String),
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ParseError::YamlSyntax(s) => write!(f, "YAML syntax error: {}", s),
            ParseError::UnknownNodeKind { kind } => write!(f, "unknown node kind: {}", kind),
            ParseError::UnknownEvaluatorKind { kind } => {
                write!(f, "unknown evaluator kind: {}", kind)
            }
            ParseError::UnknownParserKind { kind } => {
                write!(f, "unknown parser kind: {}", kind)
            }
            ParseError::MissingProperty(p) => write!(f, "missing required property: {}", p),
            ParseError::MissingRules => {
                write!(f, "DecisionRules must have at least one rule")
            }
            ParseError::DuplicatePriority { priority } => {
                write!(f, "duplicate rule priority: {}", priority)
            }
            ParseError::InvalidProperty { key, value, reason } => {
                write!(f, "invalid property '{}' = '{}': {}", key, value, reason)
            }
            ParseError::UnresolvedSubTree { name } => {
                write!(f, "unresolved subtree reference: {}", name)
            }
            ParseError::CircularSubTreeRef { name } => {
                write!(f, "circular subtree reference: {}", name)
            }
            ParseError::DuplicateName { name } => write!(f, "duplicate node name: {}", name),
            ParseError::UnexpectedValue { got, expected } => {
                write!(f, "unexpected value '{}', expected one of: {:?}", got, expected)
            }
            ParseError::NoMatch { pattern } => write!(f, "no match for pattern: {}", pattern),
            ParseError::MissingCaptureGroup { group, pattern } => {
                write!(f, "missing capture group {} in pattern: {}", group, pattern)
            }
            ParseError::TypeMismatch { field, expected, got } => {
                write!(
                    f,
                    "type mismatch for field '{}': expected {}, got {}",
                    field, expected, got
                )
            }
            ParseError::JsonSyntax(s) => write!(f, "JSON syntax error: {}", s),
            ParseError::UnsupportedVersion(v) => write!(f, "unsupported api version: {}", v),
            ParseError::Custom(msg) => write!(f, "{}", msg),
        }
    }
}

impl std::error::Error for ParseError {}

impl From<serde_yaml::Error> for ParseError {
    fn from(e: serde_yaml::Error) -> Self {
        ParseError::YamlSyntax(e.to_string())
    }
}

// ── RuntimeError ────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub enum RuntimeError {
    MissingVariable { key: String },
    UnknownFilter { filter: String },
    FilterError(String),
    TypeMismatch { key: String, expected: &'static str, got: String },
    Session { kind: SessionErrorKind, message: String },
    MaxRecursion,
    SubTreeNotResolved { name: String },
    Custom(String),
}

impl fmt::Display for RuntimeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RuntimeError::MissingVariable { key } => write!(f, "missing variable: {}", key),
            RuntimeError::UnknownFilter { filter } => write!(f, "unknown filter: {}", filter),
            RuntimeError::FilterError(msg) => write!(f, "filter error: {}", msg),
            RuntimeError::TypeMismatch { key, expected, got } => {
                write!(
                    f,
                    "type mismatch for '{}': expected {}, got {}",
                    key, expected, got
                )
            }
            RuntimeError::Session { kind, message } => {
                write!(f, "session error ({:?}): {}", kind, message)
            }
            RuntimeError::MaxRecursion => write!(f, "maximum recursion depth exceeded"),
            RuntimeError::SubTreeNotResolved { name } => {
                write!(f, "subtree '{}' not resolved", name)
            }
            RuntimeError::Custom(msg) => write!(f, "{}", msg),
        }
    }
}

impl std::error::Error for RuntimeError {}

impl From<SessionError> for RuntimeError {
    fn from(e: SessionError) -> Self {
        RuntimeError::Session {
            kind: e.kind,
            message: e.message,
        }
    }
}

// ── DslError ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub enum DslError {
    Parse(ParseError),
    Runtime(RuntimeError),
}

impl fmt::Display for DslError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DslError::Parse(p) => write!(f, "parse error: {}", p),
            DslError::Runtime(r) => write!(f, "runtime error: {}", r),
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
        DslError::Parse(ParseError::JsonSyntax(err.to_string()))
    }
}
