import {
  GUILD_RANK_PERMISSION_KEYS,
  projectGuildRanks,
  type GuildRankPermission,
  type ProjectedGuildRanks,
} from "./social-parity-actions";

export type GuildRankViewer = Readonly<{ name: string; index: number }>;
export type GuildMemberChangeStatus = 0 | 1 | 2 | 3 | 4 | 5 | 6 | 7 | 8 | 255;
export type GuildOwnRankEvidence =
  | { kind: "unchanged" | "unknown" | "left" }
  | { kind: "confirmed"; index: number; name: string; permissions: GuildRankPermission[]; changed: boolean };
export type GuildRankProjectionResult =
  | { kind: "invalid" }
  | {
      kind: "applied";
      projection: ProjectedGuildRanks | null;
      evidence: {
        status: GuildMemberChangeStatus;
        /** Only status 255 establishes a complete roster; a delta never does. */
        fullRoster: boolean;
        needsFullRoster: boolean;
        guildLeft: boolean;
        ownRank: GuildOwnRankEvidence;
      };
    };

type Rank = ProjectedGuildRanks["ranks"][number];
type Member = ProjectedGuildRanks["members"][number];
type DataRecord = Record<string, unknown>;
const MAX_RANKS = 255;
const MAX_MEMBERS = 8192;
const invalid = (): GuildRankProjectionResult => ({ kind: "invalid" });
const byte = (value: unknown): value is number =>
  typeof value === "number" && Number.isInteger(value) && value >= 0 && value <= 255;
const index = (value: unknown): value is number =>
  typeof value === "number" && Number.isSafeInteger(value) && value >= 0 && value <= 0x7fff_ffff;
const name = (value: unknown): value is string =>
  typeof value === "string" && value.trim().length > 0 && value.length <= 256 && !value.includes("\0");
const record = (value: unknown): value is DataRecord => {
  if (!value || typeof value !== "object" || Array.isArray(value)) return false;
  const prototype: unknown = Object.getPrototypeOf(value);
  return prototype === Object.prototype || prototype === null;
};
/** Wire JSON has own data properties. Do not evaluate accessors supplied by a caller. */
function field(value: DataRecord, key: string): unknown {
  const descriptor = Object.getOwnPropertyDescriptor(value, key);
  if (descriptor && !("value" in descriptor)) throw new Error("Invalid guild projection property");
  return descriptor?.value;
}
function array(value: unknown, limit: number): unknown[] | null {
  if (!Array.isArray(value)) return null;
  const length = Object.getOwnPropertyDescriptor(value, "length");
  if (!length || !("value" in length) || !Number.isInteger(length.value)
    || length.value < 0 || length.value > limit) return null;
  const count: number = length.value;
  const copy: unknown[] = [];
  for (let cursor = 0; cursor < count; cursor++) {
    const descriptor = Object.getOwnPropertyDescriptor(value, String(cursor));
    if (!descriptor || !("value" in descriptor)) return null;
    copy.push(descriptor.value);
  }
  return copy;
}

function wireRanks(value: unknown): ProjectedGuildRanks | null {
  const raw = array(value, MAX_RANKS);
  if (!raw) return null;
  const ranks: { index: number; name: string; options: number; members: unknown[] }[] = [];
  let memberCount = 0;
  for (const entry of raw) {
    if (!record(entry)) return null;
    const rankIndex = field(entry, "index"), rankName = field(entry, "name"), options = field(entry, "options");
    const members = array(field(entry, "members"), MAX_MEMBERS);
    if (!byte(rankIndex) || !name(rankName) || !byte(options) || !members
      || (memberCount += members.length) > MAX_MEMBERS) return null;
    const copied: { id: number; name: string; online: boolean }[] = [];
    for (const member of members) {
      if (!record(member)) return null;
      const id = field(member, "id"), memberName = field(member, "name"), online = field(member, "online");
      if (!index(id) || !name(memberName) || typeof online !== "boolean") return null;
      copied.push({ id, name: memberName, online });
    }
    ranks.push({ index: rankIndex, name: rankName, options, members: copied });
  }
  return projectGuildRanks(ranks);
}

function copyProjection(value: ProjectedGuildRanks): ProjectedGuildRanks | null {
  if (!record(value)) return null;
  const ranks = array(field(value, "ranks"), MAX_RANKS), members = array(field(value, "members"), MAX_MEMBERS);
  if (!ranks || !members) return null;
  const raw: { index: number; name: string; options: number; members: unknown[] }[] = [];
  for (const rank of ranks) {
    if (!record(rank)) return null;
    const rankIndex = field(rank, "index"), rankName = field(rank, "name");
    const permissions = array(field(rank, "permissions"), GUILD_RANK_PERMISSION_KEYS.length);
    if (!byte(rankIndex) || !name(rankName) || !permissions || new Set(permissions).size !== permissions.length
      || permissions.some(permission => !(GUILD_RANK_PERMISSION_KEYS as readonly unknown[]).includes(permission))) return null;
    raw.push({ index: rankIndex, name: rankName, options: GUILD_RANK_PERMISSION_KEYS.reduce(
      (bits, permission, bit) => permissions.includes(permission) ? bits | (1 << bit) : bits, 0), members: [] });
  }
  for (const member of members) {
    if (!record(member)) return null;
    const rankIndex = field(member, "rankIndex"), rankName = field(member, "rank");
    const matches = raw.filter(rank => rank.index === rankIndex);
    if (matches.length !== 1 || rankName !== matches[0].name) return null;
    matches[0].members.push({ id: field(member, "id"), name: field(member, "name"), online: field(member, "online") });
  }
  return wireRanks(raw);
}

function ownMember(projection: ProjectedGuildRanks | null, viewer: GuildRankViewer | null): Member | null {
  if (!projection || !viewer) return null;
  return projection.members.find(member => member.id === viewer.index && member.name === viewer.name) ?? null;
}
function ownRank(projection: ProjectedGuildRanks | null, viewer: GuildRankViewer | null): Rank | null {
  const member = ownMember(projection, viewer);
  return member ? projection!.ranks.find(rank => rank.index === member.rankIndex) ?? null : null;
}
function confirmed(previous: ProjectedGuildRanks | null, projection: ProjectedGuildRanks, viewer: GuildRankViewer): GuildOwnRankEvidence {
  const next = ownRank(projection, viewer);
  if (!next) return { kind: "unknown" };
  const old = ownRank(previous, viewer);
  return { kind: "confirmed", index: next.index, name: next.name, permissions: [...next.permissions],
    changed: !old || old.index !== next.index || old.name !== next.name
      || old.permissions.length !== next.permissions.length || old.permissions.some(permission => !next.permissions.includes(permission)) };
}

/** Replace one authoritative rank snapshot, keeping every other rank definition. */
function replaceRank(previous: ProjectedGuildRanks, delta: ProjectedGuildRanks): ProjectedGuildRanks | null {
  const target = delta.ranks[0];
  const oldById = new Map(previous.members.map(member => [member.id, member]));
  const oldByName = new Map(previous.members.map(member => [member.name, member]));
  for (const member of delta.members) {
    // An exact identity may have moved since the last roster. A conflicting id/name is ambiguous.
    const byId = oldById.get(member.id), byName = oldByName.get(member.name);
    if ((byId && byId.name !== member.name) || (byName && byName.id !== member.id)) return null;
  }
  const ranks = previous.ranks.filter(rank => rank.index !== target.index);
  const position = previous.ranks.findIndex(rank => rank.index === target.index);
  ranks.splice(position < 0 ? ranks.length : position, 0, target);
  const movedIds = new Set(delta.members.map(member => member.id));
  const members = previous.members.filter(member => member.rankIndex !== target.index && !movedIds.has(member.id));
  members.push(...delta.members);
  return copyProjection({ ranks, members });
}

/**
 * Crystal GameScene.GuildMemberChange statuses, also emitted by the shared Gateway.
 * This read model grants no command authority: the host must first verify owner + guild scope.
 * `previous` may be partial. Only evidence.fullRoster (255) establishes completeness; retain
 * that fact separately rather than treating any non-null projection as a full roster.
 * `unknown` ownRank requires invalidating cached own-rank authority; `unchanged` preserves it.
 */
export function applyGuildMemberChange(
  previous: ProjectedGuildRanks | null,
  payload: unknown,
  viewer: GuildRankViewer | null,
): GuildRankProjectionResult {
  try {
    if (!record(payload) || (viewer !== null && !record(viewer))) return invalid();
    // Snapshot viewer data once; returned data never aliases the caller's objects.
    const viewerName = viewer === null ? null : field(viewer, "name");
    const viewerIndex = viewer === null ? null : field(viewer, "index");
    if (viewer !== null && (!name(viewerName) || !index(viewerIndex))) return invalid();
    const self = viewer === null ? null : { name: viewerName as string, index: viewerIndex as number };
    const status = field(payload, "status"), subject = field(payload, "name"), rankIndex = field(payload, "rankIndex");
    if (!byte(status) || (status > 8 && status !== 255) || typeof subject !== "string"
      || (subject !== "" && !name(subject)) || !byte(rankIndex)) return invalid();
    const delta = wireRanks(field(payload, "ranks"));
    if (!delta || (status <= 5 && delta.ranks.length !== 0)
      || (status >= 6 && status <= 8 && delta.ranks.length !== 1)
      || (status <= 8 && !name(subject))) return invalid();
    const before = previous === null ? null : copyProjection(previous);
    if (previous !== null && !before && status !== 255) return invalid();
    let projection = before ? copyProjection(before) : null;
    let needsFullRoster = before === null;
    let guildLeft = false;
    let evidence: GuildOwnRankEvidence = { kind: self ? "unchanged" : "unknown" };

    if (status === 255) {
      projection = delta;
      needsFullRoster = false;
      evidence = self ? confirmed(before, projection, self) : { kind: "unknown" };
    } else if (status === 0 || status === 1) {
      const member = projection?.members.find(member => member.name === subject);
      if (member) member.online = status === 1;
      else needsFullRoster = true;
    } else if (status === 2) {
      // Name alone has no character id/rank. Crystal sets MembersChanged and fetches a roster.
      needsFullRoster = true;
    } else if (status === 3 || status === 4) {
      guildLeft = self !== null && subject === self.name;
      if (guildLeft) {
        projection = { ranks: [], members: [] };
        evidence = { kind: "left" };
        needsFullRoster = false;
      } else if (projection) {
        if (!projection.members.some(member => member.name === subject)) needsFullRoster = true;
        projection.members = projection.members.filter(member => member.name !== subject);
      }
    } else if (status === 5) {
      const member = projection?.members.find(member => member.name === subject);
      const rank = projection?.ranks.find(rank => rank.index === rankIndex);
      if (member && rank) {
        member.rankIndex = rank.index;
        member.rank = rank.name;
      } else needsFullRoster = true;
      // The current Gateway delivers own-rank authority with status 8, not this peer delta.
      if (self && subject === self.name) evidence = { kind: "unknown" };
    } else if (status === 6) {
      const added = delta.ranks[0];
      if (delta.members.length !== 0 || added.permissions.length !== 0) return invalid();
      const sorted = projection?.ranks.slice().sort((left, right) => left.index - right.index);
      const count = sorted?.length ?? 0;
      const insertion = count > 1 ? count - 1 : 1;
      if (!projection || !sorted || count === 0 || sorted.some((rank, cursor) => rank.index !== cursor)) {
        needsFullRoster = true;
        evidence = { kind: "unknown" };
      } else {
        if (count >= 255 || added.index !== insertion) return invalid();
        for (const rank of projection.ranks) if (rank.index >= insertion) rank.index++;
        for (const member of projection.members) if (member.rankIndex >= insertion) member.rankIndex++;
        projection.ranks.sort((left, right) => left.index - right.index);
        projection.ranks.splice(insertion, 0, added);
        if (self && ownMember(before, self)) evidence = confirmed(before, projection, self);
      }
    } else if (status === 7) {
      const target = delta.ranks[0].index;
      if (!projection || !projection.ranks.some(rank => rank.index === target)) {
        needsFullRoster = true;
        evidence = { kind: "unknown" };
      } else {
        projection = replaceRank(projection, delta);
        if (!projection) return invalid();
        if (self && (ownMember(before, self)?.rankIndex === target || ownMember(delta, self))) {
          evidence = confirmed(before, projection, self);
        }
      }
    } else if (status === 8) {
      // `name` is the actor, not necessarily the viewer. Match both stable id and opaque name.
      if (self && !ownMember(delta, self)) return invalid();
      const hadTarget = !!projection?.ranks.some(rank => rank.index === delta.ranks[0].index);
      projection = projection ? replaceRank(projection, delta) : delta;
      if (!projection) return invalid();
      needsFullRoster ||= !hadTarget;
      evidence = self ? confirmed(before, projection, self) : { kind: "unknown" };
    }
    if (projection) {
      projection = copyProjection(projection);
      if (!projection) return invalid();
    }
    return { kind: "applied", projection, evidence: { status: status as GuildMemberChangeStatus,
      fullRoster: status === 255, needsFullRoster, guildLeft, ownRank: evidence } };
  } catch {
    return invalid();
  }
}
