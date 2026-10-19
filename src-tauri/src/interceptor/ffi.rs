#![allow(dead_code)]

use std::ffi::c_void;
use std::os::raw::c_char;

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
        Self {
            timestamp: 0,
            bitfield: 0,
            reserved2: 0,
            data: [0u8; 64],
        }
    }
}

impl WinDivertAddress {
    #[inline]
    pub fn layer(&self) -> u8 {
        (self.bitfield & 0xFF) as u8
    }

    #[inline]
    pub fn set_layer(&mut self, layer: u8) {
        self.bitfield = (self.bitfield & !0xFF) | (layer as u32);
    }

    #[inline]
    pub fn is_outbound(&self) -> bool {
        (self.bitfield & (1 << 17)) != 0
    }

    #[inline]
    pub fn set_outbound(&mut self, outbound: bool) {
        if outbound {
            self.bitfield |= 1 << 17;
        } else {
            self.bitfield &= !(1 << 17);
        }
    }

    #[inline]
    pub fn is_loopback(&self) -> bool {
        (self.bitfield & (1 << 18)) != 0
    }

    #[inline]
    pub fn set_loopback(&mut self, loopback: bool) {
        if loopback {
            self.bitfield |= 1 << 18;
        } else {
            self.bitfield &= !(1 << 18);
        }
    }

    #[inline]
    pub fn is_ipv6(&self) -> bool {
        (self.bitfield & (1 << 20)) != 0
    }

    #[inline]
    pub fn network_data(&self) -> WinDivertDataNetwork {
        unsafe { *(self.data.as_ptr() as *const WinDivertDataNetwork) }
    }
}

pub type WinDivertHandle = *mut c_void;

#[link(name = "WinDivert")]
extern "system" {
    pub fn WinDivertOpen(
        filter: *const c_char,
        layer: u32,
        priority: i16,
        flags: u64,
    ) -> WinDivertHandle;

    pub fn WinDivertRecv(
        handle: WinDivertHandle,
        packet: *mut c_void,
        packet_len: u32,
        recv_len: *mut u32,
        addr: *mut WinDivertAddress,
    ) -> i32;

    pub fn WinDivertSend(
        handle: WinDivertHandle,
        packet: *const c_void,
        packet_len: u32,
        send_len: *mut u32,
        addr: *const WinDivertAddress,
    ) -> i32;

    pub fn WinDivertShutdown(handle: WinDivertHandle, how: u32) -> i32;

    pub fn WinDivertClose(handle: WinDivertHandle) -> i32;

    pub fn WinDivertHelperCalcChecksums(
        packet: *mut c_void,
        packet_len: u32,
        addr: *mut WinDivertAddress,
        flags: u64,
    ) -> i32;
}
