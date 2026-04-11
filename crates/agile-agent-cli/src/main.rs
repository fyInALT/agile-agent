use agile_agent_core::probe;
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
    /// Print structured provider probe results.
    Probe {
        #[arg(long)]
        json: bool,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        None => agile_agent_tui::run_tui(),
        Some(Command::ResumeLast) => agile_agent_tui::run_tui_with_resume_last(),
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
