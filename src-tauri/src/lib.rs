mod commands;
mod state;

use state::AppState;
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let _ = dotenvy::dotenv();

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .manage(AppState::new())
        .setup(|app| {
            let window = app.get_webview_window("main").unwrap();

            #[cfg(target_os = "macos")]
            {
                use objc2_app_kit::{NSColor, NSWindow};
                use raw_window_handle::{HasWindowHandle, RawWindowHandle};

                if let Ok(handle) = window.window_handle() {
                    if let RawWindowHandle::AppKit(h) = handle.as_raw() {
                        unsafe {
                            let ns_view =
                                h.ns_view.as_ptr() as *const objc2::runtime::AnyObject;
                            let ns_window: *const NSWindow =
                                objc2::msg_send![ns_view, window];
                            let ns_window = &*ns_window;
                            ns_window.setOpaque(false);
                            ns_window.setBackgroundColor(Some(&NSColor::clearColor()));
                            ns_window.setHasShadow(true);
                        }
                    }
                }

                // Clear WKWebView background
                window
                    .with_webview(|webview| {
                        use objc2_foundation::{NSNumber, NSString};
                        unsafe {
                            let wv = webview.inner() as *mut objc2::runtime::AnyObject;
                            let key = NSString::from_str("drawsBackground");
                            let no = NSNumber::new_bool(false);
                            let _: () =
                                objc2::msg_send![wv, setValue: &*no, forKey: &*key];
                        }
                    })
                    .ok();
            }

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::files::set_working_directory,
            commands::files::set_api_key,
            commands::files::has_api_key,
            commands::files::list_asc_files,
            commands::files::read_asc_file,
            commands::chat::send_chat_message_stream,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
