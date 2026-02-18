use crate::state::AppState;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::State;

#[derive(Serialize, Deserialize, Clone)]
pub struct ChatSessionMeta {
    pub id: String,
    pub title: String,
    pub created_at: String,
    pub updated_at: String,
    pub message_count: usize,
}

#[derive(Serialize, Deserialize)]
pub struct SessionIndex {
    pub sessions: Vec<ChatSessionMeta>,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct StoredMessage {
    pub id: String,
    pub role: String,
    pub content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thinking: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub changes: Option<Vec<serde_json::Value>>,
}

#[derive(Serialize, Deserialize)]
pub struct SessionData {
    pub id: String,
    pub title: String,
    pub messages: Vec<StoredMessage>,
}

fn timestamp_now() -> String {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .to_string()
}

fn sanitize_filename(name: &str) -> String {
    name.chars()
        .map(|c| match c {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' | ' ' => '_',
            _ => c,
        })
        .collect()
}

fn chats_dir(working_dir: &str, file: &str) -> PathBuf {
    PathBuf::from(working_dir)
        .join(".spicy")
        .join("chats")
        .join(sanitize_filename(file))
}

fn read_index(dir: &PathBuf) -> SessionIndex {
    let index_path = dir.join("sessions.json");
    match std::fs::read_to_string(&index_path) {
        Ok(content) => {
            serde_json::from_str(&content).unwrap_or(SessionIndex { sessions: vec![] })
        }
        Err(_) => SessionIndex { sessions: vec![] },
    }
}

fn write_index(dir: &PathBuf, index: &SessionIndex) -> Result<(), String> {
    std::fs::create_dir_all(dir).map_err(|e| format!("Failed to create directory: {}", e))?;
    let json = serde_json::to_string_pretty(index).map_err(|e| e.to_string())?;
    std::fs::write(dir.join("sessions.json"), json)
        .map_err(|e| format!("Failed to write index: {}", e))
}

#[tauri::command]
pub fn list_chat_sessions(state: State<AppState>, file: String) -> Result<SessionIndex, String> {
    let dir = state.working_directory.lock().map_err(|e| e.to_string())?;
    let dir = dir.as_ref().ok_or("No working directory set")?;
    let chat_dir = chats_dir(dir, &file);
    let mut index = read_index(&chat_dir);
    // Sort by most recently updated
    index
        .sessions
        .sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
    Ok(index)
}

#[tauri::command]
pub fn load_chat_session(
    state: State<AppState>,
    file: String,
    session_id: String,
) -> Result<SessionData, String> {
    let dir = state.working_directory.lock().map_err(|e| e.to_string())?;
    let dir = dir.as_ref().ok_or("No working directory set")?;
    let session_path = chats_dir(dir, &file).join(format!("{}.json", session_id));
    let content = std::fs::read_to_string(&session_path)
        .map_err(|e| format!("Failed to read session: {}", e))?;
    serde_json::from_str(&content).map_err(|e| format!("Failed to parse session: {}", e))
}

#[tauri::command]
pub fn save_chat_session(
    state: State<AppState>,
    file: String,
    session: SessionData,
) -> Result<(), String> {
    let dir = state.working_directory.lock().map_err(|e| e.to_string())?;
    let dir = dir.as_ref().ok_or("No working directory set")?;
    let chat_dir = chats_dir(dir, &file);

    std::fs::create_dir_all(&chat_dir)
        .map_err(|e| format!("Failed to create directory: {}", e))?;

    // Write session file
    let json = serde_json::to_string_pretty(&session).map_err(|e| e.to_string())?;
    std::fs::write(chat_dir.join(format!("{}.json", session.id)), json)
        .map_err(|e| format!("Failed to write session: {}", e))?;

    // Update index
    let mut index = read_index(&chat_dir);
    let now = timestamp_now();

    if let Some(existing) = index.sessions.iter_mut().find(|s| s.id == session.id) {
        existing.title = session.title.clone();
        existing.updated_at = now;
        existing.message_count = session.messages.len();
    } else {
        index.sessions.insert(
            0,
            ChatSessionMeta {
                id: session.id.clone(),
                title: session.title.clone(),
                created_at: now.clone(),
                updated_at: now,
                message_count: session.messages.len(),
            },
        );
    }

    write_index(&chat_dir, &index)
}

#[tauri::command]
pub fn delete_chat_session(
    state: State<AppState>,
    file: String,
    session_id: String,
) -> Result<(), String> {
    let dir = state.working_directory.lock().map_err(|e| e.to_string())?;
    let dir = dir.as_ref().ok_or("No working directory set")?;
    let chat_dir = chats_dir(dir, &file);

    // Remove session file
    let session_path = chat_dir.join(format!("{}.json", session_id));
    if session_path.exists() {
        std::fs::remove_file(&session_path)
            .map_err(|e| format!("Failed to delete session: {}", e))?;
    }

    // Update index
    let mut index = read_index(&chat_dir);
    index.sessions.retain(|s| s.id != session_id);
    write_index(&chat_dir, &index)
}
