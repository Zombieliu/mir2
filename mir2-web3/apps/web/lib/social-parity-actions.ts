import type { SocialReplyOwner } from "./social-incoming-replies";

/** Crystal's eight rank options, in wire bit order. */
export const GUILD_RANK_PERMISSION_KEYS = [
  "CanChangeRank", "CanRecruit", "CanKick", "CanStoreItem", "CanRetrieveItem",
  "CanAlterAlliance", "CanChangeNotice", "CanActivateBuff",
] as const;
export type GuildRankPermission = typeof GUILD_RANK_PERMISSION_KEYS[number];

const nonNegativeI32 = (value: unknown): value is number =>
  typeof value === "number" && Number.isSafeInteger(value) && value >= 0 && value <= 0x7fff_ffff;
const validName = (value: unknown): value is string =>
  typeof value === "string" && value.trim().length > 0 && value.length <= 256 && !value.includes("\0");
const asciiFold = (name: string) => name.replace(/[A-Z]/g, letter => letter.toLowerCase());

/** Resolve only an explicit protocol identity. Display ordering never grants an index. */
export function resolveFriendCharacterIndex(
  entries: readonly {name: string; index?: number}[], name: string,
): number | null {
  if (!validName(name) || entries.length > 8192) return null;
  const matches = entries.filter(entry => validName(entry.name) && asciiFold(entry.name) === asciiFold(name));
  if (matches.length !== 1 || !nonNegativeI32(matches[0].index)) return null;
  const index = matches[0].index;
  return entries.filter(entry => entry.index === index).length === 1 ? index : null;
}

export type GuildRankRenameCommand = Readonly<{
  type: "editGuildMember"; changeType: 3; rankIndex: number; name: ""; rankName: string;
}>;
export type GuildRankPermissionCommand = Readonly<{
  type: "editGuildMember"; changeType: 5; rankIndex: number; name: "true" | "false"; rankName: string;
}>;
export type GuildRankCreateCommand = Readonly<{
  type: "editGuildMember"; changeType: 4; rankIndex: 0; name: ""; rankName: string;
}>;
const validGuildRankIndex = (value: unknown): value is number =>
  nonNegativeI32(value) && value <= 255;

/** Native valid_rank_rename_name counts UTF-16 units and preserves the exact draft. */
export function guildRankRenameCommand(rankIndex: number, name: string): GuildRankRenameCommand | null {
  if (!validGuildRankIndex(rankIndex) || typeof name !== "string"
    || name.length < 3 || name.length > 20 || name.includes("\\")) return null;
  return {type: "editGuildMember", changeType: 3, rankIndex, name: "", rankName: name};
}

/** One checkbox sends one type-5 option; neither rename nor a full options rewrite. */
export function guildRankPermissionCommand(
  rankIndex: number, key: GuildRankPermission, enabled: boolean,
): GuildRankPermissionCommand | null {
  if (!validGuildRankIndex(rankIndex) || typeof enabled !== "boolean") return null;
  const option = (GUILD_RANK_PERMISSION_KEYS as readonly unknown[]).indexOf(key);
  if (option < 0) return null;
  return {type: "editGuildMember", changeType: 5, rankIndex,
    name: enabled ? "true" : "false", rankName: String(option)};
}

/** Native confirm_create_rank's ordinary command; the server owns the actual new name/index. */
export function guildRankCreateCommand(rankCount: number): GuildRankCreateCommand | null {
  if (!Number.isInteger(rankCount) || rankCount < 1 || rankCount >= 255) return null;
  return {type: "editGuildMember", changeType: 4, rankIndex: 0, name: "", rankName: `Rank-${rankCount - 1}`};
}

export type ProjectedGuildRanks = {
  ranks: {index: number; name: string; permissions: GuildRankPermission[]}[];
  members: {name: string; id: number; rank: string; rankIndex: number; online: boolean}[];
};
/** GuildMemberChange.ranks is authoritative and already uses the protocol's camelCase JSON. */
export function projectGuildRanks(raw: unknown): ProjectedGuildRanks | null {
  if (!Array.isArray(raw) || raw.length > 256) return null;
  const result: ProjectedGuildRanks = {ranks: [], members: []};
  const rankIds = new Set<number>(), memberIds = new Set<number>(), memberNames = new Set<string>();
  for (const rank of raw) {
    if (!rank || typeof rank !== "object" || Array.isArray(rank) || !nonNegativeI32(rank.index)
      || rankIds.has(rank.index) || !validName(rank.name) || !Number.isInteger(rank.options)
      || rank.options < 0 || rank.options > 255 || !Array.isArray(rank.members)
      || result.members.length + rank.members.length > 8192) return null;
    rankIds.add(rank.index);
    result.ranks.push({index: rank.index, name: rank.name,
      permissions: GUILD_RANK_PERMISSION_KEYS.filter((_, bit) => (rank.options & (1 << bit)) !== 0)});
    for (const member of rank.members) {
      if (!member || typeof member !== "object" || Array.isArray(member) || !validName(member.name)
        || !nonNegativeI32(member.id) || memberIds.has(member.id) || memberNames.has(asciiFold(member.name))
        || typeof member.online !== "boolean") return null;
      memberIds.add(member.id); memberNames.add(asciiFold(member.name));
      result.members.push({name: member.name, id: member.id, rank: rank.name, rankIndex: rank.index, online: member.online});
    }
  }
  return result;
}

export type RankingQuery = Readonly<{rankType: number; rankIndex: number; onlineOnly: boolean}>;
export type RankingQueryProof = Readonly<{query: RankingQuery; owner: SocialReplyOwner}>;
const sameQuery = (a: RankingQuery, b: RankingQuery) => a.rankType === b.rankType
  && a.rankIndex === b.rankIndex && a.onlineOnly === b.onlineOnly;
const samePhysicalOwner = (a: SocialReplyOwner, b: SocialReplyOwner | null) => !!b
  && a.socket === b.socket && a.connectionGeneration === b.connectionGeneration
  && a.sessionGeneration === b.sessionGeneration && a.playerObjectId === b.playerObjectId;

/** Ranking responses have no offset/filter request ID. Keep exactly one wire request outstanding. */
export class RankingQueries {
  desired: RankingQuery = Object.freeze({rankType: 0, rankIndex: 0, onlineOnly: false});
  private active: {proof: RankingQueryProof; entered: boolean} | null = null;
  get pending(): RankingQueryProof | null { return this.active?.proof ?? null; }

  request(query: RankingQuery, owner: SocialReplyOwner | null): RankingQueryProof | null {
    if (!owner || !nonNegativeI32(query.rankType) || query.rankType > 5
      || !nonNegativeI32(query.rankIndex) || typeof query.onlineOnly !== "boolean") return null;
    this.desired = Object.freeze({...query});
    if (this.active) return null;
    const proof = Object.freeze({query: this.desired, owner: Object.freeze({...owner})});
    this.active = {proof, entered: false};
    return proof;
  }
  claim(proof: RankingQueryProof, owner: SocialReplyOwner | null, command: Record<string, unknown>): boolean {
    if (this.active?.proof !== proof || this.active.entered || !samePhysicalOwner(proof.owner, owner)
      || proof.owner.sceneRevision !== owner!.sceneRevision || proof.owner.mapFileName !== owner!.mapFileName
      || Object.keys(command).length !== 4 || command.type !== "getRanking"
      || command.rankType !== proof.query.rankType || command.rankIndex !== proof.query.rankIndex
      || command.onlineOnly !== proof.query.onlineOnly) return false;
    this.active.entered = true;
    return true;
  }
  cancelDefinitelyUnsent(proof: RankingQueryProof): void {
    if (this.active?.proof === proof && !this.active.entered) this.active = null;
  }
  receive(rankType: unknown, owner: SocialReplyOwner | null): RankingQuery | null {
    const active = this.active;
    if (!active?.entered || rankType !== active.proof.query.rankType || !samePhysicalOwner(active.proof.owner, owner)) return null;
    this.active = null;
    return active.proof.query;
  }
  wantsAnother(received: RankingQuery): boolean { return !sameQuery(this.desired, received); }
  retire(): void {
    this.active = null;
    this.desired = Object.freeze({rankType: 0, rankIndex: 0, onlineOnly: false});
  }
}
