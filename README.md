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
| MinGW runtime (MSVCRT) → x64 | `x86_64-pc-windows-gnu` | Windows 10+, 64-bit | `form1-x64.exe` |
| MinGW runtime (MSVCRT) → x86 | `i686-pc-windows-gnu` | Windows 10+, 32-bit or 64-bit | `form1-x86.exe` |
| LLVM runtime (UCRT) → x64 | `x86_64-pc-windows-gnullvm` | Windows 10+, 64-bit | `form1-x64-ucrt.exe` |
| LLVM runtime (UCRT) → x86 | `i686-pc-windows-gnullvm` | Windows 10+, 32-bit or 64-bit | `form1-x86-ucrt.exe` |
| LLVM runtime (UCRT) → ARM64 | `aarch64-pc-windows-gnullvm` | Windows 10+ on ARM64 | `form1-arm64.exe` |

Every Windows target needs **Windows 10 or later**: current rustc's own platform-support docs list `x86_64-pc-windows-gnu` and `i686-pc-windows-gnu` as Windows 10+, and Windows 7 support exists only in separate, unsupported-tier `*-win7-windows-gnu` targets, which this designer does not offer. The two groups differ in toolchain, not in Windows version: the `-gnu` targets use the classic `msvcrt.dll` with GCC-style MinGW runtime pieces (`libgcc`), while the `-gnullvm` targets use an all-LLVM toolchain (llvm-mingw, LLVM libunwind) and the Universal C Runtime. (The 64-bit download used to be called `form1.exe`; it is now `form1-x64.exe`.)

**ARM64 output has only been checked as a well-formed PE (machine `0xAA64`, correct imports, GUI subsystem); it has never been run**, because our test machine is x86-64. Treat it as untested on real hardware.

All of `rad-core` and the backend are mounted as sibling `mod` files of a single crate, because that toolchain has no Cargo and cannot produce `.rlib`s. `rad-backend-win32` has no dependencies and builds for all architectures from the same source.

### Toolchain is not in this repo

The native `.exe` build needs a patched wasm-hosted `rustc` (about 220 MB) plus a Windows standard library and MinGW runtime per architecture, expected under `crates/rad-designer/native-toolchain/` (ignored by git):

```
native-toolchain/
  rustc_opt.wasm                  the compiler, shared by all targets
  x86_64-pc-windows-gnu/          and the same again for i686-pc-windows-gnu/,
                                  x86_64-pc-windows-gnullvm/, i686-pc-windows-gnullvm/ and
                                  aarch64-pc-windows-gnullvm/ (no rsbegin.o/rsend.o for gnullvm)
    crt2.o                        MinGW C runtime start-up object
    mingw-libs/*.a, index.json    import libraries + libgcc (libunwind for gnullvm); index.json lists their stems
    sysroot/*.rlib, *.rmeta,      the target's standard library (each .rlib needs its
      rsbegin.o, rsend.o,         .rmeta next to it) and a manifest of the rlib stems
      index.json
```

[rad-rs/wasm-rustc-windows](https://github.com/rad-rs/wasm-rustc-windows) builds the compiler and has `scripts/assemble-designer-toolchain.sh`, which writes exactly this layout. A missing file fails the build with its name. The web-app build downloads its toolchain from a public host on first use and caches it in the browser.

## Status

Early and experimental. Not every widget is covered by tests, and the build pipelines depend on unreleased compiler work.
