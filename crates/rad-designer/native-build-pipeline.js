// Native `.exe` counterpart to `build-pipeline.js`. Same mount-tree idea
// (see that file and `native_build.rs`'s doc comments for why source is
// mounted directly instead of `--extern`/`.rlib`), but a materially
// different toolchain underneath: producing a real Windows PE needs the
// *threaded* WASI driver (`WASIFarmAnimal`, not the simpler single-shot
// farm `build-pipeline.js` uses), a locally-hosted compiler (nobody
// publishes a build with the PE/COFF linker-driver fix this project
// needed — see `.research/spike-exe/`), and a swapped-in real MinGW-w64
// `crt2.o` in place of the one bundled with that compiler's own sysroot,
// which is stale enough to make the program crash at startup
// (`STATUS_STACK_OVERFLOW`) despite linking and loading just fine.
//
// Absolute paths throughout for the same reason `build-pipeline.js` uses
// them: `wasm-bindgen` relocates this file into `snippets/<hash>/` at
// build time.
import {
  Directory,
  PreopenDirectory,
  File,
  OpenFile,
  ConsoleStdout,
} from "/vendor/browser_wasi_shim/index.js";
import { WASIFarm } from "/vendor/browser_wasi_shim_threads/browser-wasi-shim-threads.es.js";

const TOOLCHAIN_BASE = "/native-toolchain/";
const CACHE_NAME = "rad-native-build-toolchain-v3";

/** Targets a toolchain can serve. The compiler (`rustc_opt.wasm`) is shared;
 *  everything target-specific lives in `native-toolchain/<triple>/`:
 *  `crt2.o`, `mingw-libs/` and `sysroot/`, each with `index.json` manifests.
 *
 *  `rsObjects`: whether the sysroot carries `rsbegin.o`/`rsend.o`. The
 *  `*-windows-gnu` targets link them; `*-windows-gnullvm` does not (its only
 *  self-contained object is `crt2.o`), so fetching them there would 404. */
const TARGET_INFO = {
  "x86_64-pc-windows-gnu": { rsObjects: true },
  "i686-pc-windows-gnu": { rsObjects: true },
  "aarch64-pc-windows-gnullvm": { rsObjects: false },
};
export const NATIVE_TARGETS = Object.keys(TARGET_INFO);

async function cachedFetch(url, onProgress) {
  const cache = await caches.open(CACHE_NAME);
  const cached = await cache.match(url);
  if (cached) return new Uint8Array(await cached.arrayBuffer());
  onProgress?.(`Downloading ${url.split("/").pop()}...`);
  const response = await fetch(url);
  if (!response.ok) throw new Error(`fetch ${url} failed: ${response.status}`);
  await cache.put(url, response.clone());
  return new Uint8Array(await response.arrayBuffer());
}

function buildTree(flatFiles) {
  const root = new Map();
  for (const [filePath, contents] of Object.entries(flatFiles)) {
    const parts = filePath.replace(/^\//, "").split("/");
    let dir = root;
    for (let i = 0; i < parts.length - 1; i++) {
      const seg = parts[i];
      if (!dir.has(seg)) dir.set(seg, new Directory(new Map()));
      dir = dir.get(seg).contents;
    }
    const bytes = typeof contents === "string" ? new TextEncoder().encode(contents) : contents;
    dir.set(parts[parts.length - 1], new File(bytes));
  }
  return root;
}

function runWorker(farmRef, rustcWasmBytes, args) {
  return new Promise((resolve, reject) => {
    // Shared with `build-pipeline.js` — same WASIFarmAnimal/thread-spawn
    // setup regardless of which compiler/target it's actually running.
    const worker = new Worker("/build-worker.js", { type: "module" });
    worker.onmessage = (e) => resolve(e.data);
    worker.onerror = (e) => reject(new Error(e.message));
    worker.postMessage({ farmRef, rustcWasmBytes, args }, [rustcWasmBytes.buffer]);
  });
}

/** A toolchain lists its own files in `index.json` manifests (an array of
 *  file stems): which crates make up the sysroot, and which import
 *  libraries the linker may ask for, depend on the compiler version. */
async function fetchManifest(base, path) {
  const response = await fetch(`${base}${path}`);
  if (!response.ok) throw new Error(`toolchain is missing ${base}${path} (HTTP ${response.status})`);
  return response.json();
}

/**
 * Compiles `files` (a `Map`/plain-object of mount path -> source text,
 * rooted at `/main.rs`) to a native Windows `.exe` for `triple` (one of
 * `NATIVE_TARGETS`). `onProgress(message)` reports status for a UI to show.
 */
export async function buildNativeApp(files, triple, onProgress) {
  if (!NATIVE_TARGETS.includes(triple)) {
    throw new Error(`unsupported native target ${triple} (expected one of ${NATIVE_TARGETS.join(", ")})`);
  }
  const TARGET_BASE = `${TOOLCHAIN_BASE}${triple}/`;
  onProgress?.(`Downloading native toolchain (${triple})...`);
  const [rustcWasmBytes, mingwCrt2] = await Promise.all([
    cachedFetch(`${TOOLCHAIN_BASE}rustc_opt.wasm`, onProgress),
    cachedFetch(`${TARGET_BASE}crt2.o`, onProgress),
  ]);

  // Each `.rlib` is only a metadata stub; current rustc needs its sibling
  // `.rmeta` next to it. A missing file fails the build loudly here, not as
  // an obscure rustc error later.
  const stems = await fetchManifest(TARGET_BASE, "sysroot/index.json");
  const libDir = new Map();
  await Promise.all(
    stems.map(async (stem) => {
      const [rlib, rmeta] = await Promise.all([
        cachedFetch(`${TARGET_BASE}sysroot/${stem}.rlib`, onProgress),
        cachedFetch(`${TARGET_BASE}sysroot/${stem}.rmeta`, onProgress),
      ]);
      libDir.set(`${stem}.rlib`, new File(rlib));
      libDir.set(`${stem}.rmeta`, new File(rmeta));
    }),
  );
  if (TARGET_INFO[triple].rsObjects) {
    const [rsbegin, rsend] = await Promise.all([
      cachedFetch(`${TARGET_BASE}sysroot/rsbegin.o`, onProgress),
      cachedFetch(`${TARGET_BASE}sysroot/rsend.o`, onProgress),
    ]);
    libDir.set("rsbegin.o", new File(rsbegin));
    libDir.set("rsend.o", new File(rsend));
  }
  libDir.set("self-contained", new Directory(new Map([["crt2.o", new File(mingwCrt2)]])));

  const mingwStems = await fetchManifest(TARGET_BASE, "mingw-libs/index.json");
  const mingwLibDir = new Map();
  await Promise.all(
    mingwStems.map(async (stem) => {
      const bytes = await cachedFetch(`${TARGET_BASE}mingw-libs/${stem}.a`, onProgress);
      mingwLibDir.set(`${stem}.a`, new File(bytes));
    }),
  );

  onProgress?.("Compiling...");

  const sysrootDir = new PreopenDirectory(
    "/sysroot",
    new Map([
      ["lib", new Directory(new Map([
        ["rustlib", new Directory(new Map([
          [triple, new Directory(new Map([["lib", new Directory(libDir)]]))],
        ]))],
      ]))],
    ]),
  );
  const mingwDir = new PreopenDirectory("/mingw-libs", mingwLibDir);

  const flatAppFiles = files instanceof Map ? Object.fromEntries(files) : files;
  const workDir = new PreopenDirectory("/work", buildTree({ ...flatAppFiles, "out.exe": new Uint8Array() }));

  let stdoutBuf = "";
  let stderrBuf = "";
  const stdout = new ConsoleStdout((buf) => { stdoutBuf += new TextDecoder().decode(buf); });
  const stderr = new ConsoleStdout((buf) => { stderrBuf += new TextDecoder().decode(buf); });
  const stdin = new OpenFile(new File(new Uint8Array()));
  const rootDir = new PreopenDirectory("/", new Map());

  const farm = new WASIFarm(stdin, stdout, stderr, [sysrootDir, mingwDir, workDir, rootDir], {
    allocator_size: 256 * 1024 * 1024,
  });

  const SYSLIB = `/sysroot/lib/rustlib/${triple}/lib`;
  const result = await runWorker(farm.get_ref(), rustcWasmBytes, [
    "rustc", "/work/main.rs",
    "--crate-name", "app",
    "--crate-type", "bin",
    "--edition", "2021",
    "--target", triple,
    "--sysroot", "/sysroot",
    "-C", "linker-flavor=ld.lld",
    "-C", `link-arg=${SYSLIB}/self-contained/crt2.o`,
    "-C", `link-arg=-L${SYSLIB}`,
    "-C", "link-arg=-L/mingw-libs",
    // A real GUI app shouldn't pop a console window alongside its
    // window — mingw's crt2.o provides both entry points, and its
    // WinMainCRTStartup still ends up calling our `main()`.
    "-C", "link-arg=--subsystem=windows",
    "-o", "/work/out.exe",
  ]);

  if (result.crashed) {
    throw new Error(stderrBuf ? `Compile error:\n${stderrBuf}` : `rustc crashed: ${result.error}`);
  }
  if (result.exitCode !== 0) throw new Error(`rustc failed (exit ${result.exitCode}):\n${stderrBuf}`);

  const outFile = workDir.dir.contents.get("out.exe");
  if (!outFile || !outFile.data || outFile.data.byteLength === 0) {
    throw new Error("rustc exited 0 but did not produce out.exe");
  }
  return outFile.data;
}
