// Black-key + edge-feather + dither-shadow resolve for Mir2 map object sprites (trees/decor/walls).
//
// Pass 1 (black key): the legacy art uses a near-black background as a color key rather than
// real alpha. We flood-fill from the image borders inward, keying out connected dark pixels and
// feathering the edge by brightness (solid below SOLID, fully opaque above FEATHER, ramped
// between). Interior dark pixels are preserved because the fill only reaches background-connected
// ones.
//
// Pass 2 (dither shadow): the 8-bit art has no real alpha, so object ground-shadows are baked as
// a ~50% ordered dither — a checkerboard of near-black opaque texels alternating with transparent
// ones. The border flood-fill cannot reach those texels (each is ringed by transparency), so on a
// crisp (image-rendering: pixelated) canvas they show as a hard halftone "screen door" instead of
// the soft shadow Crystal gets from DirectX texture filtering. Pass 2 detects the checkerboard
// geometrically and rewrites it to a uniform translucent black. See the inline comment for why
// this is inert on solid art.
//
// `alphaKeyMapObjectPixels` is intentionally SELF-CONTAINED: it is both imported directly on
// the main thread (the fallback path) AND serialized via `.toString()` into the off-thread
// worker below. It MUST NOT reference any module-scope binding (constants, helpers, imports) —
// everything it needs is declared inside its own body or is a JS global (Math, Uint8Array…).
// If you change the algorithm, both paths update automatically; do not reintroduce closures.
export function alphaKeyMapObjectPixels(
  pixels: Uint8ClampedArray,
  width: number,
  height: number,
): number {
  const SOLID = 18;
  const FEATHER = 72;
  const brightness = (off: number) => Math.max(pixels[off], pixels[off + 1], pixels[off + 2]);
  const queued = new Uint8Array(width * height);
  const stack: number[] = [];
  let changed = 0;

  const enqueue = (x: number, y: number) => {
    if (x < 0 || y < 0 || x >= width || y >= height) {
      return;
    }
    const pixelIndex = y * width + x;
    if (queued[pixelIndex]) {
      return;
    }
    const offset = pixelIndex * 4;
    if (pixels[offset + 3] === 0 || brightness(offset) > FEATHER) {
      return;
    }
    queued[pixelIndex] = 1;
    stack.push(pixelIndex);
  };

  for (let x = 0; x < width; x += 1) {
    enqueue(x, 0);
    enqueue(x, height - 1);
  }
  for (let y = 1; y < height - 1; y += 1) {
    enqueue(0, y);
    enqueue(width - 1, y);
  }

  while (stack.length) {
    const pixelIndex = stack.pop()!;
    const offset = pixelIndex * 4;
    const b = brightness(offset);
    const alpha = pixels[offset + 3];
    const nextAlpha = b <= SOLID ? 0 : Math.round(alpha * ((b - SOLID) / (FEATHER - SOLID)));
    if (nextAlpha < alpha) {
      pixels[offset + 3] = Math.max(0, Math.min(alpha, nextAlpha));
      changed += 1;
    }

    const x = pixelIndex % width;
    const y = Math.floor(pixelIndex / width);
    enqueue(x + 1, y);
    enqueue(x - 1, y);
    enqueue(x, y + 1);
    enqueue(x, y - 1);
  }

  // Pass 2: resolve 1-bit dithered shadows into a uniform translucent shadow.
  //
  // Object ground-shadows are a perfect checkerboard: a near-black opaque texel whose four
  // orthogonal neighbours are ALL transparent (a dither dot), interleaved with transparent
  // texels whose four neighbours are ALL near-black (a dither gap). We rewrite both to one
  // translucent black so the region reads as a smooth ~50% shadow rather than hard dots.
  //
  // Requiring all four neighbours to share the opposite state (and treating the image edge as
  // transparent) is what makes this safe on real art: a solid shape's interior/edge texels —
  // and even 1px-thin dark lines — always keep at least one same-state orthogonal neighbour, so
  // they never satisfy the all-four test and are left untouched. Classification reads the
  // post-pass-1 buffer and is applied in a separate loop, so no in-place edit changes a
  // neighbour's class mid-scan.
  const SHADOW_TEXEL_MAX_LUMA = 48;
  const SHADOW_ALPHA = 120;
  const isShadowTexel = (pixelIndex: number) => {
    const offset = pixelIndex * 4;
    return (
      pixels[offset + 3] !== 0 &&
      pixels[offset] <= SHADOW_TEXEL_MAX_LUMA &&
      pixels[offset + 1] <= SHADOW_TEXEL_MAX_LUMA &&
      pixels[offset + 2] <= SHADOW_TEXEL_MAX_LUMA
    );
  };
  const isClear = (pixelIndex: number) => pixels[pixelIndex * 4 + 3] === 0;
  const shadowTargets = new Uint8Array(width * height);
  for (let y = 0; y < height; y += 1) {
    for (let x = 0; x < width; x += 1) {
      const pixelIndex = y * width + x;
      const upClear = y === 0 || isClear(pixelIndex - width);
      const downClear = y === height - 1 || isClear(pixelIndex + width);
      const leftClear = x === 0 || isClear(pixelIndex - 1);
      const rightClear = x === width - 1 || isClear(pixelIndex + 1);
      if (isShadowTexel(pixelIndex)) {
        if (upClear && downClear && leftClear && rightClear) {
          shadowTargets[pixelIndex] = 1;
        }
      } else if (isClear(pixelIndex)) {
        const upDark = y > 0 && isShadowTexel(pixelIndex - width);
        const downDark = y < height - 1 && isShadowTexel(pixelIndex + width);
        const leftDark = x > 0 && isShadowTexel(pixelIndex - 1);
        const rightDark = x < width - 1 && isShadowTexel(pixelIndex + 1);
        if (upDark && downDark && leftDark && rightDark) {
          shadowTargets[pixelIndex] = 1;
        }
      }
    }
  }
  for (let pixelIndex = 0; pixelIndex < shadowTargets.length; pixelIndex += 1) {
    if (!shadowTargets[pixelIndex]) {
      continue;
    }
    const offset = pixelIndex * 4;
    pixels[offset] = 0;
    pixels[offset + 1] = 0;
    pixels[offset + 2] = 0;
    pixels[offset + 3] = SHADOW_ALPHA;
    changed += 1;
  }

  return changed;
}

// ---------------------------------------------------------------------------
// Off-thread driver: runs the keying in a Web Worker so the per-pixel flood-fill,
// the getImageData readback, and the PNG encode never block the main thread (which
// also drives Bevy's render loop). The worker is built from a Blob URL with the
// algorithm injected via .toString(), so it needs no bundler worker support.
// ---------------------------------------------------------------------------

export type OffThreadAlphaKeyResult = { url: string; bytes: number } | null;

const OFF_THREAD_MAX_FAILURES = 3;
const OFF_THREAD_TIMEOUT_MS = 15_000;

let offThreadSupported: boolean | null = null;
let offThreadFailures = 0;
let workerBlobUrl: string | null = null;
let workerPool: Worker[] | null = null;
let nextWorkerIndex = 0;
let nextRequestId = 1;

type PendingRequest = {
  resolve: (result: OffThreadAlphaKeyResult) => void;
  reject: (error: unknown) => void;
  timer: ReturnType<typeof setTimeout>;
};
const pendingRequests = new Map<number, PendingRequest>();

export function offThreadAlphaKeyAvailable(): boolean {
  if (offThreadSupported !== null) {
    return offThreadSupported && offThreadFailures < OFF_THREAD_MAX_FAILURES;
  }
  offThreadSupported =
    typeof Worker !== "undefined" &&
    typeof OffscreenCanvas !== "undefined" &&
    typeof createImageBitmap === "function" &&
    typeof Blob !== "undefined" &&
    typeof URL !== "undefined" &&
    typeof URL.createObjectURL === "function" &&
    typeof OffscreenCanvas.prototype.convertToBlob === "function";
  return offThreadSupported && offThreadFailures < OFF_THREAD_MAX_FAILURES;
}

function workerSource(): string {
  // `alphaKeyMapObjectPixels` is self-contained, so its .toString() is valid standalone JS.
  return `const alphaKeyMapObjectPixels = ${alphaKeyMapObjectPixels.toString()};
self.onmessage = async (event) => {
  const { id, bitmap, width, height } = event.data;
  try {
    const canvas = new OffscreenCanvas(width, height);
    const ctx = canvas.getContext("2d", { willReadFrequently: true });
    if (!ctx) {
      if (bitmap.close) bitmap.close();
      self.postMessage({ id, blob: null });
      return;
    }
    ctx.drawImage(bitmap, 0, 0, width, height);
    if (bitmap.close) bitmap.close();
    const imageData = ctx.getImageData(0, 0, width, height);
    const changed = alphaKeyMapObjectPixels(imageData.data, width, height);
    if (!changed) {
      self.postMessage({ id, blob: null });
      return;
    }
    ctx.putImageData(imageData, 0, 0);
    const blob = await canvas.convertToBlob({ type: "image/png" });
    self.postMessage({ id, blob });
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
    const { id, blob, error } = (event.data ?? {}) as { id: number; blob?: Blob | null; error?: string };
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
    if (!blob) {
      entry.resolve(null);
      return;
    }
    entry.resolve({ url: URL.createObjectURL(blob), bytes: blob.size });
  };
  worker.onerror = () => {
    // A worker-level error means the (blob) worker script is unusable — every worker in the
    // pool shares the same source, so disable off-thread entirely and fail any in-flight
    // requests immediately so they fall back to main-thread keying instead of waiting out
    // the per-request timeout.
    offThreadFailures = OFF_THREAD_MAX_FAILURES;
    for (const [id, entry] of pendingRequests) {
      pendingRequests.delete(id);
      clearTimeout(entry.timer);
      entry.reject(new Error("alpha key worker error"));
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

export async function keyMapObjectImageOffThread(
  image: HTMLImageElement,
  width: number,
  height: number,
): Promise<OffThreadAlphaKeyResult> {
  if (!offThreadAlphaKeyAvailable()) {
    throw new Error("off-thread alpha key unavailable");
  }
  const bitmap = await createImageBitmap(image);
  const worker = nextWorker();
  const id = nextRequestId++;
  return new Promise<OffThreadAlphaKeyResult>((resolve, reject) => {
    const timer = setTimeout(() => {
      if (pendingRequests.delete(id)) {
        offThreadFailures += 1;
        reject(new Error("off-thread alpha key timeout"));
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
