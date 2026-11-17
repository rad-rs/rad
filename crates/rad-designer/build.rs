//! Embeds `rad-core`'s sources for the in-browser build pipelines, and works out
//! which component files exist so nothing here names a component.
//!
//! Writes `$OUT_DIR/rad_core_mount.rs`, defining:
//! - `RAD_CORE_FILES`: every `.rs` file under `rad-core/src` as `(path relative to
//!   src, include_str!)`, so a new file is picked up without editing a list;
//! - `COMPONENTS_MODULE`: the body of rad-core's `components` module for the
//!   mount tree (plain `pub mod x;` lines, since the files sit next to where an
//!   inline `mod components` looks for them), from the same scan rad-core's own
//!   `build.rs` uses.

#[path = "../rad-core/components_scan.rs"]
mod scan;

use std::fs;
use std::path::{Path, PathBuf};

fn collect(dir: &Path, base: &Path, out: &mut Vec<(String, PathBuf)>) {
    for entry in fs::read_dir(dir).unwrap_or_else(|e| panic!("cannot read {}: {e}", dir.display())) {
        let path = entry.expect("directory entry").path();
        if path.is_dir() {
            collect(&path, base, out);
        } else if path.extension().and_then(|e| e.to_str()) == Some("rs") {
            let relative = path.strip_prefix(base).expect("under base").to_string_lossy().replace('\\', "/");
            out.push((relative, path));
        }
    }
}

fn main() {
    let manifest = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR"));
    let core = manifest.join("..").join("rad-core").canonicalize().expect("rad-core directory");
    let src = core.join("src");
    // Scans the whole tree for changes, so adding or removing a file re-runs this.
    println!("cargo:rerun-if-changed={}", src.display());
    println!("cargo:rerun-if-changed={}", core.join("components_scan.rs").display());

    let mut files = Vec::new();
    collect(&src, &src, &mut files);
    files.sort();

    let mut out = String::from("pub const RAD_CORE_FILES: &[(&str, &str)] = &[\n");
    for (relative, path) in &files {
        out.push_str(&format!(
            "    ({relative:?}, include_str!({:?})),\n",
            path.to_string_lossy().replace('\\', "/").trim_start_matches("//?/")
        ));
    }
    out.push_str("];\n\n");

    let components = scan::scan(&src.join("components"));
    out.push_str(&format!("pub const COMPONENTS_MODULE: &str = {:?};\n", scan::module_text(&components, false)));

    let out_path = PathBuf::from(std::env::var("OUT_DIR").expect("OUT_DIR")).join("rad_core_mount.rs");
    fs::write(out_path, out).expect("write rad_core_mount.rs");
}
