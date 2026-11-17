//! Native Linux build of the all-controls demo: the same `Form1` that `rad-demo`
//! shows in the browser and `rad-demo-native` on Win32, on `rad-backend-gtk`.
//! Needs `libgtk-3-dev` to build. On other OSes this is an empty program, so a
//! workspace build there is unaffected.

#[cfg(target_os = "linux")]
#[path = "../../rad-demo/src/form1.rs"]
mod form1;

#[cfg(target_os = "linux")]
fn main() {
    use rad_core::Window;

    let backend = rad_backend_gtk::GtkBackend;
    let window = Window::new(&backend, "All controls demo", 460, 520);
    let controls = form1::build(&window);

    rad_backend_gtk::run();

    std::mem::forget(controls);
    std::mem::forget(window);
}

#[cfg(not(target_os = "linux"))]
fn main() {
    eprintln!("rad-demo-linux runs on Linux (GTK 3) only");
}
