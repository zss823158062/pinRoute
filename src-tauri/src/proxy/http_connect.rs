use crate::config::ProxyConfig;
use crate::proxy::socks5::Socks5Target;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::time::timeout;

pub struct HttpConnectClient;

impl HttpConnectClient {
    /// Connects to the upstream HTTP proxy and negotiates a CONNECT tunnel to the target.
    pub async fn connect(
        proxy: &ProxyConfig,
        target: &Socks5Target,
    ) -> Result<TcpStream, String> {
        let proxy_addr = format!("{}:{}", proxy.host, proxy.port);
        let mut stream = TcpStream::connect(&proxy_addr)
            .await
            .map_err(|e| format!("Failed to connect to HTTP upstream ({}): {}", proxy_addr, e))?;

        let target_str = target.to_string();

        // Construct HTTP CONNECT request
        let mut req = format!(
            "CONNECT {} HTTP/1.1\r\nHost: {}\r\nProxy-Connection: Keep-Alive\r\n",
            target_str, target_str
        );

        // Optional Basic Auth
        if let (Some(ref u), Some(ref p)) = (&proxy.username, &proxy.password) {
            if !u.is_empty() {
                use base64::Engine;
                let credentials = format!("{}:{}", u, p);
                let encoded = base64::engine::general_purpose::STANDARD.encode(credentials.as_bytes());
                req.push_str(&format!("Proxy-Authorization: Basic {}\r\n", encoded));
            }
        }

        req.push_str("\r\n");

        stream
            .write_all(req.as_bytes())
            .await
            .map_err(|e| format!("Failed to write HTTP CONNECT request: {}", e))?;

        // Read response header until \r\n\r\n
        let mut response_buf = Vec::with_capacity(1024);
        let mut temp_buf = [0u8; 512];

        loop {
            let n = stream
                .read(&mut temp_buf)
                .await
                .map_err(|e| format!("Failed to read HTTP CONNECT response: {}", e))?;

            if n == 0 {
                return Err("HTTP proxy closed connection prematurely".to_string());
            }

            response_buf.extend_from_slice(&temp_buf[..n]);

            // Check if end of headers reached
            if let Some(pos) = response_buf.windows(4).position(|w| w == b"\r\n\r\n") {
                let header_part = String::from_utf8_lossy(&response_buf[..pos]);
                let status_line = header_part.lines().next().unwrap_or("");

                // Valid HTTP response: "HTTP/1.1 200 ..." or "HTTP/1.0 200 ..."
                if status_line.contains(" 200") {
                    return Ok(stream);
                } else {
                    return Err(format!(
                        "HTTP proxy rejected tunnel request: {}",
                        status_line.trim()
                    ));
                }
            }

            if response_buf.len() > 8192 {
                return Err("HTTP proxy response header too large".to_string());
            }
        }
    }

    /// Tests whether the configured HTTP proxy is reachable and can establish a CONNECT tunnel.
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
                Ok(format!("Connected successfully via HTTP tunnel (latency: {} ms)", elapsed))
            }
            Ok(Err(e)) => Err(e),
            Err(_) => Err("HTTP proxy connection timed out after 5 seconds".to_string()),
        }
    }
}
