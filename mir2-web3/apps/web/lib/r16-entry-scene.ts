import entryScene from "./generated/r16-entry-scene.json";
import entryFloor from "./generated/r16-entry-floor.json";
import entryVisuals from "./generated/r16-entry-visuals.json";
import type { OriginalSceneSpriteLibraryMeta } from "./original-scene-sprite-meta";
import {
  createCrystalSceneBlueprintRequestKey,
  SCENE_BLUEPRINT_SCHEMA_VERSION,
  type NormalizedCrystalSceneBlueprintRequest,
} from "./scene-blueprint-request";
import type { SceneBlueprint } from "./scene-types";

const entryKey = createCrystalSceneBlueprintRequestKey({
  ...entryScene.request,
  assetReleaseId: entryScene.assetReleaseId,
});
const PREFETCH_LIMIT_MS = 12_000;
const ENTRY_GRACE_MS = 2_500;
let entryLoad: Promise<SceneBlueprint | null> | null = null;

export type R16EntrySpriteImages = Readonly<Partial<Record<string, string>>>;
export type R16EntryActorAssets = {
  libraries: Readonly<Record<string, Omit<OriginalSceneSpriteLibraryMeta, "frameMap" | "startupFramesOnly">>>;
  images: R16EntrySpriteImages;
};
export const R16_ENTRY_FLOOR_MAP = entryFloor.mapFileName;
let floorLoad: Promise<R16EntrySpriteImages | null> | null = null;
let objectLoad: Promise<R16EntrySpriteImages | null> | null = null;
let actorLoad: Promise<R16EntryActorAssets | null> | null = null;

export function preloadR16EntryScene(pinnedRelease?: string): Promise<SceneBlueprint | null> | null {
  if (pinnedRelease !== entryScene.assetReleaseId || entryScene.schemaVersion !== SCENE_BLUEPRINT_SCHEMA_VERSION ||
      !globalThis.crypto?.subtle) return null;
  entryLoad ??= loadPinnedEntryJson<SceneBlueprint>(entryScene).catch(() => null).then((blueprint) => {
    if (!blueprint) entryLoad = null;
    return blueprint;
  });
  return entryLoad;
}

export function preloadR16EntryFloor(pinnedRelease?: string): Promise<R16EntrySpriteImages | null> | null {
  if (pinnedRelease !== entryFloor.assetReleaseId || entryFloor.assetReleaseId !== entryScene.assetReleaseId ||
      entryFloor.sceneSha256 !== entryScene.sha256 || !globalThis.crypto?.subtle) return null;
  floorLoad ??= loadPinnedEntryJson<R16EntrySpriteImages>(entryFloor).catch(() => null).then((images) => {
    if (!images) floorLoad = null;
    return images;
  });
  return floorLoad;
}

function entryVisualsMatch(pinnedRelease?: string) {
  return pinnedRelease === entryVisuals.assetReleaseId && entryVisuals.assetReleaseId === entryScene.assetReleaseId &&
    entryVisuals.sceneSha256 === entryScene.sha256 && entryVisuals.mapFileName === R16_ENTRY_FLOOR_MAP &&
    Boolean(globalThis.crypto?.subtle);
}

export function preloadR16EntryObjects(pinnedRelease?: string) {
  if (!entryVisualsMatch(pinnedRelease)) return null;
  objectLoad ??= loadPinnedEntryJson<R16EntrySpriteImages>(entryVisuals.objects, 25_000, "low").catch(() => null).then((images) => {
    if (!images) objectLoad = null;
    return images;
  });
  return objectLoad;
}

export function preloadR16EntryActors(pinnedRelease?: string) {
  if (!entryVisualsMatch(pinnedRelease)) return null;
  actorLoad ??= loadPinnedEntryJson<R16EntryActorAssets>(entryVisuals.actors, 25_000).catch(() => null).then((assets) => {
    if (!assets) actorLoad = null;
    return assets;
  });
  return actorLoad;
}

async function loadPinnedEntryJson<T>(
  asset: { path: string; bytes: number; sha256: string },
  timeoutMs = PREFETCH_LIMIT_MS,
  priority?: RequestPriority,
): Promise<T | null> {
  const signal = AbortSignal.timeout(timeoutMs);
  const response = await fetch(asset.path, { cache: "force-cache", credentials: "omit", signal, priority });
  if (!response.ok) {
    void response.body?.cancel().catch(() => {});
    return null;
  }
  const bytes = await response.arrayBuffer();
  if (bytes.byteLength !== asset.bytes) return null;
  const digest = new Uint8Array(await crypto.subtle.digest("SHA-256", bytes));
  const sha256 = Array.from(digest, (byte) => byte.toString(16).padStart(2, "0")).join("");
  if (sha256 !== asset.sha256) return null;
  return JSON.parse(new TextDecoder("utf-8", { fatal: true }).decode(bytes)) as T;
}

export function readR16EntryScene(
  request: NormalizedCrystalSceneBlueprintRequest,
  pinnedRelease: string | undefined,
  owner: AbortSignal,
): Promise<SceneBlueprint | null> | null {
  if (owner.aborted || request.assetReleaseId !== entryScene.assetReleaseId ||
      createCrystalSceneBlueprintRequestKey(request) !== entryKey) return null;
  const pending = preloadR16EntryScene(pinnedRelease);
  if (!pending) return null;

  // A slow optional preload must leave the ordinary map route available.
  const wait = AbortSignal.any([owner, AbortSignal.timeout(ENTRY_GRACE_MS)]);
  if (wait.aborted) return null;
  return new Promise((resolve) => {
    const stop = () => resolve(null);
    wait.addEventListener("abort", stop, { once: true });
    void pending.then((blueprint) => {
      wait.removeEventListener("abort", stop);
      resolve(wait.aborted ? null : blueprint);
    });
  });
}
