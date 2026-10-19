use std::env;
use std::fs;
use std::path::PathBuf;

fn main() {
    let manifest_dir = env::var("CARGO_MANIFEST_DIR").unwrap();
    let bin_dir = PathBuf::from(&manifest_dir).join("windivert_bin");

    // Link WinDivert.lib
    println!("cargo:rustc-link-search=native={}", bin_dir.display());
    println!("cargo:rustc-link-lib=dylib=WinDivert");

    // Copy WinDivert.dll and WinDivert64.sys to target output directory
    let out_dir = env::var("OUT_DIR").unwrap();
    let target_dir = PathBuf::from(&out_dir)
        .ancestors()
        .nth(3)
        .map(|p| p.to_path_buf());

    if let Some(target) = target_dir {
        for file in &["WinDivert.dll", "WinDivert64.sys"] {
            let src = bin_dir.join(file);
            let dst = target.join(file);
            if src.exists() {
                let _ = fs::copy(&src, &dst);
            }
        }
    }

    tauri_build::build();
}
