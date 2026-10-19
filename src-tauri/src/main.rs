#![windows_subsystem = "windows"]

mod commands;
mod config;
mod engine;
mod firewall;
mod interceptor;
mod privilege;
mod process;
mod proxy;
mod relay;
mod sni;

use commands::*;
use engine::AppEngine;
use std::sync::Arc;
use tauri::Manager;

fn main() {
    let engine = Arc::new(AppEngine::new(None));
    let engine_exit_clone = engine.clone();
    let engine_setup_clone = engine.clone();

    tauri::Builder::default()
        .manage(engine)
        .setup(move |app| {
            // Connection logs are pushed to the frontend through the app handle
            engine_setup_clone.relay.set_app_handle(app.app_handle());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            check_admin_status,
            trigger_restart_as_admin,
            get_config,
            save_config,
            start_proxy,
            stop_proxy,
            get_proxy_status,
            list_running_processes,
            test_proxy_connection,
        ])
        .on_window_event(move |event| {
            if let tauri::WindowEvent::Destroyed = event.event() {
                // Ensure interceptor handle and sockets are cleanly dropped on close
                engine_exit_clone.stop();
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
