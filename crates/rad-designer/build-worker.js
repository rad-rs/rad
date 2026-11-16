// Worker-side counterpart to `build-pipeline.js`: owns the actual
// `rustc_opt.wasm` instance and the WASI thread farm's "animal" (a farm
// has exactly one background-worker owner, and the main thread is
// already spoken for running the page), same split the research spike
// found necessary — farm and animal deadlock if colocated.
import { WASIFarmAnimal } from "./vendor/browser_wasi_shim_threads/browser-wasi-shim-threads.es.js";

self.onmessage = async (e) => {
  const { farmRef, rustcWasmBytes, args } = e.data;
  try {
    const rustcModule = await WebAssembly.compile(rustcWasmBytes);

    const animal = new WASIFarmAnimal(
      farmRef,
      args,
      ["RUST_MIN_STACK=16777216", "RUSTC_BOOTSTRAP=1", "TMPDIR=/work"],
      {
        can_thread_spawn: true,
        thread_spawn_worker_url: new URL("./thread_spawn.js", import.meta.url).href,
        thread_spawn_wasm: rustcModule,
      },
    );

    await animal.wait_worker_background_worker();
    // 2000 extra pages (~125MB): the wasm-output toolchain only ever
    // needed 200, but the native-`.exe` one's compiler binary is larger
    // and declares a correspondingly bigger minimum memory import.
    animal.get_share_memory().memory.grow(2000);

    const exitCode = animal.block_start_on_thread();
    postMessage({ done: true, exitCode });
  } catch (err) {
    postMessage({ done: true, crashed: true, error: String(err && err.stack ? err.stack : err) });
  }
};
