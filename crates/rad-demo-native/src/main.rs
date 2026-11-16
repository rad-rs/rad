//! Real native Windows demo: a Form1 with one Button, built entirely on
//! `rad-backend-win32` — no WASM, no browser, just a normal `cargo build`
//! producing a real `.exe`. `rad-backend-win32::Win32Backend` only creates
//! the window; running it needs the usual Win32 message loop, which this
//! entry point owns (same role `rad-demo`'s wasm entry point plays for
//! the browser backend).

use rad_core::{Button, Window};
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
    let window = Window::new(&backend, "Form1", 400, 260);

    let button = Button::new(&window, "Click me");
    button.set_pos(20, 20);
    button.set_width(100);
    button.set_height(32);
    button.set_on_click(Box::new(|| {
        println!("Button clicked!");
    }));

    window.on_click(|| {
        println!("Form clicked!");
    });

    unsafe {
        let mut msg: [usize; 7] = [0; 7];
        while GetMessageW(&mut msg, core::ptr::null_mut(), 0, 0) != 0 {
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }

    std::mem::forget(window);
    std::mem::forget(button);
}
