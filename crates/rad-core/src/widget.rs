/// An RGB color, for `set_fore_color`/`set_back_color`. No alpha — every
/// backend here renders controls as fully opaque (or, for a back color
/// of `None`, lets the parent show through entirely), so a translucency
/// channel would have nothing to mean.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl Color {
    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }
}

/// Position and size — the only things every placed control has in
/// common, regardless of what it is. A backend's handle for any control
/// type implements this once, the same way, so adding a new control type
/// never means re-deriving pos/size plumbing.
///
/// Every setter takes `&self`, not `&mut self`, for the same reason
/// `WindowHandle`'s do: the mutation happens on the OS/DOM side, not in
/// Rust-owned data. This is what lets an event handler hold onto a
/// control (via `Rc`) and mutate it later from inside a closure that
/// outlives whatever first created it.
pub trait WidgetHandle {
    fn set_pos(&self, x: i32, y: i32);
    fn pos(&self) -> (i32, i32);

    fn set_width(&self, width: u32);
    fn width(&self) -> u32;

    fn set_height(&self, height: u32);
    fn height(&self) -> u32;

    /// Wires the control's default event — click, for everything so far.
    /// Takes `&self` like `WindowHandle::set_on_resize`: the listener
    /// attaches to the OS/DOM widget itself, not to any state owned here.
    fn set_on_click(&self, callback: Box<dyn FnMut()>);

    /// `None` means inherited — the control renders text in whatever
    /// color its parent would use (`ControlText` by default), rather
    /// than a color of its own. Same ambient-property model as
    /// `back_color`.
    fn set_fore_color(&self, color: Option<Color>);
    fn fore_color(&self) -> Option<Color>;

    /// `None` means transparent — the control shows whatever's behind
    /// it (its parent's own background) rather than painting one of its
    /// own. This is the default every control starts with; a backend
    /// that can't actually render transparency falls back to matching
    /// its parent's background color instead; see each backend's own
    /// handle implementation for exactly how.
    fn set_back_color(&self, color: Option<Color>);
    fn back_color(&self) -> Option<Color>;
}

/// Generates the pos/size forwarding methods on a wrapper struct (one
/// with a `handle: Box<dyn SomeHandle>` field, where `SomeHandle:
/// WidgetHandle`). Every control wrapper (`Button`, `Label`, ...) needs
/// these same six methods; this is the one place they're written.
macro_rules! impl_widget_common {
    ($ty:ty) => {
        impl $ty {
            pub fn set_pos(&self, x: i32, y: i32) {
                self.handle.set_pos(x, y);
            }

            pub fn pos(&self) -> (i32, i32) {
                self.handle.pos()
            }

            pub fn set_width(&self, width: u32) {
                self.handle.set_width(width);
            }

            pub fn width(&self) -> u32 {
                self.handle.width()
            }

            pub fn set_height(&self, height: u32) {
                self.handle.set_height(height);
            }

            pub fn height(&self) -> u32 {
                self.handle.height()
            }

            pub fn set_on_click(&self, callback: Box<dyn FnMut()>) {
                self.handle.set_on_click(callback);
            }

            pub fn set_fore_color(&self, color: Option<crate::Color>) {
                self.handle.set_fore_color(color);
            }

            pub fn fore_color(&self) -> Option<crate::Color> {
                self.handle.fore_color()
            }

            pub fn set_back_color(&self, color: Option<crate::Color>) {
                self.handle.set_back_color(color);
            }

            pub fn back_color(&self) -> Option<crate::Color> {
                self.handle.back_color()
            }
        }
    };
}

pub(crate) use impl_widget_common;
