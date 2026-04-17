//! Launch Configuration Overlay for agent creation
//!
//! Provides UI for collecting launch configuration input for Work and Decision agents.

use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyEventKind;

use agent_core::launch_config::LaunchSourceMode;
use agent_core::provider::ProviderKind;

/// Preview of parsed launch configuration
#[derive(Debug, Clone)]
pub struct ConfigPreview {
    pub source_mode: LaunchSourceMode,
    pub env_count: usize,
    pub arg_count: usize,
    pub executable: Option<String>,
    pub error: Option<String>,
}

impl Default for ConfigPreview {
    fn default() -> Self {
        Self {
            source_mode: LaunchSourceMode::HostDefault,
            env_count: 0,
            arg_count: 0,
            executable: None,
            error: None,
        }
    }
}

impl ConfigPreview {
    /// Create a successful preview
    fn success(
        mode: LaunchSourceMode,
        env_count: usize,
        arg_count: usize,
        executable: Option<String>,
    ) -> Self {
        Self {
            source_mode: mode,
            env_count,
            arg_count,
            executable,
            error: None,
        }
    }

    /// Create an error preview
    fn error(message: String) -> Self {
        Self {
            source_mode: LaunchSourceMode::HostDefault,
            env_count: 0,
            arg_count: 0,
            executable: None,
            error: Some(message),
        }
    }

    /// Check if preview represents a valid config
    pub fn is_valid(&self) -> bool {
        self.error.is_none()
    }
}

/// Launch configuration overlay state
#[derive(Debug, Clone)]
pub struct LaunchConfigOverlayState {
    /// Provider (locked after selection)
    pub provider: ProviderKind,
    /// Work agent config text
    pub work_config_text: String,
    /// Decision agent config text
    pub decision_config_text: String,
    /// Preview for work config
    pub work_preview: ConfigPreview,
    /// Preview for decision config
    pub decision_preview: ConfigPreview,
    /// Current focus area
    pub focus: LaunchConfigFocus,
    /// Error message to display
    pub error_message: Option<String>,
    /// Whether overlay has been initialized
    pub initialized: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LaunchConfigFocus {
    WorkConfig,
    DecisionConfig,
    Confirm,
}

impl Default for LaunchConfigFocus {
    fn default() -> Self {
        LaunchConfigFocus::WorkConfig
    }
}

/// Command returned from overlay key handling
#[derive(Debug, Clone)]
pub enum LaunchConfigOverlayCommand {
    /// Close the overlay without saving
    Close,
    /// Confirm and create agent with configuration
    Confirm {
        work_config: String,
        decision_config: String,
    },
}

impl LaunchConfigOverlayState {
    /// Create a new overlay for a selected provider
    pub fn new(provider: ProviderKind) -> Self {
        Self {
            provider,
            work_config_text: String::new(),
            decision_config_text: String::new(),
            work_preview: ConfigPreview::default(),
            decision_preview: ConfigPreview::default(),
            focus: LaunchConfigFocus::WorkConfig,
            error_message: None,
            initialized: false,
        }
    }

    /// Update previews based on current text
    pub fn update_previews(&mut self) {
        self.work_preview = self.parse_preview(&self.work_config_text);
        self.decision_preview = self.parse_preview(&self.decision_config_text);
        self.error_message = None;
    }

    /// Parse config text and generate preview
    fn parse_preview(&self, text: &str) -> ConfigPreview {
        if text.trim().is_empty() {
            return ConfigPreview::success(LaunchSourceMode::HostDefault, 0, 0, None);
        }

        match agent_core::launch_config::parse(self.provider, text) {
            Ok(spec) => {
                let mode = spec.source_mode;
                let env_count = spec.env_overrides.len();
                let arg_count = spec.extra_args.len();
                let executable = spec.requested_executable.clone();
                ConfigPreview::success(mode, env_count, arg_count, executable)
            }
            Err(e) => ConfigPreview::error(e.to_string()),
        }
    }

    /// Handle key event
    pub fn handle_key_event(&mut self, key_event: KeyEvent) -> Option<LaunchConfigOverlayCommand> {
        if key_event.kind != KeyEventKind::Press {
            return None;
        }

        // Initialize on first key event
        if !self.initialized {
            self.initialized = true;
            self.update_previews();
        }

        // Handle Ctrl+C as close (same as Esc)
        if key_event.code == KeyCode::Char('c')
            && key_event
                .modifiers
                .contains(crossterm::event::KeyModifiers::CONTROL)
        {
            return Some(LaunchConfigOverlayCommand::Close);
        }

        match key_event.code {
            KeyCode::Esc => Some(LaunchConfigOverlayCommand::Close),
            KeyCode::Enter if self.focus == LaunchConfigFocus::Confirm => {
                // Confirm and close
                self.update_previews();
                if self.work_preview.is_valid() && self.decision_preview.is_valid() {
                    Some(LaunchConfigOverlayCommand::Confirm {
                        work_config: self.work_config_text.clone(),
                        decision_config: self.decision_config_text.clone(),
                    })
                } else {
                    self.error_message = Some("Invalid configuration".to_string());
                    None
                }
            }
            KeyCode::Tab => {
                // Cycle focus
                self.focus = match self.focus {
                    LaunchConfigFocus::WorkConfig => LaunchConfigFocus::DecisionConfig,
                    LaunchConfigFocus::DecisionConfig => LaunchConfigFocus::Confirm,
                    LaunchConfigFocus::Confirm => LaunchConfigFocus::WorkConfig,
                };
                None
            }
            KeyCode::Up => {
                self.focus = match self.focus {
                    LaunchConfigFocus::WorkConfig => LaunchConfigFocus::Confirm,
                    LaunchConfigFocus::DecisionConfig => LaunchConfigFocus::WorkConfig,
                    LaunchConfigFocus::Confirm => LaunchConfigFocus::DecisionConfig,
                };
                None
            }
            KeyCode::Down => {
                self.focus = match self.focus {
                    LaunchConfigFocus::WorkConfig => LaunchConfigFocus::DecisionConfig,
                    LaunchConfigFocus::DecisionConfig => LaunchConfigFocus::Confirm,
                    LaunchConfigFocus::Confirm => LaunchConfigFocus::WorkConfig,
                };
                None
            }
            KeyCode::Char('s')
                if key_event
                    .modifiers
                    .contains(crossterm::event::KeyModifiers::CONTROL) =>
            {
                // Ctrl+S to confirm early
                self.update_previews();
                if self.work_preview.is_valid() && self.decision_preview.is_valid() {
                    Some(LaunchConfigOverlayCommand::Confirm {
                        work_config: self.work_config_text.clone(),
                        decision_config: self.decision_config_text.clone(),
                    })
                } else {
                    self.error_message = Some("Invalid configuration".to_string());
                    None
                }
            }
            KeyCode::Char(c)
                if !key_event
                    .modifiers
                    .contains(crossterm::event::KeyModifiers::CONTROL) =>
            {
                // Add character to focused field (no Ctrl modifier)
                match self.focus {
                    LaunchConfigFocus::WorkConfig => {
                        self.work_config_text.push(c);
                        self.update_previews();
                    }
                    LaunchConfigFocus::DecisionConfig => {
                        self.decision_config_text.push(c);
                        self.update_previews();
                    }
                    LaunchConfigFocus::Confirm => {}
                }
                None
            }
            KeyCode::Backspace => {
                // Remove character from focused field
                match self.focus {
                    LaunchConfigFocus::WorkConfig => {
                        self.work_config_text.pop();
                        self.update_previews();
                    }
                    LaunchConfigFocus::DecisionConfig => {
                        self.decision_config_text.pop();
                        self.update_previews();
                    }
                    LaunchConfigFocus::Confirm => {}
                }
                None
            }
            KeyCode::Delete => {
                // Clear focused field
                match self.focus {
                    LaunchConfigFocus::WorkConfig => {
                        self.work_config_text.clear();
                        self.update_previews();
                    }
                    LaunchConfigFocus::DecisionConfig => {
                        self.decision_config_text.clear();
                        self.update_previews();
                    }
                    LaunchConfigFocus::Confirm => {}
                }
                None
            }
            _ => None,
        }
    }

    /// Check if configuration is valid for confirmation
    pub fn is_valid(&self) -> bool {
        self.work_preview.is_valid() && self.decision_preview.is_valid()
    }

    /// Handle paste event - insert text into focused field
    pub fn handle_paste(&mut self, text: &str) {
        if text.is_empty() {
            return;
        }

        match self.focus {
            LaunchConfigFocus::WorkConfig => {
                self.work_config_text.push_str(text);
                self.update_previews();
            }
            LaunchConfigFocus::DecisionConfig => {
                self.decision_config_text.push_str(text);
                self.update_previews();
            }
            LaunchConfigFocus::Confirm => {}
        }
    }

    /// Get provider label
    pub fn provider_label(&self) -> &'static str {
        match self.provider {
            ProviderKind::Claude => "claude",
            ProviderKind::Codex => "codex",
            ProviderKind::Mock => "mock",
        }
    }
}

/// Parse result for launch config text
pub fn parse_and_preview(provider: ProviderKind, text: &str) -> ConfigPreview {
    if text.trim().is_empty() {
        return ConfigPreview::success(LaunchSourceMode::HostDefault, 0, 0, None);
    }

    match agent_core::launch_config::parse(provider, text) {
        Ok(spec) => ConfigPreview::success(
            spec.source_mode,
            spec.env_overrides.len(),
            spec.extra_args.len(),
            spec.requested_executable,
        ),
        Err(e) => ConfigPreview::error(e.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_launch_config_overlay_new() {
        let overlay = LaunchConfigOverlayState::new(ProviderKind::Claude);
        assert_eq!(overlay.provider, ProviderKind::Claude);
        assert!(overlay.work_config_text.is_empty());
        assert!(overlay.decision_config_text.is_empty());
        assert_eq!(overlay.focus, LaunchConfigFocus::WorkConfig);
    }

    #[test]
    fn test_parse_and_preview_empty() {
        let preview = parse_and_preview(ProviderKind::Claude, "");
        assert!(preview.is_valid());
        assert_eq!(preview.env_count, 0);
    }

    #[test]
    fn test_parse_and_preview_env_only() {
        let preview = parse_and_preview(ProviderKind::Claude, "KEY=value");
        assert!(preview.is_valid());
        assert_eq!(preview.env_count, 1);
        assert_eq!(preview.source_mode, LaunchSourceMode::EnvOnly);
    }

    #[test]
    fn test_parse_and_preview_command_fragment() {
        let preview = parse_and_preview(ProviderKind::Claude, "claude --flag");
        assert!(preview.is_valid());
        assert_eq!(preview.arg_count, 1);
        assert_eq!(preview.executable, Some("claude".to_string()));
    }

    #[test]
    fn test_parse_and_preview_invalid() {
        let preview = parse_and_preview(ProviderKind::Claude, "=invalid");
        assert!(!preview.is_valid());
        assert!(preview.error.is_some());
    }

    #[test]
    fn test_config_preview_success() {
        let preview =
            ConfigPreview::success(LaunchSourceMode::EnvOnly, 2, 1, Some("claude".to_string()));
        assert!(preview.is_valid());
        assert_eq!(preview.env_count, 2);
        assert_eq!(preview.arg_count, 1);
    }

    #[test]
    fn test_config_preview_error() {
        let preview = ConfigPreview::error("parse error".to_string());
        assert!(!preview.is_valid());
        assert_eq!(preview.error, Some("parse error".to_string()));
    }
}
