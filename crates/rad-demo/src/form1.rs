//! Hand-written half of Form1, same split WinForms uses: the designer owns
//! `form1.designer.rs` and regenerates it freely on every save; this file
//! is yours, and the designer never touches it. Event handlers and other
//! logic belong here, wired up against `Form1Controls`' fields.

#[path = "form1.designer.rs"]
mod designer;

pub use designer::Form1Controls;

use rad_core::Window;

pub fn build(window: &Window) -> Form1Controls {
    designer::build(window)
}
