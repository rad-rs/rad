use crate::widget::impl_widget_common;
use crate::{Host, WidgetHandle};

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
        Self {
            handle: host.create_textbox(text),
        }
    }

    pub fn set_text(&self, text: &str) {
        self.handle.set_text(text);
    }

    pub fn text(&self) -> String {
        self.handle.text()
    }
}

impl_widget_common!(TextBox);
