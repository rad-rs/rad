//! Per-platform plumbing that component files build on: how to create a native
//! child, positions and colours, event tables. Each platform is one module,
//! compiled only where it applies, and exports a `Parent`: what a component
//! needs from its host to create a control.
//!
//! The platform is chosen by one of the `rad_win32`, `rad_web` and `rad_web_min`
//! cfgs. Under Cargo, `build.rs` sets them from the target and the `web` /
//! `web-min` features; the designer's in-browser pipeline, which has no Cargo,
//! passes `--cfg` to rustc itself.
//!
//! A component's own platform code lives in its file, in a `mod imp` per
//! platform gated by the same cfg. To add a platform (macOS, say), add its
//! module here and a `mod imp` to each component.

#[cfg(rad_win32)]
pub mod win32;
#[cfg(rad_win32)]
pub use win32::Parent;

#[cfg(rad_web)]
pub mod web;
#[cfg(rad_web)]
pub use web::Parent;

#[cfg(rad_web_min)]
pub mod web_min;
#[cfg(rad_web_min)]
pub use web_min::Parent;

#[cfg(not(any(rad_win32, rad_web, rad_web_min)))]
compile_error!(
    "rad-core has no platform for this target: build for Windows, or for wasm32 with the `web` or `web-min` feature"
);
