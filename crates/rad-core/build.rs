//! Generates the list of components from the files in `src/components/`.

#[path = "components_scan.rs"]
mod scan;

use std::path::PathBuf;

fn main() {
    emit_platform_cfg();
    let manifest = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR"));
    let dir = manifest.join("src").join("components");
    // Re-scan when a component file is added or removed (the directory's own
    // mtime changes) as well as when one is edited.
    println!("cargo:rerun-if-changed={}", dir.display());
    println!("cargo:rerun-if-changed=components_scan.rs");
    let components = scan::scan(&dir);
    let out = PathBuf::from(std::env::var("OUT_DIR").expect("OUT_DIR")).join("components.rs");
    std::fs::write(out, scan::module_text(&components, true)).expect("write components.rs");
}

/// Picks the platform components are compiled for. `web` wins over `web-min` if
/// both features are on (Cargo unifies features across a workspace build).
fn emit_platform_cfg() {
    for cfg in ["rad_win32", "rad_web", "rad_web_min", "rad_macos"] {
        println!("cargo::rustc-check-cfg=cfg({cfg})");
    }
    let os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    let arch = std::env::var("CARGO_CFG_TARGET_ARCH").unwrap_or_default();
    let feature = |name: &str| std::env::var_os(format!("CARGO_FEATURE_{name}")).is_some();
    if os == "windows" {
        println!("cargo:rustc-cfg=rad_win32");
    } else if arch == "wasm32" && feature("WEB") {
        println!("cargo:rustc-cfg=rad_web");
    } else if arch == "wasm32" && feature("WEB_MIN") {
        println!("cargo:rustc-cfg=rad_web_min");
    }
}
