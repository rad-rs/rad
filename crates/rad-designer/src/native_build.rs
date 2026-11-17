//! The native-`.exe` counterpart to `build.rs`: same mount-tree idea,
//! `rad-backend-win32` in place of `rad-backend-web-min`, and a Win32
//! message loop in place of `runApp`. See `build.rs`'s doc comment for
//! why mounting source directly (not `--extern`/`.rlib`) is how every
//! build in this designer works. The same crate is also an ordinary
//! Cargo dependency of `rad-demo-native`; it has no external dependencies,
//! so both uses compile identical source.

use js_sys::Map;

macro_rules! rad_core_file {
    ($name:literal) => {
        include_str!(concat!("../../rad-core/src/", $name))
    };
}

const RAD_CORE_LIB: &str = rad_core_file!("lib.rs");
const RAD_CORE_BUTTON: &str = rad_core_file!("button.rs");
const RAD_CORE_CHECKBOX: &str = rad_core_file!("checkbox.rs");
const RAD_CORE_LABEL: &str = rad_core_file!("label.rs");
const RAD_CORE_PICTURE_BOX: &str = rad_core_file!("picture_box.rs");
const RAD_CORE_PROGRESS_BAR: &str = rad_core_file!("progress_bar.rs");
const RAD_CORE_TREE_VIEW: &str = rad_core_file!("tree_view.rs");
const RAD_CORE_RADIO_BUTTON: &str = rad_core_file!("radio_button.rs");
const RAD_CORE_TEXTBOX: &str = rad_core_file!("textbox.rs");
const RAD_CORE_TIMER: &str = rad_core_file!("timer.rs");
const RAD_CORE_TRACK_BAR: &str = rad_core_file!("track_bar.rs");
const RAD_CORE_COMBO_BOX: &str = rad_core_file!("combo_box.rs");
const RAD_CORE_LIST_BOX: &str = rad_core_file!("list_box.rs");
const RAD_CORE_WIDGET: &str = rad_core_file!("widget.rs");
const RAD_CORE_WINDOW: &str = rad_core_file!("window.rs");

const RAD_BACKEND_WIN32_MIN: &str = include_str!("../../rad-backend-win32/src/lib.rs");

fn as_rad_core_submodule(src: &str) -> String {
    src.replace("crate::", "crate::rad_core::")
}

fn as_mounted_rad_core_user(src: &str) -> String {
    src.replace("rad_core::", "crate::rad_core::")
}

pub fn build_native_mount_files(main_rs: &str, form1_rs: &str, form1_designer_rs: &str) -> Map {
    let map = Map::new();
    map.set(&"/rad_core.rs".into(), &RAD_CORE_LIB.into());
    map.set(&"/rad_core/button.rs".into(), &as_rad_core_submodule(RAD_CORE_BUTTON).into());
    map.set(&"/rad_core/checkbox.rs".into(), &as_rad_core_submodule(RAD_CORE_CHECKBOX).into());
    map.set(&"/rad_core/label.rs".into(), &as_rad_core_submodule(RAD_CORE_LABEL).into());
    map.set(&"/rad_core/picture_box.rs".into(), &as_rad_core_submodule(RAD_CORE_PICTURE_BOX).into());
    map.set(&"/rad_core/progress_bar.rs".into(), &as_rad_core_submodule(RAD_CORE_PROGRESS_BAR).into());
    map.set(&"/rad_core/tree_view.rs".into(), &as_rad_core_submodule(RAD_CORE_TREE_VIEW).into());
    map.set(&"/rad_core/radio_button.rs".into(), &as_rad_core_submodule(RAD_CORE_RADIO_BUTTON).into());
    map.set(&"/rad_core/textbox.rs".into(), &as_rad_core_submodule(RAD_CORE_TEXTBOX).into());
    map.set(&"/rad_core/timer.rs".into(), &as_rad_core_submodule(RAD_CORE_TIMER).into());
    map.set(&"/rad_core/track_bar.rs".into(), &as_rad_core_submodule(RAD_CORE_TRACK_BAR).into());
    map.set(&"/rad_core/combo_box.rs".into(), &as_rad_core_submodule(RAD_CORE_COMBO_BOX).into());
    map.set(&"/rad_core/list_box.rs".into(), &as_rad_core_submodule(RAD_CORE_LIST_BOX).into());
    map.set(&"/rad_core/widget.rs".into(), &as_rad_core_submodule(RAD_CORE_WIDGET).into());
    map.set(&"/rad_core/window.rs".into(), &as_rad_core_submodule(RAD_CORE_WINDOW).into());

    map.set(
        &"/rad_backend_win32.rs".into(),
        &RAD_BACKEND_WIN32_MIN.replace("rad_core::", "crate::rad_core::").into(),
    );

    map.set(&"/main.rs".into(), &main_rs.into());
    map.set(&"/form1.rs".into(), &as_mounted_rad_core_user(form1_rs).into());
    map.set(&"/form1.designer.rs".into(), &as_mounted_rad_core_user(form1_designer_rs).into());
    map
}

/// The generated app's entry point: build the window on
/// `rad-backend-win32`, then own the Win32 message loop —
/// `Win32Backend` only ever creates the window; nothing runs it without
/// one, the same role `rad-demo-native`'s hand-written entry point plays
/// for the real `rad-backend-win32`. `GetMessageW`'s `MSG` out-param is
/// declared here as a correctly-aligned `[usize; 7]` (56 bytes on 64-bit
/// where 48 are needed; on 32-bit exactly the 28 needed) rather than a
/// proper struct — harmless since
/// the OS only ever writes within the real struct's bounds, and it
/// means this entry point needs no FFI declarations beyond what it
/// calls directly, matching the point of mounting source instead of
/// `--extern`-ing a crate: nothing here depends on anything we can't see.
pub fn generate_native_main_source(width: u32, height: u32) -> String {
    format!(
        "mod rad_core;\n\
         mod rad_backend_win32;\n\
         #[path = \"form1.rs\"]\n\
         mod form1;\n\n\
         use rad_core::Window;\n\n\
         #[link(name = \"user32\")]\n\
         unsafe extern \"system\" {{\n\
         \x20   fn GetMessageW(lpmsg: *mut [usize; 7], hwnd: *mut core::ffi::c_void, wmsgfiltermin: u32, wmsgfiltermax: u32) -> i32;\n\
         \x20   fn TranslateMessage(lpmsg: *const [usize; 7]) -> i32;\n\
         \x20   fn DispatchMessageW(lpmsg: *const [usize; 7]) -> isize;\n\
         }}\n\n\
         fn main() {{\n\
         \x20   let backend = rad_backend_win32::Win32Backend;\n\
         \x20   let window = Window::new(&backend, \"Form1\", {width}, {height});\n\
         \x20   let form = form1::build(&window);\n\n\
         \x20   unsafe {{\n\
         \x20       let mut msg: [usize; 7] = [0; 7];\n\
         \x20       while GetMessageW(&mut msg, core::ptr::null_mut(), 0, 0) != 0 {{\n\
         \x20           TranslateMessage(&msg);\n\
         \x20           DispatchMessageW(&msg);\n\
         \x20       }}\n\
         \x20   }}\n\n\
         \x20   std::mem::forget(form);\n\
         \x20   std::mem::forget(window);\n\
         }}\n"
    )
}
