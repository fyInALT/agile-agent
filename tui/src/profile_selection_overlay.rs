//! Profile selection overlay for agent creation
//!
//! Provides UI for selecting provider profiles when spawning a new agent.
//! Allows selection of both work agent profile and decision agent profile.

use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyEventKind;

use agent_core::provider_profile::ProviderProfile;

/// Which profile section is currently focused
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProfileSection {
    Work,
    Decision,
}

/// Profile selection overlay state
#[derive(Debug, Clone)]
pub struct ProfileSelectionOverlay {
    /// Currently selected section
    section: ProfileSection,
    /// Selected work profile index
    work_selected_index: usize,
    /// Selected decision profile index
    decision_selected_index: usize,
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
}

/// Command returned from overlay key handling
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProfileSelectionCommand {
    /// Close the overlay without selecting
    Close,
    /// Select both profiles and spawn agent
    Select {
        work_profile_id: String,
        decision_profile_id: String,
    },
}

impl ProfileSelectionOverlay {
    /// Create a new overlay with profiles from the store
    pub fn new(
        profiles: Vec<ProviderProfile>,
        default_work_id: &str,
        default_decision_id: &str,
    ) -> Self {
        let profiles: Vec<ProfileDisplayInfo> = profiles
            .into_iter()
            .map(|p| {
                let cli_label = p.base_cli.label().to_string();
                ProfileDisplayInfo {
                    id: p.id.clone(),
                    display_name: p.display_name.clone(),
                    cli_label,
                }
            })
            .collect();

        // Find the default work profile index
        let work_selected_index = profiles
            .iter()
            .position(|p| p.id == default_work_id)
            .unwrap_or(0);

        // Find the default decision profile index
        let decision_selected_index = profiles
            .iter()
            .position(|p| p.id == default_decision_id)
            .unwrap_or(0);

        Self {
            section: ProfileSection::Work,
            work_selected_index,
            decision_selected_index,
            profiles,
        }
    }

    /// Get the currently selected work profile ID
    pub fn selected_work_profile_id(&self) -> Option<String> {
        self.profiles
            .get(self.work_selected_index)
            .map(|p| p.id.clone())
    }

    /// Get the currently selected decision profile ID
    pub fn selected_decision_profile_id(&self) -> Option<String> {
        self.profiles
            .get(self.decision_selected_index)
            .map(|p| p.id.clone())
    }

    /// Get the work profile selected index
    pub fn work_selected_index(&self) -> usize {
        self.work_selected_index
    }

    /// Get the decision profile selected index
    pub fn decision_selected_index(&self) -> usize {
        self.decision_selected_index
    }

    /// Get display info for all profiles
    pub fn profiles(&self) -> &[ProfileDisplayInfo] {
        &self.profiles
    }

    /// Get the current section
    pub fn section(&self) -> ProfileSection {
        self.section
    }

    /// Get the selected index for current section
    fn selected_index(&self) -> usize {
        match self.section {
            ProfileSection::Work => self.work_selected_index,
            ProfileSection::Decision => self.decision_selected_index,
        }
    }

    /// Move selection up
    fn move_up(&mut self) {
        let idx = self.selected_index();
        if idx > 0 {
            match self.section {
                ProfileSection::Work => self.work_selected_index -= 1,
                ProfileSection::Decision => self.decision_selected_index -= 1,
            }
        }
    }

    /// Move selection down
    fn move_down(&mut self) {
        let idx = self.selected_index();
        if idx < self.profiles.len() - 1 {
            match self.section {
                ProfileSection::Work => self.work_selected_index += 1,
                ProfileSection::Decision => self.decision_selected_index += 1,
            }
        }
    }

    /// Switch to the other section
    fn toggle_section(&mut self) {
        self.section = match self.section {
            ProfileSection::Work => ProfileSection::Decision,
            ProfileSection::Decision => ProfileSection::Work,
        };
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
            KeyCode::Left | KeyCode::Right => {
                self.toggle_section();
                None
            }
            KeyCode::Enter => {
                let work_id = self.selected_work_profile_id()?;
                let decision_id = self.selected_decision_profile_id()?;
                Some(ProfileSelectionCommand::Select {
                    work_profile_id: work_id,
                    decision_profile_id: decision_id,
                })
            }
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
            section: ProfileSection::Work,
            work_selected_index: 0,
            decision_selected_index: 0,
            profiles: Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{ProfileSection, ProfileSelectionCommand, ProfileSelectionOverlay};
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
        let overlay =
            ProfileSelectionOverlay::new(profiles, "mock-default", "mock-default");
        assert_eq!(overlay.profiles.len(), 3);
        assert_eq!(overlay.section(), ProfileSection::Work);
        assert_eq!(overlay.work_selected_index, 0);
    }

    #[test]
    fn new_overlay_selects_default_profiles() {
        let profiles = make_test_profiles();
        let overlay =
            ProfileSelectionOverlay::new(profiles, "claude-default", "mock-default");
        assert_eq!(overlay.work_selected_index, 1); // claude-default
        assert_eq!(overlay.decision_selected_index, 0); // mock-default
    }

    #[test]
    fn move_down_selects_next_profile() {
        let profiles = make_test_profiles();
        let mut overlay =
            ProfileSelectionOverlay::new(profiles, "mock-default", "mock-default");
        overlay.move_down();
        assert_eq!(overlay.work_selected_index, 1);
    }

    #[test]
    fn move_up_at_first_stays_at_first() {
        let profiles = make_test_profiles();
        let mut overlay =
            ProfileSelectionOverlay::new(profiles, "mock-default", "mock-default");
        overlay.move_up();
        assert_eq!(overlay.work_selected_index, 0);
    }

    #[test]
    fn toggle_section_switches() {
        let profiles = make_test_profiles();
        let mut overlay =
            ProfileSelectionOverlay::new(profiles, "claude-default", "mock-default");
        assert_eq!(overlay.section(), ProfileSection::Work);

        overlay.toggle_section();
        assert_eq!(overlay.section(), ProfileSection::Decision);

        overlay.toggle_section();
        assert_eq!(overlay.section(), ProfileSection::Work);
    }

    #[test]
    fn enter_returns_both_profile_ids() {
        let profiles = make_test_profiles();
        let mut overlay =
            ProfileSelectionOverlay::new(profiles, "claude-default", "codex-default");

        overlay.section = ProfileSection::Work;
        overlay.work_selected_index = 1;
        overlay.decision_selected_index = 2;

        let result = overlay.handle_key_event(KeyEvent::new(
            KeyCode::Enter,
            crossterm::event::KeyModifiers::NONE,
        ));
        assert_eq!(
            result,
            Some(ProfileSelectionCommand::Select {
                work_profile_id: "claude-default".to_string(),
                decision_profile_id: "codex-default".to_string()
            })
        );
    }

    #[test]
    fn esc_closes_overlay() {
        let profiles = make_test_profiles();
        let mut overlay =
            ProfileSelectionOverlay::new(profiles, "mock-default", "mock-default");
        let result = overlay.handle_key_event(KeyEvent::new(
            KeyCode::Esc,
            crossterm::event::KeyModifiers::NONE,
        ));
        assert_eq!(result, Some(ProfileSelectionCommand::Close));
    }
}
