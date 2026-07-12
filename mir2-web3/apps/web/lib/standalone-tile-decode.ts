import {
  decodeImagePixelsOffThread,
  offThreadImageDecodeAvailable,
  type DecodedPixels,
} from "./map-atlas-decode";
import { alphaKeyMapObjectPixels } from "./scene-alpha-key";

export type StandaloneTileDecodeSource = {
  imageKey: string;
  fetchUrl: string;
  alphaKeyMapObject: boolean;
};

export type StandaloneTilePixels = DecodedPixels & {
  imageKey: string;
};

type ResolvedStandaloneTile = {
  source: StandaloneTileDecodeSource;
  pixels: StandaloneTilePixels;
};

type StandaloneTileDecodeJob = {
  source: StandaloneTileDecodeSource;
  promise: Promise<StandaloneTilePixels | null>;
  resolve: (pixels: StandaloneTilePixels | null) => void;
  state: "queued" | "running";
  cancelled: boolean;
  settled: boolean;
  abort?: () => void;
};

const STANDALONE_TILE_DECODE_CONCURRENCY = 6;
const STANDALONE_TILE_RESOLVED_CACHE_LIMIT = 192;
const STANDALONE_TILE_LOAD_TIMEOUT_MS = 15_000;

const resolvedStandaloneTiles = new Map<string, ResolvedStandaloneTile>();
const inFlightStandaloneTiles = new Map<string, StandaloneTileDecodeJob>();
const standaloneTileDecodeQueue: StandaloneTileDecodeJob[] = [];
let activeStandaloneTileDecodes = 0;

export function decodeStandaloneTilePixels(
  source: StandaloneTileDecodeSource,
): Promise<StandaloneTilePixels | null> {
  if (typeof document === "undefined") {
    return Promise.resolve(null);
  }

  const resolved = resolvedStandaloneTiles.get(source.imageKey);
  if (resolved && sameStandaloneTileSource(resolved.source, source)) {
    // Refresh insertion order so the bounded resolved cache behaves as an LRU.
    resolvedStandaloneTiles.delete(source.imageKey);
    resolvedStandaloneTiles.set(source.imageKey, resolved);
    return Promise.resolve(resolved.pixels);
  }
  if (resolved) {
    resolvedStandaloneTiles.delete(source.imageKey);
  }

  const inFlight = inFlightStandaloneTiles.get(source.imageKey);
  if (inFlight && sameStandaloneTileSource(inFlight.source, source)) {
    return inFlight.promise;
  }
  if (inFlight) {
    evictStandaloneTilePixels([source.imageKey]);
  }

  let resolveJob: (pixels: StandaloneTilePixels | null) => void = () => undefined;
  const promise = new Promise<StandaloneTilePixels | null>((resolve) => {
    resolveJob = resolve;
  });
  const job: StandaloneTileDecodeJob = {
    source: { ...source },
    promise,
    resolve: resolveJob,
    state: "queued",
    cancelled: false,
    settled: false,
  };
  inFlightStandaloneTiles.set(source.imageKey, job);
  standaloneTileDecodeQueue.push(job);
  pumpStandaloneTileDecodeQueue();
  return promise;
}

export function evictStandaloneTilePixels(imageKeys: Iterable<string>) {
  for (const imageKey of imageKeys) {
    resolvedStandaloneTiles.delete(imageKey);
    const job = inFlightStandaloneTiles.get(imageKey);
    if (!job) {
      continue;
    }
    inFlightStandaloneTiles.delete(imageKey);
    job.cancelled = true;
    job.abort?.();
    settleStandaloneTileDecodeJob(job, null);
  }
}

function pumpStandaloneTileDecodeQueue() {
  while (
    activeStandaloneTileDecodes < STANDALONE_TILE_DECODE_CONCURRENCY &&
    standaloneTileDecodeQueue.length > 0
  ) {
    const job = standaloneTileDecodeQueue.shift();
    if (
      !job ||
      job.cancelled ||
      job.settled ||
      inFlightStandaloneTiles.get(job.source.imageKey) !== job
    ) {
      continue;
    }

    job.state = "running";
    activeStandaloneTileDecodes += 1;
    void loadStandaloneTilePixels(job)
      .then((decoded) => finishStandaloneTileDecodeJob(job, decoded))
      .catch(() => finishStandaloneTileDecodeJob(job, null));
  }
}

function finishStandaloneTileDecodeJob(
  job: StandaloneTileDecodeJob,
  decoded: DecodedPixels | null,
) {
  if (job.state === "running") {
    activeStandaloneTileDecodes = Math.max(0, activeStandaloneTileDecodes - 1);
  }
  if (inFlightStandaloneTiles.get(job.source.imageKey) === job) {
    inFlightStandaloneTiles.delete(job.source.imageKey);
  }

  let result: StandaloneTilePixels | null = null;
  if (!job.cancelled && decoded) {
    if (job.source.alphaKeyMapObject) {
      alphaKeyMapObjectPixels(
        new Uint8ClampedArray(
          decoded.pixels.buffer,
          decoded.pixels.byteOffset,
          decoded.pixels.byteLength,
        ),
        decoded.width,
        decoded.height,
      );
    }
    result = { imageKey: job.source.imageKey, ...decoded };
    resolvedStandaloneTiles.set(job.source.imageKey, {
      source: job.source,
      pixels: result,
    });
    trimResolvedStandaloneTiles();
  }

  settleStandaloneTileDecodeJob(job, result);
  pumpStandaloneTileDecodeQueue();
}

function settleStandaloneTileDecodeJob(
  job: StandaloneTileDecodeJob,
  result: StandaloneTilePixels | null,
) {
  if (job.settled) {
    return;
  }
  job.settled = true;
  job.resolve(result);
}

function trimResolvedStandaloneTiles() {
  while (resolvedStandaloneTiles.size > STANDALONE_TILE_RESOLVED_CACHE_LIMIT) {
    const oldestKey = resolvedStandaloneTiles.keys().next().value as string | undefined;
    if (!oldestKey) {
      break;
    }
    resolvedStandaloneTiles.delete(oldestKey);
  }
}

function sameStandaloneTileSource(
  left: StandaloneTileDecodeSource,
  right: StandaloneTileDecodeSource,
) {
  return (
    left.imageKey === right.imageKey &&
    left.fetchUrl === right.fetchUrl &&
    left.alphaKeyMapObject === right.alphaKeyMapObject
  );
}

function loadStandaloneTilePixels(job: StandaloneTileDecodeJob): Promise<DecodedPixels | null> {
  return new Promise((resolve) => {
    const image = new Image();
    let finished = false;
    let timeout: number | null = null;
    const finish = (result: DecodedPixels | null) => {
      if (finished) {
        return;
      }
      finished = true;
      if (timeout !== null) {
        window.clearTimeout(timeout);
      }
      image.onload = null;
      image.onerror = null;
      resolve(result);
    };
    timeout = window.setTimeout(() => finish(null), STANDALONE_TILE_LOAD_TIMEOUT_MS);

    job.abort = () => {
      try {
        image.src = "";
      } catch {
        // Resolving null is sufficient when a browser rejects clearing the URL.
      }
      finish(null);
    };
    image.decoding = "async";
    image.crossOrigin = "anonymous";
    image.onload = async () => {
      try {
        const width = image.naturalWidth;
        const height = image.naturalHeight;
        if (job.cancelled || width <= 0 || height <= 0) {
          finish(null);
          return;
        }
        if (offThreadImageDecodeAvailable()) {
          try {
            const decoded = await decodeImagePixelsOffThread(image, width, height);
            if (job.cancelled) {
              finish(null);
              return;
            }
            if (decoded) {
              finish(decoded);
              return;
            }
          } catch {
            // Fall through to main-thread canvas readback.
          }
        }
        if (job.cancelled) {
          finish(null);
          return;
        }
        const canvas = document.createElement("canvas");
        canvas.width = width;
        canvas.height = height;
        const context = canvas.getContext("2d", { willReadFrequently: true });
        if (!context) {
          finish(null);
          return;
        }
        context.drawImage(image, 0, 0, width, height);
        const imageData = context.getImageData(0, 0, width, height);
        finish({
          width,
          height,
          pixels: new Uint8Array(imageData.data.buffer.slice(0)),
        });
      } catch {
        finish(null);
      }
    };
    image.onerror = () => finish(null);
    image.src = job.source.fetchUrl;
  });
}
