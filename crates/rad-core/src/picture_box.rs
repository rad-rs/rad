use crate::widget::impl_widget_common;
use crate::{Host, WidgetHandle};

/// Minimal for now — an image referenced by a filesystem/URL path, not
/// embedded bytes. Good enough for a first pass; embedding (so a built
/// app doesn't depend on files sitting next to its `.exe`) is a
/// reasonable follow-up, not yet implemented.
pub trait PictureBoxHandle: WidgetHandle {
    fn set_image_path(&self, path: &str);
    fn image_path(&self) -> String;
}

/// Displays an image.
pub struct PictureBox {
    handle: Box<dyn PictureBoxHandle>,
}

impl PictureBox {
    pub fn new(host: &dyn Host) -> Self {
        Self { handle: host.create_picture_box() }
    }

    pub fn set_image_path(&self, path: &str) {
        self.handle.set_image_path(path);
    }

    pub fn image_path(&self) -> String {
        self.handle.image_path()
    }
}

impl_widget_common!(PictureBox);
