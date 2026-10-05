import type { BevyInventoryModel, BevyBagItem } from "./bevy-bag-model";
import { validBagModel } from "./bevy-bag-ui";
export type StateItemMetadata = { library: "StateItem"; version: number; frames: Map<number, { x: number; y: number; width: number; height: number }> };
export type CharacterItem = BevyBagItem & { stateImageX: number; stateImageY: number; stateImageWidth: number; stateImageHeight: number };
export type CharacterModel = Omit<BevyInventoryModel, "items"> & { items: CharacterItem[] };
const bounded = (v: unknown, low: number, high: number): v is number => Number.isInteger(v) && Number(v) >= low && Number(v) <= high;
/** Exact library metadata; no per-instance or old-cell geometry cache. */
export function parseStateItemMetadata(value: unknown): StateItemMetadata | null {
  if (!value || typeof value !== "object") return null;
  const v = value as { version?: unknown; frames?: unknown };
  if (!Number.isSafeInteger(v.version) || Number(v.version) < 0 || !Array.isArray(v.frames)) return null;
  const frames: StateItemMetadata["frames"] = new Map();
  for (const row of v.frames) {
    if (!row || !bounded(row.index, 0, 65535) || frames.has(row.index)
      || !bounded(row.x, -2147483648, 2147483647) || !bounded(row.y, -2147483648, 2147483647)
      || !bounded(row.width, 0, 65535) || !bounded(row.height, 0, 65535)) return null;
    frames.set(row.index, { x: row.x, y: row.y, width: row.width, height: row.height });
  }
  return { library: "StateItem", version: Number(v.version), frames };
}
export function projectCharacterModel(model: BevyInventoryModel | null, metadata: StateItemMetadata | null): CharacterModel | null {
  if (!validBagModel(model)) return null;
  return { ...model, items: model.items.map(item => {
    const frame = item.container === 2 && item.stateImage !== undefined ? metadata?.frames.get(item.stateImage) : undefined;
    return { ...item, stateImageX: frame?.x ?? 0, stateImageY: frame?.y ?? 0,
      stateImageWidth: frame?.width ?? 0, stateImageHeight: frame?.height ?? 0 };
  }) };
}
