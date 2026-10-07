/** Complete raw Crystal carriers. Validation never supplies protocol defaults. */
export type CrystalCatalogInfo = Readonly<{
  itemIndex: number;
  name: string;
  icon: number;
  raw: Readonly<Record<string, unknown>>;
}>;

const record = (value: unknown): value is Record<string, unknown> => {
  if (typeof value !== "object" || value === null || Array.isArray(value)) return false;
  const prototype = Object.getPrototypeOf(value);
  return (prototype === Object.prototype || prototype === null) && Object.getOwnPropertySymbols(value).length === 0
    && Object.values(Object.getOwnPropertyDescriptors(value)).every(descriptor => descriptor.enumerable && "value" in descriptor);
};
const integer = (value: unknown, max: number): value is number => typeof value === "number" && Number.isSafeInteger(value) && value >= 0 && value <= max;
const signedI32 = (value: unknown): value is number => typeof value === "number" && Number.isSafeInteger(value)
  && value >= -2147483648 && value <= 2147483647;
const utf8Length = (value: string): number => new TextEncoder().encode(value).length;
const exactKeys = (value: Record<string, unknown>, keys: readonly string[]): boolean =>
  Object.keys(value).length === keys.length && Object.keys(value).every(key => keys.includes(key));
const boundedString = (value: unknown, max: number): value is string => typeof value === "string"
  && utf8Length(value) <= max && !value.includes("\0");

type StatBudget = { total: number };
/** Same 32-field metadata validator and immutable output as the shared getter. */
export function parseCrystalItemInfo(value: unknown, itemIndex: number, budget?: StatBudget): CrystalCatalogInfo | null {
  if (!record(value) || !signedI32(itemIndex)) return null;
    const raw = value;
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
    let statBudget = budget?.total ?? 0;
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

  if (budget) budget.total = statBudget;
  return result;
}

/** Literal data only; whole catalogues may select the separate 2 MiB ceiling. */
export function crystalItemSourceJson(value: unknown, maxBytes = 262144, maxNodes = 8192, maxArray = 256, allowRootRateNegativeZero = false): string {
  const ancestors = new Set<object>();
  let nodes = 0;
  function visit(v: unknown, depth: number, key?: string): void {
    if (++nodes > maxNodes || depth > 16) throw Error("Crystal JSON limit");
    if (v === null || typeof v === "boolean") return;
    if (typeof v === "string") { if (utf8Length(v) > 16384) throw Error("Crystal string limit"); return; }
    if (typeof v === "number" && Number.isFinite(v)) {
      // Only the typed root f32 rate treats -0 as ordinary zero. Integer/date
      // facts cannot borrow that exception at another key or nesting level.
      if (Object.is(v, -0) && !(allowRootRateNegativeZero && depth === 1 && key === "rate")) throw Error("Negative-zero Crystal fact");
      return;
    }
    if (!v || typeof v !== "object") throw Error("Non-JSON Crystal source");
    const prototype = Object.getPrototypeOf(v), array = Array.isArray(v);
    if ((array ? prototype !== Array.prototype : prototype !== Object.prototype && prototype !== null)
      || ancestors.has(v) || "toJSON" in v || Object.getOwnPropertySymbols(v).length) throw Error("Non-plain Crystal source");
    const descriptors = Object.getOwnPropertyDescriptors(v), keys = Object.keys(descriptors);
    if (array ? v.length > maxArray : keys.length > 64) throw Error("Crystal collection limit");
    ancestors.add(v);
    for (const key of keys) {
      if (array && key === "length") continue;
      const descriptor = descriptors[key];
      if (!descriptor.enumerable || !("value" in descriptor) || utf8Length(key) > 128) throw Error("Non-data Crystal source");
      if (array && (!/^(0|[1-9][0-9]*)$/.test(key) || Number(key) >= v.length)) throw Error("Non-index Crystal array");
      visit(descriptor.value, depth + 1, key);
    }
    if (array && Object.keys(v).length !== v.length) throw Error("Sparse Crystal source");
    ancestors.delete(v);
  }
  visit(value, 0);
  const json = JSON.stringify(value);
  if (typeof json !== "string" || utf8Length(json) > maxBytes) throw Error("Crystal JSON size");
  return json;
}

function deepFreeze(value: unknown): void {
  if (!value || typeof value !== "object") return;
  for (const child of Object.values(value)) deepFreeze(child);
  Object.freeze(value);
}

export const crystalUserItemFields = Object.freeze(["unique_id", "item_index", "current_dura", "max_dura", "count",
  "soul_bound_id", "identified", "cursed", "slots", "gem_count", "added_stats", "awake_type", "awake_values",
  "refined_value", "refine_added", "refine_success_chance", "wedding_ring", "expire_info", "rental_information",
  "is_shop_item", "sealed_info", "gm_made"] as const);

type UserValidation = { total: number; nodes: number; ids: Set<number> };
function stats(value: unknown, budget: StatBudget): boolean {
  if (!Array.isArray(value) || value.length > 256) return false;
  for (const row of value) {
    if (!record(row) || !exactKeys(row, ["stat", "value"]) || !integer(row.stat, 255) || !signedI32(row.value)) return false;
    budget.total += Math.abs(row.value);
    if (budget.total > 536870911) return false;
  }
  return true;
}
function binaryDate(value: unknown): boolean {
  if (typeof value === "number") return Number.isSafeInteger(value) && !Object.is(value, -0);
  if (typeof value !== "string" || value.length > 20 || !/^(0|-?[1-9][0-9]*)$/.test(value)) return false;
  const ticks = BigInt(value);
  return ticks >= -9223372036854775808n && ticks <= 9223372036854775807n;
}
function dates(value: unknown, kind: "expire_info" | "rental_information" | "sealed_info"): boolean {
  if (value === null) return true;
  const keys = kind === "expire_info" ? ["expiry_binary_datetime"] : kind === "sealed_info"
    ? ["expiry_binary_datetime", "next_seal_binary_datetime"] : ["owner_name", "binding_flags", "expiry_binary_datetime", "rental_locked"];
  if (!record(value) || !exactKeys(value, keys) || !binaryDate(value.expiry_binary_datetime)) return false;
  if (kind === "sealed_info" && !binaryDate(value.next_seal_binary_datetime)) return false;
  return kind !== "rental_information" || boundedString(value.owner_name, 512)
    && typeof value.binding_flags === "number" && Number.isSafeInteger(value.binding_flags)
    && value.binding_flags >= -32768 && value.binding_flags <= 32767 && typeof value.rental_locked === "boolean";
}
function userItem(value: unknown, state: UserValidation, depth: number): value is Record<string, unknown> {
  if (++state.nodes > 64 || depth > 4 || !record(value) || !exactKeys(value, crystalUserItemFields)
    || !integer(value.unique_id, Number.MAX_SAFE_INTEGER) || state.ids.has(value.unique_id)) return false;
  state.ids.add(value.unique_id);
  for (const [key, max] of [["current_dura",65535],["max_dura",65535],["count",65535],["gem_count",65535],
    ["awake_type",255],["refined_value",255],["refine_added",255]] as const) if (!integer(value[key], max)) return false;
  if (value.count === 0) return false;
  for (const key of ["item_index","soul_bound_id","refine_success_chance","wedding_ring"]) if (!signedI32(value[key])) return false;
  for (const key of ["identified","cursed","is_shop_item","gm_made"]) if (typeof value[key] !== "boolean") return false;
  if (!stats(value.added_stats, state) || !Array.isArray(value.awake_values) || value.awake_values.length > 32
    || value.awake_values.some(v => !integer(v, 255)) || !Array.isArray(value.slots) || value.slots.length > 32) return false;
  for (const child of value.slots) if (child !== null && !userItem(child, state, depth + 1)) return false;
  return dates(value.expire_info, "expire_info") && dates(value.rental_information, "rental_information") && dates(value.sealed_info, "sealed_info");
}
function strictInfo(value: unknown, budget: StatBudget): value is Record<string, unknown> {
  return record(value) && signedI32(value.item_index) && boundedString(value.name, 512)
    && (value.tooltip === null || boundedString(value.tooltip, 8192)) && !!parseCrystalItemInfo(value, value.item_index, budget);
}

/** All raw facts must be present. Unknown carriers do not become default items. */
export function parseCrystalItemTooltipSource(value: unknown, identity: Readonly<{uniqueId: number; itemIndex: number; count: number}>): Readonly<Record<string, unknown>> | null {
  try {
    const raw: unknown = JSON.parse(crystalItemSourceJson(value));
    if (!record(raw) || !exactKeys(raw, ["info","realInfo","userItem","socketInfos","realSocketInfos"])) return null;
    const state: UserValidation = { total: 0, nodes: 0, ids: new Set() };
    if (!strictInfo(raw.info, state) || raw.info.item_index !== identity.itemIndex || !userItem(raw.userItem, state, 0)
      || raw.userItem.unique_id !== identity.uniqueId || raw.userItem.item_index !== identity.itemIndex
      || raw.userItem.count !== identity.count || !(raw.realInfo === null || strictInfo(raw.realInfo, state))
      || !Array.isArray(raw.socketInfos) || !Array.isArray(raw.realSocketInfos)
      || raw.socketInfos.length > (raw.userItem.slots as unknown[]).length
      || raw.realSocketInfos.length > (raw.userItem.slots as unknown[]).length) return null;
    const slots = raw.userItem.slots as unknown[];
    for (let index = 0; index < slots.length; ++index) {
      const slot = slots[index], base = raw.socketInfos[index], real = raw.realSocketInfos[index];
      if (slot === null) {
        if (base !== undefined && base !== null || real !== undefined && real !== null) return null;
      } else if (base !== undefined && base !== null
        && (!record(slot) || !strictInfo(base, state) || base.item_index !== slot.item_index)) return null;
      if (real !== undefined && real !== null && !strictInfo(real, state)) return null;
      // Missing templates stay absent/null. The shared formatter reports their
      // partial source; they never become manufactured physical socket facts.
    }
    deepFreeze(raw);
    return raw;
  } catch { return null; }
}

/** Compare already validated literal carriers without depending on key order. */
export function sameCrystalItemData(left: unknown, right: unknown): boolean {
  if (left === right) return true;
  if (Array.isArray(left) && Array.isArray(right)) return left.length === right.length && left.every((value, index) => sameCrystalItemData(value, right[index]));
  if (!record(left) || !record(right)) return false;
  const keys = Object.keys(left);
  return keys.length === Object.keys(right).length && keys.every(key => Object.prototype.hasOwnProperty.call(right, key) && sameCrystalItemData(left[key], right[key]));
}
