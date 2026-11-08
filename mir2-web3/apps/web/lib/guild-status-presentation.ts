import type { SocialReplyOwner } from "./social-incoming-replies";

/** Display data only. This projection never establishes membership or command authority. */
export type GuildStatusPresentation = Readonly<{
  level?: number;
  memberCount?: number;
  maxMembers?: number;
  experience?: number;
  maxExperience?: number;
  gold?: number;
  sparePoints?: number;
}>;
export type GuildStatusPresentationSource = Readonly<{
  owner: SocialReplyOwner; guildName: string; value: GuildStatusPresentation;
}>;

/** Always overwrite all seven fields: a missing or invalid value must not reuse an older guild's data. */
export function guildStatusPresentation(payload: unknown): GuildStatusPresentation {
  const unknown: GuildStatusPresentation = {level: undefined, memberCount: undefined, maxMembers: undefined,
    experience: undefined, maxExperience: undefined, gold: undefined, sparePoints: undefined};
  try {
    if (!payload || typeof payload !== "object" || Array.isArray(payload)
      || ![Object.prototype, null].includes(Object.getPrototypeOf(payload))) return Object.freeze(unknown);
    const field = (key: string): unknown => {
      const descriptor = Object.getOwnPropertyDescriptor(payload, key);
      return descriptor && "value" in descriptor ? descriptor.value : undefined;
    };
    const guildName = field("guildName");
    if (typeof guildName !== "string" || !guildName.trim() || guildName.length > 256 || guildName.includes("\0")) {
      return Object.freeze(unknown);
    }
    const integer = (key: string, max: number): number | undefined => {
      const value = field(key);
      return typeof value === "number" && Number.isSafeInteger(value) && value >= 0 && value <= max ? value : undefined;
    };
    return Object.freeze({level: integer("level", 255), memberCount: integer("memberCount", 0x7fff_ffff),
      maxMembers: integer("maxMembers", 0x7fff_ffff), experience: integer("experience", Number.MAX_SAFE_INTEGER),
      maxExperience: integer("maxExperience", Number.MAX_SAFE_INTEGER), gold: integer("gold", 0xffff_ffff),
      sparePoints: integer("sparePoints", 255)});
  } catch { return Object.freeze(unknown); }
}

/** Ordinary snapshots omit these packet fields. Retain them only for the same physical player and guild. */
export function readGuildStatusPresentation(
  source: GuildStatusPresentationSource | null, owner: SocialReplyOwner | null, guildName: unknown,
): GuildStatusPresentation {
  if (source && owner && source.guildName === guildName && source.owner.socket === owner.socket
    && source.owner.connectionGeneration === owner.connectionGeneration
    && source.owner.sessionGeneration === owner.sessionGeneration
    && source.owner.playerObjectId === owner.playerObjectId) return source.value;
  return guildStatusPresentation(null);
}
