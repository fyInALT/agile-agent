use serde::{Deserialize, Serialize};

/// Kind of LLM provider
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ProviderKind {
    /// Test/mock provider
    #[default]
    Mock,
    /// Anthropic Claude
    Claude,
    /// OpenAI Codex
    Codex,
    /// ACP provider (OpenCode/Kimi)
    ACP,
    /// Unknown provider (fallback)
    Unknown,
}

impl ProviderKind {
    /// Human-readable label (lowercase)
    pub fn label(self) -> &'static str {
        match self {
            Self::Mock => "mock",
            Self::Claude => "claude",
            Self::Codex => "codex",
            Self::ACP => "acp",
            Self::Unknown => "unknown",
        }
    }

    /// Display name (title case)
    pub fn display_name(self) -> &'static str {
        match self {
            Self::Mock => "Mock",
            Self::Claude => "Claude",
            Self::Codex => "Codex",
            Self::ACP => "ACP",
            Self::Unknown => "Unknown",
        }
    }

    /// Check if this is Claude
    pub fn is_claude(self) -> bool {
        matches!(self, Self::Claude)
    }

    /// Check if this is Codex
    pub fn is_codex(self) -> bool {
        matches!(self, Self::Codex)
    }

    /// Check if this is ACP
    pub fn is_acp(self) -> bool {
        matches!(self, Self::ACP)
    }

    /// Cycle to next provider (for manual rotation)
    pub fn next(self) -> Self {
        match self {
            Self::Mock => Self::Claude,
            Self::Claude => Self::Codex,
            Self::Codex => Self::ACP,
            Self::ACP => Self::Unknown,
            Self::Unknown => Self::Mock,
        }
    }

    /// All known provider kinds
    pub fn all() -> [ProviderKind; 5] {
        [
            ProviderKind::Mock,
            ProviderKind::Claude,
            ProviderKind::Codex,
            ProviderKind::ACP,
            ProviderKind::Unknown,
        ]
    }
}

impl std::fmt::Display for ProviderKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.display_name())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn provider_kind_labels() {
        assert_eq!(ProviderKind::Claude.label(), "claude");
        assert_eq!(ProviderKind::Codex.label(), "codex");
        assert_eq!(ProviderKind::ACP.label(), "acp");
        assert_eq!(ProviderKind::Unknown.label(), "unknown");
        assert_eq!(ProviderKind::Mock.label(), "mock");
    }

    #[test]
    fn provider_kind_display_name() {
        assert_eq!(ProviderKind::Claude.display_name(), "Claude");
        assert_eq!(ProviderKind::ACP.display_name(), "ACP");
    }

    #[test]
    fn provider_kind_is_checks() {
        assert!(ProviderKind::Claude.is_claude());
        assert!(ProviderKind::Codex.is_codex());
        assert!(ProviderKind::ACP.is_acp());
        assert!(!ProviderKind::Mock.is_claude());
    }

    #[test]
    fn provider_kind_next_cycles() {
        assert_eq!(ProviderKind::Mock.next(), ProviderKind::Claude);
        assert_eq!(ProviderKind::Claude.next(), ProviderKind::Codex);
        assert_eq!(ProviderKind::Codex.next(), ProviderKind::ACP);
        assert_eq!(ProviderKind::ACP.next(), ProviderKind::Unknown);
        assert_eq!(ProviderKind::Unknown.next(), ProviderKind::Mock);
    }

    #[test]
    fn provider_kind_all() {
        assert_eq!(ProviderKind::all().len(), 5);
    }
}
