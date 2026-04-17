//! Global Configuration Framework
//!
//! Provides global configuration storage independent of workplace-specific data.
//! Configuration files are stored in ~/.agile-agent/ directory.

use std::env;
use std::fs;
use std::path::PathBuf;

use anyhow::Context;
use anyhow::Result;
use dirs::home_dir;
use serde::Deserialize;
use serde::Serialize;

use crate::agent_role::AgentRole;
use crate::logging;
use crate::runtime_mode::RuntimeMode;

// ============================================================================
// Default value functions for serde defaults
// ============================================================================

fn default_provider() -> String {
    "claude".to_string()
}

fn default_log_level() -> String {
    "info".to_string()
}

fn default_base_url() -> String {
    "https://api.openai.com/v1".to_string()
}

fn default_simple_model() -> String {
    "gpt-4o-mini".to_string()
}

fn default_thinking_model() -> String {
    "gpt-4o".to_string()
}

fn default_timeout() -> u64 {
    60
}

fn default_engine_type() -> String {
    "tiered".to_string()
}

fn default_reflection_threshold() -> u32 {
    3
}

fn default_human_decision_timeout() -> u64 {
    300
}

fn default_max_retry() -> u32 {
    3
}

fn default_max_agents() -> usize {
    10
}

fn default_true() -> bool {
    true
}

// ============================================================================
// Configuration Structures
// ============================================================================

/// Main global configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlobalConfig {
    /// Default provider to use (claude, codex, mock)
    #[serde(default = "default_provider")]
    pub default_provider: String,

    /// Runtime mode (single_agent, multi_agent)
    #[serde(default)]
    pub runtime_mode: RuntimeMode,

    /// Log level (debug, info, warn, error)
    #[serde(default = "default_log_level")]
    pub log_level: String,

    /// Configuration version
    #[serde(default = "default_config_version")]
    pub version: String,
}

fn default_config_version() -> String {
    "1.0".to_string()
}

impl Default for GlobalConfig {
    fn default() -> Self {
        Self {
            default_provider: default_provider(),
            runtime_mode: RuntimeMode::default(),
            log_level: default_log_level(),
            version: default_config_version(),
        }
    }
}

/// LLM provider configuration file format
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmConfigFile {
    /// OpenAI API key (can be null, will use env var)
    #[serde(default)]
    pub api_key: Option<String>,

    /// Base URL for OpenAI API
    #[serde(default = "default_base_url")]
    pub base_url: String,

    /// Simple/fast model name
    #[serde(default = "default_simple_model")]
    pub simple_model: String,

    /// Thinking/complex model name
    #[serde(default = "default_thinking_model")]
    pub thinking_model: String,

    /// Request timeout in seconds
    #[serde(default = "default_timeout")]
    pub timeout_secs: u64,
}

impl Default for LlmConfigFile {
    fn default() -> Self {
        Self {
            api_key: None,
            base_url: default_base_url(),
            simple_model: default_simple_model(),
            thinking_model: default_thinking_model(),
            timeout_secs: default_timeout(),
        }
    }
}

/// Decision layer configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecisionConfig {
    /// Engine type (tiered, llm, cli, rule, mock)
    #[serde(default = "default_engine_type")]
    pub engine_type: String,

    /// Enable auto-approval for safe actions
    #[serde(default = "default_true")]
    pub auto_approval_enabled: bool,

    /// Number of consecutive similar outputs before reflection
    #[serde(default = "default_reflection_threshold")]
    pub reflection_threshold: u32,

    /// Timeout for human decision prompts (seconds)
    #[serde(default = "default_human_decision_timeout")]
    pub human_decision_timeout_secs: u64,

    /// Maximum retry attempts for error recovery
    #[serde(default = "default_max_retry")]
    pub max_retry_attempts: u32,
}

impl Default for DecisionConfig {
    fn default() -> Self {
        Self {
            engine_type: default_engine_type(),
            auto_approval_enabled: true,
            reflection_threshold: default_reflection_threshold(),
            human_decision_timeout_secs: default_human_decision_timeout(),
            max_retry_attempts: default_max_retry(),
        }
    }
}

/// Multi-agent configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MultiAgentConfig {
    /// Maximum number of concurrent agents
    #[serde(default = "default_max_agents")]
    pub max_agents: usize,

    /// Default role for spawned agents
    #[serde(default)]
    pub default_role: AgentRole,

    /// Enable overview agent (ProductOwner) for coordination
    #[serde(default = "default_true")]
    pub overview_agent_enabled: bool,

    /// Spawn agents on startup (for multi-agent mode)
    #[serde(default)]
    pub spawn_on_startup: bool,
}

impl Default for MultiAgentConfig {
    fn default() -> Self {
        Self {
            max_agents: default_max_agents(),
            default_role: AgentRole::default(),
            overview_agent_enabled: true,
            spawn_on_startup: false,
        }
    }
}

// ============================================================================
// Global Config Store
// ============================================================================

/// Global configuration store
///
/// Manages configuration files in ~/.agile-agent/ directory.
pub struct GlobalConfigStore {
    path: PathBuf,
}

impl GlobalConfigStore {
    /// Create a new global config store
    ///
    /// Uses ~/.agile-agent/ as the base directory.
    pub fn new() -> Result<Self> {
        let home = home_dir().context("home directory is unavailable")?;
        let path = home.join(".agile-agent");
        Ok(Self { path })
    }

    /// Create with a custom path (for testing)
    pub fn for_path(path: PathBuf) -> Self {
        Self { path }
    }

    /// Ensure the config directory exists
    pub fn ensure(&self) -> Result<()> {
        fs::create_dir_all(&self.path).with_context(|| {
            format!("failed to create config directory {}", self.path.display())
        })?;
        logging::debug_event(
            "global_config.ensure",
            "ensured config directory",
            serde_json::json!({
                "path": self.path.display().to_string(),
            }),
        );
        Ok(())
    }

    /// Get the base path for config files
    pub fn path(&self) -> &PathBuf {
        &self.path
    }

    // ========================================================================
    // Config File Paths
    // ========================================================================

    /// Path to main config file
    pub fn config_path(&self) -> PathBuf {
        self.path.join("config.json")
    }

    /// Path to LLM config file
    pub fn llm_path(&self) -> PathBuf {
        self.path.join("llm.json")
    }

    /// Path to decision config file
    pub fn decision_path(&self) -> PathBuf {
        self.path.join("decision.json")
    }

    /// Path to multi-agent config file
    pub fn multi_agent_path(&self) -> PathBuf {
        self.path.join("multi_agent.json")
    }

    /// Path to prompts config file
    pub fn prompts_path(&self) -> PathBuf {
        self.path.join("prompts.json")
    }

    // ========================================================================
    // GlobalConfig Load/Save
    // ========================================================================

    /// Load global config, create default if not exists
    pub fn load_global_config(&self) -> Result<GlobalConfig> {
        let path = self.config_path();
        if !path.exists() {
            logging::debug_event(
                "global_config.load_global.not_found",
                "config file not found, using defaults",
                serde_json::json!({
                    "path": path.display().to_string(),
                }),
            );
            return Ok(GlobalConfig::default());
        }

        let payload = fs::read_to_string(&path)
            .with_context(|| format!("failed to read {}", path.display()))?;
        let config: GlobalConfig = serde_json::from_str(&payload)
            .with_context(|| format!("failed to parse {}", path.display()))?;
        logging::debug_event(
            "global_config.load_global",
            "loaded global config",
            serde_json::json!({
                "path": path.display().to_string(),
                "default_provider": config.default_provider,
            }),
        );
        Ok(config)
    }

    /// Save global config
    pub fn save_global_config(&self, config: &GlobalConfig) -> Result<()> {
        self.ensure()?;
        let path = self.config_path();
        let payload =
            serde_json::to_string_pretty(config).context("failed to serialize global config")?;
        fs::write(&path, payload).with_context(|| format!("failed to write {}", path.display()))?;
        logging::debug_event(
            "global_config.save_global",
            "saved global config",
            serde_json::json!({
                "path": path.display().to_string(),
            }),
        );
        Ok(())
    }

    // ========================================================================
    // LlmConfigFile Load/Save
    // ========================================================================

    /// Load LLM config, create default if not exists
    ///
    /// Environment variables override file values:
    /// - OPENAI_API_KEY
    /// - OPENAI_BASE_URL
    /// - OPENAI_SIMPLE_MODEL
    /// - OPENAI_THINKING_MODEL
    pub fn load_llm_config(&self) -> Result<LlmConfigFile> {
        let path = self.llm_path();
        let mut config = if path.exists() {
            let payload = fs::read_to_string(&path)
                .with_context(|| format!("failed to read {}", path.display()))?;
            serde_json::from_str(&payload)
                .with_context(|| format!("failed to parse {}", path.display()))?
        } else {
            LlmConfigFile::default()
        };

        // Environment variable overrides
        if let Ok(key) = env::var("OPENAI_API_KEY") {
            config.api_key = Some(key);
        }
        if let Ok(url) = env::var("OPENAI_BASE_URL") {
            config.base_url = url;
        }
        if let Ok(model) = env::var("OPENAI_SIMPLE_MODEL") {
            config.simple_model = model;
        }
        if let Ok(model) = env::var("OPENAI_THINKING_MODEL") {
            config.thinking_model = model;
        }

        logging::debug_event(
            "global_config.load_llm",
            "loaded llm config",
            serde_json::json!({
                "path": path.display().to_string(),
                "simple_model": config.simple_model,
            }),
        );
        Ok(config)
    }

    /// Save LLM config
    pub fn save_llm_config(&self, config: &LlmConfigFile) -> Result<()> {
        self.ensure()?;
        let path = self.llm_path();
        let payload =
            serde_json::to_string_pretty(config).context("failed to serialize llm config")?;
        fs::write(&path, payload).with_context(|| format!("failed to write {}", path.display()))?;
        logging::debug_event(
            "global_config.save_llm",
            "saved llm config",
            serde_json::json!({
                "path": path.display().to_string(),
            }),
        );
        Ok(())
    }

    // ========================================================================
    // DecisionConfig Load/Save
    // ========================================================================

    /// Load decision config, create default if not exists
    pub fn load_decision_config(&self) -> Result<DecisionConfig> {
        let path = self.decision_path();
        if !path.exists() {
            logging::debug_event(
                "global_config.load_decision.not_found",
                "decision config not found, using defaults",
                serde_json::json!({
                    "path": path.display().to_string(),
                }),
            );
            return Ok(DecisionConfig::default());
        }

        let payload = fs::read_to_string(&path)
            .with_context(|| format!("failed to read {}", path.display()))?;
        let config: DecisionConfig = serde_json::from_str(&payload)
            .with_context(|| format!("failed to parse {}", path.display()))?;
        logging::debug_event(
            "global_config.load_decision",
            "loaded decision config",
            serde_json::json!({
                "path": path.display().to_string(),
                "engine_type": config.engine_type,
            }),
        );
        Ok(config)
    }

    /// Save decision config
    pub fn save_decision_config(&self, config: &DecisionConfig) -> Result<()> {
        self.ensure()?;
        let path = self.decision_path();
        let payload =
            serde_json::to_string_pretty(config).context("failed to serialize decision config")?;
        fs::write(&path, payload).with_context(|| format!("failed to write {}", path.display()))?;
        logging::debug_event(
            "global_config.save_decision",
            "saved decision config",
            serde_json::json!({
                "path": path.display().to_string(),
            }),
        );
        Ok(())
    }

    // ========================================================================
    // MultiAgentConfig Load/Save
    // ========================================================================

    /// Load multi-agent config, create default if not exists
    pub fn load_multi_agent_config(&self) -> Result<MultiAgentConfig> {
        let path = self.multi_agent_path();
        if !path.exists() {
            logging::debug_event(
                "global_config.load_multi_agent.not_found",
                "multi-agent config not found, using defaults",
                serde_json::json!({
                    "path": path.display().to_string(),
                }),
            );
            return Ok(MultiAgentConfig::default());
        }

        let payload = fs::read_to_string(&path)
            .with_context(|| format!("failed to read {}", path.display()))?;
        let config: MultiAgentConfig = serde_json::from_str(&payload)
            .with_context(|| format!("failed to parse {}", path.display()))?;
        logging::debug_event(
            "global_config.load_multi_agent",
            "loaded multi-agent config",
            serde_json::json!({
                "path": path.display().to_string(),
                "max_agents": config.max_agents,
            }),
        );
        Ok(config)
    }

    /// Save multi-agent config
    pub fn save_multi_agent_config(&self, config: &MultiAgentConfig) -> Result<()> {
        self.ensure()?;
        let path = self.multi_agent_path();
        let payload = serde_json::to_string_pretty(config)
            .context("failed to serialize multi-agent config")?;
        fs::write(&path, payload).with_context(|| format!("failed to write {}", path.display()))?;
        logging::debug_event(
            "global_config.save_multi_agent",
            "saved multi-agent config",
            serde_json::json!({
                "path": path.display().to_string(),
            }),
        );
        Ok(())
    }

    // ========================================================================
    // PromptsConfig Load/Save (for agent-decision crate)
    // ========================================================================

    /// Load prompts config JSON as raw value
    ///
    /// Returns raw JSON value that can be used to construct PromptConfig
    /// in agent-decision crate.
    pub fn load_prompts_config_raw(&self) -> Result<serde_json::Value> {
        let path = self.prompts_path();
        if !path.exists() {
            logging::debug_event(
                "global_config.load_prompts.not_found",
                "prompts config not found, using defaults",
                serde_json::json!({
                    "path": path.display().to_string(),
                }),
            );
            return Ok(serde_json::json!({}));
        }

        let payload = fs::read_to_string(&path)
            .with_context(|| format!("failed to read {}", path.display()))?;
        let config: serde_json::Value = serde_json::from_str(&payload)
            .with_context(|| format!("failed to parse {}", path.display()))?;
        logging::debug_event(
            "global_config.load_prompts",
            "loaded prompts config",
            serde_json::json!({
                "path": path.display().to_string(),
            }),
        );
        Ok(config)
    }

    /// Save prompts config JSON
    pub fn save_prompts_config_raw(&self, config: &serde_json::Value) -> Result<()> {
        self.ensure()?;
        let path = self.prompts_path();
        let payload =
            serde_json::to_string_pretty(config).context("failed to serialize prompts config")?;
        fs::write(&path, payload).with_context(|| format!("failed to write {}", path.display()))?;
        logging::debug_event(
            "global_config.save_prompts",
            "saved prompts config",
            serde_json::json!({
                "path": path.display().to_string(),
            }),
        );
        Ok(())
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn global_config_defaults() {
        let config = GlobalConfig::default();
        assert_eq!(config.default_provider, "claude");
        assert_eq!(config.runtime_mode, RuntimeMode::SingleAgent);
        assert_eq!(config.log_level, "info");
        assert_eq!(config.version, "1.0");
    }

    #[test]
    fn llm_config_defaults() {
        let config = LlmConfigFile::default();
        assert!(config.api_key.is_none());
        assert_eq!(config.base_url, "https://api.openai.com/v1");
        assert_eq!(config.simple_model, "gpt-4o-mini");
        assert_eq!(config.thinking_model, "gpt-4o");
        assert_eq!(config.timeout_secs, 60);
    }

    #[test]
    fn decision_config_defaults() {
        let config = DecisionConfig::default();
        assert_eq!(config.engine_type, "tiered");
        assert!(config.auto_approval_enabled);
        assert_eq!(config.reflection_threshold, 3);
        assert_eq!(config.human_decision_timeout_secs, 300);
        assert_eq!(config.max_retry_attempts, 3);
    }

    #[test]
    fn multi_agent_config_defaults() {
        let config = MultiAgentConfig::default();
        assert_eq!(config.max_agents, 10);
        assert_eq!(config.default_role, AgentRole::Developer);
        assert!(config.overview_agent_enabled);
        assert!(!config.spawn_on_startup);
    }

    #[test]
    fn store_creates_directory() {
        let temp = TempDir::new().expect("tempdir");
        let store = GlobalConfigStore::for_path(temp.path().join(".agile-agent"));

        store.ensure().expect("ensure");

        assert!(store.path().exists());
    }

    #[test]
    fn global_config_save_load_roundtrip() {
        let temp = TempDir::new().expect("tempdir");
        let store = GlobalConfigStore::for_path(temp.path().join(".agile-agent"));
        store.ensure().expect("ensure");

        let config = GlobalConfig {
            default_provider: "codex".to_string(),
            runtime_mode: RuntimeMode::MultiAgent,
            log_level: "debug".to_string(),
            version: "1.0".to_string(),
        };

        store.save_global_config(&config).expect("save");
        let loaded = store.load_global_config().expect("load");

        assert_eq!(loaded.default_provider, "codex");
        assert_eq!(loaded.runtime_mode, RuntimeMode::MultiAgent);
        assert_eq!(loaded.log_level, "debug");
    }

    #[test]
    fn llm_config_save_load_roundtrip() {
        let temp = TempDir::new().expect("tempdir");
        let store = GlobalConfigStore::for_path(temp.path().join(".agile-agent"));
        store.ensure().expect("ensure");

        let config = LlmConfigFile {
            api_key: Some("test-key".to_string()),
            base_url: "https://custom.api.com/v1".to_string(),
            simple_model: "gpt-3.5-turbo".to_string(),
            thinking_model: "gpt-4".to_string(),
            timeout_secs: 120,
        };

        store.save_llm_config(&config).expect("save");
        // Clear env vars to test file load (unsafe in edition 2024)
        unsafe {
            std::env::remove_var("OPENAI_API_KEY");
            std::env::remove_var("OPENAI_BASE_URL");
        }
        let loaded = store.load_llm_config().expect("load");

        assert_eq!(loaded.api_key, Some("test-key".to_string()));
        assert_eq!(loaded.base_url, "https://custom.api.com/v1");
        assert_eq!(loaded.simple_model, "gpt-3.5-turbo");
    }

    #[test]
    fn llm_config_env_override() {
        let temp = TempDir::new().expect("tempdir");
        let store = GlobalConfigStore::for_path(temp.path().join(".agile-agent"));
        store.ensure().expect("ensure");

        // Save config with some values
        let config = LlmConfigFile {
            api_key: Some("file-key".to_string()),
            base_url: "https://file.api.com/v1".to_string(),
            simple_model: "gpt-3.5-turbo".to_string(),
            thinking_model: "gpt-4".to_string(),
            timeout_secs: 60,
        };
        store.save_llm_config(&config).expect("save");

        // Set env vars (unsafe in edition 2024)
        unsafe {
            std::env::set_var("OPENAI_API_KEY", "env-key");
            std::env::set_var("OPENAI_BASE_URL", "https://env.api.com/v1");
        }

        let loaded = store.load_llm_config().expect("load");

        // Env vars should override
        assert_eq!(loaded.api_key, Some("env-key".to_string()));
        assert_eq!(loaded.base_url, "https://env.api.com/v1");

        // Cleanup (unsafe in edition 2024)
        unsafe {
            std::env::remove_var("OPENAI_API_KEY");
            std::env::remove_var("OPENAI_BASE_URL");
        }
    }

    #[test]
    fn decision_config_save_load_roundtrip() {
        let temp = TempDir::new().expect("tempdir");
        let store = GlobalConfigStore::for_path(temp.path().join(".agile-agent"));
        store.ensure().expect("ensure");

        let config = DecisionConfig {
            engine_type: "llm".to_string(),
            auto_approval_enabled: false,
            reflection_threshold: 5,
            human_decision_timeout_secs: 600,
            max_retry_attempts: 5,
        };

        store.save_decision_config(&config).expect("save");
        let loaded = store.load_decision_config().expect("load");

        assert_eq!(loaded.engine_type, "llm");
        assert!(!loaded.auto_approval_enabled);
        assert_eq!(loaded.reflection_threshold, 5);
    }

    #[test]
    fn multi_agent_config_save_load_roundtrip() {
        let temp = TempDir::new().expect("tempdir");
        let store = GlobalConfigStore::for_path(temp.path().join(".agile-agent"));
        store.ensure().expect("ensure");

        let config = MultiAgentConfig {
            max_agents: 5,
            default_role: AgentRole::ScrumMaster,
            overview_agent_enabled: false,
            spawn_on_startup: true,
        };

        store.save_multi_agent_config(&config).expect("save");
        let loaded = store.load_multi_agent_config().expect("load");

        assert_eq!(loaded.max_agents, 5);
        assert_eq!(loaded.default_role, AgentRole::ScrumMaster);
        assert!(!loaded.overview_agent_enabled);
        assert!(loaded.spawn_on_startup);
    }

    #[test]
    fn config_file_not_exists_returns_default() {
        let temp = TempDir::new().expect("tempdir");
        let store = GlobalConfigStore::for_path(temp.path().join(".agile-agent"));

        // Don't ensure/create directory
        let global = store.load_global_config().expect("load global");
        let decision = store.load_decision_config().expect("load decision");
        let multi = store.load_multi_agent_config().expect("load multi");

        // Should return defaults
        assert_eq!(global.default_provider, "claude");
        assert_eq!(decision.engine_type, "tiered");
        assert_eq!(multi.max_agents, 10);
    }

    #[test]
    fn global_config_serialization() {
        let config = GlobalConfig {
            default_provider: "mock".to_string(),
            runtime_mode: RuntimeMode::MultiAgent,
            log_level: "warn".to_string(),
            version: "2.0".to_string(),
        };

        let json = serde_json::to_string(&config).expect("serialize");
        assert!(json.contains("mock"));
        assert!(json.contains("multi_agent"));

        let parsed: GlobalConfig = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(parsed.default_provider, "mock");
    }

    #[test]
    fn agent_role_in_config() {
        let config = MultiAgentConfig {
            max_agents: 3,
            default_role: AgentRole::ProductOwner,
            overview_agent_enabled: true,
            spawn_on_startup: false,
        };

        let json = serde_json::to_string(&config).expect("serialize");
        assert!(json.contains("product_owner"));

        let parsed: MultiAgentConfig = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(parsed.default_role, AgentRole::ProductOwner);
    }

    #[test]
    fn prompts_config_save_load_roundtrip() {
        let temp = TempDir::new().expect("tempdir");
        let store = GlobalConfigStore::for_path(temp.path().join(".agile-agent"));
        store.ensure().expect("ensure");

        let config = serde_json::json!({
            "max_reflection_rounds": 3,
            "custom_prompts": {
                "custom_situation": "Custom prompt template"
            }
        });

        store.save_prompts_config_raw(&config).expect("save");
        let loaded = store.load_prompts_config_raw().expect("load");

        assert_eq!(loaded["max_reflection_rounds"], 3);
        assert!(loaded["custom_prompts"]["custom_situation"].is_string());
    }

    #[test]
    fn prompts_config_missing_returns_empty() {
        let temp = TempDir::new().expect("tempdir");
        let store = GlobalConfigStore::for_path(temp.path().join(".agile-agent"));

        // Don't create file
        let loaded = store.load_prompts_config_raw().expect("load");
        assert!(loaded.is_object());
        assert!(loaded.as_object().unwrap().is_empty());
    }
}
