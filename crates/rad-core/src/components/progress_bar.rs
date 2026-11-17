use crate::widget::impl_widget_common;
use crate::{ComponentInfo, Designable, Host, Preview, PropInfo, PropType, PropValue, WidgetHandle};

pub trait ProgressBarHandle: WidgetHandle {
    /// Clamped to `0..=100` by the backend — there's no custom
    /// min/max here yet, just a plain percentage.
    fn set_value(&self, value: u32);
    fn value(&self) -> u32;
}

/// A horizontal progress indicator, 0-100.
pub struct ProgressBar {
    handle: Box<dyn ProgressBarHandle>,
}

impl ProgressBar {
    pub fn new(host: &dyn Host) -> Self {
        Self { handle: imp::create(&host.parent()) }
    }

    pub fn set_value(&self, value: u32) {
        self.handle.set_value(value);
    }

    pub fn value(&self) -> u32 {
        self.handle.value()
    }
}

impl_widget_common!(ProgressBar);

// --- Designer metadata ---

pub(crate) const INFO: ComponentInfo = ComponentInfo {
    name: "ProgressBar",
    category: "Display",
    icon: "\u{25AE}",
    order: 110,
    visual: true,
    default_size: (150, 23),
    ctor: &[],
    properties: &[PropInfo { name: "value", ty: PropType::Int, default: "0" }],
    events: &["click"],
    preview: Preview {
        tag: "progress",
        class: "",
        attrs: &[("max", "100"), ("value", "0")],
        css: "",
        labeled_input: None,
    },
    create: |host, _| Box::new(ProgressBar::new(host)),
};

impl Designable for ProgressBar {
    fn get(&self, property: &str) -> Option<PropValue> {
        match property {
            "value" => Some(PropValue::Int(self.value() as i32)),
            _ => None,
        }
    }
    fn set(&self, property: &str, value: PropValue) {
        if let ("value", Some(value)) = (property, value.into_int()) {
            self.set_value(value.clamp(0, 100) as u32);
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
    use crate::platform::win32::{create_child, ffi, impl_widget_handle, init_common_controls_once, Parent, Win32Widget};

    pub(super) fn create(parent: &Parent) -> Box<dyn ProgressBarHandle> {
        init_common_controls_once();
        let hwnd = create_child(parent.hwnd, "msctls_progress32", 0, "");
        Box::new(Handle { base: Win32Widget::new(hwnd) })
    }

    struct Handle {
        base: Win32Widget,
    }
    impl_widget_handle!(Handle, base);
    impl ProgressBarHandle for Handle {
        fn set_value(&self, value: u32) {
            unsafe { ffi::SendMessageW(self.base.hwnd, ffi::PBM_SETPOS, value.min(100) as usize, 0) };
        }
        fn value(&self) -> u32 {
            unsafe { ffi::SendMessageW(self.base.hwnd, ffi::PBM_GETPOS, 0, 0) as u32 }
        }
    }
}

#[cfg(rad_web)]
mod imp {
    use super::*;
    use crate::platform::web::{impl_widget_handle, Base, Parent};

    pub(super) fn create(parent: &Parent) -> Box<dyn ProgressBarHandle> {
        Box::new(Handle { base: Base::new(parent, &INFO.preview, None) })
    }

    struct Handle {
        base: Base,
    }
    impl_widget_handle!(Handle, base);
    impl ProgressBarHandle for Handle {
        fn set_value(&self, value: u32) {
            self.base.el.set_attribute("value", &value.min(100).to_string()).ok();
        }
        fn value(&self) -> u32 {
            self.base.el.get_attribute("value").and_then(|v| v.parse().ok()).unwrap_or(0)
        }
    }
}

#[cfg(rad_web_min)]
mod imp {
    use super::*;
    use crate::platform::web_min::{ffi, impl_min_widget, ElementHandle, Parent};

    pub(super) fn create(parent: &Parent) -> Box<dyn ProgressBarHandle> {
        Box::new(Handle { el: ElementHandle::create(parent, &INFO.preview, "", "") })
    }

    struct Handle {
        el: ElementHandle,
    }
    impl_min_widget!(Handle, el);
    impl ProgressBarHandle for Handle {
        fn set_value(&self, value: u32) {
            unsafe { ffi::rad_set_value(self.el.id, value.min(100)) };
        }
        fn value(&self) -> u32 {
            unsafe { ffi::rad_get_value(self.el.id) }
        }
    }
}
