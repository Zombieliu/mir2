import { authoritativeItemUniqueId, crystalInventoryCapacities, equipmentSourceGrid } from "./world-model/item-identity";
import type { EquipmentItem, WorldItem } from "./world-model/types";

const SLOT_INDEX: Readonly<Record<string, number>> = {
  weapon: 0, armour: 1, armor: 1, helmet: 2, torch: 3, necklace: 4,
  braceletleft: 5, braceletright: 6, ringleft: 7, ringright: 8,
  amulet: 9, belt: 10, boots: 11, stone: 12, mount: 13,
};

export type EquipmentGatewaySnapshot = {
  capacity: number;
  storageCapacity?: number;
  placements: Array<{ uniqueId: number | null; container: 0 | 1 | 2 | 3 | 4; slot: number }>;
};

function record(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function integer(value: unknown): value is number {
  return typeof value === "number" && Number.isSafeInteger(value) && value >= 0;
}

/** Read only this full Gateway snapshot, never an ACK-patched React world. */
export function projectEquipmentGatewaySnapshot(raw: unknown): EquipmentGatewaySnapshot | null {
  if (!record(raw) || !integer(raw.playerObjectId)
    || !crystalInventoryCapacities(raw.inventoryCapacity)
    || !Array.isArray(raw.inventoryItems) || !Array.isArray(raw.beltItems)
    || !Array.isArray(raw.equipmentItems)
    || (raw.storageItems !== undefined && !Array.isArray(raw.storageItems))) return null;
  const placements: EquipmentGatewaySnapshot["placements"] = [];
  const groups = [raw.inventoryItems, raw.beltItems, raw.equipmentItems, raw.storageItems ?? []];
  for (const [group, items] of groups.entries()) {
    for (const item of items) {
      if (!record(item)) return null;
      const uniqueId = item.uniqueId == null ? null : authoritativeItemUniqueId(item.uniqueId);
      if (uniqueId === undefined) return null;
      let container: 0 | 1 | 2 | 3 | 4;
      let slot: number;
      if (group === 2) {
        if (typeof item.slot !== "string") return null;
        slot = SLOT_INDEX[item.slot.toLowerCase()];
        if (!integer(slot)) return null;
        container = 2;
      } else {
        if (!integer(item.slot)) return null;
        slot = item.slot;
        if (group === 0 && (item.container === "bag1" || item.container === "bag2")) {
          if (slot >= 40) return null;
          container = 0;
          if (item.container === "bag2") slot += 40;
        } else if (group === 0 && item.container === "quest") {
          container = 3;
        } else if (group === 1 && item.container === "belt") {
          container = 1;
        } else if (group === 3 && item.container === "storage") {
          container = 4;
        } else return null;
      }
      // Preserve every row. The shared WASM boundary rejects duplicate IDs,
      // duplicate cells and out-of-capacity placement before changing a ledger.
      placements.push({ uniqueId, container, slot });
    }
  }
  if (raw.storageSize !== undefined && (!integer(raw.storageSize) || raw.storageSize < 1 || raw.storageSize > 256)) return null;
  if (placements.some((item) => item.container === 4) && raw.storageSize === undefined) return null;
  return {
    capacity: raw.inventoryCapacity as number,
    ...(raw.storageSize === undefined ? {} : { storageCapacity: raw.storageSize as number }),
    placements,
  };
}

export type EquipmentGatewayOperation = {
  kind: "equip" | "remove";
  grid: "inventory" | "belt" | "storage";
  uniqueId: number;
  to: number;
};

export function equipmentGatewayOperation(command: Record<string, unknown>): EquipmentGatewayOperation | null {
  const kind = command.type === "equipItem" ? "equip" : command.type === "removeItem" ? "remove" : null;
  const uniqueId = authoritativeItemUniqueId(command.uniqueId);
  if (!kind || uniqueId === undefined || typeof command.grid !== "string" || !integer(command.to)) return null;
  const grid = command.grid.toLowerCase();
  if (!(grid === "inventory" || grid === "belt" || grid === "storage")) return null;
  if (command.to > 0x7fff_ffff || (kind === "equip" && command.to >= 14)) return null;
  return { kind, uniqueId, grid, to: command.to };
}

/** Type metadata identifies gear; names and image numbers never do. */
export function classifyEquipmentUse(item: { equipSlot?: unknown; tooltipSource?: unknown }):
  { kind: "equipment"; to: number } | { kind: "ordinary" } | { kind: "unknown" } {
  if (typeof item.equipSlot === "string") {
    const to = SLOT_INDEX[item.equipSlot.toLowerCase()];
    if (integer(to)) return { kind: "equipment", to };
  }
  const source = item.tooltipSource;
  const type = record(source) && record(source.info) ? source.info.item_type : undefined;
  if (!integer(type) || type > 255) return { kind: "unknown" };
  // Same Crystal type → equipment slot contract used by native inspected Use.
  const destinations: Readonly<Record<number, number>> = {
    1: 0, 2: 1, 4: 2, 5: 4, 6: 5, 7: 7, 8: 9, 9: 10, 10: 11, 11: 12, 12: 3, 19: 13,
  };
  return destinations[type] === undefined ? { kind: "ordinary" } : { kind: "equipment", to: destinations[type] };
}

type CurrentItems = {
  inventoryItems: readonly WorldItem[];
  beltItems: readonly WorldItem[];
  storageItems: readonly WorldItem[];
  equipmentItems: readonly EquipmentItem[];
};

/** Recheck the current instance at the final public send gate. */
export function currentEquipmentCommandItem(world: CurrentItems, uniqueId: number, grid: string): WorldItem | EquipmentItem | null {
  if (authoritativeItemUniqueId(uniqueId) === undefined) return null;
  const rows = [
    ...world.inventoryItems.map((item) => ({ item, grid: equipmentSourceGrid(item.container) as string })),
    ...world.beltItems.map((item) => ({ item, grid: equipmentSourceGrid(item.container) as string })),
    ...world.storageItems.map((item) => ({ item, grid: equipmentSourceGrid(item.container) as string })),
    ...world.equipmentItems.map((item) => ({ item, grid: "equipment" })),
  ];
  const matches = rows.filter(({ item }) => item.authoritativeUniqueId === uniqueId);
  if (matches.length !== 1 || matches[0].grid.toLowerCase() !== grid.toLowerCase()) return null;
  const match = matches[0];
  const atCell = rows.filter(({ item, grid: rowGrid }) => rowGrid === match.grid && item.slot === match.item.slot
    && ("container" in item ? item.container : "equipment") === ("container" in match.item ? match.item.container : "equipment"));
  return atCell.length === 1 ? match.item : null;
}
