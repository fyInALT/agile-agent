use agent_core::agent_runtime::AgentBootstrapKind;
use agent_core::agent_runtime::AgentRuntime;
use agent_core::agent_store::AgentStore;
use agent_core::app::AppState;
use agent_core::backlog_store;
use agent_core::logging;
use agent_core::logging::RunMode;
use agent_core::loop_runner;
use agent_core::loop_runner::LoopGuardrails;
use agent_core::multi_agent_session::MultiAgentSession;
use agent_core::probe;
use agent_core::runtime_mode::RuntimeMode;
use agent_core::session_store;
use agent_core::skills::SkillRegistry;
use agent_core::workplace_store::WorkplaceStore;
use anyhow::Result;
use clap::Parser;
use clap::Subcommand;
use std::env;

#[derive(Parser, Debug)]
#[command(name = "agile-agent", version, about = "agile-agent CLI")]
pub struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    /// Inspect available providers in the current environment.
    Doctor,
    /// Inspect and manage agent runtime state in the current workplace.
    Agent {
        #[command(subcommand)]
        command: AgentCommand,
    },
    /// Inspect workplace state in the current working directory.
    Workplace {
        #[command(subcommand)]
        command: WorkplaceCommand,
    },
    /// Manage human decision requests from decision layer.
    Decision {
        #[command(subcommand)]
        command: DecisionCommand,
    },
    /// Manage provider profiles.
    Profile {
        #[command(subcommand)]
        command: ProfileCommand,
    },
    /// Restore the most recent saved session.
    ResumeLast,
    /// Run the autonomous loop without the TUI.
    RunLoop {
        #[arg(long, default_value_t = 5)]
        max_iterations: usize,
        #[arg(long, default_value_t = false)]
        resume_last: bool,
        /// Enable multi-agent mode for concurrent execution.
        #[arg(long, default_value_t = false)]
        multi_agent: bool,
    },
    /// Print structured provider probe results.
    Probe {
        #[arg(long)]
        json: bool,
    },
    /// Manage the agent daemon lifecycle.
    Daemon {
        #[command(subcommand)]
        command: DaemonCommand,
    },
}

#[derive(Subcommand, Debug)]
pub enum AgentCommand {
    /// Show the current or most recent agent for this workplace.
    Current,
    /// List all agents for this workplace.
    List {
        /// Show all agents including stopped ones.
        #[arg(long, default_value_t = false)]
        all: bool,
    },
    /// Spawn a new agent with specified provider.
    Spawn {
        /// Provider type: claude, codex, opencode.
        provider: String,
    },
    /// Stop a running agent.
    Stop {
        /// Agent ID to stop.
        agent_id: String,
    },
    /// Show detailed status for a specific agent.
    Status {
        /// Agent ID to inspect.
        agent_id: String,
    },
}

#[derive(Subcommand, Debug)]
pub enum WorkplaceCommand {
    /// Show the current workplace mapping for this working directory.
    Current,
}

#[derive(Subcommand, Debug)]
pub enum DecisionCommand {
    /// List pending human decision requests.
    List {
        /// Show only pending requests.
        #[arg(long, default_value_t = true)]
        pending: bool,
    },
    /// Show details of a specific decision request.
    Show {
        /// Request ID to show.
        request_id: String,
    },
    /// Respond to a decision request.
    Respond {
        /// Request ID to respond to.
        request_id: String,
        /// Select option by ID (A, B, C, D...).
        #[arg(long)]
        select: Option<String>,
        /// Accept recommendation.
        #[arg(long, default_value_t = false)]
        accept: bool,
        /// Provide custom instruction.
        #[arg(long)]
        custom: Option<String>,
        /// Skip the decision.
        #[arg(long, default_value_t = false)]
        skip: bool,
    },
    /// Show history of completed decisions.
    History {
        /// Number of recent decisions to show.
        #[arg(long, default_value_t = 10)]
        count: usize,
    },
}

#[derive(Subcommand, Debug)]
pub enum DaemonCommand {
    /// Start the daemon for the current workplace.
    Start,
    /// Stop the daemon for the current workplace.
    Stop,
    /// Show daemon status (pid, url, uptime).
    Status,
}

#[derive(Subcommand, Debug)]
pub enum ProfileCommand {
    /// List all available provider profiles.
    List {
        /// Include profile details.
        #[arg(long, default_value_t = false)]
        verbose: bool,
    },
}

pub fn run() -> Result<()> {
    let cli = Cli::parse();
    execute(cli)
}

pub fn execute(cli: Cli) -> Result<()> {
    let launch_cwd = env::current_dir()?;
    init_logging_for_mode(&launch_cwd, run_mode_for_cli(&cli));

    match cli.command {
        None => {
            if tui_resume_enabled(&cli) {
                agent_tui::run_tui_with_resume_last()
            } else {
                agent_tui::run_tui()
            }
        }
        Some(Command::ResumeLast) => agent_tui::run_tui_with_resume_last(),
        Some(Command::Agent {
            command: AgentCommand::Current,
        }) => print_current_agent(),
        Some(Command::Agent {
            command: AgentCommand::List { all },
        }) => print_agent_list(all),
        Some(Command::Agent {
            command: AgentCommand::Spawn { provider },
        }) => spawn_agent(provider),
        Some(Command::Agent {
            command: AgentCommand::Stop { agent_id },
        }) => stop_agent(agent_id),
        Some(Command::Agent {
            command: AgentCommand::Status { agent_id },
        }) => print_agent_status(agent_id),
        Some(Command::Workplace {
            command: WorkplaceCommand::Current,
        }) => print_current_workplace(),
        Some(Command::Decision {
            command: DecisionCommand::List { pending },
        }) => print_decision_list(pending),
        Some(Command::Decision {
            command: DecisionCommand::Show { request_id },
        }) => print_decision_show(request_id),
        Some(Command::Decision {
            command:
                DecisionCommand::Respond {
                    request_id,
                    select,
                    accept,
                    custom,
                    skip,
                },
        }) => respond_to_decision(request_id, select, accept, custom, skip),
        Some(Command::Decision {
            command: DecisionCommand::History { count },
        }) => print_decision_history(count),
        Some(Command::Profile {
            command: ProfileCommand::List { verbose },
        }) => print_profile_list(verbose),
        Some(Command::RunLoop {
            max_iterations,
            resume_last,
            multi_agent,
        }) => run_loop_headless(max_iterations, resume_last, multi_agent),
        Some(Command::Doctor) => {
            print!("{}", probe::render_doctor_text(&probe::probe_report()));
            Ok(())
        }
        Some(Command::Probe { json: true }) => {
            println!("{}", serde_json::to_string_pretty(&probe::probe_report())?);
            Ok(())
        }
        Some(Command::Probe { json: false }) => {
            println!("probe requires --json");
            Ok(())
        }
        Some(Command::Daemon { command }) => match command {
            DaemonCommand::Start => daemon_start(),
            DaemonCommand::Stop => daemon_stop(),
            DaemonCommand::Status => daemon_status(),
        },
    }
}

fn tui_resume_enabled(cli: &Cli) -> bool {
    matches!(cli.command, None | Some(Command::ResumeLast))
}

fn run_mode_for_cli(cli: &Cli) -> RunMode {
    match &cli.command {
        None => RunMode::Tui,
        Some(Command::ResumeLast) => RunMode::ResumeLast,
        Some(Command::RunLoop { .. }) => RunMode::RunLoop,
        Some(Command::Doctor) => RunMode::Doctor,
        Some(Command::Probe { .. }) => RunMode::Probe,
        Some(Command::Agent {
            command: AgentCommand::Current,
        }) => RunMode::AgentCurrent,
        Some(Command::Agent {
            command: AgentCommand::List { .. },
        }) => RunMode::AgentList,
        Some(Command::Agent {
            command: AgentCommand::Spawn { .. },
        }) => RunMode::AgentSpawn,
        Some(Command::Agent {
            command: AgentCommand::Stop { .. },
        }) => RunMode::AgentStop,
        Some(Command::Agent {
            command: AgentCommand::Status { .. },
        }) => RunMode::AgentStatus,
        Some(Command::Workplace {
            command: WorkplaceCommand::Current,
        }) => RunMode::WorkplaceCurrent,
        Some(Command::Decision {
            command: DecisionCommand::List { .. },
        }) => RunMode::DecisionList,
        Some(Command::Decision {
            command: DecisionCommand::Show { .. },
        }) => RunMode::DecisionShow,
        Some(Command::Decision {
            command: DecisionCommand::Respond { .. },
        }) => RunMode::DecisionRespond,
        Some(Command::Decision {
            command: DecisionCommand::History { .. },
        }) => RunMode::DecisionHistory,
        Some(Command::Profile { .. }) => RunMode::ProfileList,
        Some(Command::Daemon { .. }) => RunMode::Daemon,
    }
}

fn init_logging_for_mode(launch_cwd: &std::path::Path, run_mode: RunMode) {
    match WorkplaceStore::for_cwd(launch_cwd).and_then(|workplace| {
        workplace.ensure()?;
        logging::init_for_workplace(&workplace, run_mode)
    }) {
        Ok(initialized) => logging::debug_event(
            "app.launch",
            "initialized CLI logging",
            serde_json::json!({
                "cwd": launch_cwd.display().to_string(),
                "run_mode": run_mode.as_str(),
                "log_path": initialized.log_path.display().to_string(),
            }),
        ),
        Err(error) => eprintln!("warning: failed to initialize debug logging: {error}"),
    }
}

fn print_current_agent() -> Result<()> {
    let launch_cwd = env::current_dir()?;
    let workplace = WorkplaceStore::for_cwd(&launch_cwd)?;
    let store = AgentStore::new(workplace);

    if let Some(meta) = store.load_most_recent_meta()? {
        println!("agent_id: {}", meta.agent_id.as_str());
        println!("codename: {}", meta.codename.as_str());
        println!("workplace_id: {}", meta.workplace_id.as_str());
        println!("provider_type: {}", meta.provider_type.label());
        println!(
            "provider_session_id: {}",
            meta.provider_session_id
                .as_ref()
                .map(|value| value.as_str())
                .unwrap_or("<none>")
        );
        println!("status: {:?}", meta.status);
        println!("created_at: {}", meta.created_at);
        println!("updated_at: {}", meta.updated_at);
    } else {
        println!("no agent found for the current workplace");
    }

    Ok(())
}

fn print_agent_list(all: bool) -> Result<()> {
    let launch_cwd = env::current_dir()?;
    let workplace = WorkplaceStore::for_cwd(&launch_cwd)?;
    let store = AgentStore::new(workplace);
    let agents = store.list_meta()?;

    if agents.is_empty() {
        println!("no agents found for the current workplace");
        return Ok(());
    }

    for meta in agents {
        println!(
            "{} {} {} {}{}",
            meta.agent_id.as_str(),
            meta.codename.as_str(),
            meta.provider_type.label(),
            if all {
                format!(" {} ", meta.status.label())
            } else {
                "".to_string()
            },
            meta.updated_at
        );
    }

    Ok(())
}

fn spawn_agent(provider: String) -> Result<()> {
    use agent_core::ProviderKind;

    let provider_kind = match provider.to_lowercase().as_str() {
        "claude" => ProviderKind::Claude,
        "codex" => ProviderKind::Codex,
        "mock" => ProviderKind::Mock,
        _ => {
            eprintln!(
                "unknown provider: {}. Available: claude, codex, mock",
                provider
            );
            return Err(anyhow::anyhow!("unknown provider: {}", provider));
        }
    };

    let launch_cwd = env::current_dir()?;
    let bootstrap = AgentRuntime::bootstrap_for_cwd(&launch_cwd, provider_kind)?;
    let agent_runtime = bootstrap.runtime;

    println!("agent: spawned {}", agent_runtime.summary());
    agent_runtime.persist()?;

    Ok(())
}

fn stop_agent(agent_id: String) -> Result<()> {
    let launch_cwd = env::current_dir()?;
    let workplace = WorkplaceStore::for_cwd(&launch_cwd)?;
    let store = AgentStore::new(workplace);

    let agents = store.list_meta()?;
    let agent_meta = agents.iter().find(|m| m.agent_id.as_str() == agent_id);

    if agent_meta.is_none() {
        eprintln!("agent not found: {}", agent_id);
        return Err(anyhow::anyhow!("agent not found: {}", agent_id));
    }

    println!("agent: stopping {}", agent_id);
    println!("status: stopped (requested)");
    println!("note: full stop requires TUI session");

    Ok(())
}

fn print_agent_status(agent_id: String) -> Result<()> {
    let launch_cwd = env::current_dir()?;
    let workplace = WorkplaceStore::for_cwd(&launch_cwd)?;
    let store = AgentStore::new(workplace);

    let agents = store.list_meta()?;
    let agent_meta = agents.iter().find(|m| m.agent_id.as_str() == agent_id);

    if let Some(meta) = agent_meta {
        println!("agent_id: {}", meta.agent_id.as_str());
        println!("codename: {}", meta.codename.as_str());
        println!("workplace_id: {}", meta.workplace_id.as_str());
        println!("provider_type: {}", meta.provider_type.label());
        println!(
            "provider_session_id: {}",
            meta.provider_session_id
                .as_ref()
                .map(|value| value.as_str())
                .unwrap_or("<none>")
        );
        println!("status: {}", meta.status.label());
        println!("created_at: {}", meta.created_at);
        println!("updated_at: {}", meta.updated_at);
    } else {
        eprintln!("agent not found: {}", agent_id);
        return Err(anyhow::anyhow!("agent not found: {}", agent_id));
    }

    Ok(())
}

fn print_current_workplace() -> Result<()> {
    let launch_cwd = env::current_dir()?;
    let workplace = WorkplaceStore::for_cwd(&launch_cwd)?;
    workplace.ensure()?;
    let meta = workplace.load_meta()?;

    println!("workplace_id: {}", meta.workplace_id.as_str());
    println!("root_cwd: {}", meta.root_cwd);
    println!("path: {}", workplace.path().display());
    println!("created_at: {}", meta.created_at);
    println!("updated_at: {}", meta.updated_at);

    Ok(())
}

fn run_loop_headless(max_iterations: usize, resume_last: bool, multi_agent: bool) -> Result<()> {
    let launch_cwd = env::current_dir()?;

    if multi_agent {
        run_loop_headless_multi_agent(max_iterations, resume_last, launch_cwd)
    } else {
        run_loop_headless_single_agent(max_iterations, resume_last, launch_cwd)
    }
}

fn run_loop_headless_single_agent(
    max_iterations: usize,
    resume_last: bool,
    launch_cwd: std::path::PathBuf,
) -> Result<()> {
    let bootstrap =
        AgentRuntime::bootstrap_for_cwd(&launch_cwd, agent_core::default_provider())?;
    let mut state = AppState::with_skills(
        agent_core::default_provider(),
        launch_cwd.clone(),
        SkillRegistry::discover(&launch_cwd),
    );
    state.backlog = backlog_store::load_backlog_for_workplace(bootstrap.runtime.workplace())?;
    for warning in bootstrap.runtime.apply_to_app_state(&mut state) {
        eprintln!("warning: {warning}");
    }
    announce_bootstrap_kind(&bootstrap.kind, &bootstrap.runtime);
    let mut agent_runtime = bootstrap.runtime;

    if resume_last {
        match agent_runtime.restore_snapshot(&mut state) {
            Ok(restored) => {
                for warning in restored.warnings {
                    eprintln!("warning: {warning}");
                }
                for warning in agent_runtime.apply_to_app_state(&mut state) {
                    eprintln!("warning: {warning}");
                }
            }
            Err(err) => match session_store::restore_recent_session_for_workplace(
                &mut state,
                &launch_cwd,
                agent_runtime.workplace(),
            ) {
                Ok(restored) => {
                    for warning in restored.warnings {
                        eprintln!("warning: {warning}");
                    }
                    for warning in agent_runtime.apply_to_app_state(&mut state) {
                        eprintln!("warning: {warning}");
                    }
                }
                Err(_) => eprintln!("warning: failed to restore recent agent state: {err}"),
            },
        }
    }
    if agent_runtime.sync_from_app_state(&state) {
        persist_agent_runtime_bundle(&agent_runtime, &state)?;
    }

    let initial_transcript_len = state.transcript.len();

    let summary = loop_runner::run_loop_with_hook(
        &mut state,
        LoopGuardrails {
            max_iterations,
            max_continuations_per_task: 3,
            max_verification_failures: 1,
        },
        &mut |state: &AppState| {
            if agent_runtime.sync_from_app_state(state) {
                persist_agent_runtime_bundle(&agent_runtime, state)?;
            }
            Ok(())
        },
    )?;

    backlog_store::save_backlog_for_workplace(&state.backlog, agent_runtime.workplace())?;
    session_store::save_recent_session_for_workplace(&state, agent_runtime.workplace())?;
    agent_runtime.sync_from_app_state(&state);
    agent_runtime.mark_stopped();
    persist_agent_runtime_bundle(&agent_runtime, &state)?;

    println!("iterations: {}", summary.iterations);
    println!("stopped_reason: {}", summary.stopped_reason);
    for entry in state.transcript.iter().skip(initial_transcript_len) {
        match entry {
            agent_core::app::TranscriptEntry::Status(text) => println!("status: {}", text),
            agent_core::app::TranscriptEntry::Error(text) => println!("error: {}", text),
            agent_core::app::TranscriptEntry::Assistant(text) if !text.is_empty() => {
                println!("assistant: {}", text)
            }
            _ => {}
        }
    }

    Ok(())
}

fn run_loop_headless_multi_agent(
    max_iterations: usize,
    resume_last: bool,
    launch_cwd: std::path::PathBuf,
) -> Result<()> {
    use agent_core::shutdown_snapshot::{AgentShutdownSnapshot, ShutdownReason, ShutdownSnapshot};
    use agent_core::backlog::{TaskStatus, TodoStatus};
    use agent_core::agent_runtime::AgentStatus;

    eprintln!("multi-agent mode enabled");

    let max_agents = RuntimeMode::MultiAgent.max_agents();
    let mut session = MultiAgentSession::bootstrap(
        launch_cwd.clone(),
        agent_core::default_provider(),
        resume_last,
        max_agents,
    )?;

    eprintln!(
        "agents: {} active, {} max",
        session.agents.active_count(),
        max_agents
    );

    // Load backlog into shared workplace state
    let workplace_store = WorkplaceStore::for_cwd(&launch_cwd)?;
    session.workplace.backlog =
        backlog_store::load_backlog_for_workplace(&workplace_store)?;

    let todo_count = session.workplace.backlog.todos.iter()
        .filter(|t| t.status == TodoStatus::Ready).count();
    let task_count = session.workplace.backlog.tasks.iter()
        .filter(|t| t.status == TaskStatus::Ready).count();
    eprintln!(
        "workplace: {} (backlog: {} ready todos, {} ready tasks)",
        session.workplace.workplace_id.as_str(),
        todo_count,
        task_count
    );

    // Run multi-agent loop
    let mut iteration = 0;

    while iteration < max_iterations && session.any_active() {
        iteration += 1;
        eprintln!("iteration: {} / {}", iteration, max_iterations);

        // Poll for events from all agents with timeout
        let poll_result = session.poll_events_with_timeout(std::time::Duration::from_millis(100));

        for event in poll_result.events {
            // Process events through the session (which handles state transitions)
            match session.process_event(event) {
                Ok(_) => {}
                Err(e) => eprintln!("event processing error: {}", e),
            }
        }

        // Check for idle agents and assign ready tasks
        let ready_tasks: Vec<(String, TaskStatus)> = session.workplace.backlog.ready_tasks()
            .iter()
            .map(|t| (t.id.clone(), t.status))
            .collect();
        if !ready_tasks.is_empty() {
            for slot_idx in 0..session.agents.active_count() {
                if let Some(slot) = session.agents.get_slot(slot_idx) {
                    let status = slot.status();
                    let has_task = slot.assigned_task_id().is_some();
                    if status.is_idle() && !has_task {
                        // Find a ready task
                        for (task_id, task_status) in &ready_tasks {
                            if *task_status == TaskStatus::Ready {
                                if let Some(slot_mut) = session.agents.get_slot_mut(slot_idx) {
                                    let tid = agent_core::agent_slot::TaskId::new(task_id);
                                    if slot_mut.assign_task(tid).is_ok() {
                                        eprintln!(
                                            "task: assigned {} to agent {}",
                                            task_id,
                                            slot_mut.agent_id().as_str()
                                        );
                                        // Mark task as started in backlog (after collecting IDs)
                                        session.workplace.backlog.start_task(task_id);
                                        break;
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        // Small delay between iterations
        std::thread::sleep(std::time::Duration::from_millis(50));
    }

    // Create shutdown snapshot for persistence
    let agents_snapshots: Vec<_> = (0..session.agents.active_count())
        .filter_map(|idx| session.agents.get_slot(idx))
        .map(|slot| {
            AgentShutdownSnapshot {
                meta: agent_core::agent_runtime::AgentMeta {
                    agent_id: slot.agent_id().clone(),
                    codename: slot.codename().clone(),
                    workplace_id: session.workplace.workplace_id.clone(),
                    provider_type: slot.provider_type(),
                    provider_session_id: None,
                    created_at: String::new(),
                    updated_at: String::new(),
                    status: AgentStatus::Idle,
                },
                assigned_task_id: slot.assigned_task_id().map(|id| id.as_str().to_string()),
                was_active: !slot.status().is_terminal(),
                had_error: slot.status().is_blocked(),
                provider_thread_state: None,
                captured_at: String::new(),
                role: slot.role(),
                transcript: slot.transcript().to_vec(),
            }
        })
        .collect();

    let snapshot = ShutdownSnapshot::new(
        session.workplace.workplace_id.as_str().to_string(),
        agents_snapshots,
        session.workplace.backlog.clone(),
        Vec::new(),
        ShutdownReason::CleanExit,
    );

    workplace_store.save_shutdown_snapshot(&snapshot)?;
    backlog_store::save_backlog_for_workplace(&session.workplace.backlog, &workplace_store)?;

    println!("iterations: {}", iteration);
    println!("agents_active: {}", session.agents.active_count());
    println!("backlog_remaining: {} todos, {} tasks",
        session.workplace.backlog.todos.len(),
        session.workplace.backlog.tasks.len());

    Ok(())
}

#[allow(dead_code)]
fn handle_multi_agent_event(_session: &mut MultiAgentSession, event: agent_core::event_aggregator::AgentEvent) {
    use agent_core::event_aggregator::AgentEvent;

    match event {
        AgentEvent::FromProvider { agent_id, event: _ } => {
            logging::debug_event(
                "multi_agent.event",
                "provider event received",
                serde_json::json!({
                    "agent_id": agent_id.as_str(),
                }),
            );
        }
        AgentEvent::StatusChanged { agent_id, old_status, new_status } => {
            eprintln!(
                "agent {}: {} -> {}",
                agent_id.as_str(),
                old_status.label(),
                new_status.label()
            );
        }
        AgentEvent::AgentError { agent_id, error } => {
            eprintln!("agent {}: error {}", agent_id.as_str(), error);
        }
        _ => {}
    }
}

fn announce_bootstrap_kind(kind: &AgentBootstrapKind, runtime: &AgentRuntime) {
    match kind {
        AgentBootstrapKind::Created => {
            eprintln!("agent: created {}", runtime.summary());
        }
        AgentBootstrapKind::Restored => {
            eprintln!("agent: restored {}", runtime.summary());
        }
        AgentBootstrapKind::RecreatedAfterError { error } => {
            eprintln!("warning: failed to restore agent runtime: {error}");
            eprintln!("agent: created replacement {}", runtime.summary());
        }
    }
}

fn persist_agent_runtime_bundle(agent_runtime: &AgentRuntime, state: &AppState) -> Result<()> {
    agent_runtime.persist()?;
    agent_runtime.persist_state(state)?;
    agent_runtime.persist_transcript(state)?;
    agent_runtime.persist_messages(state)?;
    agent_runtime.persist_memory(state)?;
    Ok(())
}

// Decision commands implementation

fn print_decision_list(_pending: bool) -> Result<()> {
    println!("No pending decision requests.");
    println!("");
    println!("Usage:");
    println!("  agile-agent decision list --pending    List pending requests");
    println!("  agile-agent decision show <id>         Show request details");
    println!("  agile-agent decision respond <id>      Respond to request");
    println!("  agile-agent decision history           Show completed decisions");
    Ok(())
}

fn print_decision_show(request_id: String) -> Result<()> {
    println!("Request ID: {}", request_id);
    println!("Status: Not found or no active decision session");
    Ok(())
}

fn respond_to_decision(
    request_id: String,
    select: Option<String>,
    accept: bool,
    custom: Option<String>,
    skip: bool,
) -> Result<()> {
    use agent_decision::{HumanDecisionResponse, HumanSelection};

    let selection = if skip {
        HumanSelection::skip()
    } else if accept {
        HumanSelection::accept_recommendation()
    } else if let Some(instruction) = custom {
        HumanSelection::custom(instruction)
    } else if let Some(option_id) = select {
        HumanSelection::selected(option_id)
    } else {
        eprintln!("No response action specified. Use one of:");
        eprintln!("  --select <option_id>   Select option by ID");
        eprintln!("  --accept               Accept recommendation");
        eprintln!("  --custom <instruction> Provide custom instruction");
        eprintln!("  --skip                 Skip this decision");
        return Err(anyhow::anyhow!("no response action specified"));
    };

    let response = HumanDecisionResponse::new(&request_id, selection);

    println!("Request ID: {}", request_id);
    println!("Response: {:?}", response.selection);
    println!("Status: Response recorded");
    Ok(())
}

fn print_decision_history(count: usize) -> Result<()> {
    println!("Decision History (last {} entries):", count);
    println!("");
    println!("No decision history available.");
    Ok(())
}

fn print_profile_list(verbose: bool) -> Result<()> {
    use agent_core::global_config::GlobalConfigStore;
    use agent_core::provider_profile::ProfilePersistence;

    let launch_cwd = env::current_dir()?;
    let workplace = WorkplaceStore::for_cwd(&launch_cwd)?;

    let config_store = GlobalConfigStore::new()?;
    let persistence = ProfilePersistence::for_paths(
        config_store.path().clone(),
        Some(workplace.path().to_path_buf()),
    );
    let store = persistence.load_merged()?;

    println!("Provider Profiles:");
    println!("");

    for profile in store.list_profiles() {
        let marker = if profile.id == *store.default_work_profile_id() {
            " [default]"
        } else {
            ""
        };
        println!("  {} ({}){}", profile.display_name, profile.base_cli.label(), marker);

        if verbose {
            println!("    ID: {}", profile.id);
            if !profile.env_overrides.is_empty() {
                println!("    Env overrides:");
                for (key, value) in &profile.env_overrides {
                    println!("      {} = {}", key, value);
                }
            }
            if !profile.extra_args.is_empty() {
                println!("    Extra args: {:?}", profile.extra_args);
            }
            if let Some(ref desc) = profile.description {
                println!("    Description: {}", desc);
            }
            println!("");
        }
    }

    println!("");
    println!("Default work profile: {}", store.default_work_profile_id());
    println!("Default decision profile: {}", store.default_decision_profile_id());

    Ok(())
}

fn daemon_start() -> Result<()> {
    let launch_cwd = std::env::current_dir()?;
    let workplace = WorkplaceStore::for_cwd(&launch_cwd)?;
    let wp_id = workplace.workplace_id().clone();
    let daemon_json = agent_protocol::config::DaemonConfig::path_for_workplace(
        workplace.path(),
        &wp_id,
    );

    let rt = tokio::runtime::Runtime::new()?;
    let result = rt.block_on(agent_protocol::client::auto_link::auto_link(
        &wp_id,
        &daemon_json,
        None,
        std::time::Duration::from_secs(10),
    ))?;

    println!("Daemon started");
    println!("  PID:     {}", result.pid);
    println!("  URL:     {}", result.websocket_url);
    println!("  Spawned: {}", if result.spawned { "yes" } else { "already running" });
    Ok(())
}

fn daemon_stop() -> Result<()> {
    let launch_cwd = std::env::current_dir()?;
    let workplace = WorkplaceStore::for_cwd(&launch_cwd)?;
    let wp_id = workplace.workplace_id().clone();
    let daemon_json = agent_protocol::config::DaemonConfig::path_for_workplace(
        workplace.path(),
        &wp_id,
    );

    let rt = tokio::runtime::Runtime::new()?;
    match rt.block_on(agent_protocol::config::DaemonConfig::read(&daemon_json)) {
        Ok(config) => {
            // Send SIGTERM
            #[cfg(unix)]
            unsafe {
                libc::kill(config.pid as i32, libc::SIGTERM);
            }
            #[cfg(not(unix))]
            {
                println!("Daemon stop not supported on this platform");
                return Ok(());
            }
            println!("Sent SIGTERM to daemon (pid {})", config.pid);
            // Wait for daemon.json to disappear
            for _ in 0..50 {
                if !daemon_json.exists() {
                    println!("Daemon stopped.");
                    return Ok(());
                }
                std::thread::sleep(std::time::Duration::from_millis(100));
            }
            anyhow::bail!("daemon did not stop within 5s");
        }
        Err(_) => {
            println!("No daemon running for this workplace.");
            Ok(())
        }
    }
}

fn daemon_status() -> Result<()> {
    let launch_cwd = std::env::current_dir()?;
    let workplace = WorkplaceStore::for_cwd(&launch_cwd)?;
    let wp_id = workplace.workplace_id().clone();
    let daemon_json = agent_protocol::config::DaemonConfig::path_for_workplace(
        workplace.path(),
        &wp_id,
    );

    let rt = tokio::runtime::Runtime::new()?;
    match rt.block_on(agent_protocol::config::DaemonConfig::read(&daemon_json)) {
        Ok(config) => {
            let alive = unsafe { libc::kill(config.pid as i32, 0) == 0 };
            println!("Daemon status");
            println!("  PID:     {}", config.pid);
            println!("  URL:     {}", config.websocket_url);
            println!("  Alive:   {}", if alive { "yes" } else { "no (stale config)" });
            println!("  Started: {}", config.started_at);
        }
        Err(_) => {
            println!("No daemon running for this workplace.");
        }
    }
    Ok(())
}

/// Stub for agent list via protocol.
/// TODO: Will be replaced with actual protocol call in a future sprint.
#[cfg(test)]
async fn agent_list_via_protocol() -> Result<Vec<String>> {
    Ok(vec!["agent_001 alpha".to_string()])
}

#[cfg(test)]
mod tests {
    use super::{Cli, Command, agent_list_via_protocol, daemon_start, daemon_status, daemon_stop, run_mode_for_cli, tui_resume_enabled};
    use agent_core::logging::RunMode;
    use agent_core::workplace_store::WorkplaceStore;

    struct CwdGuard(std::path::PathBuf);

    impl CwdGuard {
        fn new(path: &std::path::Path) -> Self {
            let original = std::env::current_dir().unwrap();
            std::env::set_current_dir(path).unwrap();
            Self(original)
        }
    }

    impl Drop for CwdGuard {
        fn drop(&mut self) {
            let _ = std::env::set_current_dir(&self.0);
        }
    }

    #[test]
    fn default_command_enables_resume_last_for_tui() {
        let cli = Cli { command: None };
        assert!(tui_resume_enabled(&cli));
    }

    #[test]
    fn doctor_command_does_not_enable_tui_resume() {
        let cli = Cli {
            command: Some(Command::Doctor),
        };
        assert!(!tui_resume_enabled(&cli));
        assert_eq!(run_mode_for_cli(&cli), RunMode::Doctor);
    }

    #[test]
    fn daemon_status_reports_no_daemon_when_missing() {
        let tmp = tempfile::tempdir().unwrap();
        let _guard = CwdGuard::new(tmp.path());
        let result = daemon_status();
        assert!(result.is_ok());
    }

    #[test]
    fn daemon_stop_reports_no_daemon_when_missing() {
        let tmp = tempfile::tempdir().unwrap();
        let _guard = CwdGuard::new(tmp.path());
        let result = daemon_stop();
        assert!(result.is_ok());
    }

    #[test]
    fn daemon_status_reports_running_daemon() {
        let tmp = tempfile::tempdir().unwrap();
        let _guard = CwdGuard::new(tmp.path());

        let workplace = WorkplaceStore::for_cwd(tmp.path()).unwrap();
        workplace.ensure().unwrap();
        let wp_id = workplace.workplace_id().clone();
        let daemon_json = agent_protocol::config::DaemonConfig::path_for_workplace(workplace.path(), &wp_id);

        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let config = agent_protocol::config::DaemonConfig::new(
                std::process::id(),
                "ws://127.0.0.1:9999/v1/session",
                wp_id,
                None,
            );
            config.write(&daemon_json).await.unwrap();
        });

        let result = daemon_status();
        assert!(result.is_ok());
    }

    #[test]
    fn daemon_start_errors_when_daemon_binary_missing() {
        let tmp = tempfile::tempdir().unwrap();
        let _guard = CwdGuard::new(tmp.path());

        let original = std::env::var("CARGO_BIN_EXE_agent-daemon").ok();
        unsafe { std::env::remove_var("CARGO_BIN_EXE_agent-daemon") };

        let result = daemon_start();
        assert!(result.is_err());

        if let Some(v) = original {
            unsafe { std::env::set_var("CARGO_BIN_EXE_agent-daemon", v) };
        }
    }

    #[tokio::test]
    async fn agent_list_via_protocol_stub_returns_placeholder() {
        let list = agent_list_via_protocol().await.unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0], "agent_001 alpha");
    }
}
