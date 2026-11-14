//! WASM preview backend: renders widgets as styled DOM nodes inside a
//! container element. Not the real app — this is the browser-side
//! prototyping surface. The OS skin is a CSS class on the container,
//! swapped at runtime to flip the preview's look between platforms.

mod window;

pub use window::{WebBackend, WebWindow};

use rad_core::{ButtonHandle, CheckBoxHandle, Host, LabelHandle, OsSkin, WidgetHandle};
use std::cell::Cell;
use std::rc::Rc;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::{Document, Element, HtmlElement, HtmlInputElement, PointerEvent};

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

/// Generates a `WidgetHandle` impl for `$ty` from a `$field: HtmlElement`
/// (or anything that derefs to one), using the shared `el_*` functions
/// above. Every control handle in this crate needs this same impl.
macro_rules! impl_widget_handle {
    ($ty:ty, $field:ident) => {
        impl WidgetHandle for $ty {
            fn set_pos(&mut self, x: i32, y: i32) {
                el_set_pos(&self.$field, x, y);
            }
            fn pos(&self) -> (i32, i32) {
                el_pos(&self.$field)
            }
            fn set_width(&mut self, width: u32) {
                el_set_width(&self.$field, width);
            }
            fn width(&self) -> u32 {
                el_width(&self.$field)
            }
            fn set_height(&mut self, height: u32) {
                el_set_height(&self.$field, height);
            }
            fn height(&self) -> u32 {
                el_height(&self.$field)
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
    fn set_text(&mut self, text: &str) {
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
    fn set_text(&mut self, text: &str) {
        self.el.set_text_content(Some(text));
    }

    fn text(&self) -> String {
        self.el.text_content().unwrap_or_default()
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
        Self { wrapper, input, text_el, _drag: drag }
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
    fn set_text(&mut self, text: &str) {
        self.text_el.set_text_content(Some(text));
    }

    fn text(&self) -> String {
        self.text_el.text_content().unwrap_or_default()
    }

    fn set_checked(&mut self, checked: bool) {
        self.input.set_checked(checked);
    }

    fn checked(&self) -> bool {
        self.input.checked()
    }
}
