import type { SocialReplyOwner } from "./social-incoming-replies";

type Row = Record<string, unknown>;
export type GuildBuffDefinition = Readonly<{
  id: number; icon: number; name: string; levelRequirement: number; pointsRequirement: number;
  timeLimit: number; activationCost: number; stats: readonly Readonly<{ stat: number; value: number }>[];
}>;
export type GuildBuffState = Readonly<{ id: number; active: boolean; activeTimeRemaining: number; revision: number }>;
export type GuildBuffList = Readonly<{
  remove: 0 | 1; activeBuffs: readonly Omit<GuildBuffState, "revision">[]; guildBuffs: readonly GuildBuffDefinition[];
}>;
export type GuildBuffSource = Readonly<{
  owner: SocialReplyOwner; guildName: string; guildGeneration: number; revision: number; sourceKey: string;
  statusRevision: number; buffRevision: number; catalogRevision: number;
  level: number; sparePoints: number; gold: number; myOptions: number; myRankId: number;
  catalogReady: boolean; enabledReady: boolean;
  catalog: readonly GuildBuffDefinition[]; enabled: readonly GuildBuffState[];
}>;
export type GuildBuffWire = Readonly<{ type: "guildBuffUpdate"; action: 0 | 1 | 2; id: number }>;
export type GuildBuffActionDto = Readonly<{
  owner: SocialReplyOwner; guildName: string; guildGeneration: number; revision: number; sourceKey: string;
  wire: GuildBuffWire;
}>;
export type GuildBuffActionError = "unavailable" | "permission" | "level" | "points" | "funds" | "active";
export type GuildBuffActionPlan = Readonly<{ wire: GuildBuffWire; goldCost: number; pointsCost: number }>;
export type GuildBuffRowStatus = "insufficientLevel" | "available" | "countingDown" | "expired" | "obtained";
export type GuildBuffRow = Readonly<{
  definition: GuildBuffDefinition; state: GuildBuffState | null; icon: number;
  status: GuildBuffRowStatus; error: GuildBuffActionError | null; plan: GuildBuffActionPlan | null;
}>;
const row = (v: unknown): v is Row => typeof v === "object" && v !== null && !Array.isArray(v);
const integer = (v: unknown, min = 0, max = Number.MAX_SAFE_INTEGER): v is number =>
  typeof v === "number" && Number.isSafeInteger(v) && v >= min && v <= max;
const name = (v: unknown): v is string => typeof v === "string" && !!v.trim() && v.length <= 256 && !v.includes("\0");
function canonical(v: unknown): string {
  if (v === null || typeof v === "string" || typeof v === "boolean") return JSON.stringify(v);
  if (typeof v === "number" && Number.isFinite(v)) return JSON.stringify(v);
  if (Array.isArray(v)) return `[${v.map(canonical).join(",")}]`;
  if (row(v)) return `{${Object.keys(v).sort().map(k => `${JSON.stringify(k)}:${canonical(v[k])}`).join(",")}}`;
  throw new Error("Incomplete guild buff source");
}
function frozen<T>(v: T): T {
  if (v && typeof v === "object") { Object.values(v).forEach(frozen); Object.freeze(v); }
  return v;
}
function validOwner(o: SocialReplyOwner): boolean {
  return !!o && [o.connectionGeneration, o.sessionGeneration, o.sceneRevision, o.playerObjectId]
    .every(n => integer(n, 1)) && row(o.socket) && name(o.mapFileName) && o.playerObjectId <= 4294967295;
}
export function sameGuildBuffPhysicalOwner(a: SocialReplyOwner, b: SocialReplyOwner): boolean {
  return a.socket === b.socket && a.connectionGeneration === b.connectionGeneration
    && a.sessionGeneration === b.sessionGeneration && a.playerObjectId === b.playerObjectId;
}
function sameOwner(a: SocialReplyOwner, b: SocialReplyOwner): boolean {
  return sameGuildBuffPhysicalOwner(a, b) && a.sceneRevision === b.sceneRevision && a.mapFileName === b.mapFileName;
}

/** GuildBuffInfo is snake_case inside the camelCase GuildBuffList envelope. */
export function parseGuildBuffDefinition(raw: unknown): GuildBuffDefinition | null {
  if (!row(raw) || !integer(raw.id, 0, 2147483647) || !integer(raw.icon, -2147483648, 2147483647) || !name(raw.name)
    || !integer(raw.level_requirement, 0, 255) || !integer(raw.points_requirement, 0, 255)
    || !integer(raw.time_limit, 0, 2147483647) || !integer(raw.activation_cost, 0, 2147483647)
    || !Array.isArray(raw.stats) || raw.stats.length > 256
    || !raw.stats.every(s => row(s) && integer(s.stat, 0, 255) && integer(s.value, -2147483648, 2147483647))) return null;
  return frozen({ id: raw.id, icon: raw.icon, name: raw.name, levelRequirement: raw.level_requirement,
    pointsRequirement: raw.points_requirement, timeLimit: raw.time_limit, activationCost: raw.activation_cost,
    stats: raw.stats.map(s => ({ stat: s.stat as number, value: s.value as number })) });
}
export function parseGuildBuffList(raw: unknown): GuildBuffList | null {
  if (!row(raw) || (raw.remove !== 0 && raw.remove !== 1) || !Array.isArray(raw.activeBuffs)
    || !Array.isArray(raw.guildBuffs) || raw.activeBuffs.length > 256 || raw.guildBuffs.length > 256) return null;
  const activeBuffs: Omit<GuildBuffState, "revision">[] = [], guildBuffs: GuildBuffDefinition[] = [];
  for (const active of raw.activeBuffs) {
    if (!row(active) || !integer(active.id, 0, 2147483647) || typeof active.active !== "boolean"
      || !integer(active.active_time_remaining, -2147483648, 2147483647)
      || (active.active && active.active_time_remaining < 0)) return null;
    activeBuffs.push({ id: active.id, active: active.active, activeTimeRemaining: active.active_time_remaining });
  }
  for (const definition of raw.guildBuffs) {
    const parsed = parseGuildBuffDefinition(definition); if (!parsed) return null;
    guildBuffs.push(parsed);
  }
  if (new Set(activeBuffs.map(b => b.id)).size !== activeBuffs.length || new Set(guildBuffs.map(b => b.id)).size !== guildBuffs.length) return null;
  return frozen({ remove: raw.remove, activeBuffs, guildBuffs });
}

type Status = { guildName: string; level: number; sparePoints: number; gold: number; myOptions: number; myRankId: number; buffCount: number };
function parseStatus(raw: unknown): Status | null {
  if (!row(raw) || !name(raw.guildName) || !integer(raw.level, 0, 255) || !integer(raw.sparePoints, 0, 255)
    || !integer(raw.gold, 0, 4294967295) || !integer(raw.myOptions, 0, 255)
    || !integer(raw.myRankId, 0, 2147483647) || !integer(raw.buffCount, 0, 255)) return null;
  return { guildName: raw.guildName, level: raw.level, sparePoints: raw.sparePoints,
    gold: raw.gold, myOptions: raw.myOptions, myRankId: raw.myRankId, buffCount: raw.buffCount };
}

/** Cached authority survives UI closure. Membership and physical sessions are separate boundaries. */
export class GuildBuffAuthority {
  private owner: SocialReplyOwner | null = null;
  private guildName = "";
  private generation = 0;
  private revision = 0;
  private statusRevision = 0;
  private buffRevision = 0;
  private catalogRevision = 0;
  private status: Status | null = null;
  private catalog: readonly GuildBuffDefinition[] = [];
  private enabled = new Map<number, GuildBuffState>();
  receiveStatus(payload: unknown, owner: SocialReplyOwner): boolean {
    if (!this.accept(owner)) return false;
    ++this.revision;
    if (row(payload) && typeof payload.guildName === "string" && payload.guildName !== this.guildName) {
      this.guildName = payload.guildName; ++this.generation;
      this.catalog = []; this.enabled.clear(); this.catalogRevision = this.buffRevision = 0;
    }
    this.status = parseStatus(payload);
    this.statusRevision = this.status ? this.revision : 0;
    const withdrew = row(payload) && payload.guildName === "" && payload.myRankId === -1 && payload.myOptions === 0
      && integer(payload.level, 0, 255) && integer(payload.sparePoints, 0, 255)
      && integer(payload.gold, 0, 4294967295) && integer(payload.buffCount, 0, 255);
    return this.status !== null || withdrew;
  }
  receiveList(payload: unknown, owner: SocialReplyOwner, guildName: string): boolean {
    if (!this.owner || !validOwner(owner) || !sameGuildBuffPhysicalOwner(this.owner, owner)
      || !name(guildName) || guildName !== this.guildName) return false;
    const parsed = parseGuildBuffList(payload);
    if (!parsed) { this.catalog = []; this.catalogRevision = 0; ++this.revision; return false; }
    ++this.revision;
    if (parsed.guildBuffs.length) { this.catalog = parsed.guildBuffs; this.catalogRevision = this.revision; }
    // Empty active arrays are deltas, not an authoritative command to erase acquired buffs.
    const next = new Map(this.enabled);
    for (const buff of parsed.activeBuffs) {
      if (parsed.remove === 1) { next.delete(buff.id); continue; }
      const previous = next.get(buff.id);
      const changed = !previous || previous.active !== buff.active || previous.activeTimeRemaining !== buff.activeTimeRemaining;
      next.set(buff.id, frozen({ ...buff, revision: changed ? this.revision : previous?.revision ?? this.revision }));
    }
    if (next.size > 256) { this.status = null; this.statusRevision = 0; return false; }
    this.enabled = next;
    if (parsed.activeBuffs.length) this.buffRevision = this.revision;
    return true;
  }
  /** A real rank/permission replacement cannot leave stale CanActivateBuff authority. */
  invalidatePermissions(owner: SocialReplyOwner, guildName: string): boolean {
    if (!this.owner || !sameGuildBuffPhysicalOwner(this.owner, owner) || guildName !== this.guildName) return false;
    this.status = null; this.statusRevision = 0; ++this.revision; return true;
  }
  read(owner: SocialReplyOwner, guildName: string): GuildBuffSource | null {
    if (!validOwner(owner) || !this.owner || !sameGuildBuffPhysicalOwner(this.owner, owner)
      || !this.status || this.guildName !== guildName || !name(guildName)) return null;
    const enabled = [...this.enabled.values()].sort((a, b) => a.id - b.id);
    const value = { guildName, guildGeneration: this.generation, revision: this.revision,
      statusRevision: this.statusRevision, buffRevision: this.buffRevision, catalogRevision: this.catalogRevision,
      level: this.status.level, sparePoints: this.status.sparePoints, gold: this.status.gold,
      myOptions: this.status.myOptions, myRankId: this.status.myRankId,
      catalogReady: this.catalogRevision > 0, enabledReady: enabled.length === this.status.buffCount,
      catalog: this.catalog, enabled };
    return Object.freeze({ ...frozen(value), sourceKey: canonical(value), owner: Object.freeze({ ...owner }) });
  }
  retireSession(nextOwner: SocialReplyOwner): boolean {
    if (!validOwner(nextOwner) || !this.owner || sameGuildBuffPhysicalOwner(this.owner, nextOwner)) return false;
    this.owner = { ...nextOwner }; this.guildName = ""; this.status = null; this.catalog = []; this.enabled.clear();
    this.statusRevision = this.buffRevision = this.catalogRevision = 0; ++this.generation; ++this.revision;
    return true;
  }
  private accept(owner: SocialReplyOwner): boolean {
    if (!validOwner(owner)) return false;
    if (!this.owner) this.owner = { ...owner };
    return sameGuildBuffPhysicalOwner(this.owner, owner);
  }
}

export function guildBuffActionError(source: GuildBuffSource, id: number): GuildBuffActionError | null {
  const definition = source.catalog.find(b => b.id === id), current = source.enabled.find(b => b.id === id);
  if (!source.catalogReady || !source.enabledReady || !definition) return "unavailable";
  if (!(source.myOptions & 128)) return "permission";
  if (current) {
    if (source.gold < definition.activationCost) return "funds";
    return current.active ? "active" : null;
  }
  if (source.level < definition.levelRequirement) return "level";
  if (source.sparePoints < definition.pointsRequirement) return "points";
  // Production acquisition also charges activation gold for timed buffs.
  if (definition.timeLimit > 0 && source.gold < definition.activationCost) return "funds";
  return null;
}
export function planGuildBuffAction(source: GuildBuffSource, action: 0 | 1 | 2, id: number): GuildBuffActionPlan | null {
  if (action === 0) return id === 0 ? frozen({ wire: { type: "guildBuffUpdate", action, id }, goldCost: 0, pointsCost: 0 }) : null;
  if (action !== 1 && action !== 2) return null;
  const definition = source.catalog.find(b => b.id === id), current = source.enabled.find(b => b.id === id);
  if (!definition || guildBuffActionError(source, id) || (action === 1 ? !!current : !current)) return null;
  return frozen({ wire: { type: "guildBuffUpdate", action, id },
    goldCost: action === 1 && definition.timeLimit === 0 ? 0 : definition.activationCost,
    pointsCost: action === 1 ? definition.pointsRequirement : 0 });
}
export function guildBuffRows(source: GuildBuffSource): readonly GuildBuffRow[] {
  return frozen(source.catalog.map(definition => {
    const state = source.enabled.find(b => b.id === definition.id) ?? null;
    const status: GuildBuffRowStatus = !state ? definition.levelRequirement > source.level ? "insufficientLevel" : "available"
      : definition.timeLimit <= 0 ? "obtained" : state.active ? "countingDown" : "expired";
    const action = state ? 2 : 1;
    return frozen({ definition, state, icon: Math.min(2147483647, definition.icon + (state ? Number(state.active) : 2)), status,
      error: guildBuffActionError(source, definition.id), plan: planGuildBuffAction(source, action, definition.id) });
  }));
}
/** Named Crystal Stat labels from protocol/types.rs; these are presentation, never authority IDs. */
export function guildBuffStatLabel(stat: number): string {
  const labels: Record<number, string> = {
    0: "MinAC", 1: "MaxAC", 2: "MinMAC", 3: "MaxMAC", 4: "MinDC", 5: "MaxDC", 6: "MinMC", 7: "MaxMC", 8: "MinSC", 9: "MaxSC",
    10: "Accuracy", 11: "Agility", 12: "HP", 13: "MP", 14: "AttackSpeed", 15: "Luck", 16: "BagWeight", 17: "HandWeight", 18: "WearWeight",
    19: "Reflect", 20: "Strong", 21: "Holy", 22: "Freezing", 23: "PoisonAttack", 30: "MagicResist", 31: "PoisonResist",
    32: "HealthRecovery", 33: "SpellRecovery", 34: "PoisonRecovery", 35: "CriticalRate", 36: "CriticalDamage",
    40: "MaxACRatePercent", 41: "MaxMACRatePercent", 42: "MaxDCRatePercent", 43: "MaxMCRatePercent", 44: "MaxSCRatePercent",
    45: "AttackSpeedRatePercent", 46: "HPRatePercent", 47: "MPRatePercent", 48: "HPDrainRatePercent", 100: "ExpRatePercent",
    101: "ItemDropRatePercent", 102: "GoldDropRatePercent", 103: "MineRatePercent", 104: "GemRatePercent", 105: "FishRatePercent",
    106: "CraftRatePercent", 107: "SkillGainMultiplier", 108: "AttackBonus", 120: "LoverExpRatePercent", 121: "MentorDamageRatePercent",
    123: "MentorExpRatePercent", 124: "DamageReductionPercent", 125: "EnergyShieldPercent", 126: "EnergyShieldHPGain",
    127: "ManaPenaltyPercent", 128: "TeleportManaPenaltyPercent", 129: "Hero",
  };
  return labels[stat] ?? `Stat${stat}`;
}
export function captureGuildBuffAction(source: GuildBuffSource, action: 0 | 1 | 2, id: number): GuildBuffActionDto | null {
  const plan = planGuildBuffAction(source, action, id);
  return plan ? Object.freeze({ owner: source.owner, guildName: source.guildName, guildGeneration: source.guildGeneration,
    revision: source.revision, sourceKey: source.sourceKey, wire: plan.wire }) : null;
}
export function guildBuffActionCurrent(dto: GuildBuffActionDto, source: GuildBuffSource): boolean {
  return sameOwner(dto.owner, source.owner) && dto.guildName === source.guildName && dto.guildGeneration === source.guildGeneration
    && dto.revision === source.revision && dto.sourceKey === source.sourceKey
    && !!planGuildBuffAction(source, dto.wire.action, dto.wire.id);
}

export type GuildBuffProof = Readonly<{ dto: GuildBuffActionDto; wire: GuildBuffWire;
  beforeCatalogRevision: number; beforeBuffRevision: number; beforeStatusRevision: number; definition: GuildBuffDefinition | null }>;
export type GuildBuffPending = Readonly<{ proof: GuildBuffProof; state: "reserved" | "entered" | "unknown" }>;
/** A broadcast may prove the desired target exists; it never proves this client owned a successful ACK. */
export class GuildBuffOperations {
  private value: GuildBuffPending | null = null;
  get pending(): GuildBuffPending | null { return this.value; }
  reserve(dto: GuildBuffActionDto, source: GuildBuffSource): GuildBuffProof | null {
    if (this.value || !guildBuffActionCurrent(dto, source)) return null;
    const captured = Object.freeze({ ...dto, owner: Object.freeze({ ...dto.owner }), wire: Object.freeze({ ...dto.wire }) });
    const proof = Object.freeze({ dto: captured, wire: captured.wire, beforeCatalogRevision: source.catalogRevision,
      beforeBuffRevision: source.buffRevision, beforeStatusRevision: source.statusRevision,
      definition: source.catalog.find(b => b.id === dto.wire.id) ?? null });
    this.value = Object.freeze({ proof, state: "reserved" }); return proof;
  }
  allows(proof: GuildBuffProof, source: GuildBuffSource, body: unknown = proof.wire): boolean {
    if (this.value?.proof !== proof || this.value.state !== "reserved" || !guildBuffActionCurrent(proof.dto, source)) return false;
    try { return canonical(body) === canonical(proof.wire); } catch { return false; }
  }
  claim(proof: GuildBuffProof, source: GuildBuffSource, body: unknown = proof.wire): boolean {
    if (!this.allows(proof, source, body)) return false;
    this.value = Object.freeze({ proof, state: "entered" }); return true;
  }
  cancelDefinitelyUnsent(proof: GuildBuffProof): boolean {
    if (this.value?.proof !== proof || !["reserved", "entered"].includes(this.value.state)) return false;
    this.value = null; return true;
  }
  outcomeUnknown(proof: GuildBuffProof): boolean {
    if (this.value?.proof !== proof || this.value.state !== "entered") return false;
    this.value = Object.freeze({ proof, state: "unknown" }); return true;
  }
  observe(source: GuildBuffSource): "observed" | null {
    const pending = this.value, proof = pending?.proof;
    if (!pending || !proof || pending.state === "reserved" || !sameGuildBuffPhysicalOwner(proof.dto.owner, source.owner)
      || proof.dto.guildName !== source.guildName || proof.dto.guildGeneration !== source.guildGeneration) return null;
    if (proof.wire.action === 0) {
      if (!source.catalogReady || source.catalogRevision <= proof.beforeCatalogRevision) return null;
    } else {
      const target = source.enabled.find(b => b.id === proof.wire.id), definition = source.catalog.find(b => b.id === proof.wire.id);
      if (!target?.active || target.revision <= proof.beforeBuffRevision || !definition || !proof.definition
        || canonical(definition) !== canonical(proof.definition)
        // Costs and spare points must have a complete fresh GuildStatus after the target transition.
        || source.statusRevision <= target.revision || source.statusRevision <= proof.beforeStatusRevision) return null;
    }
    this.value = null; return "observed";
  }
  retireSession(nextOwner: SocialReplyOwner): boolean {
    if (!validOwner(nextOwner) || !this.value || sameGuildBuffPhysicalOwner(this.value.proof.dto.owner, nextOwner)) return false;
    this.value = null; return true;
  }
}
