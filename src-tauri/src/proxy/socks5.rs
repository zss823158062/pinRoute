use crate::config::ProxyConfig;
use std::net::{Ipv4Addr, Ipv6Addr};
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::time::timeout;

#[derive(Debug, Clone)]
pub enum Socks5Target {
    Domain(String, u16),
    IPv4(Ipv4Addr, u16),
    IPv6(Ipv6Addr, u16),
}

impl std::fmt::Display for Socks5Target {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Socks5Target::Domain(d, p) => write!(f, "{}:{}", d, p),
            Socks5Target::IPv4(ip, p) => write!(f, "{}:{}", ip, p),
            Socks5Target::IPv6(ip, p) => write!(f, "[{}]:{}", ip, p),
        }
    }
}

pub struct Socks5Client;

impl Socks5Client {
    /// Connects to the upstream SOCKS5 proxy and negotiates a connection to the target.
    pub async fn connect(
        proxy: &ProxyConfig,
        target: &Socks5Target,
    ) -> Result<TcpStream, String> {
        let proxy_addr = format!("{}:{}", proxy.host, proxy.port);
        let mut stream = TcpStream::connect(&proxy_addr)
            .await
            .map_err(|e| format!("Failed to connect to SOCKS5 upstream ({}): {}", proxy_addr, e))?;

        // 1. Send Version & Authentication Methods
        let has_auth = proxy.username.is_some() && proxy.password.is_some();
        if has_auth {
            // SOCKS5 (0x05), 2 methods: 0x00 (NO AUTH), 0x02 (USER/PASS)
            stream.write_all(&[0x05, 0x02, 0x00, 0x02]).await
                .map_err(|e| format!("SOCKS5 handshake write failed: {}", e))?;
        } else {
            // SOCKS5 (0x05), 1 method: 0x00 (NO AUTH)
            stream.write_all(&[0x05, 0x01, 0x00]).await
                .map_err(|e| format!("SOCKS5 handshake write failed: {}", e))?;
        }

        let mut auth_resp = [0u8; 2];
        stream.read_exact(&mut auth_resp).await
            .map_err(|e| format!("SOCKS5 handshake read failed: {}", e))?;

        if auth_resp[0] != 0x05 {
            return Err("Invalid SOCKS5 version in server response".to_string());
        }

        // Handle authentication method
        match auth_resp[1] {
            0x00 => {
                // No authentication required
            }
            0x02 => {
                // Username/Password authentication (RFC 1929)
                let user = proxy.username.as_deref().unwrap_or("");
                let pass = proxy.password.as_deref().unwrap_or("");

                let mut auth_buf = Vec::with_capacity(3 + user.len() + pass.len());
                auth_buf.push(0x01); // Auth subnegotiation version
                auth_buf.push(user.len() as u8);
                auth_buf.extend_from_slice(user.as_bytes());
                auth_buf.push(pass.len() as u8);
                auth_buf.extend_from_slice(pass.as_bytes());

                stream.write_all(&auth_buf).await
                    .map_err(|e| format!("SOCKS5 auth write failed: {}", e))?;

                let mut sub_resp = [0u8; 2];
                stream.read_exact(&mut sub_resp).await
                    .map_err(|e| format!("SOCKS5 auth read failed: {}", e))?;

                if sub_resp[1] != 0x00 {
                    return Err("SOCKS5 username/password authentication failed".to_string());
                }
            }
            0xFF => {
                return Err("No acceptable authentication methods by SOCKS5 server".to_string());
            }
            other => {
                return Err(format!("Unsupported SOCKS5 auth method selected: 0x{:02X}", other));
            }
        }

        // 2. Send CONNECT Request
        let mut req = Vec::new();
        req.push(0x05); // Version
        req.push(0x01); // CMD: CONNECT
        req.push(0x00); // Reserved

        match target {
            Socks5Target::Domain(domain, port) => {
                req.push(0x03); // ATYP: Domain
                req.push(domain.len() as u8);
                req.extend_from_slice(domain.as_bytes());
                req.extend_from_slice(&port.to_be_bytes());
            }
            Socks5Target::IPv4(ip, port) => {
                req.push(0x01); // ATYP: IPv4
                req.extend_from_slice(&ip.octets());
                req.extend_from_slice(&port.to_be_bytes());
            }
            Socks5Target::IPv6(ip, port) => {
                req.push(0x04); // ATYP: IPv6
                req.extend_from_slice(&ip.octets());
                req.extend_from_slice(&port.to_be_bytes());
            }
        }

        stream.write_all(&req).await
            .map_err(|e| format!("SOCKS5 connect request write failed: {}", e))?;

        // 3. Read CONNECT Reply
        let mut reply_hdr = [0u8; 4];
        stream.read_exact(&mut reply_hdr).await
            .map_err(|e| format!("SOCKS5 connect reply read failed: {}", e))?;

        if reply_hdr[0] != 0x05 {
            return Err("Invalid SOCKS5 reply version".to_string());
        }

        if reply_hdr[1] != 0x00 {
            let msg = match reply_hdr[1] {
                0x01 => "General SOCKS server failure",
                0x02 => "Connection not allowed by ruleset",
                0x03 => "Network unreachable",
                0x04 => "Host unreachable",
                0x05 => "Connection refused",
                0x06 => "TTL expired",
                0x07 => "Command not supported",
                0x08 => "Address type not supported",
                _ => "Unknown error",
            };
            return Err(format!("SOCKS5 connection failed: {} (code 0x{:02X})", msg, reply_hdr[1]));
        }

        // Discard BND.ADDR and BND.PORT
        match reply_hdr[3] {
            0x01 => {
                // IPv4: 4 bytes IP + 2 bytes port
                let mut discard = [0u8; 6];
                stream.read_exact(&mut discard).await.ok();
            }
            0x03 => {
                // Domain: 1 byte len + domain + 2 bytes port
                let mut len_buf = [0u8; 1];
                stream.read_exact(&mut len_buf).await.ok();
                let mut discard = vec![0u8; len_buf[0] as usize + 2];
                stream.read_exact(&mut discard).await.ok();
            }
            0x04 => {
                // IPv6: 16 bytes IP + 2 bytes port
                let mut discard = [0u8; 18];
                stream.read_exact(&mut discard).await.ok();
            }
            _ => {}
        }

        Ok(stream)
    }

    /// Tests whether the configured proxy is reachable and can connect to a target.
    pub async fn test_proxy(proxy: &ProxyConfig) -> Result<String, String> {
        let test_target = Socks5Target::Domain("www.google.com".to_string(), 80);
        let start = std::time::Instant::now();

        let conn_res = timeout(
            Duration::from_secs(5),
            Self::connect(proxy, &test_target),
        ).await;

        match conn_res {
            Ok(Ok(_stream)) => {
                let elapsed = start.elapsed().as_millis();
                Ok(format!("Connected successfully (latency: {} ms)", elapsed))
            }
            Ok(Err(e)) => Err(e),
            Err(_) => Err("Connection timed out after 5 seconds".to_string()),
        }
    }
}
