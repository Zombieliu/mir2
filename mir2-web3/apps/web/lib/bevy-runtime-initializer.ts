type RuntimeInitializer = {
  default?: (input: { module_or_path: WebAssembly.Module }) => Promise<unknown>;
};

/** Fetch/compile the binary while importing its glue, then initialize once the canvas exists. */
export async function initializeBevyRuntime<T extends RuntimeInitializer>({
  loadModule,
  wasmUrl,
  waitForCanvas,
  timeoutMs,
  fetchImpl = fetch,
}: {
  loadModule: () => Promise<T>;
  wasmUrl: string;
  waitForCanvas: () => Promise<void>;
  timeoutMs: number;
  fetchImpl?: typeof fetch;
}): Promise<T> {
  const controller = new AbortController();
  const timeout = setTimeout(() => controller.abort(), timeoutMs);
  let onAbort: (() => void) | undefined;
  const cancelled = new Promise<never>((_, reject) => {
    onAbort = () => reject(controller.signal.reason);
    controller.signal.addEventListener("abort", onAbort, { once: true });
  });
  const compile = async () => {
    const response = await fetchImpl(new Request(wasmUrl, { signal: controller.signal }));
    if (!response.ok) {
      void response.body?.cancel().catch(() => {});
      throw new TypeError(`Failed to fetch runtime binary (HTTP ${response.status})`);
    }
    if (typeof WebAssembly.compileStreaming === "function" && response.headers.get("content-type") === "application/wasm") {
      return WebAssembly.compileStreaming(response);
    }
    // Match wasm-bindgen's compatibility path for a host with a wrong MIME type
    // or an older browser, without making another network request.
    return WebAssembly.compile(await response.arrayBuffer());
  };
  try {
    // Aborting fetch alone cannot reject an uncancellable import that remains
    // pending after the binary has already compiled. Retire that whole attempt;
    // a late import must never reach the canvas or initialize the expired boot.
    const [runtime, compiled] = await Promise.race([Promise.all([loadModule(), compile()]), cancelled]);
    if (typeof runtime.default === "function") {
      await waitForCanvas();
      await runtime.default({ module_or_path: compiled });
    }
    return runtime;
  } finally {
    clearTimeout(timeout);
    if (onAbort) controller.signal.removeEventListener("abort", onAbort);
    // Also cancel an in-progress binary if the independent script import fails.
    controller.abort();
  }
}
