import type { R16EntryActorAssets } from "./r16-entry-scene";

type ActorPart = Readonly<{
  libraryKey: string;
  bytes: number;
  sha256: string;
  load: () => Promise<{ default: unknown }>;
}>;
const object = (value: unknown): value is Record<string, unknown> =>
  typeof value === "object" && value !== null && !Array.isArray(value);

/** Assemble complete verified parts in manifest order; the caller still verifies the original full pack. */
export async function joinPinnedEntryActorParts(
  parts: readonly ActorPart[], signal: AbortSignal,
): Promise<{ default: R16EntryActorAssets }> {
  signal.throwIfAborted();
  if (parts.length < 1 || parts.length > 8 || new Set(parts.map(part => part.libraryKey)).size !== parts.length
    || parts.some(part => !/^Monster\/[0-9]{3}$/.test(part.libraryKey)
      || !Number.isSafeInteger(part.bytes) || part.bytes < 1 || part.bytes > 256 * 1024
      || !/^[a-f0-9]{64}$/.test(part.sha256))) throw new Error("Invalid pinned actor parts.");
  const values: R16EntryActorAssets[] = new Array(parts.length);
  let next = 0, failed = false;
  const consume = async () => {
    while (next < parts.length) {
      signal.throwIfAborted();
      if (failed) return;
      const index = next++, part = parts[index];
      try {
        const { default: payload } = await part.load();
        signal.throwIfAborted();
        const body = new TextEncoder().encode(JSON.stringify(payload) + "\n");
        if (body.byteLength !== part.bytes) throw new Error("Pinned actor part is incomplete.");
        const digest = new Uint8Array(await crypto.subtle.digest("SHA-256", body));
        signal.throwIfAborted();
        if (Array.from(digest, byte => byte.toString(16).padStart(2, "0")).join("") !== part.sha256)
          throw new Error("Pinned actor part digest mismatch.");
        if (!object(payload) || Object.keys(payload).sort().join() !== "images,libraries"
          || !object(payload.libraries) || Object.keys(payload.libraries).join() !== part.libraryKey
          || !object(payload.images)) throw new Error("Invalid pinned actor part shape.");
        const partImages = payload.images;
        const library = payload.libraries[part.libraryKey];
        if (!object(library) || !Array.isArray(library.frames)
          || library.frames.length !== Object.keys(partImages).length
          || library.frames.some(frame => !object(frame) || typeof frame.path !== "string"
            || !frame.path.startsWith("/original-ui/" + part.libraryKey + "/")
            || typeof partImages[frame.path] !== "string"
            || !(partImages[frame.path] as string).startsWith("data:image/png;base64,")))
          throw new Error("Invalid pinned actor frames.");
        values[index] = payload as R16EntryActorAssets;
      } catch (error) {
        failed = true;
        throw error;
      }
    }
  };
  // The parent supplies one deadline for the entire pack, rather than one
  // renewable deadline per part. Retired observations cannot start more parts.
  await Promise.all(Array.from({ length: Math.min(2, parts.length) }, consume));
  signal.throwIfAborted();
  const libraries = values.flatMap(value => Object.entries(value.libraries));
  const images = values.flatMap(value => Object.entries(value.images));
  if (new Set(images.map(([path]) => path)).size !== images.length)
    throw new Error("Duplicate pinned actor images.");
  return { default: { libraries: Object.fromEntries(libraries), images: Object.fromEntries(images) } };
}
