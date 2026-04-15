use crate::command_bus::model::{CommandInvocation, CommandNamespace};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LocalCommand {
    Help,
    Provider,
    Skills,
    Doctor,
    Backlog,
    TodoAdd(String),
    RunOnce,
    RunLoop,
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
        "run-loop" => Ok(LocalCommand::RunLoop),
        "quit" => Ok(LocalCommand::Quit),
        other => Err(format!("unsupported command: /{other}")),
    };

    Some(parsed)
}

pub fn parse_legacy_alias(input: &str) -> Option<CommandInvocation> {
    let trimmed = input.trim();
    match trimmed {
        "/help" => Some(local_invocation(&["help"], &[])),
        "/provider" => Some(local_invocation(&["legacy", "provider"], &[])),
        "/skills" => Some(local_invocation(&["legacy", "skills"], &[])),
        "/doctor" => Some(local_invocation(&["legacy", "doctor"], &[])),
        "/backlog" => Some(local_invocation(&["kanban", "list"], &[])),
        "/run-once" => Some(local_invocation(&["legacy", "run-once"], &[])),
        "/run-loop" => Some(local_invocation(&["legacy", "run-loop"], &[])),
        "/quit" => Some(local_invocation(&["legacy", "quit"], &[])),
        _ if trimmed.starts_with("/todo-add ") => Some(local_invocation(
            &["legacy", "todo-add"],
            &[trimmed.trim_start_matches("/todo-add ").trim()],
        )),
        _ => None,
    }
}

fn local_invocation(path: &[&str], args: &[&str]) -> CommandInvocation {
    CommandInvocation {
        namespace: CommandNamespace::Local,
        target: None,
        path: path.iter().map(|value| value.to_string()).collect(),
        args: args.iter().map(|value| value.to_string()).collect(),
        raw_tail: None,
    }
}

#[cfg(test)]
mod tests {
    use super::LocalCommand;
    use super::parse_legacy_alias;
    use super::parse_local_command;
    use crate::command_bus::model::CommandNamespace;

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
        assert_eq!(
            parse_local_command("/run-loop"),
            Some(Ok(LocalCommand::RunLoop))
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

    #[test]
    fn legacy_help_alias_maps_to_local_help() {
        let parsed = parse_legacy_alias("/help").expect("alias");
        assert_eq!(parsed.namespace, CommandNamespace::Local);
        assert_eq!(parsed.path, vec!["help".to_string()]);
    }

    #[test]
    fn unsupported_flat_slash_command_is_not_a_legacy_alias() {
        assert!(parse_legacy_alias("/status").is_none());
    }
}
