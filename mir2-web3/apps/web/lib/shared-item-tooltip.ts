/** Read-only bridge to the same Crystal tooltip calculation used by Native. */
export type CrystalTooltipKind = "name" | "attack" | "defence" | "weight" | "awake" | "socket" | "need" | "bind" | "overlap" | "story" | "gmMade";
export type CrystalTooltipColour = "white" | "yellow" | "deepSkyBlue" | "darkOrange" | "plum" | "red" | "cyan" | "darkKhaki" | "khaki" | "orchid";
export type CrystalTooltipDocument = Readonly<{
  sections: readonly Readonly<{kind: CrystalTooltipKind; lines: readonly Readonly<{text: string; colour: CrystalTooltipColour}>[]}>[];
  broken: boolean;
  sourceComplete: boolean;
}>;
export type CrystalTooltipRuntime = {
  getMir2ItemTooltipDocument?: (json: string) => string;
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
}>;

const kinds = new Set<string>(["name", "attack", "defence", "weight", "awake", "socket", "need", "bind", "overlap", "story", "gmMade"]);
const colours = new Set<string>(["white", "yellow", "deepSkyBlue", "darkOrange", "plum", "red", "cyan", "darkKhaki", "khaki", "orchid"]);
const record = (value: unknown): value is Record<string, unknown> => typeof value === "object" && value !== null && !Array.isArray(value);
const integer = (value: unknown, max: number): value is number => typeof value === "number" && Number.isSafeInteger(value) && value >= 0 && value <= max;
const cache = new WeakMap<CrystalTooltipRuntime, Map<string, CrystalTooltipDocument>>();
export type CrystalCatalogInfo = Readonly<{
  itemIndex: number;
  name: string;
  icon: number;
  raw: Readonly<Record<string, unknown>>;
}>;
const catalogCache = new WeakMap<CrystalTooltipRuntime, {
  getter: (json: string) => string;
  entries: Map<number, CrystalCatalogInfo>;
}>();

/** Static display metadata only; instance identity still comes from the server. */
export function readSharedItemCatalogInfo(runtime: CrystalTooltipRuntime | null, itemIndex: number): CrystalCatalogInfo | null {
  const getter = runtime?.getMir2ItemCatalogInfo;
  if (!runtime || typeof getter !== "function" || !integer(itemIndex, 2147483647)) return null;
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
    const raw = response.itemInfo;
    if (raw.item_index !== itemIndex || typeof raw.name !== "string" || !raw.name
      || raw.name.length > 512 || raw.name.includes("\0")) return null;
    const unsignedFields: ReadonlyArray<readonly [string, number]> = [
      ["item_type", 255], ["grade", 255], ["required_type", 255], ["required_class", 255],
      ["required_gender", 255], ["item_set", 255], ["weight", 255], ["light", 255], ["required_amount", 255],
      ["image", 65535], ["durability", 65535], ["stack_size", 65535], ["price", 4294967295],
      ["effect", 255], ["random_stats_id", 255], ["slots", 255],
    ];
    if (unsignedFields.some(([key, max]) => !integer(raw[key], max))) return null;
    for (const key of ["shape", "bind", "unique"]) {
      if (typeof raw[key] !== "number" || !Number.isSafeInteger(raw[key]) || raw[key] < -32768 || raw[key] > 32767) return null;
    }
    const flags = ["start_item", "need_identify", "show_group_pickup", "class_based", "level_based", "can_mine",
      "global_drop_notify", "can_fast_run", "can_awakening"];
    if (flags.some(key => typeof raw[key] !== "boolean")
      || !(raw.tooltip === null || typeof raw.tooltip === "string" && raw.tooltip.length <= 8192 && !raw.tooltip.includes("\0"))
      || !Array.isArray(raw.stats) || raw.stats.length > 256) return null;
    let statBudget = 0;
    const stats = [];
    for (const row of raw.stats) {
      if (!record(row) || Object.keys(row).length !== 2 || !integer(row.stat, 255)
        || typeof row.value !== "number" || !Number.isSafeInteger(row.value) || row.value < -2147483648 || row.value > 2147483647) return null;
      statBudget += Math.abs(row.value);
      if (statBudget > 536870911) return null;
      stats.push(Object.freeze({stat: row.stat, value: row.value}));
    }
    const fields = new Set(["item_index", "name", "shape", "bind", "unique", "tooltip", "stats",
      ...unsignedFields.map(([key]) => key), ...flags]);
    if (Object.keys(raw).length !== fields.size || Object.keys(raw).some(key => !fields.has(key))) return null;
    const result = Object.freeze({itemIndex, name: raw.name, icon: raw.image as number,
      raw: Object.freeze({...raw, stats: Object.freeze(stats)})});
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

/** Missing/older ABI and partial or malformed responses preserve the basic tooltip. */
export function readSharedItemTooltip(runtime: CrystalTooltipRuntime | null, item: CrystalTooltipItem,
  player: Readonly<Record<string, unknown>>, nowMs = Date.now()): CrystalTooltipDocument | null {
  if (typeof runtime?.getMir2ItemTooltipDocument !== "function" || !integer(item.uniqueId, Number.MAX_SAFE_INTEGER)
    || !integer(item.itemIndex, 2147483647) || !integer(item.icon, 65535) || !integer(item.count, 65535) || item.count === 0
    || typeof item.name !== "string" || !item.name || item.name.length > 256 || item.name.includes("\0")
    || !integer(player.level, 4294967295) || !Number.isFinite(nowMs)) return null;
  try {
    const request = JSON.stringify({version: 1, item, player});
    if (request.length > 262144) return null;
    // Expiry labels depend on time. Keep only a small one-second read cache;
    // this never carries item authority or any outgoing command proof.
    const key = `${Math.floor(nowMs / 1000)}:${request}`;
    const entries = cache.get(runtime) ?? new Map<string, CrystalTooltipDocument>();
    cache.set(runtime, entries);
    const existing = entries.get(key);
    if (existing) return existing;
    const text = runtime.getMir2ItemTooltipDocument(request);
    if (typeof text !== "string" || text.length > 262144) return null;
    const response: unknown = JSON.parse(text);
    if (!record(response) || response.version !== 1 || response.ok !== true) return null;
    const document = parseCrystalTooltipDocument(response.document);
    if (!document) return null;
    if (entries.size >= 16) entries.delete(entries.keys().next().value!);
    entries.set(key, document);
    return document;
  } catch { return null; }
}
