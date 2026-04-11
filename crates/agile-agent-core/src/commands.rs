#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LocalCommand {
    Help,
    Provider,
    Skills,
    Doctor,
    Quit,
}

pub fn parse_local_command(input: &str) -> Option<Result<LocalCommand, String>> {
    let trimmed = input.trim();
    if !trimmed.starts_with('/') {
        return None;
    }

    let command = trimmed
        .split_whitespace()
        .next()
        .unwrap_or(trimmed)
        .trim_start_matches('/');

    let parsed = match command {
        "help" => Ok(LocalCommand::Help),
        "provider" => Ok(LocalCommand::Provider),
        "skills" => Ok(LocalCommand::Skills),
        "doctor" => Ok(LocalCommand::Doctor),
        "quit" => Ok(LocalCommand::Quit),
        other => Err(format!("unsupported command: /{other}")),
    };

    Some(parsed)
}

#[cfg(test)]
mod tests {
    use super::LocalCommand;
    use super::parse_local_command;

    #[test]
    fn parses_supported_commands() {
        assert_eq!(parse_local_command("/help"), Some(Ok(LocalCommand::Help)));
        assert_eq!(
            parse_local_command("/provider"),
            Some(Ok(LocalCommand::Provider))
        );
        assert_eq!(
            parse_local_command("/skills"),
            Some(Ok(LocalCommand::Skills))
        );
        assert_eq!(
            parse_local_command("/doctor"),
            Some(Ok(LocalCommand::Doctor))
        );
        assert_eq!(parse_local_command("/quit"), Some(Ok(LocalCommand::Quit)));
    }

    #[test]
    fn ignores_non_command_input() {
        assert_eq!(parse_local_command("hello"), None);
    }

    #[test]
    fn reports_unsupported_commands() {
        assert_eq!(
            parse_local_command("/unknown"),
            Some(Err("unsupported command: /unknown".to_string()))
        );
    }
}
