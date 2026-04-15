use std::env;
use std::path::Path;
use std::process::Command;

use chrono::Utc;
use serde::Serialize;

pub const CODEX_PATH_ENV: &str = "AGILE_AGENT_CODEX_PATH";
pub const CLAUDE_PATH_ENV: &str = "AGILE_AGENT_CLAUDE_PATH";

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ProbeReport {
    pub checked_at: String,
    pub providers: Vec<ProviderProbe>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ProviderProbe {
    pub name: String,
    pub available: bool,
    pub path: Option<String>,
    pub version: Option<String>,
    pub protocol: String,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Copy)]
struct ProviderSpec {
    name: &'static str,
    protocol: &'static str,
    env_key: &'static str,
    default_command: &'static str,
}

const PROVIDERS: [ProviderSpec; 2] = [
    ProviderSpec {
        name: "codex",
        protocol: "app-server-stdio",
        env_key: CODEX_PATH_ENV,
        default_command: "codex",
    },
    ProviderSpec {
        name: "claude",
        protocol: "stream-json",
        env_key: CLAUDE_PATH_ENV,
        default_command: "claude",
    },
];

pub fn probe_report() -> ProbeReport {
    ProbeReport {
        checked_at: Utc::now().to_rfc3339(),
        providers: PROVIDERS.iter().map(probe_provider).collect(),
    }
}

pub fn render_doctor_text(report: &ProbeReport) -> String {
    let mut lines = vec![
        "agile-agent doctor".to_string(),
        format!("checked_at: {}", report.checked_at),
        String::new(),
    ];

    for provider in &report.providers {
        lines.push(format!("{}:", provider.name));
        lines.push(format!(
            "  available: {}",
            if provider.available { "yes" } else { "no" }
        ));
        lines.push(format!(
            "  path: {}",
            provider.path.as_deref().unwrap_or("-")
        ));
        lines.push(format!(
            "  version: {}",
            provider.version.as_deref().unwrap_or("-")
        ));
        lines.push(format!("  protocol: {}", provider.protocol));
        if let Some(error) = &provider.error {
            lines.push(format!("  error: {error}"));
        }
        lines.push(String::new());
    }

    lines.join("\n")
}

pub fn is_provider_available(name: &str) -> bool {
    probe_report()
        .providers
        .into_iter()
        .find(|provider| provider.name == name)
        .map(|provider| provider.available)
        .unwrap_or(false)
}

pub fn has_any_real_provider() -> bool {
    probe_report()
        .providers
        .into_iter()
        .any(|provider| provider.available)
}

fn probe_provider(spec: &ProviderSpec) -> ProviderProbe {
    let configured_command = env::var(spec.env_key)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| spec.default_command.to_string());

    probe_provider_with_candidate(spec, &configured_command)
}

fn probe_provider_with_candidate(spec: &ProviderSpec, candidate: &str) -> ProviderProbe {
    let resolved = which::which(candidate);
    match resolved {
        Ok(path) => {
            let version_result = detect_version(&path);
            let (version, error) = match version_result {
                Ok(version) => (Some(version), None),
                Err(err) => (None, Some(format!("failed to read version: {err}"))),
            };

            ProviderProbe {
                name: spec.name.to_string(),
                available: true,
                path: Some(path.display().to_string()),
                version,
                protocol: spec.protocol.to_string(),
                error,
            }
        }
        Err(err) => ProviderProbe {
            name: spec.name.to_string(),
            available: false,
            path: None,
            version: None,
            protocol: spec.protocol.to_string(),
            error: Some(format!("not found: {candidate} ({err})")),
        },
    }
}

fn detect_version(path: &Path) -> Result<String, String> {
    let output = Command::new(path)
        .arg("--version")
        .output()
        .map_err(|err| err.to_string())?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let reason = stderr.trim();
        return if reason.is_empty() {
            Err(format!("process exited with status {}", output.status))
        } else {
            Err(reason.to_string())
        };
    }

    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if stdout.is_empty() {
        Err("empty version output".to_string())
    } else {
        Ok(stdout)
    }
}

#[cfg(test)]
mod tests {
    use super::ProviderSpec;
    use super::detect_version;
    use super::probe_provider_with_candidate;
    use super::probe_report;
    use super::render_doctor_text;
    use std::fs;
    use std::path::PathBuf;

    use tempfile::TempDir;

    #[cfg(unix)]
    fn write_fake_executable(dir: &TempDir, name: &str, version_output: &str) -> PathBuf {
        use std::os::unix::fs::PermissionsExt;

        // Use unique name to avoid race conditions in parallel tests
        let unique_name = format!("{}-{}", name, std::process::id());
        let path = dir.path().join(&unique_name);
        let script = format!("#!/bin/sh\necho \"{version_output}\"\n");
        fs::write(&path, script).expect("write fake executable");
        let mut permissions = fs::metadata(&path).expect("metadata").permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&path, permissions).expect("set permissions");
        path
    }

    #[cfg(unix)]
    #[test]
    fn detect_version_reads_fake_binary_output() {
        let temp = TempDir::new().expect("tempdir");
        let executable = write_fake_executable(&temp, "fake-codex", "codex-cli 9.9.9");

        let version = detect_version(&executable).expect("read version");

        assert_eq!(version, "codex-cli 9.9.9");
    }

    #[test]
    fn unavailable_provider_has_stable_shape() {
        let spec = ProviderSpec {
            name: "claude",
            protocol: "stream-json",
            env_key: "IGNORED",
            default_command: "claude",
        };

        let probe = probe_provider_with_candidate(&spec, "definitely-not-a-real-binary-name");

        assert_eq!(probe.name, "claude");
        assert!(!probe.available);
        assert_eq!(probe.path, None);
        assert_eq!(probe.version, None);
        assert_eq!(probe.protocol, "stream-json");
        assert!(probe.error.is_some());
    }

    #[test]
    fn probe_report_serializes_with_providers_array() {
        let report = probe_report();
        let json = serde_json::to_value(&report).expect("serialize report");

        assert!(json.get("checked_at").is_some());
        assert!(json.get("providers").is_some());
        assert!(json["providers"].is_array());
    }

    #[test]
    fn doctor_text_mentions_providers() {
        let report = probe_report();
        let rendered = render_doctor_text(&report);

        assert!(rendered.contains("agile-agent doctor"));
        assert!(rendered.contains("codex:"));
        assert!(rendered.contains("claude:"));
    }
}
