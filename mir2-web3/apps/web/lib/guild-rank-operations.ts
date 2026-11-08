import { GUILD_RANK_PERMISSION_KEYS, guildRankCreateCommand, guildRankPermissionCommand, guildRankRenameCommand, projectGuildRanks,
  type GuildRankPermission, type ProjectedGuildRanks } from "./social-parity-actions";
import type { SocialReplyOwner } from "./social-incoming-replies";

export type GuildRankEditSource = Readonly<{
  owner: SocialReplyOwner; scope: object; guildName: string; actor: string;
  ownRankIndex: number; permissions: readonly string[];
  ranks: readonly {index: number; name: string; permissions: readonly string[]}[];
  /** Create needs a complete, packet-versioned roster and the authenticated character index. */
  roster?: Readonly<{revision: number; complete: boolean; actorIndex: number;
    members: readonly {id: number; name: string; rankIndex: number}[]}>;
}>;
export type GuildRankEditCommand = NonNullable<ReturnType<typeof guildRankRenameCommand>>
  | NonNullable<ReturnType<typeof guildRankPermissionCommand>> | NonNullable<ReturnType<typeof guildRankCreateCommand>>;
export type GuildRankEditProof = Readonly<{
  kind: "rename" | "permission" | "create"; owner: SocialReplyOwner; scope: object;
  guildName: string; actor: string; rankIndex: number; sourceKey: string;
  command: Readonly<GuildRankEditCommand>; deadline: number;
}>;
export type GuildRankReadback = Readonly<{revision: number; complete: boolean; value: ProjectedGuildRanks}>;
type Roster = {ranks: {index: number; name: string; permissions: string[]}[];
  members: {id: number; name: string; rankIndex: number}[]};
type CreateContext = {before: Roster; revision: number; insertion: number; actorIndex: number};
type OperationRow = {entered: boolean; create?: CreateContext;
  candidate?: {name: string; revision: number}; readRequested?: boolean};
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
function snapshotRoster(ranks: unknown, members: unknown): Roster | null {
  if (!Array.isArray(ranks) || ranks.length < 1 || ranks.length > 255
    || !Array.isArray(members) || members.length > 8192) return null;
  const result: Roster = {ranks: [], members: []}, ids = new Set<number>(), names = new Set<string>();
  for (const rank of ranks) {
    if (!record(rank) || !u8(rank.index) || !name(rank.name) || !permissions(rank.permissions)) return null;
    result.ranks.push({index: rank.index, name: rank.name,
      permissions: GUILD_RANK_PERMISSION_KEYS.filter(key => (rank.permissions as string[]).includes(key))});
  }
  result.ranks.sort((a,b) => a.index - b.index);
  if (result.ranks.some((rank,index) => rank.index !== index)) return null;
  for (const member of members) {
    if (!record(member) || !Number.isSafeInteger(member.id) || Number(member.id) < 0 || Number(member.id) > 0x7fff_ffff
      || !name(member.name) || !u8(member.rankIndex) || !result.ranks[member.rankIndex]
      || ids.has(Number(member.id)) || names.has(member.name.replace(/[A-Z]/g,c => c.toLowerCase()))) return null;
    ids.add(Number(member.id)); names.add(member.name.replace(/[A-Z]/g,c => c.toLowerCase()));
    result.members.push({id:Number(member.id),name:member.name,rankIndex:member.rankIndex});
  }
  result.members.sort((a,b) => a.id - b.id);
  return result;
}
function createContext(source: GuildRankEditSource | null): {context: CreateContext; key: string} | null {
  if (!source?.roster || !source.roster.complete || !clock(source.roster.revision) || source.roster.revision === 0
    || !Number.isSafeInteger(source.roster.actorIndex) || source.roster.actorIndex < 0 || source.roster.actorIndex > 0x7fff_ffff
    || !authority(source,source.ownRankIndex)) return null;
  const before = snapshotRoster(source.ranks,source.roster.members);
  if (!before || !guildRankCreateCommand(before.ranks.length)
    || !before.members.some(member => member.id === source.roster!.actorIndex && member.name === source.actor
      && member.rankIndex === source.ownRankIndex)
    || JSON.stringify(before.ranks[source.ownRankIndex].permissions)
      !== JSON.stringify(GUILD_RANK_PERMISSION_KEYS.filter(key => source.permissions.includes(key)))) return null;
  const context = {before,revision:source.roster.revision,actorIndex:source.roster.actorIndex,
    insertion:before.ranks.length > 1 ? before.ranks.length - 1 : 1};
  return {context,key:JSON.stringify([source.guildName,source.actor,source.ownRankIndex,context])};
}
function createdRosterMatches(context: CreateContext, newName: string, readback: GuildRankReadback): boolean {
  if (!readback.complete) return false;
  const current = snapshotRoster(readback.value.ranks,readback.value.members);
  if (!current) return false;
  const ranks = context.before.ranks.map(rank => ({...rank,index:rank.index >= context.insertion ? rank.index + 1 : rank.index}));
  ranks.splice(context.insertion,0,{index:context.insertion,name:newName,permissions:[]});
  const members = context.before.members.map(member => ({...member,
    rankIndex:member.rankIndex >= context.insertion ? member.rankIndex + 1 : member.rankIndex}));
  return JSON.stringify(current) === JSON.stringify({ranks,members});
}

/** Local input admission only. Names remain drafts; checkbox state always comes from server ranks. */
export class GuildRankOperations {
  private rows = new Map<GuildRankEditProof,OperationRow>();
  private permissionReadyAt = 0;
  private createReadyAt = 0;

  view(now: number): Readonly<{renameRankIndices: readonly number[]; permission: boolean; create?: boolean}> {
    return Object.freeze({renameRankIndices: Object.freeze([...this.rows.keys()]
      .filter(p => p.kind === "rename" && now <= p.deadline).map(p => p.rankIndex)),
      permission: now < this.permissionReadyAt,
      ...([...this.rows.keys()].some(proof => proof.kind === "create") || now < this.createReadyAt ? {create:true} : {})});
  }
  nextWake(now: number): number | null {
    const deadlines = [...this.rows.keys()].filter(p => p.kind === "rename" && now <= p.deadline).map(p => p.deadline + 1);
    if (now < this.permissionReadyAt) deadlines.push(this.permissionReadyAt);
    if (now < this.createReadyAt) deadlines.push(this.createReadyAt);
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
      || [...this.rows.keys()].some(p => p.kind === "create" && p.scope === source.scope)
      || [...this.rows.keys()].some(p => p.kind === "rename" && p.scope === source.scope && p.rankIndex === index)) return null;
    return this.reserve("rename",command,source,key,now + 5000);
  }
  reservePermission(index: number, option: GuildRankPermission, enabled: boolean,
    source: GuildRankEditSource | null, now: number): GuildRankEditProof | null {
    const command = guildRankPermissionCommand(index,option,enabled), key = authority(source,index);
    if (!clock(now) || now > Number.MAX_SAFE_INTEGER - 301 || now < this.permissionReadyAt || !command || !key || !source) return null;
    if ([...this.rows.keys()].some(p => p.kind === "create" && p.scope === source.scope)) return null;
    const target = source.ranks.find(rank => rank.index === index)!;
    if (target.permissions.includes(option) === enabled) return null;
    return this.reserve("permission",command,source,key,Number.MAX_SAFE_INTEGER);
  }
  reserveCreate(source: GuildRankEditSource | null, now: number): GuildRankEditProof | null {
    try {
      const captured = createContext(source);
      if (!source || !captured || !clock(now) || now > Number.MAX_SAFE_INTEGER - 5000 || now < this.createReadyAt
        || [...this.rows.keys()].some(proof => proof.scope === source.scope)) return null;
      const command = guildRankCreateCommand(captured.context.before.ranks.length)!;
      const proof = this.reserve("create",command,source,captured.key,Number.MAX_SAFE_INTEGER);
      this.rows.get(proof)!.create = captured.context;
      return proof;
    } catch { return null; }
  }
  private reserve(kind: GuildRankEditProof["kind"], command: GuildRankEditCommand,
    source: GuildRankEditSource, sourceKey: string, deadline: number): GuildRankEditProof {
    const proof = Object.freeze({kind,command:Object.freeze({...command}),owner:Object.freeze({...source.owner}),
      scope:source.scope,guildName:source.guildName,actor:source.actor,rankIndex:command.rankIndex,sourceKey,deadline});
    this.rows.set(proof,{entered:false});
    return proof;
  }
  allows(proof: GuildRankEditProof, source: GuildRankEditSource | null, command: unknown, now: number): boolean {
    try {
    const row = this.rows.get(proof);
    return !!row && !row.entered && !!source && clock(now) && now <= proof.deadline
      && source.scope === proof.scope && exactOwner(proof.owner,source.owner)
      && (proof.kind === "create" ? createContext(source)?.key : authority(source,proof.rankIndex)) === proof.sourceKey
      && sameCommand(command,proof.command)
      && (proof.kind !== "create" || now >= this.createReadyAt && now <= Number.MAX_SAFE_INTEGER - 5000)
      && (proof.kind !== "permission" || now >= this.permissionReadyAt)
      && this.rows.get(proof) === row && !row.entered;
    } catch { return false; }
  }
  claim(proof: GuildRankEditProof, source: GuildRankEditSource | null, command: unknown, now: number): boolean {
    const row = this.rows.get(proof);
    if (!row || !this.allows(proof,source,command,now) || this.rows.get(proof) !== row || row.entered) return false;
    row.entered = true;
    if (proof.kind === "permission") this.permissionReadyAt = now + 300;
    if (proof.kind === "create") this.createReadyAt = now + 5000;
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
  receipt(payload: unknown, owner: SocialReplyOwner | null, scope: object | null, readback?: GuildRankReadback): boolean {
    let created = false;
    try {
      if (record(payload) && readback && clock(readback.revision)) {
        for (const [proof,row] of this.rows) {
          if (!row.entered || proof.kind !== "create" || !row.create || proof.scope !== scope || !physical(proof.owner,owner)
            || readback.revision <= row.create.revision) continue;
          let matchingSix = false;
          if (payload.status === 6 && payload.rankIndex === 0 && payload.name === proof.actor
            && Array.isArray(payload.ranks) && payload.ranks.length === 1) {
            const parsed = projectGuildRanks(payload.ranks), rank = payload.ranks[0];
            if (parsed && record(rank) && rank.index === row.create.insertion && rank.options === 0
              && Array.isArray(rank.members) && rank.members.length === 0 && name(rank.name)
              && (!row.candidate || row.candidate.name === rank.name)) {
              matchingSix = true;
              row.candidate ??= {name:rank.name,revision:readback.revision};
            }
          }
          const full = payload.status === 255 && payload.name === "" && payload.rankIndex === 0
            && Array.isArray(payload.ranks) ? projectGuildRanks(payload.ranks) : null;
          const matchingFull = full && JSON.stringify(snapshotRoster(full.ranks,full.members))
            === JSON.stringify(snapshotRoster(readback.value.ranks,readback.value.members));
          if (row.candidate && (matchingSix || matchingFull && readback.revision > row.candidate.revision)
            && createdRosterMatches(row.create,row.candidate.name,readback)) {
            this.rows.delete(proof); created = true;
          }
        }
      }
    } catch { /* An incomplete/ambiguous readback cannot resolve an entered mutation. */ }
    if (!record(payload) || payload.status !== 7 || !name(payload.name) || !Array.isArray(payload.ranks)
      || payload.ranks.length !== 1 || !record(payload.ranks[0])) return created;
    const rank = payload.ranks[0];
    const parsed = projectGuildRanks(payload.ranks);
    if (!u8(rank.index) || typeof rank.name !== "string" || !parsed) return created;
    let matched = created;
    for (const [proof,row] of this.rows) if (row.entered && proof.kind === "rename"
      && physical(proof.owner,owner) && proof.scope === scope && proof.actor === payload.name
      && proof.rankIndex === rank.index && proof.command.rankName === rank.name) {
      this.rows.delete(proof); matched = true;
    }
    return matched;
  }
  takeCreateReadbackRequest(owner: SocialReplyOwner | null, scope: object | null): boolean {
    for (const [proof,row] of this.rows) if (proof.kind === "create" && row.entered && row.candidate && !row.readRequested
      && proof.scope === scope && physical(proof.owner,owner)) { row.readRequested = true; return true; }
    return false;
  }
  retire(): void { this.rows.clear(); this.permissionReadyAt = 0; this.createReadyAt = 0; }
}
