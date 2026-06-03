/**
 * Typed adapters that map the loosely-typed stage-5 page state into the strict
 * prop shapes consumed by the standalone Crystal UI windows.
 *
 * The page (`app/page.tsx`) tracks several systems as `Record<string, unknown>`
 * (or arrays of them) because they arrive over the wire and are merged
 * incrementally. These adapters defensively read those records — validating
 * each field and falling back gracefully — so the windows can stay strict and
 * presentation-only.
 *
 * Field names follow the gateway's camelCase JSON serialization of the protocol
 * structs (see `packages/protocol/src/types.rs`) and the incremental patches
 * applied by the page packet handlers.
 */

import type { CreatureSummary, HeroSummary } from "../app/components/original-client-hero-pet-window";
import type { GroupMember, GroupSummary } from "../app/components/original-client-group-window";
import type { FriendEntry, FriendsSummary } from "../app/components/original-client-friends-window";
import type {
  MentorSummary,
  RelationshipSummary,
} from "../app/components/original-client-bonds-window";
import type {
  RankingEntry,
  RankingPage,
  RankingTabKey,
} from "../app/components/original-client-ranking-window";
import type { MarketListing } from "../app/components/original-client-market-window";
import type {
  ConquestSummary,
  GuildTerritorySummary,
} from "../app/components/original-client-conquest-window";
import type { TradeSummary } from "../app/components/original-client-trade-window";
import type { BuffEntry } from "../app/components/original-client-buff-window";
import type { WorldMapMarker } from "../app/components/original-client-world-map-window";

type UnknownRecord = Record<string, unknown>;

type EntityClassKey = NonNullable<HeroSummary["classKey"]>;

/** The subset of the page's stage-5 state these adapters read from. */
export type Stage5SystemsLike = {
  group?: { members?: string[]; lootMode?: string } | null;
  social?: { friends?: string[]; blocked?: string[] } | null;
  relationship?: UnknownRecord | null;
  mentor?: UnknownRecord | null;
  auction?: Array<UnknownRecord> | null;
  conquest?: UnknownRecord | null;
  guildTerritory?: UnknownRecord | null;
  hero?: UnknownRecord | null;
  intelligentCreatures?: Array<UnknownRecord> | null;
  trade?: UnknownRecord | null;
};

// ---------------------------------------------------------------------------
// Primitive readers (defensive)
// ---------------------------------------------------------------------------

function asRecord(value: unknown): UnknownRecord | null {
  return value && typeof value === "object" && !Array.isArray(value) ? (value as UnknownRecord) : null;
}

function readString(record: UnknownRecord | null, keys: string[]): string | undefined {
  if (!record) return undefined;
  for (const key of keys) {
    const value = record[key];
    if (typeof value === "string") {
      const trimmed = value.trim();
      if (trimmed.length > 0) return trimmed;
    } else if (typeof value === "number" && Number.isFinite(value)) {
      return String(value);
    }
  }
  return undefined;
}

function readNumber(record: UnknownRecord | null, keys: string[]): number | undefined {
  if (!record) return undefined;
  for (const key of keys) {
    const value = record[key];
    if (typeof value === "number" && Number.isFinite(value)) {
      return value;
    }
    if (typeof value === "string") {
      const parsed = Number(value);
      if (Number.isFinite(parsed)) return parsed;
    }
  }
  return undefined;
}

function readBool(record: UnknownRecord | null, keys: string[]): boolean | undefined {
  if (!record) return undefined;
  for (const key of keys) {
    const value = record[key];
    if (typeof value === "boolean") return value;
    if (typeof value === "number") return value !== 0;
    if (typeof value === "string") {
      const normalized = value.trim().toLowerCase();
      if (normalized === "true") return true;
      if (normalized === "false") return false;
    }
  }
  return undefined;
}

function readStringArray(value: unknown): string[] {
  if (!Array.isArray(value)) return [];
  return value.filter((entry): entry is string => typeof entry === "string");
}

function readNumberArray(value: unknown): number[] {
  if (!Array.isArray(value)) return [];
  return value.filter((entry): entry is number => typeof entry === "number" && Number.isFinite(entry));
}

/** Maps the protocol `MirClass` (string variant or numeric) to a UI class key. */
export function classKeyFromUnknown(value: unknown): EntityClassKey | undefined {
  if (typeof value === "string") {
    const normalized = value.toLowerCase();
    if (normalized.includes("wizard")) return "wizard";
    if (normalized.includes("tao")) return "taoist";
    if (normalized.includes("assassin")) return "assassin";
    if (normalized.includes("archer")) return "archer";
    if (normalized.includes("warrior")) return "warrior";
    return undefined;
  }
  if (typeof value === "number") {
    switch (value) {
      case 0:
        return "warrior";
      case 1:
        return "wizard";
      case 2:
        return "taoist";
      case 3:
        return "assassin";
      case 4:
        return "archer";
      default:
        return undefined;
    }
  }
  return undefined;
}

// ---------------------------------------------------------------------------
// Hero & creatures (feeds the existing Hero/Pet window)
// ---------------------------------------------------------------------------

/**
 * Adapts the stage-5 `hero` slice into the Hero/Pet window's `HeroSummary`.
 *
 * The slice merges two sources: the `ManageHeroes` packet (`currentHero` =
 * {@link ClientHeroInformation}, `maximumCount`, `heroes` count) and the
 * incremental HUD mirrors from `HeroHealthChanged` / `HeroLevelChanged`
 * (`hp`, `mp`, `experience`, `maxExperience`, `level`). Returns `null` when no
 * hero is present so the window shows its empty state.
 */
export function adaptHero(hero: UnknownRecord | null | undefined): HeroSummary | null {
  const record = asRecord(hero);
  if (!record) return null;

  const current = asRecord(record.currentHero);
  const name = readString(current, ["name"]) ?? readString(record, ["name"]);
  // Without a name and without any live stats there is nothing meaningful to show.
  const hp = readNumber(record, ["hp"]);
  const maxHp = readNumber(record, ["maxHp", "maxHP"]);
  const level = readNumber(current, ["level"]) ?? readNumber(record, ["level"]);

  if (!name && hp === undefined && level === undefined) {
    return null;
  }

  const spawnState = readNumber(record, ["spawnState"]);
  const active = readBool(record, ["active", "summoned"]) ?? (spawnState !== undefined ? spawnState > 0 : undefined);

  return {
    name: name ?? "Hero",
    classKey: classKeyFromUnknown(current?.class ?? record.class),
    level: level ?? 1,
    hp: hp ?? 0,
    maxHp: maxHp ?? Math.max(hp ?? 0, 1),
    mp: readNumber(record, ["mp"]),
    maxMp: readNumber(record, ["maxMp", "maxMP"]),
    experience: readNumber(record, ["experience", "exp"]),
    maxExperience: readNumber(record, ["maxExperience", "maxExp"]),
    loyalty: readNumber(record, ["loyalty"]),
    maxLoyalty: readNumber(record, ["maxLoyalty"]),
    attack: readNumber(record, ["attack", "ac"]),
    defence: readNumber(record, ["defence", "defense", "dc"]),
    active,
  };
}

/** Maps the numeric `petMode` to a short pickup label. */
function pickupModeLabel(mode: number | undefined): string | undefined {
  switch (mode) {
    case 0:
      return "Both";
    case 1:
      return "Group";
    case 2:
      return "Guild";
    case 3:
      return "None";
    case 4:
      return "Attack";
    case 5:
      return "Move";
    default:
      return mode === undefined ? undefined : `Mode ${mode}`;
  }
}

/**
 * Adapts the stage-5 `intelligentCreatures` list into the Hero/Pet window's
 * `CreatureSummary[]`. Each entry mirrors {@link ClientIntelligentCreature}
 * (camelCase): `petType`, `icon`, `customName`, `fullness`, `slotIndex`,
 * `petMode`, … Lifespan is derived from `fullness` (0-1000 in the original
 * client) when present.
 */
export function adaptCreatures(creatures: Array<UnknownRecord> | null | undefined): CreatureSummary[] {
  if (!Array.isArray(creatures)) return [];
  return creatures.flatMap((entry, index) => {
    const record = asRecord(entry);
    if (!record) return [];

    const slotIndex = readNumber(record, ["slotIndex"]);
    const id = slotIndex !== undefined ? `creature-${slotIndex}` : `creature-${index}`;
    const name =
      readString(record, ["customName", "name", "petName"]) ??
      `Pet ${slotIndex ?? index + 1}`;
    const icon = readNumber(record, ["icon"]);
    const fullness = readNumber(record, ["fullness"]);
    const petMode = readNumber(record, ["petMode"]);

    const summary: CreatureSummary = {
      id,
      name,
      icon: typeof icon === "number" && icon > 0 ? icon : undefined,
      level: readNumber(record, ["petLevel", "level"]),
      hp: readNumber(record, ["hp"]),
      maxHp: readNumber(record, ["maxHp", "maxHP"]),
      pickupMode: pickupModeLabel(petMode),
      summoned: readBool(record, ["summoned", "summonMe", "visible"]) ?? undefined,
    };

    // The original client tracks creature condition via "fullness" (0-1000).
    if (typeof fullness === "number") {
      summary.lifespan = Math.max(0, fullness);
      summary.maxLifespan = 1000;
    }

    return [summary];
  });
}

// ---------------------------------------------------------------------------
// Group / party
// ---------------------------------------------------------------------------

/**
 * Adapts the stage-5 `group` slice into the Group window's `GroupSummary`.
 * Optionally enriches member rows with online/level/class/HP data looked up
 * from the world entity list by name.
 */
export function adaptGroup(
  group: { members?: string[]; lootMode?: string } | null | undefined,
  options?: { enrich?: (name: string) => Partial<GroupMember> | undefined },
): GroupSummary | null {
  if (!group) return null;
  const names = readStringArray(group.members);
  const members: GroupMember[] = names.map((name, index) => {
    const base: GroupMember = { name, leader: index === 0 };
    const extra = options?.enrich?.(name);
    return extra ? { ...base, ...extra, name } : base;
  });
  return {
    members,
    lootMode: typeof group.lootMode === "string" ? group.lootMode : undefined,
  };
}

// ---------------------------------------------------------------------------
// Friends / blocked
// ---------------------------------------------------------------------------

/**
 * Adapts the stage-5 `social` slice into the Friends window's `FriendsSummary`.
 * The page stores friends/blocked as `string[]`; an optional enricher can add
 * online/memo metadata (e.g. from a richer friend cache).
 */
export function adaptFriends(
  social: { friends?: string[]; blocked?: string[] } | null | undefined,
  options?: { enrich?: (name: string) => Partial<FriendEntry> | undefined },
): FriendsSummary | null {
  if (!social) return null;
  const toEntries = (names: string[]): FriendEntry[] =>
    names.map((name) => {
      const extra = options?.enrich?.(name);
      return extra ? { ...extra, name } : { name };
    });
  return {
    friends: toEntries(readStringArray(social.friends)),
    blocked: toEntries(readStringArray(social.blocked)),
  };
}

// ---------------------------------------------------------------------------
// Bonds (relationship + mentor)
// ---------------------------------------------------------------------------

/** Adapts the stage-5 `relationship` slice into a `RelationshipSummary`. */
export function adaptRelationship(
  relationship: UnknownRecord | null | undefined,
): RelationshipSummary | null {
  const record = asRecord(relationship);
  if (!record) return null;
  return {
    partnerName: readString(record, ["partnerName", "name"]),
    partnerMap: readString(record, ["mapName", "partnerMap"]),
    marriedDays: readNumber(record, ["marriedDays"]),
    allowMarriage: readBool(record, ["allowMarriage"]),
    pendingRequestFrom: readString(record, ["pendingRequestFrom", "pendingDivorceFrom"]),
  };
}

/** Adapts the stage-5 `mentor` slice into a `MentorSummary`. */
export function adaptMentor(mentor: UnknownRecord | null | undefined): MentorSummary | null {
  const record = asRecord(mentor);
  if (!record) return null;
  return {
    name: readString(record, ["name"]),
    level: readNumber(record, ["level"]),
    online: readBool(record, ["online"]),
    menteeExp: readNumber(record, ["menteeExp"]),
    allowMentor: readBool(record, ["allowMentor"]),
    pendingRequestFrom: readString(record, ["pendingRequestFrom"]),
  };
}

// ---------------------------------------------------------------------------
// Ranking
// ---------------------------------------------------------------------------

const RANKING_TAB_BY_RANK_TYPE: Record<number, RankingTabKey> = {
  0: "overall",
  1: "warrior",
  2: "wizard",
  3: "taoist",
  4: "assassin",
  5: "archer",
};

/** Maps a `(rankType, onlineOnly)` pair to the Ranking window tab key. */
export function rankingTabKey(rankType: number, onlineOnly: boolean): RankingTabKey {
  if (onlineOnly) return "online";
  return RANKING_TAB_BY_RANK_TYPE[rankType] ?? "overall";
}

/**
 * The page's `rankings` value is keyed `"<rankType>:<all|online>"`. This is the
 * key for a given tab so callers can look up the active page.
 */
export function rankingPageKeyForTab(tab: RankingTabKey): string {
  switch (tab) {
    case "warrior":
      return "1:all";
    case "wizard":
      return "2:all";
    case "taoist":
      return "3:all";
    case "assassin":
      return "4:all";
    case "archer":
      return "5:all";
    case "online":
      return "0:online";
    case "overall":
    default:
      return "0:all";
  }
}

type RankingStateLike = {
  rankType?: number;
  onlineOnly?: boolean;
  myRank?: number;
  count?: number;
  entries?: Array<{
    rank?: number;
    playerId?: number;
    name?: string;
    level?: number;
    classKey?: unknown;
  }>;
};

/** Adapts a page `RankingState` into the Ranking window's `RankingPage`. */
export function adaptRankingPage(page: RankingStateLike | null | undefined): RankingPage | null {
  if (!page || typeof page !== "object") return null;
  const entries: RankingEntry[] = (Array.isArray(page.entries) ? page.entries : []).flatMap(
    (entry, index) => {
      if (!entry || typeof entry !== "object") return [];
      const name = typeof entry.name === "string" ? entry.name : undefined;
      if (!name) return [];
      return [
        {
          rank: typeof entry.rank === "number" ? entry.rank : index + 1,
          playerId: typeof entry.playerId === "number" ? entry.playerId : 0,
          name,
          level: typeof entry.level === "number" ? entry.level : 0,
          classKey: classKeyFromUnknown(entry.classKey) ?? "warrior",
        },
      ];
    },
  );
  return {
    rankType: typeof page.rankType === "number" ? page.rankType : 0,
    onlineOnly: page.onlineOnly === true,
    myRank: typeof page.myRank === "number" ? page.myRank : 0,
    count: typeof page.count === "number" ? page.count : entries.length,
    entries,
  };
}

/** Convenience: pick the active ranking page from the page's `rankings` map. */
export function adaptActiveRankingPage(
  rankings: Record<string, RankingStateLike> | null | undefined,
  currentKey: string | null | undefined,
): { tab: RankingTabKey; page: RankingPage | null } {
  if (!rankings || !currentKey || !rankings[currentKey]) {
    return { tab: "overall", page: null };
  }
  const state = rankings[currentKey];
  const adapted = adaptRankingPage(state);
  return {
    tab: rankingTabKey(adapted?.rankType ?? 0, adapted?.onlineOnly ?? false),
    page: adapted,
  };
}

// ---------------------------------------------------------------------------
// Market / auction
// ---------------------------------------------------------------------------

/**
 * Adapts the stage-5 `auction` array into the Market window's `MarketListing[]`.
 * Field names mirror the social-system reader: `item`/`itemName`/`name`,
 * `seller`/`owner`, `price`/`gold`, `id`/`listingId`, plus optional
 * `icon`/`count`/`state`. When `viewerName` is given, listings whose seller
 * matches are flagged `mine` so the window enables their Cancel action.
 */
export function adaptMarketListings(
  auction: Array<UnknownRecord> | null | undefined,
  options?: { viewerName?: string | null },
): MarketListing[] {
  if (!Array.isArray(auction)) return [];
  const viewer = options?.viewerName?.trim() ?? "";
  return auction.flatMap((entry, index) => {
    const record = asRecord(entry);
    if (!record) return [];
    const id = readString(record, ["id", "listingId", "auctionId", "uniqueId"]) ?? `listing-${index}`;
    const itemName = readString(record, ["item", "itemName", "name"]) ?? `Listing ${index + 1}`;
    const seller = readString(record, ["seller", "owner", "sellerName"]) ?? "Market";
    const icon = readNumber(record, ["icon", "image"]);
    const mine =
      readBool(record, ["mine", "isOwner"]) ?? (viewer.length > 0 && seller === viewer);
    return [
      {
        id,
        itemName,
        seller,
        price: readNumber(record, ["price", "gold"]) ?? 0,
        icon: typeof icon === "number" && icon > 0 ? icon : undefined,
        count: readNumber(record, ["count", "quantity"]),
        state: readString(record, ["state", "status"]),
        mine,
      },
    ];
  });
}

// ---------------------------------------------------------------------------
// Conquest / guild territory
// ---------------------------------------------------------------------------

/** Adapts the stage-5 `conquest` slice into the Conquest window's summary. */
export function adaptConquest(conquest: UnknownRecord | null | undefined): ConquestSummary | null {
  const record = asRecord(conquest);
  if (!record) return null;
  return {
    castleOwner: readString(record, ["castleOwner", "owner"]),
    activeWars: readStringArray(record.activeWars),
    eventLog: readStringArray(record.eventLog),
    taxRatePercent: readNumber(record, ["taxRatePercent", "taxRate"]),
    gold: readNumber(record, ["gold"]),
    guards: readNumberArray(record.guards),
    walls: readNumberArray(record.walls),
    gates: readNumberArray(record.gates),
    openGates: readNumberArray(record.openGates),
  };
}

/** Adapts the stage-5 `guildTerritory` slice into the Conquest window's summary. */
export function adaptGuildTerritory(
  territory: UnknownRecord | null | undefined,
): GuildTerritorySummary | null {
  const record = asRecord(territory);
  if (!record) return null;
  return {
    owned: readBool(record, ["owned"]),
    mapFileName: readString(record, ["mapFileName"]),
    rentalDaysLeft: readNumber(record, ["rentalDaysLeft"]),
    recallLog: readStringArray(record.recallLog),
  };
}

// ---------------------------------------------------------------------------
// Trade
// ---------------------------------------------------------------------------

/**
 * Adapts the stage-5 `trade` slice into the Trade window's `TradeSummary`.
 *
 * The page builds this incrementally from the trade packets: `partner` +
 * `state` ("requested" | "open") from `TradeRequest`/`TradeAccept`,
 * `partnerGold` from `TradeGold`, `partnerItemCount` from `TradeItem`, and
 * `confirmed` from `TradeConfirm`. Returns `null` when no trade is in flight so
 * the host can keep the window closed.
 */
export function adaptTrade(trade: UnknownRecord | null | undefined): TradeSummary | null {
  const record = asRecord(trade);
  if (!record) return null;
  const partner = readString(record, ["partner", "partnerName", "name"]);
  const state = readString(record, ["state", "status"]);
  // A trade with neither a partner nor a known state has nothing to show.
  if (!partner && !state) return null;
  return {
    partner,
    state,
    partnerGold: readNumber(record, ["partnerGold", "gold"]),
    partnerItemCount: readNumber(record, ["partnerItemCount", "itemCount"]),
    confirmed: readBool(record, ["confirmed", "partnerConfirmed"]),
  };
}

// ---------------------------------------------------------------------------
// Buffs
// ---------------------------------------------------------------------------

type ActiveBuffLike = {
  key?: unknown;
  name?: unknown;
  description?: unknown;
  remainingTicks?: unknown;
  attackBonus?: unknown;
  defenceBonus?: unknown;
};

/**
 * Adapts the world's `activeBuffs` list (mirrors the gateway `ActiveBuff`
 * struct) into the Buff window's `BuffEntry[]`. Entries without a usable name
 * are dropped; a missing `key` falls back to a positional id.
 */
export function adaptBuffs(buffs: Array<ActiveBuffLike> | null | undefined): BuffEntry[] {
  if (!Array.isArray(buffs)) return [];
  return buffs.flatMap((entry, index) => {
    if (!entry || typeof entry !== "object") return [];
    const name = typeof entry.name === "string" ? entry.name.trim() : "";
    if (!name) return [];
    const key = typeof entry.key === "string" && entry.key.length > 0 ? entry.key : `buff-${index}`;
    return [
      {
        key,
        name,
        description: typeof entry.description === "string" ? entry.description : undefined,
        remainingTicks:
          typeof entry.remainingTicks === "number" && Number.isFinite(entry.remainingTicks)
            ? entry.remainingTicks
            : undefined,
        attackBonus:
          typeof entry.attackBonus === "number" && Number.isFinite(entry.attackBonus)
            ? entry.attackBonus
            : undefined,
        defenceBonus:
          typeof entry.defenceBonus === "number" && Number.isFinite(entry.defenceBonus)
            ? entry.defenceBonus
            : undefined,
      },
    ];
  });
}

// ---------------------------------------------------------------------------
// World map markers
// ---------------------------------------------------------------------------

type MapTransferLike = {
  key?: unknown;
  toMapTitle?: unknown;
  toMapFileName?: unknown;
  toPosition?: { x?: unknown; y?: unknown } | null;
  bounds?: { minX?: unknown; maxX?: unknown; minY?: unknown; maxY?: unknown } | null;
};

/**
 * Adapts the world's `mapTransfers` list into World map markers. Each transfer
 * exposes a destination map plus the in-map exit bounds; the marker is plotted
 * at the centre of those bounds, normalised against the supplied map size so it
 * lands on the world image. Duplicate destinations are collapsed by title.
 */
export function adaptWorldMapMarkers(
  transfers: Array<MapTransferLike> | null | undefined,
  options?: { mapWidth?: number; mapHeight?: number; canTeleport?: (mapFileName: string) => boolean },
): WorldMapMarker[] {
  if (!Array.isArray(transfers)) return [];
  const width = options?.mapWidth && options.mapWidth > 0 ? options.mapWidth : 1024;
  const height = options?.mapHeight && options.mapHeight > 0 ? options.mapHeight : 1024;
  const seen = new Set<string>();
  const markers: WorldMapMarker[] = [];

  transfers.forEach((entry, index) => {
    if (!entry || typeof entry !== "object") return;
    const title = typeof entry.toMapTitle === "string" ? entry.toMapTitle.trim() : "";
    const mapFile = typeof entry.toMapFileName === "string" ? entry.toMapFileName : "";
    const label = title || mapFile;
    if (!label || seen.has(label)) return;
    seen.add(label);

    const bounds = entry.bounds && typeof entry.bounds === "object" ? entry.bounds : null;
    const cx = midpoint(bounds?.minX, bounds?.maxX, entry.toPosition?.x);
    const cy = midpoint(bounds?.minY, bounds?.maxY, entry.toPosition?.y);

    markers.push({
      key: typeof entry.key === "string" && entry.key.length > 0 ? entry.key : `transfer-${index}`,
      title: label,
      x: cx === undefined ? 0.5 : Math.max(0, Math.min(1, cx / width)),
      y: cy === undefined ? 0.5 : Math.max(0, Math.min(1, cy / height)),
      teleport: mapFile ? options?.canTeleport?.(mapFile) ?? false : false,
    });
  });

  return markers;
}

function midpoint(min: unknown, max: unknown, fallback: unknown): number | undefined {
  const lo = typeof min === "number" && Number.isFinite(min) ? min : undefined;
  const hi = typeof max === "number" && Number.isFinite(max) ? max : undefined;
  if (lo !== undefined && hi !== undefined) return (lo + hi) / 2;
  if (lo !== undefined) return lo;
  if (hi !== undefined) return hi;
  if (typeof fallback === "number" && Number.isFinite(fallback)) return fallback;
  return undefined;
}
