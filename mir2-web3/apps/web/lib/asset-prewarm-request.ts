type PrewarmRequestOptions = {
  signal?: AbortSignal;
  fetchImpl?: typeof fetch;
  headerTimeoutMs?: number;
  bodyIdleTimeoutMs?: number;
  maxBodyBytes?: number;
};

/** A warmed asset owns a complete body; receiving 200 headers is insufficient. */
export async function fetchPrewarmAsset(url: string, {
  signal,
  fetchImpl = fetch,
  headerTimeoutMs = 10_000,
  bodyIdleTimeoutMs = 15_000,
  maxBodyBytes = 32 * 1024 * 1024,
}: PrewarmRequestOptions = {}): Promise<{ response: Response; body: Uint8Array | null }> {
  let lastError: unknown;
  for (let attempt = 0; attempt < 3; attempt += 1) {
    if (signal?.aborted) throw abortError();
    const controller = new AbortController();
    const abortOwner = () => controller.abort(abortError());
    signal?.addEventListener("abort", abortOwner, { once: true });
    let timer = setTimeout(() => controller.abort(timeoutError()), headerTimeoutMs);
    let reader: ReadableStreamDefaultReader<Uint8Array> | undefined;
    const cancelReader = () => { void reader?.cancel(controller.signal.reason).catch(() => {}); };
    controller.signal.addEventListener("abort", cancelReader, { once: true });
    try {
      const response = await fetchImpl(url, { cache: "force-cache", signal: controller.signal });
      clearTimeout(timer);
      if (!response.ok) {
        void response.body?.cancel().catch(() => {});
        if (attempt === 2) return { response, body: null };
      } else {
        reader = response.body?.getReader();
        const chunks: Uint8Array[] = [];
        let bytes = 0;
        timer = setTimeout(() => controller.abort(timeoutError()), bodyIdleTimeoutMs);
        while (reader) {
          const chunk = await reader.read();
          if (controller.signal.aborted) throw controller.signal.reason;
          if (chunk.done) break;
          if (!chunk.value.byteLength) continue;
          bytes += chunk.value.byteLength;
          if (bytes > maxBodyBytes) throw new RangeError("Prewarm asset exceeds the byte limit");
          chunks.push(chunk.value);
          clearTimeout(timer);
          timer = setTimeout(() => controller.abort(timeoutError()), bodyIdleTimeoutMs);
        }
        if (controller.signal.aborted) throw controller.signal.reason;
        if (bytes === 0) throw new TypeError("Prewarm asset has an empty body");
        const body = new Uint8Array(bytes);
        let offset = 0;
        for (const chunk of chunks) { body.set(chunk, offset); offset += chunk.byteLength; }
        return { response, body };
      }
    } catch (error) {
      if (signal?.aborted) throw abortError();
      if (error instanceof RangeError) throw error;
      lastError = error;
    } finally {
      clearTimeout(timer);
      signal?.removeEventListener("abort", abortOwner);
      controller.signal.removeEventListener("abort", cancelReader);
      if (reader) {
        void reader.cancel().catch(() => {});
        reader.releaseLock();
      }
      controller.abort();
    }
    if (attempt < 2) await retryDelay(150 * (attempt + 1), signal);
  }
  throw lastError ?? new TypeError("Prewarm asset could not be read");
}

function abortError() { return new DOMException("Asset prewarm aborted", "AbortError"); }
function timeoutError() { return new DOMException("Asset prewarm stalled", "TimeoutError"); }

function retryDelay(ms: number, signal?: AbortSignal): Promise<void> {
  return new Promise((resolve, reject) => {
    if (signal?.aborted) { reject(abortError()); return; }
    const abort = () => { clearTimeout(timer); reject(abortError()); };
    const timer = setTimeout(() => {
      signal?.removeEventListener("abort", abort);
      resolve();
    }, ms);
    signal?.addEventListener("abort", abort, { once: true });
  });
}
