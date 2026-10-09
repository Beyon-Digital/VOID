//! VOID studio shell library (Tauri). All authoritative state lives in
//! Rust + the engine worker; the WebView holds presentation only.

mod codec;
mod commands;

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .manage(std::sync::Arc::new(commands::AppState::new()))
        .invoke_handler(tauri::generate_handler![
            commands::engine_status,
            commands::spawn_engine,
            commands::stop_engine,
            commands::send_command,
            commands::send_transport,
            commands::read_view,
        ])
        .run(tauri::generate_context!())
        .expect("error running VOID application");
}
