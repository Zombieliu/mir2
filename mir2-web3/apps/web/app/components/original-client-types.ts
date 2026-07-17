import type { OriginalMapRegion } from "../../lib/scene-types";

export type EntityKind = "selfPlayer" | "player" | "monster" | "npc";
export type ItemContainer = "bag1" | "bag2" | "quest" | "belt" | "storage";
export type EntityClassKey = "warrior" | "wizard" | "taoist" | "assassin" | "archer";
export type EntityGenderKey = "male" | "female";
export type CreateCharacterDraft = {
  name: string;
  classKey: EntityClassKey;
  gender: EntityGenderKey;
};
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

export type EntitySprite = {
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
  // Crystal `Libraries.Mounts[MountType]` (`Data\Mount\NN`); present only while the
  // entity is riding a mount, in which case weapon layers are suppressed.
  mountLibrary?: string | null;
  mountFrameOffset?: number | null;
};

export type EntitySpriteAnimationState =
  | "standing"
  | "walking"
  | "running"
  | "attackMelee"
  | "attackRange"
  | "struck"
  | "dying"
  | "dead"
  | "reviving";

export type EntityMotionSnapshot = {
  fromX: number;
  fromY: number;
  toX: number;
  toY: number;
  animationState: EntitySpriteAnimationState;
  frameCount?: number;
  startedAt: number;
  expiresAt: number;
};

export type DisplayEntity = {
  objectId: string;
  kind: EntityKind;
  name: string;
  ownerName?: string;
  x: number;
  y: number;
  direction?: string;
  classKey?: EntityClassKey;
  genderKey?: EntityGenderKey;
  level?: number;
  hp?: number;
  maxHp?: number;
  light?: number;
  nameColourArgb?: number;
  dead?: boolean;
  sneaking?: boolean;
  sprite?: EntitySprite | null;
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
  dieStartedAt?: number;
  dieUntil?: number;
  reviveStartedAt?: number;
  reviveUntil?: number;
};

export type PredictedPlayerMotion = {
  x: number;
  y: number;
  direction?: string;
  movementAnimation?: "walking" | "running";
  movementStartedAt?: number;
  movementUntil?: number;
  movementFrameCount?: number;
};

export type DisplayProjectile = {
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

export type DisplaySceneEffect = {
  key: string;
  source: "spell" | "objectSpell" | "map" | "object";
  spellOrEffect: string | number;
  objectId?: string;
  x: number;
  y: number;
  direction: number;
  value: number;
  startedAt: number;
  expiresAt: number;
};

// One floating combat number anchored over a struck object — Crystal's `Damage`
// (GameScene.DamageIndicator): white over monsters, red over players, plus Miss/Crit.
// `startedAt`/`expiresAt` use the shell motion clock so the overlay rises+fades it
// without owning a timer, and the snapshot merge prunes it once expired.
export type DisplayDamageFloater = {
  key: string;
  objectId: string;
  text: string;
  variant: "hit" | "miss" | "crit" | "heal";
  isPlayerTarget: boolean;
  startedAt: number;
  expiresAt: number;
};

export type DisplayRankingEntry = {
  rank: number;
  playerId: number;
  name: string;
  level: number;
  classKey: EntityClassKey;
};

export type DisplayRankingState = {
  rankType: number;
  rankIndex: number;
  onlineOnly: boolean;
  myRank: number;
  count: number;
  entries: DisplayRankingEntry[];
  updatedAt: number;
};

export type DisplayItem = {
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

export type ItemActionRef = Pick<DisplayItem, "key" | "uniqueId" | "slot" | "container">;
export type EquipmentActionRef = Pick<DisplayEquipmentItem, "slot">;
export type MoveItemRef = Pick<DisplayItem, "uniqueId" | "slot" | "container">;
export type MergeItemRef = Pick<DisplayItem, "uniqueId" | "slot" | "container">;

export type DisplayEquipmentItem = {
  slot: EquipmentSlot;
  name: string;
  icon: number;
  description: string;
  durabilityCurrent: number;
  durabilityMax: number;
  attack: number;
  defence: number;
};

export type DisplayQuest = {
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
};

export type DisplayKnownSkill = {
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

export type DisplayActiveBuff = {
  key: string;
  name: string;
  description: string;
  remainingTicks: number;
  attackBonus: number;
  defenceBonus: number;
};

export type DisplayNpcDialog = {
  npcObjectId: string;
  npcName: string;
  title: string;
  body: string[];
  footer: string;
  links: Array<{
    text: string;
    target: string;
  }>;
  input?: {
    target: string;
    prompt: string;
  } | null;
};

export type DisplayLogLine = {
  text: string;
  tone: "chat" | "system" | "network";
  channel:
    | "normal"
    | "shout"
    | "trade"
    | "whisper"
    | "group"
    | "guild"
    | "mentor"
    | "relationship"
    | "system"
    | "hint"
    | "server"
    | "line"
    | "announcement"
    | "network";
};

export type DisplayWorld = {
  connected: boolean;
  mapTitle: string | null;
  mapFileName: string | null;
  inSafeZone: boolean;
  miniMapIndex: number | null;
  bigMapIndex?: number | null;
  lightSetting?: number | null;
  playerName: string | null;
  playerHp?: number;
  playerMaxHp?: number;
  playerMp?: number;
  playerMaxMp?: number;
  playerExperience: number;
  playerMaxExperience: number;
  gold: number;
  credit: number;
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
  sceneView: {
    center: { x: number; y: number };
    width: number;
    height: number;
  } | null;
  terrainPatches: Array<{
    x: number;
    y: number;
    width: number;
    height: number;
    kind: "grass" | "dirt" | "road" | "water" | "stone";
  }>;
  decorObjects: Array<{
    id: string;
    x: number;
    y: number;
    kind: "lantern" | "banner" | "tree" | "rock" | "campfire" | "stump";
  }>;
  originalMapRegion: OriginalMapRegion | null;
  entities: DisplayEntity[];
  groundDrops: Array<{
    objectId: string;
    name: string;
    nameColourArgb?: number;
    icon?: number;
    x: number;
    y: number;
    quantity: number;
    sourceMonster: string;
  }>;
  beltItems: DisplayItem[];
  inventoryItems: DisplayItem[];
  storageItems: DisplayItem[];
  equipmentItems: DisplayEquipmentItem[];
  questLog: DisplayQuest[];
  activeNpcDialog: DisplayNpcDialog | null;
  knownSkills: DisplayKnownSkill[];
  activeBuffs: DisplayActiveBuff[];
  rankings: Record<string, DisplayRankingState>;
  rankingCurrentKey?: string | null;
  stage5Systems?: {
    group?: { members?: string[]; lootMode?: string };
    guild?: { name?: string; members?: string[]; rank?: string; permissions?: string[]; chatLog?: string[] };
    social?: { friends?: string[]; blocked?: string[] };
    relationship?: Record<string, unknown>;
    mentor?: Record<string, unknown>;
    mail?: DisplayMailMessage[];
    trade?: Record<string, unknown> | null;
    auction?: Array<Record<string, unknown>>;
    conquest?: { castleOwner?: string; activeWars?: string[]; eventLog?: string[]; taxRatePercent?: number; gold?: number };
    guildTerritory?: { owned?: boolean; mapFileName?: string; rentalDaysLeft?: number; recallLog?: string[] };
    hero?: Record<string, unknown> | null;
    itemRental?: Record<string, unknown>;
    profession?: { miningLevel?: number; ore?: number; craftedItems?: string[] };
    appearance?: { hair?: number };
    nameLists?: string[];
    intelligentCreatures?: Array<Record<string, unknown>>;
  };
  interactionHints: string[];
  projectiles: DisplayProjectile[];
  effects: DisplaySceneEffect[];
  damageFloaters: DisplayDamageFloater[];
};

export type DisplayMailMessage = {
  id?: number;
  from?: string;
  to?: string;
  subject?: string;
  body?: string;
  gold?: number;
  items?: string[];
  claimed?: boolean;
  deleted?: boolean;
};

export type SelectCharacterEntry = {
  index: number;
  name: string;
  level: number;
  classKey: EntityClassKey;
  gender: EntityGenderKey;
  lastAccess: string;
};

export type TranslateFn = (
  key: string,
  args?: Array<string | number>,
  fallback?: string,
) => string;
