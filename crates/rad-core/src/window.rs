use crate::Host;

/// A top-level, user-resizable surface that hosts controls. A `Window` is
/// itself a `Host` — controls are placed directly on it, same as any other
/// container.
///
/// Every setter takes `&self`, not `&mut self`: the mutation actually
/// happens on the OS/DOM side (an `HWND`, a DOM node), not in Rust-owned
/// data, so there's nothing for the borrow checker to protect here. This
/// lets a `Window` be shared via `Rc` and still be driven from several
/// places at once (a property panel, a resize handle, ...).
pub trait WindowHandle: Host {
    fn set_title(&self, title: &str);
    fn title(&self) -> String;

    fn set_size(&self, width: u32, height: u32);
    fn size(&self) -> (u32, u32);

    /// Replaces the resize callback, fired with the new client size
    /// whenever the user (or code) resizes the window.
    fn set_on_resize(&self, callback: Box<dyn FnMut(u32, u32)>);

    /// Wires the form's default event — a click anywhere on it that
    /// didn't land on one of its controls.
    fn set_on_click(&self, callback: Box<dyn FnMut()>);

    /// Applies a generic default icon, the way every new WinForms form
    /// gets one even before you set a custom one. No-op for a backend
    /// that has nothing sensible to show (default here, not `unimplemented!`,
    /// so a backend only needs to override this once it actually has an
    /// icon story).
    fn set_default_icon(&self) {}
}

/// Creates top-level windows. Implemented per-backend.
pub trait Backend {
    fn create_window(&self, title: &str, width: u32, height: u32) -> Box<dyn WindowHandle>;
}

/// A resizable top-level window. Thin wrapper over a backend's
/// `WindowHandle`, and itself a `Host` so controls can be placed on it
/// directly: `Button::new(&window, "OK")`.
pub struct Window {
    handle: Box<dyn WindowHandle>,
}

impl Window {
    pub fn new(backend: &dyn Backend, title: &str, width: u32, height: u32) -> Self {
        let handle = backend.create_window(title, width, height);
        handle.set_default_icon();
        Self { handle }
    }

    pub fn set_title(&self, title: &str) {
        self.handle.set_title(title);
    }

    pub fn title(&self) -> String {
        self.handle.title()
    }

    pub fn set_size(&self, width: u32, height: u32) {
        self.handle.set_size(width, height);
    }

    pub fn size(&self) -> (u32, u32) {
        self.handle.size()
    }

    /// Runs `callback` with the new client size whenever the window is
    /// resized, including by the user dragging its edge.
    pub fn on_resize(&self, callback: impl FnMut(u32, u32) + 'static) {
        self.handle.set_on_resize(Box::new(callback));
    }

    pub fn on_click(&self, callback: impl FnMut() + 'static) {
        self.handle.set_on_click(Box::new(callback));
    }
}

impl Host for Window {
    fn create_button(&self, text: &str) -> Box<dyn crate::ButtonHandle> {
        self.handle.create_button(text)
    }

    fn create_label(&self, text: &str) -> Box<dyn crate::LabelHandle> {
        self.handle.create_label(text)
    }

    fn create_checkbox(&self, text: &str) -> Box<dyn crate::CheckBoxHandle> {
        self.handle.create_checkbox(text)
    }

    fn create_textbox(&self, text: &str) -> Box<dyn crate::TextBoxHandle> {
        self.handle.create_textbox(text)
    }

    fn create_radio_button(&self, group: &str, text: &str) -> Box<dyn crate::RadioButtonHandle> {
        self.handle.create_radio_button(group, text)
    }

    fn create_progress_bar(&self) -> Box<dyn crate::ProgressBarHandle> {
        self.handle.create_progress_bar()
    }

    fn create_picture_box(&self) -> Box<dyn crate::PictureBoxHandle> {
        self.handle.create_picture_box()
    }

    fn create_tree_view(&self) -> Box<dyn crate::TreeViewHandle> {
        self.handle.create_tree_view()
    }

    fn create_timer(&self) -> Box<dyn crate::TimerHandle> {
        self.handle.create_timer()
    }

    fn create_track_bar(&self) -> Box<dyn crate::TrackBarHandle> {
        self.handle.create_track_bar()
    }

    fn create_combo_box(&self) -> Box<dyn crate::ComboBoxHandle> {
        self.handle.create_combo_box()
    }

    fn create_list_box(&self) -> Box<dyn crate::ListBoxHandle> {
        self.handle.create_list_box()
    }
}
