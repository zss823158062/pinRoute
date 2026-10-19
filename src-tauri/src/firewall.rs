use std::os::windows::process::CommandExt;
use std::process::{Command, Stdio};

const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// Ensure an inbound allow rule exists for this executable.
///
/// Redirected connections are injected into the stack as inbound packets addressed to the
/// relay port, so a default-deny inbound firewall would silently drop them.
pub fn ensure_inbound_rule() {
    let exe = match std::env::current_exe() {
        Ok(p) => p.to_string_lossy().to_string(),
        Err(_) => return,
    };
    let name = format!("Pinroute relay {:08x}", fnv1a(&exe.to_lowercase()));

    if netsh(&["show", "rule", &format!("name={}", name)]) {
        return;
    }
    let ok = netsh(&[
        "add",
        "rule",
        &format!("name={}", name),
        "dir=in",
        "action=allow",
        &format!("program={}", exe),
        "enable=yes",
        "profile=any",
        "protocol=tcp",
    ]);
    if !ok {
        eprintln!("Failed to add Windows Firewall rule for relay");
    }
}

fn netsh(args: &[&str]) -> bool {
    Command::new("netsh.exe")
        .args(["advfirewall", "firewall"])
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .creation_flags(CREATE_NO_WINDOW)
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

fn fnv1a(s: &str) -> u32 {
    s.bytes()
        .fold(0x811c_9dc5u32, |h, b| (h ^ b as u32).wrapping_mul(0x0100_0193))
}
