use crate::widget::impl_widget_common;
use crate::{
    ComponentInfo, CtorArg, Designable, Host, Preview, PropInfo, PropType, PropValue, WidgetHandle,
};

pub trait RadioButtonHandle: WidgetHandle {
    fn set_text(&self, text: &str);
    fn text(&self) -> String;

    fn set_checked(&self, checked: bool);
    fn checked(&self) -> bool;
}

/// A labeled, mutually-exclusive option. Mutual exclusivity is scoped to
/// `group`: radio buttons sharing a group on the same host are the ones
/// that exclude each other. Win32 gets this almost for free from
/// contiguous `BS_AUTORADIOBUTTON` siblings; the web has no such native
/// grouping, so `group` is baked into the input's `name` at creation time.
pub struct RadioButton {
    handle: Box<dyn RadioButtonHandle>,
}

impl RadioButton {
    pub fn new(host: &dyn Host, group: &str, text: &str) -> Self {
        Self { handle: imp::create(&host.parent(), group, text) }
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

impl_widget_common!(RadioButton);

// --- Designer metadata ---

pub(crate) const INFO: ComponentInfo = ComponentInfo {
    name: "RadioButton",
    category: "Common Controls",
    icon: "\u{25CF}",
    order: 50,
    visual: true,
    default_size: (90, 17),
    ctor: &[CtorArg::Lit("\"group1\""), CtorArg::Text],
    properties: &[
        PropInfo { name: "text", ty: PropType::Text, default: "" },
        PropInfo { name: "checked", ty: PropType::Bool, default: "false" },
    ],
    events: &["click"],
    preview: Preview {
        tag: "label",
        class: "rad-radio-button",
        attrs: &[],
        css: "",
        labeled_input: Some("radio"),
    },
    create: |host, text| Box::new(RadioButton::new(host, "group1", text)),
};

impl Designable for RadioButton {
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

    pub(super) fn create(parent: &Parent, group: &str, text: &str) -> Box<dyn RadioButtonHandle> {
        let _ = group; // Win32 groups contiguous radio buttons itself.
        let hwnd = create_child(parent.hwnd, "BUTTON", ffi::BS_AUTORADIOBUTTON, text);
        Box::new(Handle { base: Win32Widget::new(hwnd) })
    }

    struct Handle {
        base: Win32Widget,
    }
    impl_widget_handle!(Handle, base);
    impl RadioButtonHandle for Handle {
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

#[cfg(rad_web)]
mod imp {
    use super::*;
    use crate::platform::web::{impl_widget_handle, Base, Parent};

    pub(super) fn create(parent: &Parent, group: &str, text: &str) -> Box<dyn RadioButtonHandle> {
        let base = Base::new(parent, &INFO.preview, Some(group));
        base.labeled_text().set_text_content(Some(text));
        Box::new(Handle { base })
    }

    struct Handle {
        base: Base,
    }
    impl_widget_handle!(Handle, base);
    impl RadioButtonHandle for Handle {
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

    pub(super) fn create(parent: &Parent, group: &str, text: &str) -> Box<dyn RadioButtonHandle> {
        Box::new(Handle { el: ElementHandle::create(parent, &INFO.preview, text, group) })
    }

    struct Handle {
        el: ElementHandle,
    }
    impl_min_widget!(Handle, el);
    impl RadioButtonHandle for Handle {
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
