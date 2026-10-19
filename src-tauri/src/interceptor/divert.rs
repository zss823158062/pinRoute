use super::ffi::*;
use crate::config::AppConfig;
use crate::process::{ProcessManager, Transport};
use parking_lot::{Mutex, RwLock};
use std::collections::HashMap;
use std::ffi::CString;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

const PROTO_TCP: u8 = 6;
const PROTO_UDP: u8 = 17;

const TCP_FIN: u8 = 0x01;
const TCP_SYN: u8 = 0x02;
const TCP_RST: u8 = 0x04;
const TCP_ACK: u8 = 0x10;

const PID_CACHE_TTL: Duration = Duration::from_secs(10);
const NAT_IDLE_TIMEOUT: Duration = Duration::from_secs(3600);
const NAT_CLOSED_GRACE: Duration = Duration::from_secs(10);

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct OriginalTarget {
    pub dest_ip: IpAddr,
    pub dest_port: u16,
    pub pid: u32,
    pub process_name: String,
    pub rule_name: String,
}

#[derive(Debug, Clone)]
struct NatEntry {
    target: OriginalTarget,
    last_active: Instant,
    /// bit0: client sent FIN, bit1: relay sent FIN
    fin_mask: u8,
    closed_at: Option<Instant>,
}

/// NAT key: (is_ipv6, client source port)
type NatKey = (bool, u16);
type NatTable = Arc<RwLock<HashMap<NatKey, NatEntry>>>;

#[derive(Clone)]
struct Matched {
    process_name: String,
    rule_name: String,
}

type PidCache = Arc<Mutex<HashMap<u32, (Instant, Option<Matched>)>>>;

struct RunState {
    stop: Arc<AtomicBool>,
    handle: usize,
}

pub struct WinDivertInterceptor {
    state: Mutex<Option<RunState>>,
    api: Mutex<Option<Arc<WinDivertApi>>>,
    nat_table: NatTable,
    pid_cache: PidCache,
}

impl WinDivertInterceptor {
    pub fn new() -> Self {
        Self {
            state: Mutex::new(None),
            api: Mutex::new(None),
            nat_table: Arc::new(RwLock::new(HashMap::new())),
            pid_cache: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn get_original_target(&self, ipv6: bool, client_port: u16) -> Option<OriginalTarget> {
        self.nat_table
            .read()
            .get(&(ipv6, client_port))
            .map(|entry| entry.target.clone())
    }

    /// Drop cached per-PID rule decisions (call after rules change).
    pub fn invalidate_cache(&self) {
        self.pid_cache.lock().clear();
    }

    pub fn start(
        &self,
        config: Arc<RwLock<AppConfig>>,
        relay_port: u16,
        ipv6_enabled: bool,
    ) -> Result<(), String> {
        let mut state = self.state.lock();
        if state.is_some() {
            return Ok(());
        }

        // Only outbound non-loopback traffic is needed: both the client's packets and the
        // relay's replies leave the stack as outbound; rewritten packets are injected inbound.
        let filter = "outbound and not loopback and (tcp or udp.DstPort == 443)";
        let c_filter = CString::new(filter).map_err(|e| e.to_string())?;
        let api = self.api()?;
        let handle = unsafe { api.open(c_filter.as_ptr(), WINDIVERT_LAYER_NETWORK, 0, 0) };
        if handle.is_null() || handle as isize == -1 {
            return Err(format!(
                "Failed to open WinDivert handle (error {}). Ensure running as Administrator.",
                std::io::Error::last_os_error()
            ));
        }

        let stop = Arc::new(AtomicBool::new(false));
        let worker = DivertWorker {
            api: api.clone(),
            handle: handle as usize,
            stop: stop.clone(),
            nat_table: self.nat_table.clone(),
            pid_cache: self.pid_cache.clone(),
            config,
            relay_port,
            ipv6_enabled,
            self_pid: std::process::id(),
        };

        if let Err(e) = thread::Builder::new()
            .name("windivert-interceptor".to_string())
            .spawn(move || worker.run())
        {
            unsafe { api.close(handle) };
            return Err(format!("Failed to spawn interceptor thread: {}", e));
        }

        *state = Some(RunState { stop, handle: handle as usize });
        Ok(())
    }

    /// Extract the embedded WinDivert files and load the DLL once per process.
    fn api(&self) -> Result<Arc<WinDivertApi>, String> {
        let mut api = self.api.lock();
        if let Some(api) = api.as_ref() {
            return Ok(api.clone());
        }
        let dir = AppConfig::app_dir();
        WinDivertApi::prepare_files(&dir)?;
        let loaded = Arc::new(WinDivertApi::load(&dir.join("WinDivert.dll"))?);
        *api = Some(loaded.clone());
        Ok(loaded)
    }

    pub fn stop(&self) {
        if let Some(run) = self.state.lock().take() {
            run.stop.store(true, Ordering::SeqCst);
            // Wake up the blocking WinDivertRecv; the worker thread closes the handle itself.
            if let Some(api) = self.api.lock().as_ref() {
                unsafe { api.shutdown(run.handle as WinDivertHandle, WINDIVERT_SHUTDOWN_BOTH) };
            }
        }
        self.nat_table.write().clear();
        self.invalidate_cache();
    }
}

/// Parsed view of an IPv4/IPv6 packet header.
struct Packet {
    ipv6: bool,
    proto: u8,
    l4: usize,
}

impl Packet {
    fn parse(p: &[u8]) -> Option<Self> {
        match p.first()? >> 4 {
            4 if p.len() >= 20 => {
                let ihl = (p[0] & 0x0F) as usize * 4;
                Some(Self { ipv6: false, proto: p[9], l4: ihl })
            }
            // Extension headers are not followed; such packets are passed through untouched.
            6 if p.len() >= 40 => Some(Self { ipv6: true, proto: p[6], l4: 40 }),
            _ => None,
        }
    }

    fn dst_ip(&self, p: &[u8]) -> IpAddr {
        if self.ipv6 {
            IpAddr::V6(Ipv6Addr::from(<[u8; 16]>::try_from(&p[24..40]).unwrap()))
        } else {
            IpAddr::V4(Ipv4Addr::new(p[16], p[17], p[18], p[19]))
        }
    }

    fn swap_ips(&self, p: &mut [u8]) {
        let (src, len) = if self.ipv6 { (8, 16) } else { (12, 4) };
        let (a, b) = p[src..src + 2 * len].split_at_mut(len);
        a.swap_with_slice(b);
    }

    fn src_port(&self, p: &[u8]) -> u16 {
        u16::from_be_bytes([p[self.l4], p[self.l4 + 1]])
    }

    fn dst_port(&self, p: &[u8]) -> u16 {
        u16::from_be_bytes([p[self.l4 + 2], p[self.l4 + 3]])
    }

    fn set_src_port(&self, p: &mut [u8], port: u16) {
        p[self.l4..self.l4 + 2].copy_from_slice(&port.to_be_bytes());
    }

    fn set_dst_port(&self, p: &mut [u8], port: u16) {
        p[self.l4 + 2..self.l4 + 4].copy_from_slice(&port.to_be_bytes());
    }
}

struct DivertWorker {
    api: Arc<WinDivertApi>,
    handle: usize,
    stop: Arc<AtomicBool>,
    nat_table: NatTable,
    pid_cache: PidCache,
    config: Arc<RwLock<AppConfig>>,
    relay_port: u16,
    ipv6_enabled: bool,
    self_pid: u32,
}

impl DivertWorker {
    fn run(self) {
        let mut buf = vec![0u8; 65535];
        let mut addr = WinDivertAddress::default();
        let mut recv_len = 0u32;
        let mut last_cleanup = Instant::now();

        while !self.stop.load(Ordering::SeqCst) {
            if last_cleanup.elapsed() > Duration::from_secs(30) {
                self.cleanup();
                last_cleanup = Instant::now();
            }

            let ok = unsafe {
                self.api.recv(self.raw(), buf.as_mut_ptr() as *mut _, buf.len() as u32, &mut recv_len, &mut addr)
            };
            if ok == 0 {
                if self.stop.load(Ordering::SeqCst) {
                    break;
                }
                thread::sleep(Duration::from_millis(5));
                continue;
            }

            let p = &mut buf[..recv_len as usize];
            match self.process(p, &mut addr) {
                Verdict::Pass => self.send(p, &addr),
                Verdict::Modified => {
                    unsafe {
                        self.api.calc_checksums(p.as_mut_ptr() as *mut _, recv_len, &mut addr, 0);
                    }
                    self.send(p, &addr);
                }
                Verdict::Drop => {}
            }
        }

        unsafe { self.api.close(self.raw()) };
    }

    fn raw(&self) -> WinDivertHandle {
        self.handle as WinDivertHandle
    }

    fn send(&self, p: &[u8], addr: &WinDivertAddress) {
        unsafe {
            self.api.send(self.raw(), p.as_ptr() as *const _, p.len() as u32, std::ptr::null_mut(), addr);
        }
    }

    fn process(&self, p: &mut [u8], addr: &mut WinDivertAddress) -> Verdict {
        let pkt = match Packet::parse(p) {
            Some(pkt) if !pkt.ipv6 || self.ipv6_enabled => pkt,
            _ => return Verdict::Pass,
        };

        match pkt.proto {
            PROTO_UDP if p.len() >= pkt.l4 + 8 => self.process_udp(&pkt, p),
            PROTO_TCP if p.len() >= pkt.l4 + 20 => self.process_tcp(&pkt, p, addr),
            _ => Verdict::Pass,
        }
    }

    /// Drop QUIC (UDP 443) of matched processes so they fall back to TCP (F-27).
    fn process_udp(&self, pkt: &Packet, p: &[u8]) -> Verdict {
        if pkt.dst_port(p) != 443 || !self.config.read().settings.block_udp_443 {
            return Verdict::Pass;
        }
        match ProcessManager::find_pid_by_port(Transport::Udp, pkt.ipv6, pkt.src_port(p)) {
            Some(pid) if self.match_pid(pid).is_some() => Verdict::Drop,
            _ => Verdict::Pass,
        }
    }

    fn process_tcp(&self, pkt: &Packet, p: &mut [u8], addr: &mut WinDivertAddress) -> Verdict {
        let src_port = pkt.src_port(p);
        let dst_port = pkt.dst_port(p);
        let flags = p[pkt.l4 + 13];
        let is_syn = flags & TCP_SYN != 0 && flags & TCP_ACK == 0;

        // 1. Relay -> client reply: restore original source port, swap IPs, inject inbound.
        if src_port == self.relay_port {
            let key = (pkt.ipv6, dst_port);
            let orig_port = match self.touch(key, flags, 0b10) {
                Some(port) => port,
                None => return Verdict::Pass,
            };
            pkt.set_src_port(p, orig_port);
            pkt.swap_ips(p);
            addr.set_outbound(false);
            return Verdict::Modified;
        }

        // 2. Client -> server packet of a tracked flow (a new SYN on the same port starts over).
        let key = (pkt.ipv6, src_port);
        if is_syn {
            self.nat_table.write().remove(&key);
        } else if self.touch(key, flags, 0b01).is_none() {
            return Verdict::Pass;
        }

        if is_syn && !self.register_flow(pkt, p, key, dst_port) {
            return Verdict::Pass;
        }

        // Redirect to the relay: dst port -> relay, swap IPs and inject as inbound so the
        // stack delivers it to the relay listener bound on the wildcard address.
        pkt.set_dst_port(p, self.relay_port);
        pkt.swap_ips(p);
        addr.set_outbound(false);
        Verdict::Modified
    }

    /// Update an existing NAT entry and return the original destination port.
    fn touch(&self, key: NatKey, flags: u8, fin_bit: u8) -> Option<u16> {
        let mut table = self.nat_table.write();
        let entry = table.get_mut(&key)?;
        let port = entry.target.dest_port;
        if flags & TCP_RST != 0 {
            table.remove(&key);
            return Some(port);
        }
        entry.last_active = Instant::now();
        if flags & TCP_FIN != 0 {
            entry.fin_mask |= fin_bit;
            if entry.fin_mask == 0b11 && entry.closed_at.is_none() {
                entry.closed_at = Some(Instant::now());
            }
        }
        Some(port)
    }

    /// Decide whether a new outbound SYN should be proxied; registers it in the NAT table if so.
    fn register_flow(&self, pkt: &Packet, p: &[u8], key: NatKey, dst_port: u16) -> bool {
        let dst_ip = pkt.dst_ip(p);
        {
            let config = self.config.read();
            if is_bypassed(&dst_ip, config.settings.bypass_lan) {
                return false;
            }
            // Avoid loop: matched apps talking to the upstream proxy directly
            if dst_port == config.proxy.port && config.proxy.host == dst_ip.to_string() {
                return false;
            }
        }

        let pid = match ProcessManager::find_pid_by_port(Transport::Tcp, pkt.ipv6, key.1) {
            Some(pid) if pid != self.self_pid => pid,
            _ => return false,
        };
        let matched = match self.match_pid(pid) {
            Some(m) => m,
            None => return false,
        };

        self.nat_table.write().insert(
            key,
            NatEntry {
                target: OriginalTarget {
                    dest_ip: dst_ip,
                    dest_port: dst_port,
                    pid,
                    process_name: matched.process_name,
                    rule_name: matched.rule_name,
                },
                last_active: Instant::now(),
                fin_mask: 0,
                closed_at: None,
            },
        );
        true
    }

    fn match_pid(&self, pid: u32) -> Option<Matched> {
        if let Some((at, decision)) = self.pid_cache.lock().get(&pid) {
            if at.elapsed() < PID_CACHE_TTL {
                return decision.clone();
            }
        }

        let rules = self.config.read().rules.clone();
        let tree = ProcessManager::get_process_tree_map();
        let decision = ProcessManager::matches_rules(pid, &rules, &tree).map(|rule| Matched {
            process_name: ProcessManager::get_process_path(pid).unwrap_or_else(|| format!("PID:{}", pid)),
            rule_name: rule.name,
        });

        self.pid_cache.lock().insert(pid, (Instant::now(), decision.clone()));
        decision
    }

    fn cleanup(&self) {
        let now = Instant::now();
        self.nat_table.write().retain(|_, v| {
            let closed = v.closed_at.map_or(false, |t| now.duration_since(t) > NAT_CLOSED_GRACE);
            !closed && now.duration_since(v.last_active) < NAT_IDLE_TIMEOUT
        });
        self.pid_cache
            .lock()
            .retain(|_, (at, _)| now.duration_since(*at) < PID_CACHE_TTL);
    }
}

enum Verdict {
    Pass,
    Modified,
    Drop,
}

/// Loopback / link-local are never redirected; private ranges only when bypass_lan is on.
fn is_bypassed(ip: &IpAddr, bypass_lan: bool) -> bool {
    match ip {
        IpAddr::V4(v4) => {
            v4.is_loopback()
                || v4.is_link_local()
                || v4.is_unspecified()
                || v4.is_multicast()
                || (bypass_lan && v4.is_private())
        }
        IpAddr::V6(v6) => {
            let seg0 = v6.segments()[0];
            v6.is_loopback()
                || v6.is_unspecified()
                || v6.is_multicast()
                || (seg0 & 0xffc0) == 0xfe80 // link-local
                || (bypass_lan && (seg0 & 0xfe00) == 0xfc00) // unique local
        }
    }
}
