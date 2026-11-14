use crate::to_wide;
use rad_core::{Backend, ButtonHandle, CheckBoxHandle, Host, LabelHandle, WindowHandle};
use std::sync::Once;
use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, GetClientRect, GetWindowLongPtrW, GetWindowTextLengthW,
    GetWindowTextW, LoadIconW, PostQuitMessage, RegisterClassW, SendMessageW, SetWindowLongPtrW,
    SetWindowPos, SetWindowTextW, ShowWindow, CW_USEDEFAULT, GWLP_USERDATA, ICON_BIG, ICON_SMALL,
    IDI_APPLICATION, SWP_NOMOVE, SWP_NOZORDER, SW_SHOW, WM_DESTROY, WM_SETICON, WM_SIZE, WNDCLASSW,
    WS_OVERLAPPEDWINDOW, WS_VISIBLE,
};

const CLASS_NAME: &str = "RadWindow";

static REGISTER_CLASS: Once = Once::new();

fn register_class_once() {
    REGISTER_CLASS.call_once(|| unsafe {
        let class_name = to_wide(CLASS_NAME);
        let class = WNDCLASSW {
            style: 0,
            lpfnWndProc: Some(wndproc),
            cbClsExtra: 0,
            cbWndExtra: 0,
            hInstance: GetModuleHandleW(std::ptr::null()),
            hIcon: std::ptr::null_mut(),
            hCursor: std::ptr::null_mut(),
            hbrBackground: std::ptr::null_mut(),
            lpszMenuName: std::ptr::null(),
            lpszClassName: class_name.as_ptr(),
        };
        RegisterClassW(&class);
    });
}

/// The resize callback for a window, stashed in its `GWLP_USERDATA` slot so
/// the raw `wndproc` can reach it without any global registry.
type ResizeCallback = Box<dyn FnMut(u32, u32)>;

unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    unsafe {
        match msg {
            WM_SIZE => {
                let user_data = GetWindowLongPtrW(hwnd, GWLP_USERDATA);
                if user_data != 0 {
                    let callback = &mut *(user_data as *mut ResizeCallback);
                    let width = (lparam as u32) & 0xFFFF;
                    let height = ((lparam as u32) >> 16) & 0xFFFF;
                    callback(width, height);
                }
                0
            }
            WM_DESTROY => {
                PostQuitMessage(0);
                0
            }
            _ => DefWindowProcW(hwnd, msg, wparam, lparam),
        }
    }
}

/// Creates real top-level windows via `CreateWindowExW`.
pub struct Win32Backend;

impl Backend for Win32Backend {
    fn create_window(&self, title: &str, width: u32, height: u32) -> Box<dyn WindowHandle> {
        Box::new(Win32Window::new(title, width, height))
    }
}

pub struct Win32Window {
    hwnd: HWND,
    // Leaked into GWLP_USERDATA for wndproc's reach; reclaimed on drop.
    callback_slot: *mut ResizeCallback,
}

impl Win32Window {
    fn new(title: &str, width: u32, height: u32) -> Self {
        register_class_once();

        let class_name = to_wide(CLASS_NAME);
        let window_title = to_wide(title);

        let hwnd = unsafe {
            CreateWindowExW(
                0,
                class_name.as_ptr(),
                window_title.as_ptr(),
                WS_OVERLAPPEDWINDOW | WS_VISIBLE,
                CW_USEDEFAULT,
                CW_USEDEFAULT,
                width as i32,
                height as i32,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null(),
            )
        };
        unsafe { ShowWindow(hwnd, SW_SHOW) };

        // No-op callback until `set_on_resize` replaces it — keeps wndproc's
        // "is there a callback" check simple (pointer is always valid once set).
        let callback_slot = Box::into_raw(Box::new(Box::new(|_, _| {}) as ResizeCallback));
        unsafe { SetWindowLongPtrW(hwnd, GWLP_USERDATA, callback_slot as isize) };

        Self { hwnd, callback_slot }
    }
}

impl Drop for Win32Window {
    fn drop(&mut self) {
        unsafe { drop(Box::from_raw(self.callback_slot)) };
    }
}

impl WindowHandle for Win32Window {
    fn set_title(&self, title: &str) {
        let wide = to_wide(title);
        unsafe { SetWindowTextW(self.hwnd, wide.as_ptr()) };
    }

    fn title(&self) -> String {
        unsafe {
            let len = GetWindowTextLengthW(self.hwnd);
            if len <= 0 {
                return String::new();
            }
            let mut buf = vec![0u16; len as usize + 1];
            let copied = GetWindowTextW(self.hwnd, buf.as_mut_ptr(), buf.len() as i32);
            String::from_utf16_lossy(&buf[..copied.max(0) as usize])
        }
    }

    fn set_size(&self, width: u32, height: u32) {
        unsafe {
            SetWindowPos(
                self.hwnd,
                std::ptr::null_mut(),
                0,
                0,
                width as i32,
                height as i32,
                SWP_NOMOVE | SWP_NOZORDER,
            );
        }
    }

    fn size(&self) -> (u32, u32) {
        unsafe {
            let mut rect = std::mem::zeroed();
            GetClientRect(self.hwnd, &mut rect);
            ((rect.right - rect.left) as u32, (rect.bottom - rect.top) as u32)
        }
    }

    fn set_on_resize(&self, callback: Box<dyn FnMut(u32, u32)>) {
        unsafe {
            *self.callback_slot = callback;
        }
    }

    fn set_default_icon(&self) {
        unsafe {
            let icon = LoadIconW(std::ptr::null_mut(), IDI_APPLICATION);
            SendMessageW(self.hwnd, WM_SETICON, ICON_BIG as usize, icon as isize);
            SendMessageW(self.hwnd, WM_SETICON, ICON_SMALL as usize, icon as isize);
        }
    }
}

impl Host for Win32Window {
    fn create_button(&self, text: &str) -> Box<dyn ButtonHandle> {
        crate::create_button(self.hwnd, text)
    }

    fn create_label(&self, text: &str) -> Box<dyn LabelHandle> {
        crate::create_label(self.hwnd, text)
    }

    fn create_checkbox(&self, text: &str) -> Box<dyn CheckBoxHandle> {
        crate::create_checkbox(self.hwnd, text)
    }
}
