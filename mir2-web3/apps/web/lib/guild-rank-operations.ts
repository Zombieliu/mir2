import { GUILD_RANK_PERMISSION_KEYS, guildRankPermissionCommand, guildRankRenameCommand, projectGuildRanks,
  type GuildRankPermission } from "./social-parity-actions";
import type { SocialReplyOwner } from "./social-incoming-replies";

export type GuildRankEditSource = Readonly<{
  owner: SocialReplyOwner; scope: object; guildName: string; actor: string;
  ownRankIndex: number; permissions: readonly string[];
  ranks: readonly {index: number; name: string; permissions: readonly string[]}[];
}>;
export type GuildRankEditCommand = NonNullable<ReturnType<typeof guildRankRenameCommand>>
  | NonNullable<ReturnType<typeof guildRankPermissionCommand>>;
export type GuildRankEditProof = Readonly<{
  kind: "rename" | "permission"; owner: SocialReplyOwner; scope: object;
  guildName: string; actor: string; rankIndex: number; sourceKey: string;
  command: Readonly<GuildRankEditCommand>; deadline: number;
}>;
const record = (v: unknown): v is Record<string, unknown> => !!v && typeof v === "object" && !Array.isArray(v);
const u8 = (v: unknown): v is number => typeof v === "number" && Number.isInteger(v) && v >= 0 && v <= 255;
const clock = (v: unknown): v is number => typeof v === "number" && Number.isSafeInteger(v) && v >= 0;
const name = (v: unknown): v is string => typeof v === "string" && v.length > 0 && v.length <= 256 && !v.includes("\0");
const physical = (a: SocialReplyOwner, b: SocialReplyOwner | null) => !!b && a.socket === b.socket
  && a.connectionGeneration === b.connectionGeneration && a.sessionGeneration === b.sessionGeneration
  && a.playerObjectId === b.playerObjectId;
const exactOwner = (a: SocialReplyOwner, b: SocialReplyOwner) => physical(a,b)
  && a.sceneRevision === b.sceneRevision && a.mapFileName === b.mapFileName;
const permissions = (v: unknown): v is string[] => Array.isArray(v) && v.length <= 8
  && new Set(v).size === v.length && v.every(p => (GUILD_RANK_PERMISSION_KEYS as readonly unknown[]).includes(p));
function authority(source: GuildRankEditSource | null, index: number): string | null {
  if (!source || !record(source.scope) || !record(source.owner) || !record(source.owner.socket)
    || ![source.owner.connectionGeneration, source.owner.sessionGeneration, source.owner.sceneRevision, source.owner.playerObjectId]
      .every(v => Number.isSafeInteger(v) && v > 0)
    || !name(source.owner.mapFileName) || !name(source.guildName) || !name(source.actor)
    || !u8(index) || !u8(source.ownRankIndex) || index < source.ownRankIndex
    || !permissions(source.permissions) || !source.permissions.includes("CanChangeRank")
    || !Array.isArray(source.ranks) || source.ranks.length > 255) return null;
  const targets = source.ranks.filter(rank => rank.index === index);
  if (targets.length !== 1 || !name(targets[0].name) || !permissions(targets[0].permissions)) return null;
  return JSON.stringify([source.guildName, source.actor, source.ownRankIndex, source.permissions,
    index, targets[0].name, targets[0].permissions]);
}
function sameCommand(value: unknown, expected: Readonly<GuildRankEditCommand>): boolean {
  if (!record(value) || "toJSON" in value || ![Object.prototype,null].includes(Object.getPrototypeOf(value))) return false;
  const keys = Reflect.ownKeys(value), wanted = Object.keys(expected);
  return keys.length === wanted.length && wanted.every(key => {
    const field = Object.getOwnPropertyDescriptor(value,key);
    return field?.enumerable && "value" in field && field.value === expected[key as keyof GuildRankEditCommand];
  });
}

/** Local input admission only. Names remain drafts; checkbox state always comes from server ranks. */
export class GuildRankOperations {
  private rows = new Map<GuildRankEditProof,{entered: boolean}>();
  private permissionReadyAt = 0;

  view(now: number): Readonly<{renameRankIndices: readonly number[]; permission: boolean}> {
    return Object.freeze({renameRankIndices: Object.freeze([...this.rows.keys()]
      .filter(p => p.kind === "rename" && now <= p.deadline).map(p => p.rankIndex)),
      permission: now < this.permissionReadyAt});
  }
  nextWake(now: number): number | null {
    const deadlines = [...this.rows.keys()].filter(p => p.kind === "rename" && now <= p.deadline).map(p => p.deadline + 1);
    if (now < this.permissionReadyAt) deadlines.push(this.permissionReadyAt);
    return deadlines.length ? Math.min(...deadlines) : null;
  }
  expire(now: number): void {
    if (!clock(now)) return;
    // Only rename has Native's explicit five-second unknown-result retry rule.
    for (const proof of this.rows.keys()) if (proof.kind === "rename" && now > proof.deadline) this.rows.delete(proof);
  }
  reserveRename(index: number, text: string, source: GuildRankEditSource | null, now: number): GuildRankEditProof | null {
    this.expire(now);
    const command = guildRankRenameCommand(index,text), key = authority(source,index);
    if (!clock(now) || now > Number.MAX_SAFE_INTEGER - 5001 || !command || !key || !source
      || [...this.rows.keys()].some(p => p.kind === "rename" && p.scope === source.scope && p.rankIndex === index)) return null;
    return this.reserve("rename",command,source,key,now + 5000);
  }
  reservePermission(index: number, option: GuildRankPermission, enabled: boolean,
    source: GuildRankEditSource | null, now: number): GuildRankEditProof | null {
    const command = guildRankPermissionCommand(index,option,enabled), key = authority(source,index);
    if (!clock(now) || now > Number.MAX_SAFE_INTEGER - 301 || now < this.permissionReadyAt || !command || !key || !source) return null;
    const target = source.ranks.find(rank => rank.index === index)!;
    if (target.permissions.includes(option) === enabled) return null;
    return this.reserve("permission",command,source,key,Number.MAX_SAFE_INTEGER);
  }
  private reserve(kind: GuildRankEditProof["kind"], command: GuildRankEditCommand,
    source: GuildRankEditSource, sourceKey: string, deadline: number): GuildRankEditProof {
    const proof = Object.freeze({kind,command:Object.freeze({...command}),owner:Object.freeze({...source.owner}),
      scope:source.scope,guildName:source.guildName,actor:source.actor,rankIndex:command.rankIndex,sourceKey,deadline});
    this.rows.set(proof,{entered:false});
    return proof;
  }
  allows(proof: GuildRankEditProof, source: GuildRankEditSource | null, command: unknown, now: number): boolean {
    const row = this.rows.get(proof);
    return !!row && !row.entered && !!source && clock(now) && now <= proof.deadline
      && source.scope === proof.scope && exactOwner(proof.owner,source.owner)
      && authority(source,proof.rankIndex) === proof.sourceKey && sameCommand(command,proof.command)
      && (proof.kind !== "permission" || now >= this.permissionReadyAt)
      && this.rows.get(proof) === row && !row.entered;
  }
  claim(proof: GuildRankEditProof, source: GuildRankEditSource | null, command: unknown, now: number): boolean {
    const row = this.rows.get(proof);
    if (!row || !this.allows(proof,source,command,now) || this.rows.get(proof) !== row || row.entered) return false;
    row.entered = true;
    if (proof.kind === "permission") this.permissionReadyAt = now + 300;
    return true;
  }
  cancelDefinitelyUnsent(proof: GuildRankEditProof): boolean {
    return this.rows.get(proof)?.entered === false && this.rows.delete(proof);
  }
  outcomeUnknown(proof: GuildRankEditProof): boolean { return this.rows.get(proof)?.entered === true; }
  finishPermissionAttempt(proof: GuildRankEditProof): void {
    // This is only the Native 300ms input throttle, never a successful permission ACK.
    // The common social budget/authoritative read model remains a separate lifecycle.
    if (proof.kind === "permission" && this.rows.get(proof)?.entered) this.rows.delete(proof);
  }
  receipt(payload: unknown, owner: SocialReplyOwner | null, scope: object | null): boolean {
    if (!record(payload) || payload.status !== 7 || !name(payload.name) || !Array.isArray(payload.ranks)
      || payload.ranks.length !== 1 || !record(payload.ranks[0])) return false;
    const rank = payload.ranks[0];
    const parsed = projectGuildRanks(payload.ranks);
    if (!u8(rank.index) || typeof rank.name !== "string" || !parsed) return false;
    let matched = false;
    for (const [proof,row] of this.rows) if (row.entered && proof.kind === "rename"
      && physical(proof.owner,owner) && proof.scope === scope && proof.actor === payload.name
      && proof.rankIndex === rank.index && proof.command.rankName === rank.name) {
      this.rows.delete(proof); matched = true;
    }
    return matched;
  }
  retire(): void { this.rows.clear(); this.permissionReadyAt = 0; }
}
