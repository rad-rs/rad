use crate::widget::impl_widget_common;
use crate::{Host, WidgetHandle};

/// One row: its label and how deeply nested it is (`0` = a root item).
/// Nesting is inferred from depth rather than explicit parent ids —
/// simple to author from the properties panel (one line of text per
/// item, indented), though it means every item is a sibling of
/// whatever's at the same depth immediately above it. A real parent/child
/// id scheme (and collapse/expand state, icons, ...) is a reasonable
/// follow-up, not yet implemented.
pub type TreeItem = (u32, String);

pub trait TreeViewHandle: WidgetHandle {
    fn set_items(&self, items: &[TreeItem]);
    fn items(&self) -> Vec<TreeItem>;
}

/// Displays a collapsible hierarchy of items.
pub struct TreeView {
    handle: Box<dyn TreeViewHandle>,
}

impl TreeView {
    pub fn new(host: &dyn Host) -> Self {
        Self { handle: host.create_tree_view() }
    }

    pub fn set_items(&self, items: &[TreeItem]) {
        self.handle.set_items(items);
    }

    pub fn items(&self) -> Vec<TreeItem> {
        self.handle.items()
    }
}

impl_widget_common!(TreeView);
