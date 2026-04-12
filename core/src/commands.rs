#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LocalCommand {
    Help,
    Provider,
    Skills,
    Doctor,
    Backlog,
    TodoAdd(String),
    RunOnce,
    Quit,
}

pub fn parse_local_command(input: &str) -> Option<Result<LocalCommand, String>> {
    let trimmed = input.trim();
    if !trimmed.starts_with('/') {
        return None;
    }

    let mut parts = trimmed.split_whitespace();
    let command = parts.next().unwrap_or(trimmed).trim_start_matches('/');
    let remainder = parts.collect::<Vec<_>>().join(" ");

    let parsed = match command {
        "help" => Ok(LocalCommand::Help),
        "provider" => Ok(LocalCommand::Provider),
        "skills" => Ok(LocalCommand::Skills),
        "doctor" => Ok(LocalCommand::Doctor),
        "backlog" => Ok(LocalCommand::Backlog),
        "todo-add" if !remainder.trim().is_empty() => Ok(LocalCommand::TodoAdd(remainder)),
        "todo-add" => Err("usage: /todo-add <title>".to_string()),
        "run-once" => Ok(LocalCommand::RunOnce),
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
        assert_eq!(
            parse_local_command("/backlog"),
            Some(Ok(LocalCommand::Backlog))
        );
        assert_eq!(
            parse_local_command("/run-once"),
            Some(Ok(LocalCommand::RunOnce))
        );
        assert_eq!(parse_local_command("/quit"), Some(Ok(LocalCommand::Quit)));
    }

    #[test]
    fn parses_todo_add_with_title() {
        assert_eq!(
            parse_local_command("/todo-add write readme"),
            Some(Ok(LocalCommand::TodoAdd("write readme".to_string())))
        );
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
