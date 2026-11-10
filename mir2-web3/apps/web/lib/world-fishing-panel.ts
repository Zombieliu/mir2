import type { EquipmentGatewaySnapshot } from "./equipment-gateway-adapter";

export type FishingTackleView = Readonly<{
  uniqueId: number; name: string | null; quantity: number; imageIndex: number | null;
  durabilityCurrent: number | null; durabilityMax: number | null;
}>;
export type FishingRodView = Readonly<{
  uniqueId: number; name: string; shape: 49 | 50; durabilityCurrent: number | null;
  slots: readonly (FishingTackleView | null)[]; hasHook: boolean; canAutoCast: boolean;
}>;
export type FishingStatusView = Readonly<{ chance: number; progress: number; foundFish: boolean }>;
export type FishingAttachmentCommand = Readonly<
  | { type: "equipSlotItem"; grid: "inventory"; gridTo: "fishing"; uniqueId: number; to: number; toUniqueId: number }
  | { type: "removeSlotItem"; grid: "fishing"; gridTo: "inventory"; uniqueId: number; to: number; fromUniqueId: number }
>;

function record(value: unknown): Record<string, unknown> | null {
  return value && typeof value === "object" && !Array.isArray(value) ? value as Record<string, unknown> : null;
}
function unsigned(value: unknown): value is number {
  return typeof value === "number" && Number.isSafeInteger(value) && value >= 0;
}
function durability(value: unknown): number | null {
  return unsigned(value) && value <= 65535 ? value : null;
}

export function readFishingStatus(objectId: number, payload: unknown): FishingStatusView | false | null {
  const p = record(payload);
  if (!p || p.objectId !== objectId || typeof p.fishing !== "boolean") return null;
  if (!p.fishing) return false;
  if (typeof p.foundFish !== "boolean" || typeof p.chancePercent !== "number" || typeof p.progressPercent !== "number"
    || !Number.isSafeInteger(p.chancePercent) || !Number.isSafeInteger(p.progressPercent)) return null;
  return {
    chance: Math.max(0, Math.min(100, p.chancePercent)),
    progress: Math.max(0, Math.min(100, p.progressPercent)),
    foundFish: p.foundFish,
  };
}

export function projectFishingRod(carrier: unknown, rodId: number): FishingRodView | null {
  const source = record(carrier), user = record(source?.userItem), info = record(source?.realInfo ?? source?.info);
  if (!source || !user || !info || user.unique_id !== rodId || user.item_index !== info.item_index
    || info.item_type !== 1 || (info.shape !== 49 && info.shape !== 50) || typeof info.name !== "string"
    || !Array.isArray(user.slots) || user.slots.length < 5) return null;
  const metadata = Array.isArray(source.socketInfos) ? source.socketInfos : [];
  const slots: Array<FishingTackleView | null> = [];
  const used = new Set([rodId]);
  for (const [slot, value] of user.slots.slice(0, 5).entries()) {
    if (value === null) { slots.push(null); continue; }
    const child = record(value), childInfo = record(metadata[slot]);
    if (!child || !unsigned(child.unique_id) || used.has(child.unique_id) || !unsigned(child.count) || child.count === 0) return null;
    if (childInfo && (childInfo.item_index !== child.item_index || childInfo.item_type !== 28 + slot || typeof childInfo.name !== "string")) return null;
    used.add(child.unique_id);
    slots.push({
      uniqueId: child.unique_id, name: typeof childInfo?.name === "string" ? childInfo.name : null, quantity: child.count,
      imageIndex: unsigned(childInfo?.image) && childInfo.image <= 65535 ? childInfo.image : null,
      durabilityCurrent: durability(child.current_dura), durabilityMax: durability(child.max_dura),
    });
  }
  return {
    uniqueId: rodId, name: info.name, shape: info.shape, durabilityCurrent: durability(user.current_dura),
    slots, hasHook: slots[0] !== null, canAutoCast: slots[4] !== null,
  };
}

export function fishingGearForDurability(rod: FishingRodView, uniqueId: number) {
  return uniqueId === rod.uniqueId ? rod : rod.slots.find(slot => slot?.uniqueId === uniqueId) ?? null;
}

/** False requires a known empty weapon/reel slot or a known non-fishing weapon. */
export function fishingAutocastAvailability(equipmentItems: readonly unknown[], layout: EquipmentGatewaySnapshot): boolean | null {
  const rows = equipmentItems.map(record).filter(row => typeof row?.slot === "string" && row.slot.toLowerCase() === "weapon");
  const cells = layout.placements.filter(row => row.container === 2 && row.slot === 0);
  if (!rows.length && !cells.length) return false;
  const uid = cells[0]?.uniqueId, weapon = rows[0];
  if (rows.length !== 1 || cells.length !== 1 || !unsigned(uid) || weapon?.uniqueId !== uid
    || layout.placements.filter(row => row.uniqueId === uid).length !== 1) return null;
  const source = record(weapon.tooltipSource), user = record(source?.userItem), info = record(source?.realInfo ?? source?.info);
  if (!user || !info || user.unique_id !== uid || user.item_index !== info.item_index
    || info.item_type !== 1 || typeof info.shape !== "number" || !Number.isSafeInteger(info.shape)) return null;
  if (info.shape !== 49 && info.shape !== 50) return false;
  return projectFishingRod(source, uid)?.canAutoCast ?? null;
}

export function fishingBagCell(layout: EquipmentGatewaySnapshot, bagSlot: number): number | null | undefined {
  if (!unsigned(bagSlot) || bagSlot >= layout.capacity - 6 || bagSlot >= 80) return undefined;
  const rows = layout.placements.filter(row => row.container === 0 && row.slot === bagSlot);
  if (rows.length === 0) return null;
  const id = rows[0].uniqueId;
  return rows.length === 1 && unsigned(id) && layout.placements.filter(row => row.uniqueId === id).length === 1 ? id : undefined;
}

export function fishingAttachmentReceipt(command: FishingAttachmentCommand, packet: string, payload: unknown): boolean | null {
  const attach = command.type === "equipSlotItem", p = record(payload);
  if (!p || packet !== (attach ? "EquipSlotItem" : "RemoveSlotItem") || p.uniqueId !== command.uniqueId || p.to !== command.to
    || p.grid !== (attach ? "Inventory" : "Fishing") || p.gridTo !== (attach ? "Fishing" : "Inventory")
    || typeof p.success !== "boolean") return null;
  return p.success;
}

export function fishingAttachmentApplied(command: FishingAttachmentCommand, rod: FishingRodView,
  layout: EquipmentGatewaySnapshot): boolean {
  if (command.type === "equipSlotItem") {
    return rod.slots[command.to]?.uniqueId === command.uniqueId && !layout.placements.some(row => row.uniqueId === command.uniqueId);
  }
  return !rod.slots.some(item => item?.uniqueId === command.uniqueId) && fishingBagCell(layout, command.to) === command.uniqueId;
}
