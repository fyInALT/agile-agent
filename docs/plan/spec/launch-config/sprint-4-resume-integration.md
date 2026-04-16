# Sprint 4: Resume Integration & Error Handling

## Metadata

- Sprint ID: `launch-config-sprint-04`
- Title: `Resume Integration & Error Handling`
- Duration: 1-2 weeks
- Priority: P1 (High)
- Status: `Backlog`
- Created: 2026-04-16
- Depends On: Sprint 1, Sprint 2, Sprint 3
- Design Reference: `docs/superpowers/specs/2026-04-16-agent-launch-config-and-resume-design.md`

## Sprint Goal

Complete the resume flow integration with launch bundle restoration, implement comprehensive error handling for restore failures, add sensitive data redaction for UI and logs, and integrate logging events for launch config lifecycle.

## Stories

### Story 4.1: Work Agent Restore Flow

**Priority**: P0
**Effort**: 3 points
**Status**: Backlog

Restore work agent from launch_bundle with resolved configuration.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T4.1.1 | Implement `restore_agent_with_launch_bundle()` | Todo | - |
| T4.1.2 | Extract launch_bundle from PersistedAgentSnapshot | Todo | - |
| T4.1.3 | Create AgentSlot with restored provider_type | Todo | - |
| T4.1.4 | Restore session_handle from bundle | Todo | - |
| T4.1.5 | Restore transcript and task assignment | Todo | - |
| T4.1.6 | Set status to Idle (not Active) for interrupted agents | Todo | - |
| T4.1.7 | Log restore event with bundle details | Todo | - |
| T4.1.8 | Write restore unit tests | Todo | - |

#### Restore Logic

```rust
fn restore_agent_with_launch_bundle(
    snapshot: &PersistedAgentSnapshot,
) -> Result<AgentSlot> {
    // Use launch_bundle if available
    let bundle = snapshot.launch_bundle.as_ref();
    
    let slot = AgentSlot::restored(
        snapshot.agent_id.clone(),
        snapshot.codename.clone(),
        snapshot.provider_type,
        snapshot.role,
        snapshot.restore_status(),  // Idle for interrupted
        snapshot.restore_session_handle(),
        snapshot.transcript.clone(),
        snapshot.assigned_task_id.clone(),
    );
    
    // Attach launch bundle metadata
    if let Some(bundle) = bundle {
        slot.set_launch_bundle(bundle.clone());
    }
    
    // Log restoration
    logging::debug_event(
        "launch_config.restore",
        "agent restored with launch bundle",
        serde_json::json!({
            "agent_id": snapshot.agent_id.as_str(),
            "provider": snapshot.provider_type,
            "source": "snapshot",
        }),
    );
    
    Ok(slot)
}
```

---

### Story 4.2: Decision Agent Restore Flow

**Priority**: P0
**Effort**: 2 points
**Status**: Backlog

Restore decision agent caller from decision_resolved configuration.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T4.2.1 | Extract decision_resolved from launch_bundle | Todo | - |
| T4.2.2 | Create LLMCaller from decision_resolved | Todo | - |
| T4.2.3 | Inject into DecisionAgentSlot | Todo | - |
| T4.2.4 | Restore binding relationship to work agent | Todo | - |
| T4.2.5 | Log decision agent restore event | Todo | - |
| T4.2.6 | Write decision restore tests | Todo | - |

#### Decision Agent Restore

```rust
fn restore_decision_agent(
    work_agent_id: &AgentId,
    launch_bundle: &AgentLaunchBundle,
) -> DecisionAgentSlot {
    let caller = create_llm_caller_from_spec(&launch_bundle.decision_resolved);
    
    let slot = DecisionAgentSlot::with_launch_bundle(
        work_agent_id.as_str(),
        launch_bundle,
        // ... other params
    );
    
    logging::debug_event(
        "launch_config.restore.decision",
        "decision agent restored",
        serde_json::json!({
            "work_agent_id": work_agent_id.as_str(),
            "provider": launch_bundle.decision_resolved.provider,
        }),
    );
    
    slot
}
```

---

### Story 4.3: Executable Path Validation on Restore

**Priority**: P0
**Effort**: 2 points
**Status**: Backlog

Check if resolved_executable_path still exists at restore time.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T4.3.1 | Implement `validate_executable_exists()` | Todo | - |
| T4.3.2 | Check path before agent restoration | Todo | - |
| T4.3.3 | Set agent to Error state if executable missing | Todo | - |
| T4.3.4 | Keep agent visible in list | Todo | - |
| T4.3.5 | Expose concrete error to user | Todo | - |
| T4.3.6 | Never fall back to host default silently | Todo | - |
| T4.3.7 | Write validation tests | Todo | - |

#### Validation Implementation

```rust
fn validate_executable_exists(spec: &ResolvedLaunchSpec) -> Result<(), RestoreError> {
    let path = Path::new(&spec.resolved_executable_path);
    
    if !path.exists() {
        return Err(RestoreError::ExecutableNotFound {
            path: spec.resolved_executable_path.clone(),
            provider: spec.provider,
        });
    }
    
    Ok(())
}

// On restore failure:
fn handle_restore_failure(
    snapshot: &PersistedAgentSnapshot,
    error: RestoreError,
) -> AgentSlot {
    AgentSlot::restored(
        snapshot.agent_id.clone(),
        snapshot.codename.clone(),
        snapshot.provider_type,
        snapshot.role,
        AgentSlotStatus::error(error.to_string()),
        None,  // No session handle
        snapshot.transcript.clone(),  // Keep transcripts
        None,  // Clear task
    )
}
```

---

### Story 4.4: Snapshot/Agent File Consistency Warning

**Priority**: P1
**Effort**: 2 points
**Status**: Backlog

Handle cases where snapshot and launch-config.json disagree.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T4.4.1 | Load both snapshot bundle and launch-config.json | Todo | - |
| T4.4.2 | Compare for consistency | Todo | - |
| T4.4.3 | Use snapshot if both exist and disagree | Todo | - |
| T4.4.4 | Log warning on inconsistency | Todo | - |
| T4.4.5 | Write consistency tests | Todo | - |

#### Consistency Handling

```rust
fn resolve_launch_bundle_for_restore(
    workplace: &WorkplaceStore,
    snapshot: &PersistedAgentSnapshot,
) -> Result<AgentLaunchBundle, RestoreError> {
    // Prefer snapshot bundle
    if let Some(bundle) = &snapshot.launch_bundle {
        // Check agent file for consistency
        let file_bundle = load_launch_config(workplace, &snapshot.agent_id)?;
        
        if let Some(file_bundle) = file_bundle {
            if bundle != file_bundle {
                logging::warn_event(
                    "launch_config.restore.inconsistent",
                    "snapshot and agent file launch configs differ",
                    serde_json::json!({
                        "agent_id": snapshot.agent_id.as_str(),
                        "using": "snapshot",
                    }),
                );
            }
        }
        
        return Ok(bundle.clone());
    }
    
    // Fallback to agent file
    load_launch_config(workplace, &snapshot.agent_id)?
        .ok_or(RestoreError::MissingLaunchBundle)
}
```

---

### Story 4.5: Sensitive Data Redaction

**Priority**: P1
**Effort**: 2 points
**Status**: Backlog

Redact sensitive environment variables in UI and logs.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T4.5.1 | Define `SENSITIVE_KEY_PATTERNS` list | Todo | - |
| T4.5.2 | Implement `redact_env_value()` function | Todo | - |
| T4.5.3 | Apply redaction in preview display | Todo | - |
| T4.5.4 | Apply redaction in status line summary | Todo | - |
| T4.5.5 | Apply redaction in log events | Todo | - |
| T4.5.6 | Keep full values in stored files | Todo | - |
| T4.5.7 | Write redaction tests | Todo | - |

#### Redaction Patterns

```rust
const SENSITIVE_KEY_PATTERNS: &[&str] = &[
    "*_TOKEN",
    "*_API_KEY",
    "*_AUTH_TOKEN",
    "*_SECRET",
    "*_PASSWORD",
    "Authorization",
];

fn redact_env_value(key: &str, value: &str) -> String {
    if SENSITIVE_KEY_PATTERNS.iter().any(|p| matches_pattern(key, p)) {
        // Show first 8 chars + "..."
        if value.len() > 8 {
            format!("{}...", &value[..8])
        } else {
            "***"
        }
    } else {
        value.to_string()
    }
}

fn matches_pattern(key: &str, pattern: &str) -> bool {
    // Handle wildcard patterns like *_TOKEN
    if pattern.starts_with("*") {
        key.ends_with(&pattern[1..])
    } else {
        key == pattern
    }
}
```

---

### Story 4.6: Launch Config Logging Events

**Priority**: P1
**Effort**: 2 points
**Status**: Backlog

Implement all launch_config.* logging events from design.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T4.6.1 | Add `launch_config.parse.start` event | Todo | - |
| T4.6.2 | Add `launch_config.parse.success` event | Todo | - |
| T4.6.3 | Add `launch_config.parse.failed` event | Todo | - |
| T4.6.4 | Add `launch_config.resolve.success` event | Todo | - |
| T4.6.5 | Add `launch_config.persist` event | Todo | - |
| T4.6.6 | Add `launch_config.restore` event | Todo | - |
| T4.6.7 | Add `launch_config.restore.failed` event | Todo | - |
| T4.6.8 | Write logging tests | Todo | - |

#### Event Table

| Event | When | Fields |
|------|------|--------|
| `launch_config.parse.start` | Parse begins | provider, target, source_mode_guess |
| `launch_config.parse.success` | Parse succeeds | provider, target, source_mode, executable, env_count, arg_count |
| `launch_config.parse.failed` | Parse fails | provider, target, error |
| `launch_config.resolve.success` | Resolution succeeds | provider, target, executable, env_count |
| `launch_config.persist` | Bundle saved | agent_id, provider |
| `launch_config.restore` | Bundle restored | agent_id, provider, source |
| `launch_config.restore.failed` | Restore fails | agent_id, error |

---

### Story 4.7: Resume Tests

**Priority**: P0
**Effort**: 3 points
**Status**: Backlog

Comprehensive resume flow tests with launch bundles.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T4.7.1 | Test restored slots contain launch_bundle | Todo | - |
| T4.7.2 | Test work agent uses resolved config | Todo | - |
| T4.7.3 | Test decision agent uses decision_resolved | Todo | - |
| T4.7.4 | Test missing executable -> Error state | Todo | - |
| T4.7.5 | Test corrupted launch-config.json handling | Todo | - |
| T4.7.6 | Test snapshot migration with restore | Todo | - |
| T4.7.7 | Test empty decision config -> host default | Todo | - |
| T4.7.8 | Test different work/decision configs restore correctly | Todo | - |

---

### Story 4.8: Status Line Summary Display

**Priority**: P1
**Effort**: 2 points
**Status**: Backlog

Show compact summary of parsed config after agent creation.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T4.8.1 | Implement `format_launch_summary()` | Todo | - |
| T4.8.2 | Display provider and executable | Todo | - |
| T4.8.3 | Display env override count | Todo | - |
| T4.8.4 | Display decision config source | Todo | - |
| T4.8.5 | Apply redaction to summary | Todo | - |
| T4.8.6 | Show in status line after creation | Todo | - |
| T4.8.7 | Write summary tests | Todo | - |

#### Summary Format

```text
alpha [claude]: exec=/usr/bin/claude, env=2 overrides, decision=host default
bravo [codex]: exec=/usr/bin/codex, env=1 override, decision=env-only
```

---

## Sprint Risks

| Risk | Probability | Impact | Mitigation |
|------|-------------|--------|------------|
| Resume from corrupted snapshot | Low | High | Backup snapshots, error state fallback |
| Missing executable common in CI | Medium | Medium | Clear error message, repair guide |
| Redaction incomplete | Low | Medium | Review all display/log paths |

## Sprint Deliverables

- Complete resume flow with launch bundle
- Work and decision agent restore logic
- Executable validation on restore
- Snapshot/agent file consistency handling
- Sensitive data redaction
- Complete logging event integration
- Status line summary display
- Comprehensive resume tests

## Dependencies

- Sprint 1: Data models
- Sprint 2: Persistence, resolver
- Sprint 3: Provider integration, UI

## Module Structure

```
core/src/launch_config/
├── ...
├── restore.rs        # NEW: Restore logic with bundle
├── redaction.rs      # NEW: Sensitive value redaction

tui/src/
├── ...
├── status_summary.rs # Modified: Launch summary display
```

## Completion Checklist

After Sprint 4, validate:

- [ ] Ctrl+N -> Provider selection -> Config overlay -> Agent creation works
- [ ] Mock provider skips config overlay
- [ ] Empty config uses host default
- [ ] Decision config independent from work config
- [ ] Resume restores launch bundles correctly
- [ ] Missing executable shows Error state
- [ ] Sensitive values redacted in UI/logs
- [ ] All logging events emitted correctly
- [ ] Backward compatible with old snapshots

## Feature Complete

This is the final sprint for the launch config feature. After completion, the feature is ready for:

- Manual testing
- Integration with template system (future)
- Launch config cloning (future)
- Resume-time repair flow (future)

## Future Extensions

The design document outlines these follow-on capabilities:

1. Launch-config test runs
2. Launch summaries in overview/dashboard
3. Cloning config from existing agent
4. Saving current config as template
5. Diffing two agents' launch configs
6. Resume-time repair flow for broken paths
