use crate::Host;

/// What a backend provides for a [`Timer`]. Unlike a control, a timer has no
/// position, size or appearance, so this deliberately is not a
/// `WidgetHandle`.
///
/// Every setter takes `&self` (same reasoning as `WidgetHandle`): the real
/// state lives on the OS/DOM side, and an event handler holding the timer
/// through an `Rc` must be able to start, stop or retune it from inside its
/// own tick.
pub trait TimerHandle {
    /// Time between ticks, in milliseconds. A running timer is restarted with
    /// the new interval. Backends clamp very small values (Win32 cannot go
    /// below about 10 ms).
    fn set_interval(&self, interval_ms: u32);
    fn interval(&self) -> u32;

    /// Starts (`true`) or stops (`false`) the ticking.
    fn set_enabled(&self, enabled: bool);
    fn enabled(&self) -> bool;

    /// Replaces the tick callback. Ticks are delivered on the UI thread, one
    /// at a time, never from inside another handler.
    fn set_on_tick(&self, callback: Box<dyn FnMut()>);
}

/// A non-visual component that raises a tick on a fixed interval, like
/// WinForms' `Timer`. Disabled until `set_enabled(true)` / `start()`, with a
/// default interval of 100 ms. Stops ticking when dropped.
pub struct Timer {
    handle: Box<dyn TimerHandle>,
}

impl Timer {
    pub fn new(host: &dyn Host) -> Self {
        Self { handle: host.create_timer() }
    }

    pub fn set_interval(&self, interval_ms: u32) {
        self.handle.set_interval(interval_ms);
    }

    pub fn interval(&self) -> u32 {
        self.handle.interval()
    }

    pub fn set_enabled(&self, enabled: bool) {
        self.handle.set_enabled(enabled);
    }

    pub fn enabled(&self) -> bool {
        self.handle.enabled()
    }

    pub fn start(&self) {
        self.handle.set_enabled(true);
    }

    pub fn stop(&self) {
        self.handle.set_enabled(false);
    }

    pub fn set_on_tick(&self, callback: Box<dyn FnMut()>) {
        self.handle.set_on_tick(callback);
    }
}
