use crate::exec_command::strip_shell_wrapper;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ExploringOp {
    Read(String),
    List(String),
    Search { query: String, path: Option<String> },
}

pub(crate) fn parse_exploring_ops(command: &str, source: Option<&str>) -> Option<Vec<ExploringOp>> {
    if matches!(source, Some("userShell")) {
        return None;
    }

    let command = strip_shell_wrapper(command);
    let mut ops = Vec::new();
    for line in command.lines().map(str::trim).filter(|line| !line.is_empty()) {
        ops.extend(parse_single_line(&normalize_exploring_line(line))?);
    }

    if ops.is_empty() {
        None
    } else {
        Some(ops)
    }
}

fn normalize_exploring_line(line: &str) -> String {
    let mut line = line.trim().to_string();

    while line.starts_with("cd ") {
        let Some((_, tail)) = line.split_once("&&") else {
            break;
        };
        line = tail.trim().to_string();
    }

    if line.contains('|') {
        let segments = line
            .split('|')
            .map(str::trim)
            .filter(|segment| !segment.is_empty())
            .filter(|segment| *segment != "yes" && *segment != "no")
            .collect::<Vec<_>>();
        if let Some(segment) = segments.first() {
            line = (*segment).to_string();
        }
    }

    line
}

fn parse_single_line(line: &str) -> Option<Vec<ExploringOp>> {
    let tokens = line.split_whitespace().collect::<Vec<_>>();
    let cmd = *tokens.first()?;
    match cmd {
        "cat" => {
            let files = tokens
                .iter()
                .skip(1)
                .copied()
                .filter(|token| !token.starts_with('-'))
                .map(ToOwned::to_owned)
                .collect::<Vec<_>>();
            if files.is_empty() {
                None
            } else {
                Some(files.into_iter().map(ExploringOp::Read).collect())
            }
        }
        "sed" | "head" | "tail" => {
            let file = tokens
                .iter()
                .rev()
                .copied()
                .find(|token| !token.starts_with('-'))?;
            Some(vec![ExploringOp::Read(file.to_string())])
        }
        "ls" => {
            let path = tokens
                .iter()
                .skip(1)
                .copied()
                .find(|token| !token.starts_with('-'))
                .map(ToOwned::to_owned)
                .unwrap_or_else(|| line.to_string());
            Some(vec![ExploringOp::List(path)])
        }
        "find" | "eza" | "exa" => {
            let path = positional_args(tokens.iter().skip(1).copied())
                .last()
                .copied()
                .unwrap_or(line)
                .to_string();
            Some(vec![ExploringOp::List(path)])
        }
        "rg" | "grep" | "ag" | "ack" | "pt" | "rga" => {
            if matches!(tokens.get(1), Some(&"--files")) {
                return Some(vec![ExploringOp::List("rg --files".to_string())]);
            }
            let args = tokens
                .iter()
                .skip(1)
                .copied()
                .filter(|token| !token.starts_with('-'))
                .collect::<Vec<_>>();
            let query = args.first()?.to_string();
            let path = args.get(1).map(|value| (*value).to_string());
            Some(vec![ExploringOp::Search { query, path }])
        }
        _ => None,
    }
}

fn positional_args<'a>(args: impl IntoIterator<Item = &'a str>) -> Vec<&'a str> {
    let mut positional = Vec::new();
    let mut skip_next = false;
    for arg in args {
        if skip_next {
            skip_next = false;
            continue;
        }
        if matches!(arg, "-I" | "--ignore-glob" | "-g" | "--glob") {
            skip_next = true;
            continue;
        }
        if !arg.starts_with('-') {
            positional.push(arg);
        }
    }
    positional
}

#[cfg(test)]
mod tests {
    use super::ExploringOp;
    use super::parse_exploring_ops;

    #[test]
    fn bash_cd_then_cat_is_read() {
        assert_eq!(
            parse_exploring_ops("bash -lc 'cd foo && cat foo.txt'", Some("agent")),
            Some(vec![ExploringOp::Read("foo.txt".to_string())])
        );
    }

    #[test]
    fn shell_pipeline_with_yes_and_rg_files_is_list() {
        assert_eq!(
            parse_exploring_ops("bash -lc 'yes | rg --files'", Some("agent")),
            Some(vec![ExploringOp::List("rg --files".to_string())])
        );
    }

    #[test]
    fn shell_pipeline_with_rg_files_and_head_is_list() {
        assert_eq!(
            parse_exploring_ops("bash -c 'rg --files | head -n 1'", Some("agent")),
            Some(vec![ExploringOp::List("rg --files".to_string())])
        );
    }

    #[test]
    fn supports_ag_ack_pt_and_rga_search() {
        assert_eq!(
            parse_exploring_ops("ag TODO src", Some("agent")),
            Some(vec![ExploringOp::Search {
                query: "TODO".to_string(),
                path: Some("src".to_string()),
            }])
        );
        assert_eq!(
            parse_exploring_ops("ack TODO src", Some("agent")),
            Some(vec![ExploringOp::Search {
                query: "TODO".to_string(),
                path: Some("src".to_string()),
            }])
        );
        assert_eq!(
            parse_exploring_ops("pt TODO src", Some("agent")),
            Some(vec![ExploringOp::Search {
                query: "TODO".to_string(),
                path: Some("src".to_string()),
            }])
        );
        assert_eq!(
            parse_exploring_ops("rga TODO src", Some("agent")),
            Some(vec![ExploringOp::Search {
                query: "TODO".to_string(),
                path: Some("src".to_string()),
            }])
        );
    }

    #[test]
    fn supports_eza_and_exa_listing() {
        assert_eq!(
            parse_exploring_ops("eza --color=always src", Some("agent")),
            Some(vec![ExploringOp::List("src".to_string())])
        );
        assert_eq!(
            parse_exploring_ops("exa -I target .", Some("agent")),
            Some(vec![ExploringOp::List(".".to_string())])
        );
    }
}
