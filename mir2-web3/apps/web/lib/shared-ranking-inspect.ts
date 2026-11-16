import { crystalItemSourceJson, crystalUserItemFields, parseCrystalItemTooltipSource, sameCrystalItemData } from "./crystal-item-source";

/** ABI admission is data only. The caller still owns gesture and send authority. */
export type RankingInspectAdmissionInput = Readonly<{ version: 1; opened: boolean; rankingsReady: boolean;
  pending: boolean; playerId: number; nowMs: number; nextReadyMs: number }>;
export type RankingInspectAdmissionPlan = Readonly<{ version: 1; ok: false }>
  | Readonly<{ version: 1; ok: true; objectId: number; nextReadyMs: number }>;
export type RankingInspectWasmModule = {
  ranking_inspect_abi_version?: () => number;
  ranking_inspect_admission?: (json: string) => string;
};

const integer = (value: unknown, max: number): value is number => typeof value === "number"
  && Number.isSafeInteger(value) && !Object.is(value, -0) && value >= 0 && value <= max;
const record = (value: unknown): value is Record<string, unknown> => value !== null && typeof value === "object" && !Array.isArray(value);
const exactKeys = (value: unknown, keys: readonly string[]): value is Record<string, unknown> => record(value)
  && Object.keys(value).length === keys.length && keys.every(key => Object.hasOwn(value, key));
const INPUT_KEYS = ["version", "opened", "rankingsReady", "pending", "playerId", "nowMs", "nextReadyMs"] as const;
function inputJson(value: unknown): string | null {
  // Validate literal descriptors before reading fields; accessors/toJSON never run.
  const raw: unknown = JSON.parse(crystalItemSourceJson(value, 1024, 16, 0));
  if (!exactKeys(raw, INPUT_KEYS) || raw.version !== 1 || typeof raw.opened !== "boolean"
    || typeof raw.rankingsReady !== "boolean" || typeof raw.pending !== "boolean" || !integer(raw.playerId, 4294967295)
    || !integer(raw.nowMs, Number.MAX_SAFE_INTEGER) || !integer(raw.nextReadyMs, Number.MAX_SAFE_INTEGER)) return null;
  return JSON.stringify(Object.fromEntries(INPUT_KEYS.map(key => [key, raw[key]])));
}
const activeReads = new WeakMap<object, { reentered: boolean }>();

/** No JS readiness planner and no optimistic policy fallback. */
export function readSharedRankingInspectAdmission(module: RankingInspectWasmModule,
  input: RankingInspectAdmissionInput): RankingInspectAdmissionPlan | null {
  if (!record(module)) return null;
  const active = activeReads.get(module);
  if (active) { active.reentered = true; return null; }
  const lease = { reentered: false }; activeReads.set(module, lease);
  try {
    const json = inputJson(input);
    if (!json) return null;
    const abi = module.ranking_inspect_abi_version, getter = module.ranking_inspect_admission;
    const stable = () => !lease.reentered && module.ranking_inspect_abi_version === abi
      && module.ranking_inspect_admission === getter && !lease.reentered;
    if (typeof abi !== "function" || typeof getter !== "function" || !stable() || abi.call(module) !== 1 || !stable()
      || inputJson(input) !== json || !stable()) return null;
    const text = getter.call(module, json);
    if (!stable() || abi.call(module) !== 1 || !stable() || inputJson(input) !== json || !stable()
      || typeof text !== "string" || new TextEncoder().encode(text).length > 1024) return null;
    const result: unknown = JSON.parse(text);
    if (exactKeys(result, ["version", "ok"]) && result.version === 1 && result.ok === false)
      return text === '{"version":1,"ok":false}' ? Object.freeze({ version: 1, ok: false }) : null;
    if (!exactKeys(result, ["version", "ok", "objectId", "nextReadyMs"]) || result.version !== 1 || result.ok !== true
      || !integer(result.objectId, 4294967295) || !integer(result.nextReadyMs, Number.MAX_SAFE_INTEGER)) return null;
    const snapshot = JSON.parse(json) as RankingInspectAdmissionInput;
    if (result.objectId !== snapshot.playerId) return null;
    // platform-web/src/ranking_inspect.rs emits these fixed whole literals in
    // this order. Compare the entire response, so JSON.parse cannot hide raw or
    // escaped duplicate keys, altered number forms, whitespace, or extra data.
    if (text !== `{"version":1,"ok":true,"objectId":${result.objectId},"nextReadyMs":${result.nextReadyMs}}`) return null;
    return Object.freeze({ version: 1, ok: true, objectId: result.objectId, nextReadyMs: result.nextReadyMs });
  } catch { return null; }
  finally { if (activeReads.get(module) === lease) activeReads.delete(module); }
}

export type RankingInspectEquipment = Readonly<{ slotIndex: number; uniqueId: number; itemIndex: number; count: number;
  name: string; icon: number; sourceKind: "instance"; tooltipSource: Readonly<Record<string, unknown>>;
  raw: Readonly<Record<string, unknown>> }>;
export type RankingPlayerInspect = Readonly<{ name: string; guildName: string; guildRank: string;
  class: "Warrior" | "Wizard" | "Taoist" | "Assassin" | "Archer"; gender: "Male" | "Female";
  hair: number; level: number; loverName: string; allowObserve: boolean; isHero: false;
  equipment: readonly (RankingInspectEquipment | null)[] }>;
const INSPECT_KEYS = ["name", "guildName", "guildRank", "equipment", "class", "gender", "hair", "level",
  "loverName", "allowObserve", "isHero"] as const;
const textFact = (value: unknown): value is string => typeof value === "string" && value.length <= 512 && !value.includes("\0");
function freezeData(value: unknown): void {
  if (!value || typeof value !== "object") return;
  for (const child of Object.values(value)) freezeData(child);
  Object.freeze(value);
}

/** Actual target only: no requester inventory, template stat inference, or Hero fallback. */
export function parseRankingPlayerInspect(value: unknown, expectedName: string): RankingPlayerInspect | null {
  try {
    if (!textFact(expectedName) || expectedName.length === 0) return null;
    const raw: unknown = JSON.parse(crystalItemSourceJson(value));
    if (!exactKeys(raw, INSPECT_KEYS) || raw.name !== expectedName || ![raw.name, raw.guildName, raw.guildRank, raw.loverName].every(textFact)
      || !["Warrior", "Wizard", "Taoist", "Assassin", "Archer"].includes(raw.class as string)
      || !["Male", "Female"].includes(raw.gender as string) || !integer(raw.hair, 255) || !integer(raw.level, 65535)
      || typeof raw.allowObserve !== "boolean" || raw.isHero !== false || !Array.isArray(raw.equipment) || raw.equipment.length !== 14) return null;
    const equipment: (RankingInspectEquipment | null)[] = [], ids = new Set<number>();
    function claimUserIds(item: Record<string, unknown>): boolean {
      if (!integer(item.unique_id, Number.MAX_SAFE_INTEGER) || ids.has(item.unique_id)) return false;
      ids.add(item.unique_id);
      return (item.slots as (Record<string, unknown> | null)[]).every(child => child === null || claimUserIds(child));
    }
    for (let slotIndex = 0; slotIndex < 14; slotIndex++) {
      const item: unknown = raw.equipment[slotIndex];
      if (item === null) { equipment.push(null); continue; }
      if (!record(item) || !integer(item.unique_id, Number.MAX_SAFE_INTEGER) || !integer(item.count, 65535)
        || typeof item.item_index !== "number" || !Number.isSafeInteger(item.item_index)
        || item.item_index < -2147483648 || item.item_index > 2147483647) return null;
      const source = parseCrystalItemTooltipSource(item.tooltipSource,
        { uniqueId: item.unique_id, itemIndex: item.item_index, count: item.count });
      if (!source || !crystalUserItemFields.every(key => Object.hasOwn(item, key))) return null;
      const user = Object.fromEntries(crystalUserItemFields.map(key => [key, item[key]]));
      if (!sameCrystalItemData(user, source.userItem) || !claimUserIds(source.userItem as Record<string, unknown>)) return null;
      const info = source.info as Readonly<{ name: string; image: number }>;
      // The actual Gateway template join applies Crystal UserItem.Image. An
      // Amulet stack can differ from Info.Image; TS never repeats that selector.
      if (item.name !== info.name || !integer(item.icon, 65535)) return null;
      freezeData(item);
      equipment.push(Object.freeze({ slotIndex, uniqueId: item.unique_id, itemIndex: item.item_index, count: item.count,
        name: info.name, icon: item.icon, sourceKind: "instance", tooltipSource: source, raw: item }));
    }
    return Object.freeze({ name: expectedName, guildName: raw.guildName as string, guildRank: raw.guildRank as string,
      class: raw.class as RankingPlayerInspect["class"], gender: raw.gender as RankingPlayerInspect["gender"], hair: raw.hair,
      level: raw.level, loverName: raw.loverName as string, allowObserve: raw.allowObserve, isHero: false,
      equipment: Object.freeze(equipment) });
  } catch { return null; }
}
