import type { EquipmentItem, EquipmentSlot, ItemContainer, WorldItem } from "./types";

/** A display slot is never evidence of a concrete server item instance. */
export function authoritativeItemUniqueId(value: unknown): number | undefined {
  return typeof value === "number" && Number.isSafeInteger(value) && value >= 0
    ? value
    : undefined;
}
/** Full-range source identity is text, never recovered from an unsafe Number. */
export function exactItemUniqueId(value: unknown): string | undefined {
  if (authoritativeItemUniqueId(value) !== undefined) return String(value);
  return typeof value === "string" && /^(?:0|[1-9][0-9]{0,19})$/.test(value)
    && BigInt(value) <= 18446744073709551615n ? value : undefined;
}

/** Preserve the old display key while keeping server identity separate. */
export function projectInventoryItemIdentity(
  rawUniqueId: unknown,
  container: ItemContainer,
  slot: number,
): Pick<WorldItem, "uniqueId" | "authoritativeUniqueId" | "exactUniqueId"> {
  const authoritativeUniqueId = authoritativeItemUniqueId(rawUniqueId);
  const uniqueId = typeof rawUniqueId === "number"
    ? rawUniqueId
    : container === "bag2" ? 40 + slot : slot;
  return {
    uniqueId,
    ...(authoritativeUniqueId === undefined ? {} : { authoritativeUniqueId }),
    ...(typeof rawUniqueId === "string" && exactItemUniqueId(rawUniqueId) !== undefined ? { exactUniqueId: rawUniqueId } : {}),
  };
}

/** A storage packet replaces its rows; an omitted ID must not inherit old authority. */
export function projectStoragePacketItemIdentity(
  packet: { uniqueId?: unknown; unique_id?: unknown },
  slot: number,
  previousDisplayUniqueId?: number,
): Pick<WorldItem, "uniqueId" | "authoritativeUniqueId"> {
  const camelId = typeof packet.uniqueId === "number" ? packet.uniqueId : undefined;
  const snakeId = typeof packet.unique_id === "number" ? packet.unique_id : undefined;
  const rawUniqueId = camelId ?? snakeId;
  const authoritativeUniqueId = camelId !== undefined && snakeId !== undefined && camelId !== snakeId
    ? undefined
    : authoritativeItemUniqueId(rawUniqueId);
  return {
    uniqueId: rawUniqueId ?? previousDisplayUniqueId ?? slot,
    ...(authoritativeUniqueId === undefined ? {} : { authoritativeUniqueId }),
  };
}

export type AuthoritativeItemSelection = Pick<WorldItem, "container" | "slot"> & {
  uniqueId: number;
};

/** Resolve a selected instance against the current snapshot immediately before submit. */
export function currentAuthoritativeItem<T extends Pick<WorldItem, "container" | "slot" | "authoritativeUniqueId">>(
  items: readonly T[],
  selection: AuthoritativeItemSelection,
): T | null {
  if (authoritativeItemUniqueId(selection.uniqueId) === undefined
    || !Number.isSafeInteger(selection.slot) || selection.slot < 0) return null;

  let current: T | null = null;
  for (const item of items) {
    if (item.container !== selection.container || item.slot !== selection.slot) continue;
    // Two rows claiming one slot are ambiguous even if one has the requested ID.
    if (current) return null;
    current = item;
  }
  return current?.authoritativeUniqueId === selection.uniqueId ? current : null;
}

/** Equipment removal identifies the server instance; the slot verifies selection only. */
export function currentAuthoritativeEquipmentItem<T extends Pick<EquipmentItem, "slot" | "authoritativeUniqueId">>(
  items: readonly T[],
  selection: { slot: EquipmentSlot; uniqueId: number },
): T | null {
  if (authoritativeItemUniqueId(selection.uniqueId) === undefined) return null;

  let current: T | null = null;
  for (const item of items) {
    if (item.slot !== selection.slot) continue;
    if (current) return null;
    current = item;
  }
  return current?.authoritativeUniqueId === selection.uniqueId ? current : null;
}

/** The movement fast path must notice authority gained or lost at an unchanged display slot. */
export function sameAuthoritativeItemIdentities(
  snapshotItems: readonly { uniqueId?: unknown }[] | undefined,
  projectedItems: readonly { authoritativeUniqueId?: unknown }[] | undefined,
): boolean {
  const raw = snapshotItems ?? [];
  const projected = projectedItems ?? [];
  return raw.length === projected.length && raw.every((item, index) =>
    authoritativeItemUniqueId(item.uniqueId) ===
    authoritativeItemUniqueId(projected[index].authoritativeUniqueId)
  );
}

/** Keep the selected source grid distinct from the destination equipment slot. */
export function equipmentSourceGrid(container: ItemContainer): "inventory" | "storage" | "belt" | "questInventory" {
  if (container === "storage") return "storage";
  if (container === "belt") return "belt";
  if (container === "quest") return "questInventory";
  return "inventory";
}

/** Crystal ResizeInventory.Size is the whole array, including six belt cells.
 * See Crystal/Server/MirDatabase/CharacterInfo.cs:594-607 and
 * Crystal/Client/MirScenes/GameScene.cs:6533-6536.
 */
export function crystalInventoryCapacities(rawSize: unknown): { inventoryCapacity: number; maxBagSlots: number } | null {
  if (typeof rawSize !== "number" || !Number.isSafeInteger(rawSize)
    || !(rawSize === 46 || (rawSize >= 54 && rawSize <= 86 && (rawSize - 54) % 4 === 0))) {
    return null;
  }
  return { inventoryCapacity: rawSize, maxBagSlots: rawSize - 6 };
}

/** Crystal MoveItem's inventory from/to are bag cell indices, never instance IDs. */
export function planBagMove(
  inventory: readonly Pick<WorldItem, "container" | "slot" | "authoritativeUniqueId">[],
  maxBagSlots: number,
  selection: Pick<WorldItem, "container" | "slot" | "authoritativeUniqueId">,
  toLocalSlot: number,
  toContainer: ItemContainer | undefined,
): { command: { type: "moveItem"; grid: "inventory"; from: number; to: number }; affectedUniqueIds: number[] } | null {
  if ((selection.container !== "bag1" && selection.container !== "bag2")
    || (toContainer !== "bag1" && toContainer !== "bag2")) return null;
  const identity = authoritativeItemUniqueId(selection.authoritativeUniqueId);
  const capacity = crystalInventoryCapacities(maxBagSlots + 6);
  if (identity === undefined || !capacity || capacity.maxBagSlots !== maxBagSlots
    || !Number.isSafeInteger(selection.slot) || selection.slot < 0 || selection.slot >= 40
    || !Number.isSafeInteger(toLocalSlot) || toLocalSlot < 0 || toLocalSlot >= 40) return null;
  const from = (selection.container === "bag2" ? 40 : 0) + selection.slot;
  const to = (toContainer === "bag2" ? 40 : 0) + toLocalSlot;
  if (from >= maxBagSlots || to >= maxBagSlots || from === to) return null;
  const current = currentAuthoritativeItem(inventory, {
    container: selection.container, slot: selection.slot, uniqueId: identity,
  });
  if (!current || inventory.filter((item) => item.authoritativeUniqueId === identity).length !== 1) return null;
  // MoveItem can exchange two occupied cells. A duplicate target or a target
  // without a trustworthy instance identity cannot safely bypass pending gear.
  const targetItems = inventory.filter((item) => item.container === toContainer && item.slot === toLocalSlot);
  if (targetItems.length > 1) return null;
  const affectedUniqueIds = [identity];
  if (targetItems.length === 1) {
    const targetId = authoritativeItemUniqueId(targetItems[0].authoritativeUniqueId);
    if (targetId === undefined || inventory.filter((item) => item.authoritativeUniqueId === targetId).length !== 1) return null;
    affectedUniqueIds.push(targetId);
  }
  return { command: { type: "moveItem", grid: "inventory", from, to }, affectedUniqueIds };
}

/** Build the remove request only from the current equipped instance. */
export function planEquipmentRemoval(
  equipment: readonly Pick<EquipmentItem, "slot" | "authoritativeUniqueId">[],
  inventory: readonly Pick<WorldItem, "container" | "slot">[],
  maxBagSlots: number,
  selection: { slot: EquipmentSlot; uniqueId: number },
): { type: "removeItem"; uniqueId: number; grid: "inventory"; to: number } | null {
  if (!currentAuthoritativeEquipmentItem(equipment, selection)) return null;
  // maxBagSlots is the server's total capacity less its six belt cells.
  const capacity = crystalInventoryCapacities(maxBagSlots + 6);
  if (!capacity || capacity.maxBagSlots !== maxBagSlots) return null;
  const occupiedBagSlots = new Set(
    inventory.flatMap((entry) => {
      if (!Number.isSafeInteger(entry.slot) || entry.slot < 0 || entry.slot >= 40) return [];
      if (entry.container === "bag1") return [entry.slot];
      if (entry.container === "bag2") return [40 + entry.slot];
      return [];
    }),
  );
  const targetSlot = Array.from({ length: maxBagSlots }, (_, slot) => slot).find(
    (slot) => !occupiedBagSlots.has(slot),
  );
  if (targetSlot === undefined) return null;
  return { type: "removeItem", uniqueId: selection.uniqueId, grid: "inventory", to: targetSlot };
}
