use std::env;
use std::path::PathBuf;

use agent_core::app::AppState;
use agent_core::backlog_store;
use agent_core::loop_runner;
use agent_core::loop_runner::LoopGuardrails;
use agent_core::probe;
use agent_core::session_store;
use agent_core::skills::SkillRegistry;
use anyhow::Result;
use clap::Parser;
use clap::Subcommand;

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

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        None => agent_tui::run_tui(),
        Some(Command::ResumeLast) => agent_tui::run_tui_with_resume_last(),
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

fn run_loop_headless(max_iterations: usize, resume_last: bool) -> Result<()> {
    let launch_cwd = env::current_dir()?;
    let mut state = AppState::with_skills(
        agent_core::provider::default_provider(),
        launch_cwd.clone(),
        SkillRegistry::discover(&launch_cwd),
    );
    state.backlog = backlog_store::load_backlog()?;

    if resume_last {
        if let Ok(session) = session_store::load_recent_session() {
            let restored_cwd = PathBuf::from(&session.cwd);
            let effective_cwd = if restored_cwd.is_dir() {
                restored_cwd
            } else {
                launch_cwd
            };
            state.cwd = effective_cwd.clone();
            state.skills = SkillRegistry::discover(&effective_cwd);
            session.apply_to_app_state(&mut state);
        }
    }

    let initial_transcript_len = state.transcript.len();

    let summary = loop_runner::run_loop(
        &mut state,
        LoopGuardrails {
            max_iterations,
            max_continuations_per_task: 3,
        },
    )?;

    backlog_store::save_backlog(&state.backlog)?;
    session_store::save_recent_session(&state)?;

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
