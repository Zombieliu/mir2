type SceneReadOptions = {
  cache: RequestCache;
  signal?: AbortSignal;
};

type SceneJsonResult<T> = {
  response: Response;
  body: T | null;
};

export type FirstCrystalSceneJsonReader = (
  url: string,
  options: SceneReadOptions,
  fetcher: typeof fetch,
) => Promise<SceneJsonResult<unknown>>;

const READ_IDLE_TIMEOUT_MS = 8_000;
const READ_BODY_LIMIT_MS = 24_000;
const RETRY_DELAY_MS = 200;
const TRANSIENT_STATUS = new Set([408, 425, 429, 500, 502, 503, 504]);

async function readAttempt<T>(
  url: string,
  { cache, signal: owner }: SceneReadOptions,
  fetcher: typeof fetch,
): Promise<SceneJsonResult<T>> {
  owner?.throwIfAborted();
  const deadline = new AbortController();
  const signal = owner ? AbortSignal.any([owner, deadline.signal]) : deadline.signal;
  const abortForTimeout = () => {
    deadline.abort(new DOMException("Scene response deadline elapsed", "TimeoutError"));
  };
  let idleTimer = setTimeout(abortForTimeout, READ_IDLE_TIMEOUT_MS);
  let bodyTimer: ReturnType<typeof setTimeout> | undefined;
  const restartIdleTimer = () => {
    clearTimeout(idleTimer);
    idleTimer = setTimeout(abortForTimeout, READ_IDLE_TIMEOUT_MS);
  };

  let onAbort: (() => void) | undefined;
  const aborted = new Promise<never>((_, reject) => {
    onAbort = () => reject(signal.reason);
    signal.addEventListener("abort", onAbort, { once: true });
  });
  async function read(): Promise<SceneJsonResult<T>> {
    const response = await fetcher(url, { cache, signal });
    signal.throwIfAborted();
    if (!response.ok && response.status !== 424) {
      // Cancelling a tee can wait on another reader. Its cleanup must not
      // postpone retrying a definitive HTTP status.
      void response.body?.cancel().catch(() => {});
      return { response, body: null };
    }
    // Keep reading while real bytes arrive. A trickle must still finish within
    // the body limit, and JSON is published only after the stream has ended.
    restartIdleTimer();
    bodyTimer = setTimeout(abortForTimeout, READ_BODY_LIMIT_MS);
    const stream = response.body?.pipeThrough(new TransformStream<Uint8Array, Uint8Array>({
      transform(chunk, output) {
        if (chunk.byteLength > 0 && !signal.aborted) restartIdleTimer();
        output.enqueue(chunk);
      },
    }), { signal });
    const body = (await (stream ? new Response(stream) : response).json()) as T;
    signal.throwIfAborted();
    return { response, body };
  }

  try {
    const result = await Promise.race([read(), aborted]);
    signal.throwIfAborted();
    return result;
  } finally {
    clearTimeout(idleTimer);
    clearTimeout(bodyTimer);
    if (onAbort) signal.removeEventListener("abort", onAbort);
  }
}

async function waitToRetry(signal?: AbortSignal): Promise<void> {
  signal?.throwIfAborted();
  let onAbort: (() => void) | undefined;
  try {
    await new Promise<void>((resolve, reject) => {
      const timer = setTimeout(resolve, RETRY_DELAY_MS);
      onAbort = () => {
        clearTimeout(timer);
        reject(signal?.reason);
      };
      signal?.addEventListener("abort", onAbort, { once: true });
    });
  } finally {
    if (onAbort) signal?.removeEventListener("abort", onAbort);
  }
}

function isRetryableFailure(error: unknown): boolean {
  return error instanceof TypeError ||
    (error instanceof DOMException && (error.name === "TimeoutError" || error.name === "AbortError"));
}

/** Bound the wait for headers, a stalled body, and the entire body separately. */
export async function fetchFirstCrystalSceneJson<T = unknown>(
  url: string,
  options: SceneReadOptions,
  fetcher: typeof fetch = fetch,
): Promise<SceneJsonResult<T>> {
  try {
    const result = await readAttempt<T>(url, options, fetcher);
    if (!TRANSIENT_STATUS.has(result.response.status) || result.response.headers.has("retry-after")) {
      return result;
    }
  } catch (error) {
    options.signal?.throwIfAborted();
    if (!isRetryableFailure(error)) throw error;
  }

  await waitToRetry(options.signal);
  // Chromium can reattach a force-cache retry to an unfinished HTTP body.
  // Reload the same canonical URL so the retry gets an independent read.
  return readAttempt<T>(url, { ...options, cache: "reload" }, fetcher);
}
