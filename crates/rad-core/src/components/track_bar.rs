use crate::widget::impl_widget_common;
use crate::{ComponentInfo, Designable, Host, Preview, PropInfo, PropType, PropValue, WidgetHandle};

pub trait TrackBarHandle: WidgetHandle {
    fn set_minimum(&self, minimum: i32);
    fn minimum(&self) -> i32;
    fn set_maximum(&self, maximum: i32);
    fn maximum(&self) -> i32;
    /// Clamped to `minimum..=maximum` by the backend.
    fn set_value(&self, value: i32);
    fn value(&self) -> i32;
    /// Fires when the user drags the slider or presses a key on it. Not fired
    /// for `set_value` calls made from code.
    fn set_on_change(&self, callback: Box<dyn FnMut()>);
}

/// A horizontal slider over an integer range (0 to 100 until changed).
pub struct TrackBar {
    handle: Box<dyn TrackBarHandle>,
}

impl TrackBar {
    pub fn new(host: &dyn Host) -> Self {
        Self { handle: imp::create(&host.parent()) }
    }

    pub fn set_minimum(&self, minimum: i32) {
        self.handle.set_minimum(minimum);
    }

    pub fn minimum(&self) -> i32 {
        self.handle.minimum()
    }

    pub fn set_maximum(&self, maximum: i32) {
        self.handle.set_maximum(maximum);
    }

    pub fn maximum(&self) -> i32 {
        self.handle.maximum()
    }

    pub fn set_value(&self, value: i32) {
        self.handle.set_value(value);
    }

    pub fn value(&self) -> i32 {
        self.handle.value()
    }

    pub fn set_on_change(&self, callback: Box<dyn FnMut()>) {
        self.handle.set_on_change(callback);
    }
}

impl_widget_common!(TrackBar);

// --- Designer metadata ---

pub(crate) const INFO: ComponentInfo = ComponentInfo {
    name: "TrackBar",
    category: "Common Controls",
    icon: "\u{25AC}",
    order: 80,
    visual: true,
    default_size: (150, 30),
    ctor: &[],
    properties: &[
        PropInfo { name: "minimum", ty: PropType::Int, default: "0" },
        PropInfo { name: "maximum", ty: PropType::Int, default: "100" },
        PropInfo { name: "value", ty: PropType::Int, default: "0" },
    ],
    events: &["change", "click"],
    preview: Preview {
        tag: "input",
        class: "",
        attrs: &[("type", "range"), ("min", "0"), ("max", "100"), ("value", "0")],
        css: "",
        labeled_input: None,
    },
    create: |host, _| Box::new(TrackBar::new(host)),
};

impl Designable for TrackBar {
    fn get(&self, property: &str) -> Option<PropValue> {
        match property {
            "minimum" => Some(PropValue::Int(self.minimum())),
            "maximum" => Some(PropValue::Int(self.maximum())),
            "value" => Some(PropValue::Int(self.value())),
            _ => None,
        }
    }
    fn set(&self, property: &str, value: PropValue) {
        let Some(value) = value.into_int() else { return };
        match property {
            "minimum" => self.set_minimum(value),
            "maximum" => self.set_maximum(value),
            // The backend clamps to the trackbar's own range.
            "value" => self.set_value(value),
            _ => {}
        }
    }
    fn widget(&self) -> Option<&dyn WidgetHandle> {
        Some(self)
    }
}

// --- Platforms: one `imp` per target; add a `#[cfg(rad_macos)]` one for macOS ---

#[cfg(rad_win32)]
mod imp {
    use super::*;
    use crate::platform::win32::{
        create_child, erase_to_form_color, ffi, impl_widget_handle, init_common_controls_once, set_change_handler,
        Parent, Win32Widget,
    };

    pub(super) fn create(parent: &Parent) -> Box<dyn TrackBarHandle> {
        init_common_controls_once();
        let hwnd = create_child(parent.hwnd, "msctls_trackbar32", ffi::TBS_AUTOTICKS, "");
        // A trackbar paints nothing behind itself, so it needs the form's colour.
        erase_to_form_color(hwnd);
        unsafe {
            ffi::SendMessageW(hwnd, ffi::TBM_SETRANGEMIN, 1, 0);
            ffi::SendMessageW(hwnd, ffi::TBM_SETRANGEMAX, 1, 100);
        }
        let handle = Handle { base: Win32Widget::new(hwnd) };
        handle.update_ticks();
        Box::new(handle)
    }

    struct Handle {
        base: Win32Widget,
    }
    impl_widget_handle!(Handle, base);
    impl Handle {
        /// `TBS_AUTOTICKS` draws a tick for every unit by default, which for a range
        /// like 100..1500 merges into one solid bar. Space them about ten across the
        /// range instead (never closer than one unit).
        fn update_ticks(&self) {
            unsafe {
                let min = ffi::SendMessageW(self.base.hwnd, ffi::TBM_GETRANGEMIN, 0, 0) as i64;
                let max = ffi::SendMessageW(self.base.hwnd, ffi::TBM_GETRANGEMAX, 0, 0) as i64;
                let freq = ((max - min) / 10).max(1);
                ffi::SendMessageW(self.base.hwnd, ffi::TBM_SETTICFREQ, freq as usize, 0);
            }
        }
    }

    impl TrackBarHandle for Handle {
        fn set_minimum(&self, minimum: i32) {
            unsafe { ffi::SendMessageW(self.base.hwnd, ffi::TBM_SETRANGEMIN, 1, minimum as isize) };
            self.update_ticks();
        }
        fn minimum(&self) -> i32 {
            unsafe { ffi::SendMessageW(self.base.hwnd, ffi::TBM_GETRANGEMIN, 0, 0) as i32 }
        }
        fn set_maximum(&self, maximum: i32) {
            unsafe { ffi::SendMessageW(self.base.hwnd, ffi::TBM_SETRANGEMAX, 1, maximum as isize) };
            self.update_ticks();
        }
        fn maximum(&self) -> i32 {
            unsafe { ffi::SendMessageW(self.base.hwnd, ffi::TBM_GETRANGEMAX, 0, 0) as i32 }
        }
        fn set_value(&self, value: i32) {
            // The control clamps to its range itself.
            unsafe { ffi::SendMessageW(self.base.hwnd, ffi::TBM_SETPOS, 1, value as isize) };
        }
        fn value(&self) -> i32 {
            unsafe { ffi::SendMessageW(self.base.hwnd, ffi::TBM_GETPOS, 0, 0) as i32 }
        }
        fn set_on_change(&self, callback: Box<dyn FnMut()>) {
            set_change_handler(self.base.hwnd, callback);
        }
    }
}

#[cfg(rad_web)]
mod imp {
    use super::*;
    use crate::platform::web::{el_set_listener, impl_widget_handle, Base, Parent};
    use std::cell::RefCell;
    use wasm_bindgen::prelude::Closure;

    pub(super) fn create(parent: &Parent) -> Box<dyn TrackBarHandle> {
        Box::new(Handle { base: Base::new(parent, &INFO.preview, None), on_change: RefCell::new(None) })
    }

    struct Handle {
        base: Base,
        on_change: RefCell<Option<Closure<dyn FnMut()>>>,
    }
    impl Handle {
        fn attr_i32(&self, name: &str, default: i32) -> i32 {
            self.base.el.get_attribute(name).and_then(|v| v.parse().ok()).unwrap_or(default)
        }
    }
    impl_widget_handle!(Handle, base);
    impl TrackBarHandle for Handle {
        fn set_minimum(&self, minimum: i32) {
            self.base.el.set_attribute("min", &minimum.to_string()).ok();
        }
        fn minimum(&self) -> i32 {
            self.attr_i32("min", 0)
        }
        fn set_maximum(&self, maximum: i32) {
            self.base.el.set_attribute("max", &maximum.to_string()).ok();
        }
        fn maximum(&self) -> i32 {
            self.attr_i32("max", 100)
        }
        fn set_value(&self, value: i32) {
            // The browser clamps a range input to its min/max itself.
            self.base.as_input().set_value(&value.to_string());
        }
        fn value(&self) -> i32 {
            self.base.as_input().value().parse().unwrap_or(0)
        }
        fn set_on_change(&self, callback: Box<dyn FnMut()>) {
            // `input` fires continuously while dragging; `change` only on release.
            el_set_listener(&self.base.el, "input", &self.on_change, callback);
        }
    }
}

#[cfg(rad_web_min)]
mod imp {
    use super::*;
    use crate::platform::web_min::{ffi, impl_min_widget, register_change, ElementHandle, Parent};
    use std::cell::Cell;

    pub(super) fn create(parent: &Parent) -> Box<dyn TrackBarHandle> {
        Box::new(Handle {
            el: ElementHandle::create(parent, &INFO.preview, "", ""),
            minimum: Cell::new(0),
            maximum: Cell::new(100),
        })
    }

    struct Handle {
        el: ElementHandle,
        minimum: Cell<i32>,
        maximum: Cell<i32>,
    }
    impl_min_widget!(Handle, el);
    impl TrackBarHandle for Handle {
        fn set_minimum(&self, minimum: i32) {
            self.minimum.set(minimum);
            unsafe { ffi::rad_set_range(self.el.id, minimum, self.maximum.get()) };
        }
        fn minimum(&self) -> i32 {
            self.minimum.get()
        }
        fn set_maximum(&self, maximum: i32) {
            self.maximum.set(maximum);
            unsafe { ffi::rad_set_range(self.el.id, self.minimum.get(), maximum) };
        }
        fn maximum(&self) -> i32 {
            self.maximum.get()
        }
        fn set_value(&self, value: i32) {
            unsafe { ffi::rad_set_int_value(self.el.id, value) };
        }
        fn value(&self) -> i32 {
            unsafe { ffi::rad_get_int_value(self.el.id) }
        }
        fn set_on_change(&self, callback: Box<dyn FnMut()>) {
            register_change(self.el.id, callback);
        }
    }
}
