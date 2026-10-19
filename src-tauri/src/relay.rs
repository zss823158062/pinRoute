use crate::config::AppConfig;
use crate::interceptor::divert::OriginalTarget;
use crate::interceptor::WinDivertInterceptor;
use crate::proxy::{ProxyClient, Socks5Target};
use crate::sni::parse_tls_sni;
use chrono::Local;
use parking_lot::{Mutex, RwLock};
use serde::{Deserialize, Serialize};
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};
use std::sync::Arc;
use std::time::Duration;
use tauri::Manager;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::broadcast;
use tokio_util::sync::CancellationToken;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionLog {
    pub id: String,
    pub timestamp: String,
    pub process_name: String,
    pub pid: u32,
    pub target: String,
    pub protocol: String,
    pub status: String, // "success", "failed", "direct"
    pub error_msg: Option<String>,
}

/// Ports the relay ended up listening on.
pub struct RelayBinding {
    pub port: u16,
    pub ipv6: bool,
}

pub struct RelayServer {
    app_handle: Arc<Mutex<Option<tauri::AppHandle>>>,
    log_tx: broadcast::Sender<ConnectionLog>,
    cancel: Mutex<Option<CancellationToken>>,
}

impl RelayServer {
    pub fn new(app_handle: Option<tauri::AppHandle>) -> Self {
        let (log_tx, _) = broadcast::channel(200);
        Self {
            app_handle: Arc::new(Mutex::new(app_handle)),
            log_tx,
            cancel: Mutex::new(None),
        }
    }

    pub fn set_app_handle(&self, app_handle: tauri::AppHandle) {
        *self.app_handle.lock() = Some(app_handle);
    }

    #[allow(dead_code)]
    pub fn subscribe_logs(&self) -> broadcast::Receiver<ConnectionLog> {
        self.log_tx.subscribe()
    }

    pub async fn start(
        &self,
        interceptor: Arc<WinDivertInterceptor>,
        config: Arc<RwLock<AppConfig>>,
    ) -> Result<RelayBinding, String> {
        // Must bind the wildcard address: redirected SYNs have their IPs swapped by the
        // interceptor, so they arrive addressed to the machine's real interface IP.
        let listener_v4 = TcpListener::bind((Ipv4Addr::UNSPECIFIED, 0))
            .await
            .map_err(|e| format!("Failed to bind local relay listener: {}", e))?;
        let port = listener_v4
            .local_addr()
            .map_err(|e| format!("Failed to get relay port: {}", e))?
            .port();
        // IPv6 is best effort on the same port; without it IPv6 traffic is left untouched.
        let listener_v6 = TcpListener::bind((Ipv6Addr::UNSPECIFIED, port)).await.ok();
        let ipv6 = listener_v6.is_some();

        let cancel = CancellationToken::new();
        if let Some(old) = self.cancel.lock().replace(cancel.clone()) {
            old.cancel();
        }

        let ctx = Arc::new(RelayContext {
            interceptor,
            config,
            app_handle: self.app_handle.clone(),
            log_tx: self.log_tx.clone(),
        });

        tokio::spawn(Self::accept_loop(listener_v4, ctx.clone(), cancel.clone()));
        if let Some(listener) = listener_v6 {
            tokio::spawn(Self::accept_loop(listener, ctx, cancel));
        }

        Ok(RelayBinding { port, ipv6 })
    }

    pub fn stop(&self) {
        if let Some(cancel) = self.cancel.lock().take() {
            cancel.cancel();
        }
    }

    async fn accept_loop(listener: TcpListener, ctx: Arc<RelayContext>, cancel: CancellationToken) {
        loop {
            let accepted = tokio::select! {
                _ = cancel.cancelled() => break,
                res = listener.accept() => res,
            };
            match accepted {
                Ok((stream, peer)) => {
                    let ctx = ctx.clone();
                    let cancel = cancel.clone();
                    tokio::spawn(async move {
                        tokio::select! {
                            _ = cancel.cancelled() => {}
                            _ = ctx.handle_connection(stream, peer) => {}
                        }
                    });
                }
                Err(e) => {
                    eprintln!("Relay listener accept error: {}", e);
                    tokio::time::sleep(Duration::from_millis(50)).await;
                }
            }
        }
    }
}

struct RelayContext {
    interceptor: Arc<WinDivertInterceptor>,
    config: Arc<RwLock<AppConfig>>,
    app_handle: Arc<Mutex<Option<tauri::AppHandle>>>,
    log_tx: broadcast::Sender<ConnectionLog>,
}

impl RelayContext {
    async fn handle_connection(&self, mut client_stream: TcpStream, peer_addr: SocketAddr) {
        let _ = client_stream.set_nodelay(true);
        let peer_ip = normalize_ip(peer_addr.ip());
        let client_port = peer_addr.port();

        let target_info = match self.interceptor.get_original_target(peer_ip.is_ipv6(), client_port) {
            // After the IP swap the relay sees the original destination as the peer address;
            // anything else is an unrelated connection to the (wildcard-bound) relay port.
            Some(t) if peer_ip == t.dest_ip || peer_ip.is_loopback() => t,
            _ => return,
        };

        // Read initial chunk to inspect TLS SNI. Server-speaks-first protocols send nothing,
        // so don't wait forever for the client.
        let mut first_buf = vec![0u8; 4096];
        let n = match tokio::time::timeout(Duration::from_millis(500), client_stream.read(&mut first_buf)).await {
            Ok(Ok(0)) | Ok(Err(_)) => return,
            Ok(Ok(n)) => n,
            Err(_) => 0,
        };
        first_buf.truncate(n);

        // Attempt SNI sniffing (F-24)
        let sni_domain = parse_tls_sni(&first_buf);
        let socks5_target = match (&sni_domain, target_info.dest_ip) {
            (Some(domain), _) => Socks5Target::Domain(domain.clone(), target_info.dest_port),
            (None, IpAddr::V4(ip)) => Socks5Target::IPv4(ip, target_info.dest_port),
            (None, IpAddr::V6(ip)) => Socks5Target::IPv6(ip, target_info.dest_port),
        };

        let log_entry = new_log(&target_info, client_port, &socks5_target, sni_domain.is_some());
        self.emit_log(log_entry.clone());

        let proxy = self.config.read().proxy.clone();
        match ProxyClient::connect(&proxy, &socks5_target).await {
            Ok(mut proxy_stream) => {
                let _ = proxy_stream.set_nodelay(true);
                if let Err(e) = proxy_stream.write_all(&first_buf).await {
                    self.emit_log(with_status(log_entry, "failed", Some(format!("Failed sending first chunk: {}", e))));
                    return;
                }

                self.emit_log(with_status(log_entry, "success", None));
                let _ = tokio::io::copy_bidirectional(&mut client_stream, &mut proxy_stream).await;
            }
            Err(e) => self.emit_log(with_status(log_entry, "failed", Some(e))),
        }
    }

    fn emit_log(&self, log: ConnectionLog) {
        let _ = self.log_tx.send(log.clone());
        if let Some(app) = self.app_handle.lock().as_ref() {
            let _ = app.emit_all("new-connection-log", &log);
        }
    }
}

fn new_log(target: &OriginalTarget, client_port: u16, dest: &Socks5Target, is_tls: bool) -> ConnectionLog {
    ConnectionLog {
        id: format!("{}-{}", client_port, chrono::Utc::now().timestamp_millis()),
        timestamp: Local::now().format("%H:%M:%S").to_string(),
        process_name: target.process_name.clone(),
        pid: target.pid,
        target: dest.to_string(),
        protocol: if is_tls { "TLS/HTTPS" } else { "TCP" }.to_string(),
        status: "connecting".to_string(),
        error_msg: None,
    }
}

fn with_status(mut log: ConnectionLog, status: &str, error_msg: Option<String>) -> ConnectionLog {
    log.status = status.to_string();
    log.error_msg = error_msg;
    log
}

/// Map IPv4-mapped IPv6 peers (::ffff:a.b.c.d) back to plain IPv4.
fn normalize_ip(ip: IpAddr) -> IpAddr {
    match ip {
        IpAddr::V6(v6) => v6.to_ipv4_mapped().map(IpAddr::V4).unwrap_or(ip),
        v4 => v4,
    }
}
