use crate::command_bus::model::{
    CommandInvocation, CommandNamespace, CommandParseError, CommandTargetSpec, ParsedSlashCommand,
};

pub fn parse_slash_command(input: &str) -> Result<ParsedSlashCommand, CommandParseError> {
    let trimmed = input.trim();
    if !trimmed.starts_with('/') {
        return Err(CommandParseError::new("slash commands must start with `/`"));
    }

    let tokens =
        shlex::split(trimmed).ok_or_else(|| CommandParseError::new("invalid slash command quoting"))?;
    let Some(first) = tokens.first() else {
        return Err(CommandParseError::new("empty slash command"));
    };

    match first.as_str() {
        "/local" => parse_local(&tokens),
        "/agent" => parse_agent(&tokens),
        "/provider" => parse_provider(&tokens),
        other => Err(CommandParseError::new(format!(
            "unsupported slash namespace: {}",
            other
        ))),
    }
}

fn parse_local(tokens: &[String]) -> Result<ParsedSlashCommand, CommandParseError> {
    if tokens.len() < 2 {
        return Err(CommandParseError::new("usage: /local <path...>"));
    }

    let (path, args) = split_path_and_args(&tokens[1..], None);
    if path.is_empty() {
        return Err(CommandParseError::new("usage: /local <path...>"));
    }

    Ok(ParsedSlashCommand::Invocation(CommandInvocation {
        namespace: CommandNamespace::Local,
        target: None,
        path,
        args,
        raw_tail: None,
    }))
}

fn parse_agent(tokens: &[String]) -> Result<ParsedSlashCommand, CommandParseError> {
    if tokens.len() < 2 {
        return Err(CommandParseError::new("usage: /agent [target] <path...>"));
    }

    let known_commands = ["status", "summary"];
    let (target, command_start) = if tokens.len() >= 3 && !known_commands.contains(&tokens[1].as_str()) {
        (Some(CommandTargetSpec::AgentName(tokens[1].clone())), 2usize)
    } else {
        (None, 1usize)
    };

    let (path, args) = split_path_and_args(&tokens[command_start..], Some(&["status", "summary"]));
    if path.is_empty() {
        return Err(CommandParseError::new("usage: /agent [target] <path...>"));
    }

    Ok(ParsedSlashCommand::Invocation(CommandInvocation {
        namespace: CommandNamespace::Agent,
        target,
        path,
        args,
        raw_tail: None,
    }))
}

fn parse_provider(tokens: &[String]) -> Result<ParsedSlashCommand, CommandParseError> {
    if tokens.len() < 2 {
        return Err(CommandParseError::new(
            "usage: /provider [target] /provider-native-command",
        ));
    }

    let (target, raw_index) = if tokens.len() >= 3 && tokens[2].starts_with('/') {
        (Some(CommandTargetSpec::AgentName(tokens[1].clone())), 2usize)
    } else {
        (None, 1usize)
    };

    let Some(raw_tail) = tokens.get(raw_index) else {
        return Err(CommandParseError::new(
            "usage: /provider [target] /provider-native-command",
        ));
    };
    if !raw_tail.starts_with('/') {
        return Err(CommandParseError::new(
            "provider commands must use raw slash passthrough syntax like `/provider /status`",
        ));
    }

    Ok(ParsedSlashCommand::Invocation(CommandInvocation {
        namespace: CommandNamespace::Provider,
        target,
        path: vec![],
        args: vec![],
        raw_tail: Some(raw_tail.clone()),
    }))
}

fn split_path_and_args(
    tokens: &[String],
    known_paths: Option<&[&str]>,
) -> (Vec<String>, Vec<String>) {
    if let Some(known_paths) = known_paths {
        if let Some((first, rest)) = tokens.split_first() {
            if known_paths.contains(&first.as_str()) {
                return (vec![first.clone()], rest.to_vec());
            }
        }
    }

    match tokens.len() {
        0 => (vec![], vec![]),
        1 => (vec![tokens[0].clone()], vec![]),
        _ => {
            let path_len = match tokens[0].as_str() {
                "kanban" | "config" => 2.min(tokens.len()),
                _ => 1,
            };
            let (path, args) = tokens.split_at(path_len);
            (path.to_vec(), args.to_vec())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::parse_slash_command;
    use crate::command_bus::model::{
        CommandInvocation, CommandNamespace, CommandTargetSpec, ParsedSlashCommand,
    };

    #[test]
    fn parses_local_command_path_and_args() {
        let parsed = parse_slash_command("/local kanban list").expect("parse");
        assert_eq!(
            parsed,
            ParsedSlashCommand::Invocation(CommandInvocation {
                namespace: CommandNamespace::Local,
                target: None,
                path: vec!["kanban".to_string(), "list".to_string()],
                args: vec![],
                raw_tail: None,
            })
        );
    }

    #[test]
    fn parses_agent_command_with_explicit_target() {
        let parsed = parse_slash_command("/agent alpha status").expect("parse");
        assert_eq!(
            parsed,
            ParsedSlashCommand::Invocation(CommandInvocation {
                namespace: CommandNamespace::Agent,
                target: Some(CommandTargetSpec::AgentName("alpha".to_string())),
                path: vec!["status".to_string()],
                args: vec![],
                raw_tail: None,
            })
        );
    }

    #[test]
    fn parses_provider_passthrough_without_target() {
        let parsed = parse_slash_command("/provider /status").expect("parse");
        assert_eq!(
            parsed,
            ParsedSlashCommand::Invocation(CommandInvocation {
                namespace: CommandNamespace::Provider,
                target: None,
                path: vec![],
                args: vec![],
                raw_tail: Some("/status".to_string()),
            })
        );
    }

    #[test]
    fn parses_provider_passthrough_with_target() {
        let parsed = parse_slash_command("/provider alpha /status").expect("parse");
        assert_eq!(
            parsed,
            ParsedSlashCommand::Invocation(CommandInvocation {
                namespace: CommandNamespace::Provider,
                target: Some(CommandTargetSpec::AgentName("alpha".to_string())),
                path: vec![],
                args: vec![],
                raw_tail: Some("/status".to_string()),
            })
        );
    }

    #[test]
    fn parses_quoted_arguments_for_local_config() {
        let parsed = parse_slash_command("/local config set ui.title \"My Agile Agent\"")
            .expect("parse");
        let ParsedSlashCommand::Invocation(invocation) = parsed;
        assert_eq!(
            invocation.path,
            vec!["config".to_string(), "set".to_string()]
        );
        assert_eq!(
            invocation.args,
            vec!["ui.title".to_string(), "My Agile Agent".to_string()]
        );
    }

    #[test]
    fn rejects_provider_without_raw_slash_tail() {
        let error = parse_slash_command("/provider status").expect_err("must fail");
        assert_eq!(
            error.to_string(),
            "provider commands must use raw slash passthrough syntax like `/provider /status`"
        );
    }
}
