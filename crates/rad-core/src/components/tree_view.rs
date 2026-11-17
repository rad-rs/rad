use crate::widget::impl_widget_common;
use crate::{
    format_tree_text, parse_tree_text, ComponentInfo, Designable, Host, Preview, PropInfo, PropType, PropValue,
    WidgetHandle,
};

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
        Self { handle: imp::create(&host.parent()) }
    }

    pub fn set_items(&self, items: &[TreeItem]) {
        self.handle.set_items(items);
    }

    pub fn items(&self) -> Vec<TreeItem> {
        self.handle.items()
    }
}

impl_widget_common!(TreeView);

// --- Designer metadata ---

pub(crate) const INFO: ComponentInfo = ComponentInfo {
    name: "TreeView",
    category: "Display",
    icon: "\u{229E}",
    order: 130,
    visual: true,
    default_size: (121, 97),
    ctor: &[],
    properties: &[PropInfo { name: "items", ty: PropType::Tree, default: "" }],
    events: &["click"],
    preview: Preview {
        tag: "div",
        class: "",
        attrs: &[],
        css: "overflow: auto; border: 1px solid #adadad; background: #fff;",
        labeled_input: None,
    },
    create: |host, _| Box::new(TreeView::new(host)),
};

impl Designable for TreeView {
    fn get(&self, property: &str) -> Option<PropValue> {
        match property {
            "items" => Some(PropValue::Text(format_tree_text(&self.items()))),
            _ => None,
        }
    }
    fn set(&self, property: &str, value: PropValue) {
        if let ("items", Some(text)) = (property, value.into_text()) {
            self.set_items(&parse_tree_text(&text));
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
    use crate::platform::win32::{
        create_child, ffi, impl_widget_handle, init_common_controls_once, to_wide, Parent, Win32Widget,
    };
    use std::cell::RefCell;

    pub(super) fn create(parent: &Parent) -> Box<dyn TreeViewHandle> {
        init_common_controls_once();
        let style = ffi::TVS_HASBUTTONS | ffi::TVS_HASLINES | ffi::TVS_LINESATROOT | ffi::WS_BORDER;
        let hwnd = create_child(parent.hwnd, "SysTreeView32", style, "");
        Box::new(Handle { base: Win32Widget::new(hwnd), items: RefCell::new(Vec::new()) })
    }

    struct Handle {
        base: Win32Widget,
        items: RefCell<Vec<TreeItem>>,
    }
    impl_widget_handle!(Handle, base);
    impl TreeViewHandle for Handle {
        fn set_items(&self, items: &[TreeItem]) {
            *self.items.borrow_mut() = items.to_vec();
            unsafe {
                ffi::SendMessageW(self.base.hwnd, ffi::TVM_DELETEITEM, 0, ffi::TVI_ROOT);
            }
            // Each item's parent is whatever's currently on top of the
            // stack for the depth one shallower than it — the simplest way
            // to turn a flat `(depth, text)` list into a real tree without
            // tracking explicit parent ids.
            let mut parent_at_depth: Vec<isize> = vec![ffi::TVI_ROOT];
            for (depth, text) in items {
                let depth = *depth as usize;
                parent_at_depth.truncate(depth + 1);
                let h_parent = *parent_at_depth.last().unwrap_or(&ffi::TVI_ROOT);
                let mut wide_text = to_wide(text);
                let insert = ffi::TvInsertStructW {
                    h_parent,
                    h_insert_after: ffi::TVI_LAST,
                    mask: ffi::TVIF_TEXT,
                    h_item: 0,
                    state: 0,
                    state_mask: 0,
                    psz_text: wide_text.as_mut_ptr(),
                    cch_text_max: wide_text.len() as i32,
                    i_image: 0,
                    i_selected_image: 0,
                    c_children: 0,
                    l_param: 0,
                    i_integral: 0,
                    u_state_ex: 0,
                    hwnd: std::ptr::null_mut(),
                    i_expanded_image: 0,
                    i_reserved: 0,
                };
                let new_item =
                    unsafe { ffi::SendMessageW(self.base.hwnd, ffi::TVM_INSERTITEMW, 0, &insert as *const _ as isize) };
                parent_at_depth.push(new_item);
            }
        }
        fn items(&self) -> Vec<TreeItem> {
            self.items.borrow().clone()
        }
    }
}

#[cfg(rad_gtk)]
mod imp {
    use super::*;
    use crate::platform::gtk::{cstr, ffi, impl_widget_handle, scrolled_around, GtkBase, Parent};
    use std::cell::RefCell;

    pub(super) fn create(parent: &Parent) -> Box<dyn TreeViewHandle> {
        unsafe {
            // One string column, shown without a header.
            let types = [ffi::G_TYPE_STRING];
            let store = ffi::gtk_tree_store_newv(1, types.as_ptr());
            let view = ffi::gtk_tree_view_new_with_model(store);
            ffi::gtk_tree_view_set_headers_visible(view, 0);
            let renderer = ffi::gtk_cell_renderer_text_new();
            let column = ffi::gtk_tree_view_column_new_with_attributes(
                c"".as_ptr(),
                renderer,
                c"text".as_ptr(),
                0 as std::ffi::c_int,
                std::ptr::null::<std::ffi::c_void>(),
            );
            ffi::gtk_tree_view_append_column(view, column);
            let base = GtkBase::new(parent, scrolled_around(view), None);
            Box::new(Handle { base, view, store, items: RefCell::new(Vec::new()) })
        }
    }

    struct Handle {
        base: GtkBase,
        view: *mut ffi::GtkWidget,
        store: *mut std::ffi::c_void,
        items: RefCell<Vec<TreeItem>>,
    }
    impl_widget_handle!(Handle, base);
    impl TreeViewHandle for Handle {
        fn set_items(&self, items: &[TreeItem]) {
            *self.items.borrow_mut() = items.to_vec();
            unsafe {
                ffi::gtk_tree_store_clear(self.store);
                // The parent of an item is the last one seen one level shallower:
                // `path[d]` is the most recent item at depth `d`.
                let mut path: Vec<ffi::GtkTreeIter> = Vec::new();
                for (depth, text) in items {
                    let depth = (*depth as usize).min(path.len());
                    path.truncate(depth);
                    let mut iter = std::mem::zeroed::<ffi::GtkTreeIter>();
                    let parent = if depth == 0 { std::ptr::null() } else { &path[depth - 1] as *const _ };
                    ffi::gtk_tree_store_append(self.store, &mut iter, parent);
                    ffi::gtk_tree_store_set(self.store, &mut iter, 0 as std::ffi::c_int, cstr(text).as_ptr(), -1 as std::ffi::c_int);
                    path.push(iter);
                }
                ffi::gtk_tree_view_expand_all(self.view);
            }
        }
        fn items(&self) -> Vec<TreeItem> {
            self.items.borrow().clone()
        }
    }
}

#[cfg(rad_web)]
mod imp {
    use super::*;
    use crate::platform::web::{create_element, impl_widget_handle, Base, Parent};
    use std::cell::RefCell;

    pub(super) fn create(parent: &Parent) -> Box<dyn TreeViewHandle> {
        Box::new(Handle { base: Base::new(parent, &INFO.preview, None), items: RefCell::new(Vec::new()) })
    }

    /// `items` is kept alongside the DOM rather than read back from it —
    /// rebuilding the rows on every `set_items` is simpler than parsing them
    /// back into `(depth, text)` pairs.
    struct Handle {
        base: Base,
        items: RefCell<Vec<TreeItem>>,
    }
    impl_widget_handle!(Handle, base);
    impl TreeViewHandle for Handle {
        fn set_items(&self, items: &[TreeItem]) {
            *self.items.borrow_mut() = items.to_vec();
            let document = self.base.el.owner_document().expect("el has a document");
            self.base.el.set_text_content(None);
            // One flat list, indenting each row by its depth rather than
            // building real nested `<ul>`s — simplest way to render a
            // depth-only hierarchy without tracking which rows are each
            // other's ancestors.
            for (depth, text) in self.items.borrow().iter() {
                let row = create_element(&document, &self.base.el, "div");
                row.set_text_content(Some(text));
                row.style().set_property("padding-left", &format!("{}px", depth * 16)).ok();
                row.style().set_property("white-space", "nowrap").ok();
            }
        }
        fn items(&self) -> Vec<TreeItem> {
            self.items.borrow().clone()
        }
    }
}

#[cfg(rad_web_min)]
mod imp {
    use super::*;
    use crate::platform::web_min::{ffi, impl_min_widget, ElementHandle, Parent};
    use std::cell::RefCell;

    pub(super) fn create(parent: &Parent) -> Box<dyn TreeViewHandle> {
        Box::new(Handle { el: ElementHandle::create(parent, &INFO.preview, "", ""), items: RefCell::new(Vec::new()) })
    }

    struct Handle {
        el: ElementHandle,
        items: RefCell<Vec<TreeItem>>,
    }
    impl_min_widget!(Handle, el);
    impl TreeViewHandle for Handle {
        fn set_items(&self, items: &[TreeItem]) {
            *self.items.borrow_mut() = items.to_vec();
            let encoded = items
                .iter()
                .map(|(depth, text)| format!("{depth}\t{text}"))
                .collect::<Vec<_>>()
                .join("\n");
            unsafe { ffi::rad_set_tree_items(self.el.id, encoded.as_ptr(), encoded.len() as u32) };
        }
        fn items(&self) -> Vec<TreeItem> {
            self.items.borrow().clone()
        }
    }
}
