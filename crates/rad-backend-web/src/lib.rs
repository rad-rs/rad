//! WASM preview backend: renders widgets as styled DOM nodes inside a
//! container element. Not the real app — this is the browser-side
//! prototyping surface. The OS skin is a CSS class on the container,
//! swapped at runtime to flip the preview's look between platforms.

mod window;

pub use window::{WebBackend, WebWindow};

use rad_core::{
    ButtonHandle, CheckBoxHandle, Color, Host, LabelHandle, OsSkin, PictureBoxHandle,
    ProgressBarHandle, RadioButtonHandle, TextBoxHandle, TimerHandle, TreeViewHandle, WidgetHandle,
};
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::{Document, Element, HtmlElement, HtmlInputElement, MouseEvent, PointerEvent};

fn skin_class(skin: OsSkin) -> &'static str {
    match skin {
        OsSkin::Windows => "rad-skin-windows",
        OsSkin::MacOs => "rad-skin-macos",
        OsSkin::Linux => "rad-skin-linux",
    }
}

/// A preview surface: one DOM element that widgets are appended into.
pub struct WebHost {
    document: Document,
    container: Element,
}

impl WebHost {
    /// Mounts a preview into `container`, skinned for `skin`.
    pub fn new(container: Element, skin: OsSkin) -> Self {
        let document = container
            .owner_document()
            .expect("container must be attached to a document");
        let host = Self { document, container };
        host.set_skin(skin);
        host
    }

    /// Switches the preview's look without touching any widget state.
    pub fn set_skin(&self, skin: OsSkin) {
        let class_list = self.container.class_list();
        for s in [OsSkin::Windows, OsSkin::MacOs, OsSkin::Linux] {
            let _ = class_list.remove_1(skin_class(s));
        }
        let _ = class_list.add_1(skin_class(skin));
    }
}

impl Host for WebHost {
    fn create_button(&self, text: &str) -> Box<dyn ButtonHandle> {
        let el = create_element(&self.document, &self.container, "button");
        el.set_text_content(Some(text));
        Box::new(WebButtonHandle::new(el))
    }

    fn create_label(&self, text: &str) -> Box<dyn LabelHandle> {
        let el = create_element(&self.document, &self.container, "span");
        el.set_text_content(Some(text));
        Box::new(WebLabelHandle::new(el))
    }

    fn create_checkbox(&self, text: &str) -> Box<dyn CheckBoxHandle> {
        Box::new(WebCheckBoxHandle::new(&self.document, &self.container, text, false))
    }

    fn create_textbox(&self, text: &str) -> Box<dyn TextBoxHandle> {
        Box::new(WebTextBoxHandle::new(&self.document, &self.container, text))
    }

    fn create_radio_button(&self, group: &str, text: &str) -> Box<dyn RadioButtonHandle> {
        Box::new(WebRadioButtonHandle::new(
            &self.document,
            &self.container,
            group,
            text,
            false,
        ))
    }

    fn create_progress_bar(&self) -> Box<dyn ProgressBarHandle> {
        let el = create_element(&self.document, &self.container, "progress");
        el.set_attribute("max", "100").ok();
        el.set_attribute("value", "0").ok();
        Box::new(WebProgressBarHandle::new(el))
    }

    fn create_picture_box(&self) -> Box<dyn PictureBoxHandle> {
        let el = create_element(&self.document, &self.container, "img");
        el.style().set_property("object-fit", "contain").ok();
        Box::new(WebPictureBoxHandle::new(el))
    }

    fn create_tree_view(&self) -> Box<dyn TreeViewHandle> {
        let el = create_element(&self.document, &self.container, "div");
        el.style().set_property("overflow", "auto").ok();
        el.style().set_property("border", "1px solid #adadad").ok();
        el.style().set_property("background", "#fff").ok();
        Box::new(WebTreeViewHandle::new(el))
    }

    fn create_timer(&self) -> Box<dyn TimerHandle> {
        Box::new(WebTimerHandle::new())
    }
}

/// Creates a bare `<tag>` and appends it to `parent`. Every control is
/// just one real DOM element (or, for a checkbox, a small fixed cluster
/// of them) — no virtual-DOM layer, so the browser's own native look for
/// that element is what you get unless CSS overrides it.
pub(crate) fn create_element(document: &Document, parent: &Element, tag: &str) -> HtmlElement {
    let el = document
        .create_element(tag)
        .unwrap_or_else(|_| panic!("create_element(\"{tag}\") should not fail"));
    parent
        .append_child(&el)
        .expect("appending to a live container should not fail");
    el.dyn_into()
        .unwrap_or_else(|_| panic!("<{tag}> is an HtmlElement"))
}

// --- Shared pos/size, backing every `WidgetHandle` impl in this crate ---

pub(crate) fn el_set_pos(el: &HtmlElement, x: i32, y: i32) {
    el.style().set_property("position", "absolute").ok();
    el.style().set_property("left", &format!("{x}px")).ok();
    el.style().set_property("top", &format!("{y}px")).ok();
}

pub(crate) fn el_pos(el: &HtmlElement) -> (i32, i32) {
    (el.offset_left(), el.offset_top())
}

pub(crate) fn el_set_width(el: &HtmlElement, width: u32) {
    el.style().set_property("width", &format!("{width}px")).ok();
}

pub(crate) fn el_width(el: &HtmlElement) -> u32 {
    el.offset_width().max(0) as u32
}

pub(crate) fn el_set_height(el: &HtmlElement, height: u32) {
    el.style().set_property("height", &format!("{height}px")).ok();
}

pub(crate) fn el_height(el: &HtmlElement) -> u32 {
    el.offset_height().max(0) as u32
}

/// `None` reads back correctly for the same reason `el_set_back_color`'s
/// does: clearing the property (not setting `"inherit"`) is what makes
/// an unset color read back as `None` rather than a parsed color.
pub(crate) fn el_set_fore_color(el: &HtmlElement, color: Option<Color>) {
    match color {
        Some(c) => {
            el.style().set_property("color", &format!("rgb({},{},{})", c.r, c.g, c.b)).ok();
        }
        None => {
            el.style().remove_property("color").ok();
        }
    };
}

pub(crate) fn el_fore_color(el: &HtmlElement) -> Option<Color> {
    parse_rgb(&el.style().get_property_value("color").unwrap_or_default())
}

/// `None` reads back correctly with no extra state to track: an
/// `HtmlElement` with no inline `background-color` already renders
/// transparent, which is exactly what `None` means here, and clearing
/// the property (rather than setting it to `"transparent"`) is what
/// makes a later read see "unset" again instead of a color.
pub(crate) fn el_set_back_color(el: &HtmlElement, color: Option<Color>) {
    match color {
        Some(c) => {
            el.style().set_property("background-color", &format!("rgb({},{},{})", c.r, c.g, c.b)).ok();
        }
        None => {
            el.style().remove_property("background-color").ok();
        }
    }
}

pub(crate) fn el_back_color(el: &HtmlElement) -> Option<Color> {
    parse_rgb(&el.style().get_property_value("background-color").unwrap_or_default())
}

fn parse_rgb(s: &str) -> Option<Color> {
    let inner = s.strip_prefix("rgb(")?.strip_suffix(')')?;
    let mut parts = inner.split(',');
    let r: u8 = parts.next()?.trim().parse().ok()?;
    let g: u8 = parts.next()?.trim().parse().ok()?;
    let b: u8 = parts.next()?.trim().parse().ok()?;
    Some(Color::rgb(r, g, b))
}

/// Wires `el`'s click — the default event every control type exposes.
/// `callback` only needs to run, never be cancelled or re-read, so it's
/// simplest to just leak the listener for the page's lifetime, same as
/// every other one-shot closure in this crate.
pub(crate) fn el_set_on_click(el: &HtmlElement, mut callback: Box<dyn FnMut()>) {
    let closure = Closure::wrap(Box::new(move || callback()) as Box<dyn FnMut()>);
    el.add_event_listener_with_callback("click", closure.as_ref().unchecked_ref())
        .ok();
    closure.forget();
}

/// Generates a `WidgetHandle` impl for `$ty` from a `$field: HtmlElement`
/// (or anything that derefs to one), using the shared `el_*` functions
/// above. Every control handle in this crate needs this same impl.
macro_rules! impl_widget_handle {
    ($ty:ty, $field:ident) => {
        impl WidgetHandle for $ty {
            fn set_pos(&self, x: i32, y: i32) {
                el_set_pos(&self.$field, x, y);
            }
            fn pos(&self) -> (i32, i32) {
                el_pos(&self.$field)
            }
            fn set_width(&self, width: u32) {
                el_set_width(&self.$field, width);
            }
            fn width(&self) -> u32 {
                el_width(&self.$field)
            }
            fn set_height(&self, height: u32) {
                el_set_height(&self.$field, height);
            }
            fn height(&self) -> u32 {
                el_height(&self.$field)
            }
            fn set_on_click(&self, callback: Box<dyn FnMut()>) {
                el_set_on_click(&self.$field, callback);
            }
            fn set_fore_color(&self, color: Option<Color>) {
                el_set_fore_color(&self.$field, color);
            }
            fn fore_color(&self) -> Option<Color> {
                el_fore_color(&self.$field)
            }
            fn set_back_color(&self, color: Option<Color>) {
                el_set_back_color(&self.$field, color);
            }
            fn back_color(&self) -> Option<Color> {
                el_back_color(&self.$field)
            }
        }
    };
}

// --- Design-time drag-to-reposition, shared by every control type ---

#[derive(Clone, Copy)]
struct DragStart {
    client_x: i32,
    client_y: i32,
    left: i32,
    top: i32,
}

/// Wires `el` to be repositionable by pointer-drag in the browser. Used
/// for every control a `WebWindow` creates (the design-time surface);
/// never for a plain embedding `WebHost`.
pub(crate) fn attach_drag(el: &HtmlElement) -> [Closure<dyn FnMut(PointerEvent)>; 3] {
    let drag_start: Rc<Cell<Option<DragStart>>> = Rc::new(Cell::new(None));

    let on_down = {
        let el = el.clone();
        let drag_start = drag_start.clone();
        Closure::wrap(Box::new(move |e: PointerEvent| {
            let _ = el.set_pointer_capture(e.pointer_id());
            drag_start.set(Some(DragStart {
                client_x: e.client_x(),
                client_y: e.client_y(),
                left: el.offset_left(),
                top: el.offset_top(),
            }));
        }) as Box<dyn FnMut(PointerEvent)>)
    };
    el.add_event_listener_with_callback("pointerdown", on_down.as_ref().unchecked_ref())
        .ok();

    let on_move = {
        let el = el.clone();
        let drag_start = drag_start.clone();
        Closure::wrap(Box::new(move |e: PointerEvent| {
            if let Some(start) = drag_start.get() {
                let dx = e.client_x() - start.client_x;
                let dy = e.client_y() - start.client_y;
                let _ = el
                    .style()
                    .set_property("left", &format!("{}px", start.left + dx));
                let _ = el
                    .style()
                    .set_property("top", &format!("{}px", start.top + dy));
            }
        }) as Box<dyn FnMut(PointerEvent)>)
    };
    el.add_event_listener_with_callback("pointermove", on_move.as_ref().unchecked_ref())
        .ok();

    let on_up = {
        let el = el.clone();
        let drag_start = drag_start.clone();
        Closure::wrap(Box::new(move |e: PointerEvent| {
            let _ = el.release_pointer_capture(e.pointer_id());
            drag_start.set(None);
        }) as Box<dyn FnMut(PointerEvent)>)
    };
    el.add_event_listener_with_callback("pointerup", on_up.as_ref().unchecked_ref())
        .ok();

    [on_down, on_move, on_up]
}

/// Stops a checkbox/radio `<input>` from actually toggling on click —
/// used only for the design-time (draggable) instances the designer
/// places, where a click should select the control, not flip its state.
/// `preventDefault` on the click still lets it bubble, so the designer's
/// own click-to-select delegation (listening higher up the DOM) still runs.
fn suppress_native_toggle(input: &HtmlInputElement) -> Closure<dyn FnMut(MouseEvent)> {
    let closure = Closure::wrap(Box::new(|e: MouseEvent| {
        e.prevent_default();
    }) as Box<dyn FnMut(MouseEvent)>);
    input
        .add_event_listener_with_callback("click", closure.as_ref().unchecked_ref())
        .ok();
    closure
}

// --- Button ---

pub(crate) struct WebButtonHandle {
    el: HtmlElement,
    // Keeps design-time drag listeners alive for the control's lifetime, if any.
    _drag: Option<[Closure<dyn FnMut(PointerEvent)>; 3]>,
}

impl WebButtonHandle {
    pub(crate) fn new(el: HtmlElement) -> Self {
        Self { el, _drag: None }
    }

    pub(crate) fn new_draggable(el: HtmlElement) -> Self {
        let drag = attach_drag(&el);
        Self { el, _drag: Some(drag) }
    }
}

impl_widget_handle!(WebButtonHandle, el);

impl ButtonHandle for WebButtonHandle {
    fn set_text(&self, text: &str) {
        self.el.set_text_content(Some(text));
    }

    fn text(&self) -> String {
        self.el.text_content().unwrap_or_default()
    }
}

// --- Label ---

pub(crate) struct WebLabelHandle {
    el: HtmlElement,
    _drag: Option<[Closure<dyn FnMut(PointerEvent)>; 3]>,
}

impl WebLabelHandle {
    pub(crate) fn new(el: HtmlElement) -> Self {
        Self { el, _drag: None }
    }

    pub(crate) fn new_draggable(el: HtmlElement) -> Self {
        let drag = attach_drag(&el);
        Self { el, _drag: Some(drag) }
    }
}

impl_widget_handle!(WebLabelHandle, el);

impl LabelHandle for WebLabelHandle {
    fn set_text(&self, text: &str) {
        self.el.set_text_content(Some(text));
    }

    fn text(&self) -> String {
        self.el.text_content().unwrap_or_default()
    }
}

// --- Timer ---

struct TimerSlot {
    callback: RefCell<Option<Box<dyn FnMut()>>>,
    alive: Cell<bool>,
    in_tick: Cell<bool>,
}

impl TimerSlot {
    /// Runs one tick with the callback taken out of its slot, so the handler
    /// can stop or retune the timer, or replace its own callback, without a
    /// re-entrancy panic.
    fn tick(&self) {
        let taken = self.callback.borrow_mut().take();
        if let Some(mut callback) = taken {
            self.in_tick.set(true);
            callback();
            self.in_tick.set(false);
            if self.alive.get() && self.callback.borrow().is_none() {
                *self.callback.borrow_mut() = Some(callback);
            }
        }
    }
}

/// A `setInterval` timer. There is no DOM node: the browser's own timer is
/// the whole implementation. One JS closure lives as long as the handle, so
/// stopping from inside a tick never drops the closure that is running.
pub(crate) struct WebTimerHandle {
    slot: Rc<TimerSlot>,
    closure: Closure<dyn FnMut()>,
    interval_ms: Cell<u32>,
    enabled: Cell<bool>,
    interval_id: Cell<Option<i32>>,
}

impl WebTimerHandle {
    pub(crate) fn new() -> Self {
        let slot = Rc::new(TimerSlot {
            callback: RefCell::new(None),
            alive: Cell::new(true),
            in_tick: Cell::new(false),
        });
        let closure = {
            let slot = slot.clone();
            Closure::wrap(Box::new(move || slot.tick()) as Box<dyn FnMut()>)
        };
        // WinForms defaults: disabled, 100 ms.
        Self { slot, closure, interval_ms: Cell::new(100), enabled: Cell::new(false), interval_id: Cell::new(None) }
    }

    fn clear(&self) {
        if let (Some(id), Some(window)) = (self.interval_id.take(), web_sys::window()) {
            window.clear_interval_with_handle(id);
        }
    }

    fn arm(&self) {
        self.clear();
        if let Some(window) = web_sys::window() {
            let ms = self.interval_ms.get().clamp(1, i32::MAX as u32) as i32;
            if let Ok(id) = window.set_interval_with_callback_and_timeout_and_arguments_0(
                self.closure.as_ref().unchecked_ref(),
                ms,
            ) {
                self.interval_id.set(Some(id));
            }
        }
    }
}

impl TimerHandle for WebTimerHandle {
    fn set_interval(&self, interval_ms: u32) {
        self.interval_ms.set(interval_ms);
        if self.enabled.get() {
            self.arm();
        }
    }
    fn interval(&self) -> u32 {
        self.interval_ms.get()
    }
    fn set_enabled(&self, enabled: bool) {
        if enabled == self.enabled.get() {
            return;
        }
        self.enabled.set(enabled);
        if enabled {
            self.arm();
        } else {
            self.clear();
        }
    }
    fn enabled(&self) -> bool {
        self.enabled.get()
    }
    fn set_on_tick(&self, callback: Box<dyn FnMut()>) {
        *self.slot.callback.borrow_mut() = Some(callback);
    }
}

impl Drop for WebTimerHandle {
    fn drop(&mut self) {
        self.slot.alive.set(false);
        self.clear();
        if self.slot.in_tick.get() {
            // Dropped from inside its own tick: that JS closure is on the
            // stack right now, so leak it instead of freeing it.
            let closure = std::mem::replace(&mut self.closure, Closure::wrap(Box::new(|| {}) as Box<dyn FnMut()>));
            closure.forget();
        }
    }
}

// --- ProgressBar ---

pub(crate) struct WebProgressBarHandle {
    el: HtmlElement,
    _drag: Option<[Closure<dyn FnMut(PointerEvent)>; 3]>,
}

impl WebProgressBarHandle {
    pub(crate) fn new(el: HtmlElement) -> Self {
        Self { el, _drag: None }
    }

    pub(crate) fn new_draggable(el: HtmlElement) -> Self {
        let drag = attach_drag(&el);
        Self { el, _drag: Some(drag) }
    }
}

impl_widget_handle!(WebProgressBarHandle, el);

impl ProgressBarHandle for WebProgressBarHandle {
    fn set_value(&self, value: u32) {
        self.el.set_attribute("value", &value.min(100).to_string()).ok();
    }

    fn value(&self) -> u32 {
        self.el
            .get_attribute("value")
            .and_then(|v| v.parse().ok())
            .unwrap_or(0)
    }
}

// --- PictureBox ---

pub(crate) struct WebPictureBoxHandle {
    el: HtmlElement,
    _drag: Option<[Closure<dyn FnMut(PointerEvent)>; 3]>,
}

impl WebPictureBoxHandle {
    pub(crate) fn new(el: HtmlElement) -> Self {
        Self { el, _drag: None }
    }

    pub(crate) fn new_draggable(el: HtmlElement) -> Self {
        let drag = attach_drag(&el);
        Self { el, _drag: Some(drag) }
    }
}

impl_widget_handle!(WebPictureBoxHandle, el);

impl PictureBoxHandle for WebPictureBoxHandle {
    fn set_image_path(&self, path: &str) {
        self.el.set_attribute("src", path).ok();
    }

    fn image_path(&self) -> String {
        self.el.get_attribute("src").unwrap_or_default()
    }
}

// --- TreeView ---

/// `items` is kept alongside the DOM rather than read back from it —
/// rebuilding a nested `<ul>` on every `set_items` is simpler than
/// parsing one back into `(depth, text)` pairs.
pub(crate) struct WebTreeViewHandle {
    el: HtmlElement,
    items: std::cell::RefCell<Vec<rad_core::TreeItem>>,
    _drag: Option<[Closure<dyn FnMut(PointerEvent)>; 3]>,
}

impl WebTreeViewHandle {
    pub(crate) fn new(el: HtmlElement) -> Self {
        Self { el, items: std::cell::RefCell::new(Vec::new()), _drag: None }
    }

    pub(crate) fn new_draggable(el: HtmlElement) -> Self {
        let drag = attach_drag(&el);
        Self { el, items: std::cell::RefCell::new(Vec::new()), _drag: Some(drag) }
    }

    fn render(&self) {
        let document = self.el.owner_document().expect("el has a document");
        self.el.set_text_content(None);
        // One flat list, indenting each row by its depth rather than
        // building real nested `<ul>`s — simplest way to render a
        // depth-only hierarchy without tracking which rows are each
        // other's ancestors.
        for (depth, text) in self.items.borrow().iter() {
            let row = create_element(&document, &self.el.clone().unchecked_into(), "div");
            row.set_text_content(Some(text));
            row.style().set_property("padding-left", &format!("{}px", depth * 16)).ok();
            row.style().set_property("white-space", "nowrap").ok();
        }
    }
}

impl_widget_handle!(WebTreeViewHandle, el);

impl TreeViewHandle for WebTreeViewHandle {
    fn set_items(&self, items: &[rad_core::TreeItem]) {
        *self.items.borrow_mut() = items.to_vec();
        self.render();
    }

    fn items(&self) -> Vec<rad_core::TreeItem> {
        self.items.borrow().clone()
    }
}

// --- CheckBox ---

/// A real `<input type="checkbox">` plus its text, wrapped in a `<label>`
/// so clicking the text toggles it too — the native HTML pattern, rather
/// than something we fake with a `<div>` and a click handler.
pub(crate) struct WebCheckBoxHandle {
    wrapper: HtmlElement,
    input: HtmlInputElement,
    text_el: HtmlElement,
    _drag: Option<[Closure<dyn FnMut(PointerEvent)>; 3]>,
    _suppress_toggle: Option<Closure<dyn FnMut(MouseEvent)>>,
}

impl WebCheckBoxHandle {
    fn build(document: &Document, parent: &Element, text: &str, checked: bool, draggable: bool) -> Self {
        let wrapper = create_element(document, parent, "label");
        wrapper.set_class_name("rad-checkbox");

        let input_el = document
            .create_element("input")
            .expect("create_element(\"input\")");
        let input: HtmlInputElement = input_el.dyn_into().expect("<input> is an HtmlInputElement");
        input.set_type("checkbox");
        input.set_checked(checked);
        wrapper.append_child(&input).expect("wrapper is live");

        let text_el_raw = document.create_element("span").expect("create_element(\"span\")");
        text_el_raw.set_text_content(Some(text));
        wrapper.append_child(&text_el_raw).expect("wrapper is live");
        let text_el: HtmlElement = text_el_raw.dyn_into().expect("<span> is an HtmlElement");

        let drag = draggable.then(|| attach_drag(&wrapper));
        let suppress_toggle = draggable.then(|| suppress_native_toggle(&input));
        Self { wrapper, input, text_el, _drag: drag, _suppress_toggle: suppress_toggle }
    }

    pub(crate) fn new(document: &Document, parent: &Element, text: &str, checked: bool) -> Self {
        Self::build(document, parent, text, checked, false)
    }

    pub(crate) fn new_draggable(document: &Document, parent: &Element, text: &str, checked: bool) -> Self {
        Self::build(document, parent, text, checked, true)
    }
}

impl_widget_handle!(WebCheckBoxHandle, wrapper);

impl CheckBoxHandle for WebCheckBoxHandle {
    fn set_text(&self, text: &str) {
        self.text_el.set_text_content(Some(text));
    }

    fn text(&self) -> String {
        self.text_el.text_content().unwrap_or_default()
    }

    fn set_checked(&self, checked: bool) {
        self.input.set_checked(checked);
    }

    fn checked(&self) -> bool {
        self.input.checked()
    }
}

// --- TextBox ---

pub(crate) struct WebTextBoxHandle {
    input: HtmlInputElement,
    _drag: Option<[Closure<dyn FnMut(PointerEvent)>; 3]>,
}

impl WebTextBoxHandle {
    fn build(document: &Document, parent: &Element, text: &str, draggable: bool) -> Self {
        let el = document.create_element("input").expect("create_element(\"input\")");
        parent.append_child(&el).expect("appending to a live container should not fail");
        let input: HtmlInputElement = el.dyn_into().expect("<input> is an HtmlInputElement");
        input.set_type("text");
        input.set_value(text);

        let as_html: &HtmlElement = input.as_ref();
        let drag = draggable.then(|| attach_drag(as_html));
        Self { input, _drag: drag }
    }

    pub(crate) fn new(document: &Document, parent: &Element, text: &str) -> Self {
        Self::build(document, parent, text, false)
    }

    pub(crate) fn new_draggable(document: &Document, parent: &Element, text: &str) -> Self {
        Self::build(document, parent, text, true)
    }
}

impl WidgetHandle for WebTextBoxHandle {
    fn set_pos(&self, x: i32, y: i32) {
        el_set_pos(self.input.as_ref(), x, y);
    }
    fn pos(&self) -> (i32, i32) {
        el_pos(self.input.as_ref())
    }
    fn set_width(&self, width: u32) {
        el_set_width(self.input.as_ref(), width);
    }
    fn width(&self) -> u32 {
        el_width(self.input.as_ref())
    }
    fn set_height(&self, height: u32) {
        el_set_height(self.input.as_ref(), height);
    }
    fn height(&self) -> u32 {
        el_height(self.input.as_ref())
    }
    fn set_on_click(&self, callback: Box<dyn FnMut()>) {
        el_set_on_click(self.input.as_ref(), callback);
    }
    fn set_fore_color(&self, color: Option<Color>) {
        el_set_fore_color(self.input.as_ref(), color);
    }
    fn fore_color(&self) -> Option<Color> {
        el_fore_color(self.input.as_ref())
    }
    fn set_back_color(&self, color: Option<Color>) {
        el_set_back_color(self.input.as_ref(), color);
    }
    fn back_color(&self) -> Option<Color> {
        el_back_color(self.input.as_ref())
    }
}

impl TextBoxHandle for WebTextBoxHandle {
    fn set_text(&self, text: &str) {
        self.input.set_value(text);
    }

    fn text(&self) -> String {
        self.input.value()
    }
}

// --- RadioButton ---

/// A real `<input type="radio">` plus its text, wrapped in a `<label>`,
/// same native pattern as `WebCheckBoxHandle`. Mutual exclusivity on the
/// web comes only from a shared `name` attribute, so `group` is baked
/// into the input's `name` at creation time.
pub(crate) struct WebRadioButtonHandle {
    wrapper: HtmlElement,
    input: HtmlInputElement,
    text_el: HtmlElement,
    _drag: Option<[Closure<dyn FnMut(PointerEvent)>; 3]>,
    _suppress_toggle: Option<Closure<dyn FnMut(MouseEvent)>>,
}

impl WebRadioButtonHandle {
    fn build(
        document: &Document,
        parent: &Element,
        group: &str,
        text: &str,
        checked: bool,
        draggable: bool,
    ) -> Self {
        let wrapper = create_element(document, parent, "label");
        wrapper.set_class_name("rad-radio-button");

        let input_el = document
            .create_element("input")
            .expect("create_element(\"input\")");
        let input: HtmlInputElement = input_el.dyn_into().expect("<input> is an HtmlInputElement");
        input.set_type("radio");
        input.set_name(group);
        input.set_checked(checked);
        wrapper.append_child(&input).expect("wrapper is live");

        let text_el_raw = document.create_element("span").expect("create_element(\"span\")");
        text_el_raw.set_text_content(Some(text));
        wrapper.append_child(&text_el_raw).expect("wrapper is live");
        let text_el: HtmlElement = text_el_raw.dyn_into().expect("<span> is an HtmlElement");

        let drag = draggable.then(|| attach_drag(&wrapper));
        let suppress_toggle = draggable.then(|| suppress_native_toggle(&input));
        Self { wrapper, input, text_el, _drag: drag, _suppress_toggle: suppress_toggle }
    }

    pub(crate) fn new(document: &Document, parent: &Element, group: &str, text: &str, checked: bool) -> Self {
        Self::build(document, parent, group, text, checked, false)
    }

    pub(crate) fn new_draggable(
        document: &Document,
        parent: &Element,
        group: &str,
        text: &str,
        checked: bool,
    ) -> Self {
        Self::build(document, parent, group, text, checked, true)
    }
}

impl_widget_handle!(WebRadioButtonHandle, wrapper);

impl RadioButtonHandle for WebRadioButtonHandle {
    fn set_text(&self, text: &str) {
        self.text_el.set_text_content(Some(text));
    }

    fn text(&self) -> String {
        self.text_el.text_content().unwrap_or_default()
    }

    fn set_checked(&self, checked: bool) {
        self.input.set_checked(checked);
    }

    fn checked(&self) -> bool {
        self.input.checked()
    }
}
