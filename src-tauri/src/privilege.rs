use std::env;
use std::ffi::c_void;
use windows::core::PCWSTR;
use windows::Win32::Foundation::{CloseHandle, HANDLE};
use windows::Win32::Security::{
    GetTokenInformation, TokenElevation, TOKEN_ELEVATION, TOKEN_QUERY,
};
use windows::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};
use windows::Win32::UI::Shell::ShellExecuteW;
use windows::Win32::UI::WindowsAndMessaging::SW_SHOW;

/// Checks if the current process is running with elevated administrator privileges.
pub fn is_admin() -> bool {
    unsafe {
        let mut token: HANDLE = HANDLE::default();
        if OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token).is_err() {
            return false;
        }

        let mut elevation = TOKEN_ELEVATION::default();
        let mut return_length = 0u32;
        let success = GetTokenInformation(
            token,
            TokenElevation,
            Some(&mut elevation as *mut _ as *mut c_void),
            std::mem::size_of::<TOKEN_ELEVATION>() as u32,
            &mut return_length,
        );

        let _ = CloseHandle(token);

        if success.is_ok() {
            elevation.TokenIsElevated != 0
        } else {
            false
        }
    }
}

/// Relaunches the current application with UAC elevation (Run as administrator).
pub fn restart_as_admin() -> Result<(), String> {
    let current_exe = env::current_exe().map_err(|e| e.to_string())?;
    let exe_path_wide: Vec<u16> = current_exe
        .to_str()
        .ok_or("Invalid exe path")?
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect();

    let verb: Vec<u16> = "runas".encode_utf16().chain(std::iter::once(0)).collect();

    let dir_wide: Option<Vec<u16>> = current_exe
        .parent()
        .and_then(|p| p.to_str())
        .map(|s| s.encode_utf16().chain(std::iter::once(0)).collect());

    let lp_dir = match &dir_wide {
        Some(d) => PCWSTR(d.as_ptr()),
        None => PCWSTR::null(),
    };

    unsafe {
        let res = ShellExecuteW(
            None,
            PCWSTR(verb.as_ptr()),
            PCWSTR(exe_path_wide.as_ptr()),
            PCWSTR::null(),
            lp_dir,
            SW_SHOW,
        );

        if res.0 as usize > 32 {
            std::process::exit(0);
        } else {
            Err(format!("ShellExecuteW failed with error code: {}", res.0 as usize))
        }
    }
}
