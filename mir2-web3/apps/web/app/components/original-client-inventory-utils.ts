import type { EquipmentSlot } from "./original-client-types";

export function originalItemIconPath(icon: number) {
  return `/original-ui/Items/${icon}.png`;
}

export function formatBinaryDateTimeLabel(locale: string, value: number, template: string) {
  const date = dateFromBinaryDateTime(value);
  if (!date) {
    return null;
  }

  const formatted = new Intl.DateTimeFormat(locale, {
    year: "numeric",
    month: "2-digit",
    day: "2-digit",
    hour: "2-digit",
    minute: "2-digit",
  }).format(date);

  return template.replace("{0}", formatted);
}

function dateFromBinaryDateTime(value: number) {
  if (!Number.isFinite(value) || value <= 0) {
    return null;
  }

  // Crystal serializes .NET DateTime ticks (100 ns since 0001-01-01), whereas
  // JavaScript Date expects milliseconds since the Unix epoch.
  const ticksFromUnixEpoch = value - 621355968000000000;
  if (ticksFromUnixEpoch <= 0) {
    return null;
  }

  return new Date(Math.floor(ticksFromUnixEpoch / 10000));
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
