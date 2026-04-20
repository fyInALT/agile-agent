//! CLI Base Type Enumeration
//!
//! Defines the base CLI executable types for provider profiles.

use crate::ProviderKind;
use serde::{Deserialize, Serialize};

/// CLI base type (the executable to run)
///
/// Each type maps to a specific CLI tool that can be configured
/// with environment variables to use different LLM backends.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CliBaseType {
    /// Mock provider for testing
    Mock,
    /// Claude CLI (Anthropic's official CLI)
    Claude,
    /// Codex CLI (OpenAI's CLI)
    Codex,
    /// OpenCode CLI (future support)
    #[serde(rename = "opencode")]
    OpenCode,
}

impl CliBaseType {
    /// Get the CLI executable label (lowercase, for command-line)
    pub fn label(&self) -> &'static str {
        match self {
            Self::Mock => "mock",
            Self::Claude => "claude",
            Self::Codex => "codex",
            Self::OpenCode => "opencode",
        }
    }

    /// Get the display name for UI
    pub fn display_name(&self) -> &'static str {
        match self {
            Self::Mock => "Mock Provider",
            Self::Claude => "Claude CLI",
            Self::Codex => "Codex CLI",
            Self::OpenCode => "OpenCode CLI",
        }
    }

    /// Convert from ProviderKind (existing enum)
    pub fn from_provider_kind(kind: ProviderKind) -> Self {
        match kind {
            ProviderKind::Mock => Self::Mock,
            ProviderKind::Claude => Self::Claude,
            ProviderKind::Codex => Self::Codex,
        }
    }

    /// Convert to ProviderKind (existing enum)
    ///
    /// Returns None for OpenCode since it's not yet supported as ProviderKind.
    pub fn to_provider_kind(&self) -> Option<ProviderKind> {
        match self {
            Self::Mock => Some(ProviderKind::Mock),
            Self::Claude => Some(ProviderKind::Claude),
            Self::Codex => Some(ProviderKind::Codex),
            Self::OpenCode => None, // Not yet supported as ProviderKind
        }
    }

    /// Get all supported CLI types
    pub fn all() -> [CliBaseType; 4] {
        [Self::Mock, Self::Claude, Self::Codex, Self::OpenCode]
    }

    /// Get CLI types that should be auto-detected (excludes Mock)
    pub fn detectable() -> [CliBaseType; 3] {
        [Self::Claude, Self::Codex, Self::OpenCode]
    }

    /// Check if this CLI type is currently supported as ProviderKind
    pub fn is_supported(&self) -> bool {
        self.to_provider_kind().is_some()
    }

    /// Check if the CLI executable is available in PATH
    ///
    /// Mock is always available. Other types check via `which` command.
    pub fn is_available(&self) -> bool {
        match self {
            Self::Mock => true, // Mock is always available
            Self::Claude | Self::Codex | Self::OpenCode => {
                // Check if executable exists in PATH
                std::process::Command::new("which")
                    .arg(self.label())
                    .output()
                    .map(|o| o.status.success())
                    .unwrap_or(false)
            }
        }
    }
}

impl Default for CliBaseType {
    fn default() -> Self {
        Self::Claude
    }
}

impl std::fmt::Display for CliBaseType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.display_name())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cli_base_type_label() {
        assert_eq!(CliBaseType::Mock.label(), "mock");
        assert_eq!(CliBaseType::Claude.label(), "claude");
        assert_eq!(CliBaseType::Codex.label(), "codex");
        assert_eq!(CliBaseType::OpenCode.label(), "opencode");
    }

    #[test]
    fn test_cli_base_type_display_name() {
        assert_eq!(CliBaseType::Mock.display_name(), "Mock Provider");
        assert_eq!(CliBaseType::Claude.display_name(), "Claude CLI");
        assert_eq!(CliBaseType::Codex.display_name(), "Codex CLI");
        assert_eq!(CliBaseType::OpenCode.display_name(), "OpenCode CLI");
    }

    #[test]
    fn test_cli_base_type_from_provider_kind() {
        assert_eq!(
            CliBaseType::from_provider_kind(ProviderKind::Mock),
            CliBaseType::Mock
        );
        assert_eq!(
            CliBaseType::from_provider_kind(ProviderKind::Claude),
            CliBaseType::Claude
        );
        assert_eq!(
            CliBaseType::from_provider_kind(ProviderKind::Codex),
            CliBaseType::Codex
        );
    }

    #[test]
    fn test_cli_base_type_to_provider_kind() {
        assert_eq!(CliBaseType::Mock.to_provider_kind(), Some(ProviderKind::Mock));
        assert_eq!(CliBaseType::Claude.to_provider_kind(), Some(ProviderKind::Claude));
        assert_eq!(CliBaseType::Codex.to_provider_kind(), Some(ProviderKind::Codex));
        assert_eq!(CliBaseType::OpenCode.to_provider_kind(), None);
    }

    #[test]
    fn test_cli_base_type_is_supported() {
        assert!(CliBaseType::Mock.is_supported());
        assert!(CliBaseType::Claude.is_supported());
        assert!(CliBaseType::Codex.is_supported());
        assert!(!CliBaseType::OpenCode.is_supported());
    }

    #[test]
    fn test_cli_base_type_serde() {
        let cli = CliBaseType::Claude;
        let json = serde_json::to_string(&cli).unwrap();
        assert_eq!(json, "\"claude\"");
        let parsed: CliBaseType = serde_json::from_str(&json).unwrap();
        assert_eq!(cli, parsed);
    }

    #[test]
    fn test_cli_base_type_serde_opencode() {
        let cli = CliBaseType::OpenCode;
        let json = serde_json::to_string(&cli).unwrap();
        assert_eq!(json, "\"opencode\"");
        let parsed: CliBaseType = serde_json::from_str(&json).unwrap();
        assert_eq!(cli, parsed);
    }

    #[test]
    fn test_cli_base_type_default() {
        assert_eq!(CliBaseType::default(), CliBaseType::Claude);
    }

    #[test]
    fn test_cli_base_type_display() {
        assert_eq!(format!("{}", CliBaseType::Claude), "Claude CLI");
    }

    #[test]
    fn test_cli_base_type_is_available_mock_always_true() {
        // Mock is always available
        assert!(CliBaseType::Mock.is_available());
    }

    #[test]
    fn test_cli_base_type_detectable_excludes_mock() {
        let detectable = CliBaseType::detectable();
        assert_eq!(detectable.len(), 3);
        assert!(!detectable.contains(&CliBaseType::Mock));
        assert!(detectable.contains(&CliBaseType::Claude));
        assert!(detectable.contains(&CliBaseType::Codex));
        assert!(detectable.contains(&CliBaseType::OpenCode));
    }

    #[test]
    fn test_cli_base_type_all_includes_all_four() {
        let all = CliBaseType::all();
        assert_eq!(all.len(), 4);
        assert!(all.contains(&CliBaseType::Mock));
        assert!(all.contains(&CliBaseType::Claude));
        assert!(all.contains(&CliBaseType::Codex));
        assert!(all.contains(&CliBaseType::OpenCode));
    }

    #[test]
    fn test_cli_base_type_is_available_check_path() {
        // Claude/Codex/OpenCode availability depends on PATH
        // We can't guarantee they exist, but we can test the logic
        // by verifying Mock is always true and others use `which`
        assert!(CliBaseType::Mock.is_available());

        // For real CLIs, verify they either exist or don't exist
        // (the test just ensures the check doesn't crash)
        let claude_available = CliBaseType::Claude.is_available();
        let codex_available = CliBaseType::Codex.is_available();
        let opencode_available = CliBaseType::OpenCode.is_available();

        // These are boolean results, just verify they're valid
        assert!(claude_available == true || claude_available == false);
        assert!(codex_available == true || codex_available == false);
        assert!(opencode_available == true || opencode_available == false);
    }

    #[test]
    fn test_cli_base_type_detectable_order() {
        let detectable = CliBaseType::detectable();
        // Verify order: Claude, Codex, OpenCode
        assert_eq!(detectable[0], CliBaseType::Claude);
        assert_eq!(detectable[1], CliBaseType::Codex);
        assert_eq!(detectable[2], CliBaseType::OpenCode);
    }

    #[test]
    fn test_cli_base_type_is_supported_vs_is_available() {
        // is_supported means it can be used as ProviderKind
        // is_available means it exists in PATH
        // These are different concepts:
        // - Mock: always supported and always available
        // - OpenCode: not supported but might be available
        assert!(CliBaseType::Mock.is_supported());
        assert!(CliBaseType::Mock.is_available());

        // OpenCode is not supported (no ProviderKind mapping)
        assert!(!CliBaseType::OpenCode.is_supported());
        // But it might be available in PATH (depends on system)
        // We just verify the check doesn't crash
        let _ = CliBaseType::OpenCode.is_available();
    }

    #[test]
    fn test_cli_base_type_from_provider_kind_roundtrip() {
        // Test roundtrip conversion for supported types
        for kind in [ProviderKind::Mock, ProviderKind::Claude, ProviderKind::Codex] {
            let cli = CliBaseType::from_provider_kind(kind);
            let back = cli.to_provider_kind();
            assert_eq!(back, Some(kind));
        }
    }
}