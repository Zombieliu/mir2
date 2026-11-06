import type { DisplayEquipmentItem, DisplayItem, EquipmentSlot } from "./original-client-types";
import { originalAssetPath } from "../../lib/asset-url";
import { classifyEquipmentUse } from "../../lib/equipment-gateway-adapter";
import itemLibraryMeta from "../../public/original-ui/Items/meta.json";

const AVAILABLE_ORIGINAL_ITEM_ICONS = new Set(
  itemLibraryMeta.frames.map((frame) => Number(frame.index)),
);
const EMPTY_ORIGINAL_ITEM_ICON =
  "data:image/gif;base64,R0lGODlhAQABAAD/ACwAAAAAAQABAAACADs=";

export function originalItemIconPath(icon: number) {
  const normalizedIcon = Math.trunc(Number(icon));
  // Crystal's Items library contains thousands of sparse/blank frame slots.
  // The export manifest lists only frames with pixels, so do not turn a known
  // blank slot into a browser 404. A transparent pixel preserves the original
  // visual result while keeping genuine exported-frame failures observable.
  return AVAILABLE_ORIGINAL_ITEM_ICONS.has(normalizedIcon)
    ? originalAssetPath(`/original-ui/Items/${normalizedIcon}.png`)
    : EMPTY_ORIGINAL_ITEM_ICON;
}

export function applyOriginalItemIconFallback(image: HTMLImageElement) {
  if (image.dataset.itemIconFallback === "1") {
    image.hidden = true;
    return;
  }
  image.dataset.itemIconFallback = "1";
  image.src = originalItemIconPath(0);
}

export function formatBinaryDateTimeLabel(locale: string, value: number | string, template: string) {
  const decoded = dateFromBinaryDateTime(value);
  if (!decoded) {
    return null;
  }

  const formatted = new Intl.DateTimeFormat(locale, {
    year: "numeric",
    month: "2-digit",
    day: "2-digit",
    hour: "2-digit",
    minute: "2-digit",
    timeZone: decoded.local ? undefined : "UTC",
  }).format(decoded.date);

  return template.replace("{0}", formatted);
}

function dateFromBinaryDateTime(value: number | string) {
  if (typeof value === "number" ? !Number.isSafeInteger(value)
    : typeof value !== "string" || !/^(?:0|[1-9][0-9]{0,18}|-[1-9][0-9]{0,18})$/.test(value)) {
    return null;
  }
  const binary = BigInt(value);
  if (binary === 0n || binary < -9223372036854775808n || binary > 9223372036854775807n) {
    return null;
  }
  // Use the same kind/tick decoding as Page's formatCrystalBinaryDateTime.
  // Only integer milliseconds within the valid .NET calendar reach Number;
  // the received i64 carrier and its sub-millisecond ticks remain untouched.
  const bits = BigInt.asUintN(64, binary);
  const ticks = bits & 0x3fff_ffff_ffff_ffffn;
  if (ticks > 3155378975999999999n) return null;
  const kind = bits & 0xc000_0000_0000_0000n;
  const delta = ticks - 621_355_968_000_000_000n;
  const milliseconds = delta / 10_000n - (delta < 0n && delta % 10_000n !== 0n ? 1n : 0n);
  const date = new Date(Number(milliseconds));
  if (Number.isNaN(date.getTime())) return null;
  return { date, local: kind === 0x8000_0000_0000_0000n || kind === 0xc000_0000_0000_0000n };
}

export function equipmentSlotForItemKey(key: string): EquipmentSlot | null {
  // This is presentation fallback metadata for legacy/string-only items. Live
  // server equipment-slot data should take precedence whenever it is present.
  if (/Sword|Dagger|Blade|Axe|Staff|Wand|Bow|Crossbow|Mace/i.test(key)) return "weapon";
  if (/Helmet|Helm/i.test(key)) return "helmet";
  if (/Armour|Armor|Robe|Dress/i.test(key)) return "armour";
  if (/Necklace|Chain/i.test(key)) return "necklace";
  if (/Bracelet|Bangle/i.test(key)) return "braceletLeft";
  if (/Ring/i.test(key)) return "ringLeft";
  if (/Boots|Shoes/i.test(key)) return "boots";
  if (/Belt/i.test(key)) return "belt";
  if (/Amulet|Poison|Charm/i.test(key)) return "amulet";
  if (/Torch|Candle/i.test(key)) return "torch";
  if (/Horse|Mount/i.test(key)) return "mount";
  return null;
}

const EQUIPMENT_SLOT_NAMES: readonly EquipmentSlot[] = [
  "weapon", "armour", "helmet", "torch", "necklace", "braceletLeft", "braceletRight",
  "ringLeft", "ringRight", "amulet", "belt", "boots", "stone", "mount",
];

type InventoryEquipmentSource = Pick<DisplayItem, "key" | "name" | "equipSlot"> & {
  tooltipSource?: unknown;
};

export function equipmentSlotForItem(
  item: InventoryEquipmentSource,
  equipmentItems: readonly Pick<DisplayEquipmentItem, "slot">[] = [],
): EquipmentSlot | null {
  const classification = classifyEquipmentUse(item, equipmentItems);
  if (classification.kind === "equipment") return EQUIPMENT_SLOT_NAMES[classification.to] ?? null;
  if (classification.kind === "ordinary") return null;
  const fallback = equipmentSlotForItemKey(`${item.key} ${item.name}`);
  if (!fallback) return null;
  const legacy = classifyEquipmentUse({ equipSlot: fallback }, equipmentItems);
  return legacy.kind === "equipment" ? EQUIPMENT_SLOT_NAMES[legacy.to] ?? null : null;
}

export function inventoryItemCanStack(item: InventoryEquipmentSource): boolean {
  if (equipmentSlotForItem(item)) return false;
  const source = item.tooltipSource as { info?: { stack_size?: unknown } } | null | undefined;
  const stackSize = source?.info?.stack_size;
  return typeof stackSize !== "number" || (Number.isInteger(stackSize) && stackSize > 1);
}

export function inventoryItemCanSplit(item: InventoryEquipmentSource & Pick<DisplayItem, "quantity">): boolean {
  return Number.isInteger(item.quantity) && item.quantity > 1 && inventoryItemCanStack(item);
}
