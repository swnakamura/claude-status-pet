/// Claude Code adapter
///
/// Parses Claude Code hook JSON from stdin:
/// - Events: PascalCase (UserPromptSubmit, PreToolUse, Stop, etc.)
/// - Tool names: PascalCase (Edit, Read, Bash, Grep, etc.)
/// - Tool input: snake_case keys (file_path, command, etc.)
/// - Session ID: provided in JSON

use super::{Adapter, NormalizedEvent, StdinInput, basename, truncate, extract_str};
use std::path::Path;

pub struct ClaudeAdapter;

impl Adapter for ClaudeAdapter {
    fn parse(&self, stdin: &StdinInput) -> Option<NormalizedEvent> {
        let hook = stdin.hook_event_name.as_deref().unwrap_or("unknown");
        let tool_name = stdin.tool_name.as_deref().unwrap_or("");
        let tool_input = stdin.tool_input.as_ref();
        let cwd = stdin.cwd.as_deref().unwrap_or("");
        let session_id = stdin.session_id.as_deref().unwrap_or("unknown");
        let cwd_name = Path::new(cwd)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or(session_id)
            .to_string();
        // A session the user renamed (/rename) shows that name; otherwise the directory name.
        let session_name = user_session_name(session_id).unwrap_or(cwd_name);

        let (event, tool, detail) = match hook {
            "UserPromptSubmit" => {
                ("prompt".into(), String::new(), "Processing your prompt...".into())
            }
            "PreToolUse" => {
                let file = extract_file(tool_input);
                let command = extract_str(tool_input, "command");
                let pattern = extract_str(tool_input, "pattern")
                    .or_else(|| extract_str(tool_input, "query"));
                let description = extract_str(tool_input, "description")
                    .or_else(|| extract_str(tool_input, "skill"));

                let detail = if tool_name.starts_with("mcp__") {
                    // MCP tools: format as "server: tool"
                    let parts: Vec<&str> = tool_name.splitn(3, "__").collect();
                    if parts.len() >= 3 {
                        format!("{}: {}", parts[1], parts[2])
                    } else {
                        format!("Using {}", tool_name)
                    }
                } else {
                    match crate::status_map::tool_to_state(tool_name) {
                        "editing" => format!("Editing {}", basename(&file.unwrap_or_default())),
                        "reading" => format!("Reading {}", basename(&file.unwrap_or_default())),
                        "searching" => format!("Searching: {}", truncate(&pattern.unwrap_or_default(), 30)),
                        "running" => format!("Running: {}", truncate(&command.unwrap_or_default(), 40)),
                        "delegating" => description.unwrap_or_else(|| "Delegating...".into()),
                        _ => format!("Using {}", tool_name),
                    }
                };

                ("tool".into(), tool_name.to_string(), detail)
            }
            "SubagentStart" => {
                ("subagent".into(), "agent".into(), "Spawning sub-agent...".into())
            }
            // A finished sub-agent says nothing about what the main agent does next: if it is
            // still working, the next PreToolUse/Stop event follows at once, and Claude Code
            // also runs helper agents *after* Stop (title/memory extraction), whose SubagentStop
            // would otherwise overwrite the final idle state with "thinking" until the next
            // prompt. So this event leaves the status untouched.
            "SubagentStop" => return None,
            "Notification" => {
                ("wait".into(), String::new(), "Waiting for approval...".into())
            }
            "Stop" => {
                ("done".into(), String::new(), "Waiting for input".into())
            }
            "StopFailure" => {
                ("error".into(), String::new(), "Something went wrong".into())
            }
            "SessionEnd" => {
                ("closed".into(), String::new(), "Session ended".into())
            }
            "SessionStart" => {
                // Write initial idle status
                ("done".into(), String::new(), "Session started".into())
            }
            _ => {
                ("prompt".into(), String::new(), format!("{}", hook))
            }
        };

        Some(NormalizedEvent {
            event,
            tool,
            detail,
            session_id: session_id.to_string(),
            session_name,
            launch_only: false,
        })
    }
}

fn extract_file(input: Option<&serde_json::Value>) -> Option<String> {
    let v = input?;
    v.get("file_path")
        .or_else(|| v.get("path"))
        .or_else(|| v.get("file"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
}


/// The name the user gave this session with /rename, read from Claude Code's session
/// registry (~/.claude/sessions/<pid>.json, one file per running process). Auto-derived
/// names are ignored so that the label stays the working directory unless the user chose
/// something. Re-read on every event, so a later rename shows up at the next hook.
fn user_session_name(session_id: &str) -> Option<String> {
    let home = std::env::var("HOME").ok()?;
    let entries = std::fs::read_dir(Path::new(&home).join(".claude").join("sessions")).ok()?;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(&path) else { continue };
        let Ok(v) = serde_json::from_str::<serde_json::Value>(&text) else { continue };
        if v["sessionId"].as_str() != Some(session_id) || v["nameSource"].as_str() != Some("user") {
            continue;
        }
        if let Some(name) = v["name"].as_str().filter(|n| !n.trim().is_empty()) {
            return Some(name.to_string());
        }
    }
    None
}
