// Off-thread decode of a map-atlas page image to raw RGBA pixels.
//
// WHY: the on-main-thread path (HTMLImageElement -> canvas.drawImage -> getImageData) blocks
// the main thread for ~50-100 ms per large atlas page — a 2048² page is a ~16 MB getImageData
// readback. During a RUN the player crosses into new map regions fast, so freshly-needed atlas
// pages decode back-to-back and each readback is a visible ~80 ms hitch. That is the residual
// "奔跑两步一卡" run stutter: walking is slow enough that the pages a step needs are already
// resident, so it feels fine; running outruns the resident set and each new page's readback
// freezes a frame. (Confirmed with ?perfDiag=1: every ServerPacket handler is <=0.3 ms, yet the
// long-task channel shows recurring ~81 ms tasks during movement — i.e. the hitch is NOT packet
// handling, it is this readback.)
//
// Moving the drawImage + getImageData into a worker via OffscreenCanvas + createImageBitmap keeps
// the readback off the main thread. This mirrors lib/scene-alpha-key.ts's proven worker infra
// (the same createImageBitmap + OffscreenCanvas + getImageData chain it already ships for
// map-object alpha keying) and keeps a main-thread fallback, so a worker-less/failed/disabled
// environment is never worse than today. Escape hatch: ?atlasDecodeWorker=0.

export type DecodedPixels = { width: number; height: number; pixels: Uint8Array };

const OFF_THREAD_MAX_FAILURES = 3;
const OFF_THREAD_TIMEOUT_MS = 15_000;

let offThreadSupported: boolean | null = null;
let offThreadFailures = 0;
let workerBlobUrl: string | null = null;
let workerPool: Worker[] | null = null;
let nextWorkerIndex = 0;
let nextRequestId = 1;

type PendingRequest = {
  resolve: (result: DecodedPixels | null) => void;
  reject: (error: unknown) => void;
  timer: ReturnType<typeof setTimeout>;
};
const pendingRequests = new Map<number, PendingRequest>();

function workerDisabledByFlag(): boolean {
  if (typeof window === "undefined") return false;
  try {
    return new URLSearchParams(window.location.search).get("atlasDecodeWorker") === "0";
  } catch {
    return false;
  }
}

export function offThreadImageDecodeAvailable(): boolean {
  if (offThreadSupported !== null) {
    return offThreadSupported && offThreadFailures < OFF_THREAD_MAX_FAILURES;
  }
  offThreadSupported =
    !workerDisabledByFlag() &&
    typeof Worker !== "undefined" &&
    typeof OffscreenCanvas !== "undefined" &&
    typeof createImageBitmap === "function" &&
    typeof Blob !== "undefined" &&
    typeof URL !== "undefined" &&
    typeof URL.createObjectURL === "function";
  return offThreadSupported && offThreadFailures < OFF_THREAD_MAX_FAILURES;
}

function workerSource(): string {
  // Self-contained: receives a decoded ImageBitmap, draws it into an OffscreenCanvas, reads the
  // RGBA back, and transfers the ArrayBuffer back (zero-copy). drawImage(bitmap)+getImageData on
  // the 2d context yields straight (non-premultiplied) alpha — identical to the main-thread path.
  return `self.onmessage = async (event) => {
  const { id, bitmap, width, height } = event.data;
  try {
    const canvas = new OffscreenCanvas(width, height);
    const ctx = canvas.getContext("2d", { willReadFrequently: true });
    if (!ctx) {
      if (bitmap && bitmap.close) bitmap.close();
      self.postMessage({ id, pixels: null });
      return;
    }
    ctx.drawImage(bitmap, 0, 0, width, height);
    if (bitmap && bitmap.close) bitmap.close();
    const imageData = ctx.getImageData(0, 0, width, height);
    const buffer = imageData.data.buffer;
    self.postMessage({ id, width, height, pixels: buffer }, [buffer]);
  } catch (err) {
    self.postMessage({ id, error: String((err && err.message) || err) });
  }
};`;
}

function buildWorker(): Worker {
  if (!workerBlobUrl) {
    workerBlobUrl = URL.createObjectURL(new Blob([workerSource()], { type: "application/javascript" }));
  }
  const worker = new Worker(workerBlobUrl);
  worker.onmessage = (event: MessageEvent) => {
    const { id, width, height, pixels, error } = (event.data ?? {}) as {
      id: number;
      width?: number;
      height?: number;
      pixels?: ArrayBuffer | null;
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
    if (!pixels || width === undefined || height === undefined) {
      entry.resolve(null);
      return;
    }
    entry.resolve({ width, height, pixels: new Uint8Array(pixels) });
  };
  worker.onerror = () => {
    // A worker-level error means the (blob) worker script is unusable — every worker in the pool
    // shares the same source, so disable off-thread entirely and fail in-flight requests so they
    // fall back to the main-thread readback instead of waiting out the per-request timeout.
    offThreadFailures = OFF_THREAD_MAX_FAILURES;
    for (const [id, entry] of pendingRequests) {
      pendingRequests.delete(id);
      clearTimeout(entry.timer);
      entry.reject(new Error("map atlas decode worker error"));
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

// Decode a loaded image to RGBA pixels on a worker thread. Throws when off-thread decode is
// unavailable/disabled (caller falls back to the main-thread readback); resolves null when the
// worker could not produce pixels (caller also falls back).
export async function decodeImagePixelsOffThread(
  image: HTMLImageElement,
  width: number,
  height: number,
): Promise<DecodedPixels | null> {
  if (!offThreadImageDecodeAvailable()) {
    throw new Error("off-thread image decode unavailable");
  }
  // premultiplyAlpha:"none" + colorSpaceConversion:"none" keep the bytes byte-identical to the
  // main-thread HTMLImageElement->getImageData path (straight alpha, no colour management).
  const bitmap = await createImageBitmap(image, {
    premultiplyAlpha: "none",
    colorSpaceConversion: "none",
  });
  const worker = nextWorker();
  const id = nextRequestId++;
  return new Promise<DecodedPixels | null>((resolve, reject) => {
    const timer = setTimeout(() => {
      if (pendingRequests.delete(id)) {
        offThreadFailures += 1;
        reject(new Error("off-thread image decode timeout"));
      }
    }, OFF_THREAD_TIMEOUT_MS);
    pendingRequests.set(id, { resolve, reject, timer });
    try {
      worker.postMessage({ id, bitmap, width, height }, [bitmap]);
    } catch (err) {
      pendingRequests.delete(id);
      clearTimeout(timer);
      if (typeof bitmap.close === "function") {
        bitmap.close();
      }
      reject(err);
    }
  });
}
