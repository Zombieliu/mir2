export type R16NpcShopRect = Readonly<{ left: number; top: number; width: number; height: number }>;
const rect = (left: number, top: number, width: number, height: number): R16NpcShopRect => ({ left, top, width, height });

/** Native NPCDropDialog, independent from the portable Buy viewport. */
export const R16_NPC_SELL_LAYOUT = {
  panel: rect(264, 224, 176, 147),
  quote: rect(30, 10, 140, 26),
  target: rect(20, 55, 75, 75),
  item: rect(38, 72, 36, 32),
  confirm: rect(114, 62, 48, 25),
  buy: rect(20, 112, 60, 22),
  // Explicit compatibility selection/quantity controls, never a fake Hold.
  quantity: rect(114, 36, 48, 25),
  close: rect(145, 106, 25, 25),
  picker: rect(184, 0, 360, 334),
  pickerRows: 8,
} as const;

export type R16NpcSellItem = Readonly<{ id: number | string; name: string; icon: number; price: number; count?: number; disabled?: boolean }>;
export type R16NpcSellSelection = Readonly<{ sourceKey: string; id: number | string; itemKey: string }>;
const itemKey = (item: R16NpcSellItem) => JSON.stringify([typeof item.id, item.id, item.name, item.icon, item.price, item.count ?? 1, item.disabled === true]);

/** Presentation fence only. The existing host/Core remains sale authority. */
export function r16NpcSellSourceKey(npcName: string | undefined, tabs: readonly string[], items: readonly R16NpcSellItem[]): string {
  return JSON.stringify([npcName ?? "", tabs, items.map(itemKey)]);
}
export function r16NpcSellCapture(sourceKey: string, items: readonly R16NpcSellItem[], id: number | string): R16NpcSellSelection | null {
  const matches = items.filter(item => item.id === id), item = matches[0];
  return matches.length === 1 && !item.disabled && r16NpcSellQuantity("1", item.count) !== null && r16NpcSellTotal(item, 1) !== null
    ? Object.freeze({ sourceKey, id: item.id, itemKey: itemKey(item) }) : null;
}
export function r16NpcSellTarget(selection: R16NpcSellSelection | null, sourceKey: string, items: readonly R16NpcSellItem[]): R16NpcSellItem | null {
  if (!selection || selection.sourceKey !== sourceKey) return null;
  const matches = items.filter(item => item.id === selection.id), item = matches[0];
  return matches.length === 1 && itemKey(item) === selection.itemKey ? item : null;
}
export function r16NpcSellQuantity(draft: string, count = 1): number | null {
  if (!/^[0-9]+$/.test(draft) || !Number.isSafeInteger(count) || count < 1) return null;
  const value = Number(draft);
  return Number.isSafeInteger(value) && value >= 1 ? Math.min(value, count, 0xffff) : null;
}
export function r16NpcSellTotal(item: R16NpcSellItem, quantity: number | null): number | null {
  if (quantity === null || !Number.isSafeInteger(quantity) || quantity < 1 || !Number.isFinite(item.price) || item.price < 0) return null;
  const total = Math.trunc(item.price) * quantity;
  return Number.isSafeInteger(total) ? total : null;
}
