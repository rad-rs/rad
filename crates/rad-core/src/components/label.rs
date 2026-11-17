use crate::widget::impl_widget_common;
use crate::{
    ComponentInfo, CtorArg, Designable, Host, Preview, PropInfo, PropType, PropValue, WidgetHandle,
};

pub trait LabelHandle: WidgetHandle {
    fn set_text(&self, text: &str);
    fn text(&self) -> String;
}

/// Static, non-interactive text.
pub struct Label {
    handle: Box<dyn LabelHandle>,
}

impl Label {
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

impl_widget_common!(Label);

// --- Designer metadata ---

pub(crate) const INFO: ComponentInfo = ComponentInfo {
    name: "Label",
    category: "Common Controls",
    icon: "A",
    order: 20,
    visual: true,
    default_size: (60, 16),
    ctor: &[CtorArg::Text],
    properties: &[PropInfo { name: "text", ty: PropType::Text, default: "" }],
    events: &["click"],
    preview: Preview { tag: "span", class: "", attrs: &[], css: "", labeled_input: None },
    create: |host, text| Box::new(Label::new(host, text)),
};

impl Designable for Label {
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

    pub(super) fn create(parent: &Parent, text: &str) -> Box<dyn LabelHandle> {
        let hwnd = create_child(parent.hwnd, "STATIC", ffi::SS_LEFT, text);
        Box::new(Handle { base: Win32Widget::new(hwnd) })
    }

    struct Handle {
        base: Win32Widget,
    }
    impl_widget_handle!(Handle, base);
    impl LabelHandle for Handle {
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
    use crate::platform::gtk::{cstr, event_box_around, ffi, from_c, impl_widget_handle, GtkBase, Parent};

    pub(super) fn create(parent: &Parent, text: &str) -> Box<dyn LabelHandle> {
        // A label has no GDK window of its own; an event box gives it clicks
        // and a background colour.
        let label = unsafe { ffi::gtk_label_new(cstr(text).as_ptr()) };
        unsafe {
            ffi::gtk_label_set_xalign(label, 0.0);
            ffi::gtk_label_set_yalign(label, 0.0);
        }
        let base = GtkBase::new(parent, event_box_around(label), None);
        Box::new(Handle { base, label })
    }

    struct Handle {
        base: GtkBase,
        label: *mut ffi::GtkWidget,
    }
    impl_widget_handle!(Handle, base);
    impl LabelHandle for Handle {
        fn set_text(&self, text: &str) {
            unsafe { ffi::gtk_label_set_text(self.label, cstr(text).as_ptr()) };
        }
        fn text(&self) -> String {
            from_c(unsafe { ffi::gtk_label_get_text(self.label) })
        }
    }
}

#[cfg(rad_web)]
mod imp {
    use super::*;
    use crate::platform::web::{impl_widget_handle, Base, Parent};

    pub(super) fn create(parent: &Parent, text: &str) -> Box<dyn LabelHandle> {
        let base = Base::new(parent, &INFO.preview, None);
        base.el.set_text_content(Some(text));
        Box::new(Handle { base })
    }

    struct Handle {
        base: Base,
    }
    impl_widget_handle!(Handle, base);
    impl LabelHandle for Handle {
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

    pub(super) fn create(parent: &Parent, text: &str) -> Box<dyn LabelHandle> {
        Box::new(Handle { el: ElementHandle::create(parent, &INFO.preview, text, "") })
    }

    struct Handle {
        el: ElementHandle,
    }
    impl_min_widget!(Handle, el);
    impl LabelHandle for Handle {
        fn set_text(&self, text: &str) {
            self.el.set_text(text);
        }
        fn text(&self) -> String {
            self.el.text()
        }
    }
}
