use std::sync::Mutex;

pub struct AppState {
    pub working_directory: Mutex<Option<String>>,
    pub api_key: Mutex<String>,
}

impl AppState {
    pub fn new() -> Self {
        let api_key = std::env::var("OPENROUTER_API_KEY").unwrap_or_default();
        Self {
            working_directory: Mutex::new(None),
            api_key: Mutex::new(api_key),
        }
    }
}
