//! Native Win32 backend with zero external crate dependencies. It
//! hand-declares the handful of WinAPI functions and constants it needs
//! instead of depending on `windows-sys`, an enormous generated-bindings
//! crate. That keeps it buildable two ways from the same source: as a
//! normal Cargo dependency, and as plain source mounted next to
//! `rad-core`'s files in the designer's no-Cargo build pipeline, which
//! has no way to fetch crates or produce `.rlib`s.

use rad_core::{
    Backend, ButtonHandle, CheckBoxHandle, ComboBoxHandle, Color, Host, LabelHandle, ListBoxHandle,
    PictureBoxHandle, ProgressBarHandle, RadioButtonHandle, TextBoxHandle, TimerHandle,
    TrackBarHandle, TreeItem, TreeViewHandle, WidgetHandle, WindowHandle,
};
use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet};
use std::rc::Rc;
use std::sync::Once;

mod ffi {
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

use ffi::HWND;

fn to_wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

fn set_window_text(hwnd: HWND, text: &str) {
    let wide = to_wide(text);
    unsafe { ffi::SetWindowTextW(hwnd, wide.as_ptr()) };
}

fn window_text(hwnd: HWND) -> String {
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

fn create_child(parent: HWND, class_name: &str, style: u32, text: &str) -> HWND {
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

fn hwnd_key(hwnd: HWND) -> usize {
    hwnd as usize
}

fn set_click_handler(hwnd: HWND, callback: Box<dyn FnMut()>) {
    CLICK_HANDLERS.with(|handlers| {
        handlers.borrow_mut().insert(hwnd_key(hwnd), callback);
    });
}

fn fire_click(hwnd: HWND) {
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

fn set_change_handler(hwnd: HWND, callback: Box<dyn FnMut()>) {
    CHANGE_HANDLERS.with(|handlers| {
        handlers.borrow_mut().insert(hwnd_key(hwnd), callback);
    });
}

/// Runs a change handler with it taken out of the table while it runs, so
/// the handler may itself install a new one without a re-entrancy panic.
fn fire_change(hwnd: HWND) {
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
/// window and the timer id, not any Rust state).
struct TimerShared {
    callback: RefCell<Option<Box<dyn FnMut()>>>,
    /// Cleared when the timer is dropped or its window is destroyed, so a
    /// tick that is mid-flight at that moment does not put its callback back.
    alive: Cell<bool>,
}

thread_local! {
    static TIMERS: RefCell<HashMap<(usize, usize), Rc<TimerShared>>> = RefCell::new(HashMap::new());
    static NEXT_TIMER_ID: Cell<usize> = const { Cell::new(1) };
}

/// Runs one tick. The callback is taken out of its slot while it runs, so it
/// may freely stop the timer, change its interval, or install a replacement
/// callback (all of which touch the same slot) without a re-entrancy panic.
fn fire_timer(hwnd: HWND, id: usize) {
    let shared = TIMERS.with(|timers| timers.borrow().get(&(hwnd_key(hwnd), id)).cloned());
    let Some(shared) = shared else { return };
    let taken = shared.callback.borrow_mut().take();
    if let Some(mut callback) = taken {
        callback();
        // Put the callback back unless the handler replaced it or the timer
        // was dropped meanwhile.
        if shared.alive.get() && shared.callback.borrow().is_none() {
            *shared.callback.borrow_mut() = Some(callback);
        }
    }
}

/// Window teardown: Windows itself discards a destroyed window's timers, so
/// only our bookkeeping needs clearing.
fn forget_timers_of(hwnd: HWND) {
    TIMERS.with(|timers| {
        timers.borrow_mut().retain(|(owner, _), shared| {
            let keep = *owner != hwnd_key(hwnd);
            if !keep {
                shared.alive.set(false);
            }
            keep
        });
    });
}

pub(crate) struct Win32TimerHandle {
    hwnd: HWND,
    id: usize,
    shared: Rc<TimerShared>,
    interval_ms: Cell<u32>,
    enabled: Cell<bool>,
}

impl Win32TimerHandle {
    fn arm(&self) {
        // Windows clamps below ~10 ms and above i32::MAX on its own.
        unsafe { ffi::SetTimer(self.hwnd, self.id, self.interval_ms.get().max(1), std::ptr::null()) };
    }
}

impl TimerHandle for Win32TimerHandle {
    fn set_interval(&self, interval_ms: u32) {
        self.interval_ms.set(interval_ms);
        if self.enabled.get() {
            self.arm(); // SetTimer on an existing id just changes its period
        }
    }
    fn interval(&self) -> u32 {
        self.interval_ms.get()
    }
    fn set_enabled(&self, enabled: bool) {
        if enabled == self.enabled.get() {
            return;
        }
        self.enabled.set(enabled);
        if enabled {
            self.arm();
        } else {
            unsafe { ffi::KillTimer(self.hwnd, self.id) };
        }
    }
    fn enabled(&self) -> bool {
        self.enabled.get()
    }
    fn set_on_tick(&self, callback: Box<dyn FnMut()>) {
        *self.shared.callback.borrow_mut() = Some(callback);
    }
}

impl Drop for Win32TimerHandle {
    fn drop(&mut self) {
        self.shared.alive.set(false);
        unsafe { ffi::KillTimer(self.hwnd, self.id) };
        TIMERS.with(|timers| timers.borrow_mut().remove(&(hwnd_key(self.hwnd), self.id)));
    }
}

fn create_timer(parent: HWND) -> Box<dyn TimerHandle> {
    let id = NEXT_TIMER_ID.with(|next| {
        let id = next.get();
        next.set(id + 1);
        id
    });
    let shared = Rc::new(TimerShared { callback: RefCell::new(None), alive: Cell::new(true) });
    TIMERS.with(|timers| timers.borrow_mut().insert((hwnd_key(parent), id), shared.clone()));
    // WinForms defaults: disabled, 100 ms.
    Box::new(Win32TimerHandle { hwnd: parent, id, shared, interval_ms: Cell::new(100), enabled: Cell::new(false) })
}

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
    // Trackbars ask their parent for a background brush with `WM_CTLCOLORSTATIC`
    // and paint nothing themselves, so answering with the "transparent"
    // `NULL_BRUSH` every other control gets would leave their old pixels on
    // screen: a black backdrop and a ghost of every earlier thumb position.
    static TRACKBARS: RefCell<HashSet<usize>> = RefCell::new(HashSet::new());
    // One cached brush per distinct back color ever used — recreating
    // on every paint would leak just as much (brushes are never freed
    // here either way) while also being slower.
    static COLOR_BRUSHES: RefCell<HashMap<(u8, u8, u8), HWND>> = RefCell::new(HashMap::new());
}

fn color_ref(c: Color) -> u32 {
    // COLORREF is 0x00BBGGRR, not RGB order.
    c.r as u32 | (c.g as u32) << 8 | (c.b as u32) << 16
}

fn set_fore_color(hwnd: HWND, color: Option<Color>) {
    COLOR_STATE.with(|state| {
        state.borrow_mut().entry(hwnd_key(hwnd)).or_insert(ColorState { fore: None, back: None }).fore = color;
    });
}

fn fore_color(hwnd: HWND) -> Option<Color> {
    COLOR_STATE.with(|state| state.borrow().get(&hwnd_key(hwnd)).and_then(|s| s.fore))
}

fn set_back_color(hwnd: HWND, color: Option<Color>) {
    COLOR_STATE.with(|state| {
        state.borrow_mut().entry(hwnd_key(hwnd)).or_insert(ColorState { fore: None, back: None }).back = color;
    });
}

fn back_color(hwnd: HWND) -> Option<Color> {
    COLOR_STATE.with(|state| state.borrow().get(&hwnd_key(hwnd)).and_then(|s| s.back))
}

fn brush_for(color: Color) -> HWND {
    COLOR_BRUSHES.with(|brushes| {
        *brushes
            .borrow_mut()
            .entry((color.r, color.g, color.b))
            .or_insert_with(|| unsafe { ffi::CreateSolidBrush(color_ref(color)) })
    })
}

/// Shared by `WM_CTLCOLORSTATIC`/`WM_CTLCOLORBTN`/`WM_CTLCOLOREDIT`.
/// `default_to_transparent` is what tells labels/buttons and edit boxes
/// apart: a `STATIC`/`BUTTON` with no custom back color should show
/// whatever's behind it (its parent's own background) rather than the
/// opaque box Windows paints by default, but an `EDIT` with no custom
/// color should keep its ordinary white box — that's the one case where
/// "default" already looks right.
fn handle_ctlcolor(hdc: HWND, child: HWND, default_to_transparent: bool) -> ffi::LRESULT {
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
            None if default_to_transparent && TRACKBARS.with(|t| t.borrow().contains(&hwnd_key(child))) => {
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

pub(crate) struct Win32Widget {
    hwnd: HWND,
    x: Cell<i32>,
    y: Cell<i32>,
    width: Cell<u32>,
    height: Cell<u32>,
    /// Added to `height` when sizing the window. A combo box's window height
    /// includes the drop-down list, not just the closed box, so it needs room
    /// beyond the height the form lays out.
    extra_height: Cell<u32>,
}

impl Win32Widget {
    fn new(hwnd: HWND) -> Self {
        Self {
            hwnd,
            x: Cell::new(0),
            y: Cell::new(0),
            width: Cell::new(80),
            height: Cell::new(24),
            extra_height: Cell::new(0),
        }
    }

    fn apply_rect(&self) {
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
    fn repaint(&self) {
        unsafe { ffi::InvalidateRect(self.hwnd, std::ptr::null(), 1) };
    }
}

macro_rules! impl_widget_handle {
    ($ty:ty, $field:ident) => {
        impl WidgetHandle for $ty {
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
                set_click_handler(self.$field.hwnd, callback);
            }
            fn set_fore_color(&self, color: Option<Color>) {
                set_fore_color(self.$field.hwnd, color);
                self.$field.repaint();
            }
            fn fore_color(&self) -> Option<Color> {
                fore_color(self.$field.hwnd)
            }
            fn set_back_color(&self, color: Option<Color>) {
                set_back_color(self.$field.hwnd, color);
                self.$field.repaint();
            }
            fn back_color(&self) -> Option<Color> {
                back_color(self.$field.hwnd)
            }
        }
    };
}

pub struct Win32Host {
    hwnd: HWND,
}

impl Win32Host {
    /// # Safety
    /// `hwnd` must be a valid window handle that outlives this `Win32Host`.
    pub unsafe fn from_hwnd(hwnd: HWND) -> Self {
        Self { hwnd }
    }
}

impl Host for Win32Host {
    fn create_button(&self, text: &str) -> Box<dyn ButtonHandle> {
        create_button(self.hwnd, text)
    }
    fn create_label(&self, text: &str) -> Box<dyn LabelHandle> {
        create_label(self.hwnd, text)
    }
    fn create_checkbox(&self, text: &str) -> Box<dyn CheckBoxHandle> {
        create_checkbox(self.hwnd, text)
    }
    fn create_textbox(&self, text: &str) -> Box<dyn TextBoxHandle> {
        create_textbox(self.hwnd, text)
    }
    fn create_radio_button(&self, _group: &str, text: &str) -> Box<dyn RadioButtonHandle> {
        create_radio_button(self.hwnd, text)
    }
    fn create_progress_bar(&self) -> Box<dyn ProgressBarHandle> {
        create_progress_bar(self.hwnd)
    }
    fn create_picture_box(&self) -> Box<dyn PictureBoxHandle> {
        create_picture_box(self.hwnd)
    }
    fn create_tree_view(&self) -> Box<dyn TreeViewHandle> {
        create_tree_view(self.hwnd)
    }
    fn create_timer(&self) -> Box<dyn TimerHandle> {
        create_timer(self.hwnd)
    }
    fn create_track_bar(&self) -> Box<dyn TrackBarHandle> {
        create_track_bar(self.hwnd)
    }
    fn create_combo_box(&self) -> Box<dyn ComboBoxHandle> {
        create_combo_box(self.hwnd)
    }
    fn create_list_box(&self) -> Box<dyn ListBoxHandle> {
        create_list_box(self.hwnd)
    }
}

fn create_button(parent: HWND, text: &str) -> Box<dyn ButtonHandle> {
    let hwnd = create_child(parent, "BUTTON", ffi::BS_PUSHBUTTON, text);
    Box::new(Win32ButtonHandle { base: Win32Widget::new(hwnd) })
}
fn create_label(parent: HWND, text: &str) -> Box<dyn LabelHandle> {
    let hwnd = create_child(parent, "STATIC", ffi::SS_LEFT, text);
    Box::new(Win32LabelHandle { base: Win32Widget::new(hwnd) })
}
fn create_checkbox(parent: HWND, text: &str) -> Box<dyn CheckBoxHandle> {
    let hwnd = create_child(parent, "BUTTON", ffi::BS_AUTOCHECKBOX, text);
    Box::new(Win32CheckBoxHandle { base: Win32Widget::new(hwnd) })
}
fn create_textbox(parent: HWND, text: &str) -> Box<dyn TextBoxHandle> {
    let hwnd = create_child(parent, "EDIT", ffi::ES_AUTOHSCROLL | ffi::WS_BORDER, text);
    Box::new(Win32TextBoxHandle { base: Win32Widget::new(hwnd) })
}
fn create_radio_button(parent: HWND, text: &str) -> Box<dyn RadioButtonHandle> {
    let hwnd = create_child(parent, "BUTTON", ffi::BS_AUTORADIOBUTTON, text);
    Box::new(Win32RadioButtonHandle { base: Win32Widget::new(hwnd) })
}
fn create_progress_bar(parent: HWND) -> Box<dyn ProgressBarHandle> {
    init_common_controls_once();
    let hwnd = create_child(parent, "msctls_progress32", 0, "");
    Box::new(Win32ProgressBarHandle { base: Win32Widget::new(hwnd) })
}
fn create_picture_box(parent: HWND) -> Box<dyn PictureBoxHandle> {
    let hwnd = create_child(parent, "STATIC", ffi::SS_BITMAP, "");
    Box::new(Win32PictureBoxHandle { base: Win32Widget::new(hwnd) })
}
fn create_tree_view(parent: HWND) -> Box<dyn TreeViewHandle> {
    init_common_controls_once();
    let style = ffi::TVS_HASBUTTONS | ffi::TVS_HASLINES | ffi::TVS_LINESATROOT | ffi::WS_BORDER;
    let hwnd = create_child(parent, "SysTreeView32", style, "");
    Box::new(Win32TreeViewHandle { base: Win32Widget::new(hwnd), items: RefCell::new(Vec::new()) })
}

pub(crate) struct Win32ButtonHandle { base: Win32Widget }
impl_widget_handle!(Win32ButtonHandle, base);
impl ButtonHandle for Win32ButtonHandle {
    fn set_text(&self, text: &str) { set_window_text(self.base.hwnd, text); }
    fn text(&self) -> String { window_text(self.base.hwnd) }
}

pub(crate) struct Win32LabelHandle { base: Win32Widget }
impl_widget_handle!(Win32LabelHandle, base);
impl LabelHandle for Win32LabelHandle {
    fn set_text(&self, text: &str) { set_window_text(self.base.hwnd, text); }
    fn text(&self) -> String { window_text(self.base.hwnd) }
}

pub(crate) struct Win32CheckBoxHandle { base: Win32Widget }
impl_widget_handle!(Win32CheckBoxHandle, base);
impl CheckBoxHandle for Win32CheckBoxHandle {
    fn set_text(&self, text: &str) { set_window_text(self.base.hwnd, text); }
    fn text(&self) -> String { window_text(self.base.hwnd) }
    fn set_checked(&self, checked: bool) {
        let state = if checked { ffi::BST_CHECKED } else { ffi::BST_UNCHECKED };
        unsafe { ffi::SendMessageW(self.base.hwnd, ffi::BM_SETCHECK, state as usize, 0) };
    }
    fn checked(&self) -> bool {
        unsafe { ffi::SendMessageW(self.base.hwnd, ffi::BM_GETCHECK, 0, 0) == ffi::BST_CHECKED }
    }
}

pub(crate) struct Win32TextBoxHandle { base: Win32Widget }
impl_widget_handle!(Win32TextBoxHandle, base);
impl TextBoxHandle for Win32TextBoxHandle {
    fn set_text(&self, text: &str) { set_window_text(self.base.hwnd, text); }
    fn text(&self) -> String { window_text(self.base.hwnd) }
}

pub(crate) struct Win32RadioButtonHandle { base: Win32Widget }
impl_widget_handle!(Win32RadioButtonHandle, base);
impl RadioButtonHandle for Win32RadioButtonHandle {
    fn set_text(&self, text: &str) { set_window_text(self.base.hwnd, text); }
    fn text(&self) -> String { window_text(self.base.hwnd) }
    fn set_checked(&self, checked: bool) {
        let state = if checked { ffi::BST_CHECKED } else { ffi::BST_UNCHECKED };
        unsafe { ffi::SendMessageW(self.base.hwnd, ffi::BM_SETCHECK, state as usize, 0) };
    }
    fn checked(&self) -> bool {
        unsafe { ffi::SendMessageW(self.base.hwnd, ffi::BM_GETCHECK, 0, 0) == ffi::BST_CHECKED }
    }
}

pub(crate) struct Win32ProgressBarHandle { base: Win32Widget }
impl_widget_handle!(Win32ProgressBarHandle, base);
impl ProgressBarHandle for Win32ProgressBarHandle {
    fn set_value(&self, value: u32) {
        unsafe { ffi::SendMessageW(self.base.hwnd, ffi::PBM_SETPOS, value.min(100) as usize, 0) };
    }
    fn value(&self) -> u32 {
        unsafe { ffi::SendMessageW(self.base.hwnd, ffi::PBM_GETPOS, 0, 0) as u32 }
    }
}

fn create_track_bar(parent: HWND) -> Box<dyn TrackBarHandle> {
    init_common_controls_once();
    let hwnd = create_child(parent, "msctls_trackbar32", ffi::TBS_AUTOTICKS, "");
    TRACKBARS.with(|t| t.borrow_mut().insert(hwnd_key(hwnd)));
    unsafe {
        ffi::SendMessageW(hwnd, ffi::TBM_SETRANGEMIN, 1, 0);
        ffi::SendMessageW(hwnd, ffi::TBM_SETRANGEMAX, 1, 100);
    }
    let handle = Win32TrackBarHandle { base: Win32Widget::new(hwnd) };
    handle.update_ticks();
    Box::new(handle)
}

fn create_combo_box(parent: HWND) -> Box<dyn ComboBoxHandle> {
    let hwnd = create_child(parent, "COMBOBOX", ffi::CBS_DROPDOWNLIST | ffi::WS_VSCROLL, "");
    SELECTION_CONTROLS.with(|set| set.borrow_mut().insert(hwnd_key(hwnd)));
    let base = Win32Widget::new(hwnd);
    // Room for the drop-down list under the closed box.
    base.extra_height.set(150);
    base.apply_rect();
    Box::new(Win32ComboBoxHandle { base, items: RefCell::new(Vec::new()) })
}

fn create_list_box(parent: HWND) -> Box<dyn ListBoxHandle> {
    let hwnd = create_child(parent, "LISTBOX", ffi::LBS_NOTIFY | ffi::WS_VSCROLL | ffi::WS_BORDER, "");
    SELECTION_CONTROLS.with(|set| set.borrow_mut().insert(hwnd_key(hwnd)));
    Box::new(Win32ListBoxHandle { base: Win32Widget::new(hwnd), items: RefCell::new(Vec::new()) })
}

pub(crate) struct Win32TrackBarHandle { base: Win32Widget }
impl_widget_handle!(Win32TrackBarHandle, base);
impl Win32TrackBarHandle {
    /// `TBS_AUTOTICKS` draws a tick for every unit by default, which for a range
    /// like 100..1500 merges into one solid bar. Space them about ten across the
    /// range instead (never closer than one unit).
    fn update_ticks(&self) {
        unsafe {
            let min = ffi::SendMessageW(self.base.hwnd, ffi::TBM_GETRANGEMIN, 0, 0) as i64;
            let max = ffi::SendMessageW(self.base.hwnd, ffi::TBM_GETRANGEMAX, 0, 0) as i64;
            let freq = ((max - min) / 10).max(1);
            ffi::SendMessageW(self.base.hwnd, ffi::TBM_SETTICFREQ, freq as usize, 0);
        }
    }
}

impl TrackBarHandle for Win32TrackBarHandle {
    fn set_minimum(&self, minimum: i32) {
        unsafe { ffi::SendMessageW(self.base.hwnd, ffi::TBM_SETRANGEMIN, 1, minimum as isize) };
        self.update_ticks();
    }
    fn minimum(&self) -> i32 {
        unsafe { ffi::SendMessageW(self.base.hwnd, ffi::TBM_GETRANGEMIN, 0, 0) as i32 }
    }
    fn set_maximum(&self, maximum: i32) {
        unsafe { ffi::SendMessageW(self.base.hwnd, ffi::TBM_SETRANGEMAX, 1, maximum as isize) };
        self.update_ticks();
    }
    fn maximum(&self) -> i32 {
        unsafe { ffi::SendMessageW(self.base.hwnd, ffi::TBM_GETRANGEMAX, 0, 0) as i32 }
    }
    fn set_value(&self, value: i32) {
        // The control clamps to its range itself.
        unsafe { ffi::SendMessageW(self.base.hwnd, ffi::TBM_SETPOS, 1, value as isize) };
    }
    fn value(&self) -> i32 {
        unsafe { ffi::SendMessageW(self.base.hwnd, ffi::TBM_GETPOS, 0, 0) as i32 }
    }
    fn set_on_change(&self, callback: Box<dyn FnMut()>) {
        set_change_handler(self.base.hwnd, callback);
    }
}

/// `CB_SETCURSEL`/`LB_SETCURSEL` take -1 (as a `WPARAM`) for "no selection".
fn selection_wparam(index: Option<usize>, len: usize) -> usize {
    match index {
        Some(i) if i < len => i,
        _ => usize::MAX,
    }
}

fn selection_from(result: isize) -> Option<usize> {
    if result < 0 { None } else { Some(result as usize) }
}

pub(crate) struct Win32ComboBoxHandle { base: Win32Widget, items: RefCell<Vec<String>> }
impl_widget_handle!(Win32ComboBoxHandle, base);
impl ComboBoxHandle for Win32ComboBoxHandle {
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

pub(crate) struct Win32ListBoxHandle { base: Win32Widget, items: RefCell<Vec<String>> }
impl_widget_handle!(Win32ListBoxHandle, base);
impl ListBoxHandle for Win32ListBoxHandle {
    fn set_items(&self, items: &[String]) {
        *self.items.borrow_mut() = items.to_vec();
        unsafe {
            ffi::SendMessageW(self.base.hwnd, ffi::LB_RESETCONTENT, 0, 0);
            for item in items {
                let wide = to_wide(item);
                ffi::SendMessageW(self.base.hwnd, ffi::LB_ADDSTRING, 0, wide.as_ptr() as isize);
            }
        }
    }
    fn items(&self) -> Vec<String> {
        self.items.borrow().clone()
    }
    fn set_selected_index(&self, index: Option<usize>) {
        let wparam = selection_wparam(index, self.items.borrow().len());
        unsafe { ffi::SendMessageW(self.base.hwnd, ffi::LB_SETCURSEL, wparam, 0) };
    }
    fn selected_index(&self) -> Option<usize> {
        selection_from(unsafe { ffi::SendMessageW(self.base.hwnd, ffi::LB_GETCURSEL, 0, 0) })
    }
    fn set_on_change(&self, callback: Box<dyn FnMut()>) {
        set_change_handler(self.base.hwnd, callback);
    }
}

thread_local! {
    // No native getter for a `STATIC`'s `STM_SETIMAGE` path, so this
    // crate's own record is the only one — same reasoning as
    // `-min`'s/`win32`'s color caches.
    static IMAGE_PATHS: RefCell<HashMap<usize, String>> = RefCell::new(HashMap::new());
}

pub(crate) struct Win32PictureBoxHandle { base: Win32Widget }
impl_widget_handle!(Win32PictureBoxHandle, base);
impl PictureBoxHandle for Win32PictureBoxHandle {
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

pub(crate) struct Win32TreeViewHandle { base: Win32Widget, items: RefCell<Vec<TreeItem>> }
impl_widget_handle!(Win32TreeViewHandle, base);
impl TreeViewHandle for Win32TreeViewHandle {
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

const CLASS_NAME: &str = "RadWindowMin";
static REGISTER_CLASS: Once = Once::new();
static VISUAL_STYLES: Once = Once::new();
static COMMON_CONTROLS: Once = Once::new();

/// Registers comctl32's own window classes (`msctls_progress32`,
/// `SysTreeView32`) — see the `InitCommonControlsEx` import's doc
/// comment for why the visual-styles manifest alone doesn't do this.
fn init_common_controls_once() {
    COMMON_CONTROLS.call_once(|| unsafe {
        let icc = ffi::InitCommonControlsExStruct {
            dw_size: std::mem::size_of::<ffi::InitCommonControlsExStruct>() as u32,
            dw_icc: ffi::ICC_PROGRESS_CLASS | ffi::ICC_TREEVIEW_CLASSES | ffi::ICC_BAR_CLASSES,
        };
        ffi::InitCommonControlsEx(&icc);
    });
}
static DPI_AWARE: Once = Once::new();

/// Without this, Windows treats the whole process as DPI-unaware and
/// bitmap-stretches its windows to match the system DPI — every control
/// ends up visibly larger/blurrier than the designer's own preview
/// (which is just a browser page, already rendered at native
/// resolution) even though both asked for the same logical size.
fn make_process_dpi_aware_once() {
    DPI_AWARE.call_once(|| unsafe {
        ffi::SetProcessDPIAware();
    });
}

/// `width`/`height` everywhere in this crate are logical (96-DPI, CSS-px
/// equivalent) sizes, to match the designer preview — a browser page,
/// which the OS already scales to the system DPI on its own. Now that
/// this process is DPI-aware (see above), Win32 no longer does that
/// scaling for us, so every logical size has to be converted to
/// physical pixels by hand before it reaches a real Win32 API.
fn dpi_scale() -> f64 {
    unsafe { ffi::GetDpiForSystem() as f64 / 96.0 }
}

/// Without this, every control falls back to ComCtl32 v5 — the flat,
/// square, Windows-2000-era rendering, even on current Windows, since
/// that's what an unmanifested process gets by default. Normally this is
/// fixed by embedding a manifest resource at link time; that needs a
/// resource compiler (`rc.exe`/`windres`), which the no-Cargo pipeline
/// this crate exists for has no access to, so this does it at runtime
/// instead — a real, standalone manifest file, written to a temp path,
/// loaded into a process-wide activation context before any window or
/// control exists. Must run before `register_class_once`'s first
/// `CreateWindowExW`, not just before the main window's: the comctl32
/// version active at a *control's* creation is what decides its theme,
/// not the window that hosts it.
fn activate_visual_styles_once() {
    VISUAL_STYLES.call_once(|| {
        const MANIFEST: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<assembly xmlns="urn:schemas-microsoft-com:asm.v1" manifestVersion="1.0">
  <dependency>
    <dependentAssembly>
      <assemblyIdentity type="win32" name="Microsoft.Windows.Common-Controls" version="6.0.0.0" processorArchitecture="*" publicKeyToken="6595b64144ccf1df" language="*"/>
    </dependentAssembly>
  </dependency>
</assembly>
"#;
        let path = std::env::temp_dir().join("rad-visual-styles.manifest");
        if std::fs::write(&path, MANIFEST).is_err() {
            return;
        }
        let wide_path = to_wide(&path.to_string_lossy());
        let act_ctx = ffi::ActCtxW {
            cb_size: std::mem::size_of::<ffi::ActCtxW>() as u32,
            dw_flags: 0,
            lp_source: wide_path.as_ptr(),
            w_processor_architecture: 0,
            w_lang_id: 0,
            lp_assembly_directory: std::ptr::null(),
            lp_resource_name: std::ptr::null(),
            lp_application_name: std::ptr::null(),
            h_module: std::ptr::null_mut(),
        };
        unsafe {
            let handle = ffi::CreateActCtxW(&act_ctx);
            if (handle as isize) != -1 {
                let mut cookie: usize = 0;
                ffi::ActivateActCtx(handle, &mut cookie);
                // Never deactivated/closed — the context needs to stay
                // live for the whole process, same "leak it" reasoning
                // already used for other process-lifetime handles here.
            }
        }
    });
}

fn register_class_once() {
    REGISTER_CLASS.call_once(|| unsafe {
        let class_name = to_wide(CLASS_NAME);
        let class = ffi::WndClassW {
            style: 0,
            lpfn_wnd_proc: wndproc,
            cb_cls_extra: 0,
            cb_wnd_extra: 0,
            h_instance: ffi::GetModuleHandleW(std::ptr::null()),
            h_icon: std::ptr::null_mut(),
            h_cursor: std::ptr::null_mut(),
            // A system-color *index* used as a pseudo-brush handle, per
            // the `WNDCLASS.hbrBackground` convention: low-order word
            // is the color index plus one, not a real `HBRUSH` at all.
            hbr_background: (ffi::COLOR_BTNFACE + 1) as HWND,
            lpsz_menu_name: std::ptr::null(),
            lpsz_class_name: class_name.as_ptr(),
        };
        ffi::RegisterClassW(&class);
        // The class name buffer must outlive the registered class.
        std::mem::forget(class_name);
    });
}

type ResizeCallback = Box<dyn FnMut(u32, u32)>;

unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, wparam: ffi::WPARAM, lparam: ffi::LPARAM) -> ffi::LRESULT {
    unsafe {
        match msg {
            ffi::WM_SIZE => {
                let user_data = ffi::GetWindowLongPtrW(hwnd, ffi::GWLP_USERDATA);
                if user_data != 0 {
                    let callback = &mut *(user_data as *mut ResizeCallback);
                    // `lParam` here is in physical pixels; convert back
                    // to the logical units every other size in this
                    // crate uses.
                    let scale = dpi_scale();
                    let width = (((lparam as u32) & 0xFFFF) as f64 / scale).round() as u32;
                    let height = (((lparam as u32) >> 16 & 0xFFFF) as f64 / scale).round() as u32;
                    callback(width, height);
                }
                0
            }
            ffi::WM_TIMER => {
                fire_timer(hwnd, wparam);
                0
            }
            ffi::WM_DESTROY => {
                forget_timers_of(hwnd);
                ffi::PostQuitMessage(0);
                0
            }
            ffi::WM_COMMAND => {
                let child = lparam as HWND;
                if !child.is_null() {
                    let is_selection_control =
                        SELECTION_CONTROLS.with(|set| set.borrow().contains(&hwnd_key(child)));
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
                ffi::DefWindowProcW(hwnd, msg, wparam, lparam)
            }
            ffi::WM_HSCROLL => {
                // A trackbar reports every move (drag, keys, page clicks) here.
                let child = lparam as HWND;
                if !child.is_null() {
                    fire_change(child);
                }
                0
            }
            ffi::WM_LBUTTONDOWN => {
                fire_click(hwnd);
                ffi::DefWindowProcW(hwnd, msg, wparam, lparam)
            }
            ffi::WM_CTLCOLORSTATIC | ffi::WM_CTLCOLORBTN => {
                let hdc = wparam as HWND;
                let child = lparam as HWND;
                let result = handle_ctlcolor(hdc, child, true);
                if result != 0 { result } else { ffi::DefWindowProcW(hwnd, msg, wparam, lparam) }
            }
            ffi::WM_CTLCOLOREDIT => {
                let hdc = wparam as HWND;
                let child = lparam as HWND;
                let result = handle_ctlcolor(hdc, child, false);
                if result != 0 { result } else { ffi::DefWindowProcW(hwnd, msg, wparam, lparam) }
            }
            _ => ffi::DefWindowProcW(hwnd, msg, wparam, lparam),
        }
    }
}

pub struct Win32Backend;

impl Backend for Win32Backend {
    fn create_window(&self, title: &str, width: u32, height: u32) -> Box<dyn WindowHandle> {
        Box::new(Win32Window::new(title, width, height))
    }
}

pub struct Win32Window {
    hwnd: HWND,
    callback_slot: *mut ResizeCallback,
}

impl Win32Window {
    fn new(title: &str, width: u32, height: u32) -> Self {
        make_process_dpi_aware_once();
        activate_visual_styles_once();
        register_class_once();
        let class_name = to_wide(CLASS_NAME);
        let window_title = to_wide(title);
        let style = ffi::WS_OVERLAPPEDWINDOW | ffi::WS_VISIBLE;
        // `width`/`height` are the desired *client area* size — the
        // native-exe pipeline feeds this from the preview's own content
        // area (`WebWindow::body_size`), not its outer chrome total,
        // specifically so this matches regardless of how tall a real
        // Win32 title bar turns out to be versus the preview's
        // hand-drawn one. `CreateWindowExW` itself wants the outer
        // window size though, title bar and borders included — hence
        // `AdjustWindowRect` to grow from one to the other.
        let scale = dpi_scale();
        let scaled_width = (width as f64 * scale).round() as i32;
        let scaled_height = (height as f64 * scale).round() as i32;
        let mut rect = ffi::Rect { left: 0, top: 0, right: scaled_width, bottom: scaled_height };
        unsafe { ffi::AdjustWindowRect(&mut rect, style, 0) };
        let hwnd = unsafe {
            ffi::CreateWindowExW(
                0,
                class_name.as_ptr(),
                window_title.as_ptr(),
                style,
                ffi::CW_USEDEFAULT,
                ffi::CW_USEDEFAULT,
                rect.right - rect.left,
                rect.bottom - rect.top,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null(),
            )
        };
        unsafe { ffi::ShowWindow(hwnd, ffi::SW_SHOW) };

        let callback_slot = Box::into_raw(Box::new(Box::new(|_, _| {}) as ResizeCallback));
        unsafe { ffi::SetWindowLongPtrW(hwnd, ffi::GWLP_USERDATA, callback_slot as isize) };

        Self { hwnd, callback_slot }
    }
}

impl WindowHandle for Win32Window {
    fn set_title(&self, title: &str) {
        set_window_text(self.hwnd, title);
    }
    fn title(&self) -> String {
        window_text(self.hwnd)
    }
    fn set_size(&self, width: u32, height: u32) {
        // Same client-area and logical-vs-physical conventions
        // `Win32Window::new` uses.
        let style = ffi::WS_OVERLAPPEDWINDOW | ffi::WS_VISIBLE;
        let scale = dpi_scale();
        let scaled_width = (width as f64 * scale).round() as i32;
        let scaled_height = (height as f64 * scale).round() as i32;
        let mut rect = ffi::Rect { left: 0, top: 0, right: scaled_width, bottom: scaled_height };
        unsafe {
            ffi::AdjustWindowRect(&mut rect, style, 0);
            ffi::SetWindowPos(
                self.hwnd,
                std::ptr::null_mut(),
                0,
                0,
                rect.right - rect.left,
                rect.bottom - rect.top,
                ffi::SWP_NOMOVE | ffi::SWP_NOZORDER,
            );
        }
    }
    fn size(&self) -> (u32, u32) {
        // `GetClientRect` reports physical pixels; convert back to the
        // same logical units `set_size`/`Win32Window::new` take.
        let scale = dpi_scale();
        unsafe {
            let mut rect = ffi::Rect { left: 0, top: 0, right: 0, bottom: 0 };
            ffi::GetClientRect(self.hwnd, &mut rect);
            (
                ((rect.right - rect.left) as f64 / scale).round() as u32,
                ((rect.bottom - rect.top) as f64 / scale).round() as u32,
            )
        }
    }
    fn set_on_resize(&self, callback: Box<dyn FnMut(u32, u32)>) {
        unsafe { *self.callback_slot = callback };
    }
    fn set_on_click(&self, callback: Box<dyn FnMut()>) {
        set_click_handler(self.hwnd, callback);
    }
    fn set_default_icon(&self) {
        unsafe {
            let icon = ffi::LoadIconW(std::ptr::null_mut(), ffi::IDI_APPLICATION);
            ffi::SendMessageW(self.hwnd, ffi::WM_SETICON, ffi::ICON_BIG, icon as isize);
            ffi::SendMessageW(self.hwnd, ffi::WM_SETICON, ffi::ICON_SMALL, icon as isize);
        }
    }
}

impl Host for Win32Window {
    fn create_button(&self, text: &str) -> Box<dyn ButtonHandle> { create_button(self.hwnd, text) }
    fn create_label(&self, text: &str) -> Box<dyn LabelHandle> { create_label(self.hwnd, text) }
    fn create_checkbox(&self, text: &str) -> Box<dyn CheckBoxHandle> { create_checkbox(self.hwnd, text) }
    fn create_textbox(&self, text: &str) -> Box<dyn TextBoxHandle> { create_textbox(self.hwnd, text) }
    fn create_radio_button(&self, _group: &str, text: &str) -> Box<dyn RadioButtonHandle> {
        create_radio_button(self.hwnd, text)
    }
    fn create_progress_bar(&self) -> Box<dyn ProgressBarHandle> {
        create_progress_bar(self.hwnd)
    }
    fn create_picture_box(&self) -> Box<dyn PictureBoxHandle> {
        create_picture_box(self.hwnd)
    }
    fn create_tree_view(&self) -> Box<dyn TreeViewHandle> {
        create_tree_view(self.hwnd)
    }
    fn create_timer(&self) -> Box<dyn TimerHandle> {
        create_timer(self.hwnd)
    }
    fn create_track_bar(&self) -> Box<dyn TrackBarHandle> {
        create_track_bar(self.hwnd)
    }
    fn create_combo_box(&self) -> Box<dyn ComboBoxHandle> {
        create_combo_box(self.hwnd)
    }
    fn create_list_box(&self) -> Box<dyn ListBoxHandle> {
        create_list_box(self.hwnd)
    }
}

// Window must outlive the process in every real use of this backend
// (the message loop owns it for the app's whole lifetime), so it has no
// `Drop` that frees `callback_slot` — same reasoning `rad-backend-win32`
// itself documents.
