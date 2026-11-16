use crate::widget::impl_widget_common;
use crate::{Host, WidgetHandle};

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
        Self {
            handle: host.create_checkbox(text),
        }
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
