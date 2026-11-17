//! GTK 3 plumbing shared by every component's `#[cfg(rad_gtk)]` implementation:
//! hand-declared FFI to libgtk-3 / libgobject / libglib (no crates, so the same
//! source builds under Cargo and as plain mounted source), a widget base that
//! positions children on a `GtkFixed`, signal plumbing, and colours via CSS.
//! The backend itself only owns the top-level window and the main loop.
//!
//! Handlers only fire for user actions: every setter that can raise a GTK
//! signal runs inside [`programmatic`], which mutes [`Slot::fire`].

use crate::Color;
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::ffi::{c_char, c_int, c_uint, c_ulong, c_void, CStr, CString};
use std::rc::Rc;

pub mod ffi {
    use super::*;

    pub type GtkWidget = c_void;
    pub type GPointer = *mut c_void;
    pub type GCallback = *const c_void;
    pub type DestroyNotify = Option<unsafe extern "C" fn(GPointer, *mut c_void)>;

    pub const GTK_ORIENTATION_HORIZONTAL: c_int = 0;
    pub const GTK_POS_BOTTOM: c_int = 3;
    pub const GTK_SHADOW_IN: c_int = 3;
    pub const GTK_POLICY_AUTOMATIC: c_int = 1;
    pub const GTK_ALIGN_START: c_int = 1;
    pub const GDK_BUTTON_PRESS_MASK: c_int = 256;
    pub const G_TYPE_STRING: usize = 16 << 2;
    pub const GTK_STYLE_PROVIDER_PRIORITY_APPLICATION: c_uint = 600;

    #[repr(C)]
    pub struct GtkTreeIter {
        pub stamp: c_int,
        pub user_data: *mut c_void,
        pub user_data2: *mut c_void,
        pub user_data3: *mut c_void,
    }

    #[link(name = "glib-2.0")]
    unsafe extern "C" {
        pub fn g_timeout_add_full(
            priority: c_int,
            interval: c_uint,
            function: unsafe extern "C" fn(GPointer) -> c_int,
            data: GPointer,
            notify: Option<unsafe extern "C" fn(GPointer)>,
        ) -> c_uint;
        pub fn g_source_remove(tag: c_uint) -> c_int;
        pub fn g_list_free(list: *mut c_void);
        pub fn g_free(mem: *mut c_void);
    }

    #[link(name = "gobject-2.0")]
    unsafe extern "C" {
        pub fn g_signal_connect_data(
            instance: GPointer,
            signal: *const c_char,
            handler: GCallback,
            data: GPointer,
            destroy: DestroyNotify,
            flags: c_int,
        ) -> c_ulong;
        pub fn g_object_ref_sink(object: GPointer) -> GPointer;
    }

    #[link(name = "gdk_pixbuf-2.0")]
    unsafe extern "C" {}

    #[link(name = "gtk-3")]
    unsafe extern "C" {
        pub fn gtk_init_check(argc: *mut c_int, argv: *mut *mut *mut c_char) -> c_int;
        pub fn gtk_main();
        pub fn gtk_main_quit();

        // Windows and containers.
        pub fn gtk_window_new(kind: c_int) -> *mut GtkWidget;
        pub fn gtk_window_set_title(window: *mut GtkWidget, title: *const c_char);
        pub fn gtk_window_get_title(window: *mut GtkWidget) -> *const c_char;
        pub fn gtk_window_set_default_size(window: *mut GtkWidget, width: c_int, height: c_int);
        pub fn gtk_window_resize(window: *mut GtkWidget, width: c_int, height: c_int);
        pub fn gtk_window_get_size(window: *mut GtkWidget, width: *mut c_int, height: *mut c_int);
        pub fn gtk_container_add(container: *mut GtkWidget, child: *mut GtkWidget);
        pub fn gtk_container_get_children(container: *mut GtkWidget) -> *mut c_void;
        pub fn gtk_event_box_new() -> *mut GtkWidget;
        pub fn gtk_fixed_new() -> *mut GtkWidget;
        pub fn gtk_fixed_put(fixed: *mut GtkWidget, child: *mut GtkWidget, x: c_int, y: c_int);
        pub fn gtk_fixed_move(fixed: *mut GtkWidget, child: *mut GtkWidget, x: c_int, y: c_int);
        pub fn gtk_scrolled_window_new(h: *mut c_void, v: *mut c_void) -> *mut GtkWidget;
        pub fn gtk_scrolled_window_set_shadow_type(window: *mut GtkWidget, kind: c_int);
        pub fn gtk_scrolled_window_set_policy(window: *mut GtkWidget, h: c_int, v: c_int);

        // Any widget.
        pub fn gtk_widget_show(widget: *mut GtkWidget);
        pub fn gtk_widget_show_all(widget: *mut GtkWidget);
        pub fn gtk_widget_destroy(widget: *mut GtkWidget);
        pub fn gtk_widget_set_size_request(widget: *mut GtkWidget, width: c_int, height: c_int);
        pub fn gtk_widget_add_events(widget: *mut GtkWidget, events: c_int);
        pub fn gtk_widget_set_halign(widget: *mut GtkWidget, align: c_int);
        pub fn gtk_widget_get_style_context(widget: *mut GtkWidget) -> *mut c_void;
        pub fn gtk_css_provider_new() -> *mut c_void;
        pub fn gtk_css_provider_load_from_data(
            provider: *mut c_void,
            data: *const c_char,
            length: isize,
            error: *mut *mut c_void,
        ) -> c_int;
        pub fn gtk_style_context_add_provider(context: *mut c_void, provider: *mut c_void, priority: c_uint);

        // Text controls.
        pub fn gtk_label_new(text: *const c_char) -> *mut GtkWidget;
        pub fn gtk_label_set_text(label: *mut GtkWidget, text: *const c_char);
        pub fn gtk_label_get_text(label: *mut GtkWidget) -> *const c_char;
        pub fn gtk_label_set_xalign(label: *mut GtkWidget, xalign: f32);
        pub fn gtk_label_set_yalign(label: *mut GtkWidget, yalign: f32);
        pub fn gtk_button_new_with_label(text: *const c_char) -> *mut GtkWidget;
        pub fn gtk_button_set_label(button: *mut GtkWidget, text: *const c_char);
        pub fn gtk_button_get_label(button: *mut GtkWidget) -> *const c_char;
        pub fn gtk_check_button_new_with_label(text: *const c_char) -> *mut GtkWidget;
        pub fn gtk_radio_button_new_with_label(group: *mut c_void, text: *const c_char) -> *mut GtkWidget;
        pub fn gtk_radio_button_new_with_label_from_widget(
            member: *mut GtkWidget,
            text: *const c_char,
        ) -> *mut GtkWidget;
        pub fn gtk_toggle_button_set_active(button: *mut GtkWidget, active: c_int);
        pub fn gtk_toggle_button_get_active(button: *mut GtkWidget) -> c_int;
        pub fn gtk_entry_new() -> *mut GtkWidget;
        pub fn gtk_entry_set_text(entry: *mut GtkWidget, text: *const c_char);
        pub fn gtk_entry_get_text(entry: *mut GtkWidget) -> *const c_char;

        // Display controls.
        pub fn gtk_progress_bar_new() -> *mut GtkWidget;
        pub fn gtk_progress_bar_set_fraction(bar: *mut GtkWidget, fraction: f64);
        pub fn gtk_image_new() -> *mut GtkWidget;
        pub fn gtk_image_set_from_file(image: *mut GtkWidget, path: *const c_char);

        // Range (trackbar).
        pub fn gtk_scale_new_with_range(orientation: c_int, min: f64, max: f64, step: f64) -> *mut GtkWidget;
        pub fn gtk_scale_set_draw_value(scale: *mut GtkWidget, draw: c_int);
        pub fn gtk_scale_clear_marks(scale: *mut GtkWidget);
        pub fn gtk_scale_add_mark(scale: *mut GtkWidget, value: f64, position: c_int, markup: *const c_char);
        pub fn gtk_range_set_range(range: *mut GtkWidget, min: f64, max: f64);
        pub fn gtk_range_set_increments(range: *mut GtkWidget, step: f64, page: f64);
        pub fn gtk_range_set_value(range: *mut GtkWidget, value: f64);
        pub fn gtk_range_get_value(range: *mut GtkWidget) -> f64;

        // Selection controls.
        pub fn gtk_combo_box_text_new() -> *mut GtkWidget;
        pub fn gtk_combo_box_text_append_text(combo: *mut GtkWidget, text: *const c_char);
        pub fn gtk_combo_box_text_remove_all(combo: *mut GtkWidget);
        pub fn gtk_combo_box_set_active(combo: *mut GtkWidget, index: c_int);
        pub fn gtk_combo_box_get_active(combo: *mut GtkWidget) -> c_int;
        pub fn gtk_list_box_new() -> *mut GtkWidget;
        pub fn gtk_list_box_insert(list: *mut GtkWidget, child: *mut GtkWidget, position: c_int);
        pub fn gtk_list_box_select_row(list: *mut GtkWidget, row: *mut GtkWidget);
        pub fn gtk_list_box_unselect_all(list: *mut GtkWidget);
        pub fn gtk_list_box_get_row_at_index(list: *mut GtkWidget, index: c_int) -> *mut GtkWidget;
        pub fn gtk_list_box_get_selected_row(list: *mut GtkWidget) -> *mut GtkWidget;
        pub fn gtk_list_box_row_get_index(row: *mut GtkWidget) -> c_int;

        // Tree view.
        pub fn gtk_tree_store_newv(n_columns: c_int, types: *const usize) -> *mut c_void;
        pub fn gtk_tree_store_append(store: *mut c_void, iter: *mut GtkTreeIter, parent: *const GtkTreeIter);
        pub fn gtk_tree_store_set(store: *mut c_void, iter: *mut GtkTreeIter, ...);
        pub fn gtk_tree_store_clear(store: *mut c_void);
        pub fn gtk_tree_view_new_with_model(model: *mut c_void) -> *mut GtkWidget;
        pub fn gtk_tree_view_set_headers_visible(view: *mut GtkWidget, visible: c_int);
        pub fn gtk_tree_view_append_column(view: *mut GtkWidget, column: *mut c_void) -> c_int;
        pub fn gtk_tree_view_expand_all(view: *mut GtkWidget);
        pub fn gtk_tree_view_column_new_with_attributes(
            title: *const c_char,
            renderer: *mut c_void,
            ...
        ) -> *mut c_void;
        pub fn gtk_cell_renderer_text_new() -> *mut c_void;
    }
}

use ffi::GtkWidget;

/// What a component needs from its host to create a control: the `GtkFixed`
/// that positions children.
#[derive(Clone, Copy)]
pub struct Parent {
    pub fixed: *mut GtkWidget,
}

pub fn init_once() {
    thread_local!(static DONE: Cell<bool> = const { Cell::new(false) });
    DONE.with(|done| {
        if !done.replace(true) {
            unsafe { ffi::gtk_init_check(std::ptr::null_mut(), std::ptr::null_mut()) };
        }
    });
}

pub fn cstr(s: &str) -> CString {
    CString::new(s.replace('\0', "")).unwrap_or_default()
}

pub fn from_c(ptr: *const c_char) -> String {
    if ptr.is_null() { String::new() } else { unsafe { CStr::from_ptr(ptr) }.to_string_lossy().into_owned() }
}

// --- Signals ---

thread_local! {
    static PROGRAMMATIC: Cell<u32> = const { Cell::new(0) };
}

/// Runs `f` with user-event handlers muted: GTK emits "changed" and friends for
/// code-driven changes too, but components only report what the user did.
pub fn programmatic<R>(f: impl FnOnce() -> R) -> R {
    PROGRAMMATIC.with(|depth| depth.set(depth.get() + 1));
    let result = f();
    PROGRAMMATIC.with(|depth| depth.set(depth.get() - 1));
    result
}

/// A replaceable handler. The signal is connected once and always calls
/// through its slot, so installing a new handler (even from inside the old
/// one) never touches GObject's handler list.
#[derive(Default)]
pub struct Slot {
    callback: RefCell<Option<Box<dyn FnMut()>>>,
}

impl Slot {
    pub fn set(&self, callback: Box<dyn FnMut()>) {
        *self.callback.borrow_mut() = Some(callback);
    }

    pub fn fire(&self) {
        if PROGRAMMATIC.with(Cell::get) > 0 {
            return;
        }
        // Taken out while it runs so the handler may install a replacement.
        let taken = self.callback.borrow_mut().take();
        if let Some(mut callback) = taken {
            callback();
            if self.callback.borrow().is_none() {
                *self.callback.borrow_mut() = Some(callback);
            }
        }
    }
}

unsafe extern "C" fn plain_trampoline(_widget: *mut c_void, data: *mut c_void) {
    unsafe { (*(data as *const Slot)).fire() };
}

unsafe extern "C" fn event_trampoline(_widget: *mut c_void, _event: *mut c_void, data: *mut c_void) -> c_int {
    unsafe { (*(data as *const Slot)).fire() };
    0 // not handled: let it propagate
}

unsafe extern "C" fn drop_slot(data: *mut c_void, _closure: *mut c_void) {
    drop(unsafe { Rc::from_raw(data as *const Slot) });
}

/// Connects a signal with signature `void (GtkWidget*, gpointer)` to `slot`.
pub fn connect(widget: *mut GtkWidget, signal: &str, slot: &Rc<Slot>) {
    let name = cstr(signal);
    unsafe {
        ffi::g_signal_connect_data(
            widget,
            name.as_ptr(),
            plain_trampoline as *const c_void,
            Rc::into_raw(slot.clone()) as *mut c_void,
            Some(drop_slot),
            0,
        );
    }
}

/// Connects a GDK event signal (`gboolean (GtkWidget*, GdkEvent*, gpointer)`).
pub fn connect_event(widget: *mut GtkWidget, signal: &str, slot: &Rc<Slot>) {
    let name = cstr(signal);
    unsafe {
        ffi::g_signal_connect_data(
            widget,
            name.as_ptr(),
            event_trampoline as *const c_void,
            Rc::into_raw(slot.clone()) as *mut c_void,
            Some(drop_slot),
            0,
        );
    }
}

// --- Colours ---

fn css_color(c: Color) -> String {
    format!("#{:02x}{:02x}{:02x}", c.r, c.g, c.b)
}

/// Re-renders a widget's own CSS provider from its colours.
fn apply_colors(widget: *mut GtkWidget, provider: &Cell<*mut c_void>, fore: Option<Color>, back: Option<Color>) {
    unsafe {
        if provider.get().is_null() {
            let created = ffi::gtk_css_provider_new();
            ffi::gtk_style_context_add_provider(
                ffi::gtk_widget_get_style_context(widget),
                created,
                ffi::GTK_STYLE_PROVIDER_PRIORITY_APPLICATION,
            );
            provider.set(created);
        }
        let mut css = String::from("* {");
        if let Some(c) = fore {
            css.push_str(&format!(" color: {};", css_color(c)));
        }
        if let Some(c) = back {
            css.push_str(&format!(" background-color: {}; background-image: none;", css_color(c)));
        }
        css.push_str(" }");
        let data = cstr(&css);
        ffi::gtk_css_provider_load_from_data(provider.get(), data.as_ptr(), -1, std::ptr::null_mut());
    }
}

// --- Widget base ---

/// State every control shares: the native widget, where it sits on the form
/// and its click/change handler slots.
pub struct GtkBase {
    pub widget: *mut GtkWidget,
    parent: *mut GtkWidget,
    pub x: Cell<i32>,
    pub y: Cell<i32>,
    pub width: Cell<u32>,
    pub height: Cell<u32>,
    pub click: Rc<Slot>,
    pub change: Rc<Slot>,
    fore: Cell<Option<Color>>,
    back: Cell<Option<Color>>,
    provider: Cell<*mut c_void>,
}

impl GtkBase {
    /// Places `widget` on the form. `click_signal` is the widget's own "clicked"
    /// style signal (`void (widget, data)`); `None` means a plain mouse press,
    /// which needs a widget with its own GDK window (an event box).
    pub fn new(parent: &Parent, widget: *mut GtkWidget, click_signal: Option<&str>) -> Self {
        let base = Self {
            widget,
            parent: parent.fixed,
            x: Cell::new(0),
            y: Cell::new(0),
            width: Cell::new(80),
            height: Cell::new(24),
            click: Rc::new(Slot::default()),
            change: Rc::new(Slot::default()),
            fore: Cell::new(None),
            back: Cell::new(None),
            provider: Cell::new(std::ptr::null_mut()),
        };
        unsafe {
            ffi::gtk_fixed_put(parent.fixed, widget, 0, 0);
            ffi::gtk_widget_set_size_request(widget, 80, 24);
            ffi::gtk_widget_show(widget);
            match click_signal {
                Some(signal) => connect(widget, signal, &base.click),
                None => {
                    ffi::gtk_widget_add_events(widget, ffi::GDK_BUTTON_PRESS_MASK);
                    connect_event(widget, "button-press-event", &base.click);
                }
            }
        }
        base
    }

    pub fn apply_rect(&self) {
        unsafe {
            ffi::gtk_fixed_move(self.parent, self.widget, self.x.get(), self.y.get());
            ffi::gtk_widget_set_size_request(self.widget, self.width.get() as c_int, self.height.get() as c_int);
        }
    }

    pub fn set_fore(&self, color: Option<Color>) {
        self.fore.set(color);
        apply_colors(self.widget, &self.provider, self.fore.get(), self.back.get());
    }

    pub fn set_back(&self, color: Option<Color>) {
        self.back.set(color);
        apply_colors(self.widget, &self.provider, self.fore.get(), self.back.get());
    }

    pub fn fore(&self) -> Option<Color> {
        self.fore.get()
    }

    pub fn back(&self) -> Option<Color> {
        self.back.get()
    }
}

macro_rules! impl_widget_handle {
    ($ty:ty, $field:ident) => {
        impl crate::WidgetHandle for $ty {
            fn set_pos(&self, x: i32, y: i32) {
                self.$field.x.set(x);
                self.$field.y.set(y);
                self.$field.apply_rect();
            }
            fn pos(&self) -> (i32, i32) {
                (self.$field.x.get(), self.$field.y.get())
            }
            fn set_width(&self, width: u32) {
                self.$field.width.set(width);
                self.$field.apply_rect();
            }
            fn width(&self) -> u32 {
                self.$field.width.get()
            }
            fn set_height(&self, height: u32) {
                self.$field.height.set(height);
                self.$field.apply_rect();
            }
            fn height(&self) -> u32 {
                self.$field.height.get()
            }
            fn set_on_click(&self, callback: Box<dyn FnMut()>) {
                self.$field.click.set(callback);
            }
            fn set_fore_color(&self, color: Option<crate::Color>) {
                self.$field.set_fore(color);
            }
            fn fore_color(&self) -> Option<crate::Color> {
                self.$field.fore()
            }
            fn set_back_color(&self, color: Option<crate::Color>) {
                self.$field.set_back(color);
            }
            fn back_color(&self) -> Option<crate::Color> {
                self.$field.back()
            }
        }
    };
}

pub(crate) use impl_widget_handle;

// --- Helpers for composite controls ---

/// An event box holding `child`: a GDK window to take clicks and a CSS
/// background, which windowless widgets (label, image) lack.
pub fn event_box_around(child: *mut GtkWidget) -> *mut GtkWidget {
    unsafe {
        let event_box = ffi::gtk_event_box_new();
        ffi::gtk_container_add(event_box, child);
        ffi::gtk_widget_show(child);
        event_box
    }
}

/// A bordered scrolled window holding `child`.
pub fn scrolled_around(child: *mut GtkWidget) -> *mut GtkWidget {
    unsafe {
        let scrolled = ffi::gtk_scrolled_window_new(std::ptr::null_mut(), std::ptr::null_mut());
        ffi::gtk_scrolled_window_set_shadow_type(scrolled, ffi::GTK_SHADOW_IN);
        ffi::gtk_scrolled_window_set_policy(scrolled, ffi::GTK_POLICY_AUTOMATIC, ffi::GTK_POLICY_AUTOMATIC);
        ffi::gtk_container_add(scrolled, child);
        ffi::gtk_widget_show(child);
        scrolled
    }
}

thread_local! {
    static RADIO_GROUPS: RefCell<HashMap<(usize, String), usize>> = RefCell::new(HashMap::new());
}

/// The radio button to join for `group` on this form, if one exists already.
pub fn radio_group_leader(parent: &Parent, group: &str) -> Option<*mut GtkWidget> {
    RADIO_GROUPS.with(|groups| groups.borrow().get(&(parent.fixed as usize, group.to_string())).map(|&w| w as *mut _))
}

pub fn set_radio_group_leader(parent: &Parent, group: &str, widget: *mut GtkWidget) {
    RADIO_GROUPS.with(|groups| groups.borrow_mut().insert((parent.fixed as usize, group.to_string()), widget as usize));
}
