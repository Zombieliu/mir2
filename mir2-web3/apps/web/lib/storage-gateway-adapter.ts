import { currentEquipmentCommandItem } from "./equipment-gateway-adapter";
import { crystalInventoryCapacities, authoritativeItemUniqueId } from "./world-model/item-identity";
import { mailMutationAllowed } from "./mail-parcel-gateway-adapter";
import type { MailParcelSnapshot } from "./client-core-runtime";
import type { EquipmentItem, ItemContainer, WorldItem } from "./world-model/types";

type StorageWorld = {
  inventoryCapacity: number;
  maxBagSlots: number;
  storageSize: number;
  hasExpandedStorage: boolean;
  hasStoragePassword: boolean;
  requireStoragePassword: boolean;
  storageSessionUnlocked: boolean;
  inventoryItems: readonly WorldItem[];
  storageItems: readonly WorldItem[];
  beltItems: readonly WorldItem[];
  equipmentItems: readonly EquipmentItem[];
};

type TransferContainer = "bag1" | "bag2" | "storage";
type Selection = { container: ItemContainer; slot: number; authoritativeUniqueId?: number };
type TransferCell = Readonly<{ container: TransferContainer; slot: number; uniqueId: number | null }>;
export type StorageTransferProof = Readonly<{
  requestId: string;
  type: "storeItemV2" | "takeBackItemV2";
  from: number;
  to: number;
  source: TransferCell & { readonly uniqueId: number };
  target: TransferCell;
}>;
export type StorageTransferCommand = Pick<StorageTransferProof, "requestId" | "type" | "from" | "to">;
export type StorageTransferReservation = { proof: StorageTransferProof; enteredSocket: boolean };

function physicalSlot(world: StorageWorld, container: ItemContainer, slot: number): number | null {
  if (!Number.isSafeInteger(slot) || slot < 0) return null;
  if (container === "storage") {
    if (!Number.isSafeInteger(world.storageSize) || world.storageSize < 0 || world.storageSize > 160) return null;
    const capacity = Math.min(world.storageSize || 80, world.hasExpandedStorage ? 160 : 80);
    return slot < capacity ? slot : null;
  }
  const capacity = crystalInventoryCapacities(world.inventoryCapacity);
  if (!capacity || capacity.maxBagSlots !== world.maxBagSlots
    || (container !== "bag1" && container !== "bag2") || slot >= 40) return null;
  const physical = (container === "bag2" ? 40 : 0) + slot;
  return physical < capacity.maxBagSlots ? physical : null;
}

function cellItem(world: StorageWorld, container: TransferContainer, slot: number): WorldItem | null | undefined {
  const rows = (container === "storage" ? world.storageItems : world.inventoryItems)
    .filter(item => item.container === container && item.slot === slot);
  if (rows.length === 0) return null;
  if (rows.length !== 1) return undefined;
  const uniqueId = authoritativeItemUniqueId(rows[0].authoritativeUniqueId);
  if (uniqueId === undefined || currentEquipmentCommandItem(world, uniqueId,
    container === "storage" ? "storage" : "inventory") !== rows[0]) return undefined;
  return rows[0];
}

/** Transport identity proof only. The server remains responsible for accepting the transfer. */
export function prepareStorageTransfer(
  world: StorageWorld,
  operation: "deposit" | "withdraw",
  requestId: string,
  selection: Selection,
  target: { container: ItemContainer; slot: number },
): { command: StorageTransferCommand; proof: StorageTransferProof } | null {
  if (!/^st-[0-9]{16}$/.test(requestId)
    || world.requireStoragePassword && !world.hasStoragePassword
    || world.hasStoragePassword && !world.storageSessionUnlocked) return null;
  if (operation === "deposit"
    ? !["bag1", "bag2"].includes(selection.container) || target.container !== "storage"
    : selection.container !== "storage" || !["bag1", "bag2"].includes(target.container)) return null;
  const from = physicalSlot(world, selection.container, selection.slot);
  const to = physicalSlot(world, target.container, target.slot);
  const uniqueId = authoritativeItemUniqueId(selection.authoritativeUniqueId);
  if (from === null || to === null || uniqueId === undefined) return null;
  const sourceItem = cellItem(world, selection.container as TransferContainer, selection.slot);
  const targetItem = cellItem(world, target.container as TransferContainer, target.slot);
  if (!sourceItem || sourceItem.authoritativeUniqueId !== uniqueId || targetItem === undefined) return null;
  const proof: StorageTransferProof = Object.freeze({
    requestId, type: operation === "deposit" ? "storeItemV2" : "takeBackItemV2", from, to,
    source: Object.freeze({ container: selection.container as TransferContainer, slot: selection.slot, uniqueId }),
    target: Object.freeze({ container: target.container as TransferContainer, slot: target.slot,
      uniqueId: targetItem?.authoritativeUniqueId ?? null }),
  });
  return { command: { type: proof.type, requestId, from, to }, proof };
}

/** Called again after synchronous listeners, against the live world and immutable wire DTO. */
export function storageTransferStillCurrent(
  world: StorageWorld, command: Record<string, unknown>, proof: StorageTransferProof,
): boolean {
  if (Object.keys(command).length !== 4
    || command.type !== proof.type || command.requestId !== proof.requestId
    || command.from !== proof.from || command.to !== proof.to) return false;
  const current = prepareStorageTransfer(world, proof.type === "storeItemV2" ? "deposit" : "withdraw",
    proof.requestId, { container: proof.source.container, slot: proof.source.slot,
      authoritativeUniqueId: proof.source.uniqueId }, proof.target);
  return current !== null && current.proof.from === proof.from && current.proof.to === proof.to
    && current.proof.target.uniqueId === proof.target.uniqueId;
}

function transportSnapshot(world: StorageWorld): MailParcelSnapshot {
  const items: MailParcelSnapshot["items"] = [];
  for (const item of [...world.inventoryItems, ...world.beltItems, ...world.storageItems]) {
    const container = item.container === "storage" ? 4 : item.container === "belt" ? 1
      : item.container === "quest" ? 3 : 0;
    items.push({ container, slot: item.container === "bag2" ? 40 + item.slot : item.slot,
      uniqueId: authoritativeItemUniqueId(item.authoritativeUniqueId) ?? null, pricing: null, stamp: false });
  }
  // Equipment destinations also participate in cross-surface reservations.
  const slots = ["weapon", "armour", "helmet", "torch", "necklace", "braceletLeft", "braceletRight",
    "ringLeft", "ringRight", "amulet", "belt", "boots", "stone", "mount"];
  for (const item of world.equipmentItems) {
    items.push({ container: 2, slot: slots.indexOf(item.slot),
      uniqueId: authoritativeItemUniqueId(item.authoritativeUniqueId) ?? null, pricing: null, stamp: false });
  }
  return { bagCapacity: world.maxBagSlots, items };
}

/** Keep the existing receipt map exclusive across callbacks and other item surfaces. */
export function storageMutationAllowed(
  world: StorageWorld, command: Record<string, unknown>, pending: Iterable<StorageTransferReservation>,
  ownProof?: StorageTransferProof,
): boolean {
  const reservations = Array.from(pending).filter(entry => entry.proof !== ownProof);
  if (!reservations.length) return true;
  // Parcel collection chooses bag cells on the server. This DTO cannot prove
  // that it leaves the transfer's reserved empty destination untouched.
  if (command.type === "collectParcel") return false;
  const blocked = reservations.flatMap(({ proof }) => [proof.source.uniqueId,
    ...(proof.target.uniqueId === null ? [] : [proof.target.uniqueId])]);
  const cells = reservations.flatMap(({ proof }) => [
    { container: proof.type === "storeItemV2" ? 0 : 4, slot: proof.from },
    { container: proof.type === "storeItemV2" ? 4 : 0, slot: proof.to },
  ]);
  const snapshot = transportSnapshot(world);
  const idle = (id: unknown): boolean => {
    const uniqueId = authoritativeItemUniqueId(id);
    if (uniqueId === undefined || blocked.includes(uniqueId)) return false;
    const rows = snapshot.items.filter(item => item.uniqueId === uniqueId);
    return rows.length === 1 && !cells.some(cell => cell.container === rows[0].container && cell.slot === rows[0].slot);
  };
  // New Mail ownership must respect a transfer awaiting its own exact receipt.
  // Releasing a Mail lock is still allowed, including during connection cleanup.
  if (command.type === "mailLockedItem") return command.locked === false || command.locked === true && idle(command.uniqueId);
  if (command.type === "mailCost" || command.type === "sendMail") {
    return Array.isArray(command.itemsIdx) && command.itemsIdx.every(id => id === 0 || idle(id));
  }
  return mailMutationAllowed(command, snapshot, blocked, cells);
}
