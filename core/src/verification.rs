use std::path::Path;
use std::process::Command;

use serde::Deserialize;
use serde::Serialize;

use crate::backlog::TaskItem;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum VerificationCheck {
    AssistantOutputNonEmpty,
    CargoCheck,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VerificationPlan {
    pub checks: Vec<VerificationCheck>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum VerificationOutcome {
    Passed,
    Failed,
    NotRunnable,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VerificationResult {
    pub outcome: VerificationOutcome,
    pub evidence: Vec<String>,
    pub summary: String,
}

pub fn build_verification_plan(cwd: &Path, _task: &TaskItem) -> VerificationPlan {
    let mut checks = vec![VerificationCheck::AssistantOutputNonEmpty];
    if cwd.join("Cargo.toml").exists() {
        checks.push(VerificationCheck::CargoCheck);
    }
    VerificationPlan { checks }
}

pub fn execute_verification(
    plan: &VerificationPlan,
    cwd: &Path,
    assistant_summary: Option<&str>,
) -> VerificationResult {
    if plan.checks.is_empty() {
        return VerificationResult {
            outcome: VerificationOutcome::NotRunnable,
            evidence: Vec::new(),
            summary: "no verification checks available".to_string(),
        };
    }

    let mut evidence = Vec::new();
    let mut failed = false;

    for check in &plan.checks {
        match check {
            VerificationCheck::AssistantOutputNonEmpty => {
                let ok = assistant_summary.is_some_and(|text| !text.trim().is_empty());
                evidence.push(format!(
                    "assistant_output_nonempty={}",
                    if ok { "pass" } else { "fail" }
                ));
                if !ok {
                    failed = true;
                }
            }
            VerificationCheck::CargoCheck => {
                let output = Command::new("cargo").arg("check").current_dir(cwd).output();
                match output {
                    Ok(output) => {
                        if output.status.success() {
                            evidence.push("cargo_check=pass".to_string());
                        } else {
                            failed = true;
                            let stderr = String::from_utf8_lossy(&output.stderr);
                            evidence.push(format!(
                                "cargo_check=fail ({})",
                                stderr.lines().next().unwrap_or("unknown error")
                            ));
                        }
                    }
                    Err(err) => {
                        failed = true;
                        evidence.push(format!("cargo_check=error ({err})"));
                    }
                }
            }
        }
    }

    let outcome = if failed {
        VerificationOutcome::Failed
    } else {
        VerificationOutcome::Passed
    };
    let summary = match outcome {
        VerificationOutcome::Passed => "verification passed".to_string(),
        VerificationOutcome::Failed => "verification failed".to_string(),
        VerificationOutcome::NotRunnable => "verification not runnable".to_string(),
    };

    VerificationResult {
        outcome,
        evidence,
        summary,
    }
}

#[cfg(test)]
mod tests {
    use super::VerificationOutcome;
    use super::build_verification_plan;
    use super::execute_verification;
    use crate::backlog::TaskItem;
    use crate::backlog::TaskStatus;

    #[test]
    fn verification_passes_when_assistant_summary_exists() {
        let task = TaskItem {
            id: "task-1".to_string(),
            todo_id: "todo-1".to_string(),
            objective: "write summary".to_string(),
            scope: ".".to_string(),
            constraints: Vec::new(),
            verification_plan: Vec::new(),
            status: TaskStatus::Running,
            result_summary: None,
        };

        let plan = build_verification_plan(std::path::Path::new("."), &task);
        let result = execute_verification(&plan, std::path::Path::new("."), Some("done"));

        assert_eq!(result.outcome, VerificationOutcome::Passed);
    }

    #[test]
    fn verification_fails_when_assistant_summary_is_missing() {
        let task = TaskItem {
            id: "task-1".to_string(),
            todo_id: "todo-1".to_string(),
            objective: "write summary".to_string(),
            scope: ".".to_string(),
            constraints: Vec::new(),
            verification_plan: Vec::new(),
            status: TaskStatus::Running,
            result_summary: None,
        };

        let plan = build_verification_plan(std::path::Path::new("."), &task);
        let result = execute_verification(&plan, std::path::Path::new("."), None);

        assert_eq!(result.outcome, VerificationOutcome::Failed);
    }
}
