use std::collections::HashSet;
use std::env;
use std::io::BufRead;
use std::io::BufReader;
use std::io::Write;
use std::path::PathBuf;
use std::process::Child;
use std::process::Command;
use std::process::Stdio;
use std::sync::mpsc::Sender;
use std::thread;
use std::time::Duration;
use std::time::Instant;

use anyhow::Context;
use anyhow::Result;
use serde::Deserialize;
use serde::Serialize;

use crate::probe::CODEX_PATH_ENV;
use crate::provider::ProviderEvent;
use crate::provider::SessionHandle;

pub fn start(
    prompt: String,
    cwd: PathBuf,
    session_handle: Option<SessionHandle>,
    event_tx: Sender<ProviderEvent>,
) -> Result<()> {
    let executable = resolve_codex_executable()?;

    thread::Builder::new()
        .name("agent-codex-provider".to_string())
        .spawn(move || {
            let run_result = run_codex(prompt, cwd, session_handle, executable, &event_tx);
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
    cwd: PathBuf,
    session_handle: Option<SessionHandle>,
    executable: String,
    event_tx: &Sender<ProviderEvent>,
) -> Result<()> {
    let mut command = Command::new(&executable);
    command.args(["app-server", "--listen", "stdio://"]);
    command.current_dir(&cwd);
    command.stdin(Stdio::piped());
    command.stdout(Stdio::piped());
    command.stderr(Stdio::piped());

    let mut child = command
        .spawn()
        .with_context(|| format!("failed to start codex executable `{executable}`"))?;

    let mut stdin = child.stdin.take().context("codex stdin pipe unavailable")?;
    let stdout = child
        .stdout
        .take()
        .context("codex stdout pipe unavailable")?;
    let stderr = child
        .stderr
        .take()
        .context("codex stderr pipe unavailable")?;

    let stderr_handle = thread::spawn(move || read_stderr(stderr));
    let mut stdout_lines = BufReader::new(stdout).lines();

    let _ = event_tx.send(ProviderEvent::Status("codex provider started".to_string()));

    send_request(
        &mut stdin,
        &JsonRpcRequest {
            jsonrpc: "2.0",
            id: 1,
            method: "initialize".to_string(),
            params: serde_json::json!({
                "clientInfo": {
                    "name": "agile-agent",
                    "title": "agile-agent",
                    "version": "0.1.0"
                },
                "capabilities": {
                    "experimentalApi": true
                }
            }),
        },
    )?;
    wait_for_response(&mut stdout_lines, &mut stdin, 1, event_tx, None)?;
    send_notification(&mut stdin, "initialized")?;

    let existing_thread = match session_handle {
        Some(SessionHandle::CodexThread { thread_id }) => Some(thread_id),
        _ => None,
    };

    let thread_request = if let Some(thread_id) = existing_thread.clone() {
        JsonRpcRequest {
            jsonrpc: "2.0",
            id: 2,
            method: "thread/resume".to_string(),
            params: serde_json::json!({
                "threadId": thread_id,
                "persistExtendedHistory": true
            }),
        }
    } else {
        JsonRpcRequest {
            jsonrpc: "2.0",
            id: 2,
            method: "thread/start".to_string(),
            params: serde_json::json!({
                "model": serde_json::Value::Null,
                "modelProvider": serde_json::Value::Null,
                "profile": serde_json::Value::Null,
                "cwd": cwd.display().to_string(),
                "approvalPolicy": serde_json::Value::Null,
                "sandbox": "workspace-write",
                "config": serde_json::Value::Null,
                "baseInstructions": serde_json::Value::Null,
                "developerInstructions": serde_json::Value::Null,
                "compactPrompt": serde_json::Value::Null,
                "includeApplyPatchTool": serde_json::Value::Null,
                "experimentalRawEvents": false,
                "persistExtendedHistory": true
            }),
        }
    };
    send_request(&mut stdin, &thread_request)?;
    let thread_response = wait_for_response(&mut stdout_lines, &mut stdin, 2, event_tx, None)?;

    let thread_id = existing_thread
        .or_else(|| thread_id_from_result(thread_response.result.as_ref()))
        .context("codex thread response did not include a thread id")?;
    let _ = event_tx.send(ProviderEvent::SessionHandle(SessionHandle::CodexThread {
        thread_id: thread_id.clone(),
    }));

    send_request(
        &mut stdin,
        &JsonRpcRequest {
            jsonrpc: "2.0",
            id: 3,
            method: "turn/start".to_string(),
            params: serde_json::json!({
                "threadId": thread_id,
                "input": [
                    {
                        "type": "text",
                        "text": prompt
                    }
                ]
            }),
        },
    )?;
    let _ = wait_for_response(&mut stdout_lines, &mut stdin, 3, event_tx, None)?;

    let mut turn_started = false;
    let mut turn_completed = false;
    let mut streamed_agent_message_ids = HashSet::new();
    while let Some(line) = stdout_lines.next() {
        let line = line.context("failed to read line from codex stdout")?;
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        let message = parse_jsonrpc_message(trimmed)?;
        if handle_message(
            message,
            &mut stdin,
            event_tx,
            &mut turn_started,
            &mut turn_completed,
            &mut streamed_agent_message_ids,
        )? {
            break;
        }
    }

    drop(stdin);
    wait_for_child_shutdown(&mut child)?;

    let stderr_output = stderr_handle.join().expect("codex stderr thread panicked");
    if !stderr_output.trim().is_empty() {
        let _ = event_tx.send(ProviderEvent::Status(format!(
            "codex stderr: {}",
            stderr_output.trim()
        )));
    }

    Ok(())
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

fn wait_for_child_shutdown(child: &mut Child) -> Result<()> {
    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        if let Some(status) = child.try_wait().context("failed to poll codex process")? {
            if status.success() {
                return Ok(());
            }
            anyhow::bail!("codex exited with status {status}");
        }
        if Instant::now() >= deadline {
            child.kill().context("failed to kill codex process")?;
            let _ = child.wait();
            return Ok(());
        }
        thread::sleep(Duration::from_millis(25));
    }
}

#[derive(Debug, Serialize)]
struct JsonRpcRequest {
    jsonrpc: &'static str,
    id: u64,
    method: String,
    params: serde_json::Value,
}

#[derive(Debug, Deserialize)]
struct JsonRpcMessage {
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

fn send_request(stdin: &mut impl Write, request: &JsonRpcRequest) -> Result<()> {
    let json = serde_json::to_string(request).context("failed to serialize codex request")?;
    stdin
        .write_all(json.as_bytes())
        .context("failed to write codex request")?;
    stdin.write_all(b"\n").context("failed to write newline")?;
    stdin.flush().context("failed to flush codex stdin")?;
    Ok(())
}

fn send_notification(stdin: &mut impl Write, method: &str) -> Result<()> {
    let payload = serde_json::json!({
        "jsonrpc": "2.0",
        "method": method
    });
    let json = serde_json::to_string(&payload).context("failed to serialize codex notification")?;
    stdin
        .write_all(json.as_bytes())
        .context("failed to write codex notification")?;
    stdin
        .write_all(b"\n")
        .context("failed to write notification newline")?;
    stdin
        .flush()
        .context("failed to flush codex notification")?;
    Ok(())
}

fn send_response(stdin: &mut impl Write, id: u64, result: serde_json::Value) -> Result<()> {
    let payload = serde_json::json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": result
    });
    let json = serde_json::to_string(&payload).context("failed to serialize codex response")?;
    stdin
        .write_all(json.as_bytes())
        .context("failed to write codex response")?;
    stdin
        .write_all(b"\n")
        .context("failed to write response newline")?;
    stdin.flush().context("failed to flush codex response")?;
    Ok(())
}

fn parse_jsonrpc_message(line: &str) -> Result<JsonRpcMessage> {
    serde_json::from_str(line).with_context(|| format!("invalid codex JSON-RPC message: {line}"))
}

fn wait_for_response(
    stdout_lines: &mut impl Iterator<Item = std::io::Result<String>>,
    stdin: &mut impl Write,
    target_id: u64,
    event_tx: &Sender<ProviderEvent>,
    turn_completed: Option<&mut bool>,
) -> Result<JsonRpcMessage> {
    let mut turn_completed = turn_completed;
    let mut streamed_agent_message_ids = HashSet::new();
    while let Some(line) = stdout_lines.next() {
        let line = line.context("failed to read line from codex stdout")?;
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let message = parse_jsonrpc_message(trimmed)?;
        if message.id == Some(target_id) && (message.result.is_some() || message.error.is_some()) {
            if let Some(error) = &message.error {
                anyhow::bail!("JSON-RPC error {}: {}", error.code, error.message);
            }
            return Ok(message);
        }

        let mut local_turn_started = false;
        if handle_message(
            message,
            stdin,
            event_tx,
            &mut local_turn_started,
            turn_completed.as_deref_mut().unwrap_or(&mut false),
            &mut streamed_agent_message_ids,
        )? {
            continue;
        }
    }

    anyhow::bail!("codex closed stdout while waiting for response {target_id}")
}

fn thread_id_from_result(result: Option<&serde_json::Value>) -> Option<String> {
    let result = result?;
    result
        .get("thread")
        .and_then(|thread| thread.get("id"))
        .and_then(|value| value.as_str())
        .map(ToOwned::to_owned)
}

fn handle_message(
    message: JsonRpcMessage,
    stdin: &mut impl Write,
    event_tx: &Sender<ProviderEvent>,
    turn_started: &mut bool,
    turn_completed: &mut bool,
    streamed_agent_message_ids: &mut HashSet<String>,
) -> Result<bool> {
    if let Some(method) = message.method {
        if let Some(id) = message.id {
            handle_server_request(method, id, stdin, event_tx)?;
            return Ok(false);
        }
        return handle_notification(
            method,
            message.params,
            event_tx,
            turn_started,
            turn_completed,
            streamed_agent_message_ids,
        );
    }

    if let Some(error) = message.error {
        let _ = event_tx.send(ProviderEvent::Error(format!(
            "JSON-RPC error {}: {}",
            error.code, error.message
        )));
    }

    Ok(false)
}

fn handle_server_request(
    method: String,
    id: u64,
    stdin: &mut impl Write,
    event_tx: &Sender<ProviderEvent>,
) -> Result<()> {
    let _ = event_tx.send(ProviderEvent::Status(format!(
        "codex server request: {method}"
    )));

    let decision = match method.as_str() {
        "item/commandExecution/requestApproval" | "execCommandApproval" => {
            serde_json::json!({ "decision": "accept" })
        }
        "item/fileChange/requestApproval" | "applyPatchApproval" => {
            serde_json::json!({ "decision": "accept" })
        }
        _ => serde_json::json!({}),
    };

    send_response(stdin, id, decision)
}

fn handle_notification(
    method: String,
    params: Option<serde_json::Value>,
    event_tx: &Sender<ProviderEvent>,
    turn_started: &mut bool,
    turn_completed: &mut bool,
    streamed_agent_message_ids: &mut HashSet<String>,
) -> Result<bool> {
    let params = params.unwrap_or(serde_json::Value::Null);

    match method.as_str() {
        "thread/started" => {
            if let Some(thread_id) = params
                .get("thread")
                .and_then(|thread| thread.get("id"))
                .and_then(|value| value.as_str())
            {
                let _ = event_tx.send(ProviderEvent::SessionHandle(SessionHandle::CodexThread {
                    thread_id: thread_id.to_string(),
                }));
            }
            let _ = event_tx.send(ProviderEvent::Status("codex thread started".to_string()));
        }
        "turn/started" => {
            *turn_started = true;
            let _ = event_tx.send(ProviderEvent::Status("codex turn started".to_string()));
        }
        "turn/completed" => {
            *turn_completed = true;
            let _ = event_tx.send(ProviderEvent::Status("codex turn completed".to_string()));
            return Ok(true);
        }
        "thread/status/changed" => {
            if params
                .get("status")
                .and_then(|status| status.get("type"))
                .and_then(|value| value.as_str())
                == Some("idle")
                && *turn_started
            {
                *turn_completed = true;
                return Ok(true);
            }
        }
        "item/agentMessage/delta" => {
            if let Some(item_id) = params.get("itemId").and_then(|value| value.as_str()) {
                streamed_agent_message_ids.insert(item_id.to_string());
            }
            if let Some(delta) = params.get("delta").and_then(|value| value.as_str()) {
                if !delta.is_empty() {
                    let _ = event_tx.send(ProviderEvent::AssistantChunk(delta.to_string()));
                }
            }
        }
        "item/started" | "item/completed" => {
            let item = params.get("item").unwrap_or(&params);
            for event in parse_item_event(method.as_str(), item, streamed_agent_message_ids) {
                let _ = event_tx.send(event);
            }
        }
        "configWarning"
        | "account/rateLimits/updated"
        | "thread/tokenUsage/updated"
        | "serverRequest/resolved"
        | "item/commandExecution/outputDelta"
        | "item/fileChange/outputDelta" => {}
        other => {
            let _ = event_tx.send(ProviderEvent::Status(format!(
                "ignored codex event: {other}"
            )));
        }
    }

    Ok(false)
}

fn parse_item_event(
    method: &str,
    item: &serde_json::Value,
    streamed_agent_message_ids: &HashSet<String>,
) -> Vec<ProviderEvent> {
    let item_type = item
        .get("type")
        .and_then(|value| value.as_str())
        .unwrap_or("");
    let item_id = item
        .get("id")
        .and_then(|value| value.as_str())
        .map(ToOwned::to_owned);

    match (method, item_type) {
        ("item/started", "commandExecution") => {
            let command = item
                .get("command")
                .and_then(|value| value.as_str())
                .map(ToOwned::to_owned);
            vec![ProviderEvent::ToolCallStarted {
                name: "exec_command".to_string(),
                call_id: item_id,
                input_preview: command,
            }]
        }
        ("item/completed", "commandExecution") => {
            let output = item
                .get("aggregatedOutput")
                .and_then(|value| value.as_str())
                .map(ToOwned::to_owned);
            vec![ProviderEvent::ToolCallFinished {
                name: "exec_command".to_string(),
                call_id: item_id,
                output_preview: output,
                success: true,
            }]
        }
        ("item/started", "fileChange") => vec![ProviderEvent::ToolCallStarted {
            name: "patch_apply".to_string(),
            call_id: item_id,
            input_preview: None,
        }],
        ("item/completed", "fileChange") => vec![ProviderEvent::ToolCallFinished {
            name: "patch_apply".to_string(),
            call_id: item_id,
            output_preview: None,
            success: true,
        }],
        (_, "userMessage") => Vec::new(),
        ("item/completed", "agentMessage") => item
            .get("text")
            .and_then(|value| value.as_str())
            .filter(|text| {
                !text.is_empty()
                    && !item_id
                        .as_ref()
                        .is_some_and(|id| streamed_agent_message_ids.contains(id))
            })
            .map(|text| vec![ProviderEvent::AssistantChunk(text.to_string())])
            .unwrap_or_default(),
        (_, _) => parse_content_blocks(item, streamed_agent_message_ids),
    }
}

fn parse_content_blocks(
    item: &serde_json::Value,
    streamed_agent_message_ids: &HashSet<String>,
) -> Vec<ProviderEvent> {
    let mut events = Vec::new();
    let item_type = item
        .get("type")
        .and_then(|value| value.as_str())
        .unwrap_or("");
    if item_type == "userMessage" {
        return events;
    }

    let item_status = item
        .get("status")
        .and_then(|value| value.as_str())
        .unwrap_or("completed");
    let item_id = item
        .get("id")
        .and_then(|value| value.as_str())
        .map(ToOwned::to_owned);

    if let Some(content) = item.get("content").and_then(|value| value.as_array()) {
        for block in content {
            let block_type = block
                .get("type")
                .and_then(|value| value.as_str())
                .unwrap_or("");
            match block_type {
                "text" => {
                    if let Some(text) = block.get("text").and_then(|value| value.as_str()) {
                        let already_streamed = item_id
                            .as_ref()
                            .is_some_and(|id| streamed_agent_message_ids.contains(id));
                        if !text.is_empty() && !already_streamed {
                            events.push(ProviderEvent::AssistantChunk(text.to_string()));
                        }
                    }
                }
                "thinking" => {
                    if let Some(thinking) = block.get("thinking").and_then(|value| value.as_str()) {
                        if !thinking.is_empty() {
                            events.push(ProviderEvent::ThinkingChunk(thinking.to_string()));
                        }
                    }
                }
                "tool_use" => {
                    events.push(ProviderEvent::ToolCallStarted {
                        name: block
                            .get("name")
                            .and_then(|value| value.as_str())
                            .unwrap_or("tool_use")
                            .to_string(),
                        call_id: block
                            .get("id")
                            .and_then(|value| value.as_str())
                            .map(ToOwned::to_owned),
                        input_preview: block
                            .get("arguments")
                            .and_then(|value| serde_json::to_string(value).ok()),
                    });
                }
                "tool_result" => {
                    events.push(ProviderEvent::ToolCallFinished {
                        name: block
                            .get("name")
                            .and_then(|value| value.as_str())
                            .unwrap_or("tool_result")
                            .to_string(),
                        call_id: block
                            .get("id")
                            .and_then(|value| value.as_str())
                            .map(ToOwned::to_owned),
                        output_preview: block
                            .get("output")
                            .and_then(|value| value.as_str())
                            .map(ToOwned::to_owned),
                        success: item_status != "error",
                    });
                }
                _ => {}
            }
        }
    }

    events
}

fn read_stderr(stderr: impl std::io::Read) -> String {
    let mut buffer = String::new();
    let mut reader = BufReader::new(stderr);
    let _ = std::io::Read::read_to_string(&mut reader, &mut buffer);
    buffer
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;
    use std::sync::mpsc;

    use super::JsonRpcMessage;
    use super::JsonRpcRequest;
    use super::ProviderEvent;
    use super::handle_notification;
    use super::parse_item_event;
    use super::parse_jsonrpc_message;
    use super::resolve_codex_executable_from;
    use super::thread_id_from_result;
    use crate::provider::SessionHandle;

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
                "clientInfo": {
                    "name": "agile-agent",
                    "title": "agile-agent",
                    "version": "0.1.0"
                },
                "capabilities": {
                    "experimentalApi": true
                }
            }),
        };

        let json = serde_json::to_string(&request).expect("serialize");
        assert!(json.contains("\"jsonrpc\":\"2.0\""));
        assert!(json.contains("\"method\":\"initialize\""));
        assert!(json.contains("\"clientInfo\""));
    }

    #[test]
    fn extracts_thread_id_from_response_result() {
        let result = serde_json::json!({
            "thread": {
                "id": "thr_123"
            }
        });

        assert_eq!(
            thread_id_from_result(Some(&result)),
            Some("thr_123".to_string())
        );
    }

    #[test]
    fn parses_jsonrpc_error() {
        let line = r#"{"id":1,"error":{"code":-32600,"message":"Invalid Request"}}"#;

        let response = parse_jsonrpc_message(line).expect("parse response");
        let error = response.error.expect("must have error");
        assert_eq!(error.code, -32600);
        assert_eq!(error.message, "Invalid Request");
    }

    #[test]
    fn parses_thread_started_notification() {
        let line = r#"{"method":"thread/started","params":{"thread":{"id":"thr_123"}}}"#;

        let response: JsonRpcMessage = parse_jsonrpc_message(line).expect("parse response");
        let params = response.params.expect("params");
        let thread_id = params
            .get("thread")
            .and_then(|thread| thread.get("id"))
            .and_then(|value| value.as_str());
        assert_eq!(thread_id, Some("thr_123"));
    }

    #[test]
    fn parses_assistant_text_item() {
        let item = serde_json::json!({
            "type": "message",
            "status": "completed",
            "content": [
                { "type": "text", "text": "hello world" }
            ]
        });

        let events = parse_item_event("item/completed", &item, &HashSet::new());
        assert_eq!(
            events,
            vec![ProviderEvent::AssistantChunk("hello world".to_string())]
        );
    }

    #[test]
    fn ignores_user_message_items() {
        let item = serde_json::json!({
            "id": "user-1",
            "type": "userMessage",
            "content": [
                { "type": "text", "text": "echoed user input" }
            ]
        });

        let events = parse_item_event("item/completed", &item, &HashSet::new());
        assert!(events.is_empty());
    }

    #[test]
    fn streams_agent_message_delta_notifications() {
        let (tx, rx) = mpsc::channel();
        let mut turn_started = false;
        let mut turn_completed = false;
        let mut streamed = HashSet::new();

        let finished = handle_notification(
            "item/agentMessage/delta".to_string(),
            Some(serde_json::json!({
                "delta": "hello",
                "itemId": "msg-1",
                "threadId": "thr-1",
                "turnId": "turn-1"
            })),
            &tx,
            &mut turn_started,
            &mut turn_completed,
            &mut streamed,
        )
        .expect("handle notification");

        assert!(!finished);
        assert!(streamed.contains("msg-1"));
        assert_eq!(
            rx.recv().expect("assistant chunk"),
            ProviderEvent::AssistantChunk("hello".to_string())
        );
    }

    #[test]
    fn skips_completed_agent_message_text_after_streaming_deltas() {
        let item = serde_json::json!({
            "id": "msg-1",
            "type": "agentMessage",
            "text": "full final text"
        });
        let mut streamed = HashSet::new();
        streamed.insert("msg-1".to_string());

        let events = parse_item_event("item/completed", &item, &streamed);
        assert!(events.is_empty());
    }

    #[test]
    fn suppresses_noisy_codex_notifications() {
        let (tx, rx) = mpsc::channel();
        let mut turn_started = false;
        let mut turn_completed = false;
        let mut streamed = HashSet::new();

        let finished = handle_notification(
            "item/commandExecution/outputDelta".to_string(),
            Some(serde_json::json!({
                "delta": "partial output"
            })),
            &tx,
            &mut turn_started,
            &mut turn_completed,
            &mut streamed,
        )
        .expect("handle notification");

        assert!(!finished);
        assert!(rx.try_recv().is_err());
    }

    #[test]
    fn parses_codex_session_handle_event_shape() {
        let handle = SessionHandle::CodexThread {
            thread_id: "thr_123".to_string(),
        };
        assert_eq!(
            handle,
            SessionHandle::CodexThread {
                thread_id: "thr_123".to_string()
            }
        );
    }
}
