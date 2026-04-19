# Sprint 3: Agent Integration

## Metadata

- Sprint ID: `provider-profile-sprint-03`
- Title: `Agent Integration`
- Duration: 1 week
- Priority: P0 (Critical)
- Status: `Backlog`
- Created: 2026-04-19
- Depends On: `provider-profile-sprint-02`
- Design Reference: `docs/plan/provider-profile-requirements.md`

## Sprint Goal

Integrate profile system with AgentPool and DecisionAgentSlot, enabling profile-based agent creation with backward compatibility.

## Stories

### Story 3.1: AgentPool Profile Support

**Priority**: P0
**Effort**: 4 points
**Status**: Backlog

Add profile-based agent spawning to AgentPool.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T3.1.1 | Add `ProfileStore` to AgentPool | Todo | - |
| T3.1.2 | Implement `spawn_agent_with_profile()` | Todo | - |
| T3.1.3 | Implement `spawn_agent_with_worktree_and_profile()` | Todo | - |
| T3.1.4 | Implement backward compatibility shim | Todo | - |
| T3.1.5 | Store profile_id in AgentSlot | Todo | - |
| T3.1.6 | Handle profile resolution errors | Todo | - |
| T3.1.7 | Write unit tests for profile spawning | Todo | - |

#### Technical Design

```rust
// In agent_pool.rs

pub struct AgentPool {
    // ... existing fields ...
    
    /// Provider profile store (optional, loaded from config)
    profile_store: Option<ProfileStore>,
}

impl AgentPool {
    /// Load profiles from persistence
    pub fn load_profiles(&mut self, persistence: &ProfilePersistence) -> Result<()> {
        self.profile_store = Some(persistence.load_merged()?);
        Ok(())
    }
    
    /// Spawn agent with a specific profile
    pub fn spawn_agent_with_profile(
        &mut self,
        profile_id: &ProfileId,
    ) -> Result<AgentId, ProfileError> {
        let store = self.profile_store.as_ref()
            .ok_or(ProfileError::NoProfileStore)?;
        
        let profile = store.get_profile(profile_id)
            .ok_or(ProfileError::ProfileNotFound(profile_id.clone()))?;
        
        let resolved = resolve_profile(profile)?;
        let provider_kind = profile.base_cli.to_provider_kind()
            .unwrap_or(ProviderKind::Mock);
        
        // Create slot with profile info
        let agent_id = self.generate_agent_id();
        let codename = Self::generate_codename(self.next_agent_index - 1);
        let mut slot = AgentSlot::new(agent_id.clone(), codename, 
            ProviderType::from_provider_kind(provider_kind));
        slot.set_profile_id(profile_id.clone());
        
        self.slots.push(slot);
        Ok(agent_id)
    }
    
    /// Spawn agent with worktree and profile
    pub fn spawn_agent_with_worktree_and_profile(
        &mut self,
        profile_id: &ProfileId,
        branch_name: Option<String>,
        task_id: Option<String>,
    ) -> Result<AgentId, AgentPoolWorktreeError> {
        // Similar to spawn_agent_with_profile but also creates worktree
        // ...
    }
    
    /// Backward compatible spawn using ProviderKind
    pub fn spawn_agent(&mut self, provider_kind: ProviderKind) -> Result<AgentId, String> {
        // If profile store exists, use default profile for provider_kind
        if let Some(store) = &self.profile_store {
            let default_id = format!("{}-default", provider_kind.label());
            if store.get_profile(&default_id).is_some() {
                return self.spawn_agent_with_profile(&default_id)
                    .map_err(|e| e.to_string());
            }
        }
        
        // Fallback to original implementation
        // ... existing code ...
    }
}

// In agent_slot.rs

pub struct AgentSlot {
    // ... existing fields ...
    
    /// Profile ID used to create this agent (if profile-based)
    profile_id: Option<ProfileId>,
}

impl AgentSlot {
    pub fn set_profile_id(&mut self, id: ProfileId) {
        self.profile_id = Some(id);
    }
    
    pub fn profile_id(&self) -> Option<&ProfileId> {
        self.profile_id.as_ref()
    }
}
```

---

### Story 3.2: Decision Agent Profile Support

**Priority**: P0
**Effort**: 3 points
**Status**: Backlog

Enable independent profile selection for decision layer.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T3.2.1 | Add profile_id to DecisionAgentSlot | Todo | - |
| T3.2.2 | Modify `spawn_decision_agent_for()` to accept profile | Todo | - |
| T3.2.3 | Resolve decision profile separately | Todo | - |
| T3.2.4 | Handle decision profile errors | Todo | - |
| T3.2.5 | Write unit tests for decision profiles | Todo | - |

#### Technical Design

```rust
// In decision_agent_slot.rs

pub struct DecisionAgentSlot {
    // ... existing fields ...
    
    /// Profile ID for decision layer
    profile_id: Option<ProfileId>,
}

impl AgentPool {
    /// Spawn decision agent with specific profile
    fn spawn_decision_agent_with_profile(
        &mut self,
        work_agent_id: &AgentId,
        profile_id: Option<&ProfileId>,
    ) -> Result<AgentId, String> {
        let store = self.profile_store.as_ref();
        
        // Resolve profile for decision layer
        let decision_profile = if let Some(id) = profile_id {
            store.and_then(|s| s.get_profile(id))
        } else if let Some(store) = store {
            store.get_profile(&store.default_decision_profile)
        } else {
            None
        };
        
        // Create decision slot
        let mut slot = DecisionAgentSlot::new(...);
        if let Some(profile) = decision_profile {
            slot.set_profile_id(profile.id.clone());
            // Configure LLM engine with profile settings
        }
        
        // ...
    }
}
```

---

### Story 3.3: Profile-to-LaunchContext Flow

**Priority**: P0
**Effort**: 3 points
**Status**: Backlog

Complete integration: Profile → LaunchInputSpec → ResolvedLaunchSpec → ProviderLaunchContext.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T3.3.1 | Create `create_launch_context_from_profile()` | Todo | - |
| T3.3.2 | Pass ProviderLaunchContext to provider start | Todo | - |
| T3.3.3 | Validate profile at agent creation time | Todo | - |
| T3.3.4 | Handle missing env vars in provider start | Todo | - |
| T3.3.5 | Write integration tests for full flow | Todo | - |

#### Technical Design

```rust
/// Create ProviderLaunchContext from a profile
pub fn create_launch_context_from_profile(
    profile: &ProviderProfile,
    cwd: PathBuf,
    session_handle: Option<SessionHandle>,
) -> Result<ProviderLaunchContext> {
    let resolved = resolve_profile(profile)?;
    
    Ok(ProviderLaunchContext::new(resolved, cwd)
        .with_opt_session_handle(session_handle))
}

// Usage in AgentPool
impl AgentPool {
    fn start_provider_for_agent(
        &self,
        profile: &ProviderProfile,
        prompt: String,
        cwd: PathBuf,
    ) -> Result<ProviderThreadHandle> {
        let context = create_launch_context_from_profile(profile, cwd, None)?;
        start_provider_with_handle_and_context(context, prompt, "agent-provider".to_string())
    }
}
```

---

## Sprint Risks

| Risk | Probability | Impact | Mitigation |
|------|-------------|--------|------------|
| Backward compatibility break | Medium | High | Extensive compatibility tests |
| Profile resolution at wrong time | Low | Medium | Resolution at agent creation, not spawn |
| Decision profile inheritance | Low | Low | Explicit separation, no inheritance |

## Sprint Deliverables

- AgentPool profile support methods
- DecisionAgentSlot profile support
- AgentSlot profile_id field
- ProviderLaunchContext from profile flow
- Integration tests

## Dependencies

- Sprint 1 and 2 deliverables
- Existing AgentPool and AgentSlot
- Existing ProviderLaunchContext
- Existing provider start functions

## Next Sprint

After completing this sprint, proceed to [Sprint 4: UI & CLI Integration](./sprint-4-ui-cli.md).