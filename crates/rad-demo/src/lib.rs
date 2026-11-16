//! Entry point for the browser preview. Mirrors a WinForms app's `Main`:
//! create the form's window, let the designer-generated code build its
//! controls, then run.

mod form1;

use rad_backend_web::WebBackend;
use rad_core::Window;
use wasm_bindgen::prelude::*;

#[wasm_bindgen(start)]
pub fn start() {
    let dom_window = web_sys::window().expect("no window");
    let document = dom_window.document().expect("no document");
    let root = document
        .get_element_by_id("rad-stage")
        .unwrap_or_else(|| document.body().expect("no body").into());

    let backend = WebBackend::new(root);

    let window = Window::new(&backend, "All controls demo", 460, 410);
    window.on_resize(|w, h| {
        web_sys::console::log_1(&format!("resized to {w}x{h}").into());
    });

    let controls = form1::build(&window);

    // Window and controls must outlive the demo; leaking is fine for this
    // throwaway entry point.
    std::mem::forget(window);
    std::mem::forget(controls);
}
