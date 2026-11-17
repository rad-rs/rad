//! Finds the component files in `src/components/` and renders the Rust that
//! declares them. Shared (by `#[path]`) between this crate's `build.rs` and
//! the designer's, so the two never disagree about what a component is.
//!
//! A component is any `*.rs` file in `src/components/`. The module is named
//! after the file; every item declared `pub` at column 0 (`pub struct`,
//! `pub trait`, `pub type`, ...) is re-exported from `rad_core`; and the file
//! must define `pub(crate) const INFO: ComponentInfo`.

use std::fs;
use std::path::{Path, PathBuf};

pub struct Component {
    /// File stem, also the module name (`track_bar`).
    pub module: String,
    pub path: PathBuf,
    /// Names of the file's top-level public items.
    pub exports: Vec<String>,
}

/// The components found in `dir`, sorted by module name.
pub fn scan(dir: &Path) -> Vec<Component> {
    let mut found = Vec::new();
    for entry in fs::read_dir(dir).unwrap_or_else(|e| panic!("cannot read {}: {e}", dir.display())) {
        let path = entry.expect("directory entry").path();
        if path.extension().and_then(|e| e.to_str()) != Some("rs") {
            continue;
        }
        let module = path.file_stem().and_then(|s| s.to_str()).expect("utf-8 file name").to_string();
        let source = fs::read_to_string(&path).unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()));
        let mut exports = Vec::new();
        for line in source.lines() {
            for keyword in ["struct", "enum", "trait", "type", "fn", "const", "static"] {
                if let Some(rest) = line.strip_prefix("pub ").and_then(|r| r.strip_prefix(keyword)) {
                    if let Some(rest) = rest.strip_prefix(' ') {
                        let name: String =
                            rest.chars().take_while(|c| c.is_alphanumeric() || *c == '_').collect();
                        if !name.is_empty() {
                            exports.push(name);
                        }
                    }
                }
            }
        }
        found.push(Component { module, path, exports });
    }
    found.sort_by(|a, b| a.module.cmp(&b.module));
    found
}

/// The body of `rad_core`'s `components` module. With `absolute_paths` each
/// file is named by its full path (for Cargo, where the module lives in an
/// `include!`d file); without, by plain `pub mod x;` (for the designer's mount
/// tree, which puts the files next to where that module looks for them).
pub fn module_text(components: &[Component], absolute_paths: bool) -> String {
    let mut out = String::from("// GENERATED from the files in src/components/ by components_scan.rs.\n");
    for c in components {
        if absolute_paths {
            out.push_str(&format!("#[path = {:?}]\n", c.path.to_string_lossy().replace('\\', "/")));
        }
        out.push_str(&format!("pub mod {};\n", c.module));
        if !c.exports.is_empty() {
            out.push_str(&format!("pub use {}::{{{}}};\n", c.module, c.exports.join(", ")));
        }
    }
    out.push_str("\n/// Every component, in file-name order.\n");
    out.push_str("pub static COMPONENTS: &[&super::component::ComponentInfo] = &[\n");
    for c in components {
        out.push_str(&format!("    &{}::INFO,\n", c.module));
    }
    out.push_str("];\n");
    out
}
