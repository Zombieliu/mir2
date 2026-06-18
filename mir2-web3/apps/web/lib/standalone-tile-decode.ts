// Off-main-thread PNG -> RGBA decoder for the Bevy "standalone" (atlas-miss) map tiles.
//
// The Bevy map renderer only batches tiles whose source PNG lives in one of the prebuilt
// library atlases. Tiles that miss every atlas ("standalone" tiles) must be decoded
// individually before they can be uploaded as one-off textures. That decode — fetch +
// ImageBitmap draw + getImageData readback — is what this module moves off the main thread
// (which also drives Bevy's render loop) so a burst of atlas-miss tiles never stalls a frame.
//
// `decodeBitmapToRgba` is intentionally SELF-CONTAINED: it is serialized via `.toString()`
// into the off-thread worker below. It MUST NOT reference any module-scope binding (constants,
// helpers, imports) — everything it needs is declared inside its own body or is a JS global
// (OffscreenCanvas, Uint8Array…). If you change the decode, the worker updates automatically;
// do not reintroduce closures.

export type StandaloneTilePixels = { width: number; height: number; pixels: Uint8Array };

// ---------------------------------------------------------------------------
// Off-thread driver: runs the ImageBitmap draw + getImageData readback in a Web Worker so the
// per-tile decode never blocks the main thread. The worker is built from a Blob URL with the
// decode injected via .toString(), so it needs no bundler worker support. The fetch +
// createImageBitmap stays on the main thread (createImageBitmap is cheap and decodes off-thread
// internally), then the ImageBitmap is *transferred* into the worker so no pixel copy happens.
// ---------------------------------------------------------------------------

const OFF_THREAD_MAX_FAILURES = 3;
const OFF_THREAD_TIMEOUT_MS = 15_000;

let offThreadSupported: boolean | null = null;
let offThreadFailures = 0;
let workerBlobUrl: string | null = null;
let workerPool: Worker[] | null = null;
let nextWorkerIndex = 0;
let nextRequestId = 1;

type PendingRequest = {
  resolve: (result: StandaloneTilePixels | null) => void;
  reject: (error: unknown) => void;
  timer: ReturnType<typeof setTimeout>;
};
const pendingRequests = new Map<number, PendingRequest>();

// Each URL decodes at most once: the in-flight (and resolved) promise is memoized so concurrent
// callers for the same tile share a single fetch + worker round-trip. Mirrors `mapImagePixelCache`.
const mapImagePixelCache = new Map<string, Promise<StandaloneTilePixels | null>>();

export function offThreadStandaloneTileDecodeAvailable(): boolean {
  if (offThreadSupported !== null) {
    return offThreadSupported && offThreadFailures < OFF_THREAD_MAX_FAILURES;
  }
  offThreadSupported =
    typeof window !== "undefined" &&
    typeof Worker !== "undefined" &&
    typeof OffscreenCanvas !== "undefined" &&
    typeof createImageBitmap === "function" &&
    typeof fetch === "function" &&
    typeof Blob !== "undefined" &&
    typeof URL !== "undefined" &&
    typeof URL.createObjectURL === "function";
  return offThreadSupported && offThreadFailures < OFF_THREAD_MAX_FAILURES;
}

function workerSource(): string {
  // `decodeBitmapToRgba` is self-contained, so its .toString() is valid standalone JS.
  return `const decodeBitmapToRgba = ${decodeBitmapToRgba.toString()};
self.onmessage = (event) => {
  const { id, bitmap } = event.data;
  try {
    const result = decodeBitmapToRgba(bitmap);
    if (bitmap.close) bitmap.close();
    if (!result) {
      self.postMessage({ id, width: 0, height: 0, buffer: null });
      return;
    }
    self.postMessage(
      { id, width: result.width, height: result.height, buffer: result.buffer },
      [result.buffer],
    );
  } catch (err) {
    if (bitmap.close) bitmap.close();
    self.postMessage({ id, error: String((err && err.message) || err) });
  }
};`;
}

// Self-contained: draws a transferred ImageBitmap onto an OffscreenCanvas and reads back the
// RGBA bytes. Returns the pixel data over a detached ArrayBuffer so it can be transferred back
// to the main thread with zero copy. MUST stay closure-free (see the module header note).
function decodeBitmapToRgba(
  bitmap: ImageBitmap,
): { width: number; height: number; buffer: ArrayBuffer } | null {
  const width = bitmap.width;
  const height = bitmap.height;
  if (width <= 0 || height <= 0) {
    return null;
  }
  const canvas = new OffscreenCanvas(width, height);
  const ctx = canvas.getContext("2d", { willReadFrequently: true });
  if (!ctx) {
    return null;
  }
  ctx.drawImage(bitmap, 0, 0, width, height);
  const imageData = ctx.getImageData(0, 0, width, height);
  // imageData.data is a Uint8ClampedArray view over a fresh ArrayBuffer; hand that buffer back
  // so the main thread can wrap it as a Uint8Array without re-copying the pixels.
  return { width, height, buffer: imageData.data.buffer };
}

function buildWorker(): Worker {
  if (!workerBlobUrl) {
    workerBlobUrl = URL.createObjectURL(new Blob([workerSource()], { type: "application/javascript" }));
  }
  const worker = new Worker(workerBlobUrl);
  worker.onmessage = (event: MessageEvent) => {
    const { id, width, height, buffer, error } = (event.data ?? {}) as {
      id: number;
      width?: number;
      height?: number;
      buffer?: ArrayBuffer | null;
      error?: string;
    };
    const entry = pendingRequests.get(id);
    if (!entry) {
      return;
    }
    pendingRequests.delete(id);
    clearTimeout(entry.timer);
    if (error) {
      offThreadFailures += 1;
      entry.reject(new Error(error));
      return;
    }
    if (!buffer || !width || !height) {
      entry.resolve(null);
      return;
    }
    entry.resolve({ width, height, pixels: new Uint8Array(buffer) });
  };
  worker.onerror = () => {
    // A worker-level error means the (blob) worker script is unusable — every worker in the
    // pool shares the same source, so disable off-thread entirely and fail any in-flight
    // requests immediately so they fall back to main-thread decoding instead of waiting out
    // the per-request timeout.
    offThreadFailures = OFF_THREAD_MAX_FAILURES;
    for (const [id, entry] of pendingRequests) {
      pendingRequests.delete(id);
      clearTimeout(entry.timer);
      entry.reject(new Error("standalone tile decode worker error"));
    }
  };
  return worker;
}

function nextWorker(): Worker {
  if (!workerPool) {
    const cores = globalThis.navigator?.hardwareConcurrency ?? 4;
    const size = Math.max(1, Math.min(3, cores - 1));
    workerPool = Array.from({ length: size }, () => buildWorker());
  }
  const worker = workerPool[nextWorkerIndex % workerPool.length];
  nextWorkerIndex += 1;
  return worker;
}

async function decodeStandaloneTileUncached(url: string): Promise<StandaloneTilePixels | null> {
  if (!offThreadStandaloneTileDecodeAvailable()) {
    return null;
  }
  let bitmap: ImageBitmap;
  try {
    const response = await fetch(url);
    if (!response.ok) {
      return null;
    }
    bitmap = await createImageBitmap(await response.blob());
  } catch {
    return null;
  }
  const worker = nextWorker();
  const id = nextRequestId++;
  try {
    return await new Promise<StandaloneTilePixels | null>((resolve, reject) => {
      const timer = setTimeout(() => {
        if (pendingRequests.delete(id)) {
          offThreadFailures += 1;
          // Graceful: a timed-out decode resolves null (caller falls back to a main-thread
          // decode) rather than rejecting — but still counts toward the failure ceiling.
          resolve(null);
        }
      }, OFF_THREAD_TIMEOUT_MS);
      pendingRequests.set(id, { resolve, reject, timer });
      try {
        worker.postMessage({ id, bitmap }, [bitmap]);
      } catch (err) {
        pendingRequests.delete(id);
        clearTimeout(timer);
        if (typeof bitmap.close === "function") {
          bitmap.close();
        }
        reject(err);
      }
    });
  } catch {
    // Any worker/transfer failure degrades to null so the caller decodes on the main thread.
    return null;
  }
}

export function decodeStandaloneTileOffThread(url: string): Promise<StandaloneTilePixels | null> {
  const inFlight = mapImagePixelCache.get(url);
  if (inFlight) {
    return inFlight;
  }
  // Dedup only CONCURRENT requests for the same URL — evict on settle so the
  // decoded RGBA isn't retained here (the caller, original-client-shell.tsx, owns
  // the LRU-bounded `decodedMapTilesRef`). Retaining every decoded tile's pixels in
  // this module-scope map would leak unboundedly over a long cross-map walk.
  const pending = decodeStandaloneTileUncached(url).finally(() => {
    mapImagePixelCache.delete(url);
  });
  mapImagePixelCache.set(url, pending);
  return pending;
}
