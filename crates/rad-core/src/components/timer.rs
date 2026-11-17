use crate::{ComponentInfo, Designable, Host, Preview, PropInfo, PropType, PropValue};

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
        Self { handle: imp::create(&host.parent()) }
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

// --- Designer metadata ---

pub(crate) const INFO: ComponentInfo = ComponentInfo {
    name: "Timer",
    category: "Components",
    icon: "\u{23F1}",
    order: 210,
    // Non-visual: shown as a chip in the component tray, with no position,
    // size or DOM of its own.
    visual: false,
    default_size: (0, 0),
    ctor: &[],
    properties: &[
        PropInfo { name: "interval", ty: PropType::Int, default: "100" },
        PropInfo { name: "enabled", ty: PropType::Bool, default: "false" },
    ],
    events: &["tick"],
    preview: Preview::NONE,
    create: |host, _| Box::new(Timer::new(host)),
};

impl Designable for Timer {
    fn get(&self, property: &str) -> Option<PropValue> {
        match property {
            "interval" => Some(PropValue::Int(self.interval() as i32)),
            "enabled" => Some(PropValue::Bool(self.enabled())),
            _ => None,
        }
    }
    fn set(&self, property: &str, value: PropValue) {
        match property {
            "interval" => {
                if let Some(ms) = value.into_int() {
                    self.set_interval(ms.max(1) as u32);
                }
            }
            "enabled" => {
                if let Some(enabled) = value.into_bool() {
                    self.set_enabled(enabled);
                }
            }
            _ => {}
        }
    }
    fn set_event(&self, event: &str, callback: Box<dyn FnMut()>) {
        if event == "tick" {
            self.set_on_tick(callback);
        }
    }
}

// --- Platforms: one `imp` per target; add a `#[cfg(rad_macos)]` one for macOS ---

#[cfg(rad_win32)]
mod imp {
    use super::*;
    use crate::platform::win32::{ffi, hwnd_key, register_message_hook, Parent, HWND};
    use std::cell::{Cell, RefCell};
    use std::collections::HashMap;
    use std::rc::Rc;

    /// Everything a tick needs, shared between a `Handle` and the per-thread
    /// table the message hook looks it up in (a `WM_TIMER` only carries the
    /// window and the timer id, not any Rust state).
    struct Shared {
        callback: RefCell<Option<Box<dyn FnMut()>>>,
        /// Cleared when the timer is dropped or its window is destroyed, so a
        /// tick that is mid-flight at that moment does not put its callback back.
        alive: Cell<bool>,
    }

    thread_local! {
        static TIMERS: RefCell<HashMap<(usize, usize), Rc<Shared>>> = RefCell::new(HashMap::new());
        static NEXT_TIMER_ID: Cell<usize> = const { Cell::new(1) };
    }

    /// Runs one tick. The callback is taken out of its slot while it runs, so it
    /// may freely stop the timer, change its interval, or install a replacement
    /// callback (all of which touch the same slot) without a re-entrancy panic.
    fn fire_timer(hwnd: HWND, id: usize) {
        let shared = TIMERS.with(|timers| timers.borrow().get(&(hwnd_key(hwnd), id)).cloned());
        let Some(shared) = shared else { return };
        let taken = shared.callback.borrow_mut().take();
        if let Some(mut callback) = taken {
            callback();
            // Put the callback back unless the handler replaced it or the timer
            // was dropped meanwhile.
            if shared.alive.get() && shared.callback.borrow().is_none() {
                *shared.callback.borrow_mut() = Some(callback);
            }
        }
    }

    /// Window teardown: Windows itself discards a destroyed window's timers, so
    /// only our bookkeeping needs clearing.
    fn forget_timers_of(hwnd: HWND) {
        TIMERS.with(|timers| {
            timers.borrow_mut().retain(|(owner, _), shared| {
                let keep = *owner != hwnd_key(hwnd);
                if !keep {
                    shared.alive.set(false);
                }
                keep
            });
        });
    }

    /// The window procedure's route to this component.
    fn on_message(hwnd: HWND, msg: u32, wparam: usize, _lparam: isize) -> Option<isize> {
        match msg {
            ffi::WM_TIMER => {
                fire_timer(hwnd, wparam);
                Some(0)
            }
            ffi::WM_DESTROY => {
                forget_timers_of(hwnd);
                None
            }
            _ => None,
        }
    }

    pub(super) fn create(parent: &Parent) -> Box<dyn TimerHandle> {
        register_message_hook(on_message);
        let id = NEXT_TIMER_ID.with(|next| {
            let id = next.get();
            next.set(id + 1);
            id
        });
        let shared = Rc::new(Shared { callback: RefCell::new(None), alive: Cell::new(true) });
        TIMERS.with(|timers| timers.borrow_mut().insert((hwnd_key(parent.hwnd), id), shared.clone()));
        // WinForms defaults: disabled, 100 ms.
        Box::new(Handle { hwnd: parent.hwnd, id, shared, interval_ms: Cell::new(100), enabled: Cell::new(false) })
    }

    struct Handle {
        hwnd: HWND,
        id: usize,
        shared: Rc<Shared>,
        interval_ms: Cell<u32>,
        enabled: Cell<bool>,
    }

    impl Handle {
        fn arm(&self) {
            // Windows clamps below ~10 ms and above i32::MAX on its own.
            unsafe { ffi::SetTimer(self.hwnd, self.id, self.interval_ms.get().max(1), std::ptr::null()) };
        }
    }

    impl TimerHandle for Handle {
        fn set_interval(&self, interval_ms: u32) {
            self.interval_ms.set(interval_ms);
            if self.enabled.get() {
                self.arm(); // SetTimer on an existing id just changes its period
            }
        }
        fn interval(&self) -> u32 {
            self.interval_ms.get()
        }
        fn set_enabled(&self, enabled: bool) {
            if enabled == self.enabled.get() {
                return;
            }
            self.enabled.set(enabled);
            if enabled {
                self.arm();
            } else {
                unsafe { ffi::KillTimer(self.hwnd, self.id) };
            }
        }
        fn enabled(&self) -> bool {
            self.enabled.get()
        }
        fn set_on_tick(&self, callback: Box<dyn FnMut()>) {
            *self.shared.callback.borrow_mut() = Some(callback);
        }
    }

    impl Drop for Handle {
        fn drop(&mut self) {
            self.shared.alive.set(false);
            unsafe { ffi::KillTimer(self.hwnd, self.id) };
            TIMERS.with(|timers| timers.borrow_mut().remove(&(hwnd_key(self.hwnd), self.id)));
        }
    }
}

#[cfg(rad_web)]
mod imp {
    use super::*;
    use crate::platform::web::Parent;
    use std::cell::{Cell, RefCell};
    use std::rc::Rc;
    use wasm_bindgen::prelude::*;
    use wasm_bindgen::JsCast;

    struct Slot {
        callback: RefCell<Option<Box<dyn FnMut()>>>,
        alive: Cell<bool>,
        in_tick: Cell<bool>,
    }

    impl Slot {
        /// Runs one tick with the callback taken out of its slot, so the handler
        /// can stop or retune the timer, or replace its own callback, without a
        /// re-entrancy panic.
        fn tick(&self) {
            let taken = self.callback.borrow_mut().take();
            if let Some(mut callback) = taken {
                self.in_tick.set(true);
                callback();
                self.in_tick.set(false);
                if self.alive.get() && self.callback.borrow().is_none() {
                    *self.callback.borrow_mut() = Some(callback);
                }
            }
        }
    }

    /// A `setInterval` timer. There is no DOM node: the browser's own timer is
    /// the whole implementation. One JS closure lives as long as the handle, so
    /// stopping from inside a tick never drops the closure that is running.
    struct Handle {
        slot: Rc<Slot>,
        closure: Closure<dyn FnMut()>,
        interval_ms: Cell<u32>,
        enabled: Cell<bool>,
        interval_id: Cell<Option<i32>>,
    }

    pub(super) fn create(_parent: &Parent) -> Box<dyn TimerHandle> {
        let slot = Rc::new(Slot {
            callback: RefCell::new(None),
            alive: Cell::new(true),
            in_tick: Cell::new(false),
        });
        let closure = {
            let slot = slot.clone();
            Closure::wrap(Box::new(move || slot.tick()) as Box<dyn FnMut()>)
        };
        // WinForms defaults: disabled, 100 ms.
        Box::new(Handle {
            slot,
            closure,
            interval_ms: Cell::new(100),
            enabled: Cell::new(false),
            interval_id: Cell::new(None),
        })
    }

    impl Handle {
        fn clear(&self) {
            if let (Some(id), Some(window)) = (self.interval_id.take(), web_sys::window()) {
                window.clear_interval_with_handle(id);
            }
        }

        fn arm(&self) {
            self.clear();
            if let Some(window) = web_sys::window() {
                let ms = self.interval_ms.get().clamp(1, i32::MAX as u32) as i32;
                if let Ok(id) = window.set_interval_with_callback_and_timeout_and_arguments_0(
                    self.closure.as_ref().unchecked_ref(),
                    ms,
                ) {
                    self.interval_id.set(Some(id));
                }
            }
        }
    }

    impl TimerHandle for Handle {
        fn set_interval(&self, interval_ms: u32) {
            self.interval_ms.set(interval_ms);
            if self.enabled.get() {
                self.arm();
            }
        }
        fn interval(&self) -> u32 {
            self.interval_ms.get()
        }
        fn set_enabled(&self, enabled: bool) {
            if enabled == self.enabled.get() {
                return;
            }
            self.enabled.set(enabled);
            if enabled {
                self.arm();
            } else {
                self.clear();
            }
        }
        fn enabled(&self) -> bool {
            self.enabled.get()
        }
        fn set_on_tick(&self, callback: Box<dyn FnMut()>) {
            *self.slot.callback.borrow_mut() = Some(callback);
        }
    }

    impl Drop for Handle {
        fn drop(&mut self) {
            self.slot.alive.set(false);
            self.clear();
            if self.slot.in_tick.get() {
                // Dropped from inside its own tick: that JS closure is on the
                // stack right now, so leak it instead of freeing it.
                let closure = std::mem::replace(&mut self.closure, Closure::wrap(Box::new(|| {}) as Box<dyn FnMut()>));
                closure.forget();
            }
        }
    }
}

#[cfg(rad_web_min)]
mod imp {
    use super::*;
    use crate::platform::web_min::{ffi, Parent};
    use std::cell::{Cell, RefCell};
    use std::rc::Rc;

    struct Slot {
        callback: RefCell<Option<Box<dyn FnMut()>>>,
        alive: Cell<bool>,
    }

    impl Slot {
        /// Runs the callback taken out of its slot, so a handler may stop or
        /// retune its own timer, or replace its own callback, without a
        /// re-entrancy panic.
        fn tick(&self) {
            let taken = self.callback.borrow_mut().take();
            if let Some(mut callback) = taken {
                callback();
                if self.alive.get() && self.callback.borrow().is_none() {
                    *self.callback.borrow_mut() = Some(callback);
                }
            }
        }
    }

    thread_local! {
        // Indexed by the id `rad_create_timer` hands out.
        static SLOTS: RefCell<Vec<Option<Rc<Slot>>>> = RefCell::new(Vec::new());
    }

    /// Called by the JS runtime on every `setInterval` tick of an enabled timer.
    #[unsafe(no_mangle)]
    pub extern "C" fn rad_dispatch_tick(timer_id: u32) {
        let slot = SLOTS.with(|slots| slots.borrow().get(timer_id as usize).cloned().flatten());
        if let Some(slot) = slot {
            slot.tick();
        }
    }

    pub(super) fn create(_parent: &Parent) -> Box<dyn TimerHandle> {
        let id = unsafe { ffi::rad_create_timer() };
        let slot = Rc::new(Slot { callback: RefCell::new(None), alive: Cell::new(true) });
        SLOTS.with(|slots| {
            let mut slots = slots.borrow_mut();
            if slots.len() <= id as usize {
                slots.resize(id as usize + 1, None);
            }
            slots[id as usize] = Some(slot.clone());
        });
        // WinForms defaults: disabled, 100 ms.
        Box::new(Handle { id, slot, interval_ms: Cell::new(100), enabled: Cell::new(false) })
    }

    struct Handle {
        id: u32,
        slot: Rc<Slot>,
        interval_ms: Cell<u32>,
        enabled: Cell<bool>,
    }

    impl Handle {
        fn apply(&self) {
            unsafe { ffi::rad_timer_set(self.id, self.interval_ms.get().max(1), self.enabled.get() as u32) };
        }
    }

    impl TimerHandle for Handle {
        fn set_interval(&self, interval_ms: u32) {
            self.interval_ms.set(interval_ms);
            if self.enabled.get() {
                self.apply();
            }
        }
        fn interval(&self) -> u32 {
            self.interval_ms.get()
        }
        fn set_enabled(&self, enabled: bool) {
            if enabled != self.enabled.get() {
                self.enabled.set(enabled);
                self.apply();
            }
        }
        fn enabled(&self) -> bool {
            self.enabled.get()
        }
        fn set_on_tick(&self, callback: Box<dyn FnMut()>) {
            *self.slot.callback.borrow_mut() = Some(callback);
        }
    }

    impl Drop for Handle {
        fn drop(&mut self) {
            self.slot.alive.set(false);
            unsafe { ffi::rad_timer_destroy(self.id) };
            SLOTS.with(|slots| {
                if let Some(entry) = slots.borrow_mut().get_mut(self.id as usize) {
                    *entry = None;
                }
            });
        }
    }
}
