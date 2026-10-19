use crate::config::AppConfig;
use crate::firewall;
use crate::interceptor::WinDivertInterceptor;
use crate::relay::RelayServer;
use parking_lot::RwLock;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

pub struct AppEngine {
    pub is_running: Arc<AtomicBool>,
    pub interceptor: Arc<WinDivertInterceptor>,
    pub relay: Arc<RelayServer>,
    pub current_config: Arc<RwLock<AppConfig>>,
}

impl AppEngine {
    pub fn new(app_handle: Option<tauri::AppHandle>) -> Self {
        let config = AppConfig::load();
        Self {
            is_running: Arc::new(AtomicBool::new(false)),
            interceptor: Arc::new(WinDivertInterceptor::new()),
            relay: Arc::new(RelayServer::new(app_handle)),
            current_config: Arc::new(RwLock::new(config)),
        }
    }

    pub async fn start(&self) -> Result<(), String> {
        if self.is_running.load(Ordering::SeqCst) {
            return Ok(());
        }

        // Redirected packets are injected as inbound traffic, so Windows Firewall must allow it.
        firewall::ensure_inbound_rule();

        // 1. Start local relay listener
        let binding = self
            .relay
            .start(self.interceptor.clone(), self.current_config.clone())
            .await?;

        // 2. Start WinDivert packet interceptor (reads the live config, so rule edits apply immediately)
        if let Err(e) = self
            .interceptor
            .start(self.current_config.clone(), binding.port, binding.ipv6)
        {
            self.relay.stop();
            return Err(e);
        }

        self.is_running.store(true, Ordering::SeqCst);
        Ok(())
    }

    /// Apply a new configuration to the running engine.
    pub fn update_config(&self, config: AppConfig) {
        *self.current_config.write() = config;
        self.interceptor.invalidate_cache();
    }

    pub fn stop(&self) {
        if !self.is_running.swap(false, Ordering::SeqCst) {
            return;
        }

        self.interceptor.stop();
        self.relay.stop();
    }
}
