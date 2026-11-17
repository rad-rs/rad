//! WASM preview backend: renders widgets as styled DOM nodes inside a
//! container element. Not the real app — this is the browser-side
//! prototyping surface. The OS skin is a CSS class on the container,
//! swapped at runtime to flip the preview's look between platforms.

// Only meaningful in the browser: `rad-core`'s `web` platform exists for wasm32 alone.
#![cfg(target_arch = "wasm32")]

mod window;

pub use window::{WebBackend, WebWindow};

use rad_core::platform::web::Parent;
use rad_core::{Host, OsSkin};
use web_sys::{Document, Element};

fn skin_class(skin: OsSkin) -> &'static str {
    match skin {
        OsSkin::Windows => "rad-skin-windows",
        OsSkin::MacOs => "rad-skin-macos",
        OsSkin::Linux => "rad-skin-linux",
    }
}

/// A preview surface: one DOM element that widgets are appended into.
pub struct WebHost {
    document: Document,
    container: Element,
}

impl WebHost {
    /// Mounts a preview into `container`, skinned for `skin`.
    pub fn new(container: Element, skin: OsSkin) -> Self {
        let document = container
            .owner_document()
            .expect("container must be attached to a document");
        let host = Self { document, container };
        host.set_skin(skin);
        host
    }

    /// Switches the preview's look without touching any widget state.
    pub fn set_skin(&self, skin: OsSkin) {
        let class_list = self.container.class_list();
        for s in [OsSkin::Windows, OsSkin::MacOs, OsSkin::Linux] {
            let _ = class_list.remove_1(skin_class(s));
        }
        let _ = class_list.add_1(skin_class(skin));
    }
}

impl Host for WebHost {
    fn parent(&self) -> Parent {
        Parent { document: self.document.clone(), container: self.container.clone(), draggable: false }
    }
}
