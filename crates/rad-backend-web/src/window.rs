use crate::{
    WebButtonHandle, WebCheckBoxHandle, WebLabelHandle, WebPictureBoxHandle,
    WebProgressBarHandle, WebRadioButtonHandle, WebTextBoxHandle, WebTreeViewHandle,
};
use rad_core::{
    Backend, ButtonHandle, CheckBoxHandle, Host, LabelHandle, PictureBoxHandle, ProgressBarHandle,
    RadioButtonHandle, TextBoxHandle, TimerHandle, TreeViewHandle, WindowHandle,
};
use std::cell::RefCell;
use std::rc::Rc;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::{Document, Element, HtmlElement, ResizeObserver, ResizeObserverEntry};

/// Mounts preview windows into a page element. Each window is a resizable
/// `<div>` chrome (title bar + body) — not a real OS window, since this
/// backend only ever runs inside a browser tab.
pub struct WebBackend {
    root: Element,
}

impl WebBackend {
    pub fn new(root: Element) -> Self {
        Self { root }
    }
}

impl Backend for WebBackend {
    fn create_window(&self, title: &str, width: u32, height: u32) -> Box<dyn WindowHandle> {
        Box::new(WebWindow::new(&self.root, title, width, height))
    }
}

type ResizeCallback = Rc<RefCell<Option<Box<dyn FnMut(u32, u32)>>>>;

pub struct WebWindow {
    document: Document,
    chrome: HtmlElement,
    title_bar: HtmlElement,
    body: Element,
    on_resize: ResizeCallback,
    // Keeps the JS-side observer and its closure alive for the window's
    // lifetime; dropping either would silently stop resize notifications.
    _observer: ResizeObserver,
    _closure: Closure<dyn FnMut(js_sys::Array)>,
}

impl WebWindow {
    pub fn new(root: &Element, title: &str, width: u32, height: u32) -> Self {
        let document = root.owner_document().expect("root must be attached to a document");

        let chrome: HtmlElement = document
            .create_element("div")
            .expect("create_element(\"div\")")
            .dyn_into()
            .unwrap();
        chrome.set_class_name("rad-window");
        // No native browser resize handle: resizing is design-time-only,
        // driven by the designer's own selection handles via `set_size`.
        chrome.style().set_property("overflow", "hidden").ok();
        chrome.style().set_property("position", "relative").ok();
        // Flex column so `body` fills whatever height is left after the
        // title bar, instead of collapsing to zero height when empty.
        chrome.style().set_property("display", "flex").ok();
        chrome.style().set_property("flex-direction", "column").ok();
        chrome
            .style()
            .set_property("width", &format!("{width}px"))
            .ok();
        chrome
            .style()
            .set_property("height", &format!("{height}px"))
            .ok();

        let title_bar: HtmlElement = document
            .create_element("div")
            .expect("create_element(\"div\")")
            .dyn_into()
            .unwrap();
        title_bar.set_class_name("rad-window-titlebar");
        title_bar.set_text_content(Some(title));
        title_bar.style().set_property("flex", "0 0 auto").ok();
        chrome.append_child(&title_bar).unwrap();

        let body: Element = document.create_element("div").expect("create_element(\"div\")");
        body.set_attribute("class", "rad-window-body").ok();
        let body_el: &HtmlElement = body.dyn_ref().unwrap();
        body_el.style().set_property("position", "relative").ok();
        body_el.style().set_property("flex", "1 1 auto").ok();
        chrome.append_child(&body).unwrap();

        root.append_child(&chrome).unwrap();

        let on_resize: ResizeCallback = Rc::new(RefCell::new(None));
        let on_resize_for_closure = on_resize.clone();
        let closure = Closure::wrap(Box::new(move |entries: js_sys::Array| {
            if let Some(entry) = entries.get(0).dyn_ref::<ResizeObserverEntry>() {
                let rect = entry.content_rect();
                if let Some(cb) = on_resize_for_closure.borrow_mut().as_mut() {
                    cb(rect.width().max(0.0) as u32, rect.height().max(0.0) as u32);
                }
            }
        }) as Box<dyn FnMut(js_sys::Array)>);

        let observer = ResizeObserver::new(closure.as_ref().unchecked_ref())
            .expect("ResizeObserver::new should not fail");
        observer.observe(&chrome);

        Self {
            document,
            chrome,
            title_bar,
            body,
            on_resize,
            _observer: observer,
            _closure: closure,
        }
    }

    /// The window's content area — the design-time drop target. Specific
    /// to this backend; not part of the generic `WindowHandle` trait.
    pub fn body(&self) -> &Element {
        &self.body
    }

    /// The content area's own size — excludes the (hand-drawn) title
    /// bar that `size()`/`WindowHandle::size` includes, unlike a real
    /// OS window's `ClientSize` vs `Size`. The native-exe build pipeline
    /// needs this one: a real Win32 title bar is a different height
    /// than this preview's simulated one, so matching *this* size
    /// (rather than the outer chrome total) is what keeps every
    /// control's position lined up between preview and native output.
    pub fn body_size(&self) -> (u32, u32) {
        let body: &HtmlElement = self.body.dyn_ref().unwrap();
        (body.offset_width().max(0) as u32, body.offset_height().max(0) as u32)
    }

    /// The window's whole outer element (title bar + body), e.g. for
    /// drawing a design-time selection box around the form itself.
    pub fn element(&self) -> &HtmlElement {
        &self.chrome
    }
}

impl WindowHandle for WebWindow {
    fn set_title(&self, title: &str) {
        self.title_bar.set_text_content(Some(title));
        // When this app is the whole page (not embedded in a tool like the
        // designer), the form's caption is the page's own: the browser tab
        // title.
        self.document.set_title(title);
    }

    fn title(&self) -> String {
        self.title_bar.text_content().unwrap_or_default()
    }

    fn set_size(&self, width: u32, height: u32) {
        self.chrome
            .style()
            .set_property("width", &format!("{width}px"))
            .ok();
        self.chrome
            .style()
            .set_property("height", &format!("{height}px"))
            .ok();
    }

    fn size(&self) -> (u32, u32) {
        (
            self.chrome.offset_width().max(0) as u32,
            self.chrome.offset_height().max(0) as u32,
        )
    }

    fn set_on_resize(&self, callback: Box<dyn FnMut(u32, u32)>) {
        *self.on_resize.borrow_mut() = Some(callback);
    }

    fn set_on_click(&self, mut callback: Box<dyn FnMut()>) {
        let closure = Closure::wrap(Box::new(move || callback()) as Box<dyn FnMut()>);
        self.chrome
            .add_event_listener_with_callback("click", closure.as_ref().unchecked_ref())
            .ok();
        closure.forget();
    }

    fn set_default_icon(&self) {
        // A generic filled square, inline as an SVG data URI — no asset
        // file to ship, same role a stock IDI_APPLICATION icon plays on
        // Win32. The form's icon is the page's own: the browser tab's
        // favicon.
        const DEFAULT_ICON_SVG: &str =
            "data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 16 16'%3E\
             %3Crect width='16' height='16' rx='3' fill='%232b579a'/%3E\
             %3Crect x='3' y='3' width='10' height='3' fill='white'/%3E\
             %3C/svg%3E";
        set_favicon_href(&self.document, DEFAULT_ICON_SVG);
    }
}

fn set_favicon_href(document: &Document, href: &str) {
    let link = match document.query_selector("link[rel~='icon']").ok().flatten() {
        Some(existing) => existing,
        None => {
            let Some(head) = document.head() else { return };
            let Ok(created) = document.create_element("link") else { return };
            created.set_attribute("rel", "icon").ok();
            let _ = head.append_child(&created);
            created
        }
    };
    link.set_attribute("href", href).ok();
}

impl Host for WebWindow {
    fn create_button(&self, text: &str) -> Box<dyn ButtonHandle> {
        let document = self.body.owner_document().expect("body has a document");
        let el = crate::create_element(&document, &self.body, "button");
        el.set_text_content(Some(text));
        Box::new(WebButtonHandle::new_draggable(el))
    }

    fn create_label(&self, text: &str) -> Box<dyn LabelHandle> {
        let document = self.body.owner_document().expect("body has a document");
        let el = crate::create_element(&document, &self.body, "span");
        el.set_text_content(Some(text));
        Box::new(WebLabelHandle::new_draggable(el))
    }

    fn create_checkbox(&self, text: &str) -> Box<dyn CheckBoxHandle> {
        let document = self.body.owner_document().expect("body has a document");
        Box::new(WebCheckBoxHandle::new_draggable(&document, &self.body, text, false))
    }

    fn create_textbox(&self, text: &str) -> Box<dyn TextBoxHandle> {
        let document = self.body.owner_document().expect("body has a document");
        Box::new(WebTextBoxHandle::new_draggable(&document, &self.body, text))
    }

    fn create_radio_button(&self, group: &str, text: &str) -> Box<dyn RadioButtonHandle> {
        let document = self.body.owner_document().expect("body has a document");
        Box::new(WebRadioButtonHandle::new_draggable(
            &document, &self.body, group, text, false,
        ))
    }

    fn create_progress_bar(&self) -> Box<dyn ProgressBarHandle> {
        let document = self.body.owner_document().expect("body has a document");
        let el = crate::create_element(&document, &self.body, "progress");
        el.set_attribute("max", "100").ok();
        el.set_attribute("value", "0").ok();
        Box::new(WebProgressBarHandle::new_draggable(el))
    }

    fn create_picture_box(&self) -> Box<dyn PictureBoxHandle> {
        let document = self.body.owner_document().expect("body has a document");
        let el = crate::create_element(&document, &self.body, "img");
        el.style().set_property("object-fit", "contain").ok();
        Box::new(WebPictureBoxHandle::new_draggable(el))
    }

    fn create_tree_view(&self) -> Box<dyn TreeViewHandle> {
        let document = self.body.owner_document().expect("body has a document");
        let el = crate::create_element(&document, &self.body, "div");
        el.style().set_property("overflow", "auto").ok();
        el.style().set_property("border", "1px solid #adadad").ok();
        el.style().set_property("background", "#fff").ok();
        Box::new(WebTreeViewHandle::new_draggable(el))
    }

    fn create_timer(&self) -> Box<dyn TimerHandle> {
        Box::new(crate::WebTimerHandle::new())
    }
}
