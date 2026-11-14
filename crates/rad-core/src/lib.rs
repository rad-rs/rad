//! Backend-agnostic widget model. A `Backend` implementation (native or
//! preview) provides the real rendering; this crate only describes *what*
//! a widget is and forwards property changes to it.

mod button;
mod checkbox;
mod label;
mod widget;
mod window;

pub use button::{Button, ButtonHandle};
pub use checkbox::{CheckBox, CheckBoxHandle};
pub use label::{Label, LabelHandle};
pub use widget::WidgetHandle;
pub use window::{Backend, Window, WindowHandle};

/// A platform family a backend renders for, or that a preview can emulate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OsSkin {
    Windows,
    MacOs,
    Linux,
}

/// A window/surface that can host widgets. Implemented per-backend. Each
/// control type gets its own `create_*` method — Rust has no way to
/// return "a widget" generically and still let callers use what's
/// specific to it (`CheckBox::checked`, etc.), so this is the one place
/// a new control type has to be named.
pub trait Host {
    fn create_button(&self, text: &str) -> Box<dyn ButtonHandle>;
    fn create_label(&self, text: &str) -> Box<dyn LabelHandle>;
    fn create_checkbox(&self, text: &str) -> Box<dyn CheckBoxHandle>;
}
