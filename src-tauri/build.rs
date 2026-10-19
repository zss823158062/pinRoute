fn main() {
    // WinDivert.dll / WinDivert64.sys are embedded via include_bytes! and extracted
    // at runtime, so there is no link-time or packaging dependency on them here.
    println!("cargo:rerun-if-changed=windivert_bin/WinDivert.dll");
    println!("cargo:rerun-if-changed=windivert_bin/WinDivert64.sys");

    tauri_build::build();
}
