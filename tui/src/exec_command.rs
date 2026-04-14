pub(crate) fn strip_shell_wrapper(command: &str) -> String {
    let Some(parts) = shlex::split(command) else {
        return command.to_string();
    };

    if parts.len() >= 3
        && matches!(shell_basename(&parts[0]), "bash" | "sh" | "zsh")
        && matches!(parts[1].as_str(), "-c" | "-lc")
    {
        return parts[2].clone();
    }

    command.to_string()
}

fn shell_basename(command: &str) -> &str {
    command.rsplit('/').next().unwrap_or(command)
}

#[cfg(test)]
mod tests {
    use super::strip_shell_wrapper;

    #[test]
    fn strips_bash_lc_wrapper() {
        assert_eq!(strip_shell_wrapper("bash -lc 'echo hello'"), "echo hello");
        assert_eq!(
            strip_shell_wrapper("/bin/bash -lc 'echo hello'"),
            "echo hello"
        );
        assert_eq!(strip_shell_wrapper("zsh -lc 'echo hello'"), "echo hello");
    }

    #[test]
    fn preserves_plain_commands() {
        assert_eq!(strip_shell_wrapper("ls -la"), "ls -la");
    }
}
