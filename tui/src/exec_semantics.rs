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

    let mut ops = Vec::new();
    for line in command.lines().map(str::trim).filter(|line| !line.is_empty()) {
        ops.extend(parse_single_line(line)?);
    }

    if ops.is_empty() {
        None
    } else {
        Some(ops)
    }
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
