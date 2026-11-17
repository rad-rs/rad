use crate::widget::impl_widget_common;
use crate::{ComponentInfo, Designable, Host, Preview, PropInfo, PropType, PropValue, WidgetHandle};

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
        Self { handle: imp::create(&host.parent()) }
    }

    pub fn set_image_path(&self, path: &str) {
        self.handle.set_image_path(path);
    }

    pub fn image_path(&self) -> String {
        self.handle.image_path()
    }
}

impl_widget_common!(PictureBox);

// --- Designer metadata ---

pub(crate) const INFO: ComponentInfo = ComponentInfo {
    name: "PictureBox",
    category: "Display",
    icon: "\u{25A2}",
    order: 120,
    visual: true,
    default_size: (100, 50),
    ctor: &[],
    properties: &[PropInfo { name: "image_path", ty: PropType::Path, default: "" }],
    events: &["click"],
    preview: Preview {
        tag: "img",
        class: "",
        attrs: &[],
        css: "object-fit: contain;",
        labeled_input: None,
    },
    create: |host, _| Box::new(PictureBox::new(host)),
};

impl Designable for PictureBox {
    fn get(&self, property: &str) -> Option<PropValue> {
        match property {
            "image_path" => Some(PropValue::Text(self.image_path())),
            _ => None,
        }
    }
    fn set(&self, property: &str, value: PropValue) {
        if let ("image_path", Some(path)) = (property, value.into_text()) {
            self.set_image_path(&path);
        }
    }
    fn widget(&self) -> Option<&dyn WidgetHandle> {
        Some(self)
    }
}

// --- Platforms: one `imp` per target; add a `#[cfg(rad_macos)]` one for macOS ---

#[cfg(rad_win32)]
mod imp {
    use super::*;
    use crate::platform::win32::{create_child, ffi, hwnd_key, impl_widget_handle, to_wide, Parent, Win32Widget};
    use std::cell::RefCell;
    use std::collections::HashMap;

    pub(super) fn create(parent: &Parent) -> Box<dyn PictureBoxHandle> {
        let hwnd = create_child(parent.hwnd, "STATIC", ffi::SS_BITMAP, "");
        Box::new(Handle { base: Win32Widget::new(hwnd) })
    }

    thread_local! {
        // No native getter for a `STATIC`'s `STM_SETIMAGE` path, so this
        // record is the only one — same reasoning as the colour caches.
        static IMAGE_PATHS: RefCell<HashMap<usize, String>> = RefCell::new(HashMap::new());
    }

    struct Handle {
        base: Win32Widget,
    }
    impl_widget_handle!(Handle, base);
    impl PictureBoxHandle for Handle {
        fn set_image_path(&self, path: &str) {
            IMAGE_PATHS.with(|paths| paths.borrow_mut().insert(hwnd_key(self.base.hwnd), path.to_string()));
            let wide_path = to_wide(path);
            unsafe {
                // Only `.bmp` loads this way — no GDI+ in this no-dependency
                // backend to decode PNG/JPEG. A real image-format story is a
                // reasonable follow-up, not yet implemented.
                let bitmap = ffi::LoadImageW(
                    std::ptr::null_mut(),
                    wide_path.as_ptr(),
                    ffi::IMAGE_BITMAP as u32,
                    0,
                    0,
                    ffi::LR_LOADFROMFILE,
                );
                ffi::SendMessageW(self.base.hwnd, ffi::STM_SETIMAGE, ffi::IMAGE_BITMAP, bitmap as isize);
            }
        }
        fn image_path(&self) -> String {
            IMAGE_PATHS.with(|paths| paths.borrow().get(&hwnd_key(self.base.hwnd)).cloned().unwrap_or_default())
        }
    }
}

#[cfg(rad_gtk)]
mod imp {
    use super::*;
    use crate::platform::gtk::{cstr, event_box_around, ffi, impl_widget_handle, GtkBase, Parent};
    use std::cell::RefCell;

    pub(super) fn create(parent: &Parent) -> Box<dyn PictureBoxHandle> {
        // The event box takes clicks and the back colour; the image sits inside.
        let image = unsafe { ffi::gtk_image_new() };
        let base = GtkBase::new(parent, event_box_around(image), None);
        Box::new(Handle { base, image, path: RefCell::new(String::new()) })
    }

    struct Handle {
        base: GtkBase,
        image: *mut ffi::GtkWidget,
        path: RefCell<String>,
    }
    impl_widget_handle!(Handle, base);
    impl PictureBoxHandle for Handle {
        fn set_image_path(&self, path: &str) {
            *self.path.borrow_mut() = path.to_string();
            // gdk-pixbuf decodes whatever loaders are installed (PNG, JPEG, BMP...).
            unsafe { ffi::gtk_image_set_from_file(self.image, cstr(path).as_ptr()) };
        }
        fn image_path(&self) -> String {
            self.path.borrow().clone()
        }
    }
}

#[cfg(rad_web)]
mod imp {
    use super::*;
    use crate::platform::web::{impl_widget_handle, Base, Parent};

    pub(super) fn create(parent: &Parent) -> Box<dyn PictureBoxHandle> {
        Box::new(Handle { base: Base::new(parent, &INFO.preview, None) })
    }

    struct Handle {
        base: Base,
    }
    impl_widget_handle!(Handle, base);
    impl PictureBoxHandle for Handle {
        fn set_image_path(&self, path: &str) {
            self.base.el.set_attribute("src", path).ok();
        }
        fn image_path(&self) -> String {
            self.base.el.get_attribute("src").unwrap_or_default()
        }
    }
}

#[cfg(rad_web_min)]
mod imp {
    use super::*;
    use crate::platform::web_min::{impl_min_widget, ElementHandle, Parent};

    pub(super) fn create(parent: &Parent) -> Box<dyn PictureBoxHandle> {
        Box::new(Handle { el: ElementHandle::create(parent, &INFO.preview, "", "") })
    }

    struct Handle {
        el: ElementHandle,
    }
    impl_min_widget!(Handle, el);
    impl PictureBoxHandle for Handle {
        // Reuses the generic text FFI — an `<img>`'s "text" is its `src` on the
        // JS side (see `runtime.js`).
        fn set_image_path(&self, path: &str) {
            self.el.set_text(path);
        }
        fn image_path(&self) -> String {
            self.el.text()
        }
    }
}
