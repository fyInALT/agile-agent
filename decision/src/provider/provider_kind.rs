//! Provider kind enumeration

use serde::{Deserialize, Serialize};

/// Provider kind
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ProviderKind {
    /// Claude provider
    Claude,
    /// Codex provider
    Codex,
    /// ACP provider (OpenCode/Kimi)
    ACP,
    /// Unknown provider
    Unknown,
}

impl ProviderKind {
    /// Get display name
    pub fn display_name(&self) -> &'static str {
        match self {
            ProviderKind::Claude => "Claude",
            ProviderKind::Codex => "Codex",
            ProviderKind::ACP => "ACP",
            ProviderKind::Unknown => "Unknown",
        }
    }

    /// Check if this is Claude
    pub fn is_claude(&self) -> bool {
        matches!(self, ProviderKind::Claude)
    }

    /// Check if this is Codex
    pub fn is_codex(&self) -> bool {
        matches!(self, ProviderKind::Codex)
    }

    /// Check if this is ACP
    pub fn is_acp(&self) -> bool {
        matches!(self, ProviderKind::ACP)
    }
}

impl Default for ProviderKind {
    fn default() -> Self {
        ProviderKind::Unknown
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
    fn test_provider_kind_claude() {
        let kind = ProviderKind::Claude;
        assert!(kind.is_claude());
        assert!(!kind.is_codex());
        assert_eq!(kind.display_name(), "Claude");
    }

    #[test]
    fn test_provider_kind_codex() {
        let kind = ProviderKind::Codex;
        assert!(kind.is_codex());
        assert!(!kind.is_claude());
        assert_eq!(kind.display_name(), "Codex");
    }

    #[test]
    fn test_provider_kind_acp() {
        let kind = ProviderKind::ACP;
        assert!(kind.is_acp());
        assert_eq!(kind.display_name(), "ACP");
    }

    #[test]
    fn test_provider_kind_unknown() {
        let kind = ProviderKind::Unknown;
        assert!(!kind.is_claude());
        assert!(!kind.is_codex());
        assert!(!kind.is_acp());
    }

    #[test]
    fn test_provider_kind_default() {
        let kind = ProviderKind::default();
        assert_eq!(kind, ProviderKind::Unknown);
    }

    #[test]
    fn test_provider_kind_serde() {
        let kind = ProviderKind::Claude;
        let json = serde_json::to_string(&kind).unwrap();
        let parsed: ProviderKind = serde_json::from_str(&json).unwrap();
        assert_eq!(kind, parsed);
    }

    #[test]
    fn test_provider_kind_display() {
        let kind = ProviderKind::Claude;
        let display = format!("{}", kind);
        assert_eq!(display, "Claude");
    }
}
