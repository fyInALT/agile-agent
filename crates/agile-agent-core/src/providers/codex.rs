use std::env;
use std::io::{BufRead, BufReader, Write};
use std::process::{Command, Stdio};
use std::sync::mpsc::Sender;
use std::thread;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::probe::CODEX_PATH_ENV;
use crate::provider::{ProviderEvent, SessionHandle};

/// Starts a Codex provider turn.
///
/// For the first turn, uses `thread/start` to create a new thread.
/// For subsequent turns, uses `thread/resume` with the existing thread_id.
pub fn start(
    prompt: String,
    session_handle: Option<SessionHandle>,
    event_tx: Sender<ProviderEvent>,
) -> Result<()> {
    let executable = resolve_codex_executable()?;

    thread::Builder::new()
        .name("agile-agent-codex-provider".to_string())
        .spawn(move || {
            let run_result = run_codex(prompt, session_handle, executable, &event_tx);
            if let Err(err) = run_result {
                let _ = event_tx.send(ProviderEvent::Error(err.to_string()));
            }
            let _ = event_tx.send(ProviderEvent::Finished);
        })
        .map(|_| ())
        .map_err(Into::into)
}

fn run_codex(
    prompt: String,
    session_handle: Option<SessionHandle>,
    executable: String,
    event_tx: &Sender<ProviderEvent>,
) -> Result<()> {
    let mut command = Command::new(&executable);
    command.args(["app-server", "--listen", "stdio://"]);
    command.stdin(Stdio::piped());
    command.stdout(Stdio::piped());
    command.stderr(Stdio::piped());

    let mut child = command
        .spawn()
        .with_context(|| format!("failed to start codex executable `{executable}`"))?;

    let stdin = child.stdin.take().context("codex stdin pipe unavailable")?;
    let stdout = child.stdout.take().context("codex stdout pipe unavailable")?;
    let stderr = child.stderr.take().context("codex stderr pipe unavailable")?;

    thread::scope(|scope| -> Result<()> {
        let stdin_handle = scope.spawn(|| -> Result<()> {
            let mut stdin = stdin;
            run_jsonrpc_handshake(&mut stdin, prompt, session_handle)?;
            stdin.flush().context("failed to flush codex stdin")?;
            Ok(())
        });

        let stderr_handle = scope.spawn(|| read_stderr(stderr));
        let stdout_handle = scope.spawn(|| -> Result<()> {
            let _ = event_tx.send(ProviderEvent::Status("codex provider started".to_string()));
            read_jsonrpc_responses(stdout, event_tx)
        });

        stdin_handle.join().expect("codex stdin thread panicked")?;
        stdout_handle.join().expect("codex stdout thread panicked")?;
        let stderr_output = stderr_handle.join().expect("codex stderr thread panicked");

        let status = child.wait().context("failed to wait on codex process")?;
        if !status.success() {
            let stderr_output = stderr_output.trim();
            if stderr_output.is_empty() {
                anyhow::bail!("codex exited with status {status}");
            } else {
                anyhow::bail!("codex exited with status {status}: {stderr_output}");
            }
        }

        Ok(())
    })
}

fn resolve_codex_executable() -> Result<String> {
    let configured = env::var(CODEX_PATH_ENV)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "codex".to_string());

    let resolved = resolve_codex_executable_from(&configured)?;
    Ok(resolved.display().to_string())
}

fn resolve_codex_executable_from(configured: &str) -> Result<std::path::PathBuf> {
    which::which(configured)
        .with_context(|| format!("codex executable not found at `{configured}`"))
}

/// JSON-RPC 2.0 request structure
#[derive(Debug, Serialize)]
struct JsonRpcRequest {
    jsonrpc: &'static str,
    id: u64,
    method: String,
    params: serde_json::Value,
}

/// JSON-RPC 2.0 response structure
#[derive(Debug, Deserialize)]
struct JsonRpcResponse {
    #[serde(default)]
    id: Option<u64>,
    #[serde(default)]
    method: Option<String>,
    #[serde(default)]
    params: Option<serde_json::Value>,
    #[serde(default)]
    result: Option<serde_json::Value>,
    #[serde(default)]
    error: Option<JsonRpcError>,
}

#[derive(Debug, Deserialize)]
struct JsonRpcError {
    #[serde(default)]
    code: i64,
    #[serde(default)]
    message: String,
}

/// Codex-specific event structures
#[derive(Debug, Deserialize)]
struct CodexThreadStarted {
    #[serde(rename = "threadId")]
    thread_id: String,
}

#[derive(Debug, Deserialize)]
struct CodexTurnStarted {
    #[serde(default)]
    turn_id: String,
}

#[derive(Debug, Deserialize)]
struct CodexItemCreated {
    #[serde(default)]
    item: Option<CodexItem>,
}

#[derive(Debug, Deserialize)]
struct CodexItem {
    #[serde(default)]
    id: String,
    #[serde(rename = "type")]
    item_type: String,
    #[serde(default)]
    status: String,
    #[serde(default)]
    content: Vec<CodexContentBlock>,
}

#[derive(Debug, Deserialize)]
struct CodexContentBlock {
    #[serde(default)]
    id: String,
    #[serde(rename = "type")]
    content_type: String,
    #[serde(default)]
    text: String,
    #[serde(default)]
    thinking: String,
    #[serde(default)]
    name: String,
    #[serde(default)]
    arguments: Option<serde_json::Value>,
    #[serde(default)]
    output: Option<String>,
}

fn run_jsonrpc_handshake(
    stdin: &mut impl Write,
    prompt: String,
    session_handle: Option<SessionHandle>,
) -> Result<()> {
    // Step 1: Initialize
    let init_request = JsonRpcRequest {
        jsonrpc: "2.0",
        id: 1,
        method: "initialize".to_string(),
        params: serde_json::json!({
            "clientName": "agile-agent",
            "clientVersion": "0.1.0",
            "capabilities": {}
        }),
    };
    write_jsonrpc_request(stdin, &init_request)?;

    // Step 2: Start or resume thread
    let thread_request = match session_handle {
        Some(SessionHandle::CodexThread { thread_id }) => JsonRpcRequest {
            jsonrpc: "2.0",
            id: 2,
            method: "thread/resume".to_string(),
            params: serde_json::json!({
                "threadId": thread_id
            }),
        },
        _ => JsonRpcRequest {
            jsonrpc: "2.0",
            id: 2,
            method: "thread/start".to_string(),
            params: serde_json::json!({
                "cwd": env::current_dir()
                    .map(|p| p.display().to_string())
                    .unwrap_or_else(|_| ".".to_string()),
                "mode": "auto"
            }),
        },
    };
    write_jsonrpc_request(stdin, &thread_request)?;

    // Step 3: Start turn with user prompt
    let turn_request = JsonRpcRequest {
        jsonrpc: "2.0",
        id: 3,
        method: "turn/start".to_string(),
        params: serde_json::json!({
            "prompt": prompt
        }),
    };
    write_jsonrpc_request(stdin, &turn_request)?;

    Ok(())
}

fn write_jsonrpc_request(stdin: &mut impl Write, request: &JsonRpcRequest) -> Result<()> {
    let json = serde_json::to_string(request).context("failed to serialize JSON-RPC request")?;
    stdin
        .write_all(json.as_bytes())
        .context("failed to write JSON-RPC request")?;
    stdin.write_all(b"\n").context("failed to write newline")?;
    Ok(())
}

fn read_jsonrpc_responses(stdout: impl std::io::Read, event_tx: &Sender<ProviderEvent>) -> Result<()> {
    let reader = BufReader::new(stdout);

    for line in reader.lines() {
        let line = line.context("failed to read line from codex stdout")?;
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        for event in parse_jsonrpc_response(trimmed, event_tx)? {
            if event_tx.send(event).is_err() {
                return Ok(());
            }
        }
    }

    Ok(())
}

fn parse_jsonrpc_response(line: &str, event_tx: &Sender<ProviderEvent>) -> Result<Vec<ProviderEvent>> {
    let response: JsonRpcResponse = serde_json::from_str(line)
        .with_context(|| format!("invalid JSON-RPC response: {line}"))?;

    // Check for JSON-RPC error
    if let Some(error) = response.error {
        return Ok(vec![ProviderEvent::Error(format!(
            "JSON-RPC error {}: {}",
            error.code, error.message
        ))]);
    }

    // Handle notifications (methods without id)
    if let Some(method) = response.method {
        return parse_codex_notification(method, response.params, event_tx);
    }

    Ok(Vec::new())
}

fn parse_codex_notification(
    method: String,
    params: Option<serde_json::Value>,
    _event_tx: &Sender<ProviderEvent>,
) -> Result<Vec<ProviderEvent>> {
    let params = params.unwrap_or(serde_json::json!(null));

    match method.as_str() {
        "thread/started" => {
            let started: CodexThreadStarted = serde_json::from_value(params)
                .context("failed to parse thread/started")?;
            if !started.thread_id.is_empty() {
                Ok(vec![
                    ProviderEvent::SessionHandle(SessionHandle::CodexThread {
                        thread_id: started.thread_id,
                    }),
                    ProviderEvent::Status("codex thread started".to_string()),
                ])
            } else {
                Ok(vec![ProviderEvent::Status("codex thread started (no id)".to_string())])
            }
        }
        "turn/started" => {
            let started: CodexTurnStarted = serde_json::from_value(params)
                .context("failed to parse turn/started")?;
            Ok(vec![ProviderEvent::Status(format!(
                "codex turn started: {}",
                started.turn_id
            ))])
        }
        "turn/completed" => {
            Ok(vec![ProviderEvent::Status("codex turn completed".to_string())])
        }
        "item/created" => {
            let created: CodexItemCreated = serde_json::from_value(params)
                .context("failed to parse item/created")?;
            if let Some(item) = created.item {
                parse_codex_item(item)
            } else {
                Ok(Vec::new())
            }
        }
        "item/updated" => {
            // Item updates contain incremental content
            let item: CodexItem = serde_json::from_value(params)
                .context("failed to parse item/updated")?;
            parse_codex_item(item)
        }
        "log" => {
            if let Some(message) = params.get("message").and_then(|m| m.as_str()) {
                Ok(vec![ProviderEvent::Status(format!("codex: {}", message))])
            } else {
                Ok(Vec::new())
            }
        }
        other => Ok(vec![ProviderEvent::Status(format!(
            "ignored codex event: {}",
            other
        ))]),
    }
}

fn parse_codex_item(item: CodexItem) -> Result<Vec<ProviderEvent>> {
    let mut events = Vec::new();

    for block in item.content {
        match block.content_type.as_str() {
            "text" => {
                if !block.text.is_empty() {
                    events.push(ProviderEvent::AssistantChunk(block.text));
                }
            }
            "thinking" => {
                if !block.thinking.is_empty() {
                    events.push(ProviderEvent::ThinkingChunk(block.thinking));
                }
            }
            "tool_use" => {
                events.push(ProviderEvent::ToolCallStarted {
                    name: block.name,
                    call_id: Some(block.id),
                    input_preview: block.arguments.and_then(|a| serde_json::to_string(&a).ok()),
                });
            }
            "tool_result" => {
                events.push(ProviderEvent::ToolCallFinished {
                    name: block.name,
                    call_id: Some(block.id),
                    output_preview: block.output,
                    success: item.status != "error",
                });
            }
            other => {
                events.push(ProviderEvent::Status(format!(
                    "ignored content type: {}",
                    other
                )));
            }
        }
    }

    Ok(events)
}

fn read_stderr(stderr: impl std::io::Read) -> String {
    let mut buffer = String::new();
    let mut reader = BufReader::new(stderr);
    let _ = std::io::Read::read_to_string(&mut reader, &mut buffer);
    buffer
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_missing_executable_with_clear_error() {
        let error = resolve_codex_executable_from("definitely-not-a-real-codex-binary")
            .expect_err("resolution must fail");

        assert!(
            error
                .to_string()
                .contains("codex executable not found at `definitely-not-a-real-codex-binary`")
        );
    }

    #[test]
    fn builds_initialize_request() {
        let request = JsonRpcRequest {
            jsonrpc: "2.0",
            id: 1,
            method: "initialize".to_string(),
            params: serde_json::json!({
                "clientName": "agile-agent",
                "clientVersion": "0.1.0"
            }),
        };

        let json = serde_json::to_string(&request).expect("serialize");
        assert!(json.contains("\"jsonrpc\":\"2.0\""));
        assert!(json.contains("\"method\":\"initialize\""));
    }

    #[test]
    fn parses_thread_started_notification() {
        let line = r#"{"method":"thread/started","params":{"threadId":"abc123"}}"#;

        let response: JsonRpcResponse = serde_json::from_str(line).expect("parse response");
        assert_eq!(response.method, Some("thread/started".to_string()));

        let started: CodexThreadStarted =
            serde_json::from_value(response.params.unwrap()).expect("parse params");
        assert_eq!(started.thread_id, "abc123");
    }

    #[test]
    fn parses_assistant_text_item() {
        let line = r#"{"method":"item/updated","params":{"id":"msg1","type":"message","status":"completed","content":[{"type":"text","text":"hello world"}]}}"#;

        let response: JsonRpcResponse = serde_json::from_str(line).expect("parse response");

        let item: CodexItem = serde_json::from_value(response.params.unwrap()).expect("parse item");
        assert_eq!(item.content.len(), 1);
        assert_eq!(item.content[0].content_type, "text");
        assert_eq!(item.content[0].text, "hello world");
    }

    #[test]
    fn parses_jsonrpc_error() {
        let line = r#"{"id":1,"error":{"code":-32600,"message":"Invalid Request"}}"#;

        let response: JsonRpcResponse = serde_json::from_str(line).expect("parse response");
        let error = response.error.expect("must have error");
        assert_eq!(error.code, -32600);
        assert_eq!(error.message, "Invalid Request");
    }
}
