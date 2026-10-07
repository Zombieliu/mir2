import type { NpcPurchaseReceiptRuntime } from "./npc-purchase-receipt";

type Data = Record<string, unknown>;
export type NpcPurchaseRequest = Readonly<{ itemIndex: string; count: number; panelType: number }>;
type Binding = Readonly<{ actor: string; producerScope: string; beginId: string }>;
type Producer = Readonly<{ actor: string; producerScope: string; serverRevision: string }>;
type Intent = Readonly<{ request: NpcPurchaseRequest; currency: "gold" | "pearls";
  source: "trade" | "buyBack" | "used"; serviceCatalogProof: string }>;
type Operation = Readonly<{ actor: string; requestScope: string; sequence: string; intent: Intent }>;
const record = (v: unknown): v is Data => typeof v === "object" && v !== null && !Array.isArray(v);
const fields = (v: unknown, names: string[]): v is Data => record(v)
  && Object.keys(v).sort().join() === names.slice().sort().join();
const integer = (v: unknown, lo: number, hi: number): v is number => Number.isSafeInteger(v) && Number(v) >= lo && Number(v) <= hi;
export const npcPurchaseDecimal = (v: unknown): v is string => typeof v === "string"
  && /^(?:0|[1-9][0-9]{0,19})$/.test(v) && BigInt(v) <= 18446744073709551615n;
const nonzero = (v: unknown): v is string => npcPurchaseDecimal(v) && v !== "0";
export const npcPurchaseSignedDecimal = (v: unknown): v is string => typeof v === "string"
  && /^(?:0|[1-9][0-9]{0,18}|-[1-9][0-9]{0,18})$/.test(v)
  && BigInt(v) >= -9223372036854775808n && BigInt(v) <= 9223372036854775807n;
/** Only declared integer leaves of the actual owner WorldSnapshot may carry
 * lossless decimal text. A similarly named unknown field grants no identity. */
function ownerIntegerKind(path: readonly (string | number)[]): "u64" | "i64" | null {
  const key = path.map(part => typeof part === "number" ? "*" : encodeURIComponent(part).replace(/\*/g, "%2A")).join("/");
  if (/^snapshot\/(?:belt|inventory|storage|equipment|heroInventory|heroEquipment)Items\/\*\/uniqueId$/.test(key)
    || /^snapshot\/(?:belt|inventory|storage|equipment|heroInventory|heroEquipment)Items\/\*\/tooltipSource\/userItem(?:\/slots\/\*)*\/unique_id$/.test(key)
    || /^snapshot\/nativeNpcShop\/list\/\*(?:\/slots\/\*)*\/unique_id$/.test(key)
    || /^snapshot\/nativeNpcShop\/displayGoods\/\*\/(?:id|uniqueId|unique_id)$/.test(key)
    || /^snapshot\/nativeNpcShop\/displayGoods\/\*(?:\/slots\/\*)+\/unique_id$/.test(key)
    || /^snapshot\/nativeNpcShop\/displayGoods\/\*\/tooltipSource\/userItem(?:\/slots\/\*)*\/unique_id$/.test(key)
    || key === "snapshot/npcGoldTradeCapacity/freshCompatibleUniqueIds/*"
    || /^snapshot\/stage5Systems\/itemRental\/rentedItems\/\*\/itemId$/.test(key)
    || /^snapshot\/stage5Systems\/trade\/offeredUniqueIds\/[^/]+$/.test(key)
    || key === "snapshot/stage5Systems/refine/pendingUniqueId"
    || /^snapshot\/mailItemState(?:\/socketed\/\*)*\/unique_id$/.test(key)
    || /^snapshot\/mailItemState(?:\/socketed\/\*)*\/user_item_metadata\/slots\/\*(?:\/slots\/\*)*\/unique_id$/.test(key)) return "u64";
  if (/^snapshot\/(?:storagePasswordLastSetBinaryDatetime|expandedStorageExpiryTimeBinaryDatetime)$/.test(key)
    || key === "snapshot/heroMaxExperience" || key === "snapshot/stage5Systems/hero/experience"
    || /^snapshot\/equipmentItems\/\*\/(?:sealedExpiryTimeBinaryDatetime|sealedNextTimeBinaryDatetime)$/.test(key)
    || /^snapshot\/(?:belt|inventory|storage|equipment|heroInventory|heroEquipment)Items\/\*\/tooltipSource\/userItem(?:\/slots\/\*)*\/(?:expire_info|rental_information|sealed_info)\/(?:expiry_binary_datetime|next_seal_binary_datetime)$/.test(key)
    || /^snapshot\/nativeNpcShop\/list\/\*(?:\/slots\/\*)*\/(?:expire_info|rental_information|sealed_info)\/(?:expiry_binary_datetime|next_seal_binary_datetime)$/.test(key)
    || /^snapshot\/nativeNpcShop\/displayGoods\/\*(?:\/slots\/\*)*\/(?:expire_info|rental_information|sealed_info)\/(?:expiry_binary_datetime|next_seal_binary_datetime)$/.test(key)
    || /^snapshot\/nativeNpcShop\/displayGoods\/\*\/tooltipSource\/userItem(?:\/slots\/\*)*\/(?:expire_info|rental_information|sealed_info)\/(?:expiry_binary_datetime|next_seal_binary_datetime)$/.test(key)
    || key === "snapshot/stage5Systems/relationship/marriedDateBinaryDatetime"
    || /^snapshot\/stage5Systems\/mail\/\*\/(?:dateSentBinaryDatetime|date_sent_binary_datetime)$/.test(key)
    || /^snapshot\/stage5Systems\/intelligentCreatures\/\*\/(?:expireBinaryDatetime|blackstoneTime|maintainFoodTime)$/.test(key)
    || /^snapshot\/stage5Systems\/itemRental\/rentedItems\/\*\/itemReturnDateBinaryDatetime$/.test(key)
    || /^snapshot\/mailItemState(?:\/socketed\/\*)*\/(?:sealed_expiry_time_binary_datetime|sealed_next_time_binary_datetime|rental_expiry_binary_datetime)$/.test(key)
    || /^snapshot\/mailItemState(?:\/socketed\/\*)*\/user_item_metadata(?:\/slots\/\*(?:\/slots\/\*)*)?\/(?:expire_info|rental_information|sealed_info)\/(?:expiry_binary_datetime|next_seal_binary_datetime)$/.test(key)) return "i64";
  return null;
}
const opaque = (v: unknown): v is string => typeof v === "string" && /^[0-9a-f]{64}$/.test(v) && /[1-9a-f]/.test(v);
const exactUnsigned = (v: unknown): boolean => integer(v, 0, Number.MAX_SAFE_INTEGER) || npcPurchaseDecimal(v);
const exactSigned = (v: unknown): boolean => integer(v, Number.MIN_SAFE_INTEGER, Number.MAX_SAFE_INTEGER) || npcPurchaseSignedDecimal(v);
function exactSnapshotLeaves(value: unknown, path: readonly (string | number)[]): boolean {
  const kind = ownerIntegerKind(path);
  if (path.length === 2 && path[0] === "snapshot" && path[1] === "heroMaxExperience" && value === null) return true;
  if (kind) return kind === "u64" ? exactUnsigned(value) : exactSigned(value);
  if (Array.isArray(value)) return value.every((child, index) => exactSnapshotLeaves(child, [...path, index]));
  return !record(value) || Object.entries(value).every(([key, child]) => exactSnapshotLeaves(child, [...path, key]));
}
function plain(value: unknown, depth = 0, originalSnapshot?: unknown, path: readonly string[] = []): unknown {
  if (depth > 64) throw Error("Deep NPC decision");
  if (value === null || ["string", "boolean"].includes(typeof value)) return value;
  if (typeof value === "number" && Number.isFinite(value) && (!Number.isInteger(value) || Number.isSafeInteger(value))) return value;
  if (typeof value !== "object") throw Error("Invalid NPC decision value");
  const array = Array.isArray(value), proto = Object.getPrototypeOf(value);
  if (proto !== null && proto !== (array ? Array.prototype : Object.prototype)) throw Error("Nonplain NPC decision");
  const descriptors = Object.getOwnPropertyDescriptors(value), result: Data | unknown[] = array ? [] : Object.create(null);
  let count = 0;
  for (const key of Reflect.ownKeys(descriptors).sort((a, b) => String(a).localeCompare(String(b)))) {
    if (typeof key !== "string" || ["toJSON", "__proto__", "constructor", "prototype"].includes(key)) throw Error("Unsafe NPC decision field");
    const descriptor = descriptors[key];
    if (!Object.hasOwn(descriptor, "value")) throw Error("Accessor NPC decision");
    if (array && key === "length") continue;
    if (!descriptor.enumerable || array && !/^(?:0|[1-9][0-9]*)$/.test(key)) throw Error("Invalid NPC decision fields");
    count++;
    (result as Data)[key] = originalSnapshot !== undefined && path.length === 1 && path[0] === "frame" && key === "snapshot"
      ? quarantineSnapshot(descriptor.value, originalSnapshot, ["snapshot"], depth + 1)
      : plain(descriptor.value, depth + 1, originalSnapshot, [...path, key]);
  }
  if (array && count !== descriptors.length.value) throw Error("Sparse NPC decision");
  return Object.freeze(result);
}
/** A retained pre-Source31 facade may already have decoded this DTO. Its
 * rounded projection is compared, never used as the economic source. Only
 * declared integer leaves may differ in representation; every other field,
 * primitive, key, descriptor and shape must match the original accepted raw. */
function quarantineSnapshot(projected: unknown, exact: unknown, path: readonly (string | number)[], depth: number): unknown {
  if (depth > 64) throw Error("Deep old NPC projection");
  if (typeof projected === "number" && Number.isInteger(projected) && !Number.isSafeInteger(projected)) {
    const kind = ownerIntegerKind(path);
    if (typeof exact !== "string" || !(kind === "u64" ? npcPurchaseDecimal(exact) : kind === "i64" && npcPurchaseSignedDecimal(exact))
      || !Number.isFinite(projected) || Number(exact) !== projected) throw Error("Mismatched old NPC integral projection");
    return exact;
  }
  if (projected === null || typeof projected !== "object") {
    if (projected !== exact) throw Error("Mismatched old NPC projection");
    return plain(projected);
  }
  const array = Array.isArray(projected), proto = Object.getPrototypeOf(projected);
  if (exact === null || typeof exact !== "object" || array !== Array.isArray(exact)
    || proto !== null && proto !== (array ? Array.prototype : Object.prototype)) throw Error("Invalid old NPC projection shape");
  const descriptors = Object.getOwnPropertyDescriptors(projected), keys = Reflect.ownKeys(descriptors);
  const exactKeys = Reflect.ownKeys(exact);
  if (keys.length !== exactKeys.length || keys.some(key => typeof key !== "string" || !exactKeys.includes(key))) throw Error("Mismatched old NPC projection keys");
  for (const key of keys) {
    if (typeof key !== "string" || ["toJSON", "__proto__", "constructor", "prototype"].includes(key)) throw Error("Unsafe old NPC projection key");
    const descriptor = descriptors[key];
    if (!Object.hasOwn(descriptor, "value") || key !== "length" && !descriptor.enumerable) throw Error("Accessor old NPC projection");
    if (array && key === "length") {
      if (descriptor.value !== (exact as unknown[]).length) throw Error("Mismatched old NPC projection array");
      continue;
    }
    if (array && !/^(?:0|[1-9][0-9]*)$/.test(key)) throw Error("Invalid old NPC projection array key");
    quarantineSnapshot(descriptor.value, (exact as Data)[key], [...path, array ? Number(key) : key], depth + 1);
  }
  return exact;
}
const same = (a: unknown, b: unknown): boolean => JSON.stringify(plain(a)) === JSON.stringify(plain(b));
export const sameNpcPurchaseData = (a: unknown, b: unknown): boolean => {
  try { return same(a, b); } catch { return false; }
};
/** The display projection belongs to the same actual list/profile. It cannot
 * substitute a different UserItem or manufacture a selector from UI indices. */
export function npcPurchaseShopSource(value: unknown, requireDisplay = true): Data | null {
  try {
    const shop = plain(value);
    const names = ["npcObjectId", "scriptKey", "service", "packetType", "list", "rate", "panelType", "hideAddedStats"];
    if (!fields(shop, requireDisplay || record(shop) && Object.hasOwn(shop, "displayGoods") ? [...names, "displayGoods"] : names)
      || !integer(shop.npcObjectId, 1, 0xffff_ffff) || typeof shop.scriptKey !== "string" || shop.scriptKey.length === 0
      || !["BUY", "BUYSELL", "BUYBACK", "BUYUSED", "PEARLBUY", "BUYNEW", "BUYSELLNEW"].includes(String(shop.service))
      || shop.packetType !== (shop.service === "PEARLBUY" ? "NPCPearlGoods" : "NPCGoods")
      || !Array.isArray(shop.list) || requireDisplay && !Array.isArray(shop.displayGoods)
      || shop.displayGoods !== undefined && (!Array.isArray(shop.displayGoods) || shop.list.length !== shop.displayGoods.length)
      || typeof shop.rate !== "number" || !Number.isFinite(shop.rate) || !Number.isFinite(Math.fround(shop.rate)) || shop.rate < 0
      || !integer(shop.panelType, 0, 255) || shop.panelType !== (shop.service === "BUYUSED" ? 1 : 0)
      || typeof shop.hideAddedStats !== "boolean" || !exactSnapshotLeaves(shop, ["snapshot", "nativeNpcShop"])) return null;
    const ids = new Set<string>();
    for (let index = 0; index < shop.list.length; index++) {
      const item = shop.list[index];
      if (!record(item) || !exactUnsigned(item.unique_id) || ids.has(String(item.unique_id))) return null;
      ids.add(String(item.unique_id));
      if (!Array.isArray(shop.displayGoods)) continue;
      const row = shop.displayGoods[index];
      if (!record(row)
        || Object.keys(row).some(key => !Object.hasOwn(item, key)
          && !["id", "uniqueId", "purchaseItemIndex", "itemIndex", "name", "icon", "price", "tooltipSource", "grade", "description"].includes(key))
        || Object.keys(item).some(key => !Object.hasOwn(row, key) || !same(row[key], item[key]))
        || !exactUnsigned(row.id) || !exactUnsigned(row.uniqueId)
        || String(row.id) !== String(item.unique_id) || String(row.uniqueId) !== String(item.unique_id)
        || !npcPurchaseDecimal(row.purchaseItemIndex) || row.purchaseItemIndex !== String(item.unique_id)
        || row.itemIndex !== item.item_index || typeof row.name !== "string"
        || row.description !== undefined && typeof row.description !== "string"
        || !integer(row.icon, 0, 65535) || !integer(row.price, 0, 0xffff_ffff) || !integer(row.count, 1, 65535)) return null;
      if (row.tooltipSource === undefined && !requireDisplay) continue;
      if (!record(row.tooltipSource) || !record(row.tooltipSource.info) || !same(row.tooltipSource.userItem, item)
        || row.tooltipSource.info.item_index !== item.item_index || row.name !== row.tooltipSource.info.name
        || row.grade !== row.tooltipSource.info.grade || !integer(row.grade, 0, 255)) return null;
    }
    return shop;
  } catch { return null; }
}
const requestValid = (v: unknown): v is NpcPurchaseRequest => fields(v, ["itemIndex", "count", "panelType"])
  && npcPurchaseDecimal(v.itemIndex) && integer(v.count, 1, 65535) && integer(v.panelType, 0, 255);
const bindingValid = (v: unknown): v is Binding => fields(v, ["actor", "producerScope", "beginId"])
  && opaque(v.actor) && opaque(v.producerScope) && nonzero(v.beginId);
const producerValid = (v: unknown): v is Producer => fields(v, ["actor", "producerScope", "serverRevision"])
  && opaque(v.actor) && opaque(v.producerScope) && npcPurchaseDecimal(v.serverRevision) && v.serverRevision !== "18446744073709551615";
const intentValid = (v: unknown): v is Intent => fields(v, ["request", "currency", "source", "serviceCatalogProof"])
  && requestValid(v.request) && ["gold", "pearls"].includes(String(v.currency))
  && ["trade", "buyBack", "used"].includes(String(v.source)) && opaque(v.serviceCatalogProof);
const operationValid = (v: unknown): v is Operation => fields(v, ["actor", "requestScope", "sequence", "intent"])
  && opaque(v.actor) && opaque(v.requestScope) && nonzero(v.sequence) && intentValid(v.intent);

/** Parse original JSON before Number conversion. Only explicitly declared owner
 * integer leaves gain the canonical exact-string projection. */
export function parseNpcPurchaseJson(raw: string, ownerProjection: boolean | "abi" | "world" = false): unknown {
  if (typeof raw !== "string" || raw.length === 0 || raw.length > 16 * 1024 * 1024) throw Error("Invalid NPC JSON size");
  let offset = 0;
  const ws = () => { while (/[\t\n\r ]/.test(raw[offset] ?? "!")) offset++; };
  const string = (): string => {
    const start = offset++;
    while (offset < raw.length) {
      const char = raw[offset++];
      if (char === "\\") { offset++; continue; }
      if (char === '"') return JSON.parse(raw.slice(start, offset)) as string;
    }
    throw Error("Unclosed NPC JSON string");
  };
  const value = (depth: number, path: readonly (string | number)[]): unknown => {
    if (depth > 64) throw Error("Deep NPC JSON");
    ws();
    if (raw[offset] === '"') return string();
    if (raw[offset] === "{") {
      offset++; ws(); const result: Data = ownerProjection === "abi" ? {} : Object.create(null), keys = new Set<string>();
      if (raw[offset] === "}") { offset++; return Object.freeze(result); }
      for (;;) {
        ws(); if (raw[offset] !== '"') throw Error("Invalid NPC JSON key");
        const key = string();
        if (keys.has(key) || ["__proto__", "constructor", "prototype", "toJSON"].includes(key)) throw Error("Duplicate or unsafe NPC JSON key");
        keys.add(key); ws(); if (raw[offset++] !== ":") throw Error("Invalid NPC JSON field");
        result[key] = value(depth + 1, [...path, key]); ws();
        const next = raw[offset++]; if (next === "}") return Object.freeze(result);
        if (next !== ",") throw Error("Invalid NPC JSON object");
      }
    }
    if (raw[offset] === "[") {
      offset++; ws(); const result: unknown[] = [];
      if (raw[offset] === "]") { offset++; return Object.freeze(result); }
      for (;;) {
        result.push(value(depth + 1, [...path, result.length])); ws(); const next = raw[offset++];
        if (next === "]") return Object.freeze(result);
        if (next !== ",") throw Error("Invalid NPC JSON array");
      }
    }
    for (const [word, literal] of [["true", true], ["false", false], ["null", null]] as const) {
      if (raw.startsWith(word, offset)) { offset += word.length; return literal; }
    }
    const token = /^-?(?:0|[1-9][0-9]*)(?:\.[0-9]+)?(?:[eE][+-]?[0-9]+)?/.exec(raw.slice(offset))?.[0];
    if (!token) throw Error("Invalid NPC JSON value");
    offset += token.length;
    const kind = ownerProjection === "world" ? path[0] === "payload" ? ownerIntegerKind(["snapshot", ...path.slice(1)]) : null
      : ownerProjection === "abi" ? path[0] === "frame" ? ownerIntegerKind(path.slice(1)) : null
      : ownerProjection ? ownerIntegerKind(path) : null;
    if (kind) {
      if (!(kind === "u64" ? npcPurchaseDecimal(token) : npcPurchaseSignedDecimal(token))) throw Error("Invalid exact owner integer");
      const exact = BigInt(token);
      return exact >= BigInt(Number.MIN_SAFE_INTEGER) && exact <= BigInt(Number.MAX_SAFE_INTEGER) ? Number(exact) : token;
    }
    const decimal = /^(-?)([0-9]+)(?:\.([0-9]+))?(?:[eE]([+-]?[0-9]+))?$/.exec(token)!;
    const fraction = decimal[3] ?? "", exponentText = decimal[4] ?? "0";
    // Bound exact arithmetic before allocating powers of ten, including
    // underflow tokens that Number would silently turn into integral zero.
    if (decimal[2].length + fraction.length > 4096 || exponentText.replace(/^[+-]/, "").length > 5)
      throw Error("Unbounded NPC numeric token");
    const exponent = Number(exponentText);
    if (!Number.isInteger(exponent) || Math.abs(exponent) > 4096) throw Error("Unbounded NPC numeric exponent");
    const number = Number(token);
    if (!Number.isFinite(number) || Number.isInteger(number) && !Number.isSafeInteger(number)) throw Error("Unsafe integral NPC snapshot");
    if (Number.isInteger(number)) {
      const coefficient = BigInt(decimal[1] + decimal[2] + fraction), shift = exponent - fraction.length;
      const scale = 10n ** BigInt(Math.abs(shift));
      if (shift >= 0 ? coefficient * scale !== BigInt(number)
        : coefficient % scale !== 0n || coefficient / scale !== BigInt(number)) throw Error("Rounded integral NPC snapshot");
    }
    return number;
  };
  const result = value(0, []); ws(); if (offset !== raw.length) throw Error("Trailing NPC JSON");
  return result;
}
/** Stage5 keeps a complete saved ItemState as JSON text. Decode that original
 * text separately; the full owner source keeps its unmodified text carrier. */
export function parseNpcPurchaseMailItemJson(raw: string): unknown {
  const envelope = parseNpcPurchaseJson('{"snapshot":{"mailItemState":' + raw + '}}', true);
  if (!fields(envelope, ["snapshot"]) || !fields(envelope.snapshot, ["mailItemState"])
    || !exactSnapshotLeaves(envelope.snapshot.mailItemState, ["snapshot", "mailItemState"])) throw Error("Invalid mail item source");
  return envelope.snapshot.mailItemState;
}

// Detection consumes no identity, receipt or authority. The original string is
// passed untouched to the strict Rust receiver, including malformed frames.
function hasTopLevelJsonType(raw: unknown, expected: string): raw is string {
  if (typeof raw !== "string" || raw.length > 16 * 1024 * 1024) return false;
  let offset = 0;
  const ws = () => { while (/[\t\n\r ]/.test(raw[offset] ?? "!")) offset++; };
  const string = (): string => {
    const start = offset++;
    while (offset < raw.length) {
      const char = raw[offset++];
      if (char === "\\") { offset++; continue; }
      if (char === '"') return JSON.parse(raw.slice(start, offset)) as string;
    }
    throw Error("Unclosed marker string");
  };
  try {
    ws(); if (raw[offset++] !== "{") return false;
    let depth = 1;
    while (offset < raw.length && depth > 0) {
      const char = raw[offset];
      if (char === '"') {
        const key = string();
        // Iterative structural routing has no recursion-depth escape: even a
        // rejected deep value before a duplicate type cannot hide an owner.
        if (depth === 1 && key === "type") {
          ws();
          if (raw[offset] === ":") {
            offset++; ws();
            if (raw[offset] === '"' && string() === expected) return true;
          }
        }
        continue;
      }
      offset++;
      if (char === "{" || char === "[") depth++;
      else if (char === "}" || char === "]") depth--;
    }
    return false;
  } catch { return false; }
}
export function isNpcPurchaseOwnerFrame(raw: unknown): raw is string { return hasTopLevelJsonType(raw, "npcPurchaseOwner"); }
export function isNpcPurchaseWorldFrame(raw: unknown): raw is string { return hasTopLevelJsonType(raw, "worldSnapshot"); }
/** Ordinary snapshots are display sources only; this decoder creates no Core
 * receipt, producer, Applied witness or recovery authority. */
export function parseNpcPurchaseWorldFrame(raw: string): Readonly<{ type: "worldSnapshot"; payload: Data }> {
  const frame = parseNpcPurchaseJson(raw, "world");
  if (!fields(frame, ["type", "payload"]) || frame.type !== "worldSnapshot" || !record(frame.payload)) throw Error("Invalid world snapshot frame");
  return frame as Readonly<{ type: "worldSnapshot"; payload: Data }>;
}

/** Require the full economic projection; the opaque raw source retains fields
 * the visible model does not yet render. No inventory/tick/UID readiness proof. */
export function completeNpcPurchaseSnapshot(v: unknown): v is Data {
  if (!record(v) || !exactSnapshotLeaves(v, ["snapshot"])) return false;
  const required = ["tick", "mapTitle", "mapFileName", "inSafeZone", "lightSetting", "playerObjectId",
    "playerHp", "playerMaxHp", "playerMp", "playerMaxMp", "playerCrystalStats", "playerPkPoints",
    "playerExperience", "playerMaxExperience", "gold", "credit", "cityCurrencies", "currentWeight", "playerWeights",
    "maxWeight", "freeBagSlots", "maxBagSlots", "inventoryCapacity", "npcGoldTradeCapacity", "storageSize",
    "hasExpandedStorage", "hasStoragePassword", "requireStoragePassword", "storagePasswordLastSetBinaryDatetime",
    "expandedStorageExpiryTimeBinaryDatetime", "entities", "beltItems", "inventoryItems", "storageItems", "equipmentItems",
    "heroInventoryItems", "heroEquipmentItems", "heroInventoryCapacity", "heroStats", "heroVitals", "heroWeights",
    "questLog", "knownSkills", "activeBuffs", "activeNpcDialog", "stage5Systems", "terrainPatches", "decorObjects",
    "groundDrops", "mapTransfers", "interactionHints", "sceneView"];
  if (required.some(key => !Object.hasOwn(v, key)) || !integer(v.playerObjectId, 1, 0xffff_ffff)
    || !integer(v.gold, 0, 0xffff_ffff) || !integer(v.credit, 0, 0xffff_ffff)
    || !integer(v.playerExperience, 0, Number.MAX_SAFE_INTEGER) || !integer(v.playerMaxExperience, 0, Number.MAX_SAFE_INTEGER)
    || !integer(v.inventoryCapacity, 46, 86) || !record(v.cityCurrencies) || !record(v.stage5Systems)) return false;
  const arrays = ["entities", "playerCrystalStats", "beltItems", "inventoryItems", "storageItems", "equipmentItems",
    "heroInventoryItems", "heroEquipmentItems", "heroStats", "questLog", "knownSkills", "activeBuffs", "terrainPatches",
    "decorObjects", "groundDrops", "mapTransfers", "interactionHints"];
  if (arrays.some(key => !Array.isArray(v[key]))) return false;
  const nullableInt = (value: unknown) => value === null || integer(value, -2147483648, 2147483647);
  if (["playerHp", "playerMaxHp", "playerMp", "playerMaxMp"].some(key => !nullableInt(v[key]))
    || ["currentWeight", "maxWeight", "freeBagSlots", "maxBagSlots", "storageSize"].some(key => !integer(v[key], 0, 65535))
    || ["hasExpandedStorage", "hasStoragePassword", "requireStoragePassword", "inSafeZone"].some(key => typeof v[key] !== "boolean")
    || ["storagePasswordLastSetBinaryDatetime", "expandedStorageExpiryTimeBinaryDatetime"].some(key => !exactSigned(v[key]))
    || ![46,54,58,62,66,70,74,78,82,86].includes(v.inventoryCapacity)
    || Object.values(v.cityCurrencies).some(amount => !integer(amount, 0, 0xffff_ffff))
    || (v.playerCrystalStats as unknown[]).some(stat => !record(stat) || !integer(stat.stat, 0, 255) || !integer(stat.value, -2147483648, 2147483647))) return false;
  if (v.playerWeights !== null && (!fields(v.playerWeights, ["bag", "wear", "hand"])
    || Object.values(v.playerWeights).some(weight => !integer(weight, 0, 0xffff_ffff)))) return false;
  const item = (value: unknown, equipment = false): boolean => record(value)
    && typeof value.key === "string" && typeof value.name === "string" && typeof value.description === "string"
    && integer(value.quantity, 1, 0xffff_ffff) && integer(value.icon, 0, 65535)
    && (equipment ? (!Object.hasOwn(value, "uniqueId") || exactUnsigned(value.uniqueId))
      && ["weapon", "armour", "helmet", "mount", "necklace", "torch", "braceletLeft", "braceletRight", "ringLeft", "ringRight", "amulet", "boots", "belt", "stone"].includes(String(value.slot))
      : exactUnsigned(value.uniqueId) && integer(value.slot, 0, 255)
        && ["bag1", "bag2", "quest", "belt", "storage"].includes(String(value.container)));
  if (["beltItems", "inventoryItems", "storageItems", "heroInventoryItems", "heroEquipmentItems"].some(key => (v[key] as unknown[]).some(row => !item(row)))
    || (v.equipmentItems as unknown[]).some(row => !item(row, true))
    || (v.entities as unknown[]).some(entity => !record(entity) || !integer(entity.objectId, 1, 0xffff_ffff)
      || typeof entity.name !== "string" || !integer(entity.x, -2147483648, 2147483647)
      || !integer(entity.y, -2147483648, 2147483647) || typeof entity.direction !== "string" || typeof entity.dead !== "boolean")) return false;
  const self = (v.entities as unknown[]).find(e => record(e) && e.objectId === v.playerObjectId);
  if (!record(self) || typeof self.name !== "string" || !["Warrior", "Wizard", "Taoist", "Assassin", "Archer"].includes(String(self.class))
    || !["Male", "Female"].includes(String(self.gender)) || !integer(self.level, 1, 65535)) return false;
  const systems = v.stage5Systems;
  if (systems.hero !== null && (!record(systems.hero) || !Object.hasOwn(v, "heroMaxExperience")
    || !exactSigned(v.heroMaxExperience) || !exactSigned(systems.hero.experience))
    || systems.hero === null && Object.hasOwn(v, "heroMaxExperience") && v.heroMaxExperience !== null) return false;
  if (Object.hasOwn(v, "nativeNpcShop") && v.nativeNpcShop !== null) {
    const shop = npcPurchaseShopSource(v.nativeNpcShop, false);
    if (!shop
      || v.activeNpcDialog !== null && (!record(v.activeNpcDialog) || v.activeNpcDialog.npcObjectId !== shop.npcObjectId)) return false;
  }
  if (!Array.isArray(systems.mail)) return false;
  for (const mail of systems.mail) {
    if (!record(mail)) return false;
    const states = mail.itemStatesJson;
    if (states !== undefined) {
      if (!Array.isArray(states) || states.length > 5) return false;
      try { if (states.some(raw => typeof raw !== "string" || !record(parseNpcPurchaseMailItemJson(raw)))) return false; }
      catch { return false; }
    }
  }
  const nested = (value: unknown, keys: string[]) => record(value) && keys.every(key => Object.hasOwn(value, key));
  if (!nested(systems.mentor, ["isMentor", "cooldownUntilMs", "allowMentor", "name", "level", "online", "menteeExp", "pendingRequestFrom", "pendingRequestLevel"])
    || !nested(systems.relationship, ["allowLoverRecall", "cooldownUntilMs", "allowMarriage", "partnerName", "marriedDateBinaryDatetime", "mapName", "marriedDays", "pendingRequestFrom", "pendingDivorceFrom"])) return false;
  return ["group", "guild", "social", "relationship", "mentor", "mail", "gameShopIndividualPurchases",
    "economyProjectionEventIds", "trade", "auction", "refine", "conquest", "guildTerritory", "hero",
    "heroLearnedMagics", "profession", "appearance", "nameLists", "intelligentCreatures", "summonedIntelligentCreatureType",
    "intelligentCreaturePearls", "itemRental", "attackMode", "petMode", "pkDecayElapsedTicks"]
    .every(key => Object.hasOwn(systems, key)) && Array.isArray(systems.mail) && record(systems.mentor)
    && record(systems.relationship) && Array.isArray(systems.economyProjectionEventIds)
    && integer(systems.intelligentCreaturePearls, -2147483648, 2147483647);
}

type Gesture = { request: NpcPurchaseRequest; currency: "gold" | "pearls"; current(): boolean };
type Control = { action: Data; epoch: number; session: string; gesture?: Gesture };
export type NpcPurchaseClientOptions = {
  host: NpcPurchaseReceiptRuntime; socket: object;
  current(): boolean; session(): string | null; oldAttemptBlocked(): boolean;
  send(body: string): void;
  apply(snapshot: Data, rawFrame: string): boolean;
};

/** Actual socket adapter; one explicit UI gesture obtains one server Quote,
 * one reservation, one entry and one send. Reconnection only queries Core's
 * retained original operation. This object never constructs an economic host. */
export class NpcPurchaseClient {
  private readonly source = Object.freeze({});
  private binding: Binding | null = null;
  private epoch = 0;
  private active = false;
  private begunSession: string | null = null;
  private readonly controls = new Map<string, Control>();
  private readonly sent = new Set<string>();
  private quoting = false;
  constructor(private readonly options: NpcPurchaseClientOptions) {}
  open(): boolean {
    const o = this.options;
    if (this.active) return o.current();
    this.active = o.current() && o.host.attachProducer(this.source)
      && o.host.transportFor(this.source, o.socket, o.socket, true) !== null;
    return this.active;
  }
  withdraw(disconnect = false): void {
    this.epoch++; this.binding = null; this.begunSession = null; this.quoting = false; this.controls.clear();
    if (!this.active) return;
    try { this.options.host.transact(this.source, this.options.socket, { op: "withdraw", disconnect }); } catch { /* Retained Core barrier. */ }
    if (disconnect) { this.active = false; this.options.host.withdrawProducer(this.source); }
  }
  private current(session: string, epoch = this.epoch): boolean {
    return this.active && this.epoch === epoch && this.options.current() && this.options.session() === session;
  }
  private call(input: Data): Data | null {
    const response = this.options.host.transact(this.source, this.options.socket, input);
    // The optional facade returns unknown JSON: detach and validate before use.
    const v = plain(response);
    return record(v) && v.ok === true ? v : null;
  }
  private preserveFailedEntry(binding: Binding, operation: Operation, attempted: boolean): void {
    // Only Core can distinguish a refused entry from an entered original. Its
    // matching cancellation cannot release an entered operation, even if a
    // callback changed producer custody or a malformed decision hid success.
    try {
      if (attempted) {
        const unknown = this.call({ op: "unknown", binding, operation });
        if (fields(unknown, ["ok", "matched"]) && unknown.matched === true) return;
      }
      const canceled = this.call({ op: "cancelUnsent", binding, operation });
      if (fields(canceled, ["ok", "matched"]) && canceled.matched === true) return;
    } catch { /* The permanent host keeps the original operation. */ }
    this.withdraw();
  }
  private cancelMalformedReservation(binding: Binding, intent: Intent): void {
    try {
      const status = this.call({ op: "status" });
      if (statusValid(status) && same(status.binding, binding) && record(status.pending)
        && status.pending.phase === "queued" && operationValid(status.pending.operation)
        && status.pending.operation.actor === binding.actor && same(status.pending.operation.intent, intent)) {
        this.preserveFailedEntry(binding, status.pending.operation, false); return;
      }
    } catch { /* Unverifiable custody stays unavailable, never fabricated. */ }
    this.withdraw();
  }
  private dispatch(op: string, input: Data, action: Data, gesture?: Gesture): boolean {
    let attempted = false, sent = false;
    try {
      const session = this.options.session(), epoch = this.epoch;
      if (!session || !this.current(session) || this.controls.size >= 64) return false;
      attempted = op === "enter";
      const result = this.call({ op, ...input });
      if (!fields(result, ["ok", "request", "body"]) || typeof result.body !== "string"
        || !fields(result.request, ["type", "protocolVersion", "requestId", "action"])
        || result.request.type !== "npcPurchaseOwner" || result.request.protocolVersion !== 1
        || !nonzero(result.request.requestId) || !same(result.request.action, action)
        || !same(parseNpcPurchaseJson(result.body), result.request) || this.sent.has(result.request.requestId)) return false;
      if (!this.current(session, epoch) || gesture && (!gesture.current() || this.options.oldAttemptBlocked())) return false;
      if (!this.current(session, epoch)) return false;
      const requestId = result.request.requestId;
      this.sent.add(requestId); this.controls.set(requestId, { action: result.request.action as Data, epoch, session, gesture });
      // Enter has already changed Core custody. A send exception is Unknown,
      // never definitely-unsent and never a legacy fallback/retry.
      this.options.send(result.body); sent = true; return true;
    } catch { return false; }
    finally {
      if (op === "enter" && !sent && bindingValid(input.binding) && operationValid(input.operation)) {
        this.preserveFailedEntry(input.binding, input.operation, attempted);
      }
    }
  }
  begin(refresh = false): boolean {
    const session = this.options.session();
    if (!session || !this.active || !refresh && this.begunSession === session) return false;
    this.withdraw(); this.begunSession = session;
    try { return this.dispatch("begin", {}, { kind: "begin" }); } catch { return false; }
  }
  purchase(request: NpcPurchaseRequest, currency: "gold" | "pearls", current: () => boolean): boolean {
    if (!requestValid(request) || !this.binding || this.quoting || !current() || this.options.oldAttemptBlocked()) return false;
    const binding = this.binding, gesture: Gesture = { request: Object.freeze({ ...request }), currency, current };
    try {
      const status = this.call({ op: "status" });
      if (!statusValid(status) || status.pending !== null || !same(status.binding, binding)) return false;
      this.quoting = true;
      const sent = this.dispatch("quote", { binding, request: gesture.request }, { kind: "quote", request: gesture.request }, gesture);
      if (!sent) this.quoting = false;
      return sent;
    } catch { this.quoting = false; return false; }
  }
  receive(raw: string): void {
    if (!this.active || !this.options.current()) return;
    try {
      // Receipt observation still reaches Core when the generic snapshot cannot
      // safely project, but such a frame can never issue an Applied witness.
      const response = this.options.host.transact(this.source, this.options.socket, { op: "receive", frame: raw });
      const frame = parseNpcPurchaseJson(raw, true);
      const detached = record(frame) ? plain(response, 0, frame.snapshot) : null;
      const result = record(detached) && detached.ok === true ? detached : null;
      if (!result || !fields(frame, ["type", "protocolVersion", "requestId", "reply", "snapshot", "authority"])
        || frame.type !== "npcPurchaseOwner" || frame.protocolVersion !== 1 || !nonzero(frame.requestId)
        || !same(result.frame, frame) || !record(frame.reply)) return;
      const control = this.controls.get(frame.requestId);
      if (!control || !this.current(control.session, control.epoch)) return;
      if (!replyValid(frame.reply, control.action)) return;
      if (!observationValid(result.observation)) return;
      const producer = result.kind === "producer", quote = result.kind === "quote";
      if (!fields(result, producer ? ["ok", "frame", "observation", "kind", "binding"]
        : quote ? ["ok", "frame", "observation", "kind", "intent"] : ["ok", "frame", "observation", "kind"])) return;
      if (producer) {
        if (control.action.kind !== "begin" || frame.reply.kind !== "producer" || !bindingValid(result.binding)
          || result.binding.beginId !== frame.requestId || !producerValid(frame.authority)
          || !same(frame.reply.producer, frame.authority) || result.binding.actor !== frame.authority.actor
          || result.binding.producerScope !== frame.authority.producerScope) return;
        this.binding = result.binding;
      } else if (quote) {
        if (control.action.kind !== "quote" || frame.reply.kind !== "quote" || !intentValid(result.intent)
          || !same(frame.reply.intent, result.intent) || !control.gesture
          || !same(result.intent.request, control.gesture.request) || result.intent.currency !== control.gesture.currency) return;
      } else if (result.kind !== "result") return;
      this.controls.delete(frame.requestId);
      if (frame.snapshot !== null) {
        const binding = this.binding;
        if (!binding || !producerValid(frame.authority) || frame.authority.actor !== binding.actor
          || frame.authority.producerScope !== binding.producerScope || !completeNpcPurchaseSnapshot(frame.snapshot)
          || !this.options.apply(frame.snapshot, raw) || !this.current(control.session, control.epoch)) return;
        const applied = this.call({ op: "applied", binding, authority: frame.authority, complete: true });
        if (!fields(applied, ["ok", "observation"]) || !observationValid(applied.observation)) return;
      } else if (frame.authority !== null || producer) return;
      if (producer && this.binding) {
        // Core refuses Query if no entered original exists. Never reserve on
        // recovery, regenerate an operation or synthesize another Purchase.
        const status = this.call({ op: "status" });
        if (statusValid(status) && status.pending !== null && record(status.pending)
          && !["queued"].includes(String(status.pending.phase))) {
          this.dispatch("query", { binding: this.binding }, { kind: "query", operation: status.pending.operation });
        }
      }
      if (quote) {
        this.quoting = false;
        const binding = this.binding, intent = result.intent as Intent, gesture = control.gesture!;
        if (!binding || !gesture.current() || this.options.oldAttemptBlocked() || !this.current(control.session, control.epoch)) return;
        let reserved: Data | null;
        try { reserved = this.call({ op: "reserve", binding, intent }); }
        catch { this.cancelMalformedReservation(binding, intent); return; }
        if (!fields(reserved, ["ok", "operation"]) || !operationValid(reserved.operation)
          || reserved.operation.actor !== binding.actor || !same(reserved.operation.intent, intent)) {
          this.cancelMalformedReservation(binding, intent); return;
        }
        const operation = reserved.operation;
        try {
          if (!gesture.current() || this.options.oldAttemptBlocked() || !this.current(control.session, control.epoch)) {
            this.preserveFailedEntry(binding, operation, false); return;
          }
        } catch { this.preserveFailedEntry(binding, operation, false); return; }
        this.dispatch("enter", { binding, operation }, { kind: "purchase", operation }, gesture);
      }
    } catch {
      // No malformed/unsafe frame grants a new baseline. Existing ledger and
      // entered operation remain in the document-owned host for fresh Begin.
      this.withdraw();
    }
  }
}

function observationValid(v: unknown): boolean {
  if (!record(v)) return false;
  if (["ignored", "pending"].includes(String(v.kind))) return fields(v, ["kind"]);
  if (!fields(v, ["kind", "settlement"]) || v.kind !== "settled" || !fields(v.settlement, ["operation", "result"])
    || !operationValid(v.settlement.operation) || !record(v.settlement.result)) return false;
  const r = v.settlement.result;
  if (r.kind === "unknown") return fields(r, ["kind"]);
  if (r.kind === "rejected") return fields(r, ["kind", "serverRevision"]) && nonzero(r.serverRevision) && r.serverRevision !== "18446744073709551615";
  return r.kind === "committed" && fields(r, ["kind", "serverRevision", "currency", "source", "charged", "admittedCount", "incomingUniqueId"])
    && nonzero(r.serverRevision) && r.serverRevision !== "18446744073709551615"
    && r.currency === v.settlement.operation.intent.currency && r.source === v.settlement.operation.intent.source
    && integer(r.charged, 0, 0xffff_ffff) && integer(r.admittedCount, 1, v.settlement.operation.intent.request.count)
    && npcPurchaseDecimal(r.incomingUniqueId);
}

function pendingValid(v: unknown, actor?: unknown): boolean {
  return v === null || fields(v, ["operation", "minimumRevision", "phase"]) && operationValid(v.operation)
    && (actor === undefined || v.operation.actor === actor) && npcPurchaseDecimal(v.minimumRevision)
    && ["queued", "entered", "unknown", "awaitingSnapshot"].includes(String(v.phase));
}
function statusValid(v: unknown): v is Data {
  return fields(v, ["ok", "token", "binding", "pending", "actors", "controls"]) && v.ok === true
    && (v.token === null || opaque(v.token)) && (v.binding === null || bindingValid(v.binding))
    && pendingValid(v.pending, record(v.binding) ? v.binding.actor : undefined)
    && integer(v.controls, 0, 64) && Array.isArray(v.actors)
    && v.actors.every(actor => fields(actor, ["actor", "lastSequence", "appliedBaseline", "pending"])
      && opaque(actor.actor) && npcPurchaseDecimal(actor.lastSequence)
      && (actor.appliedBaseline === null || npcPurchaseDecimal(actor.appliedBaseline) && actor.appliedBaseline !== "18446744073709551615")
      && pendingValid(actor.pending, actor.actor));
}
function replyValid(v: Data, action: Data): boolean {
  if (v.kind === "producer") return action.kind === "begin" && fields(v, ["kind", "producer"]) && producerValid(v.producer);
  if (v.kind === "quote") return action.kind === "quote" && fields(v, ["kind", "intent"]) && intentValid(v.intent)
    && same(v.intent.request, action.request);
  if (v.kind === "failure") return fields(v, ["kind", "state", "receipt"])
    && (["beforeExecution", "unknown"].includes(String(v.state)) ? v.receipt === null
      : v.state === "postCommit" && receiptValid(v.receipt, action.operation));
  if (v.kind === "recovery") return action.kind === "query" && fields(v, ["kind", "receipt"])
    && (v.receipt === null || receiptValid(v.receipt, action.operation));
  return v.kind === "purchase" && action.kind === "purchase" && fields(v, ["kind", "receipt", "replayed"])
    && typeof v.replayed === "boolean" && receiptValid(v.receipt, action.operation);
}
function receiptValid(v: unknown, operation: unknown): boolean {
  if (!operationValid(operation) || !fields(v, ["producerScope", "entry"]) || !opaque(v.producerScope)
    || !fields(v.entry, ["operation", "serverRevision", "outcome"]) || !same(v.entry.operation, operation)
    || !nonzero(v.entry.serverRevision) || v.entry.serverRevision === "18446744073709551615" || !record(v.entry.outcome)) return false;
  const outcome = v.entry.outcome;
  const r = outcome.rejected;
  if (fields(outcome, ["committed"])) {
    const c = outcome.committed;
    return fields(c, ["request", "currency", "source", "charged", "admittedCount", "incomingUniqueId"])
      && same(c.request, operation.intent.request) && c.currency === operation.intent.currency && c.source === operation.intent.source
      && integer(c.charged, 0, 0xffff_ffff) && integer(c.admittedCount, 1, operation.intent.request.count) && npcPurchaseDecimal(c.incomingUniqueId);
  }
  return fields(outcome, ["rejected"]) && fields(r, ["request", "reason"]) && same(r.request, operation.intent.request)
    && ["invalidRequest", "playerDead", "serviceUnavailable", "unsupportedService", "unknownGood", "invalidQuantity",
      "insufficientCurrency", "clockUnavailable", "invalidDelivery"].includes(String(r.reason));
}
