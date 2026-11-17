use crate::widget::impl_widget_common;
use crate::{
    parse_lines, ComponentInfo, Designable, Host, Preview, PropInfo, PropType, PropValue, WidgetHandle,
};

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
        Self { handle: imp::create(&host.parent()) }
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

// --- Designer metadata ---

pub(crate) const INFO: ComponentInfo = ComponentInfo {
    name: "ComboBox",
    category: "Common Controls",
    icon: "\u{25BE}",
    order: 60,
    visual: true,
    default_size: (121, 21),
    ctor: &[],
    properties: &[
        PropInfo { name: "items", ty: PropType::Lines, default: "" },
        PropInfo { name: "selected_index", ty: PropType::Index, default: "-1" },
    ],
    events: &["change", "click"],
    preview: Preview { tag: "select", class: "", attrs: &[], css: "", labeled_input: None },
    create: |host, _| Box::new(ComboBox::new(host)),
};

impl Designable for ComboBox {
    fn get(&self, property: &str) -> Option<PropValue> {
        match property {
            "items" => Some(PropValue::Text(self.items().join("\n"))),
            "selected_index" => Some(PropValue::Int(self.selected_index().map_or(-1, |i| i as i32))),
            _ => None,
        }
    }
    fn set(&self, property: &str, value: PropValue) {
        match property {
            "items" => {
                if let Some(text) = value.into_text() {
                    // Every property edit re-applies all fields, and setting the
                    // items clears the selection, so leave an unchanged list alone.
                    let items = parse_lines(&text);
                    if items != self.items() {
                        self.set_items(&items);
                    }
                }
            }
            "selected_index" => {
                if let Some(index) = value.into_int() {
                    self.set_selected_index(usize::try_from(index).ok());
                }
            }
            _ => {}
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
        create_child, ffi, impl_widget_handle, mark_selection_control, selection_from, selection_wparam,
        set_change_handler, to_wide, Parent, Win32Widget,
    };
    use std::cell::RefCell;

    pub(super) fn create(parent: &Parent) -> Box<dyn ComboBoxHandle> {
        let hwnd = create_child(parent.hwnd, "COMBOBOX", ffi::CBS_DROPDOWNLIST | ffi::WS_VSCROLL, "");
        mark_selection_control(hwnd);
        let base = Win32Widget::new(hwnd);
        // Room for the drop-down list under the closed box.
        base.extra_height.set(150);
        base.apply_rect();
        Box::new(Handle { base, items: RefCell::new(Vec::new()) })
    }

    struct Handle {
        base: Win32Widget,
        items: RefCell<Vec<String>>,
    }
    impl_widget_handle!(Handle, base);
    impl ComboBoxHandle for Handle {
        fn set_items(&self, items: &[String]) {
            *self.items.borrow_mut() = items.to_vec();
            unsafe {
                ffi::SendMessageW(self.base.hwnd, ffi::CB_RESETCONTENT, 0, 0);
                for item in items {
                    let wide = to_wide(item);
                    ffi::SendMessageW(self.base.hwnd, ffi::CB_ADDSTRING, 0, wide.as_ptr() as isize);
                }
            }
        }
        fn items(&self) -> Vec<String> {
            self.items.borrow().clone()
        }
        fn set_selected_index(&self, index: Option<usize>) {
            let wparam = selection_wparam(index, self.items.borrow().len());
            unsafe { ffi::SendMessageW(self.base.hwnd, ffi::CB_SETCURSEL, wparam, 0) };
        }
        fn selected_index(&self) -> Option<usize> {
            selection_from(unsafe { ffi::SendMessageW(self.base.hwnd, ffi::CB_GETCURSEL, 0, 0) })
        }
        fn set_on_change(&self, callback: Box<dyn FnMut()>) {
            set_change_handler(self.base.hwnd, callback);
        }
    }
}

#[cfg(rad_gtk)]
mod imp {
    use super::*;
    use crate::platform::gtk::{connect, cstr, ffi, impl_widget_handle, programmatic, GtkBase, Parent};
    use std::cell::RefCell;

    pub(super) fn create(parent: &Parent) -> Box<dyn ComboBoxHandle> {
        let widget = unsafe { ffi::gtk_combo_box_text_new() };
        // A selection change is the control's click, as on Win32.
        let base = GtkBase::new(parent, widget, Some("changed"));
        connect(widget, "changed", &base.change);
        Box::new(Handle { base, items: RefCell::new(Vec::new()) })
    }

    struct Handle {
        base: GtkBase,
        items: RefCell<Vec<String>>,
    }
    impl_widget_handle!(Handle, base);
    impl ComboBoxHandle for Handle {
        fn set_items(&self, items: &[String]) {
            *self.items.borrow_mut() = items.to_vec();
            programmatic(|| unsafe {
                ffi::gtk_combo_box_text_remove_all(self.base.widget);
                for item in items {
                    ffi::gtk_combo_box_text_append_text(self.base.widget, cstr(item).as_ptr());
                }
            });
        }
        fn items(&self) -> Vec<String> {
            self.items.borrow().clone()
        }
        fn set_selected_index(&self, index: Option<usize>) {
            let index = match index {
                Some(i) if i < self.items.borrow().len() => i as i32,
                _ => -1,
            };
            programmatic(|| unsafe { ffi::gtk_combo_box_set_active(self.base.widget, index) });
        }
        fn selected_index(&self) -> Option<usize> {
            let active = unsafe { ffi::gtk_combo_box_get_active(self.base.widget) };
            if active < 0 { None } else { Some(active as usize) }
        }
        fn set_on_change(&self, callback: Box<dyn FnMut()>) {
            self.base.change.set(callback);
        }
    }
}

#[cfg(rad_web)]
mod imp {
    use super::*;
    use crate::platform::web::{el_set_listener, impl_widget_handle, Base, Parent};
    use std::cell::RefCell;
    use wasm_bindgen::prelude::Closure;
    use wasm_bindgen::JsCast;
    use web_sys::HtmlSelectElement;

    pub(super) fn create(parent: &Parent) -> Box<dyn ComboBoxHandle> {
        Box::new(Handle {
            base: Base::new(parent, &INFO.preview, None),
            items: RefCell::new(Vec::new()),
            on_change: RefCell::new(None),
        })
    }

    struct Handle {
        base: Base,
        items: RefCell<Vec<String>>,
        on_change: RefCell<Option<Closure<dyn FnMut()>>>,
    }
    impl Handle {
        fn select(&self) -> &HtmlSelectElement {
            self.base.el.unchecked_ref()
        }
    }
    impl_widget_handle!(Handle, base);
    impl ComboBoxHandle for Handle {
        fn set_items(&self, items: &[String]) {
            *self.items.borrow_mut() = items.to_vec();
            self.base.el.set_text_content(None);
            let document = self.base.el.owner_document().expect("el has a document");
            for item in items {
                if let Ok(option) = document.create_element("option") {
                    option.set_text_content(Some(item));
                    self.base.el.append_child(&option).ok();
                }
            }
            // Like the native controls, a fresh list starts with nothing selected
            // (browsers would otherwise preselect the first option of a combo).
            self.select().set_selected_index(-1);
        }
        fn items(&self) -> Vec<String> {
            self.items.borrow().clone()
        }
        fn set_selected_index(&self, index: Option<usize>) {
            let len = self.items.borrow().len();
            let value = match index {
                Some(i) if i < len => i as i32,
                _ => -1,
            };
            self.select().set_selected_index(value);
        }
        fn selected_index(&self) -> Option<usize> {
            let i = self.select().selected_index();
            if i < 0 { None } else { Some(i as usize) }
        }
        fn set_on_change(&self, callback: Box<dyn FnMut()>) {
            el_set_listener(&self.base.el, "change", &self.on_change, callback);
        }
    }
}

#[cfg(rad_web_min)]
mod imp {
    use super::*;
    use crate::platform::web_min::{ffi, impl_min_widget, register_change, ElementHandle, Parent};
    use std::cell::RefCell;

    pub(super) fn create(parent: &Parent) -> Box<dyn ComboBoxHandle> {
        Box::new(Handle { el: ElementHandle::create(parent, &INFO.preview, "", ""), items: RefCell::new(Vec::new()) })
    }

    struct Handle {
        el: ElementHandle,
        items: RefCell<Vec<String>>,
    }
    impl_min_widget!(Handle, el);
    impl ComboBoxHandle for Handle {
        fn set_items(&self, items: &[String]) {
            *self.items.borrow_mut() = items.to_vec();
            let joined = items.join("\n");
            unsafe { ffi::rad_set_list_items(self.el.id, joined.as_ptr(), joined.len() as u32) };
        }
        fn items(&self) -> Vec<String> {
            self.items.borrow().clone()
        }
        fn set_selected_index(&self, index: Option<usize>) {
            let len = self.items.borrow().len();
            let value = match index {
                Some(i) if i < len => i as i32,
                _ => -1,
            };
            unsafe { ffi::rad_set_selected_index(self.el.id, value) };
        }
        fn selected_index(&self) -> Option<usize> {
            let i = unsafe { ffi::rad_get_selected_index(self.el.id) };
            if i < 0 { None } else { Some(i as usize) }
        }
        fn set_on_change(&self, callback: Box<dyn FnMut()>) {
            register_change(self.el.id, callback);
        }
    }
}
