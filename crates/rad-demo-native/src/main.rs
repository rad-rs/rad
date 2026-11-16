//! Native Windows build of the all-controls demo: the same `Form1` that
//! `rad-demo` shows in the browser, on `rad-backend-win32`. No WASM, no
//! browser, just a normal `cargo build` producing a real `.exe`. The
//! backend only creates the window; running it needs the usual Win32
//! message loop, which this entry point owns.

#[path = "../../rad-demo/src/form1.rs"]
mod form1;

use rad_core::Window;

// The backend has no external dependencies, so the message loop is declared
// by hand too. `MSG` is 48 bytes; `[usize; 7]` (56) is correctly aligned and
// large enough, since the OS only writes within the real struct's bounds.
#[link(name = "user32")]
unsafe extern "system" {
    fn GetMessageW(msg: *mut [usize; 7], hwnd: *mut core::ffi::c_void, min: u32, max: u32) -> i32;
    fn TranslateMessage(msg: *const [usize; 7]) -> i32;
    fn DispatchMessageW(msg: *const [usize; 7]) -> isize;
}

fn main() {
    let backend = rad_backend_win32::Win32Backend;
    let window = Window::new(&backend, "All controls demo", 460, 410);
    let controls = form1::build(&window);

    unsafe {
        let mut msg: [usize; 7] = [0; 7];
        while GetMessageW(&mut msg, core::ptr::null_mut(), 0, 0) != 0 {
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }

    std::mem::forget(controls);
    std::mem::forget(window);
}
