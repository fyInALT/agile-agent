//! Profile selection overlay for agent creation
//!
//! Provides UI for selecting a provider profile when spawning a new agent.
//! This is the primary way to create agents, replacing direct ProviderKind selection.

use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyEventKind;

use agent_core::provider_profile::ProviderProfile;

/// Profile selection overlay state
#[derive(Debug, Clone)]
pub struct ProfileSelectionOverlay {
    /// Currently selected profile index
    selected_index: usize,
    /// Available profiles
    profiles: Vec<ProfileDisplayInfo>,
}

/// Display info for a profile
#[derive(Debug, Clone)]
pub struct ProfileDisplayInfo {
    /// Profile ID
    pub id: String,
    /// Display name
    pub display_name: String,
    /// Base CLI type label
    pub cli_label: String,
    /// Description (optional)
    pub description: Option<String>,
}

/// Command returned from overlay key handling
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProfileSelectionCommand {
    /// Close the overlay without selecting
    Close,
    /// Select the profile and spawn agent
    Select(String),
}

impl ProfileSelectionOverlay {
    /// Create a new overlay with profiles from the store
    pub fn new(profiles: Vec<ProviderProfile>, default_work_id: &str) -> Self {
        let profiles: Vec<ProfileDisplayInfo> = profiles
            .into_iter()
            .map(|p| {
                let cli_label = p.base_cli.label().to_string();
                ProfileDisplayInfo {
                    id: p.id.clone(),
                    display_name: p.display_name.clone(),
                    cli_label,
                    description: p.description.clone(),
                }
            })
            .collect();

        // Find the default work profile and select it
        let selected_index = profiles
            .iter()
            .position(|p| p.id == default_work_id)
            .unwrap_or(0);

        Self {
            selected_index,
            profiles,
        }
    }

    /// Get the currently selected profile ID
    pub fn selected_profile_id(&self) -> Option<String> {
        self.profiles
            .get(self.selected_index)
            .map(|p| p.id.clone())
    }

    /// Get display info for all profiles
    pub fn profiles(&self) -> &[ProfileDisplayInfo] {
        &self.profiles
    }

    /// Get the currently selected profile
    pub fn selected_profile(&self) -> Option<&ProfileDisplayInfo> {
        self.profiles.get(self.selected_index)
    }

    /// Get the selected index
    pub fn selected_index(&self) -> usize {
        self.selected_index
    }

    /// Move selection up
    pub fn move_up(&mut self) {
        if self.selected_index > 0 {
            self.selected_index -= 1;
        }
    }

    /// Move selection down
    pub fn move_down(&mut self) {
        if self.selected_index < self.profiles.len() - 1 {
            self.selected_index += 1;
        }
    }

    /// Handle key event
    pub fn handle_key_event(&mut self, key_event: KeyEvent) -> Option<ProfileSelectionCommand> {
        if key_event.kind != KeyEventKind::Press {
            return None;
        }

        match key_event.code {
            KeyCode::Esc => Some(ProfileSelectionCommand::Close),
            KeyCode::Up => {
                self.move_up();
                None
            }
            KeyCode::Down => {
                self.move_down();
                None
            }
            KeyCode::Enter => self.selected_profile_id().map(ProfileSelectionCommand::Select),
            KeyCode::Char('c')
                if key_event
                    .modifiers
                    .contains(crossterm::event::KeyModifiers::CONTROL) =>
            {
                Some(ProfileSelectionCommand::Close)
            }
            _ => None,
        }
    }
}

impl Default for ProfileSelectionOverlay {
    fn default() -> Self {
        Self {
            selected_index: 0,
            profiles: Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{ProfileDisplayInfo, ProfileSelectionCommand, ProfileSelectionOverlay};
    use agent_core::provider_profile::{CliBaseType, ProviderProfile};
    use crossterm::event::KeyCode;
    use crossterm::event::KeyEvent;

    fn make_test_profiles() -> Vec<ProviderProfile> {
        vec![
            ProviderProfile::default_for_cli(CliBaseType::Mock),
            ProviderProfile::default_for_cli(CliBaseType::Claude),
            ProviderProfile::default_for_cli(CliBaseType::Codex),
        ]
    }

    #[test]
    fn new_overlay_with_profiles() {
        let profiles = make_test_profiles();
        let overlay = ProfileSelectionOverlay::new(profiles, "mock-default");
        assert_eq!(overlay.profiles.len(), 3);
        assert_eq!(overlay.selected_index, 0); // mock-default is first
    }

    #[test]
    fn new_overlay_selects_default_work_profile() {
        let profiles = make_test_profiles();
        let overlay = ProfileSelectionOverlay::new(profiles, "claude-default");
        assert_eq!(overlay.selected_index, 1); // claude-default is second
    }

    #[test]
    fn move_down_selects_next_profile() {
        let profiles = make_test_profiles();
        let mut overlay = ProfileSelectionOverlay::new(profiles, "mock-default");
        overlay.move_down();
        assert_eq!(overlay.selected_index, 1);
    }

    #[test]
    fn move_up_at_first_stays_at_first() {
        let profiles = make_test_profiles();
        let mut overlay = ProfileSelectionOverlay::new(profiles, "mock-default");
        overlay.move_up();
        assert_eq!(overlay.selected_index, 0);
    }

    #[test]
    fn move_down_at_last_stays_at_last() {
        let profiles = make_test_profiles();
        let mut overlay = ProfileSelectionOverlay::new(profiles, "mock-default");
        overlay.selected_index = overlay.profiles.len() - 1;
        overlay.move_down();
        assert_eq!(overlay.selected_index, overlay.profiles.len() - 1);
    }

    #[test]
    fn enter_returns_selected_profile_id() {
        let profiles = make_test_profiles();
        let mut overlay = ProfileSelectionOverlay::new(profiles, "mock-default");
        overlay.selected_index = 1;
        let result = overlay.handle_key_event(KeyEvent::new(
            KeyCode::Enter,
            crossterm::event::KeyModifiers::NONE,
        ));
        assert_eq!(result, Some(ProfileSelectionCommand::Select("claude-default".to_string())));
    }

    #[test]
    fn esc_closes_overlay() {
        let profiles = make_test_profiles();
        let mut overlay = ProfileSelectionOverlay::new(profiles, "mock-default");
        let result = overlay.handle_key_event(KeyEvent::new(
            KeyCode::Esc,
            crossterm::event::KeyModifiers::NONE,
        ));
        assert_eq!(result, Some(ProfileSelectionCommand::Close));
    }
}
