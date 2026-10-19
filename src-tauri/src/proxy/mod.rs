pub mod http_connect;
pub mod socks5;

pub use http_connect::HttpConnectClient;
pub use socks5::{Socks5Client, Socks5Target};

use crate::config::{ProxyConfig, ProxyType};
use tokio::net::TcpStream;

pub struct ProxyClient;

impl ProxyClient {
    pub async fn connect(
        proxy: &ProxyConfig,
        target: &Socks5Target,
    ) -> Result<TcpStream, String> {
        match proxy.protocol {
            ProxyType::Socks5 => Socks5Client::connect(proxy, target).await,
            ProxyType::Http => HttpConnectClient::connect(proxy, target).await,
        }
    }

    pub async fn test_proxy(proxy: &ProxyConfig) -> Result<String, String> {
        match proxy.protocol {
            ProxyType::Socks5 => Socks5Client::test_proxy(proxy).await,
            ProxyType::Http => HttpConnectClient::test_proxy(proxy).await,
        }
    }
}
