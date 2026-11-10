import entryScene from "./generated/r16-entry-scene.json";
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

export function preloadR16EntryScene(pinnedRelease?: string): Promise<SceneBlueprint | null> | null {
  if (pinnedRelease !== entryScene.assetReleaseId || entryScene.schemaVersion !== SCENE_BLUEPRINT_SCHEMA_VERSION ||
      !globalThis.crypto?.subtle) return null;
  entryLoad ??= loadEntry().catch(() => null).then((blueprint) => {
    if (!blueprint) entryLoad = null;
    return blueprint;
  });
  return entryLoad;
}

async function loadEntry(): Promise<SceneBlueprint | null> {
  const signal = AbortSignal.timeout(PREFETCH_LIMIT_MS);
  const response = await fetch(entryScene.path, { cache: "force-cache", credentials: "omit", signal });
  if (!response.ok) {
    void response.body?.cancel().catch(() => {});
    return null;
  }
  const bytes = await response.arrayBuffer();
  if (bytes.byteLength !== entryScene.bytes) return null;
  const digest = new Uint8Array(await crypto.subtle.digest("SHA-256", bytes));
  const sha256 = Array.from(digest, (byte) => byte.toString(16).padStart(2, "0")).join("");
  if (sha256 !== entryScene.sha256) return null;
  return JSON.parse(new TextDecoder("utf-8", { fatal: true }).decode(bytes)) as SceneBlueprint;
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
