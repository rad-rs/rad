//! Native Win32 backend with zero external crate dependencies. It
//! hand-declares the handful of WinAPI functions and constants it needs
//! instead of depending on `windows-sys`, an enormous generated-bindings
//! crate. That keeps it buildable two ways from the same source: as a
//! normal Cargo dependency, and as plain source mounted next to
//! `rad-core`'s files in the designer's no-Cargo build pipeline, which
//! has no way to fetch crates or produce `.rlib`s.


#![cfg(target_os = "windows")]

use rad_core::platform::win32::{
    dpi_scale, ffi, set_click_handler, set_window_text, to_wide, window_text, Parent, HWND,
};
use rad_core::{Backend, Host, WindowHandle};
use std::sync::Once;

const CLASS_NAME: &str = "RadWindowMin";
static REGISTER_CLASS: Once = Once::new();
static VISUAL_STYLES: Once = Once::new();

static DPI_AWARE: Once = Once::new();

/// Without this, Windows treats the whole process as DPI-unaware and
/// bitmap-stretches its windows to match the system DPI — every control
/// ends up visibly larger/blurrier than the designer's own preview
/// (which is just a browser page, already rendered at native
/// resolution) even though both asked for the same logical size.
fn make_process_dpi_aware_once() {
    DPI_AWARE.call_once(|| unsafe {
        ffi::SetProcessDPIAware();
    });
}

/// Without this, every control falls back to ComCtl32 v5 — the flat,
/// square, Windows-2000-era rendering, even on current Windows, since
/// that's what an unmanifested process gets by default. Normally this is
/// fixed by embedding a manifest resource at link time; that needs a
/// resource compiler (`rc.exe`/`windres`), which the no-Cargo pipeline
/// this crate exists for has no access to, so this does it at runtime
/// instead — a real, standalone manifest file, written to a temp path,
/// loaded into a process-wide activation context before any window or
/// control exists. Must run before `register_class_once`'s first
/// `CreateWindowExW`, not just before the main window's: the comctl32
/// version active at a *control's* creation is what decides its theme,
/// not the window that hosts it.
fn activate_visual_styles_once() {
    VISUAL_STYLES.call_once(|| {
        const MANIFEST: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<assembly xmlns="urn:schemas-microsoft-com:asm.v1" manifestVersion="1.0">
  <dependency>
    <dependentAssembly>
      <assemblyIdentity type="win32" name="Microsoft.Windows.Common-Controls" version="6.0.0.0" processorArchitecture="*" publicKeyToken="6595b64144ccf1df" language="*"/>
    </dependentAssembly>
  </dependency>
</assembly>
"#;
        let path = std::env::temp_dir().join("rad-visual-styles.manifest");
        if std::fs::write(&path, MANIFEST).is_err() {
            return;
        }
        let wide_path = to_wide(&path.to_string_lossy());
        let act_ctx = ffi::ActCtxW {
            cb_size: std::mem::size_of::<ffi::ActCtxW>() as u32,
            dw_flags: 0,
            lp_source: wide_path.as_ptr(),
            w_processor_architecture: 0,
            w_lang_id: 0,
            lp_assembly_directory: std::ptr::null(),
            lp_resource_name: std::ptr::null(),
            lp_application_name: std::ptr::null(),
            h_module: std::ptr::null_mut(),
        };
        unsafe {
            let handle = ffi::CreateActCtxW(&act_ctx);
            if (handle as isize) != -1 {
                let mut cookie: usize = 0;
                ffi::ActivateActCtx(handle, &mut cookie);
                // Never deactivated/closed — the context needs to stay
                // live for the whole process, same "leak it" reasoning
                // already used for other process-lifetime handles here.
            }
        }
    });
}

fn register_class_once() {
    REGISTER_CLASS.call_once(|| unsafe {
        let class_name = to_wide(CLASS_NAME);
        let class = ffi::WndClassW {
            style: 0,
            lpfn_wnd_proc: wndproc,
            cb_cls_extra: 0,
            cb_wnd_extra: 0,
            h_instance: ffi::GetModuleHandleW(std::ptr::null()),
            h_icon: std::ptr::null_mut(),
            h_cursor: std::ptr::null_mut(),
            // A system-color *index* used as a pseudo-brush handle, per
            // the `WNDCLASS.hbrBackground` convention: low-order word
            // is the color index plus one, not a real `HBRUSH` at all.
            hbr_background: (ffi::COLOR_BTNFACE + 1) as HWND,
            lpsz_menu_name: std::ptr::null(),
            lpsz_class_name: class_name.as_ptr(),
        };
        ffi::RegisterClassW(&class);
        // The class name buffer must outlive the registered class.
        std::mem::forget(class_name);
    });
}

type ResizeCallback = Box<dyn FnMut(u32, u32)>;

unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, wparam: ffi::WPARAM, lparam: ffi::LPARAM) -> ffi::LRESULT {
    unsafe {
        match msg {
            ffi::WM_SIZE => {
                let user_data = ffi::GetWindowLongPtrW(hwnd, ffi::GWLP_USERDATA);
                if user_data != 0 {
                    let callback = &mut *(user_data as *mut ResizeCallback);
                    // `lParam` here is in physical pixels; convert back
                    // to the logical units every other size in this
                    // crate uses.
                    let scale = dpi_scale();
                    let width = (((lparam as u32) & 0xFFFF) as f64 / scale).round() as u32;
                    let height = (((lparam as u32) >> 16 & 0xFFFF) as f64 / scale).round() as u32;
                    callback(width, height);
                }
                0
            }
            ffi::WM_DESTROY => {
                // Components keep per-window state (a timer's table); let them
                // drop it before the window goes.
                rad_core::platform::win32::window_destroyed(hwnd);
                ffi::PostQuitMessage(0);
                0
            }
            // Clicks, change notifications, colours and component hooks (a
            // timer's `WM_TIMER`) are the components' business; see
            // `rad_core::platform::win32::dispatch`.
            _ => match rad_core::platform::win32::dispatch(hwnd, msg, wparam, lparam) {
                Some(result) => result,
                None => ffi::DefWindowProcW(hwnd, msg, wparam, lparam),
            },
        }
    }
}

pub struct Win32Host {
    hwnd: HWND,
}

impl Win32Host {
    /// # Safety
    /// `hwnd` must be a valid window handle that outlives this `Win32Host`.
    pub unsafe fn from_hwnd(hwnd: HWND) -> Self {
        Self { hwnd }
    }
}

impl Host for Win32Host {
    fn parent(&self) -> Parent {
        Parent { hwnd: self.hwnd }
    }
}

pub struct Win32Backend;

impl Backend for Win32Backend {
    fn create_window(&self, title: &str, width: u32, height: u32) -> Box<dyn WindowHandle> {
        Box::new(Win32Window::new(title, width, height))
    }
}

pub struct Win32Window {
    hwnd: HWND,
    callback_slot: *mut ResizeCallback,
}

impl Win32Window {
    fn new(title: &str, width: u32, height: u32) -> Self {
        make_process_dpi_aware_once();
        activate_visual_styles_once();
        register_class_once();
        let class_name = to_wide(CLASS_NAME);
        let window_title = to_wide(title);
        let style = ffi::WS_OVERLAPPEDWINDOW | ffi::WS_VISIBLE;
        // `width`/`height` are the desired *client area* size — the
        // native-exe pipeline feeds this from the preview's own content
        // area (`WebWindow::body_size`), not its outer chrome total,
        // specifically so this matches regardless of how tall a real
        // Win32 title bar turns out to be versus the preview's
        // hand-drawn one. `CreateWindowExW` itself wants the outer
        // window size though, title bar and borders included — hence
        // `AdjustWindowRect` to grow from one to the other.
        let scale = dpi_scale();
        let scaled_width = (width as f64 * scale).round() as i32;
        let scaled_height = (height as f64 * scale).round() as i32;
        let mut rect = ffi::Rect { left: 0, top: 0, right: scaled_width, bottom: scaled_height };
        unsafe { ffi::AdjustWindowRect(&mut rect, style, 0) };
        let hwnd = unsafe {
            ffi::CreateWindowExW(
                0,
                class_name.as_ptr(),
                window_title.as_ptr(),
                style,
                ffi::CW_USEDEFAULT,
                ffi::CW_USEDEFAULT,
                rect.right - rect.left,
                rect.bottom - rect.top,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null(),
            )
        };
        unsafe { ffi::ShowWindow(hwnd, ffi::SW_SHOW) };

        let callback_slot = Box::into_raw(Box::new(Box::new(|_, _| {}) as ResizeCallback));
        unsafe { ffi::SetWindowLongPtrW(hwnd, ffi::GWLP_USERDATA, callback_slot as isize) };

        Self { hwnd, callback_slot }
    }
}

impl WindowHandle for Win32Window {
    fn set_title(&self, title: &str) {
        set_window_text(self.hwnd, title);
    }
    fn title(&self) -> String {
        window_text(self.hwnd)
    }
    fn set_size(&self, width: u32, height: u32) {
        // Same client-area and logical-vs-physical conventions
        // `Win32Window::new` uses.
        let style = ffi::WS_OVERLAPPEDWINDOW | ffi::WS_VISIBLE;
        let scale = dpi_scale();
        let scaled_width = (width as f64 * scale).round() as i32;
        let scaled_height = (height as f64 * scale).round() as i32;
        let mut rect = ffi::Rect { left: 0, top: 0, right: scaled_width, bottom: scaled_height };
        unsafe {
            ffi::AdjustWindowRect(&mut rect, style, 0);
            ffi::SetWindowPos(
                self.hwnd,
                std::ptr::null_mut(),
                0,
                0,
                rect.right - rect.left,
                rect.bottom - rect.top,
                ffi::SWP_NOMOVE | ffi::SWP_NOZORDER,
            );
        }
    }
    fn size(&self) -> (u32, u32) {
        // `GetClientRect` reports physical pixels; convert back to the
        // same logical units `set_size`/`Win32Window::new` take.
        let scale = dpi_scale();
        unsafe {
            let mut rect = ffi::Rect { left: 0, top: 0, right: 0, bottom: 0 };
            ffi::GetClientRect(self.hwnd, &mut rect);
            (
                ((rect.right - rect.left) as f64 / scale).round() as u32,
                ((rect.bottom - rect.top) as f64 / scale).round() as u32,
            )
        }
    }
    fn set_on_resize(&self, callback: Box<dyn FnMut(u32, u32)>) {
        unsafe { *self.callback_slot = callback };
    }
    fn set_on_click(&self, callback: Box<dyn FnMut()>) {
        set_click_handler(self.hwnd, callback);
    }
    fn set_default_icon(&self) {
        unsafe {
            let icon = ffi::LoadIconW(std::ptr::null_mut(), ffi::IDI_APPLICATION);
            ffi::SendMessageW(self.hwnd, ffi::WM_SETICON, ffi::ICON_BIG, icon as isize);
            ffi::SendMessageW(self.hwnd, ffi::WM_SETICON, ffi::ICON_SMALL, icon as isize);
        }
    }
}

impl Host for Win32Window {
    fn parent(&self) -> Parent {
        Parent { hwnd: self.hwnd }
    }
}


// Window must outlive the process in every real use of this backend
// (the message loop owns it for the app's whole lifetime), so it has no
// `Drop` that frees `callback_slot` — same reasoning `rad-backend-win32`
// itself documents.
