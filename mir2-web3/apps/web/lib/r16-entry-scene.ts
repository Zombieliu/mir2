import entryScene from "./generated/r16-entry-scene.json";
import entryFloor from "./generated/r16-entry-floor.json";
import entryVisuals from "./generated/r16-entry-visuals.json";
import { fetchBootstrapResponse, PINNED_BOOTSTRAP_HEADER_TIMEOUT_MS } from "./first-crystal-scene-request";
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
const ENTRY_GRACE_MS = 2_500;
let entryLoad: Promise<SceneBlueprint | null> | null = null;

export type R16EntrySpriteImages = Readonly<Partial<Record<string, string>>>;
export type R16EntryActorAssets = {
  libraries: Readonly<Record<string, Omit<OriginalSceneSpriteLibraryMeta, "frameMap" | "startupFramesOnly">>>;
  images: R16EntrySpriteImages;
};
export const R16_ENTRY_FLOOR_MAP = entryFloor.mapFileName;
export const R16_ENTRY_FLOOR_BOUNDS = entryFloor.bounds;
export const R16_ENTRY_MONSTER_BOUNDS = entryVisuals.monsterBounds;
let floorLoad: Promise<R16EntrySpriteImages | null> | null = null;
type EntryAssetPin = { path: string; bytes: number; sha256: string };
type EntryPlayerGender = keyof typeof entryVisuals.players;
type EntryMonsterAction = "standing" | "walking";
const objectLoads = new Map<string, Promise<R16EntrySpriteImages | null>>();
const actorLoads = new Map<string, Promise<R16EntryActorAssets | null>>();
const transientVisualFailures = new WeakSet<Promise<unknown>>();
const VISUAL_BODY_TIMEOUT_MS = 45_000;
const PLAYER_RECOVERY_DELAY_MS = 1_000;

export function preloadR16EntryScene(pinnedRelease?: string): Promise<SceneBlueprint | null> | null {
  if (pinnedRelease !== entryScene.assetReleaseId || entryScene.schemaVersion !== SCENE_BLUEPRINT_SCHEMA_VERSION ||
      !globalThis.crypto?.subtle) return null;
  entryLoad ??= loadProgressiveEntryJson<SceneBlueprint>(entryScene, "high").catch(() => null).then((blueprint) => {
    if (!blueprint) entryLoad = null;
    return blueprint;
  });
  return entryLoad;
}

export function preloadR16EntryFloor(pinnedRelease?: string): Promise<R16EntrySpriteImages | null> | null {
  if (pinnedRelease !== entryFloor.assetReleaseId || entryFloor.assetReleaseId !== entryScene.assetReleaseId ||
      entryFloor.sceneSha256 !== entryScene.sha256 || !globalThis.crypto?.subtle) return null;
  floorLoad ??= loadProgressiveEntryJson<R16EntrySpriteImages>(entryFloor, "high").catch(() => null).then((images) => {
    if (!images) floorLoad = null;
    return images;
  });
  return floorLoad;
}

async function loadProgressiveEntryJson<T>(
  asset: EntryAssetPin,
  priority?: RequestPriority,
  bodyTimeoutMs?: number,
  cache: RequestCache = "force-cache",
): Promise<T | null> {
  const { response, body } = await fetchBootstrapResponse(
    asset.path,
    { cache, headerTimeoutMs: PINNED_BOOTSTRAP_HEADER_TIMEOUT_MS, bodyByteLimit: asset.bytes, bodyTimeoutMs },
    response => response.arrayBuffer(),
    (path, options) => fetch(path, { ...options, credentials: "omit", redirect: "error", priority }),
  );
  return response.ok && body ? decodePinnedEntryJson<T>(asset, body) : null;
}

function entryVisualsMatch(pinnedRelease?: string) {
  return pinnedRelease === entryVisuals.assetReleaseId && entryVisuals.assetReleaseId === entryScene.assetReleaseId &&
    entryVisuals.sceneSha256 === entryScene.sha256 && entryVisuals.mapFileName === R16_ENTRY_FLOOR_MAP &&
    Boolean(globalThis.crypto?.subtle);
}

function preloadVisual<T>(
  asset: EntryAssetPin,
  loads: Map<string, Promise<T | null>>,
  priority?: RequestPriority,
  cache: RequestCache = "force-cache",
): Promise<T | null> {
  const cached = loads.get(asset.path);
  if (cached) return cached;
  const pending: Promise<T | null> = loadProgressiveEntryJson<T>(asset, priority, VISUAL_BODY_TIMEOUT_MS, cache).catch((error) => {
    if (error instanceof TypeError || (error instanceof DOMException &&
        (error.name === "TimeoutError" || error.name === "AbortError"))) {
      transientVisualFailures.add(pending);
    }
    return null;
  }).then((data) => {
    if (!data) loads.delete(asset.path);
    return data;
  });
  loads.set(asset.path, pending);
  return pending;
}

export function preloadR16EntryObjects(pinnedRelease?: string) {
  if (!entryVisualsMatch(pinnedRelease)) return null;
  return entryVisuals.objectParts.map(part => preloadVisual(part, objectLoads, "low"));
}

export function preloadR16EntryPlayer(pinnedRelease: string | undefined, gender: EntryPlayerGender) {
  if (!entryVisualsMatch(pinnedRelease)) return null;
  return preloadVisual(entryVisuals.players[gender], actorLoads, "high");
}

/** Recover one failed player load in the background without suppressing loose-image fallback. */
export function loadR16EntryPlayerWithRecovery(
  pinnedRelease: string | undefined,
  gender: EntryPlayerGender,
  owner: AbortSignal,
  onSettled: (assets: R16EntryActorAssets | null) => void,
): Promise<void> | null {
  if (owner.aborted) return null;
  const first = preloadR16EntryPlayer(pinnedRelease, gender);
  if (!first) return null;
  return (async () => {
    const assets = await first;
    if (owner.aborted) return;
    onSettled(assets);
    if (assets || !transientVisualFailures.has(first)) return;

    // The initial null releases the ordinary image path immediately. Retry only
    // once after backing off; changing role or leaving the game cancels the observer.
    await new Promise<void>((resolve) => {
      const finish = () => {
        clearTimeout(timer);
        owner.removeEventListener("abort", finish);
        resolve();
      };
      const timer = setTimeout(finish, PLAYER_RECOVERY_DELAY_MS);
      owner.addEventListener("abort", finish, { once: true });
      if (owner.aborted) finish();
    });
    if (owner.aborted) return;
    const recovered = await preloadVisual(entryVisuals.players[gender], actorLoads, "high", "reload");
    if (!owner.aborted && recovered) onSettled(recovered);
  })();
}

export function preloadR16EntryNpcs(pinnedRelease?: string) {
  if (!entryVisualsMatch(pinnedRelease)) return null;
  return preloadVisual(entryVisuals.npcs, actorLoads, "low");
}

export function preloadR16EntryMonsterFrames(pinnedRelease: string | undefined, action: EntryMonsterAction) {
  if (!entryVisualsMatch(pinnedRelease)) return null;
  const asset = action === "standing" ? entryVisuals.monsterStanding : entryVisuals.monsterWalking;
  return preloadVisual(asset, actorLoads, action === "walking" ? "low" : undefined);
}

async function decodePinnedEntryJson<T>(
  asset: { bytes: number; sha256: string },
  bytes: ArrayBuffer,
): Promise<T | null> {
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
  options?: { untilSettled: boolean },
): Promise<SceneBlueprint | null> | null {
  if (owner.aborted || request.assetReleaseId !== entryScene.assetReleaseId ||
      createCrystalSceneBlueprintRequestKey(request) !== entryKey) return null;
  const pending = preloadR16EntryScene(pinnedRelease);
  if (!pending) return null;

  // A slow optional preload must leave the ordinary map route available.
  const wait = options?.untilSettled ? owner : AbortSignal.any([owner, AbortSignal.timeout(ENTRY_GRACE_MS)]);
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
