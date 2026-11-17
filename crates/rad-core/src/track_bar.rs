use crate::widget::impl_widget_common;
use crate::{Host, WidgetHandle};

pub trait TrackBarHandle: WidgetHandle {
    fn set_minimum(&self, minimum: i32);
    fn minimum(&self) -> i32;
    fn set_maximum(&self, maximum: i32);
    fn maximum(&self) -> i32;
    /// Clamped to `minimum..=maximum` by the backend.
    fn set_value(&self, value: i32);
    fn value(&self) -> i32;
    /// Fires when the user drags the slider or presses a key on it. Not fired
    /// for `set_value` calls made from code.
    fn set_on_change(&self, callback: Box<dyn FnMut()>);
}

/// A horizontal slider over an integer range (0 to 100 until changed).
pub struct TrackBar {
    handle: Box<dyn TrackBarHandle>,
}

impl TrackBar {
    pub fn new(host: &dyn Host) -> Self {
        Self { handle: host.create_track_bar() }
    }

    pub fn set_minimum(&self, minimum: i32) {
        self.handle.set_minimum(minimum);
    }

    pub fn minimum(&self) -> i32 {
        self.handle.minimum()
    }

    pub fn set_maximum(&self, maximum: i32) {
        self.handle.set_maximum(maximum);
    }

    pub fn maximum(&self) -> i32 {
        self.handle.maximum()
    }

    pub fn set_value(&self, value: i32) {
        self.handle.set_value(value);
    }

    pub fn value(&self) -> i32 {
        self.handle.value()
    }

    pub fn set_on_change(&self, callback: Box<dyn FnMut()>) {
        self.handle.set_on_change(callback);
    }
}

impl_widget_common!(TrackBar);
