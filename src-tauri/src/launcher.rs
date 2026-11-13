//! Quick-launch support for rule targets.
//!
//! Pinroute usually runs elevated (WinDivert requires admin). Spawning a child
//! directly would make the target app inherit the elevated token, which breaks
//! drag & drop, file associations, per-user settings, etc. When elevated we
//! therefore hand the launch over to the desktop shell (`explorer.exe`), which
//! starts the program with the normal (non-elevated) user token.
//!
//! MSIX / Microsoft Store apps (e.g. ChatGPT desktop) live under
//! `...\WindowsApps\` and cannot be started reliably by exe path, so they are
//! activated through `shell:AppsFolder\<AUMID>` instead.

use crate::privilege::is_admin;
use std::path::{Path, PathBuf};
use std::process::Command;

pub fn launch_program(path: &str) -> Result<(), String> {
    let path = path.trim().trim_matches('"');
    if path.is_empty() {
        return Err("程序路径为空".into());
    }
    if path.contains('*') || path.contains('?') {
        return Err("通配符规则无法直接启动，请使用精确的 .exe 路径".into());
    }

    let exe = PathBuf::from(path);

    // Packaged (Store / MSIX) apps: activate via AUMID.
    if let Some(aumid) = resolve_packaged_aumid(&exe) {
        return spawn_explorer(&format!("shell:AppsFolder\\{}", aumid));
    }

    if !exe.is_file() {
        return Err(format!("找不到程序文件：{}", path));
    }

    if is_admin() {
        // Delegate to the running shell so the app starts non-elevated.
        spawn_explorer(path)
    } else {
        let mut cmd = Command::new(&exe);
        if let Some(dir) = exe.parent() {
            cmd.current_dir(dir);
        }
        cmd.spawn()
            .map(|_| ())
            .map_err(|e| format!("启动失败：{}", e))
    }
}

fn spawn_explorer(arg: &str) -> Result<(), String> {
    // explorer.exe always returns a non-zero exit code, so only the spawn
    // result is meaningful here.
    let windir = std::env::var("WINDIR").unwrap_or_else(|_| "C:\\Windows".into());
    let explorer = Path::new(&windir).join("explorer.exe");
    Command::new(explorer)
        .arg(arg)
        .spawn()
        .map(|_| ())
        .map_err(|e| format!("启动失败：{}", e))
}

/// For an exe inside `...\WindowsApps\<PackageFullName>\...`, build the
/// AppUserModelID (`<PackageFamilyName>!<ApplicationId>`).
fn resolve_packaged_aumid(exe: &Path) -> Option<String> {
    let comps: Vec<String> = exe
        .components()
        .map(|c| c.as_os_str().to_string_lossy().to_string())
        .collect();
    let idx = comps
        .iter()
        .position(|c| c.eq_ignore_ascii_case("WindowsApps"))?;
    let full_name = comps.get(idx + 1)?;

    // PackageFullName = Name_Version_Arch_ResourceId_PublisherId
    let parts: Vec<&str> = full_name.split('_').collect();
    if parts.len() < 5 {
        return None;
    }
    let family = format!("{}_{}", parts[0], parts[parts.len() - 1]);

    let mut pkg_root = PathBuf::new();
    for c in &comps[..=idx + 1] {
        pkg_root.push(c);
    }
    let rel_exe: String = comps[idx + 2..].join("\\");

    let manifest = std::fs::read_to_string(pkg_root.join("AppxManifest.xml")).ok()?;
    let app_id = find_application_id(&manifest, &rel_exe)?;
    Some(format!("{}!{}", family, app_id))
}

/// Parse `<Application Id="..." Executable="...">` entries from the manifest,
/// preferring the one whose Executable matches the given relative path.
fn find_application_id(manifest: &str, rel_exe: &str) -> Option<String> {
    let norm = |s: &str| s.replace('/', "\\").to_ascii_lowercase();
    let target = norm(rel_exe);
    let mut first: Option<String> = None;

    let mut rest = manifest;
    while let Some(pos) = rest.find("<Application ") {
        let tag_start = &rest[pos..];
        let end = tag_start.find('>').unwrap_or(tag_start.len());
        let tag = &tag_start[..end];
        rest = &tag_start[end..];

        let Some(id) = attr(tag, "Id") else { continue };
        if let Some(exe) = attr(tag, "Executable") {
            if norm(&exe) == target {
                return Some(id);
            }
        }
        if first.is_none() {
            first = Some(id);
        }
    }
    first
}

fn attr(tag: &str, name: &str) -> Option<String> {
    let key = format!(" {}=\"", name);
    let start = tag.find(&key)? + key.len();
    let end = tag[start..].find('"')? + start;
    Some(tag[start..end].to_string())
}
