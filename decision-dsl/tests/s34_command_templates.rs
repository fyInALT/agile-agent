use decision_dsl::ast::template::render_command_templates;
use decision_dsl::ext::blackboard::Blackboard;
use decision_dsl::ext::command::{
    AgentCommand, DecisionCommand, GitCommand, HumanCommand, ProviderCommand, TaskCommand,
};

fn bb_with_vars() -> Blackboard {
    let mut bb = Blackboard::default();
    bb.provider_output = "test-output".into();
    bb.set("agent_name", decision_dsl::ext::blackboard::BlackboardValue::String("Alpha".into()));
    bb
}

// ── Agent commands ──────────────────────────────────────────────────────────

#[test]
fn render_agent_reflect() {
    let bb = bb_with_vars();
    let cmd = DecisionCommand::Agent(AgentCommand::Reflect {
        prompt: "Review: {{ provider_output }}".into(),
    });
    let rendered = render_command_templates(&cmd, &bb).unwrap();
    match rendered {
        DecisionCommand::Agent(AgentCommand::Reflect { prompt }) => {
            assert_eq!(prompt, "Review: test-output");
        }
        _ => panic!("expected Reflect"),
    }
}

#[test]
fn render_agent_send_instruction() {
    let bb = bb_with_vars();
    let cmd = DecisionCommand::Agent(AgentCommand::SendInstruction {
        prompt: "Tell {{ agent_name }}".into(),
        target_agent: "{{ agent_name }}".into(),
    });
    let rendered = render_command_templates(&cmd, &bb).unwrap();
    match rendered {
        DecisionCommand::Agent(AgentCommand::SendInstruction { prompt, target_agent }) => {
            assert_eq!(prompt, "Tell Alpha");
            assert_eq!(target_agent, "Alpha");
        }
        _ => panic!("expected SendInstruction"),
    }
}

#[test]
fn render_agent_terminate() {
    let bb = bb_with_vars();
    let cmd = DecisionCommand::Agent(AgentCommand::Terminate {
        reason: "Done: {{ provider_output }}".into(),
    });
    let rendered = render_command_templates(&cmd, &bb).unwrap();
    match rendered {
        DecisionCommand::Agent(AgentCommand::Terminate { reason }) => {
            assert_eq!(reason, "Done: test-output");
        }
        _ => panic!("expected Terminate"),
    }
}

#[test]
fn render_agent_approve_passes_through() {
    let bb = bb_with_vars();
    let cmd = DecisionCommand::Agent(AgentCommand::ApproveAndContinue);
    let rendered = render_command_templates(&cmd, &bb).unwrap();
    assert_eq!(rendered, cmd);
}

#[test]
fn render_agent_wake_up_passes_through() {
    let bb = bb_with_vars();
    let cmd = DecisionCommand::Agent(AgentCommand::WakeUp);
    let rendered = render_command_templates(&cmd, &bb).unwrap();
    assert_eq!(rendered, cmd);
}

// ── Git commands ────────────────────────────────────────────────────────────

#[test]
fn render_git_commit() {
    let bb = bb_with_vars();
    let cmd = DecisionCommand::Git(
        GitCommand::Commit {
            message: "Fix: {{ provider_output }}".into(),
            wip: true,
        },
        Some("{{ agent_name }}".into()),
    );
    let rendered = render_command_templates(&cmd, &bb).unwrap();
    match rendered {
        DecisionCommand::Git(GitCommand::Commit { message, wip }, extra) => {
            assert_eq!(message, "Fix: test-output");
            assert!(wip);
            assert_eq!(extra, Some("Alpha".into()));
        }
        _ => panic!("expected Commit"),
    }
}

#[test]
fn render_git_stash() {
    let bb = bb_with_vars();
    let cmd = DecisionCommand::Git(
        GitCommand::Stash {
            description: "WIP: {{ provider_output }}".into(),
            include_untracked: false,
        },
        None,
    );
    let rendered = render_command_templates(&cmd, &bb).unwrap();
    match rendered {
        DecisionCommand::Git(GitCommand::Stash { description, include_untracked }, extra) => {
            assert_eq!(description, "WIP: test-output");
            assert!(!include_untracked);
            assert_eq!(extra, None);
        }
        _ => panic!("expected Stash"),
    }
}

#[test]
fn render_git_create_branch() {
    let bb = bb_with_vars();
    let cmd = DecisionCommand::Git(
        GitCommand::CreateBranch {
            name: "feature/{{ agent_name }}".into(),
            base: "main".into(),
        },
        None,
    );
    let rendered = render_command_templates(&cmd, &bb).unwrap();
    match rendered {
        DecisionCommand::Git(GitCommand::CreateBranch { name, base }, extra) => {
            assert_eq!(name, "feature/Alpha");
            assert_eq!(base, "main");
            assert_eq!(extra, None);
        }
        _ => panic!("expected CreateBranch"),
    }
}

#[test]
fn render_git_rebase() {
    let bb = bb_with_vars();
    let cmd = DecisionCommand::Git(
        GitCommand::Rebase {
            base: "{{ agent_name }}".into(),
        },
        None,
    );
    let rendered = render_command_templates(&cmd, &bb).unwrap();
    match rendered {
        DecisionCommand::Git(GitCommand::Rebase { base }, extra) => {
            assert_eq!(base, "Alpha");
            assert_eq!(extra, None);
        }
        _ => panic!("expected Rebase"),
    }
}

#[test]
fn render_git_discard_passes_through() {
    let bb = bb_with_vars();
    let cmd = DecisionCommand::Git(GitCommand::Discard, None);
    let rendered = render_command_templates(&cmd, &bb).unwrap();
    assert_eq!(rendered, cmd);
}

// ── Task commands ───────────────────────────────────────────────────────────

#[test]
fn render_task_stop_if_complete() {
    let bb = bb_with_vars();
    let cmd = DecisionCommand::Task(TaskCommand::StopIfComplete {
        reason: "Done: {{ provider_output }}".into(),
    });
    let rendered = render_command_templates(&cmd, &bb).unwrap();
    match rendered {
        DecisionCommand::Task(TaskCommand::StopIfComplete { reason }) => {
            assert_eq!(reason, "Done: test-output");
        }
        _ => panic!("expected StopIfComplete"),
    }
}

#[test]
fn render_task_prepare_start() {
    let bb = bb_with_vars();
    let cmd = DecisionCommand::Task(TaskCommand::PrepareStart {
        task_id: "task-{{ agent_name }}".into(),
        description: "Desc: {{ provider_output }}".into(),
    });
    let rendered = render_command_templates(&cmd, &bb).unwrap();
    match rendered {
        DecisionCommand::Task(TaskCommand::PrepareStart { task_id, description }) => {
            assert_eq!(task_id, "task-Alpha");
            assert_eq!(description, "Desc: test-output");
        }
        _ => panic!("expected PrepareStart"),
    }
}

#[test]
fn render_task_confirm_completion_passes_through() {
    let bb = bb_with_vars();
    let cmd = DecisionCommand::Task(TaskCommand::ConfirmCompletion);
    let rendered = render_command_templates(&cmd, &bb).unwrap();
    assert_eq!(rendered, cmd);
}

// ── Human commands ──────────────────────────────────────────────────────────

#[test]
fn render_human_escalate() {
    let bb = bb_with_vars();
    let cmd = DecisionCommand::Human(HumanCommand::Escalate {
        reason: "Issue: {{ provider_output }}".into(),
        context: Some("Ctx: {{ agent_name }}".into()),
    });
    let rendered = render_command_templates(&cmd, &bb).unwrap();
    match rendered {
        DecisionCommand::Human(HumanCommand::Escalate { reason, context }) => {
            assert_eq!(reason, "Issue: test-output");
            assert_eq!(context, Some("Ctx: Alpha".into()));
        }
        _ => panic!("expected Escalate"),
    }
}

#[test]
fn render_human_escalate_no_context() {
    let bb = bb_with_vars();
    let cmd = DecisionCommand::Human(HumanCommand::Escalate {
        reason: "Issue".into(),
        context: None,
    });
    let rendered = render_command_templates(&cmd, &bb).unwrap();
    match rendered {
        DecisionCommand::Human(HumanCommand::Escalate { reason, context }) => {
            assert_eq!(reason, "Issue");
            assert_eq!(context, None);
        }
        _ => panic!("expected Escalate"),
    }
}

#[test]
fn render_human_select_option() {
    let bb = bb_with_vars();
    let cmd = DecisionCommand::Human(HumanCommand::SelectOption {
        option_id: "opt-{{ agent_name }}".into(),
    });
    let rendered = render_command_templates(&cmd, &bb).unwrap();
    match rendered {
        DecisionCommand::Human(HumanCommand::SelectOption { option_id }) => {
            assert_eq!(option_id, "opt-Alpha");
        }
        _ => panic!("expected SelectOption"),
    }
}

#[test]
fn render_human_skip_passes_through() {
    let bb = bb_with_vars();
    let cmd = DecisionCommand::Human(HumanCommand::SkipDecision);
    let rendered = render_command_templates(&cmd, &bb).unwrap();
    assert_eq!(rendered, cmd);
}

// ── Provider commands ───────────────────────────────────────────────────────

#[test]
fn render_provider_retry_tool() {
    let bb = bb_with_vars();
    let cmd = DecisionCommand::Provider(ProviderCommand::RetryTool {
        tool_name: "tool-{{ agent_name }}".into(),
        args: Some("args-{{ provider_output }}".into()),
        max_attempts: 3,
    });
    let rendered = render_command_templates(&cmd, &bb).unwrap();
    match rendered {
        DecisionCommand::Provider(ProviderCommand::RetryTool { tool_name, args, max_attempts }) => {
            assert_eq!(tool_name, "tool-Alpha");
            assert_eq!(args, Some("args-test-output".into()));
            assert_eq!(max_attempts, 3);
        }
        _ => panic!("expected RetryTool"),
    }
}

#[test]
fn render_provider_switch_provider() {
    let bb = bb_with_vars();
    let cmd = DecisionCommand::Provider(ProviderCommand::SwitchProvider {
        provider_type: "{{ agent_name }}".into(),
    });
    let rendered = render_command_templates(&cmd, &bb).unwrap();
    match rendered {
        DecisionCommand::Provider(ProviderCommand::SwitchProvider { provider_type }) => {
            assert_eq!(provider_type, "Alpha");
        }
        _ => panic!("expected SwitchProvider"),
    }
}

#[test]
fn render_provider_suggest_commit() {
    let bb = bb_with_vars();
    let cmd = DecisionCommand::Provider(ProviderCommand::SuggestCommit {
        message: "msg: {{ provider_output }}".into(),
        mandatory: true,
        reason: "r: {{ agent_name }}".into(),
    });
    let rendered = render_command_templates(&cmd, &bb).unwrap();
    match rendered {
        DecisionCommand::Provider(ProviderCommand::SuggestCommit { message, mandatory, reason }) => {
            assert_eq!(message, "msg: test-output");
            assert!(mandatory);
            assert_eq!(reason, "r: Alpha");
        }
        _ => panic!("expected SuggestCommit"),
    }
}

#[test]
fn render_provider_prepare_pr() {
    let bb = bb_with_vars();
    let cmd = DecisionCommand::Provider(ProviderCommand::PreparePr {
        title: "PR: {{ agent_name }}".into(),
        description: "Desc: {{ provider_output }}".into(),
        base: "main".into(),
        draft: false,
    });
    let rendered = render_command_templates(&cmd, &bb).unwrap();
    match rendered {
        DecisionCommand::Provider(ProviderCommand::PreparePr { title, description, base, draft }) => {
            assert_eq!(title, "PR: Alpha");
            assert_eq!(description, "Desc: test-output");
            assert_eq!(base, "main");
            assert!(!draft);
        }
        _ => panic!("expected PreparePr"),
    }
}

// ── Error handling ──────────────────────────────────────────────────────────

#[test]
fn render_missing_variable_fails() {
    let bb = Blackboard::default();
    let cmd = DecisionCommand::Agent(AgentCommand::Reflect {
        prompt: "{{ missing }}".into(),
    });
    let err = render_command_templates(&cmd, &bb).unwrap_err();
    assert!(err.to_string().contains("undefined"));
}
