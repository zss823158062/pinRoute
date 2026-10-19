use crate::config::{AppRule, MatchType};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::Path;
use windows::core::PWSTR;
use windows::Win32::Foundation::{CloseHandle, ERROR_INSUFFICIENT_BUFFER};
use windows::Win32::NetworkManagement::IpHelper::{
    GetExtendedTcpTable, GetExtendedUdpTable, MIB_TCP6ROW_OWNER_PID, MIB_TCPROW_OWNER_PID,
    MIB_UDP6ROW_OWNER_PID, MIB_UDPROW_OWNER_PID, TCP_TABLE_OWNER_PID_ALL, UDP_TABLE_OWNER_PID,
};
use windows::Win32::Networking::WinSock::{AF_INET, AF_INET6};
use windows::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W, TH32CS_SNAPPROCESS,
};
use windows::Win32::System::Threading::{
    OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32,
    PROCESS_QUERY_LIMITED_INFORMATION,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Transport {
    Tcp,
    Udp,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessInfo {
    pub pid: u32,
    pub name: String,
    pub path: String,
}

pub struct ProcessManager;

impl ProcessManager {
    /// List all currently running processes that have a valid executable path.
    pub fn list_running_processes() -> Vec<ProcessInfo> {
        let mut processes = Vec::new();
        let mut seen_paths = HashSet::new();

        unsafe {
            let snapshot = match CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) {
                Ok(h) => h,
                Err(_) => return processes,
            };

            let mut entry = PROCESSENTRY32W::default();
            entry.dwSize = std::mem::size_of::<PROCESSENTRY32W>() as u32;

            if Process32FirstW(snapshot, &mut entry).is_ok() {
                loop {
                    let pid = entry.th32ProcessID;
                    if pid > 4 {
                        let name_len = entry
                            .szExeFile
                            .iter()
                            .position(|&c| c == 0)
                            .unwrap_or(entry.szExeFile.len());
                        let name = String::from_utf16_lossy(&entry.szExeFile[..name_len]);

                        if let Some(path) = Self::get_process_path(pid) {
                            if !path.is_empty() && !seen_paths.contains(&path) {
                                seen_paths.insert(path.clone());
                                processes.push(ProcessInfo {
                                    pid,
                                    name,
                                    path,
                                });
                            }
                        }
                    }

                    if Process32NextW(snapshot, &mut entry).is_err() {
                        break;
                    }
                }
            }

            let _ = CloseHandle(snapshot);
        }

        processes.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
        processes
    }

    /// Retrieve the full executable image path for a given process ID.
    pub fn get_process_path(pid: u32) -> Option<String> {
        unsafe {
            // LIMITED_INFORMATION works for far more processes than QUERY_INFORMATION | VM_READ
            let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok()?;
            let mut buffer = [0u16; 1024];
            let mut len = buffer.len() as u32;
            let res = QueryFullProcessImageNameW(
                handle,
                PROCESS_NAME_WIN32,
                PWSTR(buffer.as_mut_ptr()),
                &mut len,
            );
            let _ = CloseHandle(handle);
            res.ok()?;

            let path = String::from_utf16_lossy(&buffer[..len as usize]);
            Some(path.trim_start_matches(r"\\?\").to_string())
        }
    }

    /// Get a snapshot of process parent-child mapping (pid -> ppid)
    pub fn get_process_tree_map() -> HashMap<u32, u32> {
        let mut tree = HashMap::new();
        unsafe {
            if let Ok(snapshot) = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) {
                let mut entry = PROCESSENTRY32W::default();
                entry.dwSize = std::mem::size_of::<PROCESSENTRY32W>() as u32;

                if Process32FirstW(snapshot, &mut entry).is_ok() {
                    loop {
                        tree.insert(entry.th32ProcessID, entry.th32ParentProcessID);
                        if Process32NextW(snapshot, &mut entry).is_err() {
                            break;
                        }
                    }
                }
                let _ = CloseHandle(snapshot);
            }
        }
        tree
    }

    /// Find the Process ID that owns a local TCP/UDP port (IPv4 or IPv6).
    /// Dual-stack sockets carry IPv4 traffic but are listed in the IPv6 table, so fall back to it.
    pub fn find_pid_by_port(transport: Transport, ipv6: bool, local_port: u16) -> Option<u32> {
        Self::find_pid_in_table(transport, ipv6, local_port)
            .or_else(|| if ipv6 { None } else { Self::find_pid_in_table(transport, true, local_port) })
    }

    fn find_pid_in_table(transport: Transport, ipv6: bool, local_port: u16) -> Option<u32> {
        let family = if ipv6 { AF_INET6.0 } else { AF_INET.0 } as u32;
        let buffer = Self::fetch_owner_table(transport, family)?;

        // Layout: { dwNumEntries: u32, table: [ROW; n] }, every ROW type is 4-byte aligned
        let count = buffer[0] as usize;
        let rows = buffer[1..].as_ptr() as *const u8;
        unsafe {
            match (transport, ipv6) {
                (Transport::Tcp, false) => Self::find_in_rows(rows, count, local_port, |r: &MIB_TCPROW_OWNER_PID| (r.dwLocalPort, r.dwOwningPid)),
                (Transport::Tcp, true) => Self::find_in_rows(rows, count, local_port, |r: &MIB_TCP6ROW_OWNER_PID| (r.dwLocalPort, r.dwOwningPid)),
                (Transport::Udp, false) => Self::find_in_rows(rows, count, local_port, |r: &MIB_UDPROW_OWNER_PID| (r.dwLocalPort, r.dwOwningPid)),
                (Transport::Udp, true) => Self::find_in_rows(rows, count, local_port, |r: &MIB_UDP6ROW_OWNER_PID| (r.dwLocalPort, r.dwOwningPid)),
            }
        }
    }

    fn fetch_owner_table(transport: Transport, family: u32) -> Option<Vec<u32>> {
        let mut size = 0u32;
        // The table may grow between the size query and the actual call, so retry a few times
        for _ in 0..4 {
            let mut buffer = vec![0u32; (size as usize + 3) / 4 + 64];
            let mut byte_len = (buffer.len() * 4) as u32;
            let ptr = Some(buffer.as_mut_ptr() as *mut core::ffi::c_void);
            let res = unsafe {
                match transport {
                    Transport::Tcp => GetExtendedTcpTable(ptr, &mut byte_len, false, family, TCP_TABLE_OWNER_PID_ALL, 0),
                    Transport::Udp => GetExtendedUdpTable(ptr, &mut byte_len, false, family, UDP_TABLE_OWNER_PID, 0),
                }
            };
            if res == 0 {
                return Some(buffer);
            }
            if res != ERROR_INSUFFICIENT_BUFFER.0 {
                return None;
            }
            size = byte_len;
        }
        None
    }

    unsafe fn find_in_rows<R>(
        rows: *const u8,
        count: usize,
        local_port: u16,
        key: impl Fn(&R) -> (u32, u32),
    ) -> Option<u32> {
        std::slice::from_raw_parts(rows as *const R, count)
            .iter()
            .map(key)
            .find(|(port, _)| u16::from_be(*port as u16) == local_port)
            .map(|(_, pid)| pid)
    }

    /// Check if a process path or its parent tree matches any configured rules.
    pub fn matches_rules(
        pid: u32,
        rules: &[AppRule],
        tree_map: &HashMap<u32, u32>,
    ) -> Option<AppRule> {
        let active_rules: Vec<&AppRule> = rules.iter().filter(|r| r.enabled).collect();
        if active_rules.is_empty() {
            return None;
        }

        // 1. Direct match on process path or filename
        if let Some(path) = Self::get_process_path(pid) {
            for rule in &active_rules {
                if Self::is_path_matched(&path, rule) {
                    return Some((*rule).clone());
                }
            }
        }

        // 2. Parent-child process tree check (F-05)
        let mut current_pid = pid;
        let mut depth = 0;
        while let Some(&parent_pid) = tree_map.get(&current_pid) {
            if parent_pid == 0 || parent_pid == current_pid || depth > 8 {
                break;
            }

            if let Some(parent_path) = Self::get_process_path(parent_pid) {
                for rule in &active_rules {
                    if rule.include_children && Self::is_path_matched(&parent_path, rule) {
                        return Some((*rule).clone());
                    }
                }
            }

            current_pid = parent_pid;
            depth += 1;
        }

        None
    }

    fn is_path_matched(target_path: &str, rule: &AppRule) -> bool {
        let norm_target = target_path.replace('/', "\\").to_lowercase();
        let norm_rule = rule.path.replace('/', "\\").to_lowercase();

        match rule.match_type {
            MatchType::Exact => {
                // If rule is just a filename (e.g. "chrome.exe")
                if !norm_rule.contains('\\') {
                    if let Some(file_name) = Path::new(&norm_target).file_name().and_then(|f| f.to_str()) {
                        return file_name == norm_rule;
                    }
                }
                norm_target == norm_rule
            }
            MatchType::Prefix => {
                norm_target.starts_with(&norm_rule)
            }
            MatchType::Wildcard => {
                if let Ok(pattern) = glob::Pattern::new(&norm_rule) {
                    pattern.matches(&norm_target)
                } else {
                    norm_target.contains(&norm_rule)
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_process_path_matching() {
        let rule_exact = AppRule {
            id: "1".into(),
            name: "Claude".into(),
            path: r"C:\Users\Admin\AppData\Local\Programs\Claude\Claude.exe".into(),
            match_type: MatchType::Exact,
            include_children: false,
            enabled: true,
        };

        // Exact match with different case and slashes
        assert!(ProcessManager::is_path_matched(
            r"c:/users/admin/appdata/local/programs/claude/claude.exe",
            &rule_exact
        ));

        // Prefix match
        let rule_prefix = AppRule {
            id: "2".into(),
            name: "Programs".into(),
            path: r"C:\Users\Admin\AppData\Local\Programs\".into(),
            match_type: MatchType::Prefix,
            include_children: true,
            enabled: true,
        };
        assert!(ProcessManager::is_path_matched(
            r"C:\Users\Admin\AppData\Local\Programs\ChatGPT\ChatGPT.exe",
            &rule_prefix
        ));

        // Wildcard match
        let rule_wildcard = AppRule {
            id: "3".into(),
            name: "Anthropic".into(),
            path: r"*claude*.exe".into(),
            match_type: MatchType::Wildcard,
            include_children: false,
            enabled: true,
        };
        assert!(ProcessManager::is_path_matched(
            r"D:\Tools\Claude-desktop.exe",
            &rule_wildcard
        ));
    }
}
