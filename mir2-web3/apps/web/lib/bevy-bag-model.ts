import { authoritativeItemUniqueId, exactItemUniqueId, crystalInventoryCapacities } from "./world-model/item-identity";
import type { EquipmentItem, EquipmentSlot, NpcGoldTradeCapacity, WorldItem, WorldState } from "./world-model/types";

/**
 * A deliberately narrow projection of the current Web read model into
 * `mir2_client_bevy::inventory::InventoryModel`. It never plans equipment
 * actions or turns a display slot/template/tooltip id into instance authority.
 *
 * WorldState may carry the gateway's optional tooltipSource and stateImage.
 * Older snapshots can omit them; this function cannot recover them from a
 * display name, key or icon.
 */
export type BevyBagWorldItem = WorldItem;
export type BevyBagEquipmentItem = EquipmentItem;

export type BevyBagWorldSource = {
  inventoryCapacity: WorldState["inventoryCapacity"];
  npcGoldTradeCapacity?: WorldState["npcGoldTradeCapacity"];
  maxBagSlots: WorldState["maxBagSlots"];
  gold: WorldState["gold"];
  inventoryItems: readonly BevyBagWorldItem[];
  beltItems: readonly BevyBagWorldItem[];
  equipmentItems: readonly BevyBagEquipmentItem[];
};

type TooltipSource = Record<string, unknown> & { info: Record<string, unknown> };

export type BevyBagItem = {
  uniqueId: number | null;
  exactUniqueId?: string;
  key: string;
  name: string;
  quantity: number;
  slot: number;
  container: 0 | 1 | 2 | 3;
  icon: number;
  description: string;
  durabilityCurrent?: number | null;
  durabilityMax?: number | null;
  equipSlot?: EquipmentSlot | null;
  grade?: string;
  attack?: number;
  defence?: number;
  addedAttack?: number;
  addedDefence?: number;
  addedLuck?: number;
  shape?: number;
  socketSlots?: number;
  stateImage?: number;
  tooltipSource?: TooltipSource;
};

export type BevyInventoryModel = {
  npcGoldTradeCapacity?: NpcGoldTradeCapacity | null;
  capacity: number;
  gold: number;
  items: BevyBagItem[];
};

export type BevyBagProjectionError = {
  code:
    | "invalidCapacity"
    | "invalidField"
    | "invalidSlot"
    | "unexpectedContainer"
    | "duplicateGrid"
    | "duplicateUniqueId";
  source?: "inventoryItems" | "beltItems" | "equipmentItems" | "storageItems";
  index?: number;
  field?: string;
};

export type BevyBagProjection =
  | { ok: true; model: BevyInventoryModel }
  | { ok: false; error: BevyBagProjectionError };

// Mirrors the explicit Web wire slot contract in page.tsx's equipmentSlotIndex.
const EQUIPMENT_SLOT_INDEX: Readonly<Record<EquipmentSlot, number>> = {
  weapon: 0,
  armour: 1,
  helmet: 2,
  torch: 3,
  necklace: 4,
  braceletLeft: 5,
  braceletRight: 6,
  ringLeft: 7,
  ringRight: 8,
  amulet: 9,
  belt: 10,
  boots: 11,
  stone: 12,
  mount: 13,
};

class ProjectionFailure extends Error {
  constructor(readonly detail: BevyBagProjectionError) {
    super(detail.code);
  }
}

type SourceName = NonNullable<BevyBagProjectionError["source"]>;

function fail(code: BevyBagProjectionError["code"], source?: SourceName, index?: number, field?: string): never {
  throw new ProjectionFailure({ code, source, index, field });
}

function integer(value: unknown, max: number, source: SourceName, index: number, field: string): number {
  if (typeof value !== "number" || !Number.isSafeInteger(value) || value < 0 || value > max) {
    fail("invalidField", source, index, field);
  }
  return value;
}

function signedInteger(value: unknown, source: SourceName, index: number, field: string): number {
  if (typeof value !== "number" || !Number.isSafeInteger(value)
    || value < -2_147_483_648 || value > 2_147_483_647) {
    fail("invalidField", source, index, field);
  }
  return value;
}

function requiredString(value: unknown, source: SourceName, index: number, field: string): string {
  if (typeof value !== "string") fail("invalidField", source, index, field);
  return value;
}

function optionalInteger(
  value: unknown,
  max: number,
  source: SourceName,
  index: number,
  field: string,
): number | undefined {
  return value === undefined || value === null ? undefined : integer(value, max, source, index, field);
}

function optionalSignedInteger(
  value: unknown,
  source: SourceName,
  index: number,
  field: string,
): number | undefined {
  return value === undefined || value === null ? undefined : signedInteger(value, source, index, field);
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

/** Top-level camelCase, nested Crystal Info/UserItem fields retain their wire schema. */
function optionalTooltipSource(value: unknown, source: SourceName, index: number): TooltipSource | undefined {
  if (value === undefined || value === null) return undefined;
  if (!isRecord(value) || !isRecord(value.info)
    || (value.realInfo !== undefined && value.realInfo !== null && !isRecord(value.realInfo))
    || (value.userItem !== undefined && value.userItem !== null && !isRecord(value.userItem))
    || (value.socketInfos !== undefined && !Array.isArray(value.socketInfos))
    || (value.realSocketInfos !== undefined && !Array.isArray(value.realSocketInfos))) {
    fail("invalidField", source, index, "tooltipSource");
  }
  return value as TooltipSource;
}

function baseItem(
  item: BevyBagWorldItem | BevyBagEquipmentItem,
  source: SourceName,
  index: number,
  container: BevyBagItem["container"],
  slot: number,
): BevyBagItem {
  const durabilityCurrent = optionalInteger(item.durabilityCurrent, 65_535, source, index, "durabilityCurrent");
  const durabilityMax = optionalInteger(item.durabilityMax, 65_535, source, index, "durabilityMax");
  const tooltipSource = optionalTooltipSource(item.tooltipSource, source, index);
  const exactUniqueId = item.exactUniqueId === undefined ? undefined : exactItemUniqueId(item.exactUniqueId);
  if (item.exactUniqueId !== undefined && (exactUniqueId === undefined
    || item.authoritativeUniqueId !== undefined && exactItemUniqueId(item.authoritativeUniqueId) !== exactUniqueId)) {
    fail("invalidField", source, index, "exactUniqueId");
  }
  return {
    // A missing, invalid or unsafe original id means read-only presentation.
    // In particular, display `uniqueId` and tooltip `user_item.unique_id`
    // have no authority here. Zero is a valid concrete id.
    uniqueId: authoritativeItemUniqueId(item.authoritativeUniqueId) ?? null,
    ...(exactUniqueId === undefined ? {} : { exactUniqueId }),
    key: requiredString(item.key, source, index, "key"),
    name: requiredString(item.name, source, index, "name"),
    quantity: integer(item.quantity, 4_294_967_295, source, index, "quantity"),
    slot,
    container,
    icon: integer(item.icon, 65_535, source, index, "icon"),
    description: requiredString(item.description, source, index, "description"),
    ...(durabilityCurrent === undefined ? {} : { durabilityCurrent }),
    ...(durabilityMax === undefined ? {} : { durabilityMax }),
    ...(tooltipSource === undefined ? {} : { tooltipSource }),
  };
}

function worldItem(item: BevyBagWorldItem, source: SourceName, index: number, maxBagSlots: number): BevyBagItem {
  let container: BevyBagItem["container"];
  let slot: number;
  if (source === "beltItems") {
    if (item.container !== "belt") fail("unexpectedContainer", source, index, "container");
    container = 1;
    slot = item.slot;
    if (!Number.isSafeInteger(slot) || slot < 0 || slot >= 6) fail("invalidSlot", source, index, "slot");
  } else {
    if (item.container === "bag1") {
      container = 0;
      slot = item.slot;
      if (!Number.isSafeInteger(slot) || slot < 0 || slot >= 40) fail("invalidSlot", source, index, "slot");
    } else if (item.container === "bag2") {
      if (!Number.isSafeInteger(item.slot) || item.slot < 0 || item.slot >= 40) {
        fail("invalidSlot", source, index, "slot");
      }
      container = 0;
      slot = 40 + item.slot;
      if (slot >= maxBagSlots) fail("invalidSlot", source, index, "slot");
    } else if (item.container === "quest") {
      container = 3;
      slot = item.slot;
      if (!Number.isSafeInteger(slot) || slot < 0 || slot >= 40) fail("invalidSlot", source, index, "slot");
    } else {
      // Storage belongs to a distinct source and is not part of this B2 read
      // model; do not silently drop it if accidentally mixed into a bag array.
      fail("unexpectedContainer", source, index, "container");
    }
  }
  const result = baseItem(item, source, index, container, slot);
  if (item.equipSlot !== undefined) {
    if (item.equipSlot !== null
      && !Object.prototype.hasOwnProperty.call(EQUIPMENT_SLOT_INDEX, item.equipSlot)) {
      fail("invalidField", source, index, "equipSlot");
    }
    result.equipSlot = item.equipSlot;
  }
  const stateImage = optionalInteger(item.stateImage, 65_535, source, index, "stateImage");
  const addedAttack = optionalSignedInteger(item.addedAttack, source, index, "addedAttack");
  const addedDefence = optionalSignedInteger(item.addedDefence, source, index, "addedDefence");
  if (item.grade !== undefined && item.grade !== null && typeof item.grade !== "string") {
    fail("invalidField", source, index, "grade");
  }
  if (stateImage !== undefined) result.stateImage = stateImage;
  if (addedAttack !== undefined) result.addedAttack = addedAttack;
  if (addedDefence !== undefined) result.addedDefence = addedDefence;
  if (item.grade !== undefined && item.grade !== null) result.grade = item.grade;
  // The page still defaults sellValue to 0 and maps addedAttack into attack.
  // Neither can be identified as the original field here, so do not mislabel
  // those values as authoritative ItemModel metadata.
  return result;
}

function equipmentItem(item: BevyBagEquipmentItem, index: number): BevyBagItem {
  const source = "equipmentItems";
  if (typeof item.slot !== "string"
    || !Object.prototype.hasOwnProperty.call(EQUIPMENT_SLOT_INDEX, item.slot)) {
    fail("invalidSlot", source, index, "slot");
  }
  const result = baseItem(item, source, index, 2, EQUIPMENT_SLOT_INDEX[item.slot]);
  const shape = optionalInteger(item.shape, 65_535, source, index, "shape");
  const stateImage = optionalInteger(item.stateImage, 65_535, source, index, "stateImage");
  const socketSlots = optionalInteger(item.socketSlots, 255, source, index, "socketSlots");
  const attack = optionalSignedInteger(item.attack, source, index, "attack");
  const defence = optionalSignedInteger(item.defence, source, index, "defence");
  const addedAttack = optionalSignedInteger(item.addedAttack, source, index, "addedAttack");
  const addedDefence = optionalSignedInteger(item.addedDefence, source, index, "addedDefence");
  const addedLuck = optionalSignedInteger(item.addedLuck, source, index, "addedLuck");
  if (item.grade !== undefined && item.grade !== null && typeof item.grade !== "string") {
    fail("invalidField", source, index, "grade");
  }
  if (shape !== undefined) result.shape = shape;
  if (stateImage !== undefined) result.stateImage = stateImage;
  if (socketSlots !== undefined) result.socketSlots = socketSlots;
  if (attack !== undefined) result.attack = attack;
  if (defence !== undefined) result.defence = defence;
  if (addedAttack !== undefined) result.addedAttack = addedAttack;
  if (addedDefence !== undefined) result.addedDefence = addedDefence;
  if (addedLuck !== undefined) result.addedLuck = addedLuck;
  if (item.grade !== undefined && item.grade !== null) result.grade = item.grade;
  return result;
}

function npcGoldTradeCapacity(value: unknown): NpcGoldTradeCapacity | null | undefined {
  if (value === undefined || value === null) return value;
  if (!isRecord(value) || Object.keys(value).sort().join("|") !== "freshCompatibleUniqueIds|rosterValid"
    || typeof value.rosterValid !== "boolean" || !Array.isArray(value.freshCompatibleUniqueIds)
    || value.freshCompatibleUniqueIds.length > 86
    || !value.rosterValid && value.freshCompatibleUniqueIds.length !== 0
    || value.freshCompatibleUniqueIds.some(id => exactItemUniqueId(id) === undefined)
    || new Set(value.freshCompatibleUniqueIds.map(exactItemUniqueId)).size !== value.freshCompatibleUniqueIds.length) {
    fail("invalidField", undefined, undefined, "npcGoldTradeCapacity");
  }
  return { rosterValid: value.rosterValid, freshCompatibleUniqueIds: [...value.freshCompatibleUniqueIds] };
}

/** The NPC path alone may use a current server attestation for grid-scoped aliases. */
export function projectBevyBagModel(world: BevyBagWorldSource, options?: { npcGoldTrade: true }): BevyBagProjection {
  const capacities = crystalInventoryCapacities(world.inventoryCapacity);
  if (!capacities || world.maxBagSlots !== capacities.maxBagSlots) {
    return { ok: false, error: { code: "invalidCapacity", field: "inventoryCapacity" } };
  }
  if (typeof world.gold !== "number" || !Number.isSafeInteger(world.gold)
    || world.gold < 0 || world.gold > 4_294_967_295) {
    return { ok: false, error: { code: "invalidField", field: "gold" } };
  }
  if (!Array.isArray(world.inventoryItems) || !Array.isArray(world.beltItems)
    || !Array.isArray(world.equipmentItems)) {
    return { ok: false, error: { code: "invalidField", field: "items" } };
  }
  try {
    const evidence = npcGoldTradeCapacity(world.npcGoldTradeCapacity);
    const npcAliases = options?.npcGoldTrade === true && evidence?.rosterValid === true;
    const compatible = new Set(evidence?.freshCompatibleUniqueIds.map(exactItemUniqueId) ?? []);
    const items: BevyBagItem[] = [];
    const occupied = new Set<string>();
    const identities = new Set<string>();
    const gridIdentities = new Set<string>();
    const add = (item: BevyBagItem, source: SourceName, index: number) => {
      const grid = `${item.container}:${item.slot}`;
      const id = item.exactUniqueId ?? exactItemUniqueId(item.uniqueId);
      if (occupied.has(grid)) fail("duplicateGrid", source, index, "slot");
      if (id !== undefined && (gridIdentities.has(item.container + ":" + id)
        || identities.has(id) && (!npcAliases || compatible.has(id)))) {
        fail("duplicateUniqueId", source, index, "authoritativeUniqueId");
      }
      occupied.add(grid);
      if (id !== undefined) {
        identities.add(id);
        gridIdentities.add(item.container + ":" + id);
      }
      items.push(item);
    };
    for (let index = 0; index < world.inventoryItems.length; index += 1) {
      const item = world.inventoryItems[index];
      if (typeof item !== "object" || item === null || Array.isArray(item)) {
        fail("invalidField", "inventoryItems", index, "item");
      }
      add(worldItem(item, "inventoryItems", index, capacities.maxBagSlots), "inventoryItems", index);
    }
    for (let index = 0; index < world.beltItems.length; index += 1) {
      const item = world.beltItems[index];
      if (typeof item !== "object" || item === null || Array.isArray(item)) {
        fail("invalidField", "beltItems", index, "item");
      }
      add(worldItem(item, "beltItems", index, capacities.maxBagSlots), "beltItems", index);
    }
    for (let index = 0; index < world.equipmentItems.length; index += 1) {
      const item = world.equipmentItems[index];
      if (typeof item !== "object" || item === null || Array.isArray(item)) {
        fail("invalidField", "equipmentItems", index, "item");
      }
      add(equipmentItem(item, index), "equipmentItems", index);
    }
    if (evidence) {
      for (const id of compatible) {
        const rows = items.filter(item => (item.exactUniqueId ?? exactItemUniqueId(item.uniqueId)) === exactItemUniqueId(id));
        if (rows.length !== 1 || rows[0].container > 1) fail("invalidField", undefined, undefined, "npcGoldTradeCapacity");
      }
    }
    return { ok: true, model: { capacity: capacities.inventoryCapacity, gold: world.gold, items,
      ...(evidence === undefined ? {} : { npcGoldTradeCapacity: evidence }) } };
  } catch (error) {
    if (error instanceof ProjectionFailure) return { ok: false, error: error.detail };
    throw error;
  }
}

export type BevyStorageItem = Omit<BevyBagItem, "container"> & { container: 4 };

/** Uses the same item metadata projection; storage owns a separate grid. */
export function projectBevyStorageItems(source: readonly WorldItem[], capacity: number):
  | { ok: true; items: BevyStorageItem[] }
  | { ok: false; error: BevyBagProjectionError } {
  if (!Array.isArray(source) || !Number.isSafeInteger(capacity) || capacity < 1 || capacity > 160) {
    return { ok: false, error: { code: "invalidCapacity", field: "storageSize" } };
  }
  try {
    const cells = new Set<number>(), ids = new Set<string>();
    const items = source.map((item, index): BevyStorageItem => {
      if (!isRecord(item) || item.container !== "storage") fail("unexpectedContainer", "storageItems", index, "container");
      const slot = integer(item.slot, capacity - 1, "storageItems", index, "slot");
      const projected = baseItem(item as WorldItem, "storageItems", index, 0, slot);
      if (cells.has(slot)) fail("duplicateGrid", "storageItems", index, "slot");
      const id = projected.exactUniqueId ?? exactItemUniqueId(projected.uniqueId);
      if (id !== undefined && ids.has(id)) fail("duplicateUniqueId", "storageItems", index, "authoritativeUniqueId");
      cells.add(slot);
      if (id !== undefined) ids.add(id);
      return { ...projected, container: 4 };
    });
    return { ok: true, items };
  } catch (error) {
    if (error instanceof ProjectionFailure) return { ok: false, error: error.detail };
    throw error;
  }
}
