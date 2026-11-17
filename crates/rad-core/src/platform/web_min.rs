//! Browser plumbing with no `wasm-bindgen`/`web-sys`, shared by every component's
//! `#[cfg(all(target_arch = "wasm32", feature = "web-min"))]` implementation. That
//! toolchain's glue-generation step has no way to run inside the WASI sandbox the
//! in-browser `rustc` itself runs in, and the pipeline also can't produce
//! `.rlib`s (no working archiver), so this is built as plain source `mod`s
//! alongside the user's generated code in one crate.
//!
//! The DOM boundary is a small, fixed set of `extern "C"` imports this module
//! calls, matched by a hand-written JS shim (`rad-backend-web-min/runtime.js`)
//! that implements them against a small per-page element registry. Strings cross
//! the boundary as `(ptr, len)` into this module's own linear memory; reading one
//! back is a `len` call followed by an `into` call that writes UTF-8 bytes into a
//! buffer this side allocates, since there's no glue code to do that marshalling.
//!
//! A control's DOM is described by its component's declarative `Preview`, sent as
//! a small `key=value` spec the runtime builds an element from, so a new component
//! needs no change to this module or the runtime.

use crate::{Color, Preview};
use std::cell::{Cell, RefCell};

pub mod ffi {
    #[link(wasm_import_module = "env")]
    unsafe extern "C" {
        pub fn rad_create_window(title_ptr: *const u8, title_len: u32, width: u32, height: u32) -> u32;
        /// `spec` is a `Preview::to_spec` string describing the element.
        pub fn rad_create_element(
            parent_id: u32,
            spec_ptr: *const u8,
            spec_len: u32,
            text_ptr: *const u8,
            text_len: u32,
            group_ptr: *const u8,
            group_len: u32,
        ) -> u32;

        pub fn rad_set_pos(id: u32, x: i32, y: i32);
        pub fn rad_get_pos_x(id: u32) -> i32;
        pub fn rad_get_pos_y(id: u32) -> i32;
        pub fn rad_set_width(id: u32, width: u32);
        pub fn rad_get_width(id: u32) -> u32;
        pub fn rad_set_height(id: u32, height: u32);
        pub fn rad_get_height(id: u32) -> u32;

        pub fn rad_set_text(id: u32, ptr: *const u8, len: u32);
        pub fn rad_get_text_len(id: u32) -> u32;
        pub fn rad_get_text_into(id: u32, ptr: *mut u8);

        pub fn rad_set_checked(id: u32, checked: u32);
        pub fn rad_get_checked(id: u32) -> u32;

        pub fn rad_register_click(id: u32, callback_id: u32);
        pub fn rad_register_resize(id: u32, callback_id: u32);
        pub fn rad_set_default_icon(id: u32);

        /// `has_color` is `0`/`1`, not an `Option` — the FFI boundary has
        /// no way to send a null-vs-present flag otherwise.
        pub fn rad_set_fore_color(id: u32, has_color: u32, r: u8, g: u8, b: u8);
        pub fn rad_set_back_color(id: u32, has_color: u32, r: u8, g: u8, b: u8);

        pub fn rad_set_value(id: u32, value: u32);
        pub fn rad_get_value(id: u32) -> u32;

        /// Items cross as one `(ptr, len)` string, one row per line,
        /// `"<depth>\t<text>"` — simplest way to send a whole list in
        /// one call without a separate FFI shape per field.
        pub fn rad_set_tree_items(id: u32, ptr: *const u8, len: u32);

        /// Timers have no DOM node: `rad_create_timer` just reserves an id
        /// in the JS runtime's own timer table.
        pub fn rad_create_timer() -> u32;
        /// (Re)starts the timer's `setInterval` when `enabled` is nonzero,
        /// otherwise stops it. Ticks come back through `rad_dispatch_tick`.
        pub fn rad_timer_set(id: u32, interval_ms: u32, enabled: u32);
        pub fn rad_timer_destroy(id: u32);

        /// A signed value, unlike `rad_set_value`: a trackbar's range can be
        /// negative.
        pub fn rad_set_int_value(id: u32, value: i32);
        pub fn rad_get_int_value(id: u32) -> i32;
        pub fn rad_set_range(id: u32, minimum: i32, maximum: i32);

        /// Items cross as one `(ptr, len)` string, one item per line.
        pub fn rad_set_list_items(id: u32, ptr: *const u8, len: u32);
        /// `-1` means no selection.
        pub fn rad_set_selected_index(id: u32, index: i32);
        pub fn rad_get_selected_index(id: u32) -> i32;
        /// Fires `rad_dispatch_change` when the user moves a trackbar or picks
        /// a different list item.
        pub fn rad_register_change(id: u32, callback_id: u32);
    }
}

/// What a component needs from its host to create a control: the window's id.
#[derive(Clone, Copy)]
pub struct Parent {
    pub window_id: u32,
}

thread_local! {
    static CHANGE_CALLBACKS: RefCell<Vec<Option<Box<dyn FnMut()>>>> = RefCell::new(Vec::new());
    static CLICK_CALLBACKS: RefCell<Vec<Option<Box<dyn FnMut()>>>> = RefCell::new(Vec::new());
    static RESIZE_CALLBACKS: RefCell<Vec<Option<Box<dyn FnMut(u32, u32)>>>> = RefCell::new(Vec::new());
}

pub fn store_click_callback(callback: Box<dyn FnMut()>) -> u32 {
    CLICK_CALLBACKS.with(|cell| {
        let mut callbacks = cell.borrow_mut();
        callbacks.push(Some(callback));
        (callbacks.len() - 1) as u32
    })
}

pub fn store_resize_callback(callback: Box<dyn FnMut(u32, u32)>) -> u32 {
    RESIZE_CALLBACKS.with(|cell| {
        let mut callbacks = cell.borrow_mut();
        callbacks.push(Some(callback));
        (callbacks.len() - 1) as u32
    })
}

fn store_change_callback(callback: Box<dyn FnMut()>) -> u32 {
    CHANGE_CALLBACKS.with(|cell| {
        let mut callbacks = cell.borrow_mut();
        callbacks.push(Some(callback));
        (callbacks.len() - 1) as u32
    })
}

/// Called by the JS runtime when the user moves a trackbar or picks a
/// different item in a combo or list box. The callback is taken out while it
/// runs, so it may register further handlers.
#[unsafe(no_mangle)]
pub extern "C" fn rad_dispatch_change(callback_id: u32) {
    let taken = CHANGE_CALLBACKS.with(|cell| cell.borrow_mut().get_mut(callback_id as usize).and_then(Option::take));
    if let Some(mut callback) = taken {
        callback();
        CHANGE_CALLBACKS.with(|cell| {
            if let Some(slot) = cell.borrow_mut().get_mut(callback_id as usize) {
                if slot.is_none() {
                    *slot = Some(callback);
                }
            }
        });
    }
}

/// Called by the JS runtime when an element with a registered click
/// handler is clicked.
#[unsafe(no_mangle)]
pub extern "C" fn rad_dispatch_click(callback_id: u32) {
    CLICK_CALLBACKS.with(|cell| {
        if let Some(Some(callback)) = cell.borrow_mut().get_mut(callback_id as usize) {
            callback();
        }
    });
}

/// Called by the JS runtime when a window with a registered resize
/// handler is resized.
#[unsafe(no_mangle)]
pub extern "C" fn rad_dispatch_resize(callback_id: u32, width: u32, height: u32) {
    RESIZE_CALLBACKS.with(|cell| {
        if let Some(Some(callback)) = cell.borrow_mut().get_mut(callback_id as usize) {
            callback(width, height);
        }
    });
}

/// Lets the JS runtime allocate a buffer in this module's own memory
/// (e.g. to write a UTF-8 string into before handing control back), since
/// there's no `wasm-bindgen`-generated glue to do that for it.
#[unsafe(no_mangle)]
pub extern "C" fn rad_alloc(size: u32) -> *mut u8 {
    let mut buf = vec![0u8; size as usize];
    let ptr = buf.as_mut_ptr();
    std::mem::forget(buf);
    ptr
}

#[unsafe(no_mangle)]
pub extern "C" fn rad_dealloc(ptr: *mut u8, size: u32) {
    unsafe {
        drop(Vec::from_raw_parts(ptr, size as usize, size as usize));
    }
}

/// Reads a string the JS runtime exposes through a `(len, into)` pair of
/// imports — the mirror of how a string is sent the other way as a
/// `(ptr, len)` pair, since nothing here can marshal a `String` for us.
pub fn read_remote_string(len: u32, into: impl FnOnce(*mut u8)) -> String {
    let mut buf = vec![0u8; len as usize];
    into(buf.as_mut_ptr());
    String::from_utf8(buf).unwrap_or_default()
}

/// A handle to one DOM element, identified by the id the JS runtime
/// assigned it at creation. Every control type is just this plus whichever
/// marker trait it needs to implement.
pub struct ElementHandle {
    pub id: u32,
    // Cached locally rather than round-tripped through JS for the
    // getters — there's no `rad_get_fore_color`/`rad_get_back_color`
    // import, so this is the only record of what was last set.
    fore_color: Cell<Option<Color>>,
    back_color: Cell<Option<Color>>,
}

impl ElementHandle {
    /// Creates the element `preview` describes in `parent`'s window. `text` is
    /// its initial text; `group` names a radio button's group.
    pub fn create(parent: &Parent, preview: &Preview, text: &str, group: &str) -> Self {
        let spec = preview.to_spec();
        let id = unsafe {
            ffi::rad_create_element(
                parent.window_id,
                spec.as_ptr(),
                spec.len() as u32,
                text.as_ptr(),
                text.len() as u32,
                group.as_ptr(),
                group.len() as u32,
            )
        };
        ElementHandle { id, fore_color: Cell::new(None), back_color: Cell::new(None) }
    }

    pub fn set_text(&self, text: &str) {
        unsafe { ffi::rad_set_text(self.id, text.as_ptr(), text.len() as u32) };
    }

    pub fn text(&self) -> String {
        let len = unsafe { ffi::rad_get_text_len(self.id) };
        read_remote_string(len, |ptr| unsafe { ffi::rad_get_text_into(self.id, ptr) })
    }

    pub fn set_checked(&self, checked: bool) {
        unsafe { ffi::rad_set_checked(self.id, checked as u32) };
    }

    pub fn checked(&self) -> bool {
        unsafe { ffi::rad_get_checked(self.id) != 0 }
    }
}

impl crate::WidgetHandle for ElementHandle {
    fn set_pos(&self, x: i32, y: i32) {
        unsafe { ffi::rad_set_pos(self.id, x, y) };
    }
    fn pos(&self) -> (i32, i32) {
        unsafe { (ffi::rad_get_pos_x(self.id), ffi::rad_get_pos_y(self.id)) }
    }
    fn set_width(&self, width: u32) {
        unsafe { ffi::rad_set_width(self.id, width) };
    }
    fn width(&self) -> u32 {
        unsafe { ffi::rad_get_width(self.id) }
    }
    fn set_height(&self, height: u32) {
        unsafe { ffi::rad_set_height(self.id, height) };
    }
    fn height(&self) -> u32 {
        unsafe { ffi::rad_get_height(self.id) }
    }
    fn set_on_click(&self, callback: Box<dyn FnMut()>) {
        let callback_id = store_click_callback(callback);
        unsafe { ffi::rad_register_click(self.id, callback_id) };
    }
    fn set_fore_color(&self, color: Option<Color>) {
        self.fore_color.set(color);
        let c = color.unwrap_or(Color::rgb(0, 0, 0));
        unsafe { ffi::rad_set_fore_color(self.id, color.is_some() as u32, c.r, c.g, c.b) };
    }
    fn fore_color(&self) -> Option<Color> {
        self.fore_color.get()
    }
    fn set_back_color(&self, color: Option<Color>) {
        self.back_color.set(color);
        let c = color.unwrap_or(Color::rgb(0, 0, 0));
        unsafe { ffi::rad_set_back_color(self.id, color.is_some() as u32, c.r, c.g, c.b) };
    }
    fn back_color(&self) -> Option<Color> {
        self.back_color.get()
    }
}

/// `WidgetHandle` for a wrapper `$ty` holding an `ElementHandle` in `$field`:
/// every call is forwarded to it.
macro_rules! impl_min_widget {
    ($ty:ty, $field:ident) => {
        impl crate::WidgetHandle for $ty {
            fn set_pos(&self, x: i32, y: i32) { self.$field.set_pos(x, y) }
            fn pos(&self) -> (i32, i32) { self.$field.pos() }
            fn set_width(&self, w: u32) { self.$field.set_width(w) }
            fn width(&self) -> u32 { self.$field.width() }
            fn set_height(&self, h: u32) { self.$field.set_height(h) }
            fn height(&self) -> u32 { self.$field.height() }
            fn set_on_click(&self, callback: Box<dyn FnMut()>) { self.$field.set_on_click(callback) }
            fn set_fore_color(&self, color: Option<crate::Color>) { self.$field.set_fore_color(color) }
            fn fore_color(&self) -> Option<crate::Color> { self.$field.fore_color() }
            fn set_back_color(&self, color: Option<crate::Color>) { self.$field.set_back_color(color) }
            fn back_color(&self) -> Option<crate::Color> { self.$field.back_color() }
        }
    };
}

pub(crate) use impl_min_widget;

pub fn register_change(id: u32, callback: Box<dyn FnMut()>) {
    let callback_id = store_change_callback(callback);
    unsafe { ffi::rad_register_change(id, callback_id) };
}
