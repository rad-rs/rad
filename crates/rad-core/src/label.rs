use crate::widget::impl_widget_common;
use crate::{Host, WidgetHandle};

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
        Self {
            handle: host.create_label(text),
        }
    }

    pub fn set_text(&self, text: &str) {
        self.handle.set_text(text);
    }

    pub fn text(&self) -> String {
        self.handle.text()
    }
}

impl_widget_common!(Label);
