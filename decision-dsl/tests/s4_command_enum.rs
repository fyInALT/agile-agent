use decision_dsl::ext::command::*;

// ── DecisionCommand grouped enum ─────────────────────────────────────────────

#[test]
fn decision_command_agent_variant() {
    let cmd = DecisionCommand::Agent(AgentCommand::ApproveAndContinue);
    assert!(matches!(cmd, DecisionCommand::Agent(_)));
}

#[test]
fn decision_command_git_with_worktree() {
    let cmd = DecisionCommand::Git(
        GitCommand::Commit {
            message: "fix bug".into(),
            wip: false,
        },
        Some("wt1".into()),
    );
    assert!(matches!(cmd, DecisionCommand::Git(_, Some(_))));
}

#[test]
fn decision_command_task_variant() {
    let cmd = DecisionCommand::Task(TaskCommand::ConfirmCompletion);
    assert!(matches!(cmd, DecisionCommand::Task(_)));
}

#[test]
fn decision_command_human_variant() {
    let cmd = DecisionCommand::Human(HumanCommand::Escalate {
        reason: "dangerous".into(),
        context: Some("delete operation".into()),
    });
    assert!(matches!(cmd, DecisionCommand::Human(_)));
}

#[test]
fn decision_command_provider_variant() {
    let cmd = DecisionCommand::Provider(ProviderCommand::SwitchProvider {
        provider_type: "claude".into(),
    });
    assert!(matches!(cmd, DecisionCommand::Provider(_)));
}

// ── AgentCommand variants ───────────────────────────────────────────────────

#[test]
fn agent_command_variants() {
    let _ = AgentCommand::ApproveAndContinue;
    let _ = AgentCommand::Reflect { prompt: "review".into() };
    let _ = AgentCommand::SendInstruction {
        prompt: "do X".into(),
        target_agent: "agent-1".into(),
    };
    let _ = AgentCommand::Terminate { reason: "done".into() };
    let _ = AgentCommand::WakeUp;
}

// ── GitCommand variants ─────────────────────────────────────────────────────

#[test]
fn git_command_variants() {
    let _ = GitCommand::Commit {
        message: "feat: auth".into(),
        wip: false,
    };
    let _ = GitCommand::Stash {
        description: "WIP".into(),
        include_untracked: true,
    };
    let _ = GitCommand::Discard;
    let _ = GitCommand::CreateBranch {
        name: "feat/auth".into(),
        base: "main".into(),
    };
    let _ = GitCommand::Rebase { base: "main".into() };
}

// ── TaskCommand variants ────────────────────────────────────────────────────

#[test]
fn task_command_variants() {
    let _ = TaskCommand::ConfirmCompletion;
    let _ = TaskCommand::StopIfComplete {
        reason: "all done".into(),
    };
    let _ = TaskCommand::PrepareStart {
        task_id: "T-1".into(),
        description: "implement auth".into(),
    };
}

// ── HumanCommand variants ──────────────────────────────────────────────────

#[test]
fn human_command_variants() {
    let _ = HumanCommand::Escalate {
        reason: "danger".into(),
        context: Some("deleting production data".into()),
    };
    let _ = HumanCommand::SelectOption {
        option_id: "1".into(),
    };
    let _ = HumanCommand::SkipDecision;
}

// ── ProviderCommand variants ────────────────────────────────────────────────

#[test]
fn provider_command_variants() {
    let _ = ProviderCommand::RetryTool {
        tool_name: "Bash".into(),
        args: Some("echo hi".into()),
        max_attempts: 3,
    };
    let _ = ProviderCommand::SwitchProvider {
        provider_type: "openai".into(),
    };
    let _ = ProviderCommand::SuggestCommit {
        message: "good checkpoint".into(),
        mandatory: false,
        reason: "tests passing".into(),
    };
    let _ = ProviderCommand::PreparePr {
        title: "feat: auth".into(),
        description: "JWT-based auth".into(),
        base: "main".into(),
        draft: false,
    };
}

// ── Serde round-trip tests ──────────────────────────────────────────────────

#[test]
fn yaml_roundtrip_agent_approve_and_continue() {
    let cmd = DecisionCommand::Agent(AgentCommand::ApproveAndContinue);
    let yaml = serde_yaml::to_string(&cmd).unwrap();
    let back: DecisionCommand = serde_yaml::from_str(&yaml).unwrap();
    assert_eq!(cmd, back);
}

#[test]
fn yaml_roundtrip_agent_reflect() {
    let cmd = DecisionCommand::Agent(AgentCommand::Reflect {
        prompt: "review your work".into(),
    });
    let yaml = serde_yaml::to_string(&cmd).unwrap();
    let back: DecisionCommand = serde_yaml::from_str(&yaml).unwrap();
    assert_eq!(cmd, back);
}

#[test]
fn yaml_roundtrip_git_commit() {
    let cmd = DecisionCommand::Git(
        GitCommand::Commit {
            message: "fix: bug".into(),
            wip: true,
        },
        None,
    );
    let yaml = serde_yaml::to_string(&cmd).unwrap();
    let back: DecisionCommand = serde_yaml::from_str(&yaml).unwrap();
    assert_eq!(cmd, back);
}

#[test]
fn yaml_roundtrip_git_stash() {
    let cmd = DecisionCommand::Git(
        GitCommand::Stash {
            description: "WIP".into(),
            include_untracked: true,
        },
        None,
    );
    let yaml = serde_yaml::to_string(&cmd).unwrap();
    let back: DecisionCommand = serde_yaml::from_str(&yaml).unwrap();
    assert_eq!(cmd, back);
}

#[test]
fn yaml_roundtrip_git_discard() {
    let cmd = DecisionCommand::Git(GitCommand::Discard, None);
    let yaml = serde_yaml::to_string(&cmd).unwrap();
    let back: DecisionCommand = serde_yaml::from_str(&yaml).unwrap();
    assert_eq!(cmd, back);
}

#[test]
fn yaml_roundtrip_git_create_branch() {
    let cmd = DecisionCommand::Git(
        GitCommand::CreateBranch {
            name: "feat/auth".into(),
            base: "main".into(),
        },
        None,
    );
    let yaml = serde_yaml::to_string(&cmd).unwrap();
    let back: DecisionCommand = serde_yaml::from_str(&yaml).unwrap();
    assert_eq!(cmd, back);
}

#[test]
fn yaml_roundtrip_git_rebase() {
    let cmd = DecisionCommand::Git(GitCommand::Rebase { base: "main".into() }, None);
    let yaml = serde_yaml::to_string(&cmd).unwrap();
    let back: DecisionCommand = serde_yaml::from_str(&yaml).unwrap();
    assert_eq!(cmd, back);
}

#[test]
fn yaml_roundtrip_human_escalate() {
    let cmd = DecisionCommand::Human(HumanCommand::Escalate {
        reason: "dangerous action".into(),
        context: Some("rm -rf /".into()),
    });
    let yaml = serde_yaml::to_string(&cmd).unwrap();
    let back: DecisionCommand = serde_yaml::from_str(&yaml).unwrap();
    assert_eq!(cmd, back);
}

#[test]
fn yaml_roundtrip_human_select_option() {
    let cmd = DecisionCommand::Human(HumanCommand::SelectOption {
        option_id: "0".into(),
    });
    let yaml = serde_yaml::to_string(&cmd).unwrap();
    let back: DecisionCommand = serde_yaml::from_str(&yaml).unwrap();
    assert_eq!(cmd, back);
}

#[test]
fn yaml_roundtrip_human_skip_decision() {
    let cmd = DecisionCommand::Human(HumanCommand::SkipDecision);
    let yaml = serde_yaml::to_string(&cmd).unwrap();
    let back: DecisionCommand = serde_yaml::from_str(&yaml).unwrap();
    assert_eq!(cmd, back);
}

#[test]
fn yaml_roundtrip_task_confirm_completion() {
    let cmd = DecisionCommand::Task(TaskCommand::ConfirmCompletion);
    let yaml = serde_yaml::to_string(&cmd).unwrap();
    let back: DecisionCommand = serde_yaml::from_str(&yaml).unwrap();
    assert_eq!(cmd, back);
}

#[test]
fn yaml_roundtrip_task_stop_if_complete() {
    let cmd = DecisionCommand::Task(TaskCommand::StopIfComplete {
        reason: "all tasks done".into(),
    });
    let yaml = serde_yaml::to_string(&cmd).unwrap();
    let back: DecisionCommand = serde_yaml::from_str(&yaml).unwrap();
    assert_eq!(cmd, back);
}

#[test]
fn yaml_roundtrip_task_prepare_start() {
    let cmd = DecisionCommand::Task(TaskCommand::PrepareStart {
        task_id: "T-42".into(),
        description: "implement auth".into(),
    });
    let yaml = serde_yaml::to_string(&cmd).unwrap();
    let back: DecisionCommand = serde_yaml::from_str(&yaml).unwrap();
    assert_eq!(cmd, back);
}

#[test]
fn yaml_roundtrip_provider_retry_tool() {
    let cmd = DecisionCommand::Provider(ProviderCommand::RetryTool {
        tool_name: "Bash".into(),
        args: Some("echo hi".into()),
        max_attempts: 3,
    });
    let yaml = serde_yaml::to_string(&cmd).unwrap();
    let back: DecisionCommand = serde_yaml::from_str(&yaml).unwrap();
    assert_eq!(cmd, back);
}

#[test]
fn yaml_roundtrip_provider_switch_provider() {
    let cmd = DecisionCommand::Provider(ProviderCommand::SwitchProvider {
        provider_type: "claude".into(),
    });
    let yaml = serde_yaml::to_string(&cmd).unwrap();
    let back: DecisionCommand = serde_yaml::from_str(&yaml).unwrap();
    assert_eq!(cmd, back);
}

#[test]
fn yaml_roundtrip_provider_suggest_commit() {
    let cmd = DecisionCommand::Provider(ProviderCommand::SuggestCommit {
        message: "good checkpoint".into(),
        mandatory: false,
        reason: "tests passing".into(),
    });
    let yaml = serde_yaml::to_string(&cmd).unwrap();
    let back: DecisionCommand = serde_yaml::from_str(&yaml).unwrap();
    assert_eq!(cmd, back);
}

#[test]
fn yaml_roundtrip_provider_prepare_pr() {
    let cmd = DecisionCommand::Provider(ProviderCommand::PreparePr {
        title: "feat: auth".into(),
        description: "JWT auth".into(),
        base: "main".into(),
        draft: false,
    });
    let yaml = serde_yaml::to_string(&cmd).unwrap();
    let back: DecisionCommand = serde_yaml::from_str(&yaml).unwrap();
    assert_eq!(cmd, back);
}

#[test]
fn yaml_roundtrip_git_with_worktree_path() {
    let cmd = DecisionCommand::Git(
        GitCommand::Commit {
            message: "fix".into(),
            wip: false,
        },
        Some("/path/to/worktree".into()),
    );
    let yaml = serde_yaml::to_string(&cmd).unwrap();
    let back: DecisionCommand = serde_yaml::from_str(&yaml).unwrap();
    assert_eq!(cmd, back);
}

// ── Serde rename verification ──────────────────────────────────────────────

#[test]
fn yaml_agent_reflect_name() {
    let cmd = DecisionCommand::Agent(AgentCommand::Reflect {
        prompt: "review".into(),
    });
    let yaml = serde_yaml::to_string(&cmd).unwrap();
    assert!(yaml.contains("Reflect"));
}

#[test]
fn yaml_agent_send_instruction_rename() {
    let cmd = DecisionCommand::Agent(AgentCommand::SendInstruction {
        prompt: "do it".into(),
        target_agent: "a1".into(),
    });
    let yaml = serde_yaml::to_string(&cmd).unwrap();
    assert!(yaml.contains("SendCustomInstruction"));
    assert!(yaml.contains("prompt"));
    assert!(yaml.contains("target_agent"));
}

#[test]
fn yaml_agent_terminate_rename() {
    let cmd = DecisionCommand::Agent(AgentCommand::Terminate {
        reason: "done".into(),
    });
    let yaml = serde_yaml::to_string(&cmd).unwrap();
    assert!(yaml.contains("TerminateAgent"));
    assert!(yaml.contains("reason"));
}

#[test]
fn yaml_git_commit_uses_is_wip() {
    let cmd = DecisionCommand::Git(
        GitCommand::Commit {
            message: "fix".into(),
            wip: true,
        },
        None,
    );
    let yaml = serde_yaml::to_string(&cmd).unwrap();
    assert!(yaml.contains("CommitChanges"));
    assert!(yaml.contains("is_wip"));
    assert!(yaml.contains("message"));
}

#[test]
fn yaml_git_stash_rename() {
    let cmd = DecisionCommand::Git(
        GitCommand::Stash {
            description: "WIP".into(),
            include_untracked: true,
        },
        None,
    );
    let yaml = serde_yaml::to_string(&cmd).unwrap();
    assert!(yaml.contains("StashChanges"));
    assert!(yaml.contains("description"));
    assert!(yaml.contains("include_untracked"));
}

#[test]
fn yaml_git_discard_rename() {
    let cmd = DecisionCommand::Git(GitCommand::Discard, None);
    let yaml = serde_yaml::to_string(&cmd).unwrap();
    assert!(yaml.contains("DiscardChanges"));
}

#[test]
fn yaml_git_create_branch_rename() {
    let cmd = DecisionCommand::Git(
        GitCommand::CreateBranch {
            name: "feat/auth".into(),
            base: "main".into(),
        },
        None,
    );
    let yaml = serde_yaml::to_string(&cmd).unwrap();
    assert!(yaml.contains("CreateTaskBranch"));
    assert!(yaml.contains("branch_name"));
    assert!(yaml.contains("base_branch"));
}

#[test]
fn yaml_git_rebase_rename() {
    let cmd = DecisionCommand::Git(GitCommand::Rebase { base: "main".into() }, None);
    let yaml = serde_yaml::to_string(&cmd).unwrap();
    assert!(yaml.contains("RebaseToMain"));
    assert!(yaml.contains("base_branch"));
}

#[test]
fn yaml_human_escalate_rename_and_context() {
    let cmd = DecisionCommand::Human(HumanCommand::Escalate {
        reason: "danger".into(),
        context: Some("delete".into()),
    });
    let yaml = serde_yaml::to_string(&cmd).unwrap();
    assert!(yaml.contains("EscalateToHuman"));
    assert!(yaml.contains("reason"));
    assert!(yaml.contains("context"));
}

#[test]
fn yaml_task_prepare_start_rename() {
    let cmd = DecisionCommand::Task(TaskCommand::PrepareStart {
        task_id: "T-1".into(),
        description: "desc".into(),
    });
    let yaml = serde_yaml::to_string(&cmd).unwrap();
    assert!(yaml.contains("PrepareTaskStart"));
    assert!(yaml.contains("task_id"));
    assert!(yaml.contains("description"));
}

#[test]
fn yaml_provider_retry_tool_fields() {
    let cmd = DecisionCommand::Provider(ProviderCommand::RetryTool {
        tool_name: "Bash".into(),
        args: Some("ls".into()),
        max_attempts: 3,
    });
    let yaml = serde_yaml::to_string(&cmd).unwrap();
    assert!(yaml.contains("RetryTool"));
    assert!(yaml.contains("tool_name"));
    assert!(yaml.contains("args"));
    assert!(yaml.contains("max_attempts"));
}

#[test]
fn yaml_provider_switch_provider_type() {
    let cmd = DecisionCommand::Provider(ProviderCommand::SwitchProvider {
        provider_type: "claude".into(),
    });
    let yaml = serde_yaml::to_string(&cmd).unwrap();
    assert!(yaml.contains("SwitchProvider"));
    assert!(yaml.contains("provider_type"));
}

#[test]
fn yaml_provider_suggest_commit_fields() {
    let cmd = DecisionCommand::Provider(ProviderCommand::SuggestCommit {
        message: "msg".into(),
        mandatory: true,
        reason: "r".into(),
    });
    let yaml = serde_yaml::to_string(&cmd).unwrap();
    assert!(yaml.contains("SuggestCommit"));
    assert!(yaml.contains("message"));
    assert!(yaml.contains("mandatory"));
    assert!(yaml.contains("reason"));
}

#[test]
fn yaml_provider_prepare_pr_fields() {
    let cmd = DecisionCommand::Provider(ProviderCommand::PreparePr {
        title: "t".into(),
        description: "d".into(),
        base: "main".into(),
        draft: false,
    });
    let yaml = serde_yaml::to_string(&cmd).unwrap();
    assert!(yaml.contains("PreparePr"));
    assert!(yaml.contains("title"));
    assert!(yaml.contains("description"));
    assert!(yaml.contains("base"));
    assert!(yaml.contains("draft"));
}

// ── Clone + PartialEq ───────────────────────────────────────────────────────

#[test]
fn decision_command_clone_and_eq() {
    let a = DecisionCommand::Agent(AgentCommand::WakeUp);
    let b = a.clone();
    assert_eq!(a, b);
}

#[test]
fn git_command_with_worktree_path() {
    let cmd = DecisionCommand::Git(GitCommand::Rebase { base: "main".into() }, Some("/wt".into()));
    if let DecisionCommand::Git(_, Some(path)) = &cmd {
        assert_eq!(path, "/wt");
    } else {
        panic!("expected Git with worktree path");
    }
}
