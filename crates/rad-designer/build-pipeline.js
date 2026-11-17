// Drives the in-browser `rustc` (proven in `.research/spike`) to compile
// the designer's current project into a standalone app `.wasm` — no
// Cargo (this build has no working archiver, so `.rlib`s/`--extern`
// don't work; everything the app needs is mounted as sibling files of
// one crate via plain `mod` declarations instead) and no server (the
// toolchain is a static download, cached in Cache Storage after the
// first build).
// Absolute (root-relative), not relative: `wasm-bindgen` copies this
// file into its own `snippets/<hash>/` directory at build time, so a
// `./`-relative import would resolve against the wrong location —
// `/vendor/...` and `/build-worker.js` are real paths at the site root
// regardless of where this file physically ends up.
import {
  Directory,
  PreopenDirectory,
  File,
  OpenFile,
  ConsoleStdout,
} from "/vendor/browser_wasi_shim/index.js";
import { WASIFarm } from "/vendor/browser_wasi_shim_threads/browser-wasi-shim-threads.es.js";

const TOOLCHAIN_VERSION = "v0.2.1";
const CDN_BASE = `https://oligamiq.github.io/rust_wasm/${TOOLCHAIN_VERSION}/`;
const CACHE_NAME = "rad-build-toolchain-" + TOOLCHAIN_VERSION;

// The sysroot tarball also carries non-rlib build metadata (`.d` files,
// `self-contained/`); only the top-level `.rlib`s are needed to link a
// `wasm32-unknown-unknown` binary against `libstd` and friends.
function isSysrootRlib(tarPath) {
  return tarPath.endsWith(".rlib") && !tarPath.includes("/self-contained/");
}

async function cachedFetch(url, onProgress) {
  const cache = await caches.open(CACHE_NAME);
  const cached = await cache.match(url);
  if (cached) return cached;
  onProgress?.(`Downloading ${url.split("/").pop()}...`);
  const response = await fetch(url);
  if (!response.ok) throw new Error(`fetch ${url} failed: ${response.status}`);
  await cache.put(url, response.clone());
  return response;
}

async function fetchAndUntar(url, onProgress) {
  const response = await cachedFetch(url, onProgress);
  const decompressed = response.body.pipeThrough(new DecompressionStream("gzip"));
  const bytes = new Uint8Array(await new Response(decompressed).arrayBuffer());
  return untar(bytes);
}

function readAsciiZ(bytes, start, len) {
  let end = start;
  while (end < start + len && bytes[end] !== 0) end++;
  return new TextDecoder().decode(bytes.subarray(start, end));
}

/** Minimal POSIX tar reader — just enough for the plain-file entries a
 *  `tar czf` of a flat directory produces; no links/sparse/PAX support. */
function untar(bytes) {
  const files = {};
  let offset = 0;
  while (offset + 512 <= bytes.length) {
    const header = bytes.subarray(offset, offset + 512);
    if (header.every((b) => b === 0)) break;
    const name = readAsciiZ(header, 0, 100);
    const sizeOctal = readAsciiZ(header, 124, 12).trim();
    const size = sizeOctal ? parseInt(sizeOctal, 8) : 0;
    const typeFlag = String.fromCharCode(header[156] || 0);
    offset += 512;
    if ((typeFlag === "0" || typeFlag === "\0") && name && size > 0) {
      files[name] = bytes.subarray(offset, offset + size);
    }
    offset += Math.ceil(size / 512) * 512;
  }
  return files;
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
    const worker = new Worker("/build-worker.js", { type: "module" });
    worker.onmessage = (e) => resolve(e.data);
    worker.onerror = (e) => reject(new Error(e.message));
    // Transferred, not cloned: the compiler module is ~90MB.
    worker.postMessage({ farmRef, rustcWasmBytes, args }, [rustcWasmBytes.buffer]);
  });
}

/**
 * Compiles `files` (a `Map`/plain-object of mount path -> source text,
 * rooted at `/main.rs`) to a `wasm32-unknown-unknown` binary.
 * `onProgress(message)` reports status for a UI to show.
 */
export async function buildApp(files, onProgress) {
  const [rustcTar, sysrootTar] = await Promise.all([
    fetchAndUntar(`${CDN_BASE}rustc_opt.wasm.tar.gz`, onProgress),
    fetchAndUntar(`${CDN_BASE}wasm32-unknown-unknown.tar.gz`, onProgress),
  ]);
  const rustcWasmEntry = Object.entries(rustcTar).find(([name]) => name.endsWith("rustc_opt.wasm"));
  if (!rustcWasmEntry) throw new Error("rustc_opt.wasm.tar.gz did not contain rustc_opt.wasm");
  const rustcWasmBytes = rustcWasmEntry[1];

  onProgress?.("Compiling...");

  const libDir = new Map();
  for (const [tarPath, bytes] of Object.entries(sysrootTar)) {
    if (isSysrootRlib(tarPath)) libDir.set(tarPath.split("/").pop(), new File(bytes));
  }
  const sysrootDir = new PreopenDirectory(
    "/sysroot",
    new Map([
      ["lib", new Directory(new Map([
        ["rustlib", new Directory(new Map([
          ["wasm32-unknown-unknown", new Directory(new Map([["lib", new Directory(libDir)]]))],
        ]))],
      ]))],
    ]),
  );

  const flatAppFiles = files instanceof Map ? Object.fromEntries(files) : files;
  const workDir = new PreopenDirectory("/", buildTree(flatAppFiles));

  let stderrBuf = "";
  const stdout = new ConsoleStdout(() => {});
  const stderr = new ConsoleStdout((buf) => { stderrBuf += new TextDecoder().decode(buf); });
  const stdin = new OpenFile(new File(new Uint8Array()));

  const farm = new WASIFarm(stdin, stdout, stderr, [sysrootDir, workDir], {
    allocator_size: 256 * 1024 * 1024,
  });

  const result = await runWorker(farm.get_ref(), rustcWasmBytes, [
    "rustc", "/main.rs",
    "--sysroot", "/sysroot",
    "--target", "wasm32-unknown-unknown",
    "--crate-type", "cdylib",
    "--edition", "2024",
    // Selects rad-core's platform: components are compiled for `rad_web_min`.
    "--cfg", "rad_web_min",
    "-Zunstable-options",
    "-o", "/out.wasm",
  ]);

  // This build of rustc traps (a wasm `unreachable`) on a plain compile
  // error instead of exiting cleanly with a nonzero code — so `crashed`
  // covers ordinary mistakes in the user's own code, not just a genuine
  // compiler bug, and needs the real diagnostic surfaced the same way.
  if (result.crashed) {
    throw new Error(stderrBuf ? `Compile error:\n${stderrBuf}` : `rustc crashed: ${result.error}`);
  }
  if (result.exitCode !== 0) throw new Error(`rustc failed (exit ${result.exitCode}):\n${stderrBuf}`);

  const outFile = workDir.dir.contents.get("out.wasm");
  if (!outFile) throw new Error("rustc exited 0 but did not produce out.wasm");
  return outFile.data;
}
