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
        Some(Command::Doctor) => {
            println!("doctor is not implemented yet");
            Ok(())
        }
        Some(Command::Probe { json: true }) => {
            println!("{{\"status\":\"not_implemented\"}}");
            Ok(())
        }
        Some(Command::Probe { json: false }) => {
            println!("probe requires --json");
            Ok(())
        }
    }
}
