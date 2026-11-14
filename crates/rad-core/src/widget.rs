/// Position and size — the only things every placed control has in
/// common, regardless of what it is. A backend's handle for any control
/// type implements this once, the same way, so adding a new control type
/// never means re-deriving pos/size plumbing.
pub trait WidgetHandle {
    fn set_pos(&mut self, x: i32, y: i32);
    fn pos(&self) -> (i32, i32);

    fn set_width(&mut self, width: u32);
    fn width(&self) -> u32;

    fn set_height(&mut self, height: u32);
    fn height(&self) -> u32;
}

/// Generates the pos/size forwarding methods on a wrapper struct (one
/// with a `handle: Box<dyn SomeHandle>` field, where `SomeHandle:
/// WidgetHandle`). Every control wrapper (`Button`, `Label`, ...) needs
/// these same six methods; this is the one place they're written.
macro_rules! impl_widget_common {
    ($ty:ty) => {
        impl $ty {
            pub fn set_pos(&mut self, x: i32, y: i32) {
                self.handle.set_pos(x, y);
            }

            pub fn pos(&self) -> (i32, i32) {
                self.handle.pos()
            }

            pub fn set_width(&mut self, width: u32) {
                self.handle.set_width(width);
            }

            pub fn width(&self) -> u32 {
                self.handle.width()
            }

            pub fn set_height(&mut self, height: u32) {
                self.handle.set_height(height);
            }

            pub fn height(&self) -> u32 {
                self.handle.height()
            }
        }
    };
}

pub(crate) use impl_widget_common;
