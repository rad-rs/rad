import { thread_spawn_on_worker } from "./vendor/browser_wasi_shim_threads/browser-wasi-shim-threads.es.js";

self.onmessage = async (event) => {
  await thread_spawn_on_worker(event.data);
};
