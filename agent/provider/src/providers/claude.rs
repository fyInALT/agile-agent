use std::env;
use std::io::Write;
use std::path::PathBuf;
use std::process::Command;
use std::process::Stdio;
use std::sync::mpsc::Sender;
use std::thread;

use anyhow::Context;
use anyhow::Result;
use serde::Deserialize;

use crate::launch_config::context::ProviderLaunchContext;
use crate::logging;
use crate::probe::CLAUDE_PATH_ENV;
use crate::provider::ProviderEvent;
use crate::provider::SessionHandle;

/// Start Claude provider with legacy signature (backward compatibility).
pub fn start(
    prompt: String,
    cwd: PathBuf,
    session_handle: Option<SessionHandle>,
    event_tx: Sender<ProviderEvent>,
) -> Result<()> {
    let executable = resolve_claude_executable()?;
    logging::debug_event(
        "provider.claude.start",
        "spawning Claude provider worker",
        serde_json::json!({
            "executable": executable,
            "cwd": cwd.display().to_string(),
            "session_handle": format!("{:?}", session_handle),
        }),
    );

    thread::Builder::new()
        .name("agent-claude-provider".to_string())
        .spawn(move || {
            let run_result = run_claude(prompt, cwd, session_handle, executable, &event_tx);
            if let Err(err) = run_result {
                let _ = event_tx.send(ProviderEvent::Error(err.to_string()));
            }
            let _ = event_tx.send(ProviderEvent::Finished);
        })
        .map(|_| ())
        .map_err(Into::into)
}

/// Start Claude provider with structured launch context.
pub fn start_with_context(
    context: ProviderLaunchContext,
    prompt: String,
    event_tx: Sender<ProviderEvent>,
) -> Result<()> {
    let executable = &context.spec.resolved_executable_path;
    let cwd = &context.cwd;
    let session_handle = context.session_handle.clone();

    logging::debug_event(
        "provider.claude.start_with_context",
        "spawning Claude provider with launch context",
        serde_json::json!({
            "executable": executable,
            "cwd": cwd.display().to_string(),
            "session_handle": format!("{:?}", session_handle),
            "extra_args": context.spec.extra_args,
            "env_count": context.spec.effective_env.len(),
        }),
    );

    thread::Builder::new()
        .name("agent-claude-provider".to_string())
        .spawn(move || {
            let run_result = run_claude_with_context(context, prompt, &event_tx);
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
    cwd: PathBuf,
    session_handle: Option<SessionHandle>,
    executable: String,
    event_tx: &Sender<ProviderEvent>,
) -> Result<()> {
    let args = build_claude_args(session_handle);
    let mut command = Command::new(&executable);
    command.args(&args);
    command.current_dir(&cwd);
    command.stdin(Stdio::piped());
    command.stdout(Stdio::piped());
    command.stderr(Stdio::piped());

    let mut child = command
        .spawn()
        .with_context(|| format!("failed to start claude executable `{executable}`"))?;

    // Send PID to event channel for lifecycle tracking
    let pid = child.id();
    let _ = event_tx.send(ProviderEvent::ProviderPid(pid));

    logging::debug_event(
        "provider.claude.process_spawned",
        "spawned Claude process",
        serde_json::json!({
            "executable": executable,
            "cwd": cwd.display().to_string(),
            "args": args,
            "pid": pid,
        }),
    );

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
    logging::debug_event(
        "provider.claude.stdin_payload",
        "writing raw Claude stdin payload",
        serde_json::json!({
            "payload": payload,
        }),
    );

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

        logging::debug_event(
            "provider.claude.exit",
            "Claude process exited successfully",
            serde_json::json!({
                "status": status.to_string(),
            }),
        );

        Ok(())
    })
}

/// Run Claude with structured launch context.
fn run_claude_with_context(
    context: ProviderLaunchContext,
    prompt: String,
    event_tx: &Sender<ProviderEvent>,
) -> Result<()> {
    let spec = &context.spec;
    let cwd = &context.cwd;

    // Build protocol args first
    let protocol_args = build_claude_args(context.session_handle.clone());

    // Inject extra args BEFORE protocol args
    let full_args: Vec<String> = spec
        .extra_args
        .iter()
        .chain(protocol_args.iter())
        .cloned()
        .collect();

    let mut command = Command::new(&spec.resolved_executable_path);
    command.args(&full_args);
    command.current_dir(cwd);

    // Use effective_env instead of implicit process environment
    for (key, value) in &spec.effective_env {
        command.env(key, value);
    }

    command.stdin(Stdio::piped());
    command.stdout(Stdio::piped());
    command.stderr(Stdio::piped());

    let executable = &spec.resolved_executable_path;
    let mut child = command
        .spawn()
        .with_context(|| format!("failed to start claude executable `{executable}`"))?;

    // Send PID to event channel for lifecycle tracking
    let pid = child.id();
    let _ = event_tx.send(ProviderEvent::ProviderPid(pid));

    logging::debug_event(
        "provider.claude.process_spawned_with_context",
        "spawned Claude process with launch context",
        serde_json::json!({
            "executable": executable,
            "cwd": cwd.display().to_string(),
            "args": full_args,
            "env_count": spec.effective_env.len(),
            "pid": pid,
        }),
    );

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

        logging::debug_event(
            "provider.claude.exit",
            "Claude process exited successfully",
            serde_json::json!({
                "status": status.to_string(),
            }),
        );

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
        logging::debug_event(
            "provider.claude.stdout_line",
            "read raw Claude stdout line",
            serde_json::json!({
                "line": trimmed,
            }),
        );
        for event in parse_output_line(trimmed)? {
            logging::debug_event(
                "provider.claude.event",
                "parsed Claude provider event",
                serde_json::json!({
                    "event_debug": format!("{:?}", event),
                }),
            );
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
    if !buffer.trim().is_empty() {
        logging::debug_event(
            "provider.claude.stderr",
            "read Claude stderr output",
            serde_json::json!({
                "stderr": buffer,
            }),
        );
    }
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
        "user" => parse_user_message(message),
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
        match block.r#type.as_str() {
            "text" if !block.text.is_empty() => {
                events.push(ProviderEvent::AssistantChunk(block.text));
            }
            "thinking" => {
                let thinking_text = if !block.text.is_empty() {
                    block.text
                } else {
                    block.thinking
                };
                if !thinking_text.is_empty() {
                    events.push(ProviderEvent::ThinkingChunk(thinking_text));
                }
            }
            "tool_use" => {
                let input_preview = block
                    .input
                    .as_ref()
                    .and_then(|value| serde_json::to_string(value).ok());
                events.push(ProviderEvent::GenericToolCallStarted {
                    name: if block.name.is_empty() {
                        "tool_use".to_string()
                    } else {
                        block.name
                    },
                    call_id: if block.id.is_empty() {
                        None
                    } else {
                        Some(block.id)
                    },
                    input_preview,
                });
            }
            _ => {}
        }
    }
    Ok(events)
}

fn parse_user_message(message: ClaudeSdkMessage) -> Result<Vec<ProviderEvent>> {
    let Some(payload) = message.message else {
        return Ok(Vec::new());
    };
    let content: ClaudeMessageContent =
        serde_json::from_value(payload).context("failed to decode user message content")?;

    let mut events = Vec::new();
    for block in content.content {
        if block.r#type == "tool_result" {
            let output_preview = block.content.as_ref().map(render_json_value);
            events.push(ProviderEvent::GenericToolCallFinished {
                name: "tool_result".to_string(),
                call_id: block.tool_use_id.clone(),
                output_preview,
                success: true,
                exit_code: None,
                duration_ms: None,
            });
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
    if let Some(log) = message.log
        && !log.message.is_empty()
    {
        return vec![ProviderEvent::Status(format!(
            "claude {}: {}",
            log.level, log.message
        ))];
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
    #[serde(default)]
    thinking: String,
    #[serde(default)]
    id: String,
    #[serde(default)]
    name: String,
    #[serde(default)]
    input: Option<serde_json::Value>,
    #[serde(default)]
    tool_use_id: Option<String>,
    #[serde(default)]
    content: Option<serde_json::Value>,
}

fn render_json_value(value: &serde_json::Value) -> String {
    value
        .as_str()
        .map(str::to_owned)
        .unwrap_or_else(|| value.to_string())
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
    fn parses_assistant_thinking_and_tool_use() {
        let line = r#"{"type":"assistant","message":{"role":"assistant","content":[{"type":"thinking","text":"plan first"},{"type":"tool_use","id":"call_1","name":"read_file","input":{"path":"README.md"}}]}}"#;

        let events = parse_output_line(line).expect("parse assistant line");

        assert_eq!(
            events,
            vec![
                ProviderEvent::ThinkingChunk("plan first".to_string()),
                ProviderEvent::GenericToolCallStarted {
                    name: "read_file".to_string(),
                    call_id: Some("call_1".to_string()),
                    input_preview: Some("{\"path\":\"README.md\"}".to_string()),
                }
            ]
        );
    }

    #[test]
    fn parses_user_tool_result() {
        let line = r#"{"type":"user","message":{"role":"user","content":[{"type":"tool_result","tool_use_id":"call_1","content":"{\"ok\":true}"}]}}"#;

        let events = parse_output_line(line).expect("parse user line");

        assert_eq!(
            events,
            vec![ProviderEvent::GenericToolCallFinished {
                name: "tool_result".to_string(),
                call_id: Some("call_1".to_string()),
                output_preview: Some("{\"ok\":true}".to_string()),
                success: true,
                exit_code: None,
                duration_ms: None,
            }]
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

    // Note: read_stdout_logs_raw_line_and_parsed_events test removed
    // This test requires full logging integration with WorkplaceStore
    // The test is preserved in agent-core where full logging is available
}
