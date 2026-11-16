use crate::widget::impl_widget_common;
use crate::{Host, WidgetHandle};

pub trait RadioButtonHandle: WidgetHandle {
    fn set_text(&self, text: &str);
    fn text(&self) -> String;

    fn set_checked(&self, checked: bool);
    fn checked(&self) -> bool;
}

/// A labeled, mutually-exclusive option. Mutual exclusivity is scoped to
/// `group`: radio buttons sharing a group on the same host are the ones
/// that exclude each other. Win32 gets this almost for free from
/// contiguous `BS_AUTORADIOBUTTON` siblings; the web backend has no such
/// native grouping below the `Host` level, so `group` is threaded through
/// `Host::create_radio_button` rather than left implicit.
pub struct RadioButton {
    handle: Box<dyn RadioButtonHandle>,
}

impl RadioButton {
    pub fn new(host: &dyn Host, group: &str, text: &str) -> Self {
        Self {
            handle: host.create_radio_button(group, text),
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

impl_widget_common!(RadioButton);
