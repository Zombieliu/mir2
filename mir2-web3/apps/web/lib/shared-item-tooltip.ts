import { parseCrystalItemInfo, type CrystalCatalogInfo } from "./crystal-item-source";
export type { CrystalCatalogInfo } from "./crystal-item-source";

/** Read-only bridge to the same Crystal tooltip calculation used by Native. */
export type CrystalTooltipKind = "name" | "attack" | "defence" | "weight" | "awake" | "socket" | "need" | "bind" | "overlap" | "story" | "gmMade";
export type CrystalTooltipColour = "white" | "yellow" | "deepSkyBlue" | "darkOrange" | "plum" | "red" | "cyan" | "darkKhaki" | "khaki" | "orchid";
export type CrystalTooltipDocument = Readonly<{
  sections: readonly Readonly<{kind: CrystalTooltipKind; lines: readonly Readonly<{text: string; colour: CrystalTooltipColour}>[]}>[];
  broken: boolean;
  sourceComplete: boolean;
}>;
export type CrystalTooltipRuntime = {
  getMir2ItemTooltipDocument?: (json: string, nowDotnetTicks: string) => string;
  getMir2ItemCatalogInfo?: (json: string) => string;
};
export type CrystalTooltipItem = Readonly<{
  uniqueId: number;
  itemIndex: number;
  name: string;
  icon: number;
  count: number;
  tooltipSource?: Readonly<Record<string, unknown>>;
  sourceKind?: "instance" | "catalogInstance" | "catalogPreview";
  legacy?: Readonly<{key?: string; grade?: string | null; description?: string; sellValue?: number;
    durabilityCurrent?: number | null; durabilityMax?: number | null; attack?: number; defence?: number;
    addedAttack?: number; addedDefence?: number; addedLuck?: number; socketSlots?: number}>;
}>;

const kinds = new Set<string>(["name", "attack", "defence", "weight", "awake", "socket", "need", "bind", "overlap", "story", "gmMade"]);
const colours = new Set<string>(["white", "yellow", "deepSkyBlue", "darkOrange", "plum", "red", "cyan", "darkKhaki", "khaki", "orchid"]);
const record = (value: unknown): value is Record<string, unknown> => typeof value === "object" && value !== null && !Array.isArray(value);
const integer = (value: unknown, max: number): value is number => typeof value === "number" && Number.isSafeInteger(value) && value >= 0 && value <= max;
const signedI32 = (value: unknown): value is number => typeof value === "number" && Number.isSafeInteger(value)
  && value >= -2147483648 && value <= 2147483647;
const cache = new WeakMap<CrystalTooltipRuntime, {getter: NonNullable<CrystalTooltipRuntime["getMir2ItemTooltipDocument"]>;
  entries: Map<string, CrystalTooltipDocument>}>();
const catalogCache = new WeakMap<CrystalTooltipRuntime, {
  getter: (json: string) => string;
  entries: Map<number, CrystalCatalogInfo>;
}>();

/** Static display metadata only; instance identity still comes from the server. */
export function readSharedItemCatalogInfo(runtime: CrystalTooltipRuntime | null, itemIndex: number): CrystalCatalogInfo | null {
  const getter = runtime?.getMir2ItemCatalogInfo;
  if (!runtime || typeof getter !== "function" || !signedI32(itemIndex)) return null;
  try {
    let cached = catalogCache.get(runtime);
    if (!cached || cached.getter !== getter) {
      cached = {getter, entries: new Map()};
      catalogCache.set(runtime, cached);
    }
    const existing = cached.entries.get(itemIndex);
    if (existing) return existing;
    const text = getter.call(runtime, JSON.stringify({version: 1, itemIndex}));
    if (typeof text !== "string" || text.length > 65536) return null;
    const response: unknown = JSON.parse(text);
    if (!record(response) || response.version !== 1 || response.ok !== true || !record(response.itemInfo)) return null;
    const result = parseCrystalItemInfo(response.itemInfo, itemIndex);
    if (!result) return null;
    if (cached.entries.size >= 512) cached.entries.delete(cached.entries.keys().next().value!);
    cached.entries.set(itemIndex, result);
    return result;
  } catch { return null; }
}

export function parseCrystalTooltipDocument(value: unknown): CrystalTooltipDocument | null {
  if (!record(value) || typeof value.broken !== "boolean" || typeof value.sourceComplete !== "boolean"
    || !Array.isArray(value.sections) || value.sections.length > 11) return null;
  const sections: Array<{kind: CrystalTooltipKind; lines: readonly {text: string; colour: CrystalTooltipColour}[]}> = [];
  const seen = new Set<string>();
  let textLength = 0, previousKind = -1;
  const order = [...kinds];
  for (const section of value.sections) {
    if (!record(section) || typeof section.kind !== "string" || !kinds.has(section.kind) || seen.has(section.kind)
      || !Array.isArray(section.lines) || section.lines.length > 512) return null;
    const index = order.indexOf(section.kind);
    if (index <= previousKind) return null;
    previousKind = index; seen.add(section.kind);
    const lines: Array<{text: string; colour: CrystalTooltipColour}> = [];
    for (const line of section.lines) {
      if (!record(line) || typeof line.text !== "string" || line.text.length > 8192 || line.text.includes("\0")
        || typeof line.colour !== "string" || !colours.has(line.colour)) return null;
      textLength += line.text.length;
      if (textLength > 65536) return null;
      lines.push(Object.freeze({text: line.text, colour: line.colour as CrystalTooltipColour}));
    }
    sections.push(Object.freeze({kind: section.kind as CrystalTooltipKind, lines: Object.freeze(lines)}));
  }
  return Object.freeze({sections: Object.freeze(sections), broken: value.broken, sourceComplete: value.sourceComplete});
}

/** Exact UTC milliseconds become canonical .NET ticks without a wide Number. */
export function crystalTooltipDotnetTicks(nowMs: number): string | null {
  if (!Number.isSafeInteger(nowMs)) return null;
  const ticks = BigInt(nowMs) * 10000n + 621355968000000000n;
  return ticks >= -9223372036854775808n && ticks <= 9223372036854775807n ? ticks.toString() : null;
}

/** Missing/older ABI and partial or malformed responses preserve the basic tooltip. */
export function readSharedItemTooltip(runtime: CrystalTooltipRuntime | null, item: CrystalTooltipItem,
  player: Readonly<Record<string, unknown>>, nowMs = Date.now()): CrystalTooltipDocument | null {
  const getter = runtime?.getMir2ItemTooltipDocument, clock = crystalTooltipDotnetTicks(nowMs);
  if (!runtime || typeof getter !== "function" || clock === null || !integer(item.uniqueId, Number.MAX_SAFE_INTEGER)
    || !signedI32(item.itemIndex) || !integer(item.icon, 65535) || !integer(item.count, 65535) || item.count === 0
    || typeof item.name !== "string" || !item.name || item.name.length > 512 || item.name.includes("\0")
    || !integer(player.level, 4294967295)) return null;
  try {
    const request = JSON.stringify({version: 1, item, player});
    if (request.length > 262144) return null;
    let cached = cache.get(runtime);
    if (!cached || cached.getter !== getter) { cached = {getter, entries: new Map()}; cache.set(runtime, cached); }
    // Time is actual formatter input. The old one-argument renderer getter can
    // ignore this additive argument while keeping its existing root schema.
    const key = clock + ":" + request, existing = cached.entries.get(key);
    if (existing) return existing;
    const text = getter.call(runtime, request, clock);
    if (runtime.getMir2ItemTooltipDocument !== getter || JSON.stringify({version: 1, item, player}) !== request
      || typeof text !== "string" || text.length > 262144) return null;
    const response: unknown = JSON.parse(text);
    if (!record(response) || response.version !== 1 || response.ok !== true) return null;
    const document = parseCrystalTooltipDocument(response.document);
    if (!document) return null;
    if (cached.entries.size >= 16) cached.entries.delete(cached.entries.keys().next().value!);
    cached.entries.set(key, document);
    return document;
  } catch { return null; }
}
