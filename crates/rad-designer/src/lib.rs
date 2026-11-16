//! Minimal designer: drag a palette tile (Button, Label, CheckBox,
//! TextBox, or RadioButton) onto a resizable preview `Window`; placed
//! controls are repositionable by pointer-drag (built into
//! `rad-backend-web`). `form1.designer.rs` regenerates automatically on
//! every form change (polled, not event-driven — see `start()`), and
//! double-clicking a control or the form stubs out its default event
//! handler in `form1.rs`, which the designer only ever appends to.
//! Both live in `files` (currently `localStorage` — see that module) and
//! open as real tabs with a Monaco editor, same as the built-in
//! "Designer" tab holding the toolbox/stage/properties/file list.
//!
//! Adding a further control kind to the palette means: one more
//! `PlacedKind` variant, one line in each of its match arms, and a
//! palette tile in `index.html` — the pos/size/text plumbing underneath
//! is already shared via `rad-core`'s `WidgetHandle`.
//!
//! The live form sits in an `<iframe>` (`frame.html`) in the designer
//! tab's stage column, and this module does its work from the copy
//! loaded *by that iframe*, not from `index.html` — see `start()` for
//! why. The designer chrome (palette, properties, file list, tab bar,
//! skin switcher) is reached through `window.parent`, and the Monaco
//! editors `monaco-bridge.js` creates live there too, for the same
//! reason `chrome_element` below does unchecked casts.

mod build;
mod files;
mod native_build;

use rad_backend_web::WebWindow;
use rad_core::{
    Button, CheckBox, Label, PictureBox, ProgressBar, RadioButton, TextBox, TreeView,
    WindowHandle,
};
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::{
    Blob, BlobPropertyBag, Document, DragEvent, Element, Event, HtmlAnchorElement, HtmlElement,
    HtmlInputElement, HtmlSelectElement, HtmlTextAreaElement, MouseEvent, Node, PointerEvent, Url,
};

/// Thin bindings to `monaco-bridge.js`, which drives real Monaco editor
/// instances mounted in the parent page from this module's own (the
/// iframe's) realm. See that file for why every DOM access in it goes
/// through `window.parent`.
mod monaco {
    use wasm_bindgen::prelude::*;
    use web_sys::Element;

    #[wasm_bindgen(module = "/monaco-bridge.js")]
    extern "C" {
        pub fn init(panels: &Element);
        #[wasm_bindgen(js_name = setOnChange)]
        pub fn set_on_change(callback: &JsValue);
        #[wasm_bindgen(js_name = openFile)]
        pub fn open_file(path: &str, content: &str, read_only: bool);
        pub fn show(path: &str);
        #[wasm_bindgen(js_name = setContent)]
        pub fn set_content(path: &str, content: &str);
        #[wasm_bindgen(js_name = revealLine)]
        pub fn reveal_line(path: &str, line: u32);
        #[wasm_bindgen(js_name = closeFile)]
        pub fn close_file(path: &str);
    }
}

/// Thin bindings to `build-pipeline.js`, which drives the in-browser
/// `rustc` (see `.research/spike` and `rad-backend-web-min`'s doc
/// comment) to compile the designer's current project to a standalone
/// app `.wasm`. Called from this module's own realm, same as everything
/// else here — the toolchain has no DOM dependency (only `Worker`,
/// `SharedArrayBuffer`, and Cache Storage), so unlike `monaco` there's no
/// realm-mismatch reason it couldn't run in either one; it's only ever
/// called from here because that's where `start()` already is.
mod build_pipeline {
    use js_sys::Map;
    use wasm_bindgen::prelude::*;

    #[wasm_bindgen(module = "/build-pipeline.js")]
    extern "C" {
        #[wasm_bindgen(js_name = buildApp, catch)]
        pub async fn build_app(files: &Map, on_progress: &JsValue) -> Result<JsValue, JsValue>;
    }
}

/// Thin bindings to `native-build-pipeline.js` — same role as
/// `build_pipeline` above, for the native `.exe` path (see
/// `native_build.rs`'s doc comment, and `.research/spike-exe/` for why
/// this needed its own research thread separate from the wasm one).
mod native_build_pipeline {
    use js_sys::Map;
    use wasm_bindgen::prelude::*;

    #[wasm_bindgen(module = "/native-build-pipeline.js")]
    extern "C" {
        #[wasm_bindgen(js_name = buildNativeApp, catch)]
        pub async fn build_native_app(files: &Map, triple: &str, on_progress: &JsValue) -> Result<JsValue, JsValue>;
    }
}

/// One of the control types the palette can place. Each variant is just a
/// `rad-core` wrapper type — this enum exists only so the designer can
/// hold "whatever got placed" in one `Vec`, since Rust has no common
/// concrete type for "any widget".
enum PlacedKind {
    Button(Button),
    Label(Label),
    CheckBox(CheckBox),
    TextBox(TextBox),
    RadioButton(RadioButton),
    ProgressBar(ProgressBar),
    PictureBox(PictureBox),
    TreeView(TreeView),
}

impl PlacedKind {
    fn new(kind: &str, host: &WebWindow, text: &str) -> Option<Self> {
        Some(match kind {
            "Button" => Self::Button(Button::new(host, text)),
            "Label" => Self::Label(Label::new(host, text)),
            "CheckBox" => Self::CheckBox(CheckBox::new(host, text)),
            "TextBox" => Self::TextBox(TextBox::new(host, text)),
            // Grouping isn't exposed in the designer yet, so every radio
            // placed this way shares one group.
            "RadioButton" => Self::RadioButton(RadioButton::new(host, "group1", text)),
            "ProgressBar" => Self::ProgressBar(ProgressBar::new(host)),
            "PictureBox" => Self::PictureBox(PictureBox::new(host)),
            "TreeView" => Self::TreeView(TreeView::new(host)),
            _ => return None,
        })
    }

    fn kind_name(&self) -> &'static str {
        match self {
            Self::Button(_) => "Button",
            Self::Label(_) => "Label",
            Self::CheckBox(_) => "CheckBox",
            Self::TextBox(_) => "TextBox",
            Self::RadioButton(_) => "RadioButton",
            Self::ProgressBar(_) => "ProgressBar",
            Self::PictureBox(_) => "PictureBox",
            Self::TreeView(_) => "TreeView",
        }
    }

    fn pos(&self) -> (i32, i32) {
        match self {
            Self::Button(c) => c.pos(),
            Self::Label(c) => c.pos(),
            Self::CheckBox(c) => c.pos(),
            Self::TextBox(c) => c.pos(),
            Self::RadioButton(c) => c.pos(),
            Self::ProgressBar(c) => c.pos(),
            Self::PictureBox(c) => c.pos(),
            Self::TreeView(c) => c.pos(),
        }
    }

    fn set_pos(&mut self, x: i32, y: i32) {
        match self {
            Self::Button(c) => c.set_pos(x, y),
            Self::Label(c) => c.set_pos(x, y),
            Self::CheckBox(c) => c.set_pos(x, y),
            Self::TextBox(c) => c.set_pos(x, y),
            Self::RadioButton(c) => c.set_pos(x, y),
            Self::ProgressBar(c) => c.set_pos(x, y),
            Self::PictureBox(c) => c.set_pos(x, y),
            Self::TreeView(c) => c.set_pos(x, y),
        }
    }

    fn width(&self) -> u32 {
        match self {
            Self::Button(c) => c.width(),
            Self::Label(c) => c.width(),
            Self::CheckBox(c) => c.width(),
            Self::TextBox(c) => c.width(),
            Self::RadioButton(c) => c.width(),
            Self::ProgressBar(c) => c.width(),
            Self::PictureBox(c) => c.width(),
            Self::TreeView(c) => c.width(),
        }
    }

    fn set_width(&mut self, width: u32) {
        match self {
            Self::Button(c) => c.set_width(width),
            Self::Label(c) => c.set_width(width),
            Self::CheckBox(c) => c.set_width(width),
            Self::TextBox(c) => c.set_width(width),
            Self::RadioButton(c) => c.set_width(width),
            Self::ProgressBar(c) => c.set_width(width),
            Self::PictureBox(c) => c.set_width(width),
            Self::TreeView(c) => c.set_width(width),
        }
    }

    fn height(&self) -> u32 {
        match self {
            Self::Button(c) => c.height(),
            Self::Label(c) => c.height(),
            Self::CheckBox(c) => c.height(),
            Self::TextBox(c) => c.height(),
            Self::RadioButton(c) => c.height(),
            Self::ProgressBar(c) => c.height(),
            Self::PictureBox(c) => c.height(),
            Self::TreeView(c) => c.height(),
        }
    }

    fn set_height(&mut self, height: u32) {
        match self {
            Self::Button(c) => c.set_height(height),
            Self::Label(c) => c.set_height(height),
            Self::CheckBox(c) => c.set_height(height),
            Self::TextBox(c) => c.set_height(height),
            Self::RadioButton(c) => c.set_height(height),
            Self::ProgressBar(c) => c.set_height(height),
            Self::PictureBox(c) => c.set_height(height),
            Self::TreeView(c) => c.set_height(height),
        }
    }

    /// `None` for a kind with no plain text property at all (shows and
    /// disables the panel's Text field) — `ProgressBar`/`PictureBox`/
    /// `TreeView` have their own properties (Value/Image path/Items)
    /// instead.
    fn text(&self) -> Option<String> {
        match self {
            Self::Button(c) => Some(c.text()),
            Self::Label(c) => Some(c.text()),
            Self::CheckBox(c) => Some(c.text()),
            Self::TextBox(c) => Some(c.text()),
            Self::RadioButton(c) => Some(c.text()),
            Self::ProgressBar(_) | Self::PictureBox(_) | Self::TreeView(_) => None,
        }
    }

    fn set_text(&mut self, text: &str) {
        match self {
            Self::Button(c) => c.set_text(text),
            Self::Label(c) => c.set_text(text),
            Self::CheckBox(c) => c.set_text(text),
            Self::TextBox(c) => c.set_text(text),
            Self::RadioButton(c) => c.set_text(text),
            Self::ProgressBar(_) | Self::PictureBox(_) | Self::TreeView(_) => {}
        }
    }

    /// `None` for a kind that has no checked state at all (shows and
    /// disables the panel's Checked field); `Some` for one that does.
    fn checked(&self) -> Option<bool> {
        match self {
            Self::CheckBox(c) => Some(c.checked()),
            Self::RadioButton(c) => Some(c.checked()),
            _ => None,
        }
    }

    fn set_checked(&mut self, checked: bool) {
        match self {
            Self::CheckBox(c) => c.set_checked(checked),
            Self::RadioButton(c) => c.set_checked(checked),
            _ => {}
        }
    }

    /// `Some` only for `ProgressBar` — shows/hides the panel's Value row.
    fn value(&self) -> Option<u32> {
        match self {
            Self::ProgressBar(c) => Some(c.value()),
            _ => None,
        }
    }

    fn set_value(&mut self, value: u32) {
        if let Self::ProgressBar(c) = self {
            c.set_value(value);
        }
    }

    /// `Some` only for `PictureBox` — shows/hides the panel's Image
    /// path row.
    fn image_path(&self) -> Option<String> {
        match self {
            Self::PictureBox(c) => Some(c.image_path()),
            _ => None,
        }
    }

    fn set_image_path(&mut self, path: &str) {
        if let Self::PictureBox(c) = self {
            c.set_image_path(path);
        }
    }

    /// `Some` only for `TreeView` — shows/hides the panel's Items row.
    /// Each item's depth is encoded as its line's leading-space count
    /// divided by two, matching `format_items_text`'s inverse.
    fn items_text(&self) -> Option<String> {
        match self {
            Self::TreeView(c) => Some(format_items_text(&c.items())),
            _ => None,
        }
    }

    fn set_items_text(&mut self, text: &str) {
        if let Self::TreeView(c) = self {
            c.set_items(&parse_items_text(text));
        }
    }

    fn default_size(kind: &str) -> (u32, u32) {
        match kind {
            "Button" => (75, 23),
            "Label" => (60, 16),
            "CheckBox" => (90, 17),
            "TextBox" => (100, 20),
            "RadioButton" => (90, 17),
            "ProgressBar" => (150, 23),
            "PictureBox" => (100, 50),
            "TreeView" => (121, 97),
            _ => (75, 23),
        }
    }
}

/// Two spaces of indent per depth level — simple to type and to read
/// back unambiguously, unlike tabs (which the `<textarea>` may or may
/// not insert literally depending on the browser).
fn format_items_text(items: &[rad_core::TreeItem]) -> String {
    items
        .iter()
        .map(|(depth, text)| format!("{}{}", "  ".repeat(*depth as usize), text))
        .collect::<Vec<_>>()
        .join("\n")
}

fn parse_items_text(text: &str) -> Vec<rad_core::TreeItem> {
    text.lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| {
            let leading_spaces = line.len() - line.trim_start_matches(' ').len();
            ((leading_spaces / 2) as u32, line.trim().to_string())
        })
        .collect()
}

/// The event double-clicking a kind in the designer stubs out — "click"
/// for every control today. A kind whose main job is firing on its own
/// schedule rather than being clicked (a `Timer`, once one exists) would
/// return something like "tick" here instead; nothing else about the
/// double-click flow below depends on which event it is.
fn default_event_name(_kind: &str) -> &'static str {
    "click"
}

/// One event handler stub the designer has created, from a double-click.
/// `Form` targets the window itself rather than a `PlacedControl`, since
/// the form isn't one of `Form1Controls`' fields.
enum HandlerTarget {
    Form,
    Control(String),
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
    value: HtmlInputElement,
    image_path: HtmlInputElement,
    items: HtmlTextAreaElement,
    row_name: HtmlElement,
    row_text: HtmlElement,
    row_x: HtmlElement,
    row_y: HtmlElement,
    row_width: HtmlElement,
    row_height: HtmlElement,
    row_checked: HtmlElement,
    row_value: HtmlElement,
    row_image_path: HtmlElement,
    row_items: HtmlElement,
}

/// Looks up an element of the designer chrome (`index.html`, the parent
/// page) by id. Unchecked cast on purpose: `dyn_into` is an `instanceof`
/// test against *this* module's realm — the iframe's — which a parent-page
/// element never passes, even when it really is the expected type.
fn chrome_element<T: JsCast>(chrome: &Document, id: &str) -> T {
    chrome
        .get_element_by_id(id)
        .unwrap_or_else(|| panic!("index.html must have #{id}"))
        .unchecked_into()
}

/// Reveals the bottom build-console panel (`index.html`'s `#build-console`)
/// and clears whatever it was last showing — called at the start of a
/// build so output from this run doesn't sit mixed in with a previous
/// one's.
fn open_console(chrome: &Document) {
    let panel: HtmlElement = chrome_element(chrome, "build-console");
    panel.class_list().add_1("visible").ok();
    chrome_element::<HtmlElement>(chrome, "build-console-log").set_text_content(Some(""));
}

/// Appends a line to the build console and scrolls it to the bottom —
/// the only way a user can watch `rustc`/`lld` output here, since this
/// pipeline runs in a web worker with no terminal of its own.
fn log_console(chrome: &Document, line: &str) {
    let log: HtmlElement = chrome_element(chrome, "build-console-log");
    let existing = log.text_content().unwrap_or_default();
    log.set_text_content(Some(&format!("{existing}{line}\n")));
    log.set_scroll_top(log.scroll_height());
}

/// A build failure is a plain JS `Error` (its message is the compiler's
/// own stdout/stderr) — this is the only way to get its text back out
/// as a `String` to print into the console panel.
fn js_error_text(err: &JsValue) -> String {
    err.as_string()
        .or_else(|| err.dyn_ref::<js_sys::Error>().map(|e| String::from(e.message())))
        .unwrap_or_else(|| format!("{err:?}"))
}

impl PropertyInputs {
    fn new(chrome: &Document) -> Self {
        Self {
            name: chrome_element(chrome, "prop-name"),
            text: chrome_element(chrome, "prop-text"),
            x: chrome_element(chrome, "prop-x"),
            y: chrome_element(chrome, "prop-y"),
            width: chrome_element(chrome, "prop-width"),
            height: chrome_element(chrome, "prop-height"),
            checked: chrome_element(chrome, "prop-checked"),
            value: chrome_element(chrome, "prop-value"),
            image_path: chrome_element(chrome, "prop-image-path"),
            items: chrome_element(chrome, "prop-items"),
            row_name: chrome_element(chrome, "row-name"),
            row_text: chrome_element(chrome, "row-text"),
            row_x: chrome_element(chrome, "row-x"),
            row_y: chrome_element(chrome, "row-y"),
            row_width: chrome_element(chrome, "row-width"),
            row_height: chrome_element(chrome, "row-height"),
            row_checked: chrome_element(chrome, "row-checked"),
            row_value: chrome_element(chrome, "row-value"),
            row_image_path: chrome_element(chrome, "row-image-path"),
            row_items: chrome_element(chrome, "row-items"),
        }
    }
}

/// The designer chrome elements tabs and files need to reach, bundled so
/// closures created well after `start()`'s setup (a new tab button, a
/// file-list row) can each just clone one `Rc` instead of five.
struct ChromeRefs {
    chrome: Document,
    tab_bar: Element,
    tab_panels: HtmlElement,
    designer_view: HtmlElement,
}

/// Shows either the "Designer" tab's own layout or (for anything else) the
/// Monaco pane for that file, and reflects the choice in the tab bar.
fn switch_tab(refs: &Rc<ChromeRefs>, name: &str) {
    if name == "designer" {
        refs.designer_view.style().set_property("display", "grid").ok();
        refs.tab_panels.style().set_property("display", "none").ok();
    } else {
        refs.designer_view.style().set_property("display", "none").ok();
        refs.tab_panels.style().set_property("display", "grid").ok();
        monaco::show(name);
    }
    let children = refs.tab_bar.children();
    for i in 0..children.length() {
        if let Some(child) = children.item(i) {
            let active = child.get_attribute("data-tab").as_deref() == Some(name);
            let class_list = child.class_list();
            if active {
                class_list.add_1("active").ok();
            } else {
                class_list.remove_1("active").ok();
            }
        }
    }
}

/// Opens `name` as a Monaco-backed tab (creating its tab button and pane
/// the first time) and switches to it. `read_only` marks a generated file
/// so Monaco blocks edits to it — `form1.designer.rs`, never `form1.rs`.
fn open_file_tab(refs: &Rc<ChromeRefs>, name: &str, read_only: bool) {
    let content = files::read(name).unwrap_or_default();
    monaco::open_file(name, &content, read_only);

    let selector = format!("[data-tab='{name}']");
    if refs.tab_bar.query_selector(&selector).ok().flatten().is_none() {
        let tab: HtmlElement = refs
            .chrome
            .create_element("div")
            .expect("create_element(\"div\")")
            .unchecked_into();
        tab.set_class_name("tab");
        tab.set_attribute("data-tab", name).ok();

        let label: HtmlElement = refs
            .chrome
            .create_element("span")
            .expect("create_element(\"span\")")
            .unchecked_into();
        label.set_text_content(Some(name));
        tab.append_child(&label).ok();

        let close: HtmlElement = refs
            .chrome
            .create_element("span")
            .expect("create_element(\"span\")")
            .unchecked_into();
        close.set_class_name("tab-close");
        close.set_text_content(Some("\u{2715}"));
        tab.append_child(&close).ok();

        refs.tab_bar.append_child(&tab).ok();

        let on_click = {
            let refs = refs.clone();
            let name = name.to_string();
            Closure::wrap(Box::new(move |_: MouseEvent| {
                switch_tab(&refs, &name);
            }) as Box<dyn FnMut(MouseEvent)>)
        };
        tab.add_event_listener_with_callback("click", on_click.as_ref().unchecked_ref())
            .ok();
        on_click.forget();

        let on_close = {
            let refs = refs.clone();
            let name = name.to_string();
            let tab = tab.clone();
            Closure::wrap(Box::new(move |e: MouseEvent| {
                e.stop_propagation();
                monaco::close_file(&name);
                tab.remove();
                switch_tab(&refs, "designer");
            }) as Box<dyn FnMut(MouseEvent)>)
        };
        close
            .add_event_listener_with_callback("click", on_close.as_ref().unchecked_ref())
            .ok();
        on_close.forget();
    }
    switch_tab(refs, name);
}

/// Rebuilds the file-list panel from whatever's in `files` right now.
/// `file_list` is the `#file-list` div inside `#files` — just the rows,
/// not the header/"+" button above them, so re-rendering never disturbs
/// that button's own listener.
fn render_file_list(refs: &Rc<ChromeRefs>, file_list: &Element) {
    file_list.set_inner_html("");
    for name in files::list() {
        let item: HtmlElement = refs
            .chrome
            .create_element("div")
            .expect("create_element(\"div\")")
            .unchecked_into();
        item.set_class_name("file-item");

        let label: HtmlElement = refs
            .chrome
            .create_element("span")
            .expect("create_element(\"span\")")
            .unchecked_into();
        label.set_class_name("file-name");
        label.set_text_content(Some(&name));
        item.append_child(&label).ok();

        let delete_button: HtmlElement = refs
            .chrome
            .create_element("span")
            .expect("create_element(\"span\")")
            .unchecked_into();
        delete_button.set_class_name("file-delete");
        delete_button.set_text_content(Some("\u{2715}"));
        item.append_child(&delete_button).ok();

        let on_click = {
            let refs = refs.clone();
            // `form1.designer.rs` is generated — opened read-only so an
            // edit can't be silently clobbered by the next auto-regen.
            let read_only = name == "form1.designer.rs";
            let name = name.clone();
            Closure::wrap(Box::new(move |_: MouseEvent| {
                open_file_tab(&refs, &name, read_only);
            }) as Box<dyn FnMut(MouseEvent)>)
        };
        item.add_event_listener_with_callback("click", on_click.as_ref().unchecked_ref())
            .ok();
        on_click.forget();

        let on_delete = {
            let refs = refs.clone();
            let file_list = file_list.clone();
            let name = name.clone();
            Closure::wrap(Box::new(move |e: MouseEvent| {
                e.stop_propagation();
                let confirmed = web_sys::window()
                    .and_then(|w| w.confirm_with_message(&format!("Delete {name}?")).ok())
                    .unwrap_or(false);
                if !confirmed {
                    return;
                }
                files::remove(&name);
                close_file_tab(&refs, &name);
                render_file_list(&refs, &file_list);
            }) as Box<dyn FnMut(MouseEvent)>)
        };
        delete_button
            .add_event_listener_with_callback("click", on_delete.as_ref().unchecked_ref())
            .ok();
        on_delete.forget();

        file_list.append_child(&item).ok();
    }
}

/// Closes `name`'s tab if it's open, falling back to the Designer tab if
/// that was the active one — shared by a file's own delete button and
/// its tab's close ("x") button.
fn close_file_tab(refs: &Rc<ChromeRefs>, name: &str) {
    let selector = format!("[data-tab='{name}']");
    if let Some(tab) = refs.tab_bar.query_selector(&selector).ok().flatten() {
        let was_active = tab.class_list().contains("active");
        monaco::close_file(name);
        tab.remove();
        if was_active {
            switch_tab(refs, "designer");
        }
    }
}

/// Prompts for a new file's name and creates it empty, ready to edit —
/// the only way to add a file the designer didn't generate itself.
fn new_file(refs: &Rc<ChromeRefs>, file_list: &Element) {
    let Some(name) = web_sys::window()
        .and_then(|w| w.prompt_with_message("New file name:").ok())
        .flatten()
        .map(|n| n.trim().to_string())
        .filter(|n| !n.is_empty())
    else {
        return;
    };
    if files::exists(&name) {
        web_sys::window()
            .and_then(|w| w.alert_with_message(&format!("{name} already exists.")).ok());
        return;
    }
    files::write(&name, "");
    render_file_list(refs, file_list);
    open_file_tab(refs, &name, false);
}

const SKINS: [&str; 3] = ["windows", "macos", "linux"];

/// Restyles the live form for `skin` (a `#skin-select` value) by swapping
/// a `rad-skin-*` class on the preview document's root, which `skins.css`
/// keys off. "web" — or anything unrecognized — leaves no skin class at
/// all, i.e. the browser's own native rendering.
fn apply_skin(preview_root: &Element, skin: &str) {
    let class_list = preview_root.class_list();
    for s in SKINS {
        class_list.remove_1(&format!("rad-skin-{s}")).ok();
    }
    if SKINS.contains(&skin) {
        class_list.add_1(&format!("rad-skin-{skin}")).ok();
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
    let (name, text, x, y, width, height, has_pos, checked, value, image_path, items_text) =
        match selection {
            Selection::None => (
                String::new(), None, String::new(), String::new(),
                String::new(), String::new(), false, None, None, None, None,
            ),
            Selection::Form => {
                let (w, h) = window.size();
                (
                    "Form1".into(), Some(window.title()), String::new(), String::new(),
                    w.to_string(), h.to_string(), false, None, None, None, None,
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
                    c.control.value(),
                    c.control.image_path(),
                    c.control.items_text(),
                )
            }
        };
    inputs.name.set_value(&name);
    inputs.text.set_value(text.as_deref().unwrap_or_default());
    inputs.x.set_value(&x);
    inputs.y.set_value(&y);
    inputs.width.set_value(&width);
    inputs.height.set_value(&height);
    inputs.checked.set_checked(checked.unwrap_or(false));
    inputs.value.set_value(&value.map(|v| v.to_string()).unwrap_or_default());
    inputs.image_path.set_value(image_path.as_deref().unwrap_or_default());
    inputs.items.set_value(items_text.as_deref().unwrap_or_default());

    let visible = !matches!(selection, Selection::None);
    set_row_visible(&inputs.row_name, visible);
    set_row_visible(&inputs.row_text, text.is_some());
    set_row_visible(&inputs.row_x, visible && has_pos);
    set_row_visible(&inputs.row_y, visible && has_pos);
    set_row_visible(&inputs.row_width, visible);
    set_row_visible(&inputs.row_height, visible);
    set_row_visible(&inputs.row_checked, checked.is_some());
    set_row_visible(&inputs.row_value, value.is_some());
    set_row_visible(&inputs.row_image_path, image_path.is_some());
    set_row_visible(&inputs.row_items, items_text.is_some());
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
            if c.control.value().is_some() {
                c.control.set_value(parse_u32(&inputs.value, 0).min(100));
            }
            if c.control.image_path().is_some() {
                c.control.set_image_path(&inputs.image_path.value());
            }
            if c.control.items_text().is_some() {
                c.control.set_items_text(&inputs.items.value());
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

/// Runs inside the preview iframe (`frame.html`). It can't run in the
/// parent page and reach *into* the iframe instead: wasm-bindgen's
/// `dyn_into` is an `instanceof` check against the realm that loaded this
/// module, so `rad-backend-web`'s own casts on the form's elements (e.g.
/// in `WebWindow::new`) would fail for elements created by the iframe's
/// document. Loaded from the iframe, the form is same-realm and only our
/// own lookups into the parent page need unchecked casts (`chrome_element`).
#[wasm_bindgen(start)]
pub fn start() {
    let dom_window = web_sys::window().expect("no window");
    // trunk's own loader in index.html instantiates this module in the
    // top-level page as well; only frame.html's copy is the designer.
    if dom_window.frame_element().ok().flatten().is_none() {
        return;
    }
    let preview = dom_window.document().expect("no document");
    let chrome = dom_window
        .parent()
        .ok()
        .flatten()
        .and_then(|parent| parent.document())
        .expect("frame.html must be loaded inside index.html's #preview iframe");

    let stage = preview
        .get_element_by_id("rad-stage")
        .expect("frame.html must have #rad-stage");

    let window = Rc::new(WebWindow::new(&stage, "Form1", 400, 260));
    let controls: Rc<RefCell<Vec<PlacedControl>>> = Rc::new(RefCell::new(Vec::new()));
    let selection: Rc<Cell<Selection>> = Rc::new(Cell::new(Selection::None));
    let inputs = Rc::new(PropertyInputs::new(&chrome));

    let refs = Rc::new(ChromeRefs {
        chrome: chrome.clone(),
        tab_bar: chrome_element(&chrome, "tab-bar"),
        tab_panels: chrome_element(&chrome, "tab-panels"),
        designer_view: chrome_element(&chrome, "designer-view"),
    });
    monaco::init(&refs.tab_panels);

    let on_file_change = Closure::wrap(Box::new(move |path: String, content: String| {
        // The only file Monaco lets anyone type into is `form1.rs`
        // (`form1.designer.rs` opens read-only), so this is always a
        // real hand-edit to persist, not feedback from our own
        // `set_content` calls below.
        files::write(&path, &content);
    }) as Box<dyn FnMut(String, String)>);
    monaco::set_on_change(on_file_change.as_ref());
    on_file_change.forget();

    {
        let on_click = {
            let refs = refs.clone();
            Closure::wrap(Box::new(move |_: MouseEvent| switch_tab(&refs, "designer"))
                as Box<dyn FnMut(MouseEvent)>)
        };
        let tab_designer: HtmlElement = chrome_element(&chrome, "tab-designer");
        tab_designer
            .add_event_listener_with_callback("click", on_click.as_ref().unchecked_ref())
            .ok();
        on_click.forget();
    }

    // Seed both project files so the file list isn't empty on first
    // load, then keep `form1.designer.rs` in lockstep with the form
    // from here on — polled rather than hooked into every mutation site,
    // since a control's position/size during a pointer-drag is read
    // live from the DOM and has no "drag finished" event to hang this on.
    files::write("form1.designer.rs", &generate_designer_source(&controls.borrow()));
    if files::read("form1.rs").is_none() {
        files::write("form1.rs", &initial_form1_rs());
    }
    let file_list: Element = chrome_element(&chrome, "file-list");
    render_file_list(&refs, &file_list);

    {
        let on_click = {
            let refs = refs.clone();
            let file_list = file_list.clone();
            Closure::wrap(Box::new(move |_: MouseEvent| new_file(&refs, &file_list))
                as Box<dyn FnMut(MouseEvent)>)
        };
        let new_file_button: HtmlElement = chrome_element(&chrome, "new-file-button");
        new_file_button
            .add_event_listener_with_callback("click", on_click.as_ref().unchecked_ref())
            .ok();
        on_click.forget();
    }

    let on_regen_tick = {
        let controls = controls.clone();
        Closure::wrap(Box::new(move || {
            let src = generate_designer_source(&controls.borrow());
            if files::read("form1.designer.rs").as_deref() != Some(src.as_str()) {
                files::write("form1.designer.rs", &src);
                monaco::set_content("form1.designer.rs", &src);
            }
        }) as Box<dyn FnMut()>)
    };
    dom_window
        .set_interval_with_callback_and_timeout_and_arguments_0(
            on_regen_tick.as_ref().unchecked_ref(),
            1000,
        )
        .ok();
    on_regen_tick.forget();

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
    let (form_overlay, form_handles) = create_selection_overlay(&stage);
    let (control_overlay, _control_handles) = create_selection_overlay(&body);

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

    let on_form_dblclick = {
        let refs = refs.clone();
        Closure::wrap(Box::new(move |_: MouseEvent| {
            open_handler(&refs, HandlerTarget::Form, "click", "form1");
        }) as Box<dyn FnMut(MouseEvent)>)
    };
    window
        .element()
        .add_event_listener_with_callback("dblclick", on_form_dblclick.as_ref().unchecked_ref())
        .ok();
    on_form_dblclick.forget();

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

    let on_control_dblclick = {
        let controls = controls.clone();
        let refs = refs.clone();
        Closure::wrap(Box::new(move |e: MouseEvent| {
            let Some(target) = e.target().and_then(|t| t.dyn_into::<HtmlElement>().ok()) else {
                return;
            };
            let controls_ref = controls.borrow();
            let Some(control) = controls_ref.iter().find(|c| {
                Node::is_same_node(&c.element, Some(&target)) || c.element.contains(Some(&target))
            }) else {
                return;
            };
            e.stop_propagation();
            let event = default_event_name(control.control.kind_name());
            open_handler(&refs, HandlerTarget::Control(control.name.clone()), event, &control.name);
        }) as Box<dyn FnMut(MouseEvent)>)
    };
    body.add_event_listener_with_callback("dblclick", on_control_dblclick.as_ref().unchecked_ref())
        .ok();
    on_control_dblclick.forget();

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
    // The palette is in the parent page while the drop lands in the
    // iframe; same-origin HTML5 drag-and-drop crosses that boundary
    // natively, and `grab_offset` is shared since it's all one module.
    let palette: Element = chrome_element(&chrome, "palette");
    let on_dragstart = {
        let grab_offset = grab_offset.clone();
        Closure::wrap(Box::new(move |e: DragEvent| {
            // Parent-page node, so a node-type test rather than a
            // (cross-realm, always-failing) `dyn_into` — text-selection
            // drags start on a Text node, which has no attributes.
            let Some(target) = e.target().map(|t| t.unchecked_into::<Node>()) else {
                return;
            };
            if target.node_type() != Node::ELEMENT_NODE {
                return;
            }
            let Some(kind) = target.unchecked_into::<Element>().get_attribute("data-kind") else {
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
            let chrome = chrome.clone();
            Closure::wrap(Box::new(move |_: Event| {
                apply_properties(
                    &inputs,
                    selection.get(),
                    &window,
                    &mut controls.borrow_mut(),
                    &form_overlay,
                    &control_overlay,
                );
                // The form's title also sets the *page's* tab title — but
                // `WebWindow::set_title` only reaches the iframe's own
                // document, which has no visible tab of its own. Only the
                // designer needs this extra hop; `WebWindow` itself is
                // correct as-is for a non-embedded app (`rad-demo`), where
                // its own document IS the top-level one.
                if selection.get() == Selection::Form {
                    chrome.set_title(&inputs.text.value());
                }
            }) as Box<dyn FnMut(Event)>)
        };
        field
            .add_event_listener_with_callback("change", on_change.as_ref().unchecked_ref())
            .ok();
        on_change.forget();
    }

    let skin_select: HtmlSelectElement = chrome_element(&chrome, "skin-select");
    let preview_root = preview.document_element().expect("frame.html has an <html>");
    // Apply whatever the select shows now, not a hard-coded default: the
    // browser may restore a previous choice into the select on reload.
    apply_skin(&preview_root, &skin_select.value());
    let on_skin_change = {
        let skin_select = skin_select.clone();
        let window = window.clone();
        let controls = controls.clone();
        let selection = selection.clone();
        let inputs = inputs.clone();
        let form_overlay = form_overlay.clone();
        let control_overlay = control_overlay.clone();
        Closure::wrap(Box::new(move |_: Event| {
            apply_skin(&preview_root, &skin_select.value());
            // A skin changes fonts, paddings and title-bar height, so the
            // selection box and panel (which read sizes back from the DOM)
            // must be re-synced to the restyled layout.
            let controls = controls.borrow();
            match selection.get() {
                Selection::None => {}
                Selection::Form => position_overlay(&form_overlay, window.element()),
                Selection::Control(i) => position_overlay(&control_overlay, &controls[i].element),
            }
            refresh_properties(&inputs, selection.get(), &window, &controls);
        }) as Box<dyn FnMut(Event)>)
    };
    skin_select
        .add_event_listener_with_callback("change", on_skin_change.as_ref().unchecked_ref())
        .ok();
    on_skin_change.forget();

    // One Build button; `#build-target` says what it builds. Its values are
    // "web" or a native rustc target triple (see `build_target_info`).
    let build_button: HtmlElement = chrome_element(&chrome, "build-button");
    let on_build_click = {
        let chrome = chrome.clone();
        let window = window.clone();
        Closure::wrap(Box::new(move |_: MouseEvent| {
            let target = chrome_element::<HtmlSelectElement>(&chrome, "build-target").value();
            if target == "web" {
                wasm_bindgen_futures::spawn_local(run_build(chrome.clone(), window.clone()));
            } else {
                wasm_bindgen_futures::spawn_local(run_native_build(chrome.clone(), window.clone(), target));
            }
        }) as Box<dyn FnMut(MouseEvent)>)
    };
    build_button
        .add_event_listener_with_callback("click", on_build_click.as_ref().unchecked_ref())
        .ok();
    on_build_click.forget();

    let console_close_button: HtmlElement = chrome_element(&chrome, "build-console-close");
    let on_console_close_click = {
        let chrome = chrome.clone();
        Closure::wrap(Box::new(move |_: MouseEvent| {
            let panel: HtmlElement = chrome_element(&chrome, "build-console");
            panel.class_list().remove_1("visible").ok();
        }) as Box<dyn FnMut(MouseEvent)>)
    };
    console_close_button
        .add_event_listener_with_callback("click", on_console_close_click.as_ref().unchecked_ref())
        .ok();
    on_console_close_click.forget();

    let reset_button: HtmlElement = chrome_element(&chrome, "reset-button");
    let on_reset_click = Closure::wrap(Box::new(move |_: MouseEvent| {
        let confirmed = web_sys::window()
            .and_then(|w| w.confirm_with_message("Clear all project files and reload?").ok())
            .unwrap_or(false);
        if !confirmed {
            return;
        }
        files::clear_all();
        if let Some(w) = web_sys::window() {
            w.location().reload().ok();
        }
    }) as Box<dyn FnMut(MouseEvent)>);
    reset_button
        .add_event_listener_with_callback("click", on_reset_click.as_ref().unchecked_ref())
        .ok();
    on_reset_click.forget();

    // Window must outlive this function; leaking is fine for this entry point.
    std::mem::forget(window);
}

/// What each `#build-target` option builds: a human-readable label for status
/// and console messages, and the name of the downloaded file. The values are
/// "web" or a rustc target triple served by `native-build-pipeline.js`
/// (`NATIVE_TARGETS` there must list every triple here).
fn build_target_info(value: &str) -> (&'static str, &'static str) {
    match value {
        "web" => ("web app", "form1.html"),
        "x86_64-pc-windows-gnullvm" => ("Windows x64", "form1-x64.exe"),
        "i686-pc-windows-gnullvm" => ("Windows x86", "form1-x86.exe"),
        "aarch64-pc-windows-gnullvm" => ("Windows ARM64", "form1-arm64.exe"),
        // Unknown value: build_native_app rejects it with a clear message.
        _ => ("unknown target", "form1.exe"),
    }
}

/// Compiles the project currently in `files` (see that module) through
/// the in-browser toolchain (`build-pipeline.js`, `rad-backend-web-min`)
/// and downloads the result as a standalone `.html` — see `build.rs`'s
/// doc comment for why that pipeline can't use this designer's own
/// `rad-backend-web`/`wasm-bindgen`-based backend.
async fn run_build(chrome: Document, window: Rc<WebWindow>) {
    let status: HtmlElement = chrome_element(&chrome, "build-status");
    let (label, file_name) = build_target_info("web");
    let starting = format!("Building {label}...");
    status.set_text_content(Some(&starting));
    open_console(&chrome);
    log_console(&chrome, &starting);

    let form1_rs = files::read("form1.rs").unwrap_or_else(initial_form1_rs);
    let form1_designer_rs = files::read("form1.designer.rs").unwrap_or_default();
    let (width, height) = window.size();
    let main_rs = build::generate_main_source(width, height);
    let mount = build::build_mount_files(&main_rs, &form1_rs, &form1_designer_rs);

    let on_progress = {
        let status = status.clone();
        let chrome = chrome.clone();
        Closure::wrap(Box::new(move |message: String| {
            status.set_text_content(Some(&message));
            log_console(&chrome, &message);
        }) as Box<dyn FnMut(String)>)
    };
    let result = build_pipeline::build_app(&mount, on_progress.as_ref()).await;
    drop(on_progress);

    match result {
        Ok(value) => {
            let bytes = js_sys::Uint8Array::new(&value).to_vec();
            let html = build::generate_standalone_html(&bytes, &build::runtime_js());
            let downloading = format!("Downloading {file_name}...");
            status.set_text_content(Some(&downloading));
            log_console(&chrome, &downloading);
            trigger_download(&chrome, file_name, &html);
            status.set_text_content(Some("Build complete."));
            log_console(&chrome, "Build complete.");
        }
        Err(err) => {
            web_sys::console::error_1(&err);
            log_console(&chrome, &js_error_text(&err));
            status.set_text_content(Some("Build failed — see console below."));
        }
    }
}

/// Hands `content` to the browser as a real file download — a `Blob`
/// behind an object URL, clicked via a throwaway anchor, same mechanism
/// every "download this" button on the web uses.
fn trigger_download(document: &Document, filename: &str, content: &str) {
    let parts = js_sys::Array::new();
    parts.push(&JsValue::from_str(content));
    let options = BlobPropertyBag::new();
    options.set_type("text/html");
    let blob = Blob::new_with_str_sequence_and_options(&parts, &options)
        .expect("Blob::new should not fail");
    let url = Url::create_object_url_with_blob(&blob).expect("create_object_url should not fail");

    let anchor: HtmlAnchorElement = document
        .create_element("a")
        .expect("create_element(\"a\")")
        .unchecked_into();
    anchor.set_href(&url);
    anchor.set_download(filename);
    let body = document.body().expect("document has a body");
    body.append_child(&anchor).ok();
    anchor.click();
    body.remove_child(&anchor).ok();
    Url::revoke_object_url(&url).ok();
}

/// Same as `trigger_download`, for a binary file (the compiled `.exe`)
/// rather than text — `Blob` needs a different constructor for raw bytes,
/// and a generic MIME type rather than `text/html`.
fn trigger_download_bytes(document: &Document, filename: &str, content: &[u8]) {
    let parts = js_sys::Array::new();
    parts.push(&js_sys::Uint8Array::from(content));
    let options = BlobPropertyBag::new();
    options.set_type("application/octet-stream");
    let blob = Blob::new_with_u8_array_sequence_and_options(&parts, &options)
        .expect("Blob::new should not fail");
    let url = Url::create_object_url_with_blob(&blob).expect("create_object_url should not fail");

    let anchor: HtmlAnchorElement = document
        .create_element("a")
        .expect("create_element(\"a\")")
        .unchecked_into();
    anchor.set_href(&url);
    anchor.set_download(filename);
    let body = document.body().expect("document has a body");
    body.append_child(&anchor).ok();
    anchor.click();
    body.remove_child(&anchor).ok();
    Url::revoke_object_url(&url).ok();
}

/// Mirrors `run_build`, compiling the same project to a native `.exe` for
/// `triple` instead of a downloadable `.html` — see `native_build.rs` and
/// `native_build_pipeline` for the rest of this path.
async fn run_native_build(chrome: Document, window: Rc<WebWindow>, triple: String) {
    let status: HtmlElement = chrome_element(&chrome, "build-status");
    let (label, exe_name) = build_target_info(&triple);
    let starting = format!("Building for {label}...");
    status.set_text_content(Some(&starting));
    open_console(&chrome);
    log_console(&chrome, &starting);

    let form1_rs = files::read("form1.rs").unwrap_or_else(initial_form1_rs);
    let form1_designer_rs = files::read("form1.designer.rs").unwrap_or_default();
    // The *content area* size, not `window.size()`'s outer chrome
    // total — a real Win32 title bar is a different height than this
    // preview's own hand-drawn one, so matching content size (not
    // outer size) is what keeps every control's position lined up
    // between the preview and the native output.
    let (width, height) = window.body_size();
    let main_rs = native_build::generate_native_main_source(width, height);
    let mount = native_build::build_native_mount_files(&main_rs, &form1_rs, &form1_designer_rs);

    let on_progress = {
        let status = status.clone();
        let chrome = chrome.clone();
        Closure::wrap(Box::new(move |message: String| {
            status.set_text_content(Some(&message));
            log_console(&chrome, &message);
        }) as Box<dyn FnMut(String)>)
    };
    let result = native_build_pipeline::build_native_app(&mount, &triple, on_progress.as_ref()).await;
    drop(on_progress);

    match result {
        Ok(value) => {
            let bytes = js_sys::Uint8Array::new(&value).to_vec();
            let downloading = format!("Downloading {exe_name}...");
            status.set_text_content(Some(&downloading));
            log_console(&chrome, &downloading);
            trigger_download_bytes(&chrome, exe_name, &bytes);
            status.set_text_content(Some("Build complete."));
            log_console(&chrome, "Build complete.");
        }
        Err(err) => {
            web_sys::console::error_1(&err);
            log_console(&chrome, &js_error_text(&err));
            status.set_text_content(Some(&format!("Build for {label} failed — see console below.")));
        }
    }
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
fn create_selection_overlay(parent: &Element) -> (HtmlElement, Vec<(&'static str, HtmlElement)>) {
    // `parent`'s own document, not whichever one is at hand: the overlay
    // must be created by the preview iframe's document to live in it.
    let document = parent.owner_document().expect("parent is attached to a document");
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

/// The boilerplate `form1.rs` starts from, the first time any handler is
/// requested. Handlers are methods on `Form1`, not bare functions, so
/// one can reach any sibling control through `self.controls.*` — the
/// same thing a WinForms handler gets for free from `this.*`. `Form1`
/// is `Rc`-shared (not owned by one place) specifically so every
/// handler closure can hold its own clone and still call back into it
/// after `build` returns; every control setter taking `&self` rather
/// than `&mut self` (see `WidgetHandle`) is what makes that safe.
fn initial_form1_rs() -> String {
    "//! Hand-written half of Form1. The designer only ever appends new\n\
     //! handler stubs below — it never rewrites a body you've edited.\n\n\
     #[path = \"form1.designer.rs\"]\n\
     mod designer;\n\n\
     pub use designer::Form1Controls;\n\n\
     use rad_core::Window;\n\
     use std::rc::Rc;\n\n\
     pub struct Form1 {\n\
     \x20   pub controls: Form1Controls,\n\
     }\n\n\
     pub fn build(window: &Window) -> Rc<Form1> {\n\
     \x20   let form = Rc::new(Form1 { controls: designer::build(window) });\n\
     \x20   form\n\
     }\n"
        .to_string()
}

/// Ensures `form1.rs` wires up and stubs out `fn_name` for `target`'s
/// event, appending to whatever's already there rather than regenerating
/// it — the file is user-editable, so its current content (read from
/// `files`) is the only copy of any edits made since it was last touched.
/// Returns the stub's 1-based line number, to scroll a freshly opened tab
/// to it; double-clicking the same control again is a no-op past this
/// point, since the stub already exists.
fn ensure_form_handler(target: &HandlerTarget, event: &'static str, fn_name: &str) -> u32 {
    let mut content = files::read("form1.rs").unwrap_or_else(initial_form1_rs);
    if !content.contains(&format!("fn {fn_name}(")) {
        // Each closure clones `form` for itself, so it keeps working
        // independently of whichever other handlers also captured one.
        let wiring = match target {
            HandlerTarget::Form => format!(
                "    window.on_{event}({{ let form = form.clone(); move || form.{fn_name}() }});\n"
            ),
            HandlerTarget::Control(name) => format!(
                // Can't shadow `form` inside the same statement that also
                // uses it as `form.controls.{name}`'s receiver — the
                // receiver's borrow and the closure's move conflict. A
                // separately-named clone sidesteps that entirely.
                "    {{\n        let {fn_name}_form = form.clone();\n        form.controls.{name}.set_on_{event}(Box::new(move || {fn_name}_form.{fn_name}()));\n    }}\n"
            ),
        };
        if let Some(at) = content.find("    form\n}") {
            content.insert_str(at, &wiring);
        } else {
            content.push_str(&wiring);
        }
        let target_desc = match target {
            HandlerTarget::Form => "the form".to_string(),
            HandlerTarget::Control(name) => name.clone(),
        };
        content.push_str(&format!(
            "\nimpl Form1 {{\n    pub fn {fn_name}(&self) {{\n        // TODO: handle {target_desc}'s `{event}`\n    }}\n}}\n"
        ));
        files::write("form1.rs", &content);
    }
    let at = content.find(&format!("fn {fn_name}(")).unwrap_or(0);
    content[..at].matches('\n').count() as u32 + 1
}

/// Opens `form1.rs` and scrolls to `base_name`'s stub for `event`,
/// creating it first if this is the first time it's been double-clicked.
fn open_handler(refs: &Rc<ChromeRefs>, target: HandlerTarget, event: &'static str, base_name: &str) {
    let fn_name = format!("{base_name}_{event}");
    let line = ensure_form_handler(&target, event, &fn_name);
    open_file_tab(refs, "form1.rs", false);
    monaco::reveal_line("form1.rs", line);
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
        let kind = c.control.kind_name();
        let name = &c.name;
        let ctor = match (&c.control, c.control.text()) {
            (PlacedKind::RadioButton(_), Some(text)) => {
                format!("{kind}::new(window, \"group1\", \"{text}\")")
            }
            (_, Some(text)) => format!("{kind}::new(window, \"{text}\")"),
            // `ProgressBar`/`PictureBox`/`TreeView` take no text.
            (_, None) => format!("{kind}::new(window)"),
        };
        src.push_str(&format!(
            "    let {name} = {ctor};\n\
             \x20   {name}.set_pos({x}, {y});\n\
             \x20   {name}.set_width({width});\n\
             \x20   {name}.set_height({height});\n",
        ));
        if let Some(checked) = c.control.checked() {
            src.push_str(&format!("    {name}.set_checked({checked});\n"));
        }
        if let Some(value) = c.control.value() {
            src.push_str(&format!("    {name}.set_value({value});\n"));
        }
        if let Some(path) = c.control.image_path().filter(|p| !p.is_empty()) {
            src.push_str(&format!("    {name}.set_image_path(\"{path}\");\n"));
        }
        if let Some(items) = c.control.items_text().filter(|t| !t.trim().is_empty()) {
            let items_literal = parse_items_text(&items)
                .iter()
                .map(|(depth, text)| format!("({depth}, \"{text}\".to_string())"))
                .collect::<Vec<_>>()
                .join(", ");
            src.push_str(&format!("    {name}.set_items(&[{items_literal}]);\n"));
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
