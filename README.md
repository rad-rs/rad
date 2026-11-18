# rad

A rapid-application-development (RAD) framework and visual designer for Rust: drag controls onto a form, wire up events, and build a real app from it, in the style of classic form designers.

## Crates

| Crate | Role |
|---|---|
| `rad-core` | Widget model and the `Backend` trait. See [Components](#components). |
| `rad-backend-win32` | Native Win32 backend. No external dependencies (hand-declared WinAPI), so the same source builds with Cargo and as plain mounted source in the designer's in-browser pipeline. |
| `rad-backend-gtk` | Native Linux backend on GTK 3. Same shape as the Win32 one: no crate dependencies (hand-declared FFI to `libgtk-3`, in `rad-core/src/platform/gtk.rs`), so it also works as plain mounted source. See [Linux](#linux-gtk-3). |
| `rad-backend-web` | Browser backend built on `wasm-bindgen`/`web-sys`. Used by the designer itself. |
| `rad-backend-web-min` | Browser backend with no `wasm-bindgen`, driven by a small JS shim (`runtime.js`). Used for apps compiled by the in-browser pipeline. |
| `rad-demo`, `rad-demo-native`, `rad-demo-linux` | The all-controls form built on the browser backend, on the Win32 backend and on the GTK backend. |
| `rad-designer` | The visual designer (runs in the browser, built with `trunk`). |

## Components

| Component | Notes |
|---|---|
| `Button`, `Label`, `CheckBox`, `TextBox`, `RadioButton` | Text controls. |
| `ProgressBar`, `PictureBox`, `TreeView` | Display controls. |
| `TrackBar` | Slider with `minimum`, `maximum` (0 to 100 by default) and `value`. |
| `ComboBox`, `ListBox` | Items plus a selected index (`None` for no selection); `text()` returns the selected item. |
| `Timer` | **Non-visual.** `set_interval(ms)`, `set_enabled(bool)` (or `start()`/`stop()`) and `set_on_tick`. Disabled until enabled, 100 ms by default. |

`TrackBar`, `ComboBox` and `ListBox` raise `set_on_change`, fired when the user changes them (not when code does). A `Timer` has no place on the form: in the designer it appears as a chip in a component tray below the form, and double-clicking it stubs out its `tick` handler.

All of them are implemented for every platform (Win32, GTK, web, web-min) and are available in the designer's toolbox.

### Adding a component

A component is **one file** in `crates/rad-core/src/components/`; nothing else is edited. Copy a small one (`button.rs`) and change:

- the model: a `<Name>Handle` trait, the `<Name>` wrapper (`new(host, ...)` calls `imp::create(&host.parent(), ...)`) and `impl_widget_common!`;
- `pub(crate) const INFO: ComponentInfo`: toolbox name, category, icon and order, default size, constructor arguments, properties (name, type, default), events, and a declarative `Preview` (tag, attributes, CSS) that both browser platforms build their DOM from;
- `impl Designable`: `get`/`set` by property name, which is how the designer edits it and writes `form1.designer.rs`;
- one `mod imp` per platform, gated by `#[cfg(rad_win32)]`, `#[cfg(rad_gtk)]`, `#[cfg(rad_web)]` or `#[cfg(rad_web_min)]` (a macOS one would be `rad_macos`), using the helpers in `rad-core/src/platform/`.

The file name is the module name; its top-level `pub` items are re-exported from `rad_core`. `rad-core`'s `build.rs` finds the files and generates the module list and the `COMPONENTS` registry; the designer's `build.rs` does the same scan to mount them in the in-browser builds and fill the toolbox. Backends only create windows and run the message loop. The platform is chosen by `build.rs` from the target (Windows: `rad_win32`, Linux: `rad_gtk`) and the `web`/`web-min` features; the in-browser pipelines pass `--cfg rad_web_min` / `--cfg rad_win32` to rustc.

In the designer, drag a control from the toolbox onto the form, click it to select it, double-click it for its event handler, and press **Delete** to remove the selected control or timer. `form1.designer.rs` is regenerated, so the removal is immediate there. A handler stub already written into `form1.rs` is yours and is left alone: the designer warns that `form1.rs` still refers to the removed control, and a build fails with the compile error until you delete that stub. A new control never reuses the name of one that `form1.rs` still refers to.

## Build

```
cargo build -p rad-demo-native        # native Windows demo
cargo build -p rad-demo-linux         # native Linux demo (needs libgtk-3-dev)
cd crates/rad-designer && trunk serve # designer, http://127.0.0.1:8080
```

The designer needs cross-origin isolation (it registers `coi-serviceworker.js`) because the in-browser compiler uses threads.

## Linux (GTK 3)

`rad-backend-gtk` and `rad-demo-linux` build only on Linux (on other OSes they are empty, so a workspace build there is unaffected), and need the GTK 3 development files:

```
sudo apt install libgtk-3-dev          # Debian/Ubuntu; also pkg-config and a C linker
cargo build -p rad-demo-linux
cargo run -p rad-demo-linux
```

There are no crate dependencies: `rad-core/src/platform/gtk.rs` declares the handful of GTK/GObject/GLib functions it needs and links `libgtk-3`, `libgobject-2.0`, `libglib-2.0` and `libgdk_pixbuf-2.0`. Under WSL2 with WSLg it opens as a normal window.

| Component | GTK widget |
|---|---|
| Button, CheckBox, RadioButton, TextBox | `GtkButton`, `GtkCheckButton`, `GtkRadioButton` (grouped by group name per form), `GtkEntry` |
| Label, PictureBox | `GtkLabel` / `GtkImage` inside a `GtkEventBox`, which gives them clicks and a back colour (`GtkImage` loads whatever gdk-pixbuf can: PNG, JPEG, BMP...) |
| ProgressBar, TrackBar | `GtkProgressBar`, `GtkScale` (about ten tick marks across the range) |
| ComboBox, ListBox | `GtkComboBoxText`, `GtkListBox` in a scrolled window |
| TreeView | `GtkTreeView` over a `GtkTreeStore`, expanded |
| Timer | `g_timeout_add` source; disabled until enabled, 100 ms by default |

Controls sit on a `GtkFixed` for absolute positions, and colours are applied with a per-widget CSS provider. As on Win32, `set_on_change` handlers fire only for user changes, never for changes made by code. `TextBox::set_on_click` fires on Enter (a `GtkEntry` consumes mouse presses).


### Targets

The same three architectures as the Windows targets, all gnu/glibc:

| Triple | Architecture | Runs on |
|---|---|---|
| `x86_64-unknown-linux-gnu` | x64 | native Linux, WSL2 + WSLg |
| `i686-unknown-linux-gnu` | x86 | any x86_64 Linux with 32-bit execution enabled (the default on Debian) |
| `aarch64-unknown-linux-gnu` | ARM64 | ARM64 Linux, or x86_64 Linux through `qemu-user-static` |

Cross-building from x86_64 Debian/Ubuntu:

```
sudo dpkg --add-architecture i386 && sudo dpkg --add-architecture arm64 && sudo apt update
sudo apt install libgtk-3-dev libgtk-3-dev:i386 libgtk-3-dev:arm64 gcc-i686-linux-gnu gcc-aarch64-linux-gnu
scripts/build-linux-all.sh             # all three; or pass triples: scripts/build-linux-all.sh i686-unknown-linux-gnu
```

The script adds the rustup targets and sets the per-target linker through `CARGO_TARGET_*_LINKER` environment variables, so there is no `.cargo/config.toml` and the Windows and native builds are untouched. Under WSL, set `CARGO_TARGET_DIR` to a path on the Linux filesystem for speed.

To run the foreign builds headless, install `xvfb xdotool imagemagick qemu-user-static` and use `scripts/verify-linux-xvfb.sh <i686|aarch64> <outdir>`, which starts Xvfb, runs the demo (aarch64 through `qemu-aarch64-static` with `QEMU_LD_PREFIX`/`LD_LIBRARY_PATH` pointing at the arm64 libraries), and screenshots the initial form, the timer ticking and the result of a Greet click.

Verified (WSL2 Debian 13): x86_64 built and run under WSLg; i686 built (ELF32, Intel 80386) and run natively under Xvfb; aarch64 built (ELF64, AArch64) and run under qemu-user with the arm64 GTK 3 libraries on Xvfb. For i686 and aarch64 the initial render, the Greet click and the timer were exercised. Not verified: real ARM64 hardware (qemu-user only; timing and rendering performance untested), the 32-bit build on a non-x86 host, and any non-glibc (musl) target. The hand-declared FFI was audited for 32-bit: every `gulong`/`gsize`/`gssize`/`GType` is `c_ulong`/`usize`/`isize`, and the only struct layouts used (`GtkTreeIter`, `GdkRectangle`) are pointer/int sized per their C definitions; no change was needed.
Not supported: in-browser Linux builds. The designer's **Target** selector has no Linux entry, and compiling a form to a Linux binary in the browser is out of scope (it would need a Linux std, a GTK link line and an ELF linker in the wasm toolchain). The designer's toolbox and generated code are unaffected: build the generated sources with Cargo on Linux.

## In-browser builds

The designer can compile the form you are editing without a server:

- **Web app:** runs `rustc` (compiled to WebAssembly) in a worker and produces a `.wasm` for the browser backend.
- **Windows `.exe`:** the same wasm-hosted `rustc` links a real Windows PE through its in-process LLD.

One **Target** selector and one **Build** button cover everything. The selection is remembered per browser.

| Target (selector entry) | rustc target | Runs on | Download |
|---|---|---|---|
| Browser → Web app | `wasm32` (browser backend) | any browser | `form1.html` |
| Windows x64 | `x86_64-pc-windows-gnullvm` | Windows 10+, 64-bit | `form1-x64.exe` |
| Windows x86 | `i686-pc-windows-gnullvm` | Windows 10+, 32-bit or 64-bit | `form1-x86.exe` |
| Windows ARM64 | `aarch64-pc-windows-gnullvm` | Windows 10+ on ARM64 | `form1-arm64.exe` |

All Windows targets use the `gnullvm` flavour: an all-LLVM toolchain (llvm-mingw, LLVM libunwind) and the Universal C Runtime, so they need **Windows 10 or later**. (Windows 7 support exists only in separate, unsupported-tier `*-win7-windows-gnu` targets, which this designer does not offer.) Earlier versions of this designer also offered `*-windows-gnu` (MinGW/MSVCRT) builds; those were dropped to keep a single, smaller toolchain.

**ARM64 output has only been checked as a well-formed PE (machine `0xAA64`, correct imports, GUI subsystem); it has never been run**, because our test machine is x86-64. Treat it as untested on real hardware.

All of `rad-core` and the backend are mounted as sibling `mod` files of a single crate, because that toolchain has no Cargo and cannot produce `.rlib`s. `rad-backend-win32` has no dependencies and builds for all architectures from the same source.

### Toolchain is not in this repo

The native `.exe` build needs a patched wasm-hosted `rustc` plus a Windows standard library and MinGW runtime per architecture, expected under `crates/rad-designer/native-toolchain/` (ignored by git): Sizes (all uncompressed): the compiler is about 104 MB (`wasm-opt -Os` of a 132 MB build) and each target directory is about 103-110 MB, so about 423 MB for the three Windows targets. The browser downloads only the compiler and the selected target's files, and caches them (Cache Storage) after the first build; the compiler compresses to about 21 MB with brotli.

```
native-toolchain/
  rustc_opt.wasm                  the compiler, shared by all targets
  x86_64-pc-windows-gnullvm/      and the same again for i686-pc-windows-gnullvm/ and
                                  aarch64-pc-windows-gnullvm/ (no rsbegin.o/rsend.o for gnullvm)
    crt2.o                        MinGW C runtime start-up object
    mingw-libs/*.a, index.json    import libraries and libunwind; index.json lists their stems
    sysroot/*.rlib, *.rmeta,      the target's standard library (each .rlib needs its
      index.json                  .rmeta next to it) and a manifest of the rlib stems
```

[rad-rs/wasm-rustc-windows](https://github.com/rad-rs/wasm-rustc-windows) builds the compiler and has `scripts/assemble-designer-toolchain.sh`, which writes exactly this layout. A missing file fails the build with its name. The web-app build downloads its toolchain from a public host on first use and caches it in the browser.

## Status

Early and experimental. Not every widget is covered by tests, and the build pipelines depend on unreleased compiler work.
