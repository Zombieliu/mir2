type RuntimeInitializer = {
  default?: (input: { module_or_path: WebAssembly.Module }) => Promise<unknown>;
};

export type RuntimeBinaryAttempt = {
  attempt: number;
  sourceIndex: number;
  retry: boolean;
  state: "started" | "completed" | "failed";
  phase: "headers" | "body" | "compile";
  bytes: number;
  status?: number;
  reason?: "headers-timeout" | "body-idle" | "body-timeout" | "consumer-timeout"
    | "http" | "network" | "invalid-binary" | "byte-limit" | "cancelled" | "other";
  retryable?: boolean;
};

/** Stream one pinned binary at a time while importing its glue, then initialize at the canvas. */
export async function initializeBevyRuntime<T extends RuntimeInitializer>({
  loadModule,
  wasmUrl,
  wasmFallbackUrls = [],
  waitForCanvas,
  timeoutMs,
  bodyIdleTimeoutMs = 15_000,
  bodyTimeoutMs = 600_000,
  maxBodyBytes = 32 * 1024 * 1024,
  fetchImpl = fetch,
  onDownloadAttempt,
}: {
  loadModule: () => Promise<T>;
  wasmUrl: string;
  wasmFallbackUrls?: readonly string[];
  waitForCanvas: () => Promise<void>;
  timeoutMs: number;
  bodyIdleTimeoutMs?: number;
  bodyTimeoutMs?: number;
  maxBodyBytes?: number;
  fetchImpl?: typeof fetch;
  onDownloadAttempt?: (attempt: RuntimeBinaryAttempt) => void;
}): Promise<T> {
  const controller = new AbortController();
  const scriptTimeout = setTimeout(() => controller.abort(), timeoutMs);
  let initializationTimeout: ReturnType<typeof setTimeout> | undefined;
  let onAbort: (() => void) | undefined;
  const cancelled = new Promise<never>((_, reject) => {
    onAbort = () => reject(controller.signal.reason);
    controller.signal.addEventListener("abort", onAbort, { once: true });
  });
  const compile = async () => {
    const sources = [...new Set([wasmUrl, ...wasmFallbackUrls])];
    const attempts = [...sources];
    let retrySource: string | undefined;
    let lastError: unknown;
    for (const [index, url] of attempts.entries()) {
      if (controller.signal.aborted) throw controller.signal.reason;
      let phase: RuntimeBinaryAttempt["phase"] = "headers";
      let reason: RuntimeBinaryAttempt["reason"];
      let bodyFailure: unknown;
      let bytes = 0, status: number | undefined;
      const report = (state: RuntimeBinaryAttempt["state"], retryable?: boolean) => {
        try {
          onDownloadAttempt?.({ attempt: index + 1, sourceIndex: sources.indexOf(url),
            retry: index >= sources.length, state, phase, bytes, status, reason, retryable });
        } catch { /* Diagnostics must not change download or startup ownership. */ }
      };
      const attempt = new AbortController();
      const abortAttempt = () => attempt.abort(controller.signal.reason);
      controller.signal.addEventListener("abort", abortAttempt, { once: true });
      const timers = new Set<ReturnType<typeof setTimeout>>();
      const timer = (callback: () => void, ms: number) => {
        const id = setTimeout(callback, ms); timers.add(id); return id;
      };
      const stop = (id: ReturnType<typeof setTimeout> | undefined) => {
        if (id !== undefined) { clearTimeout(id); timers.delete(id); }
      };
      const expire = (kind: NonNullable<RuntimeBinaryAttempt["reason"]>, message: string, name = "TimeoutError") => {
        reason = kind;
        attempt.abort(new DOMException(message, name));
      };
      let reader: ReadableStreamDefaultReader<Uint8Array> | undefined;
      const cancelReader = () => { void reader?.cancel(attempt.signal.reason).catch(() => {}); };
      attempt.signal.addEventListener("abort", cancelReader, { once: true });
      let rejectAttempt: (() => void) | undefined;
      const cancelledAttempt = new Promise<never>((_, reject) => {
        rejectAttempt = () => reject(attempt.signal.reason);
        attempt.signal.addEventListener("abort", rejectAttempt, { once: true });
      });
      try {
        report("started");
        const consume = async () => {
          // A configured alternate should not wait behind an unresponsive edge.
          // The final source retains the original response-header budget.
          const headers = timer(() => expire("headers-timeout", "Runtime binary response headers timed out", "AbortError"),
            index < sources.length - 1 ? Math.min(8_000, timeoutMs) : timeoutMs);
          const response = await fetchImpl(new Request(url, { signal: attempt.signal }));
          stop(headers);
          status = response.status;
          if (attempt.signal.aborted) {
            void response.body?.cancel().catch(() => {});
            throw attempt.signal.reason;
          }
          if (!response.ok) {
            reason = "http";
            void response.body?.cancel().catch(() => {});
            throw new TypeError(`Failed to fetch runtime binary (HTTP ${response.status})`);
          }
          phase = "body";
          reader = response.body?.getReader();
          if (!reader) throw new TypeError("Runtime binary has no body");
          const bodyDeadline = timer(() => expire("body-timeout", "Runtime binary body exceeded its time limit"), bodyTimeoutMs);
          let cpu: ReturnType<typeof setTimeout> | undefined;
          const watchConsumer = () => { stop(cpu); cpu = timer(() => expire("consumer-timeout", "Runtime binary compiler stalled", "AbortError"), timeoutMs); };
          watchConsumer();
          const watched = new ReadableStream<Uint8Array>({
            async pull(output) {
              // Compiler backpressure is not a network stall. Measure idle only
              // while a source read is pending, until a positive chunk or EOF.
              stop(cpu);
              const idle = timer(() => expire("body-idle", "Runtime binary body stalled"), bodyIdleTimeoutMs);
              try {
                while (true) {
                  const chunk = await reader!.read();
                  if (attempt.signal.aborted) throw attempt.signal.reason;
                  if (chunk.done) {
                    if (bytes === 0) throw new TypeError("Runtime binary has an empty body");
                    phase = "compile";
                    stop(bodyDeadline); watchConsumer(); output.close(); return;
                  }
                  if (!chunk.value.byteLength) continue;
                  bytes += chunk.value.byteLength;
                  if (bytes > maxBodyBytes) throw new RangeError("Runtime binary exceeds the decoded byte limit");
                  watchConsumer(); output.enqueue(chunk.value); return;
                }
              } catch (error) {
                // Some browsers wrap a streamed RangeError in a network
                // TypeError. Retain the reader's cause so bounds stay terminal.
                bodyFailure = error;
                output.error(error);
              }
              finally { stop(idle); }
            },
            cancel(reason) { attempt.abort(reason); },
          });
          const body = new Response(watched, { headers: response.headers });
          if (typeof WebAssembly.compileStreaming === "function" && response.headers.get("content-type") === "application/wasm") {
            return WebAssembly.compileStreaming(body);
          }
          // Compatibility consumes the same monitored stream, never a second GET.
          return WebAssembly.compile(await body.arrayBuffer());
        };
        const compiled = await Promise.race([consume(), cancelledAttempt]);
        report("completed");
        return compiled;
      } catch (error) {
        const failure = bodyFailure ?? error;
        const invalid = typeof WebAssembly.CompileError === "function" && failure instanceof WebAssembly.CompileError;
        const terminal = failure instanceof RangeError || invalid || reason === "consumer-timeout" || reason === "body-timeout"
          || (reason === "headers-timeout" && index >= sources.length - 1);
        const network = failure instanceof TypeError && /failed to fetch|fetch failed|network|terminated|aborted|load failed|connection/i.test(failure.message);
        const retryable = !controller.signal.aborted && !terminal && (reason === "body-idle"
          || reason === "headers-timeout" || (reason === "http"
            ? status === 408 || status === 429 || (status !== undefined && status >= 500 && status <= 599)
            : network));
        if (controller.signal.aborted) reason = "cancelled";
        else if (failure instanceof RangeError) reason = "byte-limit";
        else if (invalid) reason = "invalid-binary";
        else if (!reason) reason = network ? "network" : "other";
        report("failed", retryable);
        if (controller.signal.aborted) throw controller.signal.reason;
        if (terminal) throw failure;
        if (retryable && index < sources.length) retrySource = url;
        // Once the configured mirrors are exhausted, retry only the last
        // transient source once. All retries remain before glue initialization;
        // corrupt bytes and expired terminal phase budgets never get another GET.
        if (index === sources.length - 1 && sources.length > 1 && retrySource) attempts.push(retrySource);
        lastError = failure;
      } finally {
        for (const id of timers) clearTimeout(id);
        controller.signal.removeEventListener("abort", abortAttempt);
        attempt.signal.removeEventListener("abort", cancelReader);
        if (rejectAttempt) attempt.signal.removeEventListener("abort", rejectAttempt);
        attempt.abort(); cancelReader();
      }
    }
    throw lastError ?? new TypeError("Failed to fetch runtime binary");
  };
  try {
    const script = Promise.resolve(loadModule()).finally(() => clearTimeout(scriptTimeout));
    const initialize = async () => {
      const [runtime, compiled] = await Promise.all([script, compile()]);
      if (controller.signal.aborted) throw controller.signal.reason;
      let defaultStarted = false;
      initializationTimeout = setTimeout(() => {
        if (!defaultStarted) { controller.abort(); return; }
        // wasm-bindgen initialization cannot be canceled after entry. A late
        // completion must never race another initialization of the same glue.
        const error = Object.assign(new Error("Runtime initializer did not finish; fresh page required"), {
          runtimeInitializationUncertain: true,
        });
        controller.abort(error);
      }, timeoutMs);
      if (typeof runtime.default === "function") {
        await waitForCanvas();
        if (controller.signal.aborted) throw controller.signal.reason;
        defaultStarted = true;
        await runtime.default({ module_or_path: compiled });
      }
      if (controller.signal.aborted) throw controller.signal.reason;
      return runtime;
    };
    return await Promise.race([initialize(), cancelled]);
  } finally {
    clearTimeout(scriptTimeout);
    if (initializationTimeout !== undefined) clearTimeout(initializationTimeout);
    if (onAbort) controller.signal.removeEventListener("abort", onAbort);
    controller.abort();
  }
}
