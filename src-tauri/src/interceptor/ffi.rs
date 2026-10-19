#![allow(dead_code)]

use std::ffi::{c_void, CStr};
use std::os::raw::c_char;
use std::os::windows::ffi::OsStrExt;
use std::path::Path;
use windows::core::{PCSTR, PCWSTR};
use windows::Win32::Foundation::HMODULE;
use windows::Win32::System::LibraryLoader::{GetProcAddress, LoadLibraryW};

pub const WINDIVERT_LAYER_NETWORK: u32 = 0;
pub const WINDIVERT_LAYER_NETWORK_FORWARD: u32 = 1;
pub const WINDIVERT_LAYER_FLOW: u32 = 2;
pub const WINDIVERT_LAYER_SOCKET: u32 = 3;
pub const WINDIVERT_LAYER_REFLECT: u32 = 4;

pub const WINDIVERT_FLAG_SNIFF: u64 = 0x0001;
pub const WINDIVERT_FLAG_DROP: u64 = 0x0002;
pub const WINDIVERT_FLAG_RECV_ONLY: u64 = 0x0004;
pub const WINDIVERT_FLAG_SEND_ONLY: u64 = 0x0008;

pub const WINDIVERT_SHUTDOWN_BOTH: u32 = 0x3;

pub const WINDIVERT_HELPER_NO_IP_CHECKSUM: u64 = 1;
pub const WINDIVERT_HELPER_NO_ICMP_CHECKSUM: u64 = 2;
pub const WINDIVERT_HELPER_NO_ICMPV6_CHECKSUM: u64 = 4;

const WINDIVERT_DLL: &[u8] = include_bytes!("../../windivert_bin/WinDivert.dll");
const WINDIVERT_SYS: &[u8] = include_bytes!("../../windivert_bin/WinDivert64.sys");

#[repr(C)]
#[derive(Copy, Clone)]
pub struct WinDivertDataNetwork {
    pub if_idx: u32,
    pub sub_if_idx: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct WinDivertAddress {
    pub timestamp: i64,
    pub bitfield: u32,
    pub reserved2: u32,
    pub data: [u8; 64],
}

impl Default for WinDivertAddress {
    fn default() -> Self {
        Self { timestamp: 0, bitfield: 0, reserved2: 0, data: [0u8; 64] }
    }
}

impl WinDivertAddress {
    #[inline]
    pub fn layer(&self) -> u8 { (self.bitfield & 0xFF) as u8 }
    #[inline]
    pub fn set_layer(&mut self, layer: u8) { self.bitfield = (self.bitfield & !0xFF) | layer as u32; }
    #[inline]
    pub fn is_outbound(&self) -> bool { (self.bitfield & (1 << 17)) != 0 }
    #[inline]
    pub fn set_outbound(&mut self, outbound: bool) {
        if outbound { self.bitfield |= 1 << 17; } else { self.bitfield &= !(1 << 17); }
    }
    #[inline]
    pub fn is_loopback(&self) -> bool { (self.bitfield & (1 << 18)) != 0 }
    #[inline]
    pub fn set_loopback(&mut self, loopback: bool) {
        if loopback { self.bitfield |= 1 << 18; } else { self.bitfield &= !(1 << 18); }
    }
    #[inline]
    pub fn is_ipv6(&self) -> bool { (self.bitfield & (1 << 20)) != 0 }
    #[inline]
    pub fn network_data(&self) -> WinDivertDataNetwork {
        unsafe { *(self.data.as_ptr() as *const WinDivertDataNetwork) }
    }
}

pub type WinDivertHandle = *mut c_void;
type OpenFn = unsafe extern "system" fn(*const c_char, u32, i16, u64) -> WinDivertHandle;
type RecvFn = unsafe extern "system" fn(WinDivertHandle, *mut c_void, u32, *mut u32, *mut WinDivertAddress) -> i32;
type SendFn = unsafe extern "system" fn(WinDivertHandle, *const c_void, u32, *mut u32, *const WinDivertAddress) -> i32;
type ShutdownFn = unsafe extern "system" fn(WinDivertHandle, u32) -> i32;
type CloseFn = unsafe extern "system" fn(WinDivertHandle) -> i32;
type CalcChecksumsFn = unsafe extern "system" fn(*mut c_void, u32, *mut WinDivertAddress, u64) -> i32;

pub struct WinDivertApi {
    _module: HMODULE,
    open: OpenFn,
    recv: RecvFn,
    send: SendFn,
    shutdown: ShutdownFn,
    close: CloseFn,
    calc_checksums: CalcChecksumsFn,
}

// The module is kept loaded for the whole process lifetime.
unsafe impl Send for WinDivertApi {}
unsafe impl Sync for WinDivertApi {}

impl WinDivertApi {
    pub fn prepare_files(dir: &Path) -> Result<(), String> {
        std::fs::create_dir_all(dir).map_err(|e| format!("创建 WinDivert 目录失败: {e}"))?;
        write_embedded(&dir.join("WinDivert.dll"), WINDIVERT_DLL)?;
        write_embedded(&dir.join("WinDivert64.sys"), WINDIVERT_SYS)?;
        Ok(())
    }

    pub fn load(dll_path: &Path) -> Result<Self, String> {
        let wide: Vec<u16> = dll_path.as_os_str().encode_wide().chain(Some(0)).collect();
        let module = unsafe { LoadLibraryW(PCWSTR(wide.as_ptr())) }
            .map_err(|e| format!("加载 WinDivert.dll 失败: {e}"))?;
        unsafe {
            Ok(Self {
                _module: module,
                open: load_fn(module, b"WinDivertOpen\0")?,
                recv: load_fn(module, b"WinDivertRecv\0")?,
                send: load_fn(module, b"WinDivertSend\0")?,
                shutdown: load_fn(module, b"WinDivertShutdown\0")?,
                close: load_fn(module, b"WinDivertClose\0")?,
                calc_checksums: load_fn(module, b"WinDivertHelperCalcChecksums\0")?,
            })
        }
    }

    pub unsafe fn open(&self, filter: *const c_char, layer: u32, priority: i16, flags: u64) -> WinDivertHandle {
        (self.open)(filter, layer, priority, flags)
    }
    pub unsafe fn recv(&self, handle: WinDivertHandle, packet: *mut c_void, packet_len: u32, recv_len: *mut u32, addr: *mut WinDivertAddress) -> i32 {
        (self.recv)(handle, packet, packet_len, recv_len, addr)
    }
    pub unsafe fn send(&self, handle: WinDivertHandle, packet: *const c_void, packet_len: u32, send_len: *mut u32, addr: *const WinDivertAddress) -> i32 {
        (self.send)(handle, packet, packet_len, send_len, addr)
    }
    pub unsafe fn shutdown(&self, handle: WinDivertHandle, how: u32) -> i32 { (self.shutdown)(handle, how) }
    pub unsafe fn close(&self, handle: WinDivertHandle) -> i32 { (self.close)(handle) }
    pub unsafe fn calc_checksums(&self, packet: *mut c_void, packet_len: u32, addr: *mut WinDivertAddress, flags: u64) -> i32 {
        (self.calc_checksums)(packet, packet_len, addr, flags)
    }
}

fn write_embedded(path: &Path, bytes: &[u8]) -> Result<(), String> {
    if path.exists() && std::fs::read(path).ok().as_deref() == Some(bytes) { return Ok(()); }
    let temp = path.with_extension("tmp");
    std::fs::write(&temp, bytes).map_err(|e| format!("释放 {} 失败: {e}", path.display()))?;
    std::fs::rename(&temp, path).or_else(|_| { let _ = std::fs::remove_file(path); std::fs::rename(&temp, path) })
        .map_err(|e| format!("更新 {} 失败: {e}", path.display()))
}

unsafe fn load_fn<T: Copy>(module: HMODULE, name: &'static [u8]) -> Result<T, String> {
    let symbol = CStr::from_bytes_with_nul(name).unwrap();
    let address = GetProcAddress(module, PCSTR(symbol.as_ptr() as *const u8))
        .ok_or_else(|| format!("WinDivert 缺少导出函数 {}", symbol.to_string_lossy()))?;
    Ok(std::mem::transmute_copy(&address))
}
