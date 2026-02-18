use crate::state::AppState;
use futures::StreamExt;
use serde::{Deserialize, Serialize};
use tauri::ipc::Channel;
use tauri::State;

#[derive(Serialize, Deserialize, Clone)]
pub struct FileChange {
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

The user's currently active .asc file content will be provided at the start of their message, between ``` markers. Always use this content as context — never ask the user to paste it.

You have two modes:

1. **Analysis mode** — When the user asks to explain, analyze, or understand a circuit, respond with a clear explanation in plain text. Do NOT use JSON format for analysis.

2. **Edit mode** — When the user asks to modify, change, add, or remove something in the circuit, respond with ONLY this JSON (no markdown, no code blocks):
{
  "modified_asc": "<the complete modified .asc file content>",
  "explanation": "<a clear explanation of what you changed and why>",
  "changes": [
    { "filename": "<filename>", "description": "<brief description of change>" }
  ]
}

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
                user_content.push_str(&format!(
                    "Current file: {}\n\n```\n{}\n```\n\n",
                    filename, content
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

    // Process a single SSE data line; returns true if we should stop
    let process_line = |line: &str,
                        accumulated_text: &mut String,
                        on_event: &Channel<StreamEvent>,
                        active_file: &Option<String>,
                        dir: &str,
                        done_sent: &mut bool|
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
                            accumulated_text.push_str(text);
                            let _ = on_event.send(StreamEvent::Text {
                                content: text.to_string(),
                            });
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
                    if let Some(modified_asc) = json_val["modified_asc"].as_str() {
                        // Write the modified file to disk
                        if let Some(ref filename) = active_file {
                            let file_path = std::path::Path::new(dir).join(filename);
                            if let Err(e) = std::fs::write(&file_path, modified_asc) {
                                let _ = on_event.send(StreamEvent::Error {
                                    message: format!("Failed to write file: {}", e),
                                });
                                *done_sent = true;
                                return true;
                            }
                        }

                        let explanation = json_val["explanation"]
                            .as_str()
                            .unwrap_or("Changes applied.")
                            .to_string();

                        let changes: Vec<FileChange> =
                            if let Some(changes_arr) = json_val["changes"].as_array() {
                                changes_arr
                                    .iter()
                                    .filter_map(|c| {
                                        Some(FileChange {
                                            filename: c["filename"].as_str()?.to_string(),
                                            description: c["description"]
                                                .as_str()?
                                                .to_string(),
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
                );
            }
        }
    }

    if !done_sent {
        // Stream ended — do final edit check on accumulated text
        if let Ok(json_val) = serde_json::from_str::<serde_json::Value>(&accumulated_text) {
            if let Some(modified_asc) = json_val["modified_asc"].as_str() {
                if let Some(ref filename) = active_file {
                    let file_path = std::path::Path::new(&dir).join(filename);
                    if let Err(e) = std::fs::write(&file_path, modified_asc) {
                        let _ = on_event.send(StreamEvent::Error {
                            message: format!("Failed to write file: {}", e),
                        });
                        return Ok(());
                    }
                }

                let explanation = json_val["explanation"]
                    .as_str()
                    .unwrap_or("Changes applied.")
                    .to_string();

                let changes: Vec<FileChange> =
                    if let Some(changes_arr) = json_val["changes"].as_array() {
                        changes_arr
                            .iter()
                            .filter_map(|c| {
                                Some(FileChange {
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
