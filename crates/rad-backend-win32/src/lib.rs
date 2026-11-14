//! Native Win32 backend: real child windows (`BUTTON`, `STATIC`, ...) via
//! raw FFI. No GUI toolkit — only `windows-sys`, which is a thin bindings
//! crate over the Win32 API itself, so the resulting executable has
//! effectively no runtime dependency beyond what Windows already ships.

#![cfg(windows)]

mod window;

pub use window::{Win32Backend, Win32Window};

use rad_core::{ButtonHandle, CheckBoxHandle, Host, LabelHandle, WidgetHandle};
use std::cell::Cell;
use windows_sys::Win32::Foundation::HWND;
use windows_sys::Win32::System::SystemServices::SS_LEFT;
use windows_sys::Win32::UI::Controls::{BST_CHECKED, BST_UNCHECKED};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, GetWindowTextLengthW, GetWindowTextW, SendMessageW, SetWindowPos,
    SetWindowTextW, ShowWindow, BM_GETCHECK, BM_SETCHECK, BS_AUTOCHECKBOX, BS_PUSHBUTTON,
    SWP_NOZORDER, SW_SHOW, WS_CHILD, WS_VISIBLE,
};

fn to_wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

fn set_window_text(hwnd: HWND, text: &str) {
    let wide = to_wide(text);
    unsafe { SetWindowTextW(hwnd, wide.as_ptr()) };
}

fn window_text(hwnd: HWND) -> String {
    unsafe {
        let len = GetWindowTextLengthW(hwnd);
        if len <= 0 {
            return String::new();
        }
        let mut buf = vec![0u16; len as usize + 1];
        let copied = GetWindowTextW(hwnd, buf.as_mut_ptr(), buf.len() as i32);
        String::from_utf16_lossy(&buf[..copied.max(0) as usize])
    }
}

/// Creates a native child window of `class_name` under `parent`. Shared
/// by every control type this backend supports — adding a new one is
/// just picking the right class name and style flags.
fn create_child(parent: HWND, class_name: &str, style: u32, text: &str) -> HWND {
    let class_name = to_wide(class_name);
    let window_text = to_wide(text);
    unsafe {
        let hwnd = CreateWindowExW(
            0,
            class_name.as_ptr(),
            window_text.as_ptr(),
            WS_CHILD | WS_VISIBLE | style,
            0,
            0,
            80,
            24,
            parent,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            std::ptr::null(),
        );
        ShowWindow(hwnd, SW_SHOW);
        hwnd
    }
}

/// The position/size state every native child window needs, since Win32
/// has no single "get my rect relative to parent" call as cheap as
/// tracking it ourselves and reapplying on each change.
pub(crate) struct Win32Widget {
    hwnd: HWND,
    x: Cell<i32>,
    y: Cell<i32>,
    width: Cell<u32>,
    height: Cell<u32>,
}

impl Win32Widget {
    fn new(hwnd: HWND) -> Self {
        Self {
            hwnd,
            x: Cell::new(0),
            y: Cell::new(0),
            width: Cell::new(80),
            height: Cell::new(24),
        }
    }

    fn apply_rect(&self) {
        unsafe {
            SetWindowPos(
                self.hwnd,
                std::ptr::null_mut(),
                self.x.get(),
                self.y.get(),
                self.width.get() as i32,
                self.height.get() as i32,
                SWP_NOZORDER,
            );
        }
    }
}

/// Generates a `WidgetHandle` impl for `$ty` from a `$field: Win32Widget`.
/// Every control handle in this backend needs this same impl.
macro_rules! impl_widget_handle {
    ($ty:ty, $field:ident) => {
        impl WidgetHandle for $ty {
            fn set_pos(&mut self, x: i32, y: i32) {
                self.$field.x.set(x);
                self.$field.y.set(y);
                self.$field.apply_rect();
            }
            fn pos(&self) -> (i32, i32) {
                (self.$field.x.get(), self.$field.y.get())
            }
            fn set_width(&mut self, width: u32) {
                self.$field.width.set(width);
                self.$field.apply_rect();
            }
            fn width(&self) -> u32 {
                self.$field.width.get()
            }
            fn set_height(&mut self, height: u32) {
                self.$field.height.set(height);
                self.$field.apply_rect();
            }
            fn height(&self) -> u32 {
                self.$field.height.get()
            }
        }
    };
}

/// A native top-level window that widgets are created as children of.
pub struct Win32Host {
    hwnd: HWND,
}

impl Win32Host {
    /// Wraps an already-created top-level window. Window creation itself
    /// (class registration, message loop) belongs to the app crate, not
    /// this backend — a `Host` only needs somewhere to parent controls.
    pub unsafe fn from_hwnd(hwnd: HWND) -> Self {
        Self { hwnd }
    }
}

impl Host for Win32Host {
    fn create_button(&self, text: &str) -> Box<dyn ButtonHandle> {
        create_button(self.hwnd, text)
    }

    fn create_label(&self, text: &str) -> Box<dyn LabelHandle> {
        create_label(self.hwnd, text)
    }

    fn create_checkbox(&self, text: &str) -> Box<dyn CheckBoxHandle> {
        create_checkbox(self.hwnd, text)
    }
}

pub(crate) fn create_button(parent: HWND, text: &str) -> Box<dyn ButtonHandle> {
    let hwnd = create_child(parent, "BUTTON", BS_PUSHBUTTON as u32, text);
    Box::new(Win32ButtonHandle { base: Win32Widget::new(hwnd) })
}

pub(crate) fn create_label(parent: HWND, text: &str) -> Box<dyn LabelHandle> {
    let hwnd = create_child(parent, "STATIC", SS_LEFT as u32, text);
    Box::new(Win32LabelHandle { base: Win32Widget::new(hwnd) })
}

pub(crate) fn create_checkbox(parent: HWND, text: &str) -> Box<dyn CheckBoxHandle> {
    let hwnd = create_child(parent, "BUTTON", BS_AUTOCHECKBOX as u32, text);
    Box::new(Win32CheckBoxHandle { base: Win32Widget::new(hwnd) })
}

pub(crate) struct Win32ButtonHandle {
    base: Win32Widget,
}

impl_widget_handle!(Win32ButtonHandle, base);

impl ButtonHandle for Win32ButtonHandle {
    fn set_text(&mut self, text: &str) {
        set_window_text(self.base.hwnd, text);
    }

    fn text(&self) -> String {
        window_text(self.base.hwnd)
    }
}

pub(crate) struct Win32LabelHandle {
    base: Win32Widget,
}

impl_widget_handle!(Win32LabelHandle, base);

impl LabelHandle for Win32LabelHandle {
    fn set_text(&mut self, text: &str) {
        set_window_text(self.base.hwnd, text);
    }

    fn text(&self) -> String {
        window_text(self.base.hwnd)
    }
}

pub(crate) struct Win32CheckBoxHandle {
    base: Win32Widget,
}

impl_widget_handle!(Win32CheckBoxHandle, base);

impl CheckBoxHandle for Win32CheckBoxHandle {
    fn set_text(&mut self, text: &str) {
        set_window_text(self.base.hwnd, text);
    }

    fn text(&self) -> String {
        window_text(self.base.hwnd)
    }

    fn set_checked(&mut self, checked: bool) {
        let state = if checked { BST_CHECKED } else { BST_UNCHECKED };
        unsafe { SendMessageW(self.base.hwnd, BM_SETCHECK, state as usize, 0) };
    }

    fn checked(&self) -> bool {
        unsafe { SendMessageW(self.base.hwnd, BM_GETCHECK, 0, 0) == BST_CHECKED as isize }
    }
}
