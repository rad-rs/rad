use crate::widget::impl_widget_common;
use crate::{Host, WidgetHandle};

pub trait ButtonHandle: WidgetHandle {
    fn set_text(&mut self, text: &str);
    fn text(&self) -> String;
}

/// A clickable button. Thin wrapper around a backend's `ButtonHandle` —
/// setters forward straight through, nothing else happens here yet.
pub struct Button {
    handle: Box<dyn ButtonHandle>,
}

impl Button {
    pub fn new(host: &dyn Host, text: &str) -> Self {
        Self {
            handle: host.create_button(text),
        }
    }

    pub fn set_text(&mut self, text: &str) {
        self.handle.set_text(text);
    }

    pub fn text(&self) -> String {
        self.handle.text()
    }
}

impl_widget_common!(Button);
