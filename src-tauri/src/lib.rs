mod commands;
mod state;

use state::AppState;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Load .env from the project root (one level up from src-tauri)
    let _ = dotenvy::dotenv();

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .manage(AppState::new())
        .invoke_handler(tauri::generate_handler![
            commands::files::set_working_directory,
            commands::files::set_api_key,
            commands::files::list_asc_files,
            commands::files::read_asc_file,
            commands::chat::send_chat_message,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
