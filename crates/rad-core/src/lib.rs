//! Backend-agnostic widget model. A `Backend` implementation (native or
//! preview) provides the window and message loop; each control is a
//! *component*, one file in `src/components/` holding its model, its code for
//! every platform and its designer metadata (see `component.rs`).

mod component;
pub mod platform;
mod widget;
mod window;

/// Every component, one file each in `src/components/`; the module list, the
/// re-exports and `COMPONENTS` are generated from whatever files are there.
pub mod components {
    include!(concat!(env!("OUT_DIR"), "/components.rs"));
}

pub use component::{
    format_tree_text, parse_lines, parse_tree_text, ComponentInfo, CtorArg, Designable, Preview, PropInfo,
    PropType, PropValue,
};
pub use components::*;
pub use widget::{Color, WidgetHandle};
pub use window::{Backend, Window, WindowHandle};

/// A platform family a backend renders for, or that a preview can emulate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OsSkin {
    Windows,
    MacOs,
    Linux,
}

/// A window/surface that can host controls. Implemented per-backend, and
/// deliberately tiny: how a control is created is the component's own business
/// (see `src/components/`), so adding a control type never touches this trait.
pub trait Host {
    /// What a component needs to create a native child on this host.
    fn parent(&self) -> platform::Parent;
}
