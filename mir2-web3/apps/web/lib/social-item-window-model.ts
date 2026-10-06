import { projectBevyBagModel, type BevyBagWorldSource } from "./bevy-bag-model";
import { crystalInventoryCapacities } from "./world-model/item-identity";

/** `slot` is the physical Bag1/Bag2 protocol index, or the actual social grid index. */
export type SocialItemSlot = Readonly<{ slot: number; uniqueId: number; name: string; icon?: number; count: number }>;
export type SocialInventoryProjection =
  | { ok: true; items: SocialItemSlot[]; emptySlots: number[]; capacity: number }
  | { ok: false; error: "invalidInventory" };
export type SocialGridProjection =
  | { ok: true; slots: Array<SocialItemSlot | null> }
  | { ok: false; error: "invalidTrade" | "invalidGuildStorage" };

export const SOCIAL_TRADE_SLOT_COUNT = 10;
export const SOCIAL_GUILD_SLOT_COUNT = 112;
const record = (value: unknown): value is Record<string, unknown> =>
  typeof value === "object" && value !== null && !Array.isArray(value);
const integer = (value: unknown, max = Number.MAX_SAFE_INTEGER): value is number =>
  typeof value === "number" && Number.isSafeInteger(value) && value >= 0 && value <= max;

/** Only a concrete safe server instance identity may be used by an action. Zero remains a valid wire ID. */
export function requiredSocialItemIdentity(value: unknown): number | null {
  return integer(value) ? value : null;
}

export function validSocialItemSlot(value: unknown, capacity: number): value is number {
  return integer(capacity, 256) && capacity > 0 && integer(value, capacity - 1);
}

export function validSocialItem(item: unknown, capacity: number): item is SocialItemSlot {
  return record(item) && validSocialItemSlot(item.slot, capacity)
    && requiredSocialItemIdentity(item.uniqueId) !== null
    && typeof item.name === "string" && item.name.length > 0 && item.name.length <= 256 && !item.name.includes("\0")
    && integer(item.count, 65_535) && item.count > 0
    && (item.icon === undefined || integer(item.icon, 2_147_483_647));
}

/** Validate every occupied and empty physical Bag cell, including uniqueness and completeness. */
export function validSocialInventoryProjection(inventory: SocialInventoryProjection): boolean {
  if (!record(inventory) || !inventory.ok || !Array.isArray(inventory.items) || !Array.isArray(inventory.emptySlots)
    || !crystalInventoryCapacities(inventory.capacity + 6)) return false;
  const occupied = new Set<number>(), ids = new Set<number>();
  for (const item of inventory.items) {
    if (!validSocialItem(item, inventory.capacity) || occupied.has(item.slot) || ids.has(item.uniqueId)) return false;
    occupied.add(item.slot); ids.add(item.uniqueId);
  }
  for (const slot of inventory.emptySlots) {
    if (!validSocialItemSlot(slot, inventory.capacity) || occupied.has(slot)) return false;
    occupied.add(slot);
  }
  return occupied.size === inventory.capacity;
}

/** Full current inventory only: the existing projector verifies capacity, grids and all instance aliases. */
export function projectSocialInventory(world: BevyBagWorldSource): SocialInventoryProjection {
  const invalid = (): SocialInventoryProjection => ({ ok: false, error: "invalidInventory" });
  try {
    const projection = projectBevyBagModel(world);
    if (!projection.ok || projection.model.items.some(item => item.uniqueId === null || item.quantity <= 0)) return invalid();
    const items = projection.model.items.filter(item => item.container === 0).map(item => ({
      slot: item.slot, uniqueId: item.uniqueId!, name: item.name, icon: item.icon, count: item.quantity,
    }));
    if (items.some(item => !validSocialItem(item, world.maxBagSlots))) return invalid();
    const occupied = new Set(items.map(item => item.slot));
    return { ok: true, items, capacity: world.maxBagSlots,
      emptySlots: Array.from({ length: world.maxBagSlots }, (_, slot) => slot).filter(slot => !occupied.has(slot)) };
  } catch { return invalid(); }
}

/** Stage5TradeState retains offers in the Bag: both exact maps must resolve to the same live UID/slot. */
export function projectOwnTradeItems(trade: unknown, inventory: SocialInventoryProjection): SocialGridProjection {
  const invalid = (): SocialGridProjection => ({ ok: false, error: "invalidTrade" });
  if (!validSocialInventoryProjection(inventory) || !inventory.ok || !record(trade) || typeof trade.partner !== "string"
    || !trade.partner.trim() || trade.partner.length > 256 || trade.partner.includes("\0")
    || typeof trade.settlementNonce !== "string" || !trade.settlementNonce || trade.settlementNonce.length > 256
    || trade.settlementNonce.includes("\0")
    || !integer(trade.offeredGold, 4_294_967_295)
    || [trade.accepted, trade.locked, trade.escrowPrepared, trade.completed].some(value => typeof value !== "boolean")
    || !record(trade.offeredSlots) || !record(trade.offeredUniqueIds)) return invalid();
  const slots: Array<SocialItemSlot | null> = Array(SOCIAL_TRADE_SLOT_COUNT).fill(null);
  const keys = Object.keys(trade.offeredSlots);
  if (keys.length > SOCIAL_TRADE_SLOT_COUNT || Object.keys(trade.offeredUniqueIds).length !== keys.length) return invalid();
  const seenBags = new Set<number>(), seenIds = new Set<number>();
  for (const key of keys) {
    if (!/^[0-9]$/.test(key) || !Object.hasOwn(trade.offeredUniqueIds, key)) return invalid();
    const bagSlot = trade.offeredSlots[key], id = requiredSocialItemIdentity(trade.offeredUniqueIds[key]);
    if (!validSocialItemSlot(bagSlot, inventory.capacity) || id === null || seenBags.has(bagSlot) || seenIds.has(id)) return invalid();
    const matches = inventory.items.filter(item => item.slot === bagSlot && item.uniqueId === id);
    if (matches.length !== 1 || inventory.items.filter(item => item.uniqueId === id).length !== 1) return invalid();
    seenBags.add(bagSlot); seenIds.add(id);
    slots[Number(key)] = { ...matches[0], slot: Number(key) };
  }
  return { ok: true, slots };
}

/** Exact Web typed packet shape: GuildStorageItem {item: UserItem snake_case, userId}; array index is the slot. */
export function projectGuildStorageItems(rawItems: unknown,
  resolveItemInfo: (itemIndex: number) => { name: string; icon?: number } | null,
): SocialGridProjection {
  const invalid = (): SocialGridProjection => ({ ok: false, error: "invalidGuildStorage" });
  if (!Array.isArray(rawItems) || rawItems.length !== SOCIAL_GUILD_SLOT_COUNT) return invalid();
  const slots: Array<SocialItemSlot | null> = [], identities = new Set<number>();
  try {
    for (let slot = 0; slot < rawItems.length; slot++) {
      const row: unknown = rawItems[slot];
      if (row === null) { slots.push(null); continue; }
      if (!record(row) || !record(row.item) || Object.hasOwn(row.item, "uniqueId") || Object.hasOwn(row.item, "itemIndex")
        || typeof row.userId !== "number" || !Number.isSafeInteger(row.userId)
        || !integer(row.item.item_index, 2_147_483_647) || !integer(row.item.count, 65_535) || row.item.count === 0) return invalid();
      const uniqueId = requiredSocialItemIdentity(row.item.unique_id), info = resolveItemInfo(row.item.item_index);
      if (uniqueId === null || identities.has(uniqueId) || !info) return invalid();
      const item = { slot, uniqueId, name: info.name, icon: info.icon, count: row.item.count };
      if (!validSocialItem(item, SOCIAL_GUILD_SLOT_COUNT)) return invalid();
      identities.add(uniqueId); slots.push(item);
    }
    return { ok: true, slots };
  } catch { return invalid(); }
}

/** A selection remains display state until the host revalidates its source lease and sends. */
export function sameSocialItem(left: SocialItemSlot, right: SocialItemSlot): boolean {
  return left.slot === right.slot && left.uniqueId === right.uniqueId && left.count === right.count
    && left.name === right.name && left.icon === right.icon;
}

export function currentSocialItem(items: readonly SocialItemSlot[], selection: SocialItemSlot, capacity = 256): SocialItemSlot | null {
  if (!validSocialItem(selection, capacity) || items.some(item => !validSocialItem(item, capacity))) return null;
  const matches = items.filter(item => item.slot === selection.slot || item.uniqueId === selection.uniqueId);
  return matches.length === 1 && validSocialItem(matches[0], capacity) && sameSocialItem(matches[0], selection) ? matches[0] : null;
}
