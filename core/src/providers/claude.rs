use std::env;
use std::io::Write;
use std::process::Command;
use std::process::Stdio;
use std::sync::mpsc::Sender;
use std::thread;

use anyhow::Context;
use anyhow::Result;
use serde::Deserialize;

use crate::probe::CLAUDE_PATH_ENV;
use crate::provider::ProviderEvent;
use crate::provider::SessionHandle;

pub fn start(
    prompt: String,
    session_handle: Option<SessionHandle>,
    event_tx: Sender<ProviderEvent>,
) -> Result<()> {
    let executable = resolve_claude_executable()?;

    thread::Builder::new()
        .name("agile-agent-claude-provider".to_string())
        .spawn(move || {
            let run_result = run_claude(prompt, session_handle, executable, &event_tx);
            if let Err(err) = run_result {
                let _ = event_tx.send(ProviderEvent::Error(err.to_string()));
            }
            let _ = event_tx.send(ProviderEvent::Finished);
        })
        .map(|_| ())
        .map_err(Into::into)
}

fn run_claude(
    prompt: String,
    session_handle: Option<SessionHandle>,
    executable: String,
    event_tx: &Sender<ProviderEvent>,
) -> Result<()> {
    let args = build_claude_args(session_handle);
    let mut command = Command::new(&executable);
    command.args(&args);
    command.stdin(Stdio::piped());
    command.stdout(Stdio::piped());
    command.stderr(Stdio::piped());

    let mut child = command
        .spawn()
        .with_context(|| format!("failed to start claude executable `{executable}`"))?;

    let stdin = child
        .stdin
        .take()
        .context("claude stdin pipe unavailable")?;
    let stdout = child
        .stdout
        .take()
        .context("claude stdout pipe unavailable")?;
    let stderr = child
        .stderr
        .take()
        .context("claude stderr pipe unavailable")?;

    let payload = build_claude_input(&prompt)?;

    thread::scope(|scope| -> Result<()> {
        let write_handle = scope.spawn(|| -> Result<()> {
            let mut stdin = stdin;
            stdin
                .write_all(payload.as_bytes())
                .context("failed to write prompt to claude stdin")?;
            stdin.flush().context("failed to flush claude stdin")?;
            Ok(())
        });

        let stderr_handle = scope.spawn(|| read_stderr(stderr));
        let stdout_handle = scope.spawn(|| -> Result<()> {
            let _ = event_tx.send(ProviderEvent::Status("claude provider started".to_string()));
            read_stdout(stdout, event_tx)
        });

        write_handle.join().expect("claude stdin thread panicked")?;
        stdout_handle
            .join()
            .expect("claude stdout thread panicked")?;
        let stderr_output = stderr_handle.join().expect("claude stderr thread panicked");

        let status = child.wait().context("failed to wait on claude process")?;
        if !status.success() {
            let stderr_output = stderr_output.trim();
            if stderr_output.is_empty() {
                anyhow::bail!("claude exited with status {status}");
            } else {
                anyhow::bail!("claude exited with status {status}: {stderr_output}");
            }
        }

        Ok(())
    })
}

fn resolve_claude_executable() -> Result<String> {
    let configured = env::var(CLAUDE_PATH_ENV)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "claude".to_string());

    let resolved = resolve_claude_executable_from(&configured)?;
    Ok(resolved.display().to_string())
}

fn resolve_claude_executable_from(configured: &str) -> Result<std::path::PathBuf> {
    which::which(configured)
        .with_context(|| format!("claude executable not found at `{configured}`"))
}

fn build_claude_args(session_handle: Option<SessionHandle>) -> Vec<String> {
    let mut args = vec![
        "-p".to_string(),
        "--bare".to_string(),
        "--output-format".to_string(),
        "stream-json".to_string(),
        "--input-format".to_string(),
        "stream-json".to_string(),
        "--verbose".to_string(),
        "--strict-mcp-config".to_string(),
        "--permission-mode".to_string(),
        "bypassPermissions".to_string(),
    ];

    // Add --resume for multi-turn conversation
    if let Some(SessionHandle::ClaudeSession { session_id }) = session_handle {
        args.push("--resume".to_string());
        args.push(session_id);
    }

    args
}

fn build_claude_input(prompt: &str) -> Result<String> {
    let payload = serde_json::json!({
        "type": "user",
        "message": {
            "role": "user",
            "content": [
                {
                    "type": "text",
                    "text": prompt,
                }
            ]
        }
    });
    serde_json::to_string(&payload)
        .map(|json| format!("{json}\n"))
        .context("failed to serialize claude input")
}

fn read_stdout(stdout: impl std::io::Read, event_tx: &Sender<ProviderEvent>) -> Result<()> {
    let reader = std::io::BufReader::new(stdout);

    for line in std::io::BufRead::lines(reader) {
        let line = line.context("failed to read line from claude stdout")?;
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        for event in parse_output_line(trimmed)? {
            if event_tx.send(event).is_err() {
                return Ok(());
            }
        }
    }

    Ok(())
}

fn read_stderr(stderr: impl std::io::Read) -> String {
    let mut buffer = String::new();
    let mut reader = std::io::BufReader::new(stderr);
    let _ = std::io::Read::read_to_string(&mut reader, &mut buffer);
    buffer
}

fn parse_output_line(line: &str) -> Result<Vec<ProviderEvent>> {
    let message: ClaudeSdkMessage = serde_json::from_str(line)
        .with_context(|| format!("invalid claude output line: {line}"))?;

    match message.r#type.as_str() {
        "assistant" => parse_assistant_message(message),
        "system" => Ok(parse_system_message(message)),
        "result" => Ok(parse_result_message(message)),
        "log" => Ok(parse_log_message(message)),
        "user" => Ok(Vec::new()),
        other => Ok(vec![ProviderEvent::Status(format!(
            "ignored claude event type: {other}"
        ))]),
    }
}

fn parse_assistant_message(message: ClaudeSdkMessage) -> Result<Vec<ProviderEvent>> {
    let payload = message
        .message
        .context("assistant event missing message payload")?;
    let content: ClaudeMessageContent =
        serde_json::from_value(payload).context("failed to decode assistant message content")?;

    let mut events = Vec::new();
    for block in content.content {
        if block.r#type == "text" && !block.text.is_empty() {
            events.push(ProviderEvent::AssistantChunk(block.text));
        }
    }
    Ok(events)
}

fn parse_system_message(message: ClaudeSdkMessage) -> Vec<ProviderEvent> {
    let mut events = Vec::new();
    if let Some(session_id) = message.session_id.filter(|value| !value.is_empty()) {
        events.push(ProviderEvent::SessionHandle(SessionHandle::ClaudeSession {
            session_id: session_id.clone(),
        }));
        events.push(ProviderEvent::Status(format!(
            "claude session: {session_id}"
        )));
    }
    events
}

fn parse_result_message(message: ClaudeSdkMessage) -> Vec<ProviderEvent> {
    let mut events = Vec::new();

    if let Some(session_id) = message.session_id.filter(|value| !value.is_empty()) {
        events.push(ProviderEvent::SessionHandle(SessionHandle::ClaudeSession {
            session_id,
        }));
    }

    if message.is_error {
        events.push(ProviderEvent::Error(
            message
                .result
                .unwrap_or_else(|| "claude returned an error result".to_string()),
        ));
    }

    events
}

fn parse_log_message(message: ClaudeSdkMessage) -> Vec<ProviderEvent> {
    if let Some(log) = message.log {
        if !log.message.is_empty() {
            return vec![ProviderEvent::Status(format!(
                "claude {}: {}",
                log.level, log.message
            ))];
        }
    }
    Vec::new()
}

#[derive(Debug, Deserialize)]
struct ClaudeSdkMessage {
    #[serde(rename = "type")]
    r#type: String,
    #[serde(default)]
    message: Option<serde_json::Value>,
    #[serde(default)]
    session_id: Option<String>,
    #[serde(default)]
    result: Option<String>,
    #[serde(default)]
    is_error: bool,
    #[serde(default)]
    log: Option<ClaudeLogEntry>,
}

#[derive(Debug, Deserialize)]
struct ClaudeLogEntry {
    #[serde(default)]
    level: String,
    #[serde(default)]
    message: String,
}

#[derive(Debug, Deserialize)]
struct ClaudeMessageContent {
    content: Vec<ClaudeContentBlock>,
}

#[derive(Debug, Deserialize)]
struct ClaudeContentBlock {
    #[serde(rename = "type")]
    r#type: String,
    #[serde(default)]
    text: String,
}

#[cfg(test)]
mod tests {
    use crate::provider::SessionHandle;

    use super::ProviderEvent;
    use super::build_claude_args;
    use super::build_claude_input;
    use super::parse_output_line;
    use super::resolve_claude_executable_from;

    #[test]
    fn builds_stream_json_input() {
        let input = build_claude_input("hello").expect("build input");
        assert!(input.contains("\"type\":\"user\""));
        assert!(input.contains("\"text\":\"hello\""));
        assert!(input.ends_with('\n'));
    }

    #[test]
    fn includes_resume_arguments_when_session_handle_is_present() {
        let args = build_claude_args(Some(SessionHandle::ClaudeSession {
            session_id: "abc123".to_string(),
        }));

        assert!(
            args.windows(2)
                .any(|window| window == ["--resume", "abc123"])
        );
    }

    #[test]
    fn parses_assistant_text_chunks() {
        let line = r#"{"type":"assistant","message":{"role":"assistant","content":[{"type":"text","text":"hello"},{"type":"text","text":" world"}]}}"#;

        let events = parse_output_line(line).expect("parse assistant line");

        assert_eq!(
            events,
            vec![
                ProviderEvent::AssistantChunk("hello".to_string()),
                ProviderEvent::AssistantChunk(" world".to_string())
            ]
        );
    }

    #[test]
    fn parses_error_result() {
        let line = r#"{"type":"result","result":"boom","is_error":true}"#;

        let events = parse_output_line(line).expect("parse result line");

        assert_eq!(events, vec![ProviderEvent::Error("boom".to_string())]);
    }

    #[test]
    fn rejects_malformed_json_lines() {
        let error = parse_output_line("not-json").expect_err("must fail");
        let rendered = error.to_string();

        assert!(rendered.contains("invalid claude output line"));
    }

    #[test]
    fn parses_log_lines_into_status() {
        let line = r#"{"type":"log","log":{"level":"info","message":"starting"}}"#;

        let events = parse_output_line(line).expect("parse log line");

        assert_eq!(
            events,
            vec![ProviderEvent::Status("claude info: starting".to_string())]
        );
    }

    #[test]
    fn parses_session_updates_from_system_messages() {
        let line = r#"{"type":"system","session_id":"sess-1"}"#;

        let events = parse_output_line(line).expect("parse system line");

        assert_eq!(
            events,
            vec![
                ProviderEvent::SessionHandle(SessionHandle::ClaudeSession {
                    session_id: "sess-1".to_string()
                }),
                ProviderEvent::Status("claude session: sess-1".to_string())
            ]
        );
    }

    #[test]
    fn parses_session_updates_from_result_messages() {
        let line = r#"{"type":"result","session_id":"sess-2","is_error":false}"#;

        let events = parse_output_line(line).expect("parse result line");

        assert_eq!(
            events,
            vec![ProviderEvent::SessionHandle(SessionHandle::ClaudeSession {
                session_id: "sess-2".to_string()
            })]
        );
    }

    #[test]
    fn missing_executable_is_reported_clearly() {
        let error = resolve_claude_executable_from("definitely-not-a-real-claude-binary")
            .expect_err("resolution must fail");

        assert!(
            error
                .to_string()
                .contains("claude executable not found at `definitely-not-a-real-claude-binary`")
        );
    }
}
