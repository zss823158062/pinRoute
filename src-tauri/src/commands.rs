use crate::config::{AppConfig, ProxyConfig};
use crate::engine::AppEngine;
use crate::privilege::{is_admin, restart_as_admin};
use crate::process::{ProcessInfo, ProcessManager};
use crate::proxy::ProxyClient;
use serde::{Deserialize, Serialize};
use std::sync::atomic::Ordering;
use std::sync::Arc;
use tauri::State;

#[derive(Debug, Serialize, Deserialize)]
pub struct ProxyStatus {
    pub is_running: bool,
    pub is_admin: bool,
}

#[tauri::command]
pub fn check_admin_status() -> bool {
    is_admin()
}

#[tauri::command]
pub fn trigger_restart_as_admin() -> Result<(), String> {
    restart_as_admin()
}

#[tauri::command]
pub fn get_config(engine: State<'_, Arc<AppEngine>>) -> AppConfig {
    engine.current_config.read().clone()
}

#[tauri::command]
pub fn save_config(
    engine: State<'_, Arc<AppEngine>>,
    config: AppConfig,
) -> Result<(), String> {
    config.save()?;
    engine.update_config(config);
    Ok(())
}

#[tauri::command]
pub async fn start_proxy(engine: State<'_, Arc<AppEngine>>) -> Result<(), String> {
    if !is_admin() {
        return Err("Administrator privileges required to start proxy interceptor.".to_string());
    }
    engine.start().await
}

#[tauri::command]
pub fn stop_proxy(engine: State<'_, Arc<AppEngine>>) -> Result<(), String> {
    engine.stop();
    Ok(())
}

#[tauri::command]
pub fn get_proxy_status(engine: State<'_, Arc<AppEngine>>) -> ProxyStatus {
    ProxyStatus {
        is_running: engine.is_running.load(Ordering::SeqCst),
        is_admin: is_admin(),
    }
}

#[tauri::command]
pub fn list_running_processes() -> Vec<ProcessInfo> {
    ProcessManager::list_running_processes()
}

#[tauri::command]
pub async fn test_proxy_connection(proxy: ProxyConfig) -> Result<String, String> {
    ProxyClient::test_proxy(&proxy).await
}
