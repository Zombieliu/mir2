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
  takeRecovery,
  ownerSignal,
  onInitializationStarted,
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
  takeRecovery?: () => boolean;
  ownerSignal?: AbortSignal;
  onInitializationStarted?: () => void;
}): Promise<T> {
  if (ownerSignal?.aborted) throw ownerSignal.reason;
  const controller = new AbortController();
  let defaultStarted = false;
  const uncertain = () => Object.assign(new Error("Runtime initializer owner retired after initialization began; fresh page required"), {
    runtimeInitializationUncertain: true,
  });
  const abortOwner = () => controller.abort(defaultStarted ? uncertain() : ownerSignal?.reason);
  ownerSignal?.addEventListener("abort", abortOwner, { once: true });
  const scriptTimeout = setTimeout(() => controller.abort(), timeoutMs);
  let initializationTimeout: ReturnType<typeof setTimeout> | undefined;
  let onAbort: (() => void) | undefined;
  const cancelled = new Promise<never>((_, reject) => {
    onAbort = () => reject(controller.signal.reason);
    controller.signal.addEventListener("abort", onAbort, { once: true });
  });
  // Also cover synchronous import failures which exit before the race is built.
  void cancelled.catch(() => {});
  const recoveryCooldown = () => new Promise<void>((resolve, reject) => {
    if (controller.signal.aborted) { reject(controller.signal.reason); return; }
    const abort = () => { clearTimeout(timer); reject(controller.signal.reason); };
    const timer = setTimeout(() => {
      controller.signal.removeEventListener("abort", abort); resolve();
    }, 1_000);
    controller.signal.addEventListener("abort", abort, { once: true });
  });
  const compile = async () => {
    const sources = [...new Set([wasmUrl, ...wasmFallbackUrls])];
    const attempts = [...sources];
    const transientSources: string[] = [];
    const stalledHeaderSources = new Set<string>();
    let lastError: unknown;
    let groupStart = 0, recoveryTaken = false;
    let rotateAfterSourceFailure = false;
    for (const [index, url] of attempts.entries()) {
      if (controller.signal.aborted) throw controller.signal.reason;
      const groupIndex = index - groupStart;
      const probingSources = sources.length > 1 && groupIndex < sources.length;
      let recoverableTransport = false;
      let phase: RuntimeBinaryAttempt["phase"] = "headers";
      let reason: RuntimeBinaryAttempt["reason"];
      let bodyFailure: unknown;
      let bytes = 0, status: number | undefined;
      const report = (state: RuntimeBinaryAttempt["state"], retryable?: boolean) => {
        try {
          onDownloadAttempt?.({ attempt: index + 1, sourceIndex: sources.indexOf(url),
            retry: recoveryTaken || groupIndex >= sources.length, state, phase, bytes, status, reason, retryable });
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
          // Probe every configured source before spending the final header
          // budget on one fresh read. An unresponsive last mirror must not
          // prevent returning to an earlier transient source.
          const headers = timer(() => expire("headers-timeout", "Runtime binary response headers timed out", "AbortError"),
            probingSources ? Math.min(8_000, timeoutMs) : timeoutMs);
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
          || (reason === "headers-timeout" && !probingSources);
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
        if (retryable && groupIndex < sources.length) transientSources.push(url);
        if (retryable && reason === "headers-timeout") stalledHeaderSources.add(url);
        // A partial body or absent headers can repeat a costly/stalled source.
        // Spend the same one retry on an earlier transient source first.
        // Other zero-byte and HTTP failures retain their existing order.
        // Missing/denied sources never acquire retry authority.
        if (groupIndex === sources.length - 1 && sources.length > 1 && transientSources.length) {
          const partialBody = retryable && (phase as RuntimeBinaryAttempt["phase"]) === "body" && bytes > 0;
          const earlierStalledHeaders = retryable && reason === "headers-timeout"
            ? transientSources.find(source => source !== url && stalledHeaderSources.has(source)) : undefined;
          const retrySource = partialBody ? transientSources[0]
            : earlierStalledHeaders ?? transientSources[transientSources.length - 1];
          rotateAfterSourceFailure = (partialBody || earlierStalledHeaders !== undefined) && retrySource !== url;
          attempts.push(retrySource);
        }
        // The async reader may have reached EOF before the compiler rejects.
        recoverableTransport = retryable && (phase as RuntimeBinaryAttempt["phase"]) !== "compile";
        lastError = failure;
      } finally {
        for (const id of timers) clearTimeout(id);
        controller.signal.removeEventListener("abort", abortAttempt);
        attempt.signal.removeEventListener("abort", cancelReader);
        if (rejectAttempt) attempt.signal.removeEventListener("abort", rejectAttempt);
        attempt.abort(); cancelReader();
      }
      if (index === attempts.length - 1 && !recoveryTaken && recoverableTransport && !controller.signal.aborted) {
        let granted = false;
        try { granted = takeRecovery?.() === true; } catch { /* A failed owner guard cannot authorize more reads. */ }
        if (controller.signal.aborted) throw controller.signal.reason;
        if (granted) {
          recoveryTaken = true;
          await recoveryCooldown();
          groupStart = attempts.length;
          transientSources.length = 0;
          stalledHeaderSources.clear();
          // If the source retry was rotated, do not immediately repeat it
          // again. The authorized group still has the same size and budgets.
          const next = rotateAfterSourceFailure ? (sources.indexOf(url) + 1) % sources.length : 0;
          rotateAfterSourceFailure = false;
          attempts.push(...sources.slice(next), ...sources.slice(0, next));
        }
      }
    }
    throw lastError ?? new TypeError("Failed to fetch runtime binary");
  };
  try {
    const script = Promise.resolve(loadModule()).finally(() => clearTimeout(scriptTimeout));
    const initialize = async () => {
      const [runtime, compiled] = await Promise.all([script, compile()]);
      if (controller.signal.aborted) throw controller.signal.reason;
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
        try { onInitializationStarted?.(); } catch { /* Coordination observers cannot repeat or replace initialization. */ }
        if (controller.signal.aborted) throw controller.signal.reason;
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
    ownerSignal?.removeEventListener("abort", abortOwner);
    controller.abort();
  }
}
