//! Assembles the mount tree `build-pipeline.js` compiles: the designer's
//! current `form1.rs`/`form1.designer.rs` plus `rad-core` and
//! `rad-backend-web-min`'s own source, embedded here at *this* crate's
//! compile time (`include_str!`) so the JS side never has to fetch or
//! duplicate them. Only mechanical rewrites happen here — `crate::` and
//! `rad_core::` become `crate::rad_core::` because these files are no
//! longer their own crate root once mounted as modules of the generated
//! app's single crate (see `build-pipeline.js`'s doc comment for why
//! there's no `--extern`/`.rlib` option instead). Any real ABI mismatch
//! belongs in the real files, not a patch here.
//!
//! `rad-backend-web` (this designer's own backend, built normally via
//! `trunk`/`wasm-bindgen`) is never part of this bundle — a generated
//! app can't use it, since there's no way to run `wasm-bindgen`'s glue
//! generation inside the WASI sandbox the in-browser compiler runs in.

use js_sys::Map;
use wasm_bindgen::prelude::*;

macro_rules! rad_core_file {
    ($name:literal) => {
        include_str!(concat!("../../rad-core/src/", $name))
    };
}

const RAD_CORE_LIB: &str = rad_core_file!("lib.rs");
const RAD_CORE_BUTTON: &str = rad_core_file!("button.rs");
const RAD_CORE_CHECKBOX: &str = rad_core_file!("checkbox.rs");
const RAD_CORE_LABEL: &str = rad_core_file!("label.rs");
const RAD_CORE_PICTURE_BOX: &str = rad_core_file!("picture_box.rs");
const RAD_CORE_PROGRESS_BAR: &str = rad_core_file!("progress_bar.rs");
const RAD_CORE_TREE_VIEW: &str = rad_core_file!("tree_view.rs");
const RAD_CORE_RADIO_BUTTON: &str = rad_core_file!("radio_button.rs");
const RAD_CORE_TEXTBOX: &str = rad_core_file!("textbox.rs");
const RAD_CORE_TIMER: &str = rad_core_file!("timer.rs");
const RAD_CORE_WIDGET: &str = rad_core_file!("widget.rs");
const RAD_CORE_WINDOW: &str = rad_core_file!("window.rs");

const RAD_BACKEND_WEB_MIN: &str = include_str!("../../rad-backend-web-min/src/lib.rs");
const RUNTIME_JS: &str = include_str!("../../rad-backend-web-min/runtime.js");

/// A submodule of `rad_core` is no longer its own crate root once
/// mounted this way, so its `crate::` paths need one extra segment.
fn as_rad_core_submodule(src: &str) -> String {
    src.replace("crate::", "crate::rad_core::")
}

/// `rad_core` is a real Cargo dependency everywhere else this exact
/// source gets compiled (a normal `cargo build`, e.g. once downloaded
/// and hand-edited), so `form1.rs`/`form1.designer.rs` correctly say
/// `use rad_core::Window`. In THIS mount, `rad_core` is a sibling module
/// of the crate root instead — the same mechanical rewrite
/// `rad_backend_web_min` already needs applies here too, or `rad_core`
/// fails to resolve (edition 2018+ doesn't implicitly root bare `use`
/// paths at the crate root the way `crate::rad_core` does explicitly).
fn as_mounted_rad_core_user(src: &str) -> String {
    src.replace("rad_core::", "crate::rad_core::")
}

/// The mount tree every build needs, plus `main_rs` (the generated app's
/// entry point — see `start()`'s `generate_main_source`) at `/main.rs`.
#[wasm_bindgen]
pub fn build_mount_files(main_rs: &str, form1_rs: &str, form1_designer_rs: &str) -> Map {
    let map = Map::new();
    map.set(&"/rad_core.rs".into(), &RAD_CORE_LIB.into());
    map.set(&"/rad_core/button.rs".into(), &as_rad_core_submodule(RAD_CORE_BUTTON).into());
    map.set(&"/rad_core/checkbox.rs".into(), &as_rad_core_submodule(RAD_CORE_CHECKBOX).into());
    map.set(&"/rad_core/label.rs".into(), &as_rad_core_submodule(RAD_CORE_LABEL).into());
    map.set(&"/rad_core/picture_box.rs".into(), &as_rad_core_submodule(RAD_CORE_PICTURE_BOX).into());
    map.set(&"/rad_core/progress_bar.rs".into(), &as_rad_core_submodule(RAD_CORE_PROGRESS_BAR).into());
    map.set(&"/rad_core/tree_view.rs".into(), &as_rad_core_submodule(RAD_CORE_TREE_VIEW).into());
    map.set(&"/rad_core/radio_button.rs".into(), &as_rad_core_submodule(RAD_CORE_RADIO_BUTTON).into());
    map.set(&"/rad_core/textbox.rs".into(), &as_rad_core_submodule(RAD_CORE_TEXTBOX).into());
    map.set(&"/rad_core/timer.rs".into(), &as_rad_core_submodule(RAD_CORE_TIMER).into());
    map.set(&"/rad_core/widget.rs".into(), &as_rad_core_submodule(RAD_CORE_WIDGET).into());
    map.set(&"/rad_core/window.rs".into(), &as_rad_core_submodule(RAD_CORE_WINDOW).into());
    map.set(
        &"/rad_backend_web_min.rs".into(),
        &RAD_BACKEND_WEB_MIN.replace("rad_core::", "crate::rad_core::").into(),
    );
    map.set(&"/main.rs".into(), &main_rs.into());
    map.set(&"/form1.rs".into(), &as_mounted_rad_core_user(form1_rs).into());
    map.set(&"/form1.designer.rs".into(), &as_mounted_rad_core_user(form1_designer_rs).into());
    map
}

/// The generated app's own copy of `runtime.js`, embedded the same way —
/// so the standalone download doesn't depend on fetching anything.
#[wasm_bindgen]
pub fn runtime_js() -> String {
    RUNTIME_JS.to_string()
}

const BASE64_ALPHABET: &[u8; 64] =
    b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

/// No crate for this, the same way `rad-core` itself has none — this is
/// the one place the downloaded app's `.wasm` bytes need to become text
/// to sit inside an HTML `<script>`.
fn base64_encode(bytes: &[u8]) -> String {
    let mut out = String::with_capacity((bytes.len() + 2) / 3 * 4);
    for chunk in bytes.chunks(3) {
        let b0 = chunk[0];
        let b1 = *chunk.get(1).unwrap_or(&0);
        let b2 = *chunk.get(2).unwrap_or(&0);
        out.push(BASE64_ALPHABET[(b0 >> 2) as usize] as char);
        out.push(BASE64_ALPHABET[(((b0 & 0x03) << 4) | (b1 >> 4)) as usize] as char);
        out.push(if chunk.len() > 1 {
            BASE64_ALPHABET[(((b1 & 0x0f) << 2) | (b2 >> 6)) as usize] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            BASE64_ALPHABET[(b2 & 0x3f) as usize] as char
        } else {
            '='
        });
    }
    out
}

/// A single downloadable, double-click-runnable file: the compiled app's
/// `.wasm` inlined as base64 (so opening it via `file://` needs no fetch,
/// which would be blocked without a server) alongside its own copy of
/// `runtime.js` — no CDN, no build toolchain, nothing left to download
/// ever again once this file exists.
pub fn generate_standalone_html(wasm_bytes: &[u8], runtime_js: &str) -> String {
    let wasm_base64 = base64_encode(wasm_bytes);
    // `export` has no meaning inlined into a page's own module script.
    let runtime_js = runtime_js.replacen("export async function runApp", "async function runApp", 1);
    format!(
        "<!doctype html>\n\
         <html>\n\
         <head><meta charset=\"utf-8\" /><title>Form1</title></head>\n\
         <body>\n\
         <div id=\"rad-stage\"></div>\n\
         <script type=\"module\">\n\
         {runtime_js}\n\
         const bytes = Uint8Array.from(atob(\"{wasm_base64}\"), (c) => c.charCodeAt(0));\n\
         runApp(bytes, document.getElementById(\"rad-stage\"));\n\
         </script>\n\
         </body>\n\
         </html>\n"
    )
}

/// The entry point a build needs: build the window on `rad-backend-web-min`
/// instead of `rad-backend-web` (the only backend change a generated app
/// ever needs — `form1.rs`/`form1.designer.rs` only ever name `rad_core`
/// types, never a specific backend), hand it to `form1::build`, then leak
/// everything — same reasoning `rad-demo`'s own entry point uses.
pub fn generate_main_source(width: u32, height: u32) -> String {
    format!(
        "mod rad_core;\n\
         mod rad_backend_web_min;\n\
         #[path = \"form1.rs\"]\n\
         mod form1;\n\n\
         use rad_backend_web_min::WebMinBackend;\n\n\
         #[unsafe(no_mangle)]\n\
         pub extern \"C\" fn main() {{\n\
         \x20   let backend = WebMinBackend;\n\
         \x20   let window = rad_core::Window::new(&backend, \"Form1\", {width}, {height});\n\
         \x20   let form = form1::build(&window);\n\
         \x20   std::mem::forget(form);\n\
         \x20   std::mem::forget(window);\n\
         }}\n"
    )
}
