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

use rad_core::{
    Backend, ButtonHandle, CheckBoxHandle, Color, Host, LabelHandle, PictureBoxHandle,
    ProgressBarHandle, RadioButtonHandle, TextBoxHandle, TreeItem, TreeViewHandle, WidgetHandle,
    WindowHandle,
};
use std::cell::{Cell, RefCell};

mod ffi {
    #[link(wasm_import_module = "env")]
    unsafe extern "C" {
        pub fn rad_create_window(title_ptr: *const u8, title_len: u32, width: u32, height: u32) -> u32;
        pub fn rad_create_element(
            parent_id: u32,
            kind: u32,
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
    }
}

/// Control kinds, matched exactly on the JS side — see `runtime.js`.
mod kind {
    pub const BUTTON: u32 = 0;
    pub const LABEL: u32 = 1;
    pub const CHECKBOX: u32 = 2;
    pub const TEXTBOX: u32 = 3;
    pub const RADIO_BUTTON: u32 = 4;
    pub const PROGRESS_BAR: u32 = 5;
    pub const PICTURE_BOX: u32 = 6;
    pub const TREE_VIEW: u32 = 7;
}

thread_local! {
    static CLICK_CALLBACKS: RefCell<Vec<Option<Box<dyn FnMut()>>>> = RefCell::new(Vec::new());
    static RESIZE_CALLBACKS: RefCell<Vec<Option<Box<dyn FnMut(u32, u32)>>>> = RefCell::new(Vec::new());
}

fn store_click_callback(callback: Box<dyn FnMut()>) -> u32 {
    CLICK_CALLBACKS.with(|cell| {
        let mut callbacks = cell.borrow_mut();
        callbacks.push(Some(callback));
        (callbacks.len() - 1) as u32
    })
}

fn store_resize_callback(callback: Box<dyn FnMut(u32, u32)>) -> u32 {
    RESIZE_CALLBACKS.with(|cell| {
        let mut callbacks = cell.borrow_mut();
        callbacks.push(Some(callback));
        (callbacks.len() - 1) as u32
    })
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
fn read_remote_string(len: u32, into: impl FnOnce(*mut u8)) -> String {
    let mut buf = vec![0u8; len as usize];
    into(buf.as_mut_ptr());
    String::from_utf8(buf).unwrap_or_default()
}

/// A handle to one DOM element, identified by the id the JS runtime
/// assigned it at creation. Every control type in this crate is just
/// this plus whichever marker trait it needs to implement.
struct ElementHandle {
    id: u32,
    // Cached locally rather than round-tripped through JS for the
    // getters — there's no `rad_get_fore_color`/`rad_get_back_color`
    // import, so this is the only record of what was last set.
    fore_color: Cell<Option<Color>>,
    back_color: Cell<Option<Color>>,
}

impl WidgetHandle for ElementHandle {
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

impl ElementHandle {
    fn set_text(&self, text: &str) {
        unsafe { ffi::rad_set_text(self.id, text.as_ptr(), text.len() as u32) };
    }

    fn text(&self) -> String {
        let len = unsafe { ffi::rad_get_text_len(self.id) };
        read_remote_string(len, |ptr| unsafe { ffi::rad_get_text_into(self.id, ptr) })
    }

    fn set_checked(&self, checked: bool) {
        unsafe { ffi::rad_set_checked(self.id, checked as u32) };
    }

    fn checked(&self) -> bool {
        unsafe { ffi::rad_get_checked(self.id) != 0 }
    }
}

struct MinButtonHandle(ElementHandle);
impl WidgetHandle for MinButtonHandle {
    fn set_pos(&self, x: i32, y: i32) { self.0.set_pos(x, y) }
    fn pos(&self) -> (i32, i32) { self.0.pos() }
    fn set_width(&self, w: u32) { self.0.set_width(w) }
    fn width(&self) -> u32 { self.0.width() }
    fn set_height(&self, h: u32) { self.0.set_height(h) }
    fn height(&self) -> u32 { self.0.height() }
    fn set_on_click(&self, callback: Box<dyn FnMut()>) { self.0.set_on_click(callback) }
    fn set_fore_color(&self, color: Option<Color>) { self.0.set_fore_color(color) }
    fn fore_color(&self) -> Option<Color> { self.0.fore_color() }
    fn set_back_color(&self, color: Option<Color>) { self.0.set_back_color(color) }
    fn back_color(&self) -> Option<Color> { self.0.back_color() }
}
impl ButtonHandle for MinButtonHandle {
    fn set_text(&self, text: &str) { self.0.set_text(text) }
    fn text(&self) -> String { self.0.text() }
}

struct MinLabelHandle(ElementHandle);
impl WidgetHandle for MinLabelHandle {
    fn set_pos(&self, x: i32, y: i32) { self.0.set_pos(x, y) }
    fn pos(&self) -> (i32, i32) { self.0.pos() }
    fn set_width(&self, w: u32) { self.0.set_width(w) }
    fn width(&self) -> u32 { self.0.width() }
    fn set_height(&self, h: u32) { self.0.set_height(h) }
    fn height(&self) -> u32 { self.0.height() }
    fn set_on_click(&self, callback: Box<dyn FnMut()>) { self.0.set_on_click(callback) }
    fn set_fore_color(&self, color: Option<Color>) { self.0.set_fore_color(color) }
    fn fore_color(&self) -> Option<Color> { self.0.fore_color() }
    fn set_back_color(&self, color: Option<Color>) { self.0.set_back_color(color) }
    fn back_color(&self) -> Option<Color> { self.0.back_color() }
}
impl LabelHandle for MinLabelHandle {
    fn set_text(&self, text: &str) { self.0.set_text(text) }
    fn text(&self) -> String { self.0.text() }
}

struct MinCheckBoxHandle(ElementHandle);
impl WidgetHandle for MinCheckBoxHandle {
    fn set_pos(&self, x: i32, y: i32) { self.0.set_pos(x, y) }
    fn pos(&self) -> (i32, i32) { self.0.pos() }
    fn set_width(&self, w: u32) { self.0.set_width(w) }
    fn width(&self) -> u32 { self.0.width() }
    fn set_height(&self, h: u32) { self.0.set_height(h) }
    fn height(&self) -> u32 { self.0.height() }
    fn set_on_click(&self, callback: Box<dyn FnMut()>) { self.0.set_on_click(callback) }
    fn set_fore_color(&self, color: Option<Color>) { self.0.set_fore_color(color) }
    fn fore_color(&self) -> Option<Color> { self.0.fore_color() }
    fn set_back_color(&self, color: Option<Color>) { self.0.set_back_color(color) }
    fn back_color(&self) -> Option<Color> { self.0.back_color() }
}
impl CheckBoxHandle for MinCheckBoxHandle {
    fn set_text(&self, text: &str) { self.0.set_text(text) }
    fn text(&self) -> String { self.0.text() }
    fn set_checked(&self, checked: bool) { self.0.set_checked(checked) }
    fn checked(&self) -> bool { self.0.checked() }
}

struct MinTextBoxHandle(ElementHandle);
impl WidgetHandle for MinTextBoxHandle {
    fn set_pos(&self, x: i32, y: i32) { self.0.set_pos(x, y) }
    fn pos(&self) -> (i32, i32) { self.0.pos() }
    fn set_width(&self, w: u32) { self.0.set_width(w) }
    fn width(&self) -> u32 { self.0.width() }
    fn set_height(&self, h: u32) { self.0.set_height(h) }
    fn height(&self) -> u32 { self.0.height() }
    fn set_on_click(&self, callback: Box<dyn FnMut()>) { self.0.set_on_click(callback) }
    fn set_fore_color(&self, color: Option<Color>) { self.0.set_fore_color(color) }
    fn fore_color(&self) -> Option<Color> { self.0.fore_color() }
    fn set_back_color(&self, color: Option<Color>) { self.0.set_back_color(color) }
    fn back_color(&self) -> Option<Color> { self.0.back_color() }
}
impl TextBoxHandle for MinTextBoxHandle {
    fn set_text(&self, text: &str) { self.0.set_text(text) }
    fn text(&self) -> String { self.0.text() }
}

struct MinRadioButtonHandle(ElementHandle);
impl WidgetHandle for MinRadioButtonHandle {
    fn set_pos(&self, x: i32, y: i32) { self.0.set_pos(x, y) }
    fn pos(&self) -> (i32, i32) { self.0.pos() }
    fn set_width(&self, w: u32) { self.0.set_width(w) }
    fn width(&self) -> u32 { self.0.width() }
    fn set_height(&self, h: u32) { self.0.set_height(h) }
    fn height(&self) -> u32 { self.0.height() }
    fn set_on_click(&self, callback: Box<dyn FnMut()>) { self.0.set_on_click(callback) }
    fn set_fore_color(&self, color: Option<Color>) { self.0.set_fore_color(color) }
    fn fore_color(&self) -> Option<Color> { self.0.fore_color() }
    fn set_back_color(&self, color: Option<Color>) { self.0.set_back_color(color) }
    fn back_color(&self) -> Option<Color> { self.0.back_color() }
}
impl RadioButtonHandle for MinRadioButtonHandle {
    fn set_text(&self, text: &str) { self.0.set_text(text) }
    fn text(&self) -> String { self.0.text() }
    fn set_checked(&self, checked: bool) { self.0.set_checked(checked) }
    fn checked(&self) -> bool { self.0.checked() }
}

struct MinProgressBarHandle(ElementHandle);
impl WidgetHandle for MinProgressBarHandle {
    fn set_pos(&self, x: i32, y: i32) { self.0.set_pos(x, y) }
    fn pos(&self) -> (i32, i32) { self.0.pos() }
    fn set_width(&self, w: u32) { self.0.set_width(w) }
    fn width(&self) -> u32 { self.0.width() }
    fn set_height(&self, h: u32) { self.0.set_height(h) }
    fn height(&self) -> u32 { self.0.height() }
    fn set_on_click(&self, callback: Box<dyn FnMut()>) { self.0.set_on_click(callback) }
    fn set_fore_color(&self, color: Option<Color>) { self.0.set_fore_color(color) }
    fn fore_color(&self) -> Option<Color> { self.0.fore_color() }
    fn set_back_color(&self, color: Option<Color>) { self.0.set_back_color(color) }
    fn back_color(&self) -> Option<Color> { self.0.back_color() }
}
impl ProgressBarHandle for MinProgressBarHandle {
    fn set_value(&self, value: u32) {
        unsafe { ffi::rad_set_value(self.0.id, value.min(100)) };
    }
    fn value(&self) -> u32 {
        unsafe { ffi::rad_get_value(self.0.id) }
    }
}

struct MinPictureBoxHandle(ElementHandle);
impl WidgetHandle for MinPictureBoxHandle {
    fn set_pos(&self, x: i32, y: i32) { self.0.set_pos(x, y) }
    fn pos(&self) -> (i32, i32) { self.0.pos() }
    fn set_width(&self, w: u32) { self.0.set_width(w) }
    fn width(&self) -> u32 { self.0.width() }
    fn set_height(&self, h: u32) { self.0.set_height(h) }
    fn height(&self) -> u32 { self.0.height() }
    fn set_on_click(&self, callback: Box<dyn FnMut()>) { self.0.set_on_click(callback) }
    fn set_fore_color(&self, color: Option<Color>) { self.0.set_fore_color(color) }
    fn fore_color(&self) -> Option<Color> { self.0.fore_color() }
    fn set_back_color(&self, color: Option<Color>) { self.0.set_back_color(color) }
    fn back_color(&self) -> Option<Color> { self.0.back_color() }
}
impl PictureBoxHandle for MinPictureBoxHandle {
    // Reuses the generic text FFI — this element's "text" is its `src`
    // on the JS side (see `runtime.js`'s handling of `KIND_PICTURE_BOX`).
    fn set_image_path(&self, path: &str) { self.0.set_text(path) }
    fn image_path(&self) -> String { self.0.text() }
}

struct MinTreeViewHandle {
    el: ElementHandle,
    items: RefCell<Vec<TreeItem>>,
}
impl WidgetHandle for MinTreeViewHandle {
    fn set_pos(&self, x: i32, y: i32) { self.el.set_pos(x, y) }
    fn pos(&self) -> (i32, i32) { self.el.pos() }
    fn set_width(&self, w: u32) { self.el.set_width(w) }
    fn width(&self) -> u32 { self.el.width() }
    fn set_height(&self, h: u32) { self.el.set_height(h) }
    fn height(&self) -> u32 { self.el.height() }
    fn set_on_click(&self, callback: Box<dyn FnMut()>) { self.el.set_on_click(callback) }
    fn set_fore_color(&self, color: Option<Color>) { self.el.set_fore_color(color) }
    fn fore_color(&self) -> Option<Color> { self.el.fore_color() }
    fn set_back_color(&self, color: Option<Color>) { self.el.set_back_color(color) }
    fn back_color(&self) -> Option<Color> { self.el.back_color() }
}
impl TreeViewHandle for MinTreeViewHandle {
    fn set_items(&self, items: &[TreeItem]) {
        *self.items.borrow_mut() = items.to_vec();
        let encoded = items
            .iter()
            .map(|(depth, text)| format!("{depth}\t{text}"))
            .collect::<Vec<_>>()
            .join("\n");
        unsafe { ffi::rad_set_tree_items(self.el.id, encoded.as_ptr(), encoded.len() as u32) };
    }
    fn items(&self) -> Vec<TreeItem> {
        self.items.borrow().clone()
    }
}

fn create_element(parent_id: u32, kind: u32, text: &str, group: &str) -> ElementHandle {
    let id = unsafe {
        ffi::rad_create_element(
            parent_id,
            kind,
            text.as_ptr(),
            text.len() as u32,
            group.as_ptr(),
            group.len() as u32,
        )
    };
    ElementHandle { id, fore_color: Cell::new(None), back_color: Cell::new(None) }
}

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
    fn create_button(&self, text: &str) -> Box<dyn ButtonHandle> {
        Box::new(MinButtonHandle(create_element(self.id, kind::BUTTON, text, "")))
    }
    fn create_label(&self, text: &str) -> Box<dyn LabelHandle> {
        Box::new(MinLabelHandle(create_element(self.id, kind::LABEL, text, "")))
    }
    fn create_checkbox(&self, text: &str) -> Box<dyn CheckBoxHandle> {
        Box::new(MinCheckBoxHandle(create_element(self.id, kind::CHECKBOX, text, "")))
    }
    fn create_textbox(&self, text: &str) -> Box<dyn TextBoxHandle> {
        Box::new(MinTextBoxHandle(create_element(self.id, kind::TEXTBOX, text, "")))
    }
    fn create_radio_button(&self, group: &str, text: &str) -> Box<dyn RadioButtonHandle> {
        Box::new(MinRadioButtonHandle(create_element(self.id, kind::RADIO_BUTTON, text, group)))
    }
    fn create_progress_bar(&self) -> Box<dyn ProgressBarHandle> {
        Box::new(MinProgressBarHandle(create_element(self.id, kind::PROGRESS_BAR, "", "")))
    }
    fn create_picture_box(&self) -> Box<dyn PictureBoxHandle> {
        Box::new(MinPictureBoxHandle(create_element(self.id, kind::PICTURE_BOX, "", "")))
    }
    fn create_tree_view(&self) -> Box<dyn TreeViewHandle> {
        Box::new(MinTreeViewHandle {
            el: create_element(self.id, kind::TREE_VIEW, "", ""),
            items: RefCell::new(Vec::new()),
        })
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
