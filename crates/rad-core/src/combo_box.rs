use crate::widget::impl_widget_common;
use crate::{Host, WidgetHandle};

pub trait ComboBoxHandle: WidgetHandle {
    /// Replaces the whole list. The selection is cleared.
    fn set_items(&self, items: &[String]);
    fn items(&self) -> Vec<String>;
    /// `None` selects nothing. An index past the end is treated as `None`.
    fn set_selected_index(&self, index: Option<usize>);
    fn selected_index(&self) -> Option<usize>;
    /// Fires when the user picks a different item. Not fired for
    /// `set_selected_index` calls made from code.
    fn set_on_change(&self, callback: Box<dyn FnMut()>);
}

/// A drop-down list: the user picks one of a fixed set of items.
pub struct ComboBox {
    handle: Box<dyn ComboBoxHandle>,
}

impl ComboBox {
    pub fn new(host: &dyn Host) -> Self {
        Self { handle: host.create_combo_box() }
    }

    pub fn set_items(&self, items: &[String]) {
        self.handle.set_items(items);
    }

    pub fn items(&self) -> Vec<String> {
        self.handle.items()
    }

    pub fn set_selected_index(&self, index: Option<usize>) {
        self.handle.set_selected_index(index);
    }

    pub fn selected_index(&self) -> Option<usize> {
        self.handle.selected_index()
    }

    /// The selected item's text, or an empty string when nothing is selected.
    pub fn text(&self) -> String {
        self.selected_index()
            .and_then(|i| self.items().get(i).cloned())
            .unwrap_or_default()
    }

    pub fn set_on_change(&self, callback: Box<dyn FnMut()>) {
        self.handle.set_on_change(callback);
    }
}

impl_widget_common!(ComboBox);
