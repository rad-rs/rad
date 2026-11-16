use crate::widget::impl_widget_common;
use crate::{Host, WidgetHandle};

pub trait ProgressBarHandle: WidgetHandle {
    /// Clamped to `0..=100` by the backend — there's no custom
    /// min/max here yet, just a plain percentage.
    fn set_value(&self, value: u32);
    fn value(&self) -> u32;
}

/// A horizontal progress indicator, 0-100.
pub struct ProgressBar {
    handle: Box<dyn ProgressBarHandle>,
}

impl ProgressBar {
    pub fn new(host: &dyn Host) -> Self {
        Self { handle: host.create_progress_bar() }
    }

    pub fn set_value(&self, value: u32) {
        self.handle.set_value(value);
    }

    pub fn value(&self) -> u32 {
        self.handle.value()
    }
}

impl_widget_common!(ProgressBar);
