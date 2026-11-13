type BootstrapChunkPin = Readonly<{ bytes: number; sha256: string }>;
export type BootstrapChunkFormat = "json" | "json-line" | "pretty-json-line";
const CHUNK_OBSERVER_TIMEOUT_MS = 20_000;

/** One shared, finite observation of an immutable compiled JSON module.
 * Timing out cannot cancel webpack's shared script request. A late module may
 * enter its module cache, but cannot resolve this rejected observation. */
export function createPinnedBootstrapChunk<T>(
  pin: BootstrapChunkPin,
  load: () => Promise<{ default: unknown }>,
  format: BootstrapChunkFormat,
): () => Promise<T> {
  let observed: Promise<T> | null = null;
  return () => {
    if (observed) return observed;
    let timer: ReturnType<typeof setTimeout>;
    const deadline = new Promise<never>((_, reject) => {
      timer = setTimeout(() => reject(new DOMException("Pinned bootstrap chunk wait elapsed", "TimeoutError")),
        CHUNK_OBSERVER_TIMEOUT_MS);
    });
    const read = Promise.resolve().then(load).then(async ({ default: payload }) => {
      const text = format === "pretty-json-line" ? JSON.stringify(payload, null, 2) + "\n"
        : JSON.stringify(payload) + (format === "json-line" ? "\n" : "");
      const body = new TextEncoder().encode(text);
      if (body.byteLength !== pin.bytes) throw new Error("Pinned bootstrap chunk response is incomplete.");
      const digest = new Uint8Array(await crypto.subtle.digest("SHA-256", body));
      const sha256 = Array.from(digest, byte => byte.toString(16).padStart(2, "0")).join("");
      if (sha256 !== pin.sha256) throw new Error("Pinned bootstrap chunk digest mismatch.");
      // Publish only a verified JSON value, using the same parse boundary as HTTP.
      return JSON.parse(text) as T;
    });
    observed = Promise.race([read, deadline]).finally(() => clearTimeout(timer));
    return observed;
  };
}
