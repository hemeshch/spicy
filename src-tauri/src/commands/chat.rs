use crate::state::AppState;
use serde::{Deserialize, Serialize};
use tauri::State;

#[derive(Serialize, Deserialize, Clone)]
pub struct FileChange {
    pub filename: String,
    pub description: String,
}

#[derive(Serialize, Deserialize)]
pub struct ChatResponse {
    pub explanation: String,
    pub changes: Vec<FileChange>,
}

#[derive(Serialize, Deserialize)]
struct ClaudeMessage {
    role: String,
    content: String,
}

#[derive(Serialize)]
struct ClaudeRequest {
    model: String,
    max_tokens: u32,
    system: String,
    messages: Vec<ClaudeMessage>,
}

#[derive(Deserialize)]
struct ClaudeApiResponse {
    content: Vec<ClaudeContentBlock>,
}

#[derive(Deserialize)]
struct ClaudeContentBlock {
    text: Option<String>,
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

#[tauri::command]
pub async fn send_chat_message(
    state: State<'_, AppState>,
    message: String,
    active_file: Option<String>,
    history: Vec<serde_json::Value>,
) -> Result<ChatResponse, String> {
    // Re-read API key from env if not already set
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
        return Err("ANTHROPIC_API_KEY not set. Please set it as an environment variable.".to_string());
    }

    let dir = state.working_directory.lock().map_err(|e| e.to_string())?.clone();
    let dir = dir.ok_or("No working directory set")?;

    // Build user message with file context
    let mut user_content = String::new();

    if let Some(ref filename) = active_file {
        let file_path = std::path::Path::new(&dir).join(filename);
        match std::fs::read_to_string(&file_path) {
            Ok(content) => {
                user_content.push_str(&format!(
                    "Current file: {}\n\n```\n{}\n```\n\n",
                    filename, content
                ));
            }
            Err(e) => {
                // Try reading as bytes and converting from UTF-16
                match std::fs::read(&file_path) {
                    Ok(bytes) => {
                        // Try UTF-16 LE
                        let content = if bytes.len() >= 2 && bytes[0] == 0xFF && bytes[1] == 0xFE {
                            let u16s: Vec<u16> = bytes[2..]
                                .chunks(2)
                                .filter_map(|c| {
                                    if c.len() == 2 { Some(u16::from_le_bytes([c[0], c[1]])) } else { None }
                                })
                                .collect();
                            String::from_utf16_lossy(&u16s)
                        } else {
                            String::from_utf8_lossy(&bytes).to_string()
                        };
                        user_content.push_str(&format!(
                            "Current file: {}\n\n```\n{}\n```\n\n",
                            filename, content
                        ));
                    }
                    Err(_) => {
                        return Err(format!("Failed to read file {}: {}", filename, e));
                    }
                }
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

    let request = ClaudeRequest {
        model: "claude-sonnet-4-20250514".to_string(),
        max_tokens: 8192,
        system: SYSTEM_PROMPT.to_string(),
        messages,
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
        return Err(format!("API error ({}): {}", status, body));
    }

    let api_response: ClaudeApiResponse = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse API response: {}", e))?;

    let text = api_response
        .content
        .first()
        .and_then(|b| b.text.as_ref())
        .ok_or("No text in API response")?;

    // Try to parse as JSON with modified_asc (edit mode)
    if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(text) {
        if let Some(modified_asc) = parsed["modified_asc"].as_str() {
            // Write the modified file to disk
            if let Some(ref filename) = active_file {
                let file_path = std::path::Path::new(&dir).join(filename);
                std::fs::write(&file_path, modified_asc)
                    .map_err(|e| format!("Failed to write file: {}", e))?;
            }

            let explanation = parsed["explanation"]
                .as_str()
                .unwrap_or("Changes applied.")
                .to_string();

            let changes: Vec<FileChange> = if let Some(changes_arr) = parsed["changes"].as_array()
            {
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

            return Ok(ChatResponse {
                explanation,
                changes,
            });
        }
    }

    // Analysis mode: plain text response
    Ok(ChatResponse {
        explanation: text.to_string(),
        changes: vec![],
    })
}
