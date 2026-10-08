//! VOID studio application entry (Tauri shell).
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "void_tauri_lib=info,void_app=info".into()),
        )
        .init();
    void_tauri_lib::run();
}
