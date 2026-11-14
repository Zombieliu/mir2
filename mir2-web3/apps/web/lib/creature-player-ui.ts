import { sameSocialReplyOwner, type SocialReplyOwner } from "./social-incoming-replies";

export const CREATURE_FILTER_KEYS = ["petPickupAll", "petPickupGold", "petPickupWeapons", "petPickupArmours",
  "petPickupHelmets", "petPickupBoots", "petPickupBelts", "petPickupAccessories", "petPickupOthers"] as const;
export type CreatureFilterKey = typeof CREATURE_FILTER_KEYS[number];
export type CreatureItemFilter = Readonly<Record<CreatureFilterKey, boolean>>;
export type CreatureRules = Readonly<{
  minimalFullness: number; mousePickupEnabled: boolean; mousePickupRange: number;
  autoPickupEnabled: boolean; autoPickupRange: number; semiAutoPickupEnabled: boolean;
  semiAutoPickupRange: number; canProduceBlackstone: boolean;
}>;
/** Exact i64 text internally. The wire encoder below writes number literals. */
export type CreaturePlayerRecord = Readonly<{
  petType: number; icon: number; customName: string; fullness: number; slotIndex: number;
  expireBinaryDatetime: string; blackstoneTime: string; petMode: 0 | 1;
  creatureRules: CreatureRules; filter: CreatureItemFilter; pickupGrade: number; maintainFoodTime: string;
}>;
export type CreaturePlayerSource = Readonly<{
  owner: SocialReplyOwner; revision: number; records: readonly CreaturePlayerRecord[];
  summonedPetType: number | null; renameEnabled: boolean; fullList: boolean;
}>;
export type CreaturePlayerIntent = Readonly<{ petType: number } & (
  | { kind: "summon" | "dismiss" }
  | { kind: "rename"; text: string }
  | { kind: "release"; confirmationName: string }
  | { kind: "mode"; mode: 0 | 1 }
  | { kind: "options"; filter: CreatureItemFilter; pickupGrade: number }
  | { kind: "slot"; slotIndex: number }
)>;
export type CreaturePlayerCommand = Readonly<{
  type: "updateIntelligentCreature"; creature: CreaturePlayerRecord;
  summonMe: boolean; unsummonMe: boolean; releaseMe: boolean;
}>;
export type CreaturePlayerProof = Readonly<{
  source: CreaturePlayerSource; intent: CreaturePlayerIntent; command: CreaturePlayerCommand;
}>;

function object(v: unknown): v is Record<string, unknown> { return !!v && typeof v === "object" && !Array.isArray(v); }
function integer(v: unknown, min: number, max: number): v is number {
  return typeof v === "number" && Number.isSafeInteger(v) && v >= min && v <= max;
}
function exact(v: Record<string, unknown>, keys: readonly string[]): boolean {
  return Object.keys(v).length === keys.length && keys.every(k => Object.hasOwn(v, k));
}
export function validSocialServiceOwner(o: SocialReplyOwner | null): o is SocialReplyOwner {
  return !!o && object(o.socket) && [o.connectionGeneration, o.sessionGeneration, o.sceneRevision, o.playerObjectId]
    .every(n => integer(n, 1, Number.MAX_SAFE_INTEGER)) && typeof o.mapFileName === "string"
    && o.mapFileName.length > 0 && o.mapFileName.length <= 512;
}
export function sameSocialServiceStream(a: SocialReplyOwner | null, b: SocialReplyOwner | null): boolean {
  return !!a && !!b && a.socket === b.socket && a.connectionGeneration === b.connectionGeneration
    && a.sessionGeneration === b.sessionGeneration && a.playerObjectId === b.playerObjectId;
}
export function exactSocialServiceI64(v: unknown): string | null {
  if (typeof v === "number") return Number.isSafeInteger(v) ? String(v) : null;
  if (typeof v !== "string" || !/^(?:0|-?[1-9]\d{0,18})$/.test(v)) return null;
  try { const n = BigInt(v); return n >= -9223372036854775808n && n <= 9223372036854775807n ? v : null; }
  catch { return null; }
}
const RULE_KEYS = ["minimalFullness", "mousePickupEnabled", "mousePickupRange", "autoPickupEnabled", "autoPickupRange",
  "semiAutoPickupEnabled", "semiAutoPickupRange", "canProduceBlackstone"] as const;
const RECORD_KEYS = ["petType", "icon", "customName", "fullness", "slotIndex", "expireBinaryDatetime", "blackstoneTime",
  "petMode", "creatureRules", "filter", "pickupGrade", "maintainFoodTime"] as const;
export function readCreatureItemFilter(v: unknown): CreatureItemFilter | null {
  if (!object(v) || !exact(v, CREATURE_FILTER_KEYS) || CREATURE_FILTER_KEYS.some(k => typeof v[k] !== "boolean")) return null;
  return Object.freeze(Object.fromEntries(CREATURE_FILTER_KEYS.map(k => [k, v[k]]))) as CreatureItemFilter;
}
/** ClientIntelligentCreature, packages/protocol/src/types.rs:1654; no slot-derived identity. */
export function readCreaturePlayerRecord(v: unknown): CreaturePlayerRecord | null {
  if (!object(v) || !exact(v, RECORD_KEYS) || !integer(v.petType, 0, 255) || v.petType === 99
    || !integer(v.icon, -2147483648, 2147483647) || !integer(v.fullness, -2147483648, 2147483647)
    || !integer(v.slotIndex, 0, 9) || !integer(v.petMode, 0, 1) || !integer(v.pickupGrade, 0, 5)
    || typeof v.customName !== "string" || !v.customName || v.customName.length > 256 || v.customName.includes("\0")
    || !object(v.creatureRules) || !exact(v.creatureRules, RULE_KEYS)) return null;
  const rules = v.creatureRules;
  if (["minimalFullness", "mousePickupRange", "autoPickupRange", "semiAutoPickupRange"]
    .some(k => !integer(rules[k], -2147483648, 2147483647))
    || ["mousePickupEnabled", "autoPickupEnabled", "semiAutoPickupEnabled", "canProduceBlackstone"]
      .some(k => typeof rules[k] !== "boolean")) return null;
  const expire = exactSocialServiceI64(v.expireBinaryDatetime), blackstone = exactSocialServiceI64(v.blackstoneTime);
  const maintain = exactSocialServiceI64(v.maintainFoodTime), filter = readCreatureItemFilter(v.filter);
  if (expire === null || blackstone === null || maintain === null || !filter) return null;
  return Object.freeze({ petType: v.petType, icon: v.icon, customName: v.customName, fullness: v.fullness,
    slotIndex: v.slotIndex, expireBinaryDatetime: expire, blackstoneTime: blackstone, petMode: v.petMode as 0 | 1,
    creatureRules: Object.freeze({ minimalFullness: rules.minimalFullness as number,
      mousePickupEnabled: rules.mousePickupEnabled as boolean, mousePickupRange: rules.mousePickupRange as number,
      autoPickupEnabled: rules.autoPickupEnabled as boolean, autoPickupRange: rules.autoPickupRange as number,
      semiAutoPickupEnabled: rules.semiAutoPickupEnabled as boolean, semiAutoPickupRange: rules.semiAutoPickupRange as number,
      canProduceBlackstone: rules.canProduceBlackstone as boolean }), filter, pickupGrade: v.pickupGrade, maintainFoodTime: maintain });
}
export function makeCreaturePlayerSource(rawRecords: unknown, summonedPetType: unknown, renameEnabled: unknown,
  owner: SocialReplyOwner | null, revision: number, fullList: boolean): CreaturePlayerSource | null {
  if (!validSocialServiceOwner(owner) || !integer(revision, 0, Number.MAX_SAFE_INTEGER) || typeof renameEnabled !== "boolean"
    || typeof fullList !== "boolean" || !Array.isArray(rawRecords) || rawRecords.length > 10
    || !(summonedPetType === null || integer(summonedPetType, 0, 255) && summonedPetType !== 99)) return null;
  const records: CreaturePlayerRecord[] = [], types = new Set<number>(), slots = new Set<number>();
  for (const raw of rawRecords) {
    const row = readCreaturePlayerRecord(raw);
    if (!row || types.has(row.petType) || slots.has(row.slotIndex)) return null;
    records.push(row); types.add(row.petType); slots.add(row.slotIndex);
  }
  if (summonedPetType !== null && !types.has(summonedPetType)) return null;
  return Object.freeze({ owner: Object.freeze({ ...owner }), revision,
    records: Object.freeze(records.sort((a,b) => a.slotIndex - b.slotIndex)), summonedPetType, renameEnabled, fullList });
}
export function upsertCreaturePlayerRecord(source: CreaturePlayerSource, raw: unknown, revision: number): CreaturePlayerSource | null {
  const record = readCreaturePlayerRecord(raw);
  if (!record || !integer(revision, source.revision + 1, Number.MAX_SAFE_INTEGER)) return null;
  const rows = source.records.filter(row => row.petType !== record.petType);
  return makeCreaturePlayerSource([...rows, record], source.summonedPetType, source.renameEnabled, source.owner, revision, false);
}
export function creaturePlayerSourceKey(source: CreaturePlayerSource): string {
  return JSON.stringify([source.revision, source.records, source.summonedPetType, source.renameEnabled]);
}
export function creaturePlayerSourceCurrent(captured: CreaturePlayerSource, current: CreaturePlayerSource | null): boolean {
  return !!current && sameSocialReplyOwner(captured.owner, current.owner)
    && creaturePlayerSourceKey(captured) === creaturePlayerSourceKey(current);
}
export function asciiCreatureNameMatches(a: string, b: string): boolean {
  const fold = (s: string) => s.replace(/[A-Z]/g, c => c.toLowerCase());
  return fold(a) === fold(b);
}
/** Native creature_dialog.rs:317–413. Mutations start from the complete current record. */
export function creaturePlayerCommand(intent: CreaturePlayerIntent, source: CreaturePlayerSource): CreaturePlayerCommand | null {
  const pet = source.records.find(p => p.petType === intent.petType);
  if (!pet || !validSocialServiceOwner(source.owner)) return null;
  let creature = pet, summonMe = false, unsummonMe = false, releaseMe = false;
  switch (intent.kind) {
    case "summon": if (source.summonedPetType !== null) return null; summonMe = true; break;
    case "dismiss": if (source.summonedPetType !== pet.petType) return null; unsummonMe = true; break;
    case "release":
      if (source.summonedPetType === pet.petType || typeof intent.confirmationName !== "string"
        || intent.confirmationName.length > 256
        || !asciiCreatureNameMatches(intent.confirmationName, pet.customName)) return null;
      releaseMe = true; break;
    case "rename":
      if (!source.renameEnabled || typeof intent.text !== "string" || !/^[A-Za-z0-9]{3,15}$/.test(intent.text)
        || intent.text === pet.customName) return null;
      creature = Object.freeze({ ...pet, customName: intent.text }); break;
    case "mode":
      if (intent.mode === pet.petMode || intent.mode !== 0 && intent.mode !== 1
        || intent.mode === 0 && !pet.creatureRules.autoPickupEnabled
        || intent.mode === 1 && !pet.creatureRules.semiAutoPickupEnabled) return null;
      creature = Object.freeze({ ...pet, petMode: intent.mode }); break;
    case "options": {
      const filter = readCreatureItemFilter(intent.filter);
      if (!filter || !integer(intent.pickupGrade, 0, 5)
        || intent.pickupGrade === pet.pickupGrade && JSON.stringify(filter) === JSON.stringify(pet.filter)) return null;
      creature = Object.freeze({ ...pet, filter, pickupGrade: intent.pickupGrade }); break;
    }
    case "slot":
      if (!integer(intent.slotIndex, 0, 9) || intent.slotIndex === pet.slotIndex
        || source.records.some(other => other.petType !== pet.petType && other.slotIndex === intent.slotIndex)) return null;
      creature = Object.freeze({ ...pet, slotIndex: intent.slotIndex }); break;
    default: return null;
  }
  return Object.freeze({ type: "updateIntelligentCreature", creature, summonMe, unsummonMe, releaseMe });
}
/** Gateway expects numeric i64 literals, not decimal strings. Only these three
 * validated i64 fields are replaced; all names/filters remain normal JSON. */
export function creaturePlayerCommandJson(command: unknown): string | null {
  if (!object(command)) return null;
  const creature = readCreaturePlayerRecord(command.creature);
  if (!creature || !exact(command, ["type", "creature", "summonMe", "unsummonMe", "releaseMe"])
    || command.type !== "updateIntelligentCreature" || [command.summonMe, command.unsummonMe, command.releaseMe]
      .some(v => typeof v !== "boolean") || [command.summonMe, command.unsummonMe, command.releaseMe].filter(Boolean).length > 1) return null;
  return JSON.stringify({ type: "updateIntelligentCreature", creature, summonMe: command.summonMe,
    unsummonMe: command.unsummonMe, releaseMe: command.releaseMe }).replace(/"(expireBinaryDatetime|blackstoneTime|maintainFoodTime)":"(-?\d+)"/g,
    (_all, key: string, value: string) => `"${key}":${value}`);
}

export class CreaturePlayerOperations {
  private row: { proof: CreaturePlayerProof; entered: boolean; unknown: boolean } | null = null;
  pending(): CreaturePlayerProof | null { return this.row?.proof ?? null; }
  reserve(intent: CreaturePlayerIntent, source: CreaturePlayerSource): CreaturePlayerProof | null {
    if (this.row) return null;
    const captured = makeCreaturePlayerSource(source.records, source.summonedPetType, source.renameEnabled,
      source.owner, source.revision, source.fullList);
    if (!captured) return null;
    const command = creaturePlayerCommand(intent, captured);
    if (!command) return null;
    const frozenIntent = Object.freeze({ ...intent, ...(intent.kind === "options" ? { filter: Object.freeze({ ...intent.filter }) } : {}) });
    const proof = Object.freeze({ source: captured, intent: frozenIntent, command });
    this.row = { proof, entered: false, unknown: false }; return proof;
  }
  allows(proof: CreaturePlayerProof, source: CreaturePlayerSource | null, command: unknown): boolean {
    return this.row?.proof === proof && !this.row.entered && !!source
      && sameSocialReplyOwner(proof.source.owner, source.owner)
      && creaturePlayerSourceKey(proof.source) === creaturePlayerSourceKey(source)
      && !!creaturePlayerCommand(proof.intent, source)
      && creaturePlayerCommandJson(command as CreaturePlayerCommand) === creaturePlayerCommandJson(proof.command);
  }
  claim(proof: CreaturePlayerProof, source: CreaturePlayerSource | null, command: unknown): boolean {
    if (!this.allows(proof, source, command)) return false;
    this.row!.entered = true; return true;
  }
  cancelDefinitelyUnsent(proof: CreaturePlayerProof): boolean {
    if (this.row?.proof !== proof || this.row.entered) return false;
    this.row = null; return true;
  }
  markUnknown(proof: CreaturePlayerProof): void { if (this.row?.proof === proof && this.row.entered) this.row.unknown = true; }
  /** A matching changed authoritative target state is convergence, never ACK.
   * Unrelated or unchanged snapshots cannot release an entered operation. */
  observeAuthority(source: CreaturePlayerSource): boolean {
    const row = this.row;
    if (!row?.entered || !sameSocialServiceStream(row.proof.source.owner, source.owner)
      || source.revision <= row.proof.source.revision) return false;
    const { intent, command } = row.proof;
    const actual = source.records.find(p => p.petType === intent.petType);
    let converged = false;
    switch (intent.kind) {
      case "release": converged = source.fullList && !actual; break;
      case "summon": converged = !!actual && source.summonedPetType === intent.petType; break;
      case "dismiss": converged = !!actual && source.summonedPetType !== intent.petType; break;
      case "rename": converged = actual?.customName === command.creature.customName; break;
      case "mode": converged = actual?.petMode === command.creature.petMode; break;
      case "slot": converged = actual?.slotIndex === command.creature.slotIndex; break;
      case "options": converged = !!actual && actual.pickupGrade === command.creature.pickupGrade
        && JSON.stringify(actual.filter) === JSON.stringify(command.creature.filter); break;
    }
    if (converged) this.row = null;
    return converged;
  }
  retireConnection(socket: object, connectionGeneration: number): boolean {
    if (!this.row) return true;
    const before = this.row.proof.source.owner;
    if (!object(socket) || socket === before.socket || !integer(connectionGeneration, before.connectionGeneration + 1, Number.MAX_SAFE_INTEGER)) return false;
    this.row = null; return true;
  }
}
