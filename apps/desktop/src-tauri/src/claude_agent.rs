//! Spawn the local `claude` CLI as a subprocess and stream its output back
//! to the frontend via per-request Tauri events.
//!
//! Frontend expects events named `claude-stream:{event_id}` with payload:
//! ```ignore
//! { kind: "tool_call" | "text" | "done" | "error",
//!   text?: string, tool_name?: string }
//! ```

use std::path::{Path, PathBuf};
use std::process::Stdio;

use serde::Serialize;
use tauri::{AppHandle, Emitter};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::Command;

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "snake_case")]
pub enum EventKind {
    ToolCall,
    Text,
    Done,
    Error,
}

#[derive(Serialize, Clone, Debug)]
pub struct StreamEvent {
    pub kind: EventKind,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_name: Option<String>,
}

impl StreamEvent {
    fn text(t: impl Into<String>) -> Self {
        Self {
            kind: EventKind::Text,
            text: Some(t.into()),
            tool_name: None,
        }
    }
    fn tool_call(name: impl Into<String>) -> Self {
        Self {
            kind: EventKind::ToolCall,
            text: None,
            tool_name: Some(name.into()),
        }
    }
    fn done() -> Self {
        Self {
            kind: EventKind::Done,
            text: None,
            tool_name: None,
        }
    }
    fn error(msg: impl Into<String>) -> Self {
        Self {
            kind: EventKind::Error,
            text: Some(msg.into()),
            tool_name: None,
        }
    }
}

/// Pure parser for one line of the `claude --output-format stream-json` output.
/// Returns `None` for empty lines, malformed JSON, or event types we don't care
/// about. Returns `Some(Done)` when the stream should terminate.
pub fn parse_stream_line(line: &str) -> Option<StreamEvent> {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return None;
    }
    let obj: serde_json::Value = serde_json::from_str(trimmed).ok()?;
    match obj.get("type")?.as_str()? {
        "assistant" => {
            let blocks = obj.get("message")?.get("content")?.as_array()?;
            for block in blocks {
                match block.get("type").and_then(|v| v.as_str()) {
                    Some("text") => {
                        let text = block.get("text").and_then(|v| v.as_str()).unwrap_or("");
                        if !text.is_empty() {
                            return Some(StreamEvent::text(text));
                        }
                    }
                    Some("tool_use") => {
                        let name = block
                            .get("name")
                            .and_then(|v| v.as_str())
                            .unwrap_or("(tool)");
                        return Some(StreamEvent::tool_call(name));
                    }
                    _ => {}
                }
            }
            None
        }
        "result" => Some(StreamEvent::done()),
        _ => None,
    }
}

/// Locate the `claude` binary. Mirrors `find_claude_binary` in lib.rs.
fn find_claude_binary() -> Option<PathBuf> {
    let candidates = [
        PathBuf::from("claude"),
        PathBuf::from("/opt/homebrew/bin/claude"),
        PathBuf::from("/usr/local/bin/claude"),
    ];
    candidates.into_iter().find(|c| {
        std::process::Command::new(c)
            .arg("--version")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
    })
}

/// Locate the bundled `decks` CLI used as the rekordagent MCP server.
///
/// Resolution order:
/// 1. `REKORDAGENT_MCP_BIN` env override (absolute path)
/// 2. Sibling of the running executable (production bundle layout)
/// 3. `target/debug/decks` and `target/release/decks` relative to cwd (dev)
fn find_decks_binary() -> Option<PathBuf> {
    if let Ok(p) = std::env::var("REKORDAGENT_MCP_BIN") {
        let path = PathBuf::from(p);
        if path.is_file() {
            return Some(path);
        }
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            let sibling = dir.join("decks");
            if sibling.is_file() {
                return Some(sibling);
            }
        }
    }
    for rel in ["target/debug/decks", "target/release/decks"] {
        let path = PathBuf::from(rel);
        if path.is_file() {
            return Some(path);
        }
    }
    None
}

fn emit(app: &AppHandle, event_name: &str, ev: StreamEvent) {
    let _ = app.emit(event_name, ev);
}

/// Cap on agent turns per message, so one request cannot loop indefinitely.
const MAX_TURNS: &str = "40";

/// Arguments for the chat subprocess.
///
/// This is a library chat surface, so the session gets exactly one set of
/// tools: the bundled rekordagent server. Built-in tools are off, only the MCP
/// config passed here is loaded, and the user's own settings, plugins and
/// skills are not read. `--print` cannot prompt, so `dontAsk` declines anything
/// that is not on the allowlist rather than waiting for an answer.
fn build_args(system: String, decks_bin: Option<&Path>) -> Vec<String> {
    let mut args: Vec<String> = [
        "--print",
        "--output-format",
        "stream-json",
        "--verbose",
        "--setting-sources",
        "",
        "--settings",
        r#"{"disableAllHooks":true}"#,
        "--strict-mcp-config",
        "--tools",
        "",
        "--disable-slash-commands",
        "--no-chrome",
        "--permission-mode",
        "dontAsk",
        "--max-turns",
        MAX_TURNS,
    ]
    .map(String::from)
    .to_vec();

    if let Some(bin) = decks_bin {
        let mcp_config = serde_json::json!({
            "mcpServers": {
                "rekordagent": {
                    "command": bin.to_string_lossy(),
                    "args": ["mcp"],
                }
            }
        })
        .to_string();
        args.extend([
            "--mcp-config".into(),
            mcp_config,
            "--allowedTools".into(),
            "mcp__rekordagent__*".into(),
        ]);
    }

    args.extend(["--system-prompt".into(), system]);
    args
}

/// Spawn `claude -p` and stream its output back via `claude-stream:{event_id}`.
pub async fn run(
    app: AppHandle,
    event_id: String,
    history: String,
    message: String,
    system: String,
) -> Result<(), String> {
    let event_name = format!("claude-stream:{event_id}");

    let Some(binary) = find_claude_binary() else {
        emit(
            &app,
            &event_name,
            StreamEvent::error("Claude CLI not found on PATH"),
        );
        emit(&app, &event_name, StreamEvent::done());
        return Ok(());
    };

    let prompt = if history.is_empty() {
        message
    } else {
        format!("{history}\n\nHuman: {message}")
    };

    // Auto-wire the rekordagent MCP server so library tools are available
    // without the user having to `claude mcp add` it themselves.
    let decks_bin = find_decks_binary();
    if decks_bin.is_none() {
        emit(
            &app,
            &event_name,
            StreamEvent::error(
                "rekordagent MCP binary (decks) not found — library tools unavailable. \
                 Set REKORDAGENT_MCP_BIN or build with `cargo build -p decks-cli`.",
            ),
        );
    }
    let args = build_args(system, decks_bin.as_deref());

    let mut child = match Command::new(&binary)
        .args(&args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
    {
        Ok(c) => c,
        Err(e) => {
            emit(
                &app,
                &event_name,
                StreamEvent::error(format!("Failed to spawn claude: {e}")),
            );
            emit(&app, &event_name, StreamEvent::done());
            return Ok(());
        }
    };

    if let Some(mut stdin) = child.stdin.take() {
        let _ = stdin.write_all(prompt.as_bytes()).await;
        let _ = stdin.shutdown().await;
        drop(stdin);
    }

    // Drain stderr concurrently so the pipe never fills (which would block
    // the child), and surface its contents for debugging.
    if let Some(stderr) = child.stderr.take() {
        tokio::spawn(async move {
            let mut lines = BufReader::new(stderr).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                eprintln!("[claude stderr] {line}");
            }
        });
    }

    let stdout = match child.stdout.take() {
        Some(s) => s,
        None => {
            emit(
                &app,
                &event_name,
                StreamEvent::error("no stdout from claude"),
            );
            emit(&app, &event_name, StreamEvent::done());
            let _ = child.wait().await;
            return Ok(());
        }
    };

    let mut lines = BufReader::new(stdout).lines();
    let mut got_done = false;
    while let Ok(Some(line)) = lines.next_line().await {
        if let Some(ev) = parse_stream_line(&line) {
            if matches!(ev.kind, EventKind::Done) {
                emit(&app, &event_name, ev);
                got_done = true;
                break;
            }
            emit(&app, &event_name, ev);
        }
    }

    let status = child.wait().await;
    match status {
        Ok(s) if s.success() => {
            if !got_done {
                emit(&app, &event_name, StreamEvent::done());
            }
        }
        Ok(s) => {
            emit(
                &app,
                &event_name,
                StreamEvent::error(format!("claude exited with status {s}")),
            );
            if !got_done {
                emit(&app, &event_name, StreamEvent::done());
            }
        }
        Err(e) => {
            emit(
                &app,
                &event_name,
                StreamEvent::error(format!("claude wait failed: {e}")),
            );
            if !got_done {
                emit(&app, &event_name, StreamEvent::done());
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_empty_line() {
        assert!(parse_stream_line("").is_none());
        assert!(parse_stream_line("   ").is_none());
    }

    #[test]
    fn parse_malformed_json() {
        assert!(parse_stream_line("not json {").is_none());
    }

    #[test]
    fn parse_assistant_text() {
        let line = r#"{"type":"assistant","message":{"content":[{"type":"text","text":"hello"}]}}"#;
        let ev = parse_stream_line(line).expect("event");
        assert!(matches!(ev.kind, EventKind::Text));
        assert_eq!(ev.text.as_deref(), Some("hello"));
    }

    #[test]
    fn parse_assistant_text_empty_skipped() {
        let line = r#"{"type":"assistant","message":{"content":[{"type":"text","text":""}]}}"#;
        assert!(parse_stream_line(line).is_none());
    }

    #[test]
    fn parse_assistant_tool_use() {
        let line = r#"{"type":"assistant","message":{"content":[{"type":"tool_use","name":"library__search","input":{}}]}}"#;
        let ev = parse_stream_line(line).expect("event");
        assert!(matches!(ev.kind, EventKind::ToolCall));
        assert_eq!(ev.tool_name.as_deref(), Some("library__search"));
    }

    #[test]
    fn parse_result_emits_done() {
        let line = r#"{"type":"result","subtype":"success"}"#;
        let ev = parse_stream_line(line).expect("event");
        assert!(matches!(ev.kind, EventKind::Done));
    }

    #[test]
    fn parse_hook_started_is_ignored() {
        // SessionStart hooks always fire first in the stream; they must NOT
        // terminate the read loop. Only `type:"result"` does.
        let line = r#"{"type":"system","subtype":"hook_started"}"#;
        assert!(parse_stream_line(line).is_none());
    }

    /// The value following `flag`.
    fn value<'a>(args: &'a [String], flag: &str) -> Option<&'a str> {
        let at = args.iter().position(|a| a == flag)?;
        args.get(at + 1).map(String::as_str)
    }

    #[test]
    fn the_chat_session_only_gets_the_bundled_library_tools() {
        let args = build_args("sys".into(), Some(Path::new("/opt/decks")));
        assert_eq!(value(&args, "--tools"), Some(""));
        assert_eq!(value(&args, "--allowedTools"), Some("mcp__rekordagent__*"));
        assert!(args.iter().any(|a| a == "--strict-mcp-config"));

        let config: serde_json::Value =
            serde_json::from_str(value(&args, "--mcp-config").unwrap()).unwrap();
        let servers = config["mcpServers"].as_object().unwrap();
        assert_eq!(servers.len(), 1);
        assert_eq!(servers["rekordagent"]["command"], "/opt/decks");
    }

    #[test]
    fn the_chat_session_never_skips_permission_checks() {
        for bin in [None, Some(Path::new("/opt/decks"))] {
            let args = build_args("sys".into(), bin);
            assert_eq!(value(&args, "--permission-mode"), Some("dontAsk"));
            assert!(!args.iter().any(|a| a.contains("bypassPermissions")));
            assert!(!args.iter().any(|a| a.contains("dangerously")));
        }
    }

    #[test]
    fn the_chat_session_ignores_user_settings_and_hooks() {
        let args = build_args("sys".into(), None);
        assert_eq!(value(&args, "--setting-sources"), Some(""));
        assert!(value(&args, "--settings")
            .unwrap()
            .contains("disableAllHooks"));
        assert!(value(&args, "--max-turns").is_some());
    }

    #[test]
    fn without_the_library_server_the_session_still_loads_no_other_tools() {
        let args = build_args("sys".into(), None);
        assert!(args.iter().any(|a| a == "--strict-mcp-config"));
        assert!(!args.iter().any(|a| a == "--mcp-config"));
        assert!(!args.iter().any(|a| a == "--allowedTools"));
    }

    #[test]
    fn parse_unknown_type_ignored() {
        let line = r#"{"type":"system","subtype":"init"}"#;
        assert!(parse_stream_line(line).is_none());
    }
}
