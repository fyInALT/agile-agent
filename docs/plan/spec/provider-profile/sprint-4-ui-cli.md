# Sprint 4: UI & CLI Integration

## Metadata

- Sprint ID: `provider-profile-sprint-04`
- Title: `UI & CLI Integration`
- Duration: 1 week
- Priority: P1 (High)
- Status: `Backlog`
- Created: 2026-04-19
- Depends On: `provider-profile-sprint-03`
- Design Reference: `docs/plan/provider-profile-requirements.md`

## Sprint Goal

Implement TUI profile selector and CLI profile commands for user interaction.

## Stories

### Story 4.1: TUI Profile Selector

**Priority**: P1
**Effort**: 4 points
**Status**: Backlog

Add profile selection UI to agent creation dialog.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T4.1.1 | Add profile list to AppState | Todo | - |
| T4.1.2 | Create ProfileSelector component | Todo | - |
| T4.1.3 | Add profile dropdown to agent creation | Todo | - |
| T4.1.4 | Display profile in agent status | Todo | - |
| T4.1.5 | Show profile icon/name in agent list | Todo | - |
| T4.1.6 | Write TUI tests for profile selection | Todo | - |

#### Technical Design

```rust
// In tui/src/app_state.rs

pub struct AppState {
    // ... existing fields ...
    
    /// Available profiles (loaded at startup)
    profiles: Vec<ProviderProfile>,
    /// Selected profile for new agent creation
    selected_work_profile: Option<ProfileId>,
    /// Selected profile for decision layer
    selected_decision_profile: Option<ProfileId>,
}

// In tui/src/components/profile_selector.rs

pub struct ProfileSelector {
    profiles: Vec<ProviderProfile>,
    selected_index: usize,
    is_open: bool,
}

impl ProfileSelector {
    pub fn render(&self, frame: &mut Frame, area: Rect) {
        if !self.is_open {
            return;
        }
        
        // Render dropdown list with profile names and icons
        let items: Vec<ListItem> = self.profiles.iter()
            .map(|p| {
                let text = format!("{} {}", p.icon.as_deref().unwrap_or(""), p.display_name);
                ListItem::new(text)
            })
            .collect();
        
        let list = List::new(items)
            .block(Block::default().title("Select Profile"))
            .highlight_style(Style::default().fg(Color::Yellow));
        
        frame.render_widget(list, area);
    }
    
    pub fn select(&mut self) -> Option<&ProviderProfile> {
        self.profiles.get(self.selected_index)
    }
}
```

---

### Story 4.2: CLI Profile Commands

**Priority**: P1
**Effort**: 3 points
**Status**: Backlog

Add CLI commands for profile management.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T4.2.1 | Add `--profile <id>` flag to agent creation | Todo | - |
| T4.2.2 | Add `--decision-profile <id>` flag | Todo | - |
| T4.2.3 | Add `--list-profiles` command | Todo | - |
| T4.2.4 | Add `--create-profile` interactive mode | Todo | - |
| T4.2.5 | Add `--show-profile <id>` command | Todo | - |
| T4.2.6 | Write CLI integration tests | Todo | - |

#### Technical Design

```rust
// In cli/src/main.rs or cli/src/commands.rs

/// Profile-related CLI commands
pub enum ProfileCommand {
    /// List all available profiles
    List,
    /// Show details of a specific profile
    Show { profile_id: ProfileId },
    /// Create a new profile interactively
    Create,
    /// Delete a profile
    Delete { profile_id: ProfileId },
}

impl ProfileCommand {
    pub fn execute(&self, persistence: &ProfilePersistence) -> Result<()> {
        match self {
            Self::List => {
                let store = persistence.load_merged()?;
                println!("Available Profiles:");
                for profile in store.list_profiles() {
                    let default_mark = if profile.id == store.default_work_profile {
                        " [default work]"
                    } else if profile.id == store.default_decision_profile {
                        " [default decision]"
                    } else {
                        ""
                    };
                    println!("  {} {} - {}{}", 
                        profile.icon.as_deref().unwrap_or(""),
                        profile.id,
                        profile.display_name,
                        default_mark);
                }
            }
            Self::Show { profile_id } => {
                let store = persistence.load_merged()?;
                let profile = store.get_profile(profile_id)
                    .ok_or_else(|| anyhow::anyhow!("Profile not found"))?;
                println!("Profile: {}", profile.id);
                println!("  CLI: {}", profile.base_cli.display_name());
                println!("  Display Name: {}", profile.display_name);
                if let Some(desc) = &profile.description {
                    println!("  Description: {}", desc);
                }
                println!("  Environment:");
                for (k, v) in &profile.env_overrides {
                    println!("    {} = {}", k, v);
                }
                if !profile.extra_args.is_empty() {
                    println!("  Extra Args: {}", profile.extra_args.join(" "));
                }
            }
            Self::Create => {
                // Interactive profile creation
                // ...
            }
            Self::Delete { profile_id } => {
                let mut store = persistence.load_global()?;
                store.remove_profile(profile_id);
                persistence.save_global(&store)?;
                println!("Profile '{}' deleted", profile_id);
            }
        }
        Ok(())
    }
}
```

---

## Sprint Risks

| Risk | Probability | Impact | Mitigation |
|------|-------------|--------|------------|
| TUI dropdown complexity | Medium | Medium | Reuse existing dropdown pattern |
| Interactive profile creation UX | Medium | Low | Simple wizard-style prompts |
| Profile command integration | Low | Low | Follow existing CLI patterns |

## Sprint Deliverables

- TUI ProfileSelector component
- Profile display in agent status
- CLI profile management commands
- --profile and --decision-profile flags
- CLI integration tests

## Dependencies

- Sprint 1, 2, 3 deliverables
- Existing TUI framework
- Existing CLI command structure

## Final Integration

After completing this sprint, the Provider Profile System is fully integrated:

1. Users can define profiles in ~/.agile-agent/profiles.json
2. Workplace-level profiles override global
3. Agents can be created with specific profiles
4. Decision layer has independent profile selection
5. Existing ProviderKind-based creation still works
6. TUI shows profile selector dropdown
7. CLI supports profile flags and commands