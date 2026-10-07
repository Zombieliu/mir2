import { projectBevyBagModel, projectBevyStorageItems, type BevyBagProjectionError, type BevyInventoryModel, type BevyStorageItem } from "./bevy-bag-model";
import type { WorldState } from "./world-model/types";

export type BevyStorageModel = {
  items: BevyStorageItem[];
  size: number;
  has_password: boolean;
  unlocked: boolean;
  has_expanded: boolean;
  expiry: number;
};

export type BevyStorageProjection =
  | { ok: true; inventory: BevyInventoryModel; storage: BevyStorageModel }
  | { ok: false; error: BevyBagProjectionError };

/** Presentation only. Crystal item identities and transfers remain authoritative. */
export function projectBevyStorageModel(world: Pick<WorldState,
  "inventoryCapacity" | "maxBagSlots" | "gold" | "inventoryItems" | "beltItems" | "equipmentItems"
  | "storageItems" | "storageSize" | "hasStoragePassword" | "storageSessionUnlocked"
  | "hasExpandedStorage" | "expandedStorageExpiryTimeBinaryDatetime">): BevyStorageProjection {
  const bag = projectBevyBagModel(world);
  if (!bag.ok) return bag;
  const size = world.storageSize === 0 ? 80 : world.storageSize;
  if (![world.hasStoragePassword, world.storageSessionUnlocked, world.hasExpandedStorage].every(v => typeof v === "boolean")
    || !Number.isSafeInteger(size) || size < 1 || size > 160
    // ABI1's Rust StorageModel accepts a numeric i64 only. Exact wide date
    // strings stay in WorldState and the React surface; never round them here.
    || typeof world.expandedStorageExpiryTimeBinaryDatetime !== "number"
    || !Number.isSafeInteger(world.expandedStorageExpiryTimeBinaryDatetime)) {
    return { ok: false, error: { code: "invalidField", field: "storage" } };
  }
  const projected = projectBevyStorageItems(world.storageItems, size);
  if (!projected.ok) return projected;
  const ids = new Set(bag.model.items.flatMap(i => i.uniqueId === null ? [] : [i.uniqueId]));
  for (const item of projected.items) {
    if (item.uniqueId !== null && ids.has(item.uniqueId)) {
      return { ok: false, error: { code: "duplicateUniqueId", source: "storageItems", field: "authoritativeUniqueId" } };
    }
    if (item.uniqueId !== null) ids.add(item.uniqueId);
  }
  return { ok: true, inventory: bag.model, storage: { items: projected.items, size,
    has_password: world.hasStoragePassword, unlocked: world.storageSessionUnlocked,
    has_expanded: world.hasExpandedStorage, expiry: world.expandedStorageExpiryTimeBinaryDatetime } };
}
