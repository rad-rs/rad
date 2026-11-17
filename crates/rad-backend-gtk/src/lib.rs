//! Native Linux backend on GTK 3, with no crate dependencies: the few GTK
//! entry points it needs are hand-declared in `rad_core::platform::gtk`, which
//! links `libgtk-3` (install `libgtk-3-dev`). The backend only creates the
//! top-level window and runs the main loop; every control is a component's own
//! business.
//!
//! Compiled on Linux only, so a workspace build on another OS skips it.
#![cfg(target_os = "linux")]

use rad_core::platform::gtk::{self, cstr, from_c, Parent, Slot};
use rad_core::platform::gtk::ffi;
use rad_core::{Backend, Host, WindowHandle};
use std::ffi::{c_int, c_void};
use std::rc::Rc;

/// Runs GTK's main loop until the last window is closed.
pub fn run() {
    unsafe { ffi::gtk_main() };
}

pub struct GtkBackend;

impl Backend for GtkBackend {
    fn create_window(&self, title: &str, width: u32, height: u32) -> Box<dyn WindowHandle> {
        Box::new(GtkWindow::new(title, width, height))
    }
}

pub struct GtkWindow {
    window: *mut ffi::GtkWidget,
    fixed: *mut ffi::GtkWidget,
    click: Rc<Slot>,
    resize: Rc<ResizeSlot>,
}

#[derive(Default)]
struct ResizeSlot {
    callback: std::cell::RefCell<Option<Box<dyn FnMut(u32, u32)>>>,
}

unsafe extern "C" fn quit(_widget: *mut c_void, _data: *mut c_void) {
    unsafe { ffi::gtk_main_quit() };
}

/// `size-allocate`: `(GtkWidget*, GdkRectangle*, gpointer)`. A `GdkRectangle`
/// is `{x, y, width, height}` of `int`s.
unsafe extern "C" fn allocated(_widget: *mut c_void, rect: *const [c_int; 4], data: *mut c_void) {
    let slot = unsafe { &*(data as *const ResizeSlot) };
    let [_, _, width, height] = unsafe { *rect };
    // Taken out while it runs so the callback may replace itself.
    let taken = slot.callback.borrow_mut().take();
    if let Some(mut callback) = taken {
        callback(width.max(0) as u32, height.max(0) as u32);
        if slot.callback.borrow().is_none() {
            *slot.callback.borrow_mut() = Some(callback);
        }
    }
}

impl GtkWindow {
    fn new(title: &str, width: u32, height: u32) -> Self {
        gtk::init_once();
        unsafe {
            let window = ffi::gtk_window_new(0);
            ffi::gtk_window_set_title(window, cstr(title).as_ptr());
            ffi::gtk_window_set_default_size(window, width as c_int, height as c_int);
            // An event box under the window takes clicks that miss every control;
            // the fixed layout inside it places the controls at absolute positions.
            let event_box = ffi::gtk_event_box_new();
            let fixed = ffi::gtk_fixed_new();
            ffi::gtk_container_add(event_box, fixed);
            ffi::gtk_container_add(window, event_box);
            ffi::gtk_widget_add_events(event_box, ffi::GDK_BUTTON_PRESS_MASK);
            let click = Rc::new(Slot::default());
            gtk::connect_event(event_box, "button-press-event", &click);
            let resize = Rc::new(ResizeSlot::default());
            // Leaked with the window, which lives for the whole program.
            let resize_data = Rc::into_raw(resize.clone()) as *mut c_void;
            ffi::g_signal_connect_data(
                window,
                c"size-allocate".as_ptr(),
                allocated as *const c_void,
                resize_data,
                None,
                0,
            );
            ffi::g_signal_connect_data(window, c"destroy".as_ptr(), quit as *const c_void, std::ptr::null_mut(), None, 0);
            ffi::gtk_widget_show(fixed);
            ffi::gtk_widget_show(event_box);
            ffi::gtk_widget_show(window);
            Self { window, fixed, click, resize }
        }
    }
}

impl WindowHandle for GtkWindow {
    fn set_title(&self, title: &str) {
        unsafe { ffi::gtk_window_set_title(self.window, cstr(title).as_ptr()) };
    }
    fn title(&self) -> String {
        from_c(unsafe { ffi::gtk_window_get_title(self.window) })
    }
    fn set_size(&self, width: u32, height: u32) {
        unsafe { ffi::gtk_window_resize(self.window, width as c_int, height as c_int) };
    }
    fn size(&self) -> (u32, u32) {
        let (mut width, mut height) = (0, 0);
        unsafe { ffi::gtk_window_get_size(self.window, &mut width, &mut height) };
        (width.max(0) as u32, height.max(0) as u32)
    }
    fn set_on_resize(&self, callback: Box<dyn FnMut(u32, u32)>) {
        *self.resize.callback.borrow_mut() = Some(callback);
    }
    fn set_on_click(&self, callback: Box<dyn FnMut()>) {
        self.click.set(callback);
    }
}

impl Host for GtkWindow {
    fn parent(&self) -> Parent {
        Parent { fixed: self.fixed }
    }
}

// The window outlives the process in every real use (the main loop owns it for
// the app's whole lifetime), so it has no `Drop`, as in `rad-backend-win32`.
