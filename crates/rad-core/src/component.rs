//! What a component file declares about itself, beyond the control's own API:
//! the metadata the designer needs (toolbox entry, default size, properties,
//! events) and the declarative preview both browser platforms build their DOM
//! from. Everything here is plain data and a couple of traits, so it works with
//! plain rustc: no proc-macros, no codegen at the use site.

use crate::{Host, WidgetHandle};

/// The kind of a property, which decides how the designer edits it and how
/// `form1.designer.rs` spells its value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PropType {
    Bool,
    Int,
    /// One line of text.
    Text,
    /// A file path or URL.
    Path,
    /// One item per line (`ComboBox`, `ListBox`).
    Lines,
    /// One item per line, indented two spaces per depth level (`TreeView`).
    Tree,
    /// A list selection: an index, `-1` for none.
    Index,
}

/// A property's value as the designer sees it. `Text` carries every text-like
/// type (`Text`, `Path`, `Lines`, `Tree`) in the form the properties panel edits.
#[derive(Debug, Clone, PartialEq)]
pub enum PropValue {
    Bool(bool),
    Int(i32),
    Text(String),
}

impl PropValue {
    pub fn into_bool(self) -> Option<bool> {
        if let Self::Bool(v) = self { Some(v) } else { None }
    }
    pub fn into_int(self) -> Option<i32> {
        if let Self::Int(v) = self { Some(v) } else { None }
    }
    pub fn into_text(self) -> Option<String> {
        if let Self::Text(v) = self { Some(v) } else { None }
    }
}

/// One property. Its setter is `set_<name>` on the control, which is also how
/// the generated `form1.designer.rs` sets it.
#[derive(Debug)]
pub struct PropInfo {
    pub name: &'static str,
    pub ty: PropType,
    /// The default, as the designer shows it (`"false"`, `"100"`, `""`). A
    /// property grid shows values that differ from this in bold.
    pub default: &'static str,
}

/// One argument of the control's `new`, after the host.
#[derive(Debug, Clone, Copy)]
pub enum CtorArg {
    /// The control's initial text (the designer passes the control's name).
    Text,
    /// A fixed Rust expression, e.g. `"\"group1\""`.
    Lit(&'static str),
}

/// A declarative description of the DOM a browser platform builds for the
/// control, so adding one never means touching the web backends or runtime.js.
#[derive(Debug)]
pub struct Preview {
    pub tag: &'static str,
    pub class: &'static str,
    pub attrs: &'static [(&'static str, &'static str)],
    /// Inline style, as in a `style` attribute.
    pub css: &'static str,
    /// `Some("checkbox")` builds `<tag><input type=...><span>text</span></tag>`
    /// (the native HTML pattern: clicking the text toggles the input).
    pub labeled_input: Option<&'static str>,
}

impl Preview {
    /// For a component with no DOM of its own (a timer).
    pub const NONE: Preview = Preview { tag: "", class: "", attrs: &[], css: "", labeled_input: None };

    /// `key=value` lines, the form `runtime.js` parses when it creates an element.
    pub fn to_spec(&self) -> String {
        let mut spec = format!("tag={}\n", self.tag);
        if !self.class.is_empty() {
            spec.push_str(&format!("class={}\n", self.class));
        }
        for (name, value) in self.attrs {
            spec.push_str(&format!("attr={name}={value}\n"));
        }
        if !self.css.is_empty() {
            spec.push_str(&format!("css={}\n", self.css));
        }
        if let Some(kind) = self.labeled_input {
            spec.push_str(&format!("labeled_input={kind}\n"));
        }
        spec
    }
}

/// A component's registry entry; one per file in `src/components/`.
pub struct ComponentInfo {
    /// The type's name, which is also the toolbox label and the `data-kind`.
    pub name: &'static str,
    pub category: &'static str,
    /// A short glyph for the toolbox tile.
    pub icon: &'static str,
    /// Toolbox position: sorted by this, grouped by category.
    pub order: u32,
    /// `false` for a non-visual component (a timer): no position, size or DOM,
    /// shown in the component tray instead of on the form.
    pub visual: bool,
    /// Size of a freshly dropped control, in logical pixels.
    pub default_size: (u32, u32),
    pub ctor: &'static [CtorArg],
    pub properties: &'static [PropInfo],
    /// Event names; the first is the default (what a double-click stubs out).
    /// An event `x` is wired with `set_on_x`.
    pub events: &'static [&'static str],
    pub preview: Preview,
    /// Builds an instance on `host`; `text` is the initial text for a control
    /// whose constructor takes one.
    pub create: fn(&dyn Host, &str) -> Box<dyn Designable>,
}

impl ComponentInfo {
    pub fn property(&self, name: &str) -> Option<&PropInfo> {
        self.properties.iter().find(|p| p.name == name)
    }

    pub fn default_event(&self) -> &'static str {
        self.events.first().copied().unwrap_or("click")
    }
}

/// The designer's uniform view of a placed component. Implemented in each
/// component's file, next to its properties.
pub trait Designable {
    /// The property's value, or `None` if this component has no such property.
    fn get(&self, property: &str) -> Option<PropValue>;
    /// Sets a property; unknown names and mismatched types are ignored.
    fn set(&self, property: &str, value: PropValue);
    /// Position/size access; `None` for a non-visual component.
    fn widget(&self) -> Option<&dyn WidgetHandle> {
        None
    }
    /// Attaches `callback` to the named event, if the component raises it.
    fn set_event(&self, _event: &str, _callback: Box<dyn FnMut()>) {}
}

/// Two spaces of indent per depth level: simple to type and to read back
/// unambiguously, unlike tabs.
pub fn format_tree_text(items: &[crate::TreeItem]) -> String {
    items
        .iter()
        .map(|(depth, text)| format!("{}{}", "  ".repeat(*depth as usize), text))
        .collect::<Vec<_>>()
        .join("\n")
}

pub fn parse_tree_text(text: &str) -> Vec<crate::TreeItem> {
    text.lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| {
            let leading_spaces = line.len() - line.trim_start_matches(' ').len();
            ((leading_spaces / 2) as u32, line.trim().to_string())
        })
        .collect()
}

/// One item per line, surrounding spaces ignored, blank lines dropped.
pub fn parse_lines(text: &str) -> Vec<String> {
    text.lines().map(str::trim).filter(|line| !line.is_empty()).map(str::to_string).collect()
}
