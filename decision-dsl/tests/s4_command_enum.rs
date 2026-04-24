use decision_dsl::ext::command::*;


// ── DecisionCommand grouped enum ────────────────────────────────────────────

#[test]
fn decision_command_agent_variant() {
    let cmd = DecisionCommand::Agent(AgentCommand::WakeUp);
    assert!(matches!(cmd, DecisionCommand::Agent(_)));
}

#[test]
fn decision_command_git_with_worktree() {
    let cmd = DecisionCommand::Git(GitCommand::CommitChanges { is_wip: false }, Some("wt1".into()));
    assert!(matches!(cmd, DecisionCommand::Git(_, Some(_))));
}

#[test]
fn decision_command_task_variant() {
    let cmd = DecisionCommand::Task(TaskCommand::PrepareTaskStart);
    assert!(matches!(cmd, DecisionCommand::Task(_)));
}

#[test]
fn decision_command_human_variant() {
    let cmd = DecisionCommand::Human(HumanCommand::EscalateToHuman { reason: "stuck".into() });
    assert!(matches!(cmd, DecisionCommand::Human(_)));
}

#[test]
fn decision_command_provider_variant() {
    let cmd = DecisionCommand::Provider(ProviderCommand::SwitchProvider { provider: "openai".into() });
    assert!(matches!(cmd, DecisionCommand::Provider(_)));
}

// ── AgentCommand variants ───────────────────────────────────────────────────

#[test]
fn agent_command_variants() {
    let _ = AgentCommand::WakeUp;
    let _ = AgentCommand::SendCustomInstruction { content: "do X".into() };
    let _ = AgentCommand::TerminateAgent { status: "done".into() };
    let _ = AgentCommand::RequestSkill { skill: "git".into() };
    let _ = AgentCommand::Transfer { target_agent: "a2".into() };
}

// ── GitCommand variants ─────────────────────────────────────────────────────

#[test]
fn git_command_variants() {
    let _ = GitCommand::CommitChanges { is_wip: true };
    let _ = GitCommand::CreateBranch { branch_name: "feat".into(), base_branch: "main".into() };
    let _ = GitCommand::Rebase { base_branch: "main".into() };
    let _ = GitCommand::Push { force: false };
    let _ = GitCommand::StageAll;
}

// ── TaskCommand variants ────────────────────────────────────────────────────

#[test]
fn task_command_variants() {
    let _ = TaskCommand::PrepareTaskStart;
    let _ = TaskCommand::StartTask { task_id: "T1".into() };
    let _ = TaskCommand::FinishTask { task_id: "T1".into() };
}

// ── HumanCommand variants ───────────────────────────────────────────────────

#[test]
fn human_command_variants() {
    let _ = HumanCommand::EscalateToHuman { reason: "blocked".into() };
    let _ = HumanCommand::SelectOption { options: vec!["A".into(), "B".into()], default: Some(0) };
    let _ = HumanCommand::SkipDecision;
}

// ── ProviderCommand variants ────────────────────────────────────────────────

#[test]
fn provider_command_variants() {
    let _ = ProviderCommand::RetryTool { tool_name: "search".into(), args: vec![] };
    let _ = ProviderCommand::SwitchProvider { provider: "claude".into() };
    let _ = ProviderCommand::SuggestCommit;
    let _ = ProviderCommand::PreparePr { title: "feat".into(), description: "desc".into() };
}

// ── Serde round-trip tests ──────────────────────────────────────────────────

#[test]
fn yaml_roundtrip_agent_wake_up() {
    let cmd = DecisionCommand::Agent(AgentCommand::WakeUp);
    let yaml = serde_yaml::to_string(&cmd).unwrap();
    let back: DecisionCommand = serde_yaml::from_str(&yaml).unwrap();
    assert_eq!(cmd, back);
}

#[test]
fn yaml_roundtrip_git_commit() {
    let cmd = DecisionCommand::Git(GitCommand::CommitChanges { is_wip: true }, None);
    let yaml = serde_yaml::to_string(&cmd).unwrap();
    let back: DecisionCommand = serde_yaml::from_str(&yaml).unwrap();
    assert_eq!(cmd, back);
}

#[test]
fn yaml_roundtrip_human_escalate() {
    let cmd = DecisionCommand::Human(HumanCommand::EscalateToHuman { reason: "test".into() });
    let yaml = serde_yaml::to_string(&cmd).unwrap();
    let back: DecisionCommand = serde_yaml::from_str(&yaml).unwrap();
    assert_eq!(cmd, back);
}

#[test]
fn yaml_roundtrip_provider_switch() {
    let cmd = DecisionCommand::Provider(ProviderCommand::SwitchProvider { provider: "openai".into() });
    let yaml = serde_yaml::to_string(&cmd).unwrap();
    let back: DecisionCommand = serde_yaml::from_str(&yaml).unwrap();
    assert_eq!(cmd, back);
}

#[test]
fn yaml_roundtrip_task_start() {
    let cmd = DecisionCommand::Task(TaskCommand::StartTask { task_id: "42".into() });
    let yaml = serde_yaml::to_string(&cmd).unwrap();
    let back: DecisionCommand = serde_yaml::from_str(&yaml).unwrap();
    assert_eq!(cmd, back);
}

// ── Serde rename verification ───────────────────────────────────────────────

#[test]
fn yaml_uses_full_dsl_names() {
    let cmd = DecisionCommand::Agent(AgentCommand::WakeUp);
    let yaml = serde_yaml::to_string(&cmd).unwrap();
    assert!(yaml.contains("WakeUp"));
}

#[test]
fn yaml_agent_send_instruction_rename() {
    let cmd = DecisionCommand::Agent(AgentCommand::SendCustomInstruction { content: "hi".into() });
    let yaml = serde_yaml::to_string(&cmd).unwrap();
    assert!(yaml.contains("SendCustomInstruction"));
}

#[test]
fn yaml_git_commit_field_rename() {
    let cmd = DecisionCommand::Git(GitCommand::CommitChanges { is_wip: true }, None);
    let yaml = serde_yaml::to_string(&cmd).unwrap();
    assert!(yaml.contains("is_wip"));
}

#[test]
fn yaml_git_create_branch_fields() {
    let cmd = DecisionCommand::Git(GitCommand::CreateBranch { branch_name: "f".into(), base_branch: "m".into() }, None);
    let yaml = serde_yaml::to_string(&cmd).unwrap();
    assert!(yaml.contains("branch_name"));
    assert!(yaml.contains("base_branch"));
}

#[test]
fn yaml_human_escalate_rename() {
    let cmd = DecisionCommand::Human(HumanCommand::EscalateToHuman { reason: "r".into() });
    let yaml = serde_yaml::to_string(&cmd).unwrap();
    assert!(yaml.contains("EscalateToHuman"));
}

#[test]
fn yaml_task_prepare_start_rename() {
    let cmd = DecisionCommand::Task(TaskCommand::PrepareTaskStart);
    let yaml = serde_yaml::to_string(&cmd).unwrap();
    assert!(yaml.contains("PrepareTaskStart"));
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
    let cmd = DecisionCommand::Git(GitCommand::StageAll, Some("/path".into()));
    if let DecisionCommand::Git(_, Some(path)) = cmd {
        assert_eq!(path, "/path");
    } else {
        panic!("expected Git with worktree path");
    }
}
