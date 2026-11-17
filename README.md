# rad

A rapid-application-development (RAD) framework and visual designer for Rust: drag controls onto a form, wire up events, and build a real app from it, in the style of classic form designers.

## Crates

| Crate | Role |
|---|---|
| `rad-core` | Widget model (`Window`, `Button`, `Label`, `CheckBox`, `TextBox`, `RadioButton`, `ProgressBar`, `PictureBox`, `TreeView`) and the `Backend` trait. |
| `rad-backend-win32` | Native Win32 backend. No external dependencies (hand-declared WinAPI), so the same source builds with Cargo and as plain mounted source in the designer's in-browser pipeline. |
| `rad-backend-web` | Browser backend built on `wasm-bindgen`/`web-sys`. Used by the designer itself. |
| `rad-backend-web-min` | Browser backend with no `wasm-bindgen`, driven by a small JS shim (`runtime.js`). Used for apps compiled by the in-browser pipeline. |
| `rad-demo`, `rad-demo-native` | A form built on the browser backend, and one built on the Win32 backend. |
| `rad-designer` | The visual designer (runs in the browser, built with `trunk`). |

## Build

```
cargo build -p rad-demo-native        # native Windows demo
cd crates/rad-designer && trunk serve # designer, http://127.0.0.1:8080
```

The designer needs cross-origin isolation (it registers `coi-serviceworker.js`) because the in-browser compiler uses threads.

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
