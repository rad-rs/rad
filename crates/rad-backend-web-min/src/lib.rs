//! Minimal web backend for apps built through the in-browser compile
//! pipeline (see `.research/spike`): no `wasm-bindgen`, no `web-sys`.
//! That toolchain's glue-generation step has no way to run inside the
//! WASI sandbox the in-browser `rustc` itself runs in, and the pipeline
//! also can't produce `.rlib`s (no working archiver), so this crate is
//! built as plain source `mod`s alongside the user's generated code in
//! one crate, not linked in as a dependency.
//!
//! The DOM boundary is a small, fixed set of `extern "C"` imports this
//! crate calls — created elements, position/size, text, checked state,
//! click/resize dispatch — matched by a hand-written JS shim (see
//! `runtime.js`) that implements them against a small per-page element
//! registry. Strings cross the boundary as `(ptr, len)` into this
//! module's own linear memory; reading one back is a `len` call followed
//! by an `into` call that writes UTF-8 bytes into a buffer this side
//! allocates, since there's no glue code to do that marshalling for us.
//!
//! `rad-backend-web` (the designer's own backend, built normally via
//! `trunk`/`wasm-bindgen`) is unaffected by any of this — this crate only
//! exists for code compiled through the no-wasm-bindgen pipeline.
//!
//! Every control's own DOM calls live with its component in `rad-core`; this
//! crate only creates windows.

#![cfg(target_arch = "wasm32")]

use rad_core::platform::web_min::{
    ffi, read_remote_string, store_click_callback, store_resize_callback, Parent,
};
use rad_core::{Backend, Host, WindowHandle};

/// Creates top-level windows. There's exactly one real "page" in a
/// compiled app, but a `Backend` per `rad-core`'s model regardless —
/// `create_window` just asks the JS runtime for a new window id.
pub struct WebMinBackend;

impl Backend for WebMinBackend {
    fn create_window(&self, title: &str, width: u32, height: u32) -> Box<dyn WindowHandle> {
        let id = unsafe { ffi::rad_create_window(title.as_ptr(), title.len() as u32, width, height) };
        Box::new(WebMinWindow { id })
    }
}

pub struct WebMinWindow {
    id: u32,
}

impl Host for WebMinWindow {
    fn parent(&self) -> Parent {
        Parent { window_id: self.id }
    }
}

impl WindowHandle for WebMinWindow {
    fn set_title(&self, title: &str) {
        unsafe { ffi::rad_set_text(self.id, title.as_ptr(), title.len() as u32) };
    }
    fn title(&self) -> String {
        let len = unsafe { ffi::rad_get_text_len(self.id) };
        read_remote_string(len, |ptr| unsafe { ffi::rad_get_text_into(self.id, ptr) })
    }
    fn set_size(&self, width: u32, height: u32) {
        unsafe {
            ffi::rad_set_width(self.id, width);
            ffi::rad_set_height(self.id, height);
        }
    }
    fn size(&self) -> (u32, u32) {
        unsafe { (ffi::rad_get_width(self.id), ffi::rad_get_height(self.id)) }
    }
    fn set_on_resize(&self, callback: Box<dyn FnMut(u32, u32)>) {
        let callback_id = store_resize_callback(callback);
        unsafe { ffi::rad_register_resize(self.id, callback_id) };
    }
    fn set_on_click(&self, callback: Box<dyn FnMut()>) {
        let callback_id = store_click_callback(callback);
        unsafe { ffi::rad_register_click(self.id, callback_id) };
    }
    fn set_default_icon(&self) {
        unsafe { ffi::rad_set_default_icon(self.id) };
    }
}
