import { fetchBootstrapResponse } from "./first-crystal-scene-request";

export type SelfSpriteImageSource = { source: string; width: number; height: number };
export type DecodedSelfSpriteImage = { url: string; image: HTMLImageElement; bytes: number; release: () => void };
export class SelfSpriteImageError extends Error {
  constructor(readonly terminal = false) { super("self sprite image unavailable"); }
}

const MAX_PNG_BYTES = 1024 * 1024;
const MAX_DECODED_BYTES = 8 * 1024 * 1024;
const IMAGE_DEADLINE_MS = 30_000;

/** A request belongs to an actor residency entry, never to an animation tick. */
export async function loadSelfSpriteImage(
  frame: SelfSpriteImageSource,
  owner: AbortSignal,
  candidates: readonly string[],
  fetcher: typeof fetch = fetch,
): Promise<DecodedSelfSpriteImage> {
  if (!Number.isSafeInteger(frame.width) || !Number.isSafeInteger(frame.height) ||
      frame.width < 1 || frame.height < 1 || frame.width * frame.height * 4 > MAX_DECODED_BYTES) {
    throw new SelfSpriteImageError(true);
  }
  const deadline = new AbortController();
  const timer = setTimeout(() => deadline.abort(new DOMException("Actor image deadline", "TimeoutError")), IMAGE_DEADLINE_MS);
  const signal = AbortSignal.any([owner, deadline.signal]);
  try {
    for (const url of [...new Set(candidates)].slice(0, 2)) {
      signal.throwIfAborted();
      try {
        const { response, body } = await fetchBootstrapResponse(
          url, { cache: "force-cache", signal, headerTimeoutMs: 8_000,
            bodyTimeoutMs: 24_000, bodyByteLimit: MAX_PNG_BYTES },
          response => response.arrayBuffer(),
          (path, init) => fetcher(path, { ...init, credentials: "omit", redirect: "error", priority: "high" }),
        );
        if (!response.ok || !body) {
          if (response.headers.has("retry-after")) throw new SelfSpriteImageError(true);
          continue;
        }
        verifyPng(body, frame);
        const packed = frame.source.startsWith("data:image/png;");
        const displayUrl = packed ? frame.source : URL.createObjectURL(new Blob([body], { type: "image/png" }));
        let released = false;
        let decoded: HTMLImageElement | undefined;
        const release = () => {
          if (released) return;
          released = true; decoded?.removeAttribute("src");
          if (!packed) URL.revokeObjectURL(displayUrl);
        };
        try {
          decoded = await decodeImage(displayUrl, frame, signal);
          signal.throwIfAborted();
          return { url: displayUrl, image: decoded, bytes: body.byteLength + frame.width * frame.height * 4, release };
        } catch (error) { release(); throw error; }
      } catch (error) {
        signal.throwIfAborted();
        if (error instanceof SelfSpriteImageError && error.terminal) throw error;
      }
    }
    throw new SelfSpriteImageError();
  } finally { clearTimeout(timer); }
}

function verifyPng(bytes: ArrayBuffer, frame: SelfSpriteImageSource) {
  const view = new DataView(bytes);
  if (bytes.byteLength < 24 || view.getUint32(0) !== 0x89504e47 || view.getUint32(4) !== 0x0d0a1a0a ||
      view.getUint32(12) !== 0x49484452 || view.getUint32(16) !== frame.width || view.getUint32(20) !== frame.height) {
    throw new SelfSpriteImageError(true);
  }
}

function decodeImage(url: string, frame: SelfSpriteImageSource, signal: AbortSignal) {
  return new Promise<HTMLImageElement>((resolve, reject) => {
    const image = new Image();
    let settled = false;
    const finish = (error?: unknown) => {
      if (settled) return;
      settled = true; signal.removeEventListener("abort", abort);
      image.onload = null; image.onerror = null;
      if (error) { image.removeAttribute("src"); reject(error); } else resolve(image);
    };
    const abort = () => finish(signal.reason);
    const loaded = async () => {
      try {
        if (typeof image.decode === "function") await image.decode();
        signal.throwIfAborted();
        if (image.naturalWidth !== frame.width || image.naturalHeight !== frame.height) throw new SelfSpriteImageError(true);
        finish();
      } catch (error) { finish(error); }
    };
    signal.addEventListener("abort", abort, { once: true });
    if (signal.aborted) { abort(); return; }
    image.onload = () => { void loaded(); };
    image.onerror = () => finish(new SelfSpriteImageError());
    image.decoding = "async"; image.src = url;
    if (image.complete) {
      if (image.naturalWidth > 0) void loaded(); else finish(new SelfSpriteImageError());
    }
  });
}
