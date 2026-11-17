use crate::widget::impl_widget_common;
use crate::{
    ComponentInfo, CtorArg, Designable, Host, Preview, PropInfo, PropType, PropValue, WidgetHandle,
};

pub trait TextBoxHandle: WidgetHandle {
    fn set_text(&self, text: &str);
    fn text(&self) -> String;
}

/// Editable single-line text.
pub struct TextBox {
    handle: Box<dyn TextBoxHandle>,
}

impl TextBox {
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

impl_widget_common!(TextBox);

// --- Designer metadata ---

pub(crate) const INFO: ComponentInfo = ComponentInfo {
    name: "TextBox",
    category: "Common Controls",
    icon: "ab",
    order: 40,
    visual: true,
    default_size: (100, 20),
    ctor: &[CtorArg::Text],
    properties: &[PropInfo { name: "text", ty: PropType::Text, default: "" }],
    events: &["click"],
    preview: Preview { tag: "input", class: "", attrs: &[("type", "text")], css: "", labeled_input: None },
    create: |host, text| Box::new(TextBox::new(host, text)),
};

impl Designable for TextBox {
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

    pub(super) fn create(parent: &Parent, text: &str) -> Box<dyn TextBoxHandle> {
        let hwnd = create_child(parent.hwnd, "EDIT", ffi::ES_AUTOHSCROLL | ffi::WS_BORDER, text);
        Box::new(Handle { base: Win32Widget::new(hwnd) })
    }

    struct Handle {
        base: Win32Widget,
    }
    impl_widget_handle!(Handle, base);
    impl TextBoxHandle for Handle {
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

    pub(super) fn create(parent: &Parent, text: &str) -> Box<dyn TextBoxHandle> {
        let widget = unsafe { ffi::gtk_entry_new() };
        unsafe { ffi::gtk_entry_set_text(widget, cstr(text).as_ptr()) };
        // An entry consumes mouse presses itself, so there is no plain click to
        // hook; "activate" (Enter) is the closest thing it raises.
        Box::new(Handle { base: GtkBase::new(parent, widget, Some("activate")) })
    }

    struct Handle {
        base: GtkBase,
    }
    impl_widget_handle!(Handle, base);
    impl TextBoxHandle for Handle {
        fn set_text(&self, text: &str) {
            unsafe { ffi::gtk_entry_set_text(self.base.widget, cstr(text).as_ptr()) };
        }
        fn text(&self) -> String {
            from_c(unsafe { ffi::gtk_entry_get_text(self.base.widget) })
        }
    }
}

#[cfg(rad_web)]
mod imp {
    use super::*;
    use crate::platform::web::{impl_widget_handle, Base, Parent};

    pub(super) fn create(parent: &Parent, text: &str) -> Box<dyn TextBoxHandle> {
        let base = Base::new(parent, &INFO.preview, None);
        base.as_input().set_value(text);
        Box::new(Handle { base })
    }

    struct Handle {
        base: Base,
    }
    impl_widget_handle!(Handle, base);
    impl TextBoxHandle for Handle {
        fn set_text(&self, text: &str) {
            self.base.as_input().set_value(text);
        }
        fn text(&self) -> String {
            self.base.as_input().value()
        }
    }
}

#[cfg(rad_web_min)]
mod imp {
    use super::*;
    use crate::platform::web_min::{impl_min_widget, ElementHandle, Parent};

    pub(super) fn create(parent: &Parent, text: &str) -> Box<dyn TextBoxHandle> {
        Box::new(Handle { el: ElementHandle::create(parent, &INFO.preview, text, "") })
    }

    struct Handle {
        el: ElementHandle,
    }
    impl_min_widget!(Handle, el);
    impl TextBoxHandle for Handle {
        fn set_text(&self, text: &str) {
            self.el.set_text(text);
        }
        fn text(&self) -> String {
            self.el.text()
        }
    }
}
