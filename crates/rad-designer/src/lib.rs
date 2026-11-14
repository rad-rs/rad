//! Minimal designer: drag a palette tile (Button, Label, or CheckBox)
//! onto a resizable preview `Window`; placed controls are repositionable
//! by pointer-drag (built into `rad-backend-web`). "Save" reads each
//! placed control's current state back through its plain `rad-core` API
//! and renders it as the WinForms-style generated/hand-written file split
//! — shown in a textarea to copy into `form1.designer.rs` by hand.
//!
//! Adding a further control kind to the palette means: one more
//! `PlacedKind` variant, one line in each of its match arms, and a
//! palette tile in `index.html` — the pos/size/text plumbing underneath
//! is already shared via `rad-core`'s `WidgetHandle`.

use rad_backend_web::WebWindow;
use rad_core::{Button, CheckBox, Label, WindowHandle};
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::{
    Document, DragEvent, Element, Event, HtmlElement, HtmlInputElement, HtmlTextAreaElement,
    MouseEvent, Node, PointerEvent,
};

/// One of the control types the palette can place. Each variant is just a
/// `rad-core` wrapper type — this enum exists only so the designer can
/// hold "whatever got placed" in one `Vec`, since Rust has no common
/// concrete type for "any widget".
enum PlacedKind {
    Button(Button),
    Label(Label),
    CheckBox(CheckBox),
}

impl PlacedKind {
    fn new(kind: &str, host: &WebWindow, text: &str) -> Option<Self> {
        Some(match kind {
            "Button" => Self::Button(Button::new(host, text)),
            "Label" => Self::Label(Label::new(host, text)),
            "CheckBox" => Self::CheckBox(CheckBox::new(host, text)),
            _ => return None,
        })
    }

    fn kind_name(&self) -> &'static str {
        match self {
            Self::Button(_) => "Button",
            Self::Label(_) => "Label",
            Self::CheckBox(_) => "CheckBox",
        }
    }

    fn pos(&self) -> (i32, i32) {
        match self {
            Self::Button(c) => c.pos(),
            Self::Label(c) => c.pos(),
            Self::CheckBox(c) => c.pos(),
        }
    }

    fn set_pos(&mut self, x: i32, y: i32) {
        match self {
            Self::Button(c) => c.set_pos(x, y),
            Self::Label(c) => c.set_pos(x, y),
            Self::CheckBox(c) => c.set_pos(x, y),
        }
    }

    fn width(&self) -> u32 {
        match self {
            Self::Button(c) => c.width(),
            Self::Label(c) => c.width(),
            Self::CheckBox(c) => c.width(),
        }
    }

    fn set_width(&mut self, width: u32) {
        match self {
            Self::Button(c) => c.set_width(width),
            Self::Label(c) => c.set_width(width),
            Self::CheckBox(c) => c.set_width(width),
        }
    }

    fn height(&self) -> u32 {
        match self {
            Self::Button(c) => c.height(),
            Self::Label(c) => c.height(),
            Self::CheckBox(c) => c.height(),
        }
    }

    fn set_height(&mut self, height: u32) {
        match self {
            Self::Button(c) => c.set_height(height),
            Self::Label(c) => c.set_height(height),
            Self::CheckBox(c) => c.set_height(height),
        }
    }

    fn text(&self) -> String {
        match self {
            Self::Button(c) => c.text(),
            Self::Label(c) => c.text(),
            Self::CheckBox(c) => c.text(),
        }
    }

    fn set_text(&mut self, text: &str) {
        match self {
            Self::Button(c) => c.set_text(text),
            Self::Label(c) => c.set_text(text),
            Self::CheckBox(c) => c.set_text(text),
        }
    }

    /// `None` for a kind that has no checked state at all (shows and
    /// disables the panel's Checked field); `Some` for one that does.
    fn checked(&self) -> Option<bool> {
        match self {
            Self::CheckBox(c) => Some(c.checked()),
            _ => None,
        }
    }

    fn set_checked(&mut self, checked: bool) {
        if let Self::CheckBox(c) = self {
            c.set_checked(checked);
        }
    }

    fn default_size(kind: &str) -> (u32, u32) {
        match kind {
            "Button" => (75, 23),
            "Label" => (60, 16),
            "CheckBox" => (90, 17),
            _ => (75, 23),
        }
    }
}

struct PlacedControl {
    name: String,
    control: PlacedKind,
    // The real DOM node `control` draws into — kept so a click on it
    // (which only hands us the raw element) can be matched back to this
    // entry, and so its selection box can be repositioned after an edit.
    element: HtmlElement,
}

/// What the properties panel is currently showing.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Selection {
    None,
    Form,
    Control(usize),
}

struct PropertyInputs {
    name: HtmlInputElement,
    text: HtmlInputElement,
    x: HtmlInputElement,
    y: HtmlInputElement,
    width: HtmlInputElement,
    height: HtmlInputElement,
    checked: HtmlInputElement,
    row_name: HtmlElement,
    row_text: HtmlElement,
    row_x: HtmlElement,
    row_y: HtmlElement,
    row_width: HtmlElement,
    row_height: HtmlElement,
    row_checked: HtmlElement,
}

impl PropertyInputs {
    fn get_input(document: &Document, id: &str) -> HtmlInputElement {
        document
            .get_element_by_id(id)
            .unwrap_or_else(|| panic!("index.html must have #{id}"))
            .dyn_into()
            .unwrap_or_else(|_| panic!("#{id} must be an <input>"))
    }

    fn get_row(document: &Document, id: &str) -> HtmlElement {
        document
            .get_element_by_id(id)
            .unwrap_or_else(|| panic!("index.html must have #{id}"))
            .dyn_into()
            .unwrap_or_else(|_| panic!("#{id} must be an HtmlElement"))
    }

    fn new(document: &Document) -> Self {
        Self {
            name: Self::get_input(document, "prop-name"),
            text: Self::get_input(document, "prop-text"),
            x: Self::get_input(document, "prop-x"),
            y: Self::get_input(document, "prop-y"),
            width: Self::get_input(document, "prop-width"),
            height: Self::get_input(document, "prop-height"),
            checked: Self::get_input(document, "prop-checked"),
            row_name: Self::get_row(document, "row-name"),
            row_text: Self::get_row(document, "row-text"),
            row_x: Self::get_row(document, "row-x"),
            row_y: Self::get_row(document, "row-y"),
            row_width: Self::get_row(document, "row-width"),
            row_height: Self::get_row(document, "row-height"),
            row_checked: Self::get_row(document, "row-checked"),
        }
    }
}

fn set_row_visible(row: &HtmlElement, visible: bool) {
    let class_list = row.class_list();
    if visible {
        class_list.remove_1("hidden").ok();
    } else {
        class_list.add_1("hidden").ok();
    }
}

/// Fills the panel from whatever `selection` currently points at, hiding
/// whole rows that don't apply (the form has no position of its own
/// yet, so X/Y don't show for it; Checked only shows for a control that
/// actually has a checked state) rather than just disabling them.
fn refresh_properties(
    inputs: &PropertyInputs,
    selection: Selection,
    window: &WebWindow,
    controls: &[PlacedControl],
) {
    let (name, text, x, y, width, height, has_pos, checked) = match selection {
        Selection::None => (
            String::new(), String::new(), String::new(), String::new(),
            String::new(), String::new(), false, None,
        ),
        Selection::Form => {
            let (w, h) = window.size();
            (
                "Form1".into(), window.title(), String::new(), String::new(),
                w.to_string(), h.to_string(), false, None,
            )
        }
        Selection::Control(i) => {
            let c = &controls[i];
            let (cx, cy) = c.control.pos();
            (
                c.name.clone(),
                c.control.text(),
                cx.to_string(),
                cy.to_string(),
                c.control.width().to_string(),
                c.control.height().to_string(),
                true,
                c.control.checked(),
            )
        }
    };
    inputs.name.set_value(&name);
    inputs.text.set_value(&text);
    inputs.x.set_value(&x);
    inputs.y.set_value(&y);
    inputs.width.set_value(&width);
    inputs.height.set_value(&height);
    inputs.checked.set_checked(checked.unwrap_or(false));

    let visible = !matches!(selection, Selection::None);
    set_row_visible(&inputs.row_name, visible);
    set_row_visible(&inputs.row_text, visible);
    set_row_visible(&inputs.row_x, visible && has_pos);
    set_row_visible(&inputs.row_y, visible && has_pos);
    set_row_visible(&inputs.row_width, visible);
    set_row_visible(&inputs.row_height, visible);
    set_row_visible(&inputs.row_checked, checked.is_some());
}

/// Pushes the panel's current field values onto whatever is selected.
fn apply_properties(
    inputs: &PropertyInputs,
    selection: Selection,
    window: &WebWindow,
    controls: &mut [PlacedControl],
    form_overlay: &HtmlElement,
    control_overlay: &HtmlElement,
) {
    match selection {
        Selection::None => {}
        Selection::Form => {
            window.set_title(&inputs.text.value());
            window.set_size(parse_u32(&inputs.width, 400), parse_u32(&inputs.height, 260));
            position_overlay(form_overlay, window.element());
        }
        Selection::Control(i) => {
            let c = &mut controls[i];
            c.control.set_text(&inputs.text.value());
            c.control.set_pos(parse_i32(&inputs.x, 0), parse_i32(&inputs.y, 0));
            c.control.set_width(parse_u32(&inputs.width, 75));
            c.control.set_height(parse_u32(&inputs.height, 23));
            if c.control.checked().is_some() {
                c.control.set_checked(inputs.checked.checked());
            }
            position_overlay(control_overlay, &c.element);
        }
    }
}

fn parse_i32(input: &HtmlInputElement, fallback: i32) -> i32 {
    input.value().trim().parse().unwrap_or(fallback)
}

fn parse_u32(input: &HtmlInputElement, fallback: u32) -> u32 {
    input.value().trim().parse().unwrap_or(fallback)
}

#[wasm_bindgen(start)]
pub fn start() {
    let dom_window = web_sys::window().expect("no window");
    let document = dom_window.document().expect("no document");

    let stage = document
        .get_element_by_id("rad-stage")
        .expect("index.html must have #rad-stage");

    let window = Rc::new(WebWindow::new(&stage, "Form1", 400, 260));
    let controls: Rc<RefCell<Vec<PlacedControl>>> = Rc::new(RefCell::new(Vec::new()));
    let selection: Rc<Cell<Selection>> = Rc::new(Cell::new(Selection::None));
    let inputs = Rc::new(PropertyInputs::new(&document));

    // Where within the palette tile the user grabbed it, so the drop
    // places the new control's top-left seamlessly instead of snapping
    // its corner to the cursor.
    let grab_offset: Rc<Cell<(i32, i32)>> = Rc::new(Cell::new((0, 0)));

    let body = window.body().clone();

    // Design-time selection: a dashed box + 8 handles, drawn over whatever
    // is selected. Purely visual (pointer-events:none in CSS) — no resize
    // behavior yet except on the form itself. One overlay lives in the
    // stage (for selecting the form), the other inside the form body
    // (for selecting a control).
    let (form_overlay, form_handles) = create_selection_overlay(&document, &stage);
    let (control_overlay, _control_handles) = create_selection_overlay(&document, &body);

    // Only the right/bottom/corner handles can resize the form without
    // also having to move it — growing from the left or top edge would
    // need the form to track a position too, which it doesn't yet.
    for (class, axis) in [("e", (true, false)), ("s", (false, true)), ("se", (true, true))] {
        if let Some(handle) = form_handles.iter().find(|(c, _)| *c == class).map(|(_, h)| h) {
            attach_resize_handle(handle, window.clone(), form_overlay.clone(), axis);
        }
    }

    let on_form_click = {
        let window = window.clone();
        let controls = controls.clone();
        let selection = selection.clone();
        let inputs = inputs.clone();
        let form_overlay = form_overlay.clone();
        let control_overlay = control_overlay.clone();
        Closure::wrap(Box::new(move |_: MouseEvent| {
            selection.set(Selection::Form);
            hide_overlay(&control_overlay);
            position_overlay(&form_overlay, window.element());
            refresh_properties(&inputs, selection.get(), &window, &controls.borrow());
        }) as Box<dyn FnMut(MouseEvent)>)
    };
    window
        .element()
        .add_event_listener_with_callback("click", on_form_click.as_ref().unchecked_ref())
        .ok();
    on_form_click.forget();

    // Event delegation: every placed control ends up as a real DOM
    // element directly in the body, so one listener catches clicks on
    // all of them (present and future) without wiring each one
    // individually. The clicked element is matched back to its
    // `PlacedControl` by identity.
    let on_control_click = {
        let window = window.clone();
        let controls = controls.clone();
        let selection = selection.clone();
        let inputs = inputs.clone();
        let form_overlay = form_overlay.clone();
        let control_overlay = control_overlay.clone();
        Closure::wrap(Box::new(move |e: MouseEvent| {
            let Some(target) = e.target().and_then(|t| t.dyn_into::<HtmlElement>().ok()) else {
                return;
            };
            let controls_ref = controls.borrow();
            let Some(index) = controls_ref.iter().position(|c| {
                Node::is_same_node(&c.element, Some(&target))
                    || c.element.contains(Some(&target))
            }) else {
                return;
            };
            e.stop_propagation();
            selection.set(Selection::Control(index));
            hide_overlay(&form_overlay);
            position_overlay(&control_overlay, &controls_ref[index].element);
            refresh_properties(&inputs, selection.get(), &window, &controls_ref);
        }) as Box<dyn FnMut(MouseEvent)>)
    };
    body.add_event_listener_with_callback("click", on_control_click.as_ref().unchecked_ref())
        .ok();
    on_control_click.forget();

    // Allow drop: the body — and, per spec, dragenter too — must cancel
    // the default, or "drop" never fires.
    let on_dragover = Closure::wrap(Box::new(move |e: DragEvent| {
        e.prevent_default();
        e.stop_propagation();
        if let Some(dt) = e.data_transfer() {
            dt.set_drop_effect("copy");
        }
    }) as Box<dyn FnMut(DragEvent)>);
    body.add_event_listener_with_callback("dragenter", on_dragover.as_ref().unchecked_ref())
        .ok();
    body.add_event_listener_with_callback("dragover", on_dragover.as_ref().unchecked_ref())
        .ok();
    on_dragover.forget();

    let on_drop = {
        let window = window.clone();
        let body = body.clone();
        let controls = controls.clone();
        let grab_offset = grab_offset.clone();
        Closure::wrap(Box::new(move |e: DragEvent| {
            e.prevent_default();
            let Some(kind) = e.data_transfer().and_then(|dt| dt.get_data("text/plain").ok())
            else {
                return;
            };

            let (grab_x, grab_y) = grab_offset.get();
            let x = e.offset_x() - grab_x;
            let y = e.offset_y() - grab_y;

            let same_kind_count = controls.borrow().iter().filter(|c| c.control.kind_name() == kind).count();
            let name = format!("{}{}", lowercase_first(&kind), same_kind_count + 1);

            let Some(mut control) = PlacedKind::new(&kind, &window, &name) else {
                return;
            };
            let (default_width, default_height) = PlacedKind::default_size(&kind);
            control.set_pos(x, y);
            control.set_width(default_width);
            control.set_height(default_height);

            // The control we just created is the last thing appended to
            // the body — grab its real element now, while that's still true.
            let element: HtmlElement = body
                .last_element_child()
                .expect("the control we just created is in the body")
                .dyn_into()
                .expect("a placed control's root is always an HtmlElement");

            controls.borrow_mut().push(PlacedControl { name, control, element });
        }) as Box<dyn FnMut(DragEvent)>)
    };
    body.add_event_listener_with_callback("drop", on_drop.as_ref().unchecked_ref())
        .ok();
    on_drop.forget();

    // Event delegation again: one "dragstart" listener for the whole
    // palette, reading which tile started the drag from its
    // `data-kind` attribute, rather than wiring each tile separately.
    let palette = document
        .get_element_by_id("palette")
        .expect("index.html must have #palette");
    let on_dragstart = {
        let grab_offset = grab_offset.clone();
        Closure::wrap(Box::new(move |e: DragEvent| {
            let Some(target) = e.target().and_then(|t| t.dyn_into::<HtmlElement>().ok()) else {
                return;
            };
            let Some(kind) = target.get_attribute("data-kind") else {
                return;
            };
            grab_offset.set((e.offset_x(), e.offset_y()));
            if let Some(dt) = e.data_transfer() {
                let _ = dt.set_data("text/plain", &kind);
                dt.set_effect_allowed("copy");
            }
        }) as Box<dyn FnMut(DragEvent)>)
    };
    palette
        .add_event_listener_with_callback("dragstart", on_dragstart.as_ref().unchecked_ref())
        .ok();
    on_dragstart.forget();

    // Properties panel: any field's "change" re-applies the whole set from
    // the inputs (simpler than tracking which single field fired, and just
    // as correct since the others are unchanged).
    for field in [&inputs.text, &inputs.x, &inputs.y, &inputs.width, &inputs.height, &inputs.checked] {
        let on_change = {
            let window = window.clone();
            let controls = controls.clone();
            let selection = selection.clone();
            let inputs = inputs.clone();
            let form_overlay = form_overlay.clone();
            let control_overlay = control_overlay.clone();
            Closure::wrap(Box::new(move |_: Event| {
                apply_properties(
                    &inputs,
                    selection.get(),
                    &window,
                    &mut controls.borrow_mut(),
                    &form_overlay,
                    &control_overlay,
                );
            }) as Box<dyn FnMut(Event)>)
        };
        field
            .add_event_listener_with_callback("change", on_change.as_ref().unchecked_ref())
            .ok();
        on_change.forget();
    }

    let save_button = document
        .get_element_by_id("save-button")
        .expect("index.html must have #save-button");
    let output: HtmlTextAreaElement = document
        .get_element_by_id("generated-code")
        .expect("index.html must have #generated-code")
        .dyn_into()
        .expect("#generated-code must be a <textarea>");
    let on_save = Closure::wrap(Box::new(move |_: MouseEvent| {
        output.set_value(&generate_designer_source(&controls.borrow()));
    }) as Box<dyn FnMut(MouseEvent)>);
    save_button
        .add_event_listener_with_callback("click", on_save.as_ref().unchecked_ref())
        .ok();
    on_save.forget();

    // Window must outlive this function; leaking is fine for this entry point.
    std::mem::forget(window);
}

fn lowercase_first(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        Some(first) => first.to_lowercase().collect::<String>() + chars.as_str(),
        None => String::new(),
    }
}

/// Builds a selection box (8 resize-handle squares around a dashed
/// border) hidden inside `parent`, ready for `position_overlay` to show
/// over whatever gets selected. Returns the overlay plus its handles, so
/// callers can wire up resize behavior on whichever ones apply.
fn create_selection_overlay(
    document: &Document,
    parent: &Element,
) -> (HtmlElement, Vec<(&'static str, HtmlElement)>) {
    let overlay: HtmlElement = document
        .create_element("div")
        .expect("create_element(\"div\")")
        .dyn_into()
        .unwrap();
    overlay.set_class_name("rad-selection");
    let mut handles = Vec::new();
    for handle_class in ["nw", "n", "ne", "e", "se", "s", "sw", "w"] {
        let handle: HtmlElement = document
            .create_element("div")
            .expect("create_element(\"div\")")
            .dyn_into()
            .unwrap();
        handle.set_class_name(&format!("rad-handle {handle_class}"));
        overlay.append_child(&handle).unwrap();
        handles.push((handle_class, handle));
    }
    parent.append_child(&overlay).unwrap();
    (overlay, handles)
}

#[derive(Clone, Copy)]
struct ResizeStart {
    client_x: i32,
    client_y: i32,
    width: u32,
    height: u32,
}

/// Wires one handle to resize `window` by pointer-drag. `axis` is
/// (grows_width, grows_height) — e.g. the south handle is `(false, true)`.
fn attach_resize_handle(
    handle: &HtmlElement,
    window: Rc<WebWindow>,
    overlay: HtmlElement,
    axis: (bool, bool),
) {
    let drag_start: Rc<Cell<Option<ResizeStart>>> = Rc::new(Cell::new(None));

    let on_down = {
        let handle = handle.clone();
        let window = window.clone();
        let drag_start = drag_start.clone();
        Closure::wrap(Box::new(move |e: PointerEvent| {
            e.stop_propagation();
            let _ = handle.set_pointer_capture(e.pointer_id());
            let (width, height) = window.size();
            drag_start.set(Some(ResizeStart {
                client_x: e.client_x(),
                client_y: e.client_y(),
                width,
                height,
            }));
        }) as Box<dyn FnMut(PointerEvent)>)
    };
    handle
        .add_event_listener_with_callback("pointerdown", on_down.as_ref().unchecked_ref())
        .ok();
    on_down.forget();

    let on_move = {
        let window = window.clone();
        let overlay = overlay.clone();
        let drag_start = drag_start.clone();
        Closure::wrap(Box::new(move |e: PointerEvent| {
            let Some(start) = drag_start.get() else { return };
            let dx = if axis.0 { e.client_x() - start.client_x } else { 0 };
            let dy = if axis.1 { e.client_y() - start.client_y } else { 0 };
            let width = (start.width as i32 + dx).max(40) as u32;
            let height = (start.height as i32 + dy).max(40) as u32;

            window.set_size(width, height);
            position_overlay(&overlay, window.element());
        }) as Box<dyn FnMut(PointerEvent)>)
    };
    handle
        .add_event_listener_with_callback("pointermove", on_move.as_ref().unchecked_ref())
        .ok();
    on_move.forget();

    let on_up = {
        let handle = handle.clone();
        let drag_start = drag_start.clone();
        Closure::wrap(Box::new(move |e: PointerEvent| {
            let _ = handle.release_pointer_capture(e.pointer_id());
            drag_start.set(None);
        }) as Box<dyn FnMut(PointerEvent)>)
    };
    handle
        .add_event_listener_with_callback("pointerup", on_up.as_ref().unchecked_ref())
        .ok();
    on_up.forget();
}

fn position_overlay(overlay: &HtmlElement, target: &HtmlElement) {
    overlay.class_list().add_1("visible").ok();
    overlay
        .style()
        .set_property("left", &format!("{}px", target.offset_left()))
        .ok();
    overlay
        .style()
        .set_property("top", &format!("{}px", target.offset_top()))
        .ok();
    overlay
        .style()
        .set_property("width", &format!("{}px", target.offset_width()))
        .ok();
    overlay
        .style()
        .set_property("height", &format!("{}px", target.offset_height()))
        .ok();
}

fn hide_overlay(overlay: &HtmlElement) {
    overlay.class_list().remove_1("visible").ok();
}

/// Renders the placed controls as a `form1.designer.rs` body — the same
/// shape `rad-demo`'s hand-split file already uses.
fn generate_designer_source(controls: &[PlacedControl]) -> String {
    let mut kinds_used: Vec<&'static str> = controls.iter().map(|c| c.control.kind_name()).collect();
    kinds_used.sort_unstable();
    kinds_used.dedup();

    let mut src = String::new();
    src.push_str("//! GENERATED by the designer. Do not hand-edit — regenerated on every save.\n");
    src.push_str("//! Add event handlers and other logic in `form1.rs`, not here.\n\n");
    if kinds_used.is_empty() {
        src.push_str("use rad_core::Window;\n\n");
    } else {
        src.push_str(&format!("use rad_core::{{{}, Window}};\n\n", kinds_used.join(", ")));
    }
    src.push_str("pub struct Form1Controls {\n");
    for c in controls {
        src.push_str(&format!("    pub {}: {},\n", c.name, c.control.kind_name()));
    }
    src.push_str("}\n\n");
    src.push_str("pub fn build(window: &Window) -> Form1Controls {\n");
    for c in controls {
        let (x, y) = c.control.pos();
        let width = c.control.width();
        let height = c.control.height();
        let text = c.control.text();
        let kind = c.control.kind_name();
        let name = &c.name;
        src.push_str(&format!(
            "    let mut {name} = {kind}::new(window, \"{text}\");\n\
             \x20   {name}.set_pos({x}, {y});\n\
             \x20   {name}.set_width({width});\n\
             \x20   {name}.set_height({height});\n",
        ));
        if let Some(checked) = c.control.checked() {
            src.push_str(&format!("    {name}.set_checked({checked});\n"));
        }
        src.push('\n');
    }
    src.push_str("    Form1Controls {\n");
    for c in controls {
        src.push_str(&format!("        {},\n", c.name));
    }
    src.push_str("    }\n}\n");
    src
}
