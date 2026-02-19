use crate::state::AppState;
use futures::StreamExt;
use serde::{Deserialize, Serialize};
use tauri::ipc::Channel;
use tauri::State;

#[derive(Serialize, Deserialize, Clone)]
pub struct FileChange {
    pub component: Option<String>,
    pub filename: String,
    pub description: String,
}

#[derive(Clone, Serialize)]
#[serde(tag = "type")]
pub enum StreamEvent {
    #[serde(rename = "thinking")]
    Thinking { content: String },
    #[serde(rename = "text")]
    Text { content: String },
    #[serde(rename = "done")]
    Done {
        changes: Vec<FileChange>,
        explanation: Option<String>,
    },
    #[serde(rename = "error")]
    Error { message: String },
}

#[derive(Serialize, Deserialize)]
struct ClaudeMessage {
    role: String,
    content: String,
}

#[derive(Serialize)]
struct ThinkingConfig {
    #[serde(rename = "type")]
    thinking_type: String,
}

#[derive(Serialize)]
struct ClaudeStreamRequest {
    model: String,
    max_tokens: u32,
    system: String,
    messages: Vec<ClaudeMessage>,
    stream: bool,
    thinking: ThinkingConfig,
}

const SYSTEM_PROMPT: &str = r#"You are Spicy, an AI assistant for LTspice circuit schematics (.asc files).

The user's currently active .asc file content will be provided at the start of their message with line numbers (e.g. "1| Version 4"). Always use this content as context — never ask the user to paste it.

You have two modes:

1. **Analysis mode** — When the user asks to explain, analyze, or understand a circuit, respond with a clear explanation in plain text. Do NOT use JSON format for analysis.

2. **Edit mode** — When the user asks to modify, change, add, or remove something in the circuit, respond with ONLY this JSON (no markdown, no code blocks):
{
  "edits": [
    { "start": 15, "end": 15, "replacement": "SYMATTR Value 24k" }
  ],
  "explanation": "Changed R1 from 10kΩ to 24kΩ",
  "changes": [
    { "component": "R1", "filename": "<filename>", "description": "Value 10kΩ → 24kΩ" }
  ]
}

Edit instructions:
- "start" and "end" are 1-based inclusive line numbers matching the numbered file content
- "replacement" is the new text for that line range (can be multi-line with \n)
- To replace a line: { "start": 15, "end": 15, "replacement": "new content" }
- To delete lines: { "start": 15, "end": 17, "replacement": "" }
- To insert after line 15: { "start": 15, "end": 15, "replacement": "<original line 15>\n<new lines>" }
- Multiple edits in one response are fine; they will be applied bottom-up so line numbers stay correct
- Edits MUST NOT have overlapping line ranges — each line should be touched by at most one edit

LTspice .asc files are text-based schematics containing:
- Version header, SHEET directive (sheet size)
- WIRE directives (connections between components)
- SYMBOL directives (component placements)
- SYMATTR directives (component attributes like Value, InstName)
- FLAG directives (net labels/flags)
- TEXT directives (comments and SPICE directives)

When modifying circuits:
- Preserve the overall structure and formatting
- Update component values, add/remove components as requested
- Ensure wire connections remain valid
- Keep all coordinates as integers"#;

fn apply_edits(file_path: &std::path::Path, edits: &[serde_json::Value]) -> Result<String, String> {
    let content = std::fs::read_to_string(file_path)
        .map_err(|e| format!("Failed to read file: {}", e))?;
    let mut lines: Vec<String> = content.lines().map(|l| l.to_string()).collect();

    // Collect edits as (start, end, replacement) and sort descending by start line
    let mut edit_ops: Vec<(usize, usize, String)> = edits
        .iter()
        .filter_map(|e| {
            let start = e["start"].as_u64()? as usize;
            let end = e["end"].as_u64()? as usize;
            let replacement = e["replacement"].as_str()?.to_string();
            Some((start, end, replacement))
        })
        .collect();
    edit_ops.sort_by(|a, b| b.0.cmp(&a.0));

    for (start, end, replacement) in edit_ops {
        if start == 0 || end == 0 || start > lines.len() || end > lines.len() || start > end {
            continue;
        }
        let start_idx = start - 1;
        let end_idx = end; // exclusive for drain/splice
        let new_lines: Vec<String> = if replacement.is_empty() {
            vec![]
        } else {
            replacement.lines().map(|l| l.to_string()).collect()
        };
        lines.splice(start_idx..end_idx, new_lines);
    }

    let mut result = lines.join("\n");
    if content.ends_with('\n') && !result.ends_with('\n') {
        result.push('\n');
    }
    std::fs::write(file_path, &result)
        .map_err(|e| format!("Failed to write file: {}", e))?;
    Ok(result)
}

/// Shared helper: parse JSON edit response, apply edits, send Done event.
/// Returns true if the response was handled as a JSON edit.
fn handle_edit_response(
    json_val: &serde_json::Value,
    active_file: &Option<String>,
    dir: &str,
    on_event: &Channel<StreamEvent>,
) -> bool {
    let edits = match json_val["edits"].as_array() {
        Some(e) => e,
        None => return false,
    };

    if let Some(ref filename) = active_file {
        let file_path = std::path::Path::new(dir).join(filename);
        if let Err(e) = apply_edits(&file_path, edits) {
            let _ = on_event.send(StreamEvent::Error { message: e });
            return true;
        }
    }

    let explanation = json_val["explanation"]
        .as_str()
        .unwrap_or("Changes applied.")
        .to_string();

    let changes: Vec<FileChange> = if let Some(changes_arr) = json_val["changes"].as_array() {
        changes_arr
            .iter()
            .filter_map(|c| {
                Some(FileChange {
                    component: c["component"].as_str().map(|s| s.to_string()),
                    filename: c["filename"].as_str()?.to_string(),
                    description: c["description"].as_str()?.to_string(),
                })
            })
            .collect()
    } else {
        vec![]
    };

    let _ = on_event.send(StreamEvent::Done {
        changes,
        explanation: Some(explanation),
    });
    true
}

fn read_asc_file_content(dir: &str, filename: &str) -> Result<String, String> {
    let file_path = std::path::Path::new(dir).join(filename);
    match std::fs::read_to_string(&file_path) {
        Ok(content) => Ok(content),
        Err(_utf8_err) => {
            match std::fs::read(&file_path) {
                Ok(bytes) => {
                    let content = if bytes.len() >= 2 && bytes[0] == 0xFF && bytes[1] == 0xFE {
                        let u16s: Vec<u16> = bytes[2..]
                            .chunks(2)
                            .filter_map(|c| {
                                if c.len() == 2 {
                                    Some(u16::from_le_bytes([c[0], c[1]]))
                                } else {
                                    None
                                }
                            })
                            .collect();
                        String::from_utf16_lossy(&u16s)
                    } else {
                        String::from_utf8_lossy(&bytes).to_string()
                    };
                    Ok(content)
                }
                Err(io_err) => Err(format!("Failed to read file {}: {}", filename, io_err)),
            }
        }
    }
}

#[tauri::command]
pub async fn send_chat_message_stream(
    state: State<'_, AppState>,
    message: String,
    active_file: Option<String>,
    history: Vec<serde_json::Value>,
    on_event: Channel<StreamEvent>,
) -> Result<(), String> {
    let api_key = {
        let mut key = state.api_key.lock().map_err(|e| e.to_string())?;
        if key.is_empty() {
            if let Ok(env_key) = std::env::var("ANTHROPIC_API_KEY") {
                *key = env_key;
            }
        }
        key.clone()
    };

    if api_key.is_empty() {
        let _ = on_event.send(StreamEvent::Error {
            message: "ANTHROPIC_API_KEY not set. Please set it as an environment variable."
                .to_string(),
        });
        return Ok(());
    }

    let dir = state
        .working_directory
        .lock()
        .map_err(|e| e.to_string())?
        .clone();
    let dir = match dir {
        Some(d) => d,
        None => {
            let _ = on_event.send(StreamEvent::Error {
                message: "No working directory set".to_string(),
            });
            return Ok(());
        }
    };

    // Build user message with file context
    let mut user_content = String::new();

    if let Some(ref filename) = active_file {
        match read_asc_file_content(&dir, filename) {
            Ok(content) => {
                let numbered: String = content
                    .lines()
                    .enumerate()
                    .map(|(i, line)| format!("{}| {}", i + 1, line))
                    .collect::<Vec<_>>()
                    .join("\n");
                user_content.push_str(&format!(
                    "Current file: {}\n\n{}\n\n",
                    filename, numbered
                ));
            }
            Err(e) => {
                let _ = on_event.send(StreamEvent::Error { message: e });
                return Ok(());
            }
        }
    }
    user_content.push_str(&message);

    // Build message history
    let mut messages: Vec<ClaudeMessage> = Vec::new();

    for msg in &history {
        if let (Some(role), Some(content)) = (msg["role"].as_str(), msg["content"].as_str()) {
            messages.push(ClaudeMessage {
                role: role.to_string(),
                content: content.to_string(),
            });
        }
    }

    messages.push(ClaudeMessage {
        role: "user".to_string(),
        content: user_content,
    });

    let request = ClaudeStreamRequest {
        model: "claude-sonnet-4-6".to_string(),
        max_tokens: 16000,
        system: SYSTEM_PROMPT.to_string(),
        messages,
        stream: true,
        thinking: ThinkingConfig {
            thinking_type: "adaptive".to_string(),
        },
    };

    let client = reqwest::Client::new();
    let response = client
        .post("https://api.anthropic.com/v1/messages")
        .header("x-api-key", &api_key)
        .header("anthropic-version", "2023-06-01")
        .header("content-type", "application/json")
        .json(&request)
        .send()
        .await
        .map_err(|e| format!("API request failed: {}", e))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        let _ = on_event.send(StreamEvent::Error {
            message: format!("API error ({}): {}", status, body),
        });
        return Ok(());
    }

    // Read SSE stream
    let mut stream = response.bytes_stream();
    let mut buffer = String::new();
    let mut accumulated_text = String::new();
    let mut done_sent = false;
    let mut suppress_text = false; // true when response looks like JSON edit

    // Process a single SSE data line; returns true if we should stop
    let process_line = |line: &str,
                        accumulated_text: &mut String,
                        on_event: &Channel<StreamEvent>,
                        active_file: &Option<String>,
                        dir: &str,
                        done_sent: &mut bool,
                        suppress_text: &mut bool|
     -> bool {
        let data = match line.strip_prefix("data: ") {
            Some(d) => d,
            None => return false,
        };

        if data == "[DONE]" {
            return false;
        }

        let parsed = match serde_json::from_str::<serde_json::Value>(data) {
            Ok(p) => p,
            Err(_) => return false,
        };

        let event_type = parsed["type"].as_str().unwrap_or("");

        match event_type {
            "content_block_delta" => {
                let delta_type = parsed["delta"]["type"].as_str().unwrap_or("");
                match delta_type {
                    "thinking_delta" => {
                        if let Some(thinking) = parsed["delta"]["thinking"].as_str() {
                            let _ = on_event.send(StreamEvent::Thinking {
                                content: thinking.to_string(),
                            });
                        }
                    }
                    "text_delta" => {
                        if let Some(text) = parsed["delta"]["text"].as_str() {
                            // Detect JSON edit on first text chunk
                            if accumulated_text.is_empty() && text.trim_start().starts_with('{') {
                                *suppress_text = true;
                            }
                            accumulated_text.push_str(text);
                            if !*suppress_text {
                                let _ = on_event.send(StreamEvent::Text {
                                    content: text.to_string(),
                                });
                            }
                        }
                    }
                    _ => {}
                }
            }
            "message_stop" => {
                // Check if accumulated text is JSON edit response
                if let Ok(json_val) =
                    serde_json::from_str::<serde_json::Value>(accumulated_text)
                {
                    if handle_edit_response(&json_val, active_file, dir, on_event) {
                        *done_sent = true;
                        return true;
                    }
                }

                // Analysis mode: plain text
                let _ = on_event.send(StreamEvent::Done {
                    changes: vec![],
                    explanation: None,
                });
                *done_sent = true;
            }
            "error" => {
                let error_msg = parsed["error"]["message"]
                    .as_str()
                    .unwrap_or("Unknown API error");
                let _ = on_event.send(StreamEvent::Error {
                    message: error_msg.to_string(),
                });
                *done_sent = true;
                return true;
            }
            _ => {}
        }
        false
    };

    'outer: while let Some(chunk) = stream.next().await {
        let chunk = match chunk {
            Ok(c) => c,
            Err(e) => {
                let _ = on_event.send(StreamEvent::Error {
                    message: format!("Stream error: {}", e),
                });
                return Ok(());
            }
        };

        buffer.push_str(&String::from_utf8_lossy(&chunk));

        while let Some(newline_pos) = buffer.find('\n') {
            let line = buffer[..newline_pos].trim_end().to_string();
            buffer = buffer[newline_pos + 1..].to_string();

            if process_line(
                &line,
                &mut accumulated_text,
                &on_event,
                &active_file,
                &dir,
                &mut done_sent,
                &mut suppress_text,
            ) {
                break 'outer;
            }
        }
    }

    // Flush any remaining data in the buffer (e.g. message_stop without trailing newline)
    if !done_sent && !buffer.trim().is_empty() {
        for line in buffer.lines() {
            let line = line.trim();
            if !line.is_empty() {
                process_line(
                    line,
                    &mut accumulated_text,
                    &on_event,
                    &active_file,
                    &dir,
                    &mut done_sent,
                    &mut suppress_text,
                );
            }
        }
    }

    if !done_sent {
        // Stream ended — do final edit check on accumulated text
        if let Ok(json_val) = serde_json::from_str::<serde_json::Value>(&accumulated_text) {
            if handle_edit_response(&json_val, &active_file, &dir, &on_event) {
                return Ok(());
            }
        }

        let _ = on_event.send(StreamEvent::Done {
            changes: vec![],
            explanation: None,
        });
    }

    Ok(())
}
