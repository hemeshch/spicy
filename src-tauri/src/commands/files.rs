use crate::state::AppState;
use tauri::State;

#[tauri::command]
pub fn set_working_directory(state: State<AppState>, path: String) -> Result<(), String> {
    let mut dir = state.working_directory.lock().map_err(|e| e.to_string())?;
    *dir = Some(path);
    Ok(())
}

#[tauri::command]
pub fn set_api_key(state: State<AppState>, key: String) -> Result<(), String> {
    let mut api_key = state.api_key.lock().map_err(|e| e.to_string())?;
    *api_key = key;
    Ok(())
}

#[tauri::command]
pub fn has_api_key(state: State<AppState>) -> Result<bool, String> {
    let api_key = state.api_key.lock().map_err(|e| e.to_string())?;
    Ok(!api_key.is_empty())
}

fn collect_asc_files(dir: &std::path::Path, base: &std::path::Path, files: &mut Vec<String>) {
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                collect_asc_files(&path, base, files);
            } else if let Some(ext) = path.extension() {
                if ext == "asc" {
                    // Store path relative to the base working directory
                    if let Ok(relative) = path.strip_prefix(base) {
                        files.push(relative.to_string_lossy().to_string());
                    }
                }
            }
        }
    }
}

#[tauri::command]
pub fn list_asc_files(state: State<AppState>) -> Result<Vec<String>, String> {
    let dir = state.working_directory.lock().map_err(|e| e.to_string())?;
    let dir = dir.as_ref().ok_or("No working directory set")?;
    let base = std::path::Path::new(dir);

    let mut files = Vec::new();
    collect_asc_files(base, base, &mut files);
    files.sort();
    Ok(files)
}

#[tauri::command]
pub fn read_asc_file(state: State<AppState>, filename: String) -> Result<String, String> {
    let dir = state.working_directory.lock().map_err(|e| e.to_string())?;
    let dir = dir.as_ref().ok_or("No working directory set")?;

    let path = std::path::Path::new(dir).join(&filename);
    std::fs::read_to_string(&path).map_err(|e| format!("Failed to read {}: {}", filename, e))
}
