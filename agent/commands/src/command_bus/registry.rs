use crate::command_bus::model::CommandNamespace;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandSpec {
    pub namespace: CommandNamespace,
    pub path: &'static [&'static str],
    pub summary: &'static str,
    pub requires_target: bool,
    pub provider_passthrough: bool,
}

const COMMAND_SPECS: &[CommandSpec] = &[
    CommandSpec {
        namespace: CommandNamespace::Local,
        path: &["help"],
        summary: "Show slash command help",
        requires_target: false,
        provider_passthrough: false,
    },
    CommandSpec {
        namespace: CommandNamespace::Local,
        path: &["status"],
        summary: "Show agile-agent runtime status",
        requires_target: false,
        provider_passthrough: false,
    },
    CommandSpec {
        namespace: CommandNamespace::Local,
        path: &["clear"],
        summary: "Clear context for decision layer and work agent",
        requires_target: false,
        provider_passthrough: false,
    },
    CommandSpec {
        namespace: CommandNamespace::Local,
        path: &["kanban", "list"],
        summary: "List current kanban tasks",
        requires_target: false,
        provider_passthrough: false,
    },
    CommandSpec {
        namespace: CommandNamespace::Local,
        path: &["config", "get"],
        summary: "Read a session-scoped config value",
        requires_target: false,
        provider_passthrough: false,
    },
    CommandSpec {
        namespace: CommandNamespace::Local,
        path: &["config", "set"],
        summary: "Update a session-scoped config value",
        requires_target: false,
        provider_passthrough: false,
    },
    CommandSpec {
        namespace: CommandNamespace::Agent,
        path: &["status"],
        summary: "Show the resolved agent state",
        requires_target: false,
        provider_passthrough: false,
    },
    CommandSpec {
        namespace: CommandNamespace::Agent,
        path: &["summary"],
        summary: "Show a concise agent work summary",
        requires_target: false,
        provider_passthrough: false,
    },
    CommandSpec {
        namespace: CommandNamespace::Provider,
        path: &[],
        summary: "Pass a raw slash command to the resolved provider session",
        requires_target: false,
        provider_passthrough: true,
    },
];

pub fn command_spec(namespace: CommandNamespace, path: &[&str]) -> Option<&'static CommandSpec> {
    COMMAND_SPECS
        .iter()
        .find(|spec| spec.namespace == namespace && spec.path == path)
}

pub fn namespace_path_heads(namespace: CommandNamespace) -> Vec<&'static str> {
    COMMAND_SPECS
        .iter()
        .filter(|spec| spec.namespace == namespace && !spec.path.is_empty())
        .map(|spec| spec.path[0])
        .collect()
}

pub fn longest_registered_path_prefix(
    namespace: CommandNamespace,
    tokens: &[String],
) -> Option<Vec<String>> {
    COMMAND_SPECS
        .iter()
        .filter(|spec| spec.namespace == namespace && !spec.path.is_empty())
        .filter_map(|spec| {
            if spec.path.len() > tokens.len() {
                return None;
            }
            let matches = spec
                .path
                .iter()
                .zip(tokens.iter())
                .all(|(expected, actual)| expected == &actual.as_str());
            if matches {
                Some(
                    spec.path
                        .iter()
                        .map(|segment| segment.to_string())
                        .collect::<Vec<_>>(),
                )
            } else {
                None
            }
        })
        .max_by_key(|path| path.len())
}

pub fn render_local_help_lines() -> Vec<String> {
    vec![
        "available slash commands:".to_string(),
        "/local help".to_string(),
        "/local status".to_string(),
        "/local clear".to_string(),
        "/local kanban list".to_string(),
        "/local config get <key>".to_string(),
        "/local config set <key> <value>".to_string(),
        "/agent status".to_string(),
        "/agent <target> status".to_string(),
        "/agent summary".to_string(),
        "/provider /status".to_string(),
        "/provider <target> /status".to_string(),
    ]
}

#[cfg(test)]
mod tests {
    use super::{command_spec, render_local_help_lines};
    use crate::command_bus::model::CommandNamespace;

    #[test]
    fn registry_contains_local_status() {
        let spec = command_spec(CommandNamespace::Local, &["status"]).expect("spec");
        assert_eq!(spec.summary, "Show agile-agent runtime status");
    }

    #[test]
    fn help_lines_include_namespaced_commands() {
        let help = render_local_help_lines();
        assert!(help.iter().any(|line| line.contains("/local status")));
        assert!(help.iter().any(|line| line.contains("/agent status")));
        assert!(help.iter().any(|line| line.contains("/provider /status")));
    }
}
