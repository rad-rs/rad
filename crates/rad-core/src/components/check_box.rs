use crate::widget::impl_widget_common;
use crate::{
    ComponentInfo, CtorArg, Designable, Host, Preview, PropInfo, PropType, PropValue, WidgetHandle,
};

pub trait CheckBoxHandle: WidgetHandle {
    fn set_text(&self, text: &str);
    fn text(&self) -> String;

    fn set_checked(&self, checked: bool);
    fn checked(&self) -> bool;
}

/// A labeled, toggleable box.
pub struct CheckBox {
    handle: Box<dyn CheckBoxHandle>,
}

impl CheckBox {
    pub fn new(host: &dyn Host, text: &str) -> Self {
        Self { handle: imp::create(&host.parent(), text) }
    }

    pub fn set_text(&self, text: &str) {
        self.handle.set_text(text);
    }

    pub fn text(&self) -> String {
        self.handle.text()
    }

    pub fn set_checked(&self, checked: bool) {
        self.handle.set_checked(checked);
    }

    pub fn checked(&self) -> bool {
        self.handle.checked()
    }
}

impl_widget_common!(CheckBox);

// --- Designer metadata ---

pub(crate) const INFO: ComponentInfo = ComponentInfo {
    name: "CheckBox",
    category: "Common Controls",
    icon: "\u{2713}",
    order: 30,
    visual: true,
    default_size: (90, 17),
    ctor: &[CtorArg::Text],
    properties: &[
        PropInfo { name: "text", ty: PropType::Text, default: "" },
        PropInfo { name: "checked", ty: PropType::Bool, default: "false" },
    ],
    events: &["click"],
    preview: Preview {
        tag: "label",
        class: "rad-checkbox",
        attrs: &[],
        css: "",
        labeled_input: Some("checkbox"),
    },
    create: |host, text| Box::new(CheckBox::new(host, text)),
};

impl Designable for CheckBox {
    fn get(&self, property: &str) -> Option<PropValue> {
        match property {
            "text" => Some(PropValue::Text(self.text())),
            "checked" => Some(PropValue::Bool(self.checked())),
            _ => None,
        }
    }
    fn set(&self, property: &str, value: PropValue) {
        match property {
            "text" => {
                if let Some(text) = value.into_text() {
                    self.set_text(&text);
                }
            }
            "checked" => {
                if let Some(checked) = value.into_bool() {
                    self.set_checked(checked);
                }
            }
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
    use crate::platform::win32::{create_child, ffi, impl_widget_handle, set_window_text, window_text, Parent, Win32Widget};

    pub(super) fn create(parent: &Parent, text: &str) -> Box<dyn CheckBoxHandle> {
        let hwnd = create_child(parent.hwnd, "BUTTON", ffi::BS_AUTOCHECKBOX, text);
        Box::new(Handle { base: Win32Widget::new(hwnd) })
    }

    struct Handle {
        base: Win32Widget,
    }
    impl_widget_handle!(Handle, base);
    impl CheckBoxHandle for Handle {
        fn set_text(&self, text: &str) {
            set_window_text(self.base.hwnd, text);
        }
        fn text(&self) -> String {
            window_text(self.base.hwnd)
        }
        fn set_checked(&self, checked: bool) {
            let state = if checked { ffi::BST_CHECKED } else { ffi::BST_UNCHECKED };
            unsafe { ffi::SendMessageW(self.base.hwnd, ffi::BM_SETCHECK, state as usize, 0) };
        }
        fn checked(&self) -> bool {
            unsafe { ffi::SendMessageW(self.base.hwnd, ffi::BM_GETCHECK, 0, 0) == ffi::BST_CHECKED }
        }
    }
}

#[cfg(rad_gtk)]
mod imp {
    use super::*;
    use crate::platform::gtk::{cstr, ffi, from_c, impl_widget_handle, programmatic, GtkBase, Parent};

    pub(super) fn create(parent: &Parent, text: &str) -> Box<dyn CheckBoxHandle> {
        let widget = unsafe { ffi::gtk_check_button_new_with_label(cstr(text).as_ptr()) };
        Box::new(Handle { base: GtkBase::new(parent, widget, Some("clicked")) })
    }

    struct Handle {
        base: GtkBase,
    }
    impl_widget_handle!(Handle, base);
    impl CheckBoxHandle for Handle {
        fn set_text(&self, text: &str) {
            unsafe { ffi::gtk_button_set_label(self.base.widget, cstr(text).as_ptr()) };
        }
        fn text(&self) -> String {
            from_c(unsafe { ffi::gtk_button_get_label(self.base.widget) })
        }
        fn set_checked(&self, checked: bool) {
            programmatic(|| unsafe { ffi::gtk_toggle_button_set_active(self.base.widget, checked as i32) });
        }
        fn checked(&self) -> bool {
            unsafe { ffi::gtk_toggle_button_get_active(self.base.widget) != 0 }
        }
    }
}

#[cfg(rad_web)]
mod imp {
    use super::*;
    use crate::platform::web::{impl_widget_handle, Base, Parent};

    pub(super) fn create(parent: &Parent, text: &str) -> Box<dyn CheckBoxHandle> {
        let base = Base::new(parent, &INFO.preview, None);
        base.labeled_text().set_text_content(Some(text));
        Box::new(Handle { base })
    }

    struct Handle {
        base: Base,
    }
    impl_widget_handle!(Handle, base);
    impl CheckBoxHandle for Handle {
        fn set_text(&self, text: &str) {
            self.base.labeled_text().set_text_content(Some(text));
        }
        fn text(&self) -> String {
            self.base.labeled_text().text_content().unwrap_or_default()
        }
        fn set_checked(&self, checked: bool) {
            self.base.labeled_input().set_checked(checked);
        }
        fn checked(&self) -> bool {
            self.base.labeled_input().checked()
        }
    }
}

#[cfg(rad_web_min)]
mod imp {
    use super::*;
    use crate::platform::web_min::{impl_min_widget, ElementHandle, Parent};

    pub(super) fn create(parent: &Parent, text: &str) -> Box<dyn CheckBoxHandle> {
        Box::new(Handle { el: ElementHandle::create(parent, &INFO.preview, text, "") })
    }

    struct Handle {
        el: ElementHandle,
    }
    impl_min_widget!(Handle, el);
    impl CheckBoxHandle for Handle {
        fn set_text(&self, text: &str) {
            self.el.set_text(text);
        }
        fn text(&self) -> String {
            self.el.text()
        }
        fn set_checked(&self, checked: bool) {
            self.el.set_checked(checked);
        }
        fn checked(&self) -> bool {
            self.el.checked()
        }
    }
}
