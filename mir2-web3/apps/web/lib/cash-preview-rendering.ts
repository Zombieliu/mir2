import type { CashPreviewLayerDocument } from "./client-core-runtime";
import type { OriginalSceneSpriteLibraryMeta } from "./original-scene-sprite-meta";

export type { CashPreviewLayerDocument } from "./client-core-runtime";
export type CashPreviewDrawLayer = Readonly<{
  library: string; frame: number; path: string;
  left: number; top: number; width: number; height: number;
}>;
export type CashPreviewDrawing = Readonly<{
  complete: boolean; layers: readonly CashPreviewDrawLayer[];
}>;

const LIBRARY = /^(?:CArmour|CWeapon|AWeaponR|AWeaponL|ARWeapon|Mount|Transform)\/[0-9]{2,5}$/;
const integer = (value: unknown, low: number, high: number): value is number =>
  typeof value === "number" && Number.isSafeInteger(value) && value >= low && value <= high;
const exactKeys = (value: Record<string, unknown>, keys: readonly string[]) =>
  Object.keys(value).length === keys.length && keys.every(key => Object.hasOwn(value, key));
const object = (value: unknown): Record<string, unknown> | null =>
  value !== null && typeof value === "object" && !Array.isArray(value) ? value as Record<string, unknown> : null;

/** The shared Rust document supplies all layer/frame choices. This is a reader, not a cycle or stats algorithm. */
export function readCashPreviewLayerDocument(value: unknown, direction: number): CashPreviewLayerDocument | null {
  const doc = object(value);
  if (!doc || !exactKeys(doc, ["version", "known", "direction", "layers"]) || doc.version !== 1 ||
      typeof doc.known !== "boolean" || !integer(doc.direction, 1, 8) || doc.direction !== direction ||
      !Array.isArray(doc.layers) || doc.layers.length > 3 || (!doc.known && doc.layers.length !== 0) ||
      (doc.known && doc.layers.length === 0)) return null;
  const layers: { library: string; frame: number }[] = [];
  const identities = new Set<string>();
  for (const raw of doc.layers) {
    const layer = object(raw);
    if (!layer || !exactKeys(layer, ["library", "frame"]) || typeof layer.library !== "string" ||
        !LIBRARY.test(layer.library) || !integer(layer.frame, 0, 65535)) return null;
    const identity = layer.library + ":" + layer.frame;
    if (identities.has(identity)) return null;
    identities.add(identity);
    layers.push(Object.freeze({ library: layer.library, frame: layer.frame }));
  }
  return Object.freeze({ version: 1, known: doc.known, direction: doc.direction, layers: Object.freeze(layers) });
}

/** Missing or ambiguous frame metadata stays undrawn, exactly as in Native preview(). */
export function composeCashPreviewFrames(
  document: CashPreviewLayerDocument | null,
  libraries: ReadonlyMap<string, OriginalSceneSpriteLibraryMeta>,
): CashPreviewDrawing {
  if (!document?.known) return Object.freeze({ complete: false, layers: Object.freeze([]) });
  const layers: CashPreviewDrawLayer[] = [];
  let complete = true;
  for (const layer of document.layers) {
    const meta = libraries.get(layer.library);
    const candidates = meta?.frames.filter(frame => frame.index === layer.frame) ?? [];
    const frame = candidates.length === 1 ? candidates[0] : null;
    const path = "/original-ui/" + layer.library + "/" + layer.frame + ".png";
    if (!meta || !integer(meta.count, 1, 65536) || layer.frame >= meta.count || !frame ||
        !integer(frame.width, 1, 65535) || !integer(frame.height, 1, 65535) ||
        !integer(frame.x, -65535, 65535) || !integer(frame.y, -65535, 65535) || frame.path !== path) {
      complete = false;
      continue;
    }
    layers.push(Object.freeze({ library: layer.library, frame: layer.frame, path,
      left: 105 + frame.x, top: 160 + frame.y, width: frame.width, height: frame.height }));
  }
  return Object.freeze({ complete, layers: Object.freeze(layers) });
}
