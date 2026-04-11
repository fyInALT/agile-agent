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
            print!("{}", render_doctor(&probe::probe_report()));
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

fn render_doctor(report: &probe::ProbeReport) -> String {
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
