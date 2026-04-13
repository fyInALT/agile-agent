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
        "find" => {
            let path = tokens
                .iter()
                .skip(1)
                .copied()
                .find(|token| !token.starts_with('-'))
                .map(ToOwned::to_owned)
                .unwrap_or_else(|| line.to_string());
            Some(vec![ExploringOp::List(path)])
        }
        "rg" | "grep" => {
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
}
