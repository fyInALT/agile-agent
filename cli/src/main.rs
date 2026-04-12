use agent_core::agent_runtime::AgentBootstrapKind;
use agent_core::agent_runtime::AgentRuntime;
use agent_core::agent_store::AgentStore;
use agent_core::app::AppState;
use agent_core::backlog_store;
use agent_core::loop_runner;
use agent_core::loop_runner::LoopGuardrails;
use agent_core::probe;
use agent_core::session_store;
use agent_core::skills::SkillRegistry;
use agent_core::workplace_store::WorkplaceStore;
use anyhow::Result;
use clap::Parser;
use clap::Subcommand;
use std::env;

#[derive(Parser, Debug)]
#[command(name = "agile-agent", version, about = "agile-agent CLI")]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// Inspect available providers in the current environment.
    Doctor,
    /// Inspect agent runtime state in the current workplace.
    Agent {
        #[command(subcommand)]
        command: AgentCommand,
    },
    /// Inspect workplace state in the current working directory.
    Workplace {
        #[command(subcommand)]
        command: WorkplaceCommand,
    },
    /// Restore the most recent saved session.
    ResumeLast,
    /// Run the autonomous loop without the TUI.
    RunLoop {
        #[arg(long, default_value_t = 5)]
        max_iterations: usize,
        #[arg(long, default_value_t = false)]
        resume_last: bool,
    },
    /// Print structured provider probe results.
    Probe {
        #[arg(long)]
        json: bool,
    },
}

#[derive(Subcommand, Debug)]
enum AgentCommand {
    /// Show the current or most recent agent for this workplace.
    Current,
    /// List known agents for this workplace.
    List,
}

#[derive(Subcommand, Debug)]
enum WorkplaceCommand {
    /// Show the current workplace mapping for this working directory.
    Current,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        None => agent_tui::run_tui(),
        Some(Command::ResumeLast) => agent_tui::run_tui_with_resume_last(),
        Some(Command::Agent {
            command: AgentCommand::Current,
        }) => print_current_agent(),
        Some(Command::Agent {
            command: AgentCommand::List,
        }) => print_agent_list(),
        Some(Command::Workplace {
            command: WorkplaceCommand::Current,
        }) => print_current_workplace(),
        Some(Command::RunLoop {
            max_iterations,
            resume_last,
        }) => run_loop_headless(max_iterations, resume_last),
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

fn print_agent_list() -> Result<()> {
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
            "{} {} {} {}",
            meta.agent_id.as_str(),
            meta.codename.as_str(),
            meta.provider_type.label(),
            meta.updated_at
        );
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

fn run_loop_headless(max_iterations: usize, resume_last: bool) -> Result<()> {
    let launch_cwd = env::current_dir()?;
    let bootstrap =
        AgentRuntime::bootstrap_for_cwd(&launch_cwd, agent_core::provider::default_provider())?;
    let mut state = AppState::with_skills(
        agent_core::provider::default_provider(),
        launch_cwd.clone(),
        SkillRegistry::discover(&launch_cwd),
    );
    state.backlog = backlog_store::load_backlog_for_workplace(bootstrap.runtime.workplace())?;
    for warning in bootstrap.runtime.apply_to_app_state(&mut state) {
        eprintln!("warning: {warning}");
    }
    match &bootstrap.kind {
        AgentBootstrapKind::Created => {
            eprintln!("agent: created {}", bootstrap.runtime.summary());
        }
        AgentBootstrapKind::Restored => {
            eprintln!("agent: restored {}", bootstrap.runtime.summary());
        }
        AgentBootstrapKind::RecreatedAfterError { error } => {
            eprintln!("warning: failed to restore agent runtime: {error}");
            eprintln!("agent: created replacement {}", bootstrap.runtime.summary());
        }
    }
    let mut agent_runtime = bootstrap.runtime;

    if resume_last {
        match agent_runtime.restore_state(&mut state) {
            Ok(restored) => {
                let _ = agent_runtime.restore_transcript(&mut state);
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

fn persist_agent_runtime_bundle(agent_runtime: &AgentRuntime, state: &AppState) -> Result<()> {
    agent_runtime.persist()?;
    agent_runtime.persist_state(state)?;
    agent_runtime.persist_transcript(state)?;
    agent_runtime.persist_messages(state)?;
    agent_runtime.persist_memory(state)?;
    Ok(())
}
