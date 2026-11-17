use crate::widget::impl_widget_common;
use crate::{
    ComponentInfo, CtorArg, Designable, Host, Preview, PropInfo, PropType, PropValue, WidgetHandle,
};

pub trait ButtonHandle: WidgetHandle {
    fn set_text(&self, text: &str);
    fn text(&self) -> String;
}

/// A clickable button. Thin wrapper around a backend's `ButtonHandle` —
/// setters forward straight through, nothing else happens here yet.
pub struct Button {
    handle: Box<dyn ButtonHandle>,
}

impl Button {
    pub fn new(host: &dyn Host, text: &str) -> Self {
        Self { handle: imp::create(&host.parent(), text) }
    }

    pub fn set_text(&self, text: &str) {
        self.handle.set_text(text);
    }

    pub fn text(&self) -> String {
        self.handle.text()
    }
}

impl_widget_common!(Button);

// --- Designer metadata ---

pub(crate) const INFO: ComponentInfo = ComponentInfo {
    name: "Button",
    category: "Common Controls",
    icon: "B",
    order: 10,
    visual: true,
    default_size: (75, 23),
    ctor: &[CtorArg::Text],
    properties: &[PropInfo { name: "text", ty: PropType::Text, default: "" }],
    events: &["click"],
    preview: Preview { tag: "button", class: "", attrs: &[], css: "", labeled_input: None },
    create: |host, text| Box::new(Button::new(host, text)),
};

impl Designable for Button {
    fn get(&self, property: &str) -> Option<PropValue> {
        match property {
            "text" => Some(PropValue::Text(self.text())),
            _ => None,
        }
    }
    fn set(&self, property: &str, value: PropValue) {
        if let ("text", Some(text)) = (property, value.into_text()) {
            self.set_text(&text);
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
    use crate::platform::win32::{create_child, ffi, impl_widget_handle, set_window_text, window_text, Parent, Win32Widget};

    pub(super) fn create(parent: &Parent, text: &str) -> Box<dyn ButtonHandle> {
        let hwnd = create_child(parent.hwnd, "BUTTON", ffi::BS_PUSHBUTTON, text);
        Box::new(Handle { base: Win32Widget::new(hwnd) })
    }

    struct Handle {
        base: Win32Widget,
    }
    impl_widget_handle!(Handle, base);
    impl ButtonHandle for Handle {
        fn set_text(&self, text: &str) {
            set_window_text(self.base.hwnd, text);
        }
        fn text(&self) -> String {
            window_text(self.base.hwnd)
        }
    }
}

#[cfg(rad_gtk)]
mod imp {
    use super::*;
    use crate::platform::gtk::{cstr, ffi, from_c, impl_widget_handle, GtkBase, Parent};

    pub(super) fn create(parent: &Parent, text: &str) -> Box<dyn ButtonHandle> {
        let widget = unsafe { ffi::gtk_button_new_with_label(cstr(text).as_ptr()) };
        Box::new(Handle { base: GtkBase::new(parent, widget, Some("clicked")) })
    }

    struct Handle {
        base: GtkBase,
    }
    impl_widget_handle!(Handle, base);
    impl ButtonHandle for Handle {
        fn set_text(&self, text: &str) {
            unsafe { ffi::gtk_button_set_label(self.base.widget, cstr(text).as_ptr()) };
        }
        fn text(&self) -> String {
            from_c(unsafe { ffi::gtk_button_get_label(self.base.widget) })
        }
    }
}

#[cfg(rad_web)]
mod imp {
    use super::*;
    use crate::platform::web::{impl_widget_handle, Base, Parent};

    pub(super) fn create(parent: &Parent, text: &str) -> Box<dyn ButtonHandle> {
        let base = Base::new(parent, &INFO.preview, None);
        base.el.set_text_content(Some(text));
        Box::new(Handle { base })
    }

    struct Handle {
        base: Base,
    }
    impl_widget_handle!(Handle, base);
    impl ButtonHandle for Handle {
        fn set_text(&self, text: &str) {
            self.base.el.set_text_content(Some(text));
        }
        fn text(&self) -> String {
            self.base.el.text_content().unwrap_or_default()
        }
    }
}

#[cfg(rad_web_min)]
mod imp {
    use super::*;
    use crate::platform::web_min::{impl_min_widget, ElementHandle, Parent};

    pub(super) fn create(parent: &Parent, text: &str) -> Box<dyn ButtonHandle> {
        Box::new(Handle { el: ElementHandle::create(parent, &INFO.preview, text, "") })
    }

    struct Handle {
        el: ElementHandle,
    }
    impl_min_widget!(Handle, el);
    impl ButtonHandle for Handle {
        fn set_text(&self, text: &str) {
            self.el.set_text(text);
        }
        fn text(&self) -> String {
            self.el.text()
        }
    }
}
