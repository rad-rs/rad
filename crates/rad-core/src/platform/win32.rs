//! Win32 plumbing shared by every component's `#[cfg(target_os = "windows")]`
//! implementation: hand-declared WinAPI (no external crates, so the same source
//! builds under Cargo and as mounted source in the designer's in-browser
//! pipeline), child-window creation, DPI scaling, per-window handler tables,
//! colours, and the message dispatch the backend's window procedure forwards to.
//! The backend itself only owns the top-level window and the message loop.

use crate::Color;
use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet};
use std::sync::Once;

pub mod ffi {
    use std::ffi::c_void;

    pub type HWND = *mut c_void;
    pub type WPARAM = usize;
    pub type LPARAM = isize;
    pub type LRESULT = isize;

    pub const WS_CHILD: u32 = 0x4000_0000;
    pub const WS_VISIBLE: u32 = 0x1000_0000;
    pub const WS_BORDER: u32 = 0x0080_0000;
    pub const WS_OVERLAPPEDWINDOW: u32 = 0x00CF_0000;
    pub const BS_PUSHBUTTON: u32 = 0x0000_0000;
    pub const BS_AUTOCHECKBOX: u32 = 0x0000_0003;
    pub const BS_AUTORADIOBUTTON: u32 = 0x0000_0009;
    pub const SS_LEFT: u32 = 0x0000_0000;
    pub const ES_AUTOHSCROLL: u32 = 0x0080;
    pub const SWP_NOZORDER: u32 = 0x0004;
    pub const SWP_NOMOVE: u32 = 0x0002;
    pub const SW_SHOW: i32 = 5;
    pub const GWLP_USERDATA: i32 = -21;
    pub const ICON_BIG: usize = 1;
    pub const ICON_SMALL: usize = 0;
    pub const IDI_APPLICATION: *const u16 = 32512usize as *const u16;
    pub const CW_USEDEFAULT: i32 = -2147483648;
    pub const WM_DESTROY: u32 = 0x0002;
    pub const WM_TIMER: u32 = 0x0113;
    pub const WM_HSCROLL: u32 = 0x0114;
    pub const WM_SIZE: u32 = 0x0005;
    pub const WM_SETICON: u32 = 0x0080;
    pub const WM_COMMAND: u32 = 0x0111;
    pub const WM_LBUTTONDOWN: u32 = 0x0201;
    pub const WM_CTLCOLOREDIT: u32 = 0x0133;
    pub const WM_CTLCOLORBTN: u32 = 0x0135;
    pub const WM_CTLCOLORSTATIC: u32 = 0x0138;
    pub const TRANSPARENT: i32 = 1;
    pub const OPAQUE: i32 = 2;
    pub const NULL_BRUSH: i32 = 5;
    /// The standard dialog/control-face color — what `SystemColors.Control`
    /// means in WinForms terms. Used as the window class's background,
    /// so a transparent control (the default) shows this through rather
    /// than an undefined backdrop.
    pub const COLOR_BTNFACE: usize = 15;
    pub const BM_GETCHECK: u32 = 0x00F0;
    pub const BM_SETCHECK: u32 = 0x00F1;
    pub const BST_CHECKED: isize = 1;
    pub const BST_UNCHECKED: isize = 0;

    pub const PBM_SETPOS: u32 = 0x0402;
    pub const PBM_GETPOS: u32 = 0x0408;

    pub const SS_BITMAP: u32 = 0x0000_000E;
    pub const STM_SETIMAGE: u32 = 0x0172;
    pub const IMAGE_BITMAP: usize = 0;
    pub const LR_LOADFROMFILE: u32 = 0x0010;

    pub const TVS_HASBUTTONS: u32 = 0x0001;
    pub const TVS_HASLINES: u32 = 0x0002;
    pub const TVS_LINESATROOT: u32 = 0x0004;
    pub const TVM_INSERTITEMW: u32 = 0x1100 + 50;
    pub const TVM_DELETEITEM: u32 = 0x1100 + 1;
    pub const TVIF_TEXT: u32 = 0x0001;
    /// Special `HTREEITEM` values — see `TreeView_InsertItem`'s docs.
    pub const TVI_ROOT: isize = -65536;
    pub const TVI_LAST: isize = -65534;

    pub const ICC_TREEVIEW_CLASSES: u32 = 0x0000_0002;
    pub const ICC_PROGRESS_CLASS: u32 = 0x0000_0020;
    pub const ICC_BAR_CLASSES: u32 = 0x0000_0004;

    pub const WS_VSCROLL: u32 = 0x0020_0000;

    // Trackbar (`msctls_trackbar32`): messages are WM_USER (0x0400) + n.
    pub const TBS_AUTOTICKS: u32 = 0x0001;
    pub const TBM_GETPOS: u32 = 0x0400;
    pub const TBM_GETRANGEMIN: u32 = 0x0401;
    pub const TBM_GETRANGEMAX: u32 = 0x0402;
    pub const TBM_SETPOS: u32 = 0x0405;
    pub const TBM_SETRANGEMIN: u32 = 0x0407;
    pub const TBM_SETRANGEMAX: u32 = 0x0408;
    pub const TBM_SETTICFREQ: u32 = 0x0414;

    // Drop-down list (`COMBOBOX` with `CBS_DROPDOWNLIST`) and list box.
    pub const CBS_DROPDOWNLIST: u32 = 0x0003;
    pub const CB_GETCURSEL: u32 = 0x0147;
    pub const CB_ADDSTRING: u32 = 0x0143;
    pub const CB_RESETCONTENT: u32 = 0x014B;
    pub const CB_SETCURSEL: u32 = 0x014E;
    pub const LBS_NOTIFY: u32 = 0x0001;
    pub const LB_ADDSTRING: u32 = 0x0180;
    pub const LB_RESETCONTENT: u32 = 0x0184;
    pub const LB_SETCURSEL: u32 = 0x0186;
    pub const LB_GETCURSEL: u32 = 0x0188;
    /// The notification code in `WM_COMMAND`'s high word when the selection
    /// of a combo box (`CBN_SELCHANGE`) or list box (`LBN_SELCHANGE`) changed.
    pub const SELCHANGE: usize = 1;

    #[repr(C)]
    pub struct InitCommonControlsExStruct {
        pub dw_size: u32,
        pub dw_icc: u32,
    }

    /// Matches `TVINSERTSTRUCTW`: `hParent`/`hInsertAfter` followed
    /// inline by a full `TVITEMEXW` (the union member `TVM_INSERTITEMW`
    /// actually reads) — not just the older, shorter `TVITEMW`, so
    /// comctl32 never reads past what we actually initialized.
    #[repr(C)]
    pub struct TvInsertStructW {
        pub h_parent: isize,
        pub h_insert_after: isize,
        pub mask: u32,
        pub h_item: isize,
        pub state: u32,
        pub state_mask: u32,
        pub psz_text: *mut u16,
        pub cch_text_max: i32,
        pub i_image: i32,
        pub i_selected_image: i32,
        pub c_children: i32,
        pub l_param: isize,
        pub i_integral: i32,
        pub u_state_ex: u32,
        pub hwnd: HWND,
        pub i_expanded_image: i32,
        pub i_reserved: i32,
    }

    #[repr(C)]
    pub struct Rect {
        pub left: i32,
        pub top: i32,
        pub right: i32,
        pub bottom: i32,
    }

    #[repr(C)]
    pub struct WndClassW {
        pub style: u32,
        pub lpfn_wnd_proc: unsafe extern "system" fn(HWND, u32, WPARAM, LPARAM) -> LRESULT,
        pub cb_cls_extra: i32,
        pub cb_wnd_extra: i32,
        pub h_instance: HWND,
        pub h_icon: HWND,
        pub h_cursor: HWND,
        pub hbr_background: HWND,
        pub lpsz_menu_name: *const u16,
        pub lpsz_class_name: *const u16,
    }

    /// Mirrors `ACTCTXW` (kernel32.dll) — only the fields a plain
    /// standalone manifest *file* needs; `lpResourceName`/`lpApplicationName`/
    /// `hModule` are only meaningful when loading a manifest embedded as
    /// a resource instead, which isn't an option here (no resource
    /// compiler in this no-Cargo pipeline).
    #[repr(C)]
    pub struct ActCtxW {
        pub cb_size: u32,
        pub dw_flags: u32,
        pub lp_source: *const u16,
        pub w_processor_architecture: u16,
        pub w_lang_id: u16,
        pub lp_assembly_directory: *const u16,
        pub lp_resource_name: *const u16,
        pub lp_application_name: *const u16,
        pub h_module: HWND,
    }

    #[link(name = "kernel32")]
    unsafe extern "system" {
        pub fn GetModuleHandleW(module_name: *const u16) -> HWND;
        pub fn CreateActCtxW(act_ctx: *const ActCtxW) -> HWND;
        pub fn ActivateActCtx(act_ctx: HWND, cookie: *mut usize) -> i32;
    }

    #[link(name = "user32")]
    unsafe extern "system" {
        pub fn RegisterClassW(wnd_class: *const WndClassW) -> u16;
        pub fn CreateWindowExW(
            ex_style: u32,
            class_name: *const u16,
            window_name: *const u16,
            style: u32,
            x: i32,
            y: i32,
            width: i32,
            height: i32,
            parent: HWND,
            menu: HWND,
            instance: HWND,
            param: *const std::ffi::c_void,
        ) -> HWND;
        pub fn DefWindowProcW(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT;
        pub fn ShowWindow(hwnd: HWND, cmd_show: i32) -> i32;
        pub fn SetWindowTextW(hwnd: HWND, text: *const u16) -> i32;
        pub fn GetWindowTextW(hwnd: HWND, text: *mut u16, max_count: i32) -> i32;
        pub fn GetWindowTextLengthW(hwnd: HWND) -> i32;
        pub fn SendMessageW(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT;
        pub fn SetWindowPos(
            hwnd: HWND,
            insert_after: HWND,
            x: i32,
            y: i32,
            cx: i32,
            cy: i32,
            flags: u32,
        ) -> i32;
        // 64-bit user32 exports the *LongPtr* functions. 32-bit user32 does
        // not: there `GetWindowLongPtrW` is only a header macro for
        // `GetWindowLongW`, and `LONG` is pointer-sized, so binding the
        // same Rust name to that export is exact.
        #[cfg_attr(target_pointer_width = "32", link_name = "SetWindowLongW")]
        pub fn SetWindowLongPtrW(hwnd: HWND, index: i32, new_long: isize) -> isize;
        #[cfg_attr(target_pointer_width = "32", link_name = "GetWindowLongW")]
        pub fn GetWindowLongPtrW(hwnd: HWND, index: i32) -> isize;
        pub fn GetClientRect(hwnd: HWND, rect: *mut Rect) -> i32;
        pub fn AdjustWindowRect(rect: *mut Rect, style: u32, menu: i32) -> i32;
        pub fn LoadIconW(instance: HWND, icon_name: *const u16) -> HWND;
        pub fn PostQuitMessage(exit_code: i32);
        /// `timer_proc` is always null here: ticks arrive as `WM_TIMER` on the
        /// window's own procedure, which keeps them on the UI thread.
        pub fn SetTimer(hwnd: HWND, id_event: usize, elapse_ms: u32, timer_proc: *const std::ffi::c_void) -> usize;
        pub fn KillTimer(hwnd: HWND, id_event: usize) -> i32;
        pub fn GetStockObject(index: i32) -> HWND;
        /// A shared system-colour brush (never freed); `COLOR_BTNFACE` is the form's own background.
        pub fn GetSysColorBrush(index: i32) -> HWND;
        pub fn InvalidateRect(hwnd: HWND, rect: *const Rect, erase: i32) -> i32;
        pub fn SetProcessDPIAware() -> i32;
        pub fn GetDpiForSystem() -> u32;
        pub fn LoadImageW(
            inst: HWND,
            name: *const u16,
            img_type: u32,
            cx: i32,
            cy: i32,
            fu_load: u32,
        ) -> HWND;
    }

    #[link(name = "gdi32")]
    unsafe extern "system" {
        pub fn CreateSolidBrush(color: u32) -> HWND;
        pub fn SetTextColor(hdc: HWND, color: u32) -> u32;
        pub fn SetBkColor(hdc: HWND, color: u32) -> u32;
        pub fn SetBkMode(hdc: HWND, mode: i32) -> i32;
    }

    // `msctls_progress32`/`SysTreeView32` are comctl32's own window
    // classes — registering them needs this explicit call. The
    // visual-styles manifest (`activate_visual_styles_once`) only
    // picks a comctl32 *version* for classes already in use (`BUTTON`,
    // `STATIC`, `EDIT`, ...); it doesn't register comctl32's own
    // custom ones.
    #[link(name = "comctl32")]
    unsafe extern "system" {
        pub fn InitCommonControlsEx(picce: *const InitCommonControlsExStruct) -> i32;
    }
}


pub use ffi::HWND;

/// What a component needs from its host to create a control: the parent window.
#[derive(Clone, Copy)]
pub struct Parent {
    pub hwnd: HWND,
}

pub fn to_wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

pub fn set_window_text(hwnd: HWND, text: &str) {
    let wide = to_wide(text);
    unsafe { ffi::SetWindowTextW(hwnd, wide.as_ptr()) };
}

pub fn window_text(hwnd: HWND) -> String {
    unsafe {
        let len = ffi::GetWindowTextLengthW(hwnd);
        if len <= 0 {
            return String::new();
        }
        let mut buf = vec![0u16; len as usize + 1];
        let copied = ffi::GetWindowTextW(hwnd, buf.as_mut_ptr(), buf.len() as i32);
        String::from_utf16_lossy(&buf[..copied.max(0) as usize])
    }
}

pub fn create_child(parent: HWND, class_name: &str, style: u32, text: &str) -> HWND {
    let class_name = to_wide(class_name);
    let window_text = to_wide(text);
    unsafe {
        let hwnd = ffi::CreateWindowExW(
            0,
            class_name.as_ptr(),
            window_text.as_ptr(),
            ffi::WS_CHILD | ffi::WS_VISIBLE | style,
            0,
            0,
            80,
            24,
            parent,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            std::ptr::null(),
        );
        ffi::ShowWindow(hwnd, ffi::SW_SHOW);
        hwnd
    }
}

thread_local! {
    static CLICK_HANDLERS: RefCell<HashMap<usize, Box<dyn FnMut()>>> = RefCell::new(HashMap::new());
}

pub fn hwnd_key(hwnd: HWND) -> usize {
    hwnd as usize
}

pub fn set_click_handler(hwnd: HWND, callback: Box<dyn FnMut()>) {
    CLICK_HANDLERS.with(|handlers| {
        handlers.borrow_mut().insert(hwnd_key(hwnd), callback);
    });
}

pub fn fire_click(hwnd: HWND) {
    CLICK_HANDLERS.with(|handlers| {
        if let Some(callback) = handlers.borrow_mut().get_mut(&hwnd_key(hwnd)) {
            callback();
        }
    });
}

thread_local! {
    static CHANGE_HANDLERS: RefCell<HashMap<usize, Box<dyn FnMut()>>> = RefCell::new(HashMap::new());
    /// Combo and list boxes: their `WM_COMMAND`s carry a notification code in
    /// the high word, and only the selection-changed one (1) is a user action
    /// (the others are focus and drop-down bookkeeping).
    static SELECTION_CONTROLS: RefCell<HashSet<usize>> = RefCell::new(HashSet::new());
}

pub fn set_change_handler(hwnd: HWND, callback: Box<dyn FnMut()>) {
    CHANGE_HANDLERS.with(|handlers| {
        handlers.borrow_mut().insert(hwnd_key(hwnd), callback);
    });
}

/// Runs a change handler with it taken out of the table while it runs, so
/// the handler may itself install a new one without a re-entrancy panic.
pub fn fire_change(hwnd: HWND) {
    let key = hwnd_key(hwnd);
    let taken = CHANGE_HANDLERS.with(|handlers| handlers.borrow_mut().remove(&key));
    if let Some(mut callback) = taken {
        callback();
        CHANGE_HANDLERS.with(|handlers| {
            handlers.borrow_mut().entry(key).or_insert(callback);
        });
    }
}

/// Everything a tick needs, shared between a `Win32TimerHandle` and the
/// per-thread table `wndproc` looks it up in (a `WM_TIMER` only carries the

#[derive(Clone, Copy)]
struct ColorState {
    fore: Option<Color>,
    back: Option<Color>,
}

thread_local! {
    // Keyed by hwnd, same reasoning `CLICK_HANDLERS` already documents:
    // `WM_CTLCOLOR*` only ever hands `wndproc` the child's `HWND`, not
    // any Rust-side state attached to it, so this is the only way to
    // get a color back out when painting.
    static COLOR_STATE: RefCell<HashMap<usize, ColorState>> = RefCell::new(HashMap::new());
    // Controls (trackbars) that ask their parent for a background brush with `WM_CTLCOLORSTATIC`
    // and paint nothing themselves, so answering with the "transparent"
    // `NULL_BRUSH` every other control gets would leave their old pixels on
    // screen: a black backdrop and a ghost of every earlier thumb position.
    static FORM_COLOR_CONTROLS: RefCell<HashSet<usize>> = RefCell::new(HashSet::new());
    // One cached brush per distinct back color ever used — recreating
    // on every paint would leak just as much (brushes are never freed
    // here either way) while also being slower.
    static COLOR_BRUSHES: RefCell<HashMap<(u8, u8, u8), HWND>> = RefCell::new(HashMap::new());
}

pub fn color_ref(c: Color) -> u32 {
    // COLORREF is 0x00BBGGRR, not RGB order.
    c.r as u32 | (c.g as u32) << 8 | (c.b as u32) << 16
}

pub fn set_fore_color(hwnd: HWND, color: Option<Color>) {
    COLOR_STATE.with(|state| {
        state.borrow_mut().entry(hwnd_key(hwnd)).or_insert(ColorState { fore: None, back: None }).fore = color;
    });
}

pub fn fore_color(hwnd: HWND) -> Option<Color> {
    COLOR_STATE.with(|state| state.borrow().get(&hwnd_key(hwnd)).and_then(|s| s.fore))
}

pub fn set_back_color(hwnd: HWND, color: Option<Color>) {
    COLOR_STATE.with(|state| {
        state.borrow_mut().entry(hwnd_key(hwnd)).or_insert(ColorState { fore: None, back: None }).back = color;
    });
}

pub fn back_color(hwnd: HWND) -> Option<Color> {
    COLOR_STATE.with(|state| state.borrow().get(&hwnd_key(hwnd)).and_then(|s| s.back))
}

pub fn brush_for(color: Color) -> HWND {
    COLOR_BRUSHES.with(|brushes| {
        *brushes
            .borrow_mut()
            .entry((color.r, color.g, color.b))
            .or_insert_with(|| unsafe { ffi::CreateSolidBrush(color_ref(color)) })
    })
}

/// Marks `hwnd` as a control that paints nothing of its own behind itself and
/// asks the parent for a background brush: it must be given the form's own
/// colour, not the "transparent" brush ordinary controls get.
pub fn erase_to_form_color(hwnd: HWND) {
    FORM_COLOR_CONTROLS.with(|t| t.borrow_mut().insert(hwnd_key(hwnd)));
}

/// Marks `hwnd` as a combo/list box: only its selection-changed notification
/// is a user action.
pub fn mark_selection_control(hwnd: HWND) {
    SELECTION_CONTROLS.with(|set| set.borrow_mut().insert(hwnd_key(hwnd)));
}

/// Shared by `WM_CTLCOLORSTATIC`/`WM_CTLCOLORBTN`/`WM_CTLCOLOREDIT`.
/// `default_to_transparent` is what tells labels/buttons and edit boxes
/// apart: a `STATIC`/`BUTTON` with no custom back color should show
/// whatever's behind it (its parent's own background) rather than the
/// opaque box Windows paints by default, but an `EDIT` with no custom
/// color should keep its ordinary white box — that's the one case where
/// "default" already looks right.
pub fn handle_ctlcolor(hdc: HWND, child: HWND, default_to_transparent: bool) -> ffi::LRESULT {
    let state = COLOR_STATE.with(|s| s.borrow().get(&hwnd_key(child)).copied());
    unsafe {
        if let Some(fore) = state.and_then(|s| s.fore) {
            ffi::SetTextColor(hdc, color_ref(fore));
        }
        match state.and_then(|s| s.back) {
            Some(color) => {
                ffi::SetBkMode(hdc, ffi::OPAQUE);
                ffi::SetBkColor(hdc, color_ref(color));
                brush_for(color) as ffi::LRESULT
            }
            None if default_to_transparent && FORM_COLOR_CONTROLS.with(|t| t.borrow().contains(&hwnd_key(child))) => {
                // Erase to the form's own colour, as the window class does.
                ffi::GetSysColorBrush(ffi::COLOR_BTNFACE as i32) as ffi::LRESULT
            }
            None if default_to_transparent => {
                ffi::SetBkMode(hdc, ffi::TRANSPARENT);
                ffi::GetStockObject(ffi::NULL_BRUSH) as ffi::LRESULT
            }
            None => 0, // let DefWindowProcW's own default handling apply
        }
    }
}

pub struct Win32Widget {
    pub hwnd: HWND,
    pub x: Cell<i32>,
    pub y: Cell<i32>,
    pub width: Cell<u32>,
    pub height: Cell<u32>,
    /// Added to `height` when sizing the window. A combo box's window height
    /// includes the drop-down list, not just the closed box, so it needs room
    /// beyond the height the form lays out.
    pub extra_height: Cell<u32>,
}

impl Win32Widget {
    pub fn new(hwnd: HWND) -> Self {
        Self {
            hwnd,
            x: Cell::new(0),
            y: Cell::new(0),
            width: Cell::new(80),
            height: Cell::new(24),
            extra_height: Cell::new(0),
        }
    }

    pub fn apply_rect(&self) {
        // A child's `x`/`y`/`width`/`height` are logical units too —
        // see `dpi_scale`'s doc comment — so they need the same
        // logical-to-physical conversion the top-level window gets, or
        // a DPI-scaled display would place/size every control as if
        // the screen were still 96 DPI, visibly shrinking the whole
        // form's layout relative to its own (now correctly sized) frame.
        let scale = dpi_scale();
        unsafe {
            ffi::SetWindowPos(
                self.hwnd,
                std::ptr::null_mut(),
                (self.x.get() as f64 * scale).round() as i32,
                (self.y.get() as f64 * scale).round() as i32,
                (self.width.get() as f64 * scale).round() as i32,
                ((self.height.get() + self.extra_height.get()) as f64 * scale).round() as i32,
                ffi::SWP_NOZORDER,
            );
        }
    }

    /// Forces a redraw — needed after a color change, since Windows has
    /// no reason to repaint a control on its own just because its
    /// `WM_CTLCOLOR*` answer would now be different.
    pub fn repaint(&self) {
        unsafe { ffi::InvalidateRect(self.hwnd, std::ptr::null(), 1) };
    }
}

macro_rules! impl_widget_handle {
    ($ty:ty, $field:ident) => {
        impl crate::WidgetHandle for $ty {
            fn set_pos(&self, x: i32, y: i32) {
                self.$field.x.set(x);
                self.$field.y.set(y);
                self.$field.apply_rect();
            }
            fn pos(&self) -> (i32, i32) {
                (self.$field.x.get(), self.$field.y.get())
            }
            fn set_width(&self, width: u32) {
                self.$field.width.set(width);
                self.$field.apply_rect();
            }
            fn width(&self) -> u32 {
                self.$field.width.get()
            }
            fn set_height(&self, height: u32) {
                self.$field.height.set(height);
                self.$field.apply_rect();
            }
            fn height(&self) -> u32 {
                self.$field.height.get()
            }
            fn set_on_click(&self, callback: Box<dyn FnMut()>) {
                crate::platform::win32::set_click_handler(self.$field.hwnd, callback);
            }
            fn set_fore_color(&self, color: Option<crate::Color>) {
                crate::platform::win32::set_fore_color(self.$field.hwnd, color);
                self.$field.repaint();
            }
            fn fore_color(&self) -> Option<crate::Color> {
                crate::platform::win32::fore_color(self.$field.hwnd)
            }
            fn set_back_color(&self, color: Option<crate::Color>) {
                crate::platform::win32::set_back_color(self.$field.hwnd, color);
                self.$field.repaint();
            }
            fn back_color(&self) -> Option<crate::Color> {
                crate::platform::win32::back_color(self.$field.hwnd)
            }
        }
    };
}

pub(crate) use impl_widget_handle;

static COMMON_CONTROLS: Once = Once::new();

/// Registers comctl32's own window classes (`msctls_progress32`,
/// `SysTreeView32`) — see the `InitCommonControlsEx` import's doc
/// comment for why the visual-styles manifest alone doesn't do this.
pub fn init_common_controls_once() {
    COMMON_CONTROLS.call_once(|| unsafe {
        let icc = ffi::InitCommonControlsExStruct {
            dw_size: std::mem::size_of::<ffi::InitCommonControlsExStruct>() as u32,
            dw_icc: ffi::ICC_PROGRESS_CLASS | ffi::ICC_TREEVIEW_CLASSES | ffi::ICC_BAR_CLASSES,
        };
        ffi::InitCommonControlsEx(&icc);
    });
}

/// `width`/`height` everywhere in this crate are logical (96-DPI, CSS-px
/// equivalent) sizes, to match the designer preview — a browser page,
/// which the OS already scales to the system DPI on its own. Now that
/// this process is DPI-aware (see above), Win32 no longer does that
/// scaling for us, so every logical size has to be converted to
/// physical pixels by hand before it reaches a real Win32 API.
pub fn dpi_scale() -> f64 {
    unsafe { ffi::GetDpiForSystem() as f64 / 96.0 }
}

/// A component's hook into the window procedure, for messages only it
/// understands (a timer's `WM_TIMER`). Return `Some` to consume the message.
pub type MessageHook = fn(HWND, u32, usize, isize) -> Option<isize>;

thread_local! {
    static HOOKS: RefCell<Vec<MessageHook>> = RefCell::new(Vec::new());
}

/// Adds `hook` (once; registering the same function again is a no-op).
pub fn register_message_hook(hook: MessageHook) {
    HOOKS.with(|hooks| {
        let mut hooks = hooks.borrow_mut();
        if !hooks.iter().any(|h| *h as usize == hook as usize) {
            hooks.push(hook);
        }
    });
}

fn run_hooks(hwnd: HWND, msg: u32, wparam: usize, lparam: isize) -> Option<isize> {
    let hooks: Vec<MessageHook> = HOOKS.with(|hooks| hooks.borrow().clone());
    hooks.into_iter().find_map(|hook| hook(hwnd, msg, wparam, lparam))
}

/// Called by the backend when a top-level window is destroyed, so components
/// can drop what they keep per window.
pub fn window_destroyed(hwnd: HWND) {
    run_hooks(hwnd, ffi::WM_DESTROY, 0, 0);
}

/// Everything the window procedure does for controls: clicks, change
/// notifications, colours, and whatever components hooked. `None` means "not
/// handled here"; the backend then falls back to `DefWindowProcW`.
pub fn dispatch(hwnd: HWND, msg: u32, wparam: usize, lparam: isize) -> Option<isize> {
    match msg {
        ffi::WM_COMMAND => {
            let child = lparam as HWND;
            if !child.is_null() {
                let is_selection_control = SELECTION_CONTROLS.with(|set| set.borrow().contains(&hwnd_key(child)));
                if is_selection_control {
                    // Only "selection changed" is a user action; the other
                    // notification codes are focus and drop-down chatter.
                    if (wparam >> 16) & 0xFFFF == ffi::SELCHANGE {
                        fire_change(child);
                        fire_click(child);
                    }
                } else {
                    fire_click(child);
                }
            }
            None
        }
        ffi::WM_HSCROLL => {
            // A trackbar reports every move (drag, keys, page clicks) here.
            let child = lparam as HWND;
            if !child.is_null() {
                fire_change(child);
            }
            Some(0)
        }
        ffi::WM_LBUTTONDOWN => {
            fire_click(hwnd);
            None
        }
        ffi::WM_CTLCOLORSTATIC | ffi::WM_CTLCOLORBTN => {
            let result = handle_ctlcolor(wparam as HWND, lparam as HWND, true);
            if result != 0 { Some(result) } else { None }
        }
        ffi::WM_CTLCOLOREDIT => {
            let result = handle_ctlcolor(wparam as HWND, lparam as HWND, false);
            if result != 0 { Some(result) } else { None }
        }
        _ => run_hooks(hwnd, msg, wparam, lparam),
    }
}

/// `CB_SETCURSEL`/`LB_SETCURSEL` take -1 (as a `WPARAM`) for "no selection".
pub fn selection_wparam(index: Option<usize>, len: usize) -> usize {
    match index {
        Some(i) if i < len => i,
        _ => usize::MAX,
    }
}

pub fn selection_from(result: isize) -> Option<usize> {
    if result < 0 { None } else { Some(result as usize) }
}
