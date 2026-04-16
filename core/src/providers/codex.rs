use std::collections::HashSet;
use std::env;
use std::io::BufRead;
use std::io::BufReader;
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

use crate::logging;
use crate::probe::CODEX_PATH_ENV;
use crate::provider::ProviderEvent;
use crate::provider::SessionHandle;
use crate::tool_calls::ExecCommandStatus;
use crate::tool_calls::McpInvocation;
use crate::tool_calls::McpToolCallStatus;
use crate::tool_calls::PatchApplyStatus;
use crate::tool_calls::PatchChange;
use crate::tool_calls::PatchChangeKind;
use crate::tool_calls::WebSearchAction;

pub fn start(
    prompt: String,
    cwd: PathBuf,
    session_handle: Option<SessionHandle>,
    event_tx: Sender<ProviderEvent>,
) -> Result<()> {
    let executable = resolve_codex_executable()?;
    logging::debug_event(
        "provider.codex.start",
        "spawning Codex provider worker",
        serde_json::json!({
            "executable": executable,
            "cwd": cwd.display().to_string(),
            "session_handle": format!("{:?}", session_handle),
        }),
    );

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

    // Build exec mode args
    // SECURITY NOTE: --dangerously-bypass-approvals-and-sandbox is required because:
    // 1. codex app-server mode requires bubblewrap sandbox
    // 2. bubblewrap needs unprivileged user namespace creation
    // 3. Linux AppArmor blocks unprivileged user namespaces by default
    // This flag bypasses all approvals AND sandbox isolation.
    // Commands will execute without approval prompts.
    // Production systems should either:
    // - Use system-level fix: `sudo sysctl -w kernel.apparmor_restrict_unprivileged_userns=0`
    // - Or ensure this flag is only used in trusted environments
    let mut args: Vec<String> = vec![
        "exec".to_string(),
        "--dangerously-bypass-approvals-and-sandbox".to_string(),
        "--json".to_string(),
    ];

    // Handle session resume
    let thread_id_for_log = match session_handle {
        Some(SessionHandle::CodexThread { thread_id }) => {
            args.push("resume".to_string());
            args.push(thread_id.clone());
            Some(thread_id)
        }
        _ => None,
    };

    // Add prompt as final argument
    args.push(prompt.clone());

    command.args(&args);
    command.current_dir(&cwd);
    command.stdin(Stdio::null());  // exec mode doesn't need stdin
    command.stdout(Stdio::piped());
    command.stderr(Stdio::piped());

    let mut child = command
        .spawn()
        .with_context(|| format!("failed to start codex executable `{executable}`"))?;
    logging::debug_event(
        "provider.codex.process_spawned",
        "spawned Codex process",
        serde_json::json!({
            "executable": executable,
            "cwd": cwd.display().to_string(),
            "args": args,
            "resuming_thread": thread_id_for_log,
        }),
    );

    let stdout = child
        .stdout
        .take()
        .context("codex stdout pipe unavailable")?;
    let stderr = child
        .stderr
        .take()
        .context("codex stderr pipe unavailable")?;

    let stderr_handle = thread::spawn(move || read_stderr(stderr));
    let stdout_lines = BufReader::new(stdout).lines();

    let _ = event_tx.send(ProviderEvent::Status("codex provider started".to_string()));

    let mut streamed_agent_message_ids = HashSet::new();
    for line in stdout_lines {
        let line = line.context("failed to read line from codex stdout")?;
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        logging::debug_event(
            "provider.codex.stdout_line",
            "read raw Codex JSONL line",
            serde_json::json!({
                "line": trimmed,
            }),
        );

        let event = parse_exec_event(trimmed)?;
        if handle_exec_event(event, event_tx, &mut streamed_agent_message_ids)? {
            break;
        }
    }

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
                logging::debug_event(
                    "provider.codex.exit",
                    "Codex process exited successfully",
                    serde_json::json!({
                        "status": status.to_string(),
                        "forced_kill": false,
                    }),
                );
                return Ok(());
            }
            anyhow::bail!("codex exited with status {status}");
        }
        if Instant::now() >= deadline {
            child.kill().context("failed to kill codex process")?;
            let _ = child.wait();
            logging::warn_event(
                "provider.codex.exit",
                "forced Codex process shutdown after timeout",
                serde_json::json!({
                    "forced_kill": true,
                }),
            );
            return Ok(());
        }
        thread::sleep(Duration::from_millis(25));
    }
}

/// Event from codex exec --json output
#[derive(Debug, Deserialize)]
struct ExecEvent {
    #[serde(rename = "type")]
    event_type: String,
    #[serde(default)]
    thread_id: Option<String>,
    #[serde(default)]
    item: Option<serde_json::Value>,
    #[serde(default)]
    #[allow(dead_code)]
    usage: Option<serde_json::Value>,
}

fn parse_exec_event(line: &str) -> Result<ExecEvent> {
    serde_json::from_str(line).with_context(|| format!("invalid codex JSONL event: {line}"))
}

fn handle_exec_event(
    event: ExecEvent,
    event_tx: &Sender<ProviderEvent>,
    streamed_agent_message_ids: &mut HashSet<String>,
) -> Result<bool> {
    logging::debug_event(
        "provider.codex.event",
        "received Codex exec event",
        serde_json::json!({
            "type": event.event_type,
            "thread_id": event.thread_id,
            "item": event.item,
        }),
    );

    match event.event_type.as_str() {
        "thread.started" => {
            if let Some(thread_id) = event.thread_id {
                let _ = event_tx.send(ProviderEvent::SessionHandle(SessionHandle::CodexThread {
                    thread_id,
                }));
            }
            let _ = event_tx.send(ProviderEvent::Status("codex thread started".to_string()));
        }
        "turn.started" => {
            let _ = event_tx.send(ProviderEvent::Status("codex turn started".to_string()));
        }
        "turn.completed" => {
            let _ = event_tx.send(ProviderEvent::Status("codex turn completed".to_string()));
            return Ok(true); // End of turn
        }
        "item.started" | "item.completed" => {
            if let Some(item) = event.item {
                for e in parse_item_event(&event.event_type, &item, streamed_agent_message_ids) {
                    let _ = event_tx.send(e);
                }
            }
        }
        _ => {
            let _ = event_tx.send(ProviderEvent::Status(format!(
                "ignored codex event: {}",
                event.event_type
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
        ("item.started", "command_execution") => {
            let command = item
                .get("command")
                .and_then(|value| value.as_str())
                .map(ToOwned::to_owned);
            let source = item
                .get("source")
                .and_then(|value| value.as_str())
                .map(ToOwned::to_owned);
            vec![ProviderEvent::ExecCommandStarted {
                call_id: item_id,
                input_preview: command,
                source,
            }]
        }
        ("item.completed", "command_execution") => {
            let output = item
                .get("aggregated_output")
                .and_then(|value| value.as_str())
                .map(ToOwned::to_owned);
            let exit_code = item
                .get("exit_code")
                .and_then(|value| value.as_i64())
                .and_then(|value| i32::try_from(value).ok());
            let duration_ms = item
                .get("duration_ms")
                .and_then(|value| value.as_u64());
            let source = item
                .get("source")
                .and_then(|value| value.as_str())
                .map(ToOwned::to_owned);
            vec![ProviderEvent::ExecCommandFinished {
                call_id: item_id,
                output_preview: output,
                status: parse_exec_command_status(item, exit_code),
                exit_code,
                duration_ms,
                source,
            }]
        }
        ("item.started", "file_change") => vec![ProviderEvent::PatchApplyStarted {
            call_id: item_id,
            changes: parse_patch_changes(item),
        }],
        ("item.completed", "file_change") => vec![ProviderEvent::PatchApplyFinished {
            call_id: item_id,
            changes: parse_patch_changes(item),
            status: parse_patch_apply_status(item),
        }],
        ("item.started", "web_search") => item
            .get("query")
            .and_then(|value| value.as_str())
            .map(|query| {
                vec![ProviderEvent::WebSearchStarted {
                    call_id: item_id,
                    query: query.to_string(),
                }]
            })
            .unwrap_or_default(),
        ("item.completed", "web_search") => item
            .get("query")
            .and_then(|value| value.as_str())
            .map(|query| {
                vec![ProviderEvent::WebSearchFinished {
                    call_id: item_id,
                    query: query.to_string(),
                    action: item.get("action").and_then(parse_web_search_action),
                }]
            })
            .unwrap_or_default(),
        ("item.completed", "image_view") => item
            .get("path")
            .and_then(|value| value.as_str())
            .map(|path| {
                vec![ProviderEvent::ViewImage {
                    call_id: item_id,
                    path: path.to_string(),
                }]
            })
            .unwrap_or_default(),
        ("item.completed", "image_generation") => vec![ProviderEvent::ImageGenerationFinished {
            call_id: item_id,
            revised_prompt: item
                .get("revised_prompt")
                .and_then(|value| value.as_str())
                .map(ToOwned::to_owned),
            result: item
                .get("result")
                .and_then(|value| value.as_str())
                .map(ToOwned::to_owned),
            saved_path: item
                .get("saved_path")
                .and_then(|value| value.as_str())
                .map(ToOwned::to_owned),
        }],
        ("item.started", "mcp_tool_call") => {
            parse_mcp_invocation(item).map_or_else(Vec::new, |invocation| {
                vec![ProviderEvent::McpToolCallStarted {
                    call_id: item_id,
                    invocation,
                }]
            })
        }
        ("item.completed", "mcp_tool_call") => {
            parse_mcp_invocation(item).map_or_else(Vec::new, |invocation| {
                let (result_blocks, error, is_error) = parse_mcp_tool_call_result(item);
                vec![ProviderEvent::McpToolCallFinished {
                    call_id: item_id,
                    invocation,
                    result_blocks,
                    error,
                    status: parse_mcp_tool_call_status(item),
                    is_error,
                }]
            })
        }
        (_, "user_message") => Vec::new(),
        ("item.completed", "agent_message") => item
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

fn parse_patch_changes(item: &serde_json::Value) -> Vec<PatchChange> {
    item.get("changes")
        .and_then(|value| value.as_array())
        .into_iter()
        .flatten()
        .filter_map(parse_patch_change)
        .collect()
}

fn parse_patch_apply_status(item: &serde_json::Value) -> PatchApplyStatus {
    match item
        .get("status")
        .and_then(|value| value.as_str())
        .unwrap_or("completed")
    {
        "failed" => PatchApplyStatus::Failed,
        "declined" => PatchApplyStatus::Declined,
        "in_progress" => PatchApplyStatus::InProgress,
        _ => PatchApplyStatus::Completed,
    }
}

fn parse_mcp_tool_call_status(item: &serde_json::Value) -> McpToolCallStatus {
    match item
        .get("status")
        .and_then(|value| value.as_str())
        .unwrap_or("completed")
    {
        "failed" => McpToolCallStatus::Failed,
        "in_progress" => McpToolCallStatus::InProgress,
        _ => McpToolCallStatus::Completed,
    }
}

fn parse_exec_command_status(
    item: &serde_json::Value,
    exit_code: Option<i32>,
) -> ExecCommandStatus {
    match item
        .get("status")
        .and_then(|value| value.as_str())
        .unwrap_or_else(|| {
            if exit_code.unwrap_or(0) == 0 {
                "completed"
            } else {
                "failed"
            }
        }) {
        "declined" => ExecCommandStatus::Declined,
        "failed" => ExecCommandStatus::Failed,
        "in_progress" => ExecCommandStatus::InProgress,
        _ => ExecCommandStatus::Completed,
    }
}

fn parse_mcp_invocation(item: &serde_json::Value) -> Option<McpInvocation> {
    Some(McpInvocation {
        server: item
            .get("server")
            .and_then(|value| value.as_str())?
            .to_string(),
        tool: item
            .get("tool")
            .and_then(|value| value.as_str())?
            .to_string(),
        arguments: item.get("arguments").cloned(),
    })
}

fn parse_mcp_tool_call_result(
    item: &serde_json::Value,
) -> (Vec<serde_json::Value>, Option<String>, bool) {
    let result_blocks = item
        .get("result")
        .and_then(|result| result.get("content"))
        .and_then(|value| value.as_array())
        .cloned()
        .unwrap_or_default();
    let is_error = item
        .get("result")
        .and_then(|result| result.get("is_error").and_then(|value| value.as_bool()))
        .unwrap_or(false);
    let error = item
        .get("error")
        .and_then(|value| {
            value
                .get("message")
                .or_else(|| value.as_str().map(|_| value))
                .and_then(|message| message.as_str())
        })
        .map(ToOwned::to_owned);

    (result_blocks, error, is_error)
}

fn parse_web_search_action(action: &serde_json::Value) -> Option<WebSearchAction> {
    let action_type = action.get("type").and_then(|value| value.as_str())?;
    Some(match action_type {
        "search" => WebSearchAction::Search {
            query: action
                .get("query")
                .and_then(|value| value.as_str())
                .map(ToOwned::to_owned),
            queries: action.get("queries").and_then(|value| {
                value.as_array().map(|values| {
                    values
                        .iter()
                        .filter_map(|value| value.as_str().map(ToOwned::to_owned))
                        .collect::<Vec<_>>()
                })
            }),
        },
        "open_page" => WebSearchAction::OpenPage {
            url: action
                .get("url")
                .and_then(|value| value.as_str())
                .map(ToOwned::to_owned),
        },
        "find_in_page" => WebSearchAction::FindInPage {
            url: action
                .get("url")
                .and_then(|value| value.as_str())
                .map(ToOwned::to_owned),
            pattern: action
                .get("pattern")
                .and_then(|value| value.as_str())
                .map(ToOwned::to_owned),
        },
        _ => WebSearchAction::Other,
    })
}

fn parse_patch_change(change: &serde_json::Value) -> Option<PatchChange> {
    let path = change.get("path").and_then(|value| value.as_str())?;
    let kind = match change
        .get("kind")
        .and_then(|value| value.as_str())
        .unwrap_or("update")
    {
        "add" => PatchChangeKind::Add,
        "delete" => PatchChangeKind::Delete,
        _ => PatchChangeKind::Update,
    };
    let diff = change
        .get("diff")
        .and_then(|value| value.as_str())
        .unwrap_or("")
        .to_string();
    let (added, removed) = summarize_diff_counts(&diff);

    Some(PatchChange {
        path: path.to_string(),
        move_path: change
            .get("move_path")
            .and_then(|value| value.as_str())
            .map(ToOwned::to_owned),
        kind,
        diff,
        added,
        removed,
    })
}

fn summarize_diff_counts(diff: &str) -> (usize, usize) {
    let mut added = 0usize;
    let mut removed = 0usize;
    for line in diff.lines() {
        if line.starts_with('+') && !line.starts_with("+++") {
            added += 1;
        } else if line.starts_with('-') && !line.starts_with("---") {
            removed += 1;
        }
    }
    (added, removed)
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
    if item_type == "user_message" {
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
                    if let Some(thinking) = block.get("thinking").and_then(|value| value.as_str())
                        && !thinking.is_empty()
                    {
                        events.push(ProviderEvent::ThinkingChunk(thinking.to_string()));
                    }
                }
                "tool_use" => {
                    events.push(ProviderEvent::GenericToolCallStarted {
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
                    events.push(ProviderEvent::GenericToolCallFinished {
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
                        exit_code: None,
                        duration_ms: None,
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

    use super::ExecEvent;
    use super::ProviderEvent;
    use super::handle_exec_event;
    use super::parse_exec_event;
    use super::parse_item_event;
    use super::resolve_codex_executable_from;
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
    fn parses_exec_event_thread_started() {
        let line = r#"{"type":"thread.started","thread_id":"thr-cli-1"}"#;
        let event = parse_exec_event(line).expect("parse event");
        assert_eq!(event.event_type, "thread.started");
        assert_eq!(event.thread_id, Some("thr-cli-1".to_string()));
    }

    #[test]
    fn parses_exec_event_item_completed() {
        let line = r#"{"type":"item.completed","item":{"id":"item-1","type":"agent_message","text":"hello"}}"#;
        let event = parse_exec_event(line).expect("parse event");
        assert_eq!(event.event_type, "item.completed");
        assert!(event.item.is_some());
        let item = event.item.unwrap();
        assert_eq!(item.get("type").and_then(|v| v.as_str()), Some("agent_message"));
    }

    #[test]
    fn handle_exec_event_emits_session_handle() {
        let (tx, rx) = mpsc::channel();
        let mut streamed = HashSet::new();

        let event = ExecEvent {
            event_type: "thread.started".to_string(),
            thread_id: Some("thr-123".to_string()),
            item: None,
            usage: None,
        };

        let finished = handle_exec_event(event, &tx, &mut streamed).expect("handle event");
        assert!(!finished);
        assert_eq!(
            rx.recv().expect("session handle"),
            ProviderEvent::SessionHandle(SessionHandle::CodexThread {
                thread_id: "thr-123".to_string()
            })
        );
    }

    #[test]
    fn handle_exec_event_emits_assistant_chunk_from_item() {
        let (tx, rx) = mpsc::channel();
        let mut streamed = HashSet::new();

        let event = ExecEvent {
            event_type: "item.completed".to_string(),
            thread_id: None,
            item: Some(serde_json::json!({
                "id": "msg-1",
                "type": "agent_message",
                "text": "hello world"
            })),
            usage: None,
        };

        let finished = handle_exec_event(event, &tx, &mut streamed).expect("handle event");
        assert!(!finished);
        assert_eq!(
            rx.recv().expect("assistant chunk"),
            ProviderEvent::AssistantChunk("hello world".to_string())
        );
    }

    #[test]
    fn handle_exec_event_returns_true_on_turn_completed() {
        let (tx, _rx) = mpsc::channel();
        let mut streamed = HashSet::new();

        let event = ExecEvent {
            event_type: "turn.completed".to_string(),
            thread_id: None,
            item: None,
            usage: Some(serde_json::json!({"input_tokens": 100, "output_tokens": 50})),
        };

        let finished = handle_exec_event(event, &tx, &mut streamed).expect("handle event");
        assert!(finished);
    }

    #[test]
    fn parses_assistant_text_item_with_dot_format() {
        let item = serde_json::json!({
            "type": "message",
            "status": "completed",
            "content": [
                { "type": "text", "text": "hello world" }
            ]
        });

        let events = parse_item_event("item.completed", &item, &HashSet::new());
        assert_eq!(
            events,
            vec![ProviderEvent::AssistantChunk("hello world".to_string())]
        );
    }

    #[test]
    fn skips_completed_agent_message_text_after_streaming_deltas() {
        let item = serde_json::json!({
            "id": "msg-1",
            "type": "agent_message",
            "text": "full final text"
        });
        let mut streamed = HashSet::new();
        streamed.insert("msg-1".to_string());

        let events = parse_item_event("item.completed", &item, &streamed);
        assert!(events.is_empty());
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

    // Snake_case format tests (exec mode output format)
    #[test]
    fn parses_command_execution_with_snake_case_format() {
        let item = serde_json::json!({
            "id": "exec-1",
            "type": "command_execution",
            "aggregated_output": "done",
            "exit_code": 0,
            "duration_ms": 100,
            "status": "completed"
        });

        let events = parse_item_event("item.completed", &item, &HashSet::new());
        assert_eq!(
            events[0],
            ProviderEvent::ExecCommandFinished {
                call_id: Some("exec-1".to_string()),
                output_preview: Some("done".to_string()),
                status: crate::tool_calls::ExecCommandStatus::Completed,
                exit_code: Some(0),
                duration_ms: Some(100),
                source: None,
            }
        );
    }

    #[test]
    fn parses_agent_message_with_snake_case_format() {
        let item = serde_json::json!({
            "id": "msg-1",
            "type": "agent_message",
            "text": "hello from exec mode"
        });

        let events = parse_item_event("item.completed", &item, &HashSet::new());
        assert_eq!(
            events,
            vec![ProviderEvent::AssistantChunk("hello from exec mode".to_string())]
        );
    }

    #[test]
    fn parses_command_execution_in_progress_with_snake_case() {
        let item = serde_json::json!({
            "id": "exec-1",
            "type": "command_execution",
            "command": "ls -la",
            "aggregated_output": "",
            "exit_code": null,
            "status": "in_progress"
        });

        let events = parse_item_event("item.started", &item, &HashSet::new());
        assert_eq!(
            events[0],
            ProviderEvent::ExecCommandStarted {
                call_id: Some("exec-1".to_string()),
                input_preview: Some("ls -la".to_string()),
                source: None,
            }
        );
    }

    #[test]
    fn parses_file_change_with_snake_case_format() {
        let item = serde_json::json!({
            "id": "patch-1",
            "type": "file_change",
            "status": "completed",
            "changes": [
                {
                    "path": "/repo/lib.rs",
                    "kind": "update",
                    "diff": "+fn new() {}",
                    "move_path": null
                }
            ]
        });

        let events = parse_item_event("item.completed", &item, &HashSet::new());
        assert_eq!(
            events[0],
            ProviderEvent::PatchApplyFinished {
                call_id: Some("patch-1".to_string()),
                changes: vec![crate::tool_calls::PatchChange {
                    path: "/repo/lib.rs".to_string(),
                    move_path: None,
                    kind: crate::tool_calls::PatchChangeKind::Update,
                    diff: "+fn new() {}".to_string(),
                    added: 1,
                    removed: 0,
                }],
                status: crate::tool_calls::PatchApplyStatus::Completed,
            }
        );
    }

    #[test]
    fn parses_web_search_with_snake_case_format() {
        let started = serde_json::json!({
            "id": "search-1",
            "type": "web_search",
            "query": "rust tutorials"
        });
        let completed = serde_json::json!({
            "id": "search-1",
            "type": "web_search",
            "query": "rust tutorials",
            "action": { "type": "search", "query": "rust tutorials" }
        });

        assert_eq!(
            parse_item_event("item.started", &started, &HashSet::new()),
            vec![ProviderEvent::WebSearchStarted {
                call_id: Some("search-1".to_string()),
                query: "rust tutorials".to_string(),
            }]
        );
        assert_eq!(
            parse_item_event("item.completed", &completed, &HashSet::new()),
            vec![ProviderEvent::WebSearchFinished {
                call_id: Some("search-1".to_string()),
                query: "rust tutorials".to_string(),
                action: Some(crate::tool_calls::WebSearchAction::Search {
                    query: Some("rust tutorials".to_string()),
                    queries: None,
                }),
            }]
        );
    }

    #[test]
    fn parses_mcp_tool_call_with_snake_case_format() {
        let started = serde_json::json!({
            "id": "mcp-1",
            "type": "mcp_tool_call",
            "server": "filesystem",
            "tool": "read_file",
            "status": "in_progress",
            "arguments": { "path": "/tmp/test.txt" }
        });
        let completed = serde_json::json!({
            "id": "mcp-1",
            "type": "mcp_tool_call",
            "server": "filesystem",
            "tool": "read_file",
            "status": "completed",
            "arguments": { "path": "/tmp/test.txt" },
            "result": {
                "content": [{ "type": "text", "text": "file contents" }],
                "is_error": false
            }
        });

        assert_eq!(
            parse_item_event("item.started", &started, &HashSet::new()),
            vec![ProviderEvent::McpToolCallStarted {
                call_id: Some("mcp-1".to_string()),
                invocation: crate::tool_calls::McpInvocation {
                    server: "filesystem".to_string(),
                    tool: "read_file".to_string(),
                    arguments: Some(serde_json::json!({ "path": "/tmp/test.txt" })),
                },
            }]
        );
        assert_eq!(
            parse_item_event("item.completed", &completed, &HashSet::new()),
            vec![ProviderEvent::McpToolCallFinished {
                call_id: Some("mcp-1".to_string()),
                invocation: crate::tool_calls::McpInvocation {
                    server: "filesystem".to_string(),
                    tool: "read_file".to_string(),
                    arguments: Some(serde_json::json!({ "path": "/tmp/test.txt" })),
                },
                result_blocks: vec![serde_json::json!({
                    "type": "text",
                    "text": "file contents"
                })],
                error: None,
                status: crate::tool_calls::McpToolCallStatus::Completed,
                is_error: false,
            }]
        );
    }

    #[test]
    fn parses_image_types_with_snake_case_format() {
        let image_view = serde_json::json!({
            "id": "img-1",
            "type": "image_view",
            "path": "screenshot.png"
        });
        let image_generation = serde_json::json!({
            "id": "img-gen-1",
            "type": "image_generation",
            "status": "completed",
            "revised_prompt": "A colorful diagram",
            "result": "generated.png",
            "saved_path": "/tmp/generated.png"
        });

        assert_eq!(
            parse_item_event("item.completed", &image_view, &HashSet::new()),
            vec![ProviderEvent::ViewImage {
                call_id: Some("img-1".to_string()),
                path: "screenshot.png".to_string(),
            }]
        );
        assert_eq!(
            parse_item_event("item.completed", &image_generation, &HashSet::new()),
            vec![ProviderEvent::ImageGenerationFinished {
                call_id: Some("img-gen-1".to_string()),
                revised_prompt: Some("A colorful diagram".to_string()),
                result: Some("generated.png".to_string()),
                saved_path: Some("/tmp/generated.png".to_string()),
            }]
        );
    }
}
