use serde::{Deserialize, Serialize};

/// Kind of LLM provider
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ProviderKind {
    #[default]
    Mock,
    Claude,
    Codex,
}

impl ProviderKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::Mock => "mock",
            Self::Claude => "claude",
            Self::Codex => "codex",
        }
    }

    pub fn next(self) -> Self {
        match self {
            Self::Mock => Self::Claude,
            Self::Claude => Self::Codex,
            Self::Codex => Self::Mock,
        }
    }

    pub fn all() -> [ProviderKind; 3] {
        [ProviderKind::Mock, ProviderKind::Claude, ProviderKind::Codex]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn provider_kind_labels() {
        assert_eq!(ProviderKind::Claude.label(), "claude");
        assert_eq!(ProviderKind::Mock.label(), "mock");
        assert_eq!(ProviderKind::Codex.label(), "codex");
    }

    #[test]
    fn provider_kind_next() {
        assert_eq!(ProviderKind::Mock.next(), ProviderKind::Claude);
        assert_eq!(ProviderKind::Claude.next(), ProviderKind::Codex);
        assert_eq!(ProviderKind::Codex.next(), ProviderKind::Mock);
    }

    #[test]
    fn provider_kind_serialization() {
        let kind = ProviderKind::Claude;
        let json = serde_json::to_string(&kind).unwrap();
        assert_eq!(json, "\"claude\"");
    }
}