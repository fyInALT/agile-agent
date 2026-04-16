use std::fs;
use std::path::PathBuf;

use anyhow::Context;
use anyhow::Result;

use crate::agent_runtime::AgentId;
use crate::agent_store::AgentStore;
use crate::launch_config::AgentLaunchBundle;
use crate::logging;

/// File name for launch config in agent directory.
pub const LAUNCH_CONFIG_FILENAME: &str = "launch-config.json";

/// Get the path to an agent's launch config file.
pub fn launch_config_path(agent_store: &AgentStore, agent_id: &AgentId) -> PathBuf {
    agent_store.agent_dir(agent_id).join(LAUNCH_CONFIG_FILENAME)
}

/// Save an AgentLaunchBundle to the agent's directory.
pub fn save_launch_config(
    agent_store: &AgentStore,
    agent_id: &AgentId,
    bundle: &AgentLaunchBundle,
) -> Result<PathBuf> {
    let path = launch_config_path(agent_store, agent_id);

    // Ensure agent directory exists
    let agent_dir = agent_store.agent_dir(agent_id);
    fs::create_dir_all(&agent_dir)
        .with_context(|| format!("failed to create agent directory: {}", agent_dir.display()))?;

    // Write the launch config as pretty JSON
    let json = serde_json::to_string_pretty(bundle)
        .context("failed to serialize launch config")?;
    fs::write(&path, json)
        .with_context(|| format!("failed to write launch config: {}", path.display()))?;

    logging::debug_event(
        "launch_config.persist",
        "launch config bundle saved",
        serde_json::json!({
            "agent_id": agent_id.as_str(),
            "provider": bundle.work_resolved.provider.label(),
            "path": path.display().to_string(),
        }),
    );

    Ok(path)
}

/// Load an AgentLaunchBundle from the agent's directory.
pub fn load_launch_config(
    agent_store: &AgentStore,
    agent_id: &AgentId,
) -> Result<Option<AgentLaunchBundle>> {
    let path = launch_config_path(agent_store, agent_id);

    if !path.exists() {
        return Ok(None);
    }

    let json = fs::read_to_string(&path)
        .with_context(|| format!("failed to read launch config: {}", path.display()))?;
    let bundle = serde_json::from_str(&json)
        .with_context(|| format!("failed to parse launch config: {}", path.display()))?;

    Ok(Some(bundle))
}

/// Check if a launch config exists for an agent.
pub fn has_launch_config(agent_store: &AgentStore, agent_id: &AgentId) -> bool {
    launch_config_path(agent_store, agent_id).exists()
}

/// Delete a launch config for an agent.
pub fn delete_launch_config(
    agent_store: &AgentStore,
    agent_id: &AgentId,
) -> Result<()> {
    let path = launch_config_path(agent_store, agent_id);

    if path.exists() {
        fs::remove_file(&path)
            .with_context(|| format!("failed to delete launch config: {}", path.display()))?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use tempfile::TempDir;

    use crate::agent_runtime::AgentId;
    use crate::launch_config::{
        save_launch_config, load_launch_config, has_launch_config, delete_launch_config,
        AgentLaunchBundle, LaunchInputSpec, LaunchSourceMode,
        ResolvedLaunchSpec,
    };
    use crate::provider::ProviderKind;
    use crate::workplace_store::WorkplaceStore;

    fn create_test_workplace() -> (TempDir, WorkplaceStore) {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().to_path_buf();
        let workplace = WorkplaceStore::for_cwd(&path).expect("workplace");
        (temp_dir, workplace)
    }

    #[test]
    fn test_save_and_load_launch_config() {
        let (_temp_dir, workplace) = create_test_workplace();
        let agent_store = crate::agent_store::AgentStore::new(workplace);
        let agent_id = AgentId::new("test-agent");

        // Create a bundle
        let work_input = LaunchInputSpec::new(ProviderKind::Claude);
        let decision_input = LaunchInputSpec::new(ProviderKind::Codex);
        let work_resolved = ResolvedLaunchSpec::new(
            ProviderKind::Claude,
            "/usr/bin/claude".to_string(),
            BTreeMap::new(),
            vec![],
            LaunchSourceMode::HostDefault,
        );
        let decision_resolved = ResolvedLaunchSpec::new(
            ProviderKind::Codex,
            "/usr/bin/codex".to_string(),
            BTreeMap::new(),
            vec![],
            LaunchSourceMode::HostDefault,
        );
        let bundle = AgentLaunchBundle::asymmetric(work_input, work_resolved, decision_input, decision_resolved);

        // Save
        let path = save_launch_config(&agent_store, &agent_id, &bundle).unwrap();
        assert!(path.exists());

        // Load
        let loaded = load_launch_config(&agent_store, &agent_id).unwrap();
        assert!(loaded.is_some());
        let loaded_bundle = loaded.unwrap();
        assert_eq!(loaded_bundle.work_input.provider, ProviderKind::Claude);
        assert_eq!(loaded_bundle.decision_input.provider, ProviderKind::Codex);
    }

    #[test]
    fn test_load_nonexistent() {
        let (_temp_dir, workplace) = create_test_workplace();
        let agent_store = crate::agent_store::AgentStore::new(workplace);
        let agent_id = AgentId::new("nonexistent-agent");

        let result = load_launch_config(&agent_store, &agent_id).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_has_launch_config() {
        let (_temp_dir, workplace) = create_test_workplace();
        let agent_store = crate::agent_store::AgentStore::new(workplace);
        let agent_id = AgentId::new("test-agent");

        assert!(!has_launch_config(&agent_store, &agent_id));

        let work_input = LaunchInputSpec::new(ProviderKind::Claude);
        let work_resolved = ResolvedLaunchSpec::new(
            ProviderKind::Claude,
            "/usr/bin/claude".to_string(),
            BTreeMap::new(),
            vec![],
            LaunchSourceMode::HostDefault,
        );
        let bundle = AgentLaunchBundle::symmetric(ProviderKind::Claude, work_input, work_resolved);
        save_launch_config(&agent_store, &agent_id, &bundle).unwrap();

        assert!(has_launch_config(&agent_store, &agent_id));
    }

    #[test]
    fn test_delete_launch_config() {
        let (_temp_dir, workplace) = create_test_workplace();
        let agent_store = crate::agent_store::AgentStore::new(workplace);
        let agent_id = AgentId::new("test-agent");

        let work_input = LaunchInputSpec::new(ProviderKind::Claude);
        let work_resolved = ResolvedLaunchSpec::new(
            ProviderKind::Claude,
            "/usr/bin/claude".to_string(),
            BTreeMap::new(),
            vec![],
            LaunchSourceMode::HostDefault,
        );
        let bundle = AgentLaunchBundle::symmetric(ProviderKind::Claude, work_input, work_resolved);
        save_launch_config(&agent_store, &agent_id, &bundle).unwrap();
        assert!(has_launch_config(&agent_store, &agent_id));

        delete_launch_config(&agent_store, &agent_id).unwrap();
        assert!(!has_launch_config(&agent_store, &agent_id));
    }
}