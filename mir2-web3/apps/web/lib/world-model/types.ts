/**
 * Standalone, framework-agnostic world-model types.
 *
 * These are structurally identical to the types defined inline in
 * `app/page.tsx` so a future integration is a drop-in. They are gathered here
 * so the store, emitter, and any non-React consumers can import them without
 * pulling in React or Next.js.
 *
 * Sub-types re-exported from `lib/scene-types.ts` are re-exported here so
 * consumers only need one import path.
 */

// ---------------------------------------------------------------------------
// Re-exports from scene-types (subset used by WorldState)
// ---------------------------------------------------------------------------

export type {
  SceneView,
  TerrainPatch,
  DecorObject,
  OriginalMapRegion,
} from "../scene-types";

// ---------------------------------------------------------------------------
// Primitive enums
// ---------------------------------------------------------------------------

export type EntityKind = "selfPlayer" | "player" | "monster" | "npc";
export type EntityDisposition = "friendly" | "neutral" | "hostile";
export type ItemContainer = "bag1" | "bag2" | "quest" | "belt" | "storage";
export type EquipmentSlot =
  | "weapon"
  | "armour"
  | "helmet"
  | "mount"
  | "necklace"
  | "torch"
  | "braceletLeft"
  | "braceletRight"
  | "ringLeft"
  | "ringRight"
  | "amulet"
  | "boots"
  | "belt"
  | "stone";
export type QuestStage = "available" | "inProgress" | "readyToTurnIn" | "completed";

// ---------------------------------------------------------------------------
// Entity sprite
// ---------------------------------------------------------------------------

export type WorldEntitySprite = {
  bodyLibrary: string;
  hairLibrary?: string | null;
  weaponLibrary?: string | null;
  weaponLibrarySecondary?: string | null;
  frameBaseOffset: number;
  weaponFrameOffset?: number | null;
  altBodyLibrary?: string | null;
  altHairLibrary?: string | null;
  altWeaponLibrary?: string | null;
  altWeaponLibrarySecondary?: string | null;
  altFrameBaseOffset?: number | null;
  altWeaponFrameOffset?: number | null;
  frameCount: number;
  directionStride: number;
  mountLibrary?: string | null;
  mountFrameOffset?: number | null;
};

// ---------------------------------------------------------------------------
// World entity
// ---------------------------------------------------------------------------

export type WorldEntity = {
  objectId: string;
  kind: EntityKind;
  name: string;
  ownerName?: string;
  x: number;
  y: number;
  direction?: string;
  classKey?: "warrior" | "wizard" | "taoist" | "assassin" | "archer";
  genderKey?: "male" | "female";
  level?: number;
  hp?: number;
  maxHp?: number;
  light?: number;
  nameColourArgb?: number;
  dead?: boolean;
  sneaking?: boolean;
  disposition?: EntityDisposition;
  sprite?: WorldEntitySprite | null;
  questIds?: number[];
  bigMapIcon?: number;
  showOnBigMap?: boolean;
  canTeleportTo?: boolean;
  movementAnimation?: "walking" | "running";
  movementStartedAt?: number;
  movementUntil?: number;
  movementFrameCount?: number;
  attackAnimation?: "melee1" | "melee2" | "melee3" | "melee4" | "range" | "spell";
  attackStartedAt?: number;
  attackUntil?: number;
  struckStartedAt?: number;
  struckUntil?: number;
  pendingStruck?: {
    attackerId?: string;
    x?: number;
    y?: number;
    direction?: string;
    durationMs: number;
  };
  dieStartedAt?: number;
  dieUntil?: number;
  deathHandled?: boolean;
  reviveStartedAt?: number;
  reviveUntil?: number;
};

// ---------------------------------------------------------------------------
// Projectile
// ---------------------------------------------------------------------------

export type ProjectileState = {
  key: string;
  attackerId: string;
  targetId: string;
  fromX: number;
  fromY: number;
  toX: number;
  toY: number;
  startedAt: number;
  expiresAt: number;
};

export type SceneEffectState = {
  key: string;
  source: "spell" | "attackOverlay" | "actorEffect" | "objectSpell" | "map" | "object";
  spellOrEffect: string | number;
  objectId?: string;
  x: number;
  y: number;
  direction: number;
  value: number;
  startedAt: number;
  expiresAt: number;
};

// ---------------------------------------------------------------------------
// Damage floater (Crystal DamageIndicator)
// ---------------------------------------------------------------------------

export type DamageFloater = {
  key: string;
  objectId: string;
  text: string;
  variant: "hit" | "miss" | "crit" | "heal";
  isPlayerTarget: boolean;
  startedAt: number;
  expiresAt: number;
};

// ---------------------------------------------------------------------------
// Ground drop
// ---------------------------------------------------------------------------

export type GroundDrop = {
  objectId: string;
  name: string;
  nameColourArgb?: number;
  /** Crystal item image index → `/original-ui/Items/{icon}.png`; 0/undefined = generic marker. */
  icon?: number;
  x: number;
  y: number;
  quantity: number;
  sourceMonster: string;
};

// ---------------------------------------------------------------------------
// Items
// ---------------------------------------------------------------------------

export type WorldItem = {
  key: string;
  name: string;
  icon: number;
  uniqueId: number;
  slot: number;
  container: ItemContainer;
  quantity: number;
  description: string;
  durabilityCurrent?: number;
  durabilityMax?: number;
};

export type EquipmentItem = {
  slot: EquipmentSlot;
  name: string;
  icon: number;
  shape?: number;
  description: string;
  durabilityCurrent: number;
  durabilityMax: number;
  attack: number;
  defence: number;
};

// ---------------------------------------------------------------------------
// Quests / skills / buffs / NPC dialog
// ---------------------------------------------------------------------------

export type QuestEntry = {
  questId: number;
  title: string;
  summary: string;
  objective: string;
  progressLabel: string;
  tracker: string;
  stage: QuestStage;
  current: number;
  required: number;
  rewardPreview: string;
  descriptionLines?: string[];
  objectives?: Array<{ label: string; current?: number; required?: number; done?: boolean }>;
  rewards?: {
    gold?: number;
    experience?: number;
    credit?: number;
    items?: Array<{ name: string; icon?: number; count?: number; selectable?: boolean }>;
    selectItems?: Array<{ name: string; icon?: number; count?: number; selectable?: boolean }>;
  };
  timeLimit?: string;
};

export type NpcDialog = {
  npcObjectId: string;
  npcName: string;
  title: string;
  body: string[];
  footer: string;
  links: Array<{ text: string; target: string }>;
  input?: { target: string; prompt: string } | null;
};

export type KnownSkill = {
  key: string;
  name: string;
  description: string;
  spell?: string | null;
  castKind?: "passive" | "toggle" | "self" | "target" | "ground" | "direction";
  offensive?: boolean;
  hotkey?: number;
  delayMs?: number;
  castTimeMs?: number;
  cooldownRemainingTicks: number;
};

export type ActiveBuff = {
  key: string;
  name: string;
  description: string;
  remainingTicks: number;
  attackBonus: number;
  defenceBonus: number;
  type?: number;
  infinite?: boolean;
  paused?: boolean;
  stats?: Array<{ label?: string; value?: number }>;
};

// ---------------------------------------------------------------------------
// Map transfers / rankings
// ---------------------------------------------------------------------------

export type MapTransferArea = {
  key: string;
  mapFileName: string;
  minX: number;
  maxX: number;
  minY: number;
  maxY: number;
  toMapFileName: string;
  toMapTitle: string;
};

export type RankingEntry = {
  rank: number;
  playerId: number;
  name: string;
  level: number;
  classKey: "warrior" | "wizard" | "taoist" | "assassin" | "archer";
};

export type RankingState = {
  rankType: number;
  rankIndex: number;
  onlineOnly: boolean;
  myRank: number;
  count: number;
  entries: RankingEntry[];
  updatedAt: number;
};

// ---------------------------------------------------------------------------
// Stage-5 systems (loosely typed, matches page.tsx Stage5SystemsState)
// ---------------------------------------------------------------------------

export type Stage5SystemsState = {
  group?: {
    members?: string[];
    memberInfos?: Array<{
      name: string;
      level?: number;
      class?: number;
      hp?: number;
      maxHp?: number;
      online?: boolean;
    }>;
    lootMode?: string;
    leaderName?: string;
  };
  guild?: {
    name?: string;
    members?: string[];
    rank?: string;
    permissions?: string[];
    chatLog?: string[];
  };
  social?: {
    friends?: string[];
    blocked?: string[];
    friendInfos?: Array<{ name: string; online?: boolean; memo?: string }>;
    blockedInfos?: Array<{ name: string; memo?: string }>;
  };
  relationship?: Record<string, unknown>;
  mentor?: Record<string, unknown>;
  mail?: Array<Record<string, unknown>>;
  trade?: Record<string, unknown> | null;
  auction?: Array<Record<string, unknown>>;
  conquest?: {
    castleOwner?: string;
    activeWars?: string[];
    eventLog?: string[];
    taxRatePercent?: number;
    gold?: number;
    guards?: number[];
    walls?: number[];
    gates?: number[];
    openGates?: number[];
  };
  guildTerritory?: {
    owned?: boolean;
    mapFileName?: string;
    rentalDaysLeft?: number;
    recallLog?: string[];
  };
  hero?: Record<string, unknown> | null;
  itemRental?: Record<string, unknown>;
  profession?: {
    miningLevel?: number;
    ore?: number;
    craftedItems?: string[];
  };
  appearance?: { hair?: number };
  nameLists?: string[];
  intelligentCreatures?: Array<Record<string, unknown>>;
};

// ---------------------------------------------------------------------------
// WorldState — the primary shape
// ---------------------------------------------------------------------------

/**
 * The complete client-side world model.
 *
 * Structurally compatible with `WorldState` in `app/page.tsx` so integration
 * is a drop-in. `clientTimeMs` is additive and optional — stamped by the
 * snapshot emitter so Bevy can detect stale pushes.
 */
export type WorldState = {
  connected: boolean;
  mapTitle: string | null;
  mapFileName: string | null;
  inSafeZone: boolean;
  playerObjectId: string | null;
  playerName: string | null;
  playerHp?: number;
  playerMaxHp?: number;
  playerMp?: number;
  playerMaxMp?: number;
  playerPkPoints: number;
  playerExperience: number;
  playerMaxExperience: number;
  gold: number;
  credit: number;
  /** Net-new per-city reputation currency wallet, keyed by city. */
  cityCurrencies: Record<string, number>;
  currentWeight: number;
  maxWeight: number;
  freeBagSlots: number;
  maxBagSlots: number;
  storageSize: number;
  hasExpandedStorage: boolean;
  hasStoragePassword: boolean;
  requireStoragePassword: boolean;
  storageSessionUnlocked: boolean;
  storagePasswordLastSetBinaryDatetime: number;
  expandedStorageExpiryTimeBinaryDatetime: number;
  worldTick: number;
  selectedObjectId: string | null;
  miniMapIndex: number | null;
  bigMapIndex: number | null;
  lightSetting: number | null;
  timeOfDayLightSetting: number | null;
  mapLightSetting: number | null;
  mapDarkLight: number;
  weatherParticles: number;
  sceneView: import("../scene-types").SceneView | null;
  terrainPatches: import("../scene-types").TerrainPatch[];
  decorObjects: import("../scene-types").DecorObject[];
  originalMapRegion: import("../scene-types").OriginalMapRegion | null;
  entities: WorldEntity[];
  groundDrops: GroundDrop[];
  mineNodes: { x: number; y: number; stage: number }[];
  beltItems: WorldItem[];
  inventoryItems: WorldItem[];
  storageItems: WorldItem[];
  equipmentItems: EquipmentItem[];
  questLog: QuestEntry[];
  activeNpcDialog: NpcDialog | null;
  knownSkills: KnownSkill[];
  activeBuffs: ActiveBuff[];
  rankings: Record<string, RankingState>;
  rankingCurrentKey: string | null;
  stage5Systems: Stage5SystemsState;
  mapTransfers: MapTransferArea[];
  interactionHints: string[];
  projectiles: ProjectileState[];
  effects: SceneEffectState[];
  damageFloaters: DamageFloater[];
  /**
   * Wall-clock timestamp (ms since epoch) when the snapshot was serialized.
   * Stamped by the snapshot emitter; absent on raw world-state objects.
   * Additive and optional — existing consumers ignore it.
   */
  clientTimeMs?: number;
};

/** Default / empty world state — mirrors DEFAULT_WORLD_STATE in page.tsx. */
export const DEFAULT_WORLD_STATE: WorldState = {
  connected: false,
  mapTitle: null,
  mapFileName: null,
  inSafeZone: false,
  playerObjectId: null,
  playerName: null,
  playerHp: undefined,
  playerMaxHp: undefined,
  playerMp: undefined,
  playerMaxMp: undefined,
  playerPkPoints: 0,
  playerExperience: 0,
  playerMaxExperience: 100,
  gold: 0,
  credit: 0,
  cityCurrencies: {},
  currentWeight: 0,
  maxWeight: 0,
  freeBagSlots: 0,
  maxBagSlots: 0,
  storageSize: 80,
  hasExpandedStorage: false,
  hasStoragePassword: false,
  requireStoragePassword: false,
  storageSessionUnlocked: true,
  storagePasswordLastSetBinaryDatetime: 0,
  expandedStorageExpiryTimeBinaryDatetime: 0,
  worldTick: 0,
  selectedObjectId: null,
  miniMapIndex: null,
  bigMapIndex: null,
  lightSetting: null,
  timeOfDayLightSetting: null,
  mapLightSetting: null,
  mapDarkLight: 0,
  weatherParticles: 0,
  sceneView: null,
  terrainPatches: [],
  decorObjects: [],
  originalMapRegion: null,
  entities: [],
  groundDrops: [],
  mineNodes: [],
  beltItems: [],
  inventoryItems: [],
  storageItems: [],
  equipmentItems: [],
  questLog: [],
  activeNpcDialog: null,
  knownSkills: [],
  activeBuffs: [],
  rankings: {},
  rankingCurrentKey: null,
  stage5Systems: {},
  mapTransfers: [],
  interactionHints: [],
  projectiles: [],
  effects: [],
  damageFloaters: [],
};
