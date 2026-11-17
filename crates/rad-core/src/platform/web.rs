//! Browser plumbing (wasm-bindgen/web-sys) shared by every component's
//! `#[cfg(all(target_arch = "wasm32", feature = "web"))]` implementation: building
//! a control's DOM from its declarative `Preview`, shared position/size/colour
//! access, and the design-time drag-to-reposition behaviour. The backend itself
//! only owns the window chrome.

use crate::{Color, Preview};
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::{Document, Element, HtmlElement, HtmlInputElement, MouseEvent, PointerEvent};

/// What a component needs from its host to create a control: the document, the
/// element to append into, and whether the control should be draggable (true on
/// the designer's design surface, false when just embedding).
#[derive(Clone)]
pub struct Parent {
    pub document: Document,
    pub container: Element,
    pub draggable: bool,
}

/// Creates a bare `<tag>` and appends it to `parent`. Every control is
/// just one real DOM element (or, for a checkbox, a small fixed cluster
/// of them) — no virtual-DOM layer, so the browser's own native look for
/// that element is what you get unless CSS overrides it.
pub fn create_element(document: &Document, parent: &Element, tag: &str) -> HtmlElement {
    let el = document
        .create_element(tag)
        .unwrap_or_else(|_| panic!("create_element(\"{tag}\") should not fail"));
    parent
        .append_child(&el)
        .expect("appending to a live container should not fail");
    el.dyn_into()
        .unwrap_or_else(|_| panic!("<{tag}> is an HtmlElement"))
}

type Drag = [Closure<dyn FnMut(PointerEvent)>; 3];

/// A control's DOM, built from its `Preview`, kept alive together with its
/// design-time listeners. Every web handle in a component file holds one.
pub struct Base {
    /// The control's root element (the `<label>` of a checkbox or radio button).
    pub el: HtmlElement,
    /// The `<input>` of a labelled-input control.
    pub input: Option<HtmlInputElement>,
    /// The `<span>` holding a labelled-input control's text.
    pub text_el: Option<HtmlElement>,
    // Keep the design-time listeners alive for the control's lifetime.
    _drag: Option<Drag>,
    _suppress_toggle: Option<Closure<dyn FnMut(MouseEvent)>>,
}

impl Base {
    /// Builds the element `preview` describes under `parent`. `group` becomes
    /// the `name` of a labelled input (what makes radio buttons exclusive).
    pub fn new(parent: &Parent, preview: &Preview, group: Option<&str>) -> Self {
        let el = create_element(&parent.document, &parent.container, preview.tag);
        if !preview.class.is_empty() {
            el.set_class_name(preview.class);
        }
        for (name, value) in preview.attrs {
            el.set_attribute(name, value).ok();
        }
        if !preview.css.is_empty() {
            el.style().set_css_text(preview.css);
        }

        let (mut input, mut text_el) = (None, None);
        if let Some(kind) = preview.labeled_input {
            let input_el: HtmlInputElement = parent
                .document
                .create_element("input")
                .expect("create_element(\"input\")")
                .dyn_into()
                .expect("<input> is an HtmlInputElement");
            input_el.set_type(kind);
            if let Some(group) = group {
                input_el.set_name(group);
            }
            el.append_child(&input_el).expect("wrapper is live");
            let span: HtmlElement = parent
                .document
                .create_element("span")
                .expect("create_element(\"span\")")
                .dyn_into()
                .expect("<span> is an HtmlElement");
            el.append_child(&span).expect("wrapper is live");
            input = Some(input_el);
            text_el = Some(span);
        }

        let drag = parent.draggable.then(|| attach_drag(&el));
        let suppress_toggle = match (&input, parent.draggable) {
            (Some(input), true) => Some(suppress_native_toggle(input)),
            _ => None,
        };
        Self { el, input, text_el, _drag: drag, _suppress_toggle: suppress_toggle }
    }

    /// The root element as an `<input>` (for a control whose tag is `input`).
    pub fn as_input(&self) -> &HtmlInputElement {
        self.el.unchecked_ref()
    }

    /// The labelled input; only valid for a preview with `labeled_input`.
    pub fn labeled_input(&self) -> &HtmlInputElement {
        self.input.as_ref().expect("a labelled-input control")
    }

    /// The labelled text span; only valid for a preview with `labeled_input`.
    pub fn labeled_text(&self) -> &HtmlElement {
        self.text_el.as_ref().expect("a labelled-input control")
    }
}

// --- Shared pos/size, backing every `WidgetHandle` impl ---

pub fn el_set_pos(el: &HtmlElement, x: i32, y: i32) {
    el.style().set_property("position", "absolute").ok();
    el.style().set_property("left", &format!("{x}px")).ok();
    el.style().set_property("top", &format!("{y}px")).ok();
}

pub fn el_pos(el: &HtmlElement) -> (i32, i32) {
    (el.offset_left(), el.offset_top())
}

pub fn el_set_width(el: &HtmlElement, width: u32) {
    el.style().set_property("width", &format!("{width}px")).ok();
}

pub fn el_width(el: &HtmlElement) -> u32 {
    el.offset_width().max(0) as u32
}

pub fn el_set_height(el: &HtmlElement, height: u32) {
    el.style().set_property("height", &format!("{height}px")).ok();
}

pub fn el_height(el: &HtmlElement) -> u32 {
    el.offset_height().max(0) as u32
}

/// `None` reads back correctly for the same reason `el_set_back_color`'s
/// does: clearing the property (not setting `"inherit"`) is what makes
/// an unset color read back as `None` rather than a parsed color.
pub fn el_set_fore_color(el: &HtmlElement, color: Option<Color>) {
    match color {
        Some(c) => {
            el.style().set_property("color", &format!("rgb({},{},{})", c.r, c.g, c.b)).ok();
        }
        None => {
            el.style().remove_property("color").ok();
        }
    };
}

pub fn el_fore_color(el: &HtmlElement) -> Option<Color> {
    parse_rgb(&el.style().get_property_value("color").unwrap_or_default())
}

/// `None` reads back correctly with no extra state to track: an
/// `HtmlElement` with no inline `background-color` already renders
/// transparent, which is exactly what `None` means here, and clearing
/// the property (rather than setting it to `"transparent"`) is what
/// makes a later read see "unset" again instead of a color.
pub fn el_set_back_color(el: &HtmlElement, color: Option<Color>) {
    match color {
        Some(c) => {
            el.style().set_property("background-color", &format!("rgb({},{},{})", c.r, c.g, c.b)).ok();
        }
        None => {
            el.style().remove_property("background-color").ok();
        }
    }
}

pub fn el_back_color(el: &HtmlElement) -> Option<Color> {
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
/// simplest to just leak the listener for the page's lifetime.
pub fn el_set_on_click(el: &HtmlElement, mut callback: Box<dyn FnMut()>) {
    let closure = Closure::wrap(Box::new(move || callback()) as Box<dyn FnMut()>);
    el.add_event_listener_with_callback("click", closure.as_ref().unchecked_ref())
        .ok();
    closure.forget();
}

/// Generates a `WidgetHandle` impl for `$ty` from a `$field: Base`, using the
/// shared `el_*` functions. Every web handle needs this same impl.
macro_rules! impl_widget_handle {
    ($ty:ty, $field:ident) => {
        impl crate::WidgetHandle for $ty {
            fn set_pos(&self, x: i32, y: i32) {
                crate::platform::web::el_set_pos(&self.$field.el, x, y);
            }
            fn pos(&self) -> (i32, i32) {
                crate::platform::web::el_pos(&self.$field.el)
            }
            fn set_width(&self, width: u32) {
                crate::platform::web::el_set_width(&self.$field.el, width);
            }
            fn width(&self) -> u32 {
                crate::platform::web::el_width(&self.$field.el)
            }
            fn set_height(&self, height: u32) {
                crate::platform::web::el_set_height(&self.$field.el, height);
            }
            fn height(&self) -> u32 {
                crate::platform::web::el_height(&self.$field.el)
            }
            fn set_on_click(&self, callback: Box<dyn FnMut()>) {
                crate::platform::web::el_set_on_click(&self.$field.el, callback);
            }
            fn set_fore_color(&self, color: Option<crate::Color>) {
                crate::platform::web::el_set_fore_color(&self.$field.el, color);
            }
            fn fore_color(&self) -> Option<crate::Color> {
                crate::platform::web::el_fore_color(&self.$field.el)
            }
            fn set_back_color(&self, color: Option<crate::Color>) {
                crate::platform::web::el_set_back_color(&self.$field.el, color);
            }
            fn back_color(&self) -> Option<crate::Color> {
                crate::platform::web::el_back_color(&self.$field.el)
            }
        }
    };
}

pub(crate) use impl_widget_handle;

// --- Design-time drag-to-reposition, shared by every control type ---

#[derive(Clone, Copy)]
struct DragStart {
    client_x: i32,
    client_y: i32,
    left: i32,
    top: i32,
}

/// Wires `el` to be repositionable by pointer-drag in the browser. Used for every
/// control created on a draggable `Parent` (the design-time surface); never for a
/// plain embedding host.
pub fn attach_drag(el: &HtmlElement) -> Drag {
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

/// Stops a checkbox/radio `<input>` from actually toggling on click — used only
/// for the design-time (draggable) instances the designer places, where a click
/// should select the control, not flip its state. `preventDefault` on the click
/// still lets it bubble, so the designer's own click-to-select delegation
/// (listening higher up the DOM) still runs.
fn suppress_native_toggle(input: &HtmlInputElement) -> Closure<dyn FnMut(MouseEvent)> {
    let closure = Closure::wrap(Box::new(|e: MouseEvent| {
        e.prevent_default();
    }) as Box<dyn FnMut(MouseEvent)>);
    input
        .add_event_listener_with_callback("click", closure.as_ref().unchecked_ref())
        .ok();
    closure
}

/// Attaches `callback` as the element's `event` listener, replacing (and
/// detaching) whatever `slot` held before, so setting a handler twice leaves
/// exactly one listener behind.
pub fn el_set_listener(
    el: &HtmlElement,
    event: &str,
    slot: &RefCell<Option<Closure<dyn FnMut()>>>,
    mut callback: Box<dyn FnMut()>,
) {
    let closure = Closure::wrap(Box::new(move || callback()) as Box<dyn FnMut()>);
    if let Some(old) = slot.borrow_mut().replace(closure) {
        el.remove_event_listener_with_callback(event, old.as_ref().unchecked_ref()).ok();
    }
    if let Some(new) = slot.borrow().as_ref() {
        el.add_event_listener_with_callback(event, new.as_ref().unchecked_ref()).ok();
    }
}
