"use client";

import { useEffect, useMemo, useRef, useState } from "react";
import { flushSync } from "react-dom";
import {
  ExtraWindows,
  adaptHero,
  adaptCreatures,
  adaptGroup,
  adaptFriends,
  adaptRelationship,
  adaptMentor,
  adaptActiveRankingPage,
  adaptMarketListings,
  adaptConquest,
  adaptGuildTerritory,
  adaptTrade,
  adaptBuffs,
  adaptWorldMapMarkers,
  type RankingTabKey,
  type MailMessageSummary,
  type MailComposeDraft,
} from "./components/original-client-extra-windows";
import dynamic from "next/dynamic";

import {
  buildTranslator,
  formatRuntimeMessage,
  languageLocale,
  normalizeLanguage,
  type Mir2Language,
} from "../lib/localization";
import {
  installDebugCapture,
  recordDebugEvent,
  setSnapshotContext,
  setRenderStateProvider,
  downloadSnapshot,
} from "../lib/debug-snapshot";
import { buildRenderStateSummary } from "./components/original-client-scene-map-rendering";
import { playOriginalSoundEvent, playOriginalSoundId } from "../lib/original-audio";
import {
  DUBHE_WALLET_URL,
  getSuiWalletSummaries,
  requestSuiLoginToken,
  sendBootstrapSequence as sendGatewayBootstrapSequence,
  sendNewAccountCommand as sendGatewayNewAccountCommand,
  sendPasswordLoginCommand,
  sendSuiLoginCommand,
  subscribeToSuiWalletChanges,
  type SuiLoginKind,
  type SuiLoginToken,
  type SuiWalletSummary,
} from "../lib/client-login-runtime";
import type {
  DecorObject,
  OriginalMapRegion,
  SceneBlueprint,
  SceneView,
  TerrainPatch,
} from "../lib/scene-types";
import type { ClientScreen } from "../lib/original-ui";
import {
  attackModeLabel,
  groupMembersAfterChange,
  heroCreateResultMessage,
  mailResultMessage,
  normalizeFriendList,
  normalizeMailList,
  normalizeUserItem,
  patchItemsByUniqueId,
  petModeLabel,
  removeItemByUniqueId,
} from "../lib/extended-server-packets";
import bevyRuntimeVersion from "../lib/generated/bevy_runtime_version.json";
import {
  CRYSTAL_MONSTER_SPRITES,
  CRYSTAL_NPC_SPRITES,
  type CrystalMonsterSpriteEntry,
  type CrystalNpcSpriteEntry,
} from "../lib/generated/crystal-actor-sprite-data";
import {
  CRYSTAL_CORRECTION_BLOCK_MS,
  CRYSTAL_MOVE_DELAY_MS,
  CRYSTAL_RUN_PRIME_MS,
  MOVEMENT_PENDING_MAX_AGE_MS,
  canSendMovement,
  createPendingSelfMove,
  effectiveCrystalMovementMode,
  movementPointMatches,
  movementTransformMatches,
  reconcileMovementAck,
  reconcileMovementSnapshot,
  type MovementControllerState,
  type PendingSelfMove,
  type QueuedMoveIntent,
} from "./components/original-client-movement-controller";
import type { BevyEntityRenderState, SceneAssetReadiness } from "./components/original-client-shell-types";
import { OriginalClientTutorialOverlay } from "./components/original-client-tutorial-overlay";

const OriginalClientShell = dynamic(
  () => import("./original-client-shell").then((module) => module.OriginalClientShell),
  {
    ssr: false,
    loading: () => null,
  },
);

type RuntimeStatus = {
  phase: string;
  message: string;
};

type RuntimeModule = {
  default?: (input?: { module_or_path: string | URL | Request } | string | URL | Request) => Promise<unknown>;
  bootMir2Runtime?: () => void;
  getMir2RendererBackend?: () => string;
  setMir2WorldState?: (snapshotJson: string) => void;
  setMir2EntityRenderState?: (snapshotJson: string) => void;
  setMir2EntityRenderAtlas?: (key: string, width: number, height: number, pixels: Uint8Array) => void;
  setMir2StatusSink?: (callback: (payload: RuntimeStatus) => void) => void;
};

type BevyRuntimeBackend = "webgpu" | "webgl2";

type BevyRuntimeSupport = {
  webgpu: boolean;
  webgl2: boolean;
};

type BevyRuntimeDebug = {
  requestedBackend: string | null;
  selectedBackend: BevyRuntimeBackend;
  compiledBackend: string | null;
  fallbackFrom?: BevyRuntimeBackend;
  webgpuSupported: boolean;
  webgl2Supported: boolean;
  runtimeVersion: string;
};

type UiLogTone = "chat" | "system" | "network";
type UiLogChannel =
  | "normal"
  | "shout"
  | "whisper"
  | "trade"
  | "group"
  | "guild"
  | "mentor"
  | "relationship"
  | "system"
  | "hint"
  | "server"
  | "announcement"
  | "network";

type UiLogLine = {
  text: string;
  tone: UiLogTone;
  channel: UiLogChannel;
};

type GatewayEvent =
  | { type: "packet"; packet: string; payload?: Record<string, unknown> }
  | { type: "worldSnapshot"; payload: GatewayWorldSnapshot }
  | { type: "error"; message?: string }
  | { type: string; packet?: string; payload?: Record<string, unknown>; message?: string };

type EntityKind = "selfPlayer" | "player" | "monster" | "npc";
type EntityDisposition = "friendly" | "neutral" | "hostile";
type ItemContainer = "bag1" | "bag2" | "quest" | "belt" | "storage";
type EquipmentSlot =
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
type QuestStage = "available" | "inProgress" | "readyToTurnIn" | "completed";

type GatewayWorldEntitySprite = {
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
};

type GatewayWorldEntity = {
  objectId: number;
  kind: EntityKind;
  name: string;
  ownerName?: string | null;
  x: number;
  y: number;
  direction: string;
  class?: string | number | null;
  gender?: string | number | null;
  level?: number | null;
  hp?: number | null;
  maxHp?: number | null;
  nameColourArgb?: number | null;
  dead: boolean;
  disposition: EntityDisposition;
  sprite?: GatewayWorldEntitySprite | null;
  questIds?: number[];
  bigMapIcon?: number | null;
  showOnBigMap?: boolean | null;
  canTeleportTo?: boolean | null;
};

type GatewayGroundDrop = {
  objectId: number;
  name: string;
  nameColourArgb?: number | null;
  x: number;
  y: number;
  quantity: number;
  sourceMonster: string;
};

type GatewayWorldItem = {
  key: string;
  name: string;
  icon: number;
  uniqueId?: number;
  slot: number;
  container: ItemContainer;
  quantity: number;
  description: string;
  durabilityCurrent?: number | null;
  durabilityMax?: number | null;
};

type GatewayEquipmentItem = {
  slot: EquipmentSlot;
  name: string;
  icon: number;
  shape?: number | null;
  description: string;
  durabilityCurrent: number;
  durabilityMax: number;
  attack: number;
  defence: number;
};

type GatewayQuestEntry = {
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

type GatewayKnownSkill = {
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

type GatewayActiveBuff = {
  key: string;
  name: string;
  description: string;
  remainingTicks: number;
  attackBonus: number;
  defenceBonus: number;
  // Enriched fields emitted by the gateway BuffSnapshot (B-wave).
  buffType?: number;
  remainingMs?: number;
  infinite?: boolean;
  paused?: boolean;
  stats?: Array<{ stat?: number; label?: string; value?: number }>;
};

type Stage5SystemsState = {
  group?: { members?: string[]; memberInfos?: Array<{ name: string; level?: number; class?: number; hp?: number; maxHp?: number; online?: boolean }>; lootMode?: string; leaderName?: string };
  guild?: { name?: string; members?: string[]; rank?: string; permissions?: string[]; chatLog?: string[] };
  social?: { friends?: string[]; blocked?: string[]; friendInfos?: Array<{ name: string; online?: boolean; memo?: string }>; blockedInfos?: Array<{ name: string; memo?: string }> };
  relationship?: Record<string, unknown>;
  mentor?: Record<string, unknown>;
  mail?: Array<Record<string, unknown>>;
  trade?: Record<string, unknown> | null;
  auction?: Array<Record<string, unknown>>;
  conquest?: { castleOwner?: string; activeWars?: string[]; eventLog?: string[]; taxRatePercent?: number; gold?: number; guards?: number[]; walls?: number[]; gates?: number[]; openGates?: number[] };
  guildTerritory?: { owned?: boolean; mapFileName?: string; rentalDaysLeft?: number; recallLog?: string[] };
  hero?: Record<string, unknown> | null;
  itemRental?: Record<string, unknown>;
  profession?: { miningLevel?: number; ore?: number; craftedItems?: string[] };
  appearance?: { hair?: number };
  nameLists?: string[];
  intelligentCreatures?: Array<Record<string, unknown>>;
};

type GatewayMapTransfer = {
  key: string;
  mapFileName: string;
  bounds: {
    minX: number;
    maxX: number;
    minY: number;
    maxY: number;
  };
  toMapFileName: string;
  toMapTitle: string;
  toPosition: {
    x: number;
    y: number;
  };
  toDirection: string;
};

type GatewayNpcDialog = {
  npcObjectId: number;
  npcName: string;
  title: string;
  body: string[];
  footer: string;
  links?: Array<{
    text?: unknown;
    target?: unknown;
  }>;
  input?: {
    target: string;
    prompt: string;
  } | null;
};

type GatewayNpcScriptDiagnostic = {
  scriptKey: string;
  label: string;
  lineNumber: number;
  command: string;
  message: string;
};

type QuickTransferOption = {
  key: string;
  label: string;
};

type GatewayWorldSnapshot = {
  tick: number;
  mapTitle: string | null;
  mapFileName?: string | null;
  inSafeZone?: boolean;
  playerObjectId: number | null;
  playerHp?: number | null;
  playerMaxHp?: number | null;
  playerMp?: number | null;
  playerExperience: number;
  playerMaxExperience: number;
  gold: number;
  credit: number;
  /**
   * Net-new per-city reputation currency balances, keyed by city
   * (`"feitian"`, `"bichon"`). Optional/additive — older snapshots omit it.
   */
  cityCurrencies?: Record<string, number>;
  currentWeight: number;
  maxWeight: number;
  freeBagSlots: number;
  maxBagSlots: number;
  storageSize?: number;
  hasExpandedStorage?: boolean;
  hasStoragePassword?: boolean;
  requireStoragePassword?: boolean;
  storagePasswordLastSetBinaryDatetime?: number;
  expandedStorageExpiryTimeBinaryDatetime?: number;
  sceneView: SceneView | null;
  terrainPatches: TerrainPatch[];
  decorObjects: DecorObject[];
  entities: GatewayWorldEntity[];
  groundDrops: GatewayGroundDrop[];
  beltItems: GatewayWorldItem[];
  inventoryItems: GatewayWorldItem[];
  storageItems?: GatewayWorldItem[];
  equipmentItems: GatewayEquipmentItem[];
  questLog: GatewayQuestEntry[];
  activeNpcDialog?: GatewayNpcDialog | null;
  npcScriptDiagnostics?: GatewayNpcScriptDiagnostic[];
  knownSkills: GatewayKnownSkill[];
  activeBuffs: GatewayActiveBuff[];
  stage5Systems?: Stage5SystemsState;
  mapTransfers: GatewayMapTransfer[];
  interactionHints: string[];
};

type WorldEntity = {
  objectId: string;
  kind: EntityKind;
  name: string;
  ownerName?: string;
  x: number;
  y: number;
  direction?: string;
  classKey?: SelectCharacterEntry["classKey"];
  genderKey?: SelectCharacterEntry["gender"];
  level?: number;
  hp?: number;
  maxHp?: number;
  nameColourArgb?: number;
  dead?: boolean;
  disposition?: EntityDisposition;
  sprite?: GatewayWorldEntitySprite | null;
  questIds?: number[];
  bigMapIcon?: number;
  showOnBigMap?: boolean;
  canTeleportTo?: boolean;
  movementAnimation?: "walking" | "running";
  movementStartedAt?: number;
  movementUntil?: number;
  attackAnimation?: "melee1" | "melee2" | "melee3" | "melee4" | "range";
  attackStartedAt?: number;
  attackUntil?: number;
  struckStartedAt?: number;
  struckUntil?: number;
  dieStartedAt?: number;
  dieUntil?: number;
  reviveStartedAt?: number;
  reviveUntil?: number;
};

function entityMovementRenderFieldsMatch(left: WorldEntity, right: WorldEntity) {
  return (
    left.x === right.x &&
    left.y === right.y &&
    left.direction === right.direction &&
    left.movementAnimation === right.movementAnimation &&
    left.movementStartedAt === right.movementStartedAt &&
    left.movementUntil === right.movementUntil
  );
}

type ProjectileState = {
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

type WorldItem = {
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

type ItemCommandRef = {
  key: string;
  uniqueId: number;
  slot: number;
  container: ItemContainer;
};

type EquipmentCommandRef = {
  slot: EquipmentSlot;
};

type ItemMoveRef = {
  uniqueId?: number;
  slot: number;
  container: ItemContainer;
};

type ItemMergeRef = {
  uniqueId: number;
  slot: number;
  container: ItemContainer;
};

type EquipmentItem = {
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

type QuestEntry = {
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
  // B-wave-2 enriched fields (structurally match the quest window's optional props).
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

type NpcDialog = {
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

type KnownSkill = {
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

type ActiveBuff = {
  key: string;
  name: string;
  description: string;
  remainingTicks: number;
  attackBonus: number;
  defenceBonus: number;
  // Enriched fields (B-wave): Crystal buff type, infinite/paused flags, and stat lines.
  type?: number;
  infinite?: boolean;
  paused?: boolean;
  stats?: Array<{ label?: string; value?: number }>;
};

type MapTransferArea = {
  key: string;
  mapFileName: string;
  minX: number;
  maxX: number;
  minY: number;
  maxY: number;
  toMapFileName: string;
  toMapTitle: string;
};

type GroundDrop = {
  objectId: string;
  name: string;
  nameColourArgb?: number;
  x: number;
  y: number;
  quantity: number;
  sourceMonster: string;
};

type WorldState = {
  connected: boolean;
  mapTitle: string | null;
  mapFileName: string | null;
  inSafeZone: boolean;
  playerObjectId: string | null;
  playerName: string | null;
  playerHp?: number;
  playerMaxHp?: number;
  playerMp?: number;
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
  sceneView: SceneView | null;
  terrainPatches: TerrainPatch[];
  decorObjects: DecorObject[];
  originalMapRegion: OriginalMapRegion | null;
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
};

type SelectCharacterEntry = {
  index: number;
  name: string;
  level: number;
  classKey: "warrior" | "wizard" | "taoist" | "assassin" | "archer";
  gender: "male" | "female";
  lastAccess: string;
};

type RankingEntry = {
  rank: number;
  playerId: number;
  name: string;
  level: number;
  classKey: SelectCharacterEntry["classKey"];
};

type RankingState = {
  rankType: number;
  rankIndex: number;
  onlineOnly: boolean;
  myRank: number;
  count: number;
  entries: RankingEntry[];
  updatedAt: number;
};

type RankingRequestState = {
  rankType: number;
  rankIndex: number;
  onlineOnly: boolean;
};

type ReconnectMode = "idle" | "scheduled" | "connecting" | "resuming" | "failed";
type WorldSnapshotRealtimeMode = "bootstrap" | "reconnect" | "mapChange" | "sceneBootstrap" | "packetRefresh";

type ReconnectStatus = {
  mode: ReconnectMode;
  attempt: number;
  nextAttemptAt: number | null;
};

type ReconnectAuthSnapshot =
  | {
      kind: "password";
      accountId: string;
      password: string;
    }
  | {
      kind: "sui";
      accountId: string;
      token: string;
      expiresAt: number;
    };

type ReconnectSnapshot = {
  auth: ReconnectAuthSnapshot;
  characterIndex: number;
  characterName: string | null;
};

type DebugSnapshotUploadNotice = {
  status: "saved" | "uploading" | "uploaded" | "failed";
  message: string;
  sessionId?: string;
  snapshotId?: string | number | null;
};

const RECONNECT_DELAYS_MS = [1000, 2000, 4000, 8000, 12000];
const MAX_RECONNECT_ATTEMPTS = 6;
const CRYSTAL_NPC_SPRITE_BY_OBJECT_ID: ReadonlyMap<number, CrystalNpcSpriteEntry> = new Map(
  CRYSTAL_NPC_SPRITES.map((entry) => [entry.objectId, entry] as const),
);
const CRYSTAL_NPC_SPRITE_BY_LOCATION: ReadonlyMap<string, CrystalNpcSpriteEntry> = new Map(
  CRYSTAL_NPC_SPRITES.map((entry) => [crystalActorLocationKey(entry.map, entry.name, entry.x, entry.y), entry] as const),
);
const CRYSTAL_MONSTER_SPRITE_BY_NAME: ReadonlyMap<string, CrystalMonsterSpriteEntry> = new Map(
  CRYSTAL_MONSTER_SPRITES.map((entry) => [normalizeCrystalActorName(entry.name), entry] as const),
);

function createIdleReconnectStatus(): ReconnectStatus {
  return { mode: "idle", attempt: 0, nextAttemptAt: null };
}

const DEFAULT_WORLD_STATE: WorldState = {
  connected: false,
  mapTitle: null,
  mapFileName: null,
  inSafeZone: false,
  playerObjectId: null,
  playerName: null,
  playerHp: undefined,
  playerMaxHp: undefined,
  playerMp: undefined,
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
};

const VIEWPORT_CELL_WIDTH = 48;
const VIEWPORT_CELL_HEIGHT = 32;
const VIEWPORT_OFFSET_X = Math.floor(1024 / 2 / VIEWPORT_CELL_WIDTH);
const VIEWPORT_OFFSET_Y = Math.floor(768 / 2 / VIEWPORT_CELL_HEIGHT) - 1;
const VIEWPORT_RANGE_X = VIEWPORT_OFFSET_X + 6;
const VIEWPORT_RANGE_Y = VIEWPORT_OFFSET_Y + 6;
const SCENE_CHUNK_WIDTH = Math.max(VIEWPORT_RANGE_X, 1);
const SCENE_CHUNK_HEIGHT = Math.max(VIEWPORT_RANGE_Y, 1);
const SCENE_PREFETCH_MARGIN_X = Math.max(8, Math.floor(VIEWPORT_RANGE_X * 0.75));
const SCENE_PREFETCH_MARGIN_Y = Math.max(10, VIEWPORT_RANGE_Y);
const SCENE_REQUEST_WIDTH = VIEWPORT_RANGE_X * 2 + SCENE_PREFETCH_MARGIN_X * 2;
const SCENE_REQUEST_HEIGHT = VIEWPORT_RANGE_Y * 2 + SCENE_PREFETCH_MARGIN_Y * 2;
const SCENE_RELOAD_MARGIN_X = Math.max(4, Math.floor(SCENE_PREFETCH_MARGIN_X / 2));
const SCENE_RELOAD_MARGIN_Y = Math.max(4, Math.floor(SCENE_PREFETCH_MARGIN_Y / 2));
const WALK_STEP_INTERVAL_MS = CRYSTAL_MOVE_DELAY_MS;
const RUN_STEP_INTERVAL_MS = CRYSTAL_MOVE_DELAY_MS;
const CRYSTAL_GAMEPLAY_TICK_MS = 1200;
const CRYSTAL_KEEPALIVE_INTERVAL_MS = 15_000;
const CRYSTAL_INPUT_CORRECTION_DELAY_MS = 400;
const CRYSTAL_ENTITY_MOVE_ACTION_MS = CRYSTAL_MOVE_DELAY_MS;
const CRYSTAL_MOVE_FRAME_INTERVAL_MS = 100;
const MOVEMENT_CONFIRM_TICK_DELAY_MS = 160;
const MOVEMENT_TURN_VISUAL_HOLD_MS = CRYSTAL_ENTITY_MOVE_ACTION_MS;
const MOVEMENT_QUEUE_INPUT_LEAD_MS = 0;
const MOVEMENT_SERVER_CORRECTION_GRACE_MS = 1000;
const MOVEMENT_PENDING_ACTION_MAX_AGE_MS = MOVEMENT_PENDING_MAX_AGE_MS;
const MOVEMENT_OUTSTANDING_ACTION_SETTLE_GRACE_MS = CRYSTAL_ENTITY_MOVE_ACTION_MS + MOVEMENT_CONFIRM_TICK_DELAY_MS;
const MOVEMENT_PENDING_ACTION_RECOVERY_MS = CRYSTAL_ENTITY_MOVE_ACTION_MS + CRYSTAL_INPUT_CORRECTION_DELAY_MS;
const MOVEMENT_PREDICTED_CORRECTION_HOLD_MS = CRYSTAL_INPUT_CORRECTION_DELAY_MS;
const MOVEMENT_LOCAL_PREDICTION_MIN_HOLD_MS = CRYSTAL_ENTITY_MOVE_ACTION_MS;
const MOVEMENT_ACTION_PREDICTION_BLOCK_MS = CRYSTAL_ENTITY_MOVE_ACTION_MS + CRYSTAL_INPUT_CORRECTION_DELAY_MS;
const PACKET_RUNTIME_SNAPSHOT_TOMBSTONE_MS = 10_000;
const MOVEMENT_QUEUED_DIRECTION_MAX_AGE_MS =
  MOVEMENT_PENDING_ACTION_RECOVERY_MS + MOVEMENT_CONFIRM_TICK_DELAY_MS;
const MOVEMENT_LOCAL_ACTION_MAX_LEAD_TILES = 2;
const MOVEMENT_LOCAL_RENDER_MAX_LEAD_TILES = MOVEMENT_LOCAL_ACTION_MAX_LEAD_TILES;
const MOVEMENT_DIRECTION_PENDING_MAX = 1;
const MOVEMENT_ROUTE_REROUTE_AFTER_MS = 900;
const MOVEMENT_ROUTE_RETRY_DELAY_MS = 120;
const MOVEMENT_ROUTE_BLOCK_MEMORY_MS = 5600;
const MOVEMENT_HELD_BLOCKED_DIRECTION_SUPPRESS_MS = 60_000;
const MOVEMENT_QUEUED_DIRECTION_REPEAT_MAX = 6;
const MOVEMENT_QUEUED_DIRECTION_REPEAT_MAX_AGE_MS =
  CRYSTAL_ENTITY_MOVE_ACTION_MS * (MOVEMENT_QUEUED_DIRECTION_REPEAT_MAX + 2);
const MOVEMENT_ROUTE_MAX_BLOCKED_STEPS = 12;
const MOVEMENT_ROUTE_SEARCH_MARGIN = 14;
const MOVEMENT_ROUTE_SEARCH_MAX_NODES = 420;
const CRYSTAL_MOVEMENT_DIRECTIONS = [
  "Up",
  "UpRight",
  "Right",
  "DownRight",
  "Down",
  "DownLeft",
  "Left",
  "UpLeft",
];
const CONFIGURED_GATEWAY_WS_URL = process.env.NEXT_PUBLIC_MIR2_GATEWAY_WS_URL?.trim();
const LOCAL_GATEWAY_WS_URL = "ws://127.0.0.1:7110/ws";
const HOSTED_GATEWAY_WS_URL = "wss://165.154.65.136.sslip.io/ws";
const BEVY_RUNTIME_VERSION =
  [bevyRuntimeVersion.version, process.env.NEXT_PUBLIC_VERCEL_GIT_COMMIT_SHA]
    .filter((value): value is string => Boolean(value))
    .join("-") || "local";
const QUICK_TRANSFER_OPTIONS: QuickTransferOption[] = [
  { key: "crystal:0:330:270", label: "Bichon Province (0)" },
  { key: "crystal:1:315:82", label: "Woomyon Woods S (1)" },
  { key: "crystal:2:503:483", label: "Serpent Valley (2)" },
  { key: "crystal:n0:200:200", label: "n0 (QA)" },
  { key: "crystal:HF1:200:200", label: "HellFire 1F (HF1)" },
  { key: "crystal:HF2:200:200", label: "HellFire 2F (HF2)" },
  { key: "crystal:HF3:200:200", label: "HellFire 3F (HF3)" },
  { key: "crystal:D1801:200:200", label: "Penal Cavern (D1801)" },
  { key: "crystal:HKR:200:200", label: "HellFire Kings Room (HKR)" },
];

function isLocalWebHost(hostname: string) {
  return hostname === "localhost" || hostname === "127.0.0.1" || hostname === "::1" || hostname === "[::1]";
}

function resolveGatewayWebSocketUrl() {
  if (typeof window === "undefined") return CONFIGURED_GATEWAY_WS_URL || LOCAL_GATEWAY_WS_URL;
  const queryValue = new URLSearchParams(window.location.search).get("gatewayWs");
  if (queryValue && /^wss?:\/\//.test(queryValue)) return queryValue;
  if (CONFIGURED_GATEWAY_WS_URL) return CONFIGURED_GATEWAY_WS_URL;
  return isLocalWebHost(window.location.hostname) ? LOCAL_GATEWAY_WS_URL : HOSTED_GATEWAY_WS_URL;
}

function markMir2CacheMilestone(name: string, detail?: Record<string, unknown>) {
  if (typeof window === "undefined") return;
  const metricsWindow = window as typeof window & {
    __mir2CacheMetrics?: {
      markMilestone?: (name: string, detail?: Record<string, unknown>) => unknown;
    };
    __mir2PendingCacheMilestones?: Array<{ name: string; detail?: Record<string, unknown> }>;
  };
  if (metricsWindow.__mir2CacheMetrics?.markMilestone) {
    metricsWindow.__mir2CacheMetrics.markMilestone(name, detail);
    return;
  }
  metricsWindow.__mir2PendingCacheMilestones = [
    ...(metricsWindow.__mir2PendingCacheMilestones ?? []),
    { name, detail },
  ].slice(-50);
}

function scheduleBevyRuntimeCacheRecovery(message: string) {
  if (typeof window === "undefined") return false;
  if (!message.includes("WebAssembly.instantiate") || !message.includes("mir2_bevy_runtime_bg.js")) {
    return false;
  }

  const nextUrl = new URL(window.location.href);
  if (nextUrl.searchParams.get("runtimeRecovery") === "1") return false;

  markMir2CacheMilestone("bevyRuntimeCacheRecovery", {
    message,
    runtimeVersion: BEVY_RUNTIME_VERSION,
  });

  nextUrl.searchParams.set("runtimeRecovery", "1");
  nextUrl.searchParams.set("runtimeBust", String(Date.now()));

  let reloaded = false;
  const reloadOnce = () => {
    if (reloaded) return;
    reloaded = true;
    window.location.replace(nextUrl.toString());
  };

  const cacheWindow = window as typeof window & {
    __mir2AssetCacheReset?: (options?: { reload?: boolean }) => Promise<unknown>;
  };
  const resetPromise = cacheWindow.__mir2AssetCacheReset?.({ reload: false });
  if (resetPromise) {
    void resetPromise.finally(reloadOnce);
    window.setTimeout(reloadOnce, 1200);
  } else {
    window.setTimeout(reloadOnce, 150);
  }

  return true;
}

type MovementPlan = {
  targetX: number;
  targetY: number;
  mode: "walk" | "run";
  packetMode?: "target" | "direction";
  nextStepAt: number;
  actionX?: number;
  actionY?: number;
  pendingX?: number;
  pendingY?: number;
  pendingSentAt?: number;
  visualUntil?: number;
  sentFromX?: number;
  sentFromY?: number;
  sentDirection?: string;
  sentMode?: "walk" | "run";
  blockedSteps?: MovementBlockedStep[];
};

type MovementBlockedStep = {
  fromX: number;
  fromY: number;
  direction: string;
  mode: "walk" | "run";
  at: number;
};

type DirectionStepRequest = {
  x?: number;
  y?: number;
  direction?: string;
  mode: "walk" | "run";
  requestedAt: number;
  repeatCount?: number;
};

type DirectionStepPending = {
  x: number;
  y: number;
  direction?: string;
  mode: "walk" | "run";
  sentAt: number;
  sentFromX?: number;
  sentFromY?: number;
};

type PredictedPlayerMotion = {
  x: number;
  y: number;
  direction?: string;
};

type PendingSelfTurn = {
  direction: string;
  sentAt: number;
  visualUntil: number;
};

type LocalMovementAnchor = PredictedPlayerMotion & {
  until: number;
};

type CrystalSelfActionFeedEntry = {
  fromX: number;
  fromY: number;
  x: number;
  y: number;
  direction: string;
  mode: "walk" | "run";
  sentAt: number;
  visualUntil: number;
};

type CrystalSelfAckDisposition = "none" | "confirmed" | "staleEcho" | "correction";

type MovementDiagnosticPoint = {
  x: number;
  y: number;
  direction?: string;
};

type MovementDiagnosticSample = {
  screen: ClientScreen;
  mapFileName: string | null;
  mapTitle: string | null;
  worldTick: number;
  worldSnapshotVersion: number;
  worldSnapshotRealtimeMode: WorldSnapshotRealtimeMode;
  self: (MovementDiagnosticPoint & {
    movementAnimation?: string;
    movementStartedAt?: number;
    movementUntil?: number;
  }) | null;
  render: MovementDiagnosticPoint | null;
  predicted: PredictedPlayerMotion | null;
  scene: {
    motionNow?: number;
    playerCameraMotionOffset?: unknown;
    playerMotionSnapshot?: unknown;
  } | null;
  queues: {
    movementPlan: MovementPlan | null;
    pendingSelfMove: PendingSelfMove | null;
    queuedMoveIntent: QueuedMoveIntent | null;
    nextMoveWaitMs: number;
    queuedDirectionStep: DirectionStepRequest | null;
    directionStepPending: DirectionStepPending | null;
    directionStepPendingQueueLength: number;
    crystalSelfActionFeedLength: number;
    outstandingSelfMovementActionsLength: number;
    movementInputBlockedForMs: number;
  };
  transport: {
    lastMovementCommand: Record<string, unknown> | null;
    lastSelfMovementAck: { x: number; y: number; direction?: string; at: number } | null;
    lastSelfNoProgressAck: {
      x: number;
      y: number;
      direction?: string;
      at: number;
      count: number;
    } | null;
  };
};

type MovementDiagnosticState = {
  enabled: boolean;
  sessionId: string;
  startedAt: number;
  events: Array<Record<string, unknown>>;
  pendingFlush: boolean;
  lastSample: MovementDiagnosticSample | null;
  lastMovementCommand: Record<string, unknown> | null;
};

type MovementConsoleWindow = typeof window & {
  __mir2MovementLogEnabled?: boolean;
  __mir2MovementConsoleSeq?: number;
  __mir2MovementConsoleEvents?: Array<Record<string, unknown>>;
  __mir2PendingMovementConsoleCommands?: Array<Record<string, unknown>>;
};

const MOVEMENT_LOG_STORAGE_KEY = "mir2-movement-log";

function isMovementLogFlagEnabled(value: string | null) {
  return value === "1" || value === "true" || value === "yes" || value === "on";
}

function isMovementLogFlagDisabled(value: string | null) {
  return value === "0" || value === "false" || value === "no" || value === "off";
}

function movementConsoleLogEnabled() {
  if (typeof window === "undefined") return false;

  const debugWindow = window as MovementConsoleWindow;
  if (debugWindow.__mir2MovementLogEnabled === true) return true;
  if (debugWindow.__mir2MovementLogEnabled === false) return false;

  const params = new URLSearchParams(window.location.search);
  const queryFlag =
    params.get("movementLog") ??
    params.get("moveLog") ??
    params.get("movementConsole");
  if (isMovementLogFlagEnabled(queryFlag)) return true;
  if (isMovementLogFlagDisabled(queryFlag)) return false;

  if (
    params.get("movementDiag") === "1" ||
    params.get("moveDiag") === "1" ||
    params.get("movementDiagnostics") === "1"
  ) {
    return true;
  }

  try {
    return isMovementLogFlagEnabled(window.localStorage.getItem(MOVEMENT_LOG_STORAGE_KEY));
  } catch {
    return false;
  }
}

function isMovementConsoleCommand(command: Record<string, unknown>) {
  return (
    command.type === "walk" ||
    command.type === "run" ||
    command.type === "turn" ||
    command.type === "moveTo"
  );
}

function assignMovementConsoleSequence(command: Record<string, unknown>) {
  const debugWindow = window as MovementConsoleWindow;
  const sequence = (debugWindow.__mir2MovementConsoleSeq ?? 0) + 1;
  debugWindow.__mir2MovementConsoleSeq = sequence;
  return { ...command, movementSeq: sequence };
}

function rememberMovementConsoleCommand(command: Record<string, unknown>) {
  const debugWindow = window as MovementConsoleWindow;
  debugWindow.__mir2PendingMovementConsoleCommands = [
    ...(debugWindow.__mir2PendingMovementConsoleCommands ?? []),
    command,
  ].slice(-40);
}

function consumeMovementConsoleCommand() {
  const debugWindow = window as MovementConsoleWindow;
  const pending = debugWindow.__mir2PendingMovementConsoleCommands ?? [];
  const command = pending.shift() ?? null;
  debugWindow.__mir2PendingMovementConsoleCommands = pending.slice(-40);
  return command;
}

function recordMovementConsoleEvent(kind: "send" | "ack" | "correction", payload: Record<string, unknown>) {
  if (!movementConsoleLogEnabled()) return;

  const event = {
    kind,
    at: Date.now(),
    ...payload,
  };
  const debugWindow = window as MovementConsoleWindow;
  debugWindow.__mir2MovementConsoleEvents = [
    event,
    ...(debugWindow.__mir2MovementConsoleEvents ?? []),
  ].slice(0, 100);

  const label = `[mir2-move:${kind}]`;
  if (kind === "correction") {
    console.warn(label, event);
  } else {
    console.info(label, event);
  }
}

export default function HomePage() {
  const runtimeRef = useRef<RuntimeModule | null>(null);
  const socketRef = useRef<WebSocket | null>(null);
  const worldRef = useRef<WorldState>(DEFAULT_WORLD_STATE);
  const screenRef = useRef<ClientScreen>("login");
  const pendingLoginRef = useRef(false);
  const pendingNewAccountRef = useRef(false);
  const pendingSuiLoginRef = useRef<SuiLoginToken | null>(null);
  const lastRankingRequestRef = useRef<RankingRequestState>({
    rankType: 0,
    rankIndex: 0,
    onlineOnly: false,
  });
  const pendingTransferRef = useRef<string | null>(null);
  const pendingNpcInteractRef = useRef<string | null>(null);
  const npcCallGuardRef = useRef<{ objectId: string; until: number } | null>(null);
  const gameEntryChatSeededRef = useRef(false);
  const movementPlanRef = useRef<MovementPlan | null>(null);
  const movementBlockedStepsRef = useRef<MovementBlockedStep[]>([]);
  const directionStepNextAtRef = useRef(0);
  const directionStepVisualUntilRef = useRef(0);
  const movementInputBlockedUntilRef = useRef(0);
  const crystalRunPrimedUntilRef = useRef(0);
  const queuedDirectionStepRef = useRef<DirectionStepRequest | null>(null);
  const queuedDirectionStepBacklogRef = useRef<DirectionStepRequest[]>([]);
  const directionStepPendingRef = useRef<DirectionStepPending | null>(null);
  const directionStepPendingQueueRef = useRef<DirectionStepPending[]>([]);
  const crystalSelfActionFeedRef = useRef<CrystalSelfActionFeedEntry[]>([]);
  const outstandingSelfMovementActionsRef = useRef<CrystalSelfActionFeedEntry[]>([]);
  const recentSelfMovementActionHistoryRef = useRef<CrystalSelfActionFeedEntry[]>([]);
  const pendingSelfMoveRef = useRef<PendingSelfMove | null>(null);
  const pendingSelfTurnRef = useRef<PendingSelfTurn | null>(null);
  const queuedMoveIntentRef = useRef<QueuedMoveIntent | null>(null);
  const pendingGroundSkillRef = useRef<KnownSkill | null>(null);
  const nextMoveSendAtRef = useRef(0);
  const predictedPlayerPositionRef = useRef<PredictedPlayerMotion | null>(null);
  const predictedPlayerHoldUntilRef = useRef(0);
  const predictedPlayerUpdateSeqRef = useRef(0);
  const localMovementAnchorRef = useRef<LocalMovementAnchor | null>(null);
  const lastCrystalSelfRenderPositionRef = useRef<PredictedPlayerMotion | null>(null);
  const lastSelfMovementAckRef = useRef<{ x: number; y: number; direction?: string; at: number } | null>(null);
  const lastSelfNoProgressAckRef = useRef<{
    x: number;
    y: number;
    direction?: string;
    at: number;
    count: number;
  } | null>(null);
  const heldDirectionBlockedUntilRef = useRef<{
    x: number;
    y: number;
    direction: string;
    until: number;
  } | null>(null);
  const movementPredictionBlockedUntilRef = useRef(0);
  const loadedSceneKeyRef = useRef<string | null>(null);
  const loadingSceneKeyRef = useRef<string | null>(null);
  const lastCommandRef = useRef<Record<string, unknown> | null>(null);
  const movementConfirmTickTimerRef = useRef<number | null>(null);
  const worldSnapshotVersionRef = useRef(0);
  const packetRuntimeSnapshotModeRef = useRef<WorldSnapshotRealtimeMode>("bootstrap");
  const packetRuntimeObjectTombstonesRef = useRef<Map<string, number>>(new Map());
  const movementDiagnosticsRef = useRef<MovementDiagnosticState | null>(null);
  const sceneSpritesReadyKeyRef = useRef<string | null>(null);
  const firstPlayableFrameMarkedRef = useRef(false);
  const initialSceneAssetsReadyRef = useRef(false);
  const sceneAssetReadinessRef = useRef<SceneAssetReadiness | null>(null);
  const lastSceneAssetMilestoneKeyRef = useRef<string | null>(null);
  const lastDeferredSceneInputAtRef = useRef(0);
  const manualSocketCloseRef = useRef(false);
  const reconnectTimerRef = useRef<number | null>(null);
  const reconnectAttemptRef = useRef(0);
  const reconnectSnapshotRef = useRef<ReconnectSnapshot | null>(null);
  const reconnectStatusRef = useRef<ReconnectStatus>(createIdleReconnectStatus());
  const activeReconnectAuthRef = useRef<ReconnectAuthSnapshot | null>(null);
  const accountIdRef = useRef("demo");
  const passwordRef = useRef("demo");
  const charactersRef = useRef<SelectCharacterEntry[]>([]);
  const selectedCharacterIndexRef = useRef(0);
  const uploadedBevyEntityAtlasKeysRef = useRef<Set<string>>(new Set());
  const lastBevyEntityRenderStateJsonRef = useRef<string | null>(null);

  const [language, setLanguage] = useState<Mir2Language>("en");
  const [runtimePhase, setRuntimePhase] = useState("idle");
  const [runtimeMessage, setRuntimeMessage] = useState("Runtime not booted");
  const [bevyEntityRendererReady, setBevyEntityRendererReady] = useState(false);
  const [bevyRuntimeBackend, setBevyRuntimeBackend] = useState<BevyRuntimeBackend | null>(null);
  const [screen, setScreen] = useState<ClientScreen>("login");
  const [world, setWorld] = useState<WorldState>(DEFAULT_WORLD_STATE);
  const [logs, setLogs] = useState<UiLogLine[]>([]);
  const [accountId, setAccountId] = useState("demo");
  const [password, setPassword] = useState("demo");
  const [chatMessage, setChatMessage] = useState("");
  const [loginBusy, setLoginBusy] = useState(false);
  const [loginErrorKey, setLoginErrorKey] = useState<string | null>(null);
  const [suiWallets, setSuiWallets] = useState<SuiWalletSummary[]>([]);
  const [walletPickerOpen, setWalletPickerOpen] = useState(false);
  const [characters, setCharacters] = useState<SelectCharacterEntry[]>(() => [fallbackCharacter("en")]);
  const [selectedCharacterIndex, setSelectedCharacterIndex] = useState(0);
  const [wsState, setWsState] = useState("closed");
  const [reconnectStatus, setReconnectStatus] = useState<ReconnectStatus>(() => createIdleReconnectStatus());
  const [showInventory, setShowInventory] = useState(false);
  const [showCharacter, setShowCharacter] = useState(false);
  const [showQuestLog, setShowQuestLog] = useState(false);
  // Net-new interactive beginner tutorial overlay (no Crystal equivalent).
  const [showTutorial, setShowTutorial] = useState(false);
  const [showHeroPet, setShowHeroPet] = useState(false);
  const [showGuild, setShowGuild] = useState(false);
  const [showGroup, setShowGroup] = useState(false);
  const [showFriends, setShowFriends] = useState(false);
  const [showBonds, setShowBonds] = useState(false);
  const [showRanking, setShowRanking] = useState(false);
  const [showMarket, setShowMarket] = useState(false);
  const [showConquest, setShowConquest] = useState(false);
  const [showTrade, setShowTrade] = useState(false);
  const [showBuffs, setShowBuffs] = useState(false);
  const [showMail, setShowMail] = useState(false);
  const [showWorldMap, setShowWorldMap] = useState(false);
  const [showHelp, setShowHelp] = useState(false);
  const [showHotkeys, setShowHotkeys] = useState(false);
  const [showChatSettings, setShowChatSettings] = useState(false);
  const [debugSnapshotNotice, setDebugSnapshotNotice] = useState<DebugSnapshotUploadNotice | null>(null);
  useEffect(() => {
    let clearTimer = 0;
    const onDebugSnapshotUpload = (event: Event) => {
      const detail = (event as CustomEvent<DebugSnapshotUploadNotice>).detail;
      if (!detail || typeof detail.message !== "string") return;
      setDebugSnapshotNotice(detail);
      if (clearTimer) window.clearTimeout(clearTimer);
      const keepMs = detail.status === "uploading" ? 0 : detail.status === "failed" ? 9000 : 6500;
      if (keepMs > 0) {
        clearTimer = window.setTimeout(() => setDebugSnapshotNotice(null), keepMs);
      }
    };
    window.addEventListener("mir2:debug-snapshot-upload", onDebugSnapshotUpload);
    return () => {
      if (clearTimer) window.clearTimeout(clearTimer);
      window.removeEventListener("mir2:debug-snapshot-upload", onDebugSnapshotUpload);
    };
  }, []);
  useEffect(() => {
    installDebugCapture();
    setSnapshotContext(() => {
      const snapshotWorld = worldRef.current;
      const snapshotSelf = Array.isArray(snapshotWorld.entities)
        ? snapshotWorld.entities.find((entity) => entity.objectId === snapshotWorld.playerObjectId)
        : undefined;
      return {
        map: snapshotWorld.mapFileName ?? null,
        mapTitle: snapshotWorld.mapTitle ?? null,
        player: snapshotSelf ? { x: snapshotSelf.x, y: snapshotSelf.y, name: snapshotSelf.name } : null,
        entityCount: Array.isArray(snapshotWorld.entities) ? snapshotWorld.entities.length : 0,
        gold: snapshotWorld.gold,
      };
    });
    setRenderStateProvider(() => {
      const snapshotWorld = worldRef.current;
      const snapshotSelf = Array.isArray(snapshotWorld.entities)
        ? snapshotWorld.entities.find((entity) => entity.objectId === snapshotWorld.playerObjectId) ?? null
        : null;
      const summary = buildRenderStateSummary(snapshotWorld, snapshotSelf);
      // Movement-lock diagnostic: distinguishes "input gated off" (initialSceneAssetsReady
      // false) from "input gated ON but send is blocked" (isMovementBusy stuck on an
      // unconfirmed pending move). Pairs with window.__mir2SceneGate (readiness factors).
      const movementGate = {
        initialSceneAssetsReady: initialSceneAssetsReadyRef.current,
        firstPlayableFrameMarked: firstPlayableFrameMarkedRef.current,
        isMovementBusy: isMovementBusy(),
        pendingSelfMove: Boolean(pendingSelfMoveRef.current),
        queuedMoveIntent: Boolean(queuedMoveIntentRef.current),
        movementPlan: Boolean(movementPlanRef.current),
        playerObjectId: snapshotWorld.playerObjectId ?? null,
        hasSelfEntity: Boolean(snapshotSelf),
      };
      return { ...(summary as Record<string, unknown>), movementGate };
    });
    const onExtraWindowHotkey = (event: KeyboardEvent) => {
      if (!event.altKey || event.ctrlKey || event.metaKey) return;
      const key = event.key.toLowerCase();
      const matchesKey = (letter: string) => key === letter || event.code === `Key${letter.toUpperCase()}`;
      if (matchesKey("q")) { event.preventDefault(); setShowQuestLog((value) => !value); }
      else if (matchesKey("h")) { event.preventDefault(); setShowHeroPet((value) => !value); }
      else if (matchesKey("g")) { event.preventDefault(); setShowGuild((value) => !value); }
      else if (matchesKey("p")) { event.preventDefault(); setShowGroup((value) => !value); }
      else if (matchesKey("f")) { event.preventDefault(); setShowFriends((value) => !value); }
      else if (matchesKey("b")) { event.preventDefault(); setShowBonds((value) => !value); }
      else if (matchesKey("r")) { event.preventDefault(); setShowRanking((value) => !value); }
      else if (matchesKey("m")) { event.preventDefault(); setShowMarket((value) => !value); }
      else if (matchesKey("k")) { event.preventDefault(); setShowConquest((value) => !value); }
      else if (matchesKey("t")) { event.preventDefault(); setShowTrade((value) => !value); }
      else if (matchesKey("u")) { event.preventDefault(); setShowBuffs((value) => !value); }
      else if (matchesKey("n")) { event.preventDefault(); setShowWorldMap((value) => !value); }
      else if (matchesKey("j")) { event.preventDefault(); setShowHelp((value) => !value); }
      else if (matchesKey("y")) { event.preventDefault(); setShowHotkeys((value) => !value); }
      else if (matchesKey("c")) { event.preventDefault(); setShowChatSettings((value) => !value); }
      else if (matchesKey("l")) { event.preventDefault(); setShowMail((value) => !value); }
      else if (matchesKey("d")) { event.preventDefault(); downloadSnapshot("manual"); }
    };
    window.addEventListener("keydown", onExtraWindowHotkey);
    return () => window.removeEventListener("keydown", onExtraWindowHotkey);
  }, []);
  // Auto-start the beginner tutorial the first time a player enters the world.
  // Persisted in localStorage so it only runs once; reopenable later via Alt+J's
  // help flow / a future menu entry. Net-new (no Crystal equivalent).
  useEffect(() => {
    if (screen !== "game") return;
    let alreadySeen = false;
    try {
      alreadySeen = window.localStorage.getItem("mir2:tutorialCompleted") === "1";
    } catch {
      alreadySeen = false;
    }
    if (!alreadySeen) setShowTutorial(true);
  }, [screen]);
  // When the hero/pet window opens, ask the server to start streaming intelligent
  // creature updates (ClientPacket::RequestIntelligentCreatureUpdates { update }).
  useEffect(() => {
    if (!showHeroPet) return;
    send({ type: "requestIntelligentCreatureUpdates", update: true }, { quiet: true });
    return () => {
      send({ type: "requestIntelligentCreatureUpdates", update: false }, { quiet: true });
    };
    // `send` is a stable hoisted closure over refs; intentionally excluded.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [showHeroPet]);
  const [activeInventoryTab, setActiveInventoryTab] = useState<"bag1" | "bag2" | "quest">("bag1");
  const [activeCharacterTab, setActiveCharacterTab] = useState<"char" | "stats1" | "stats2" | "spells">("char");
  const [storageServiceOpenVersion, setStorageServiceOpenVersion] = useState(0);
  const [predictedPlayerPosition, setPredictedPlayerPosition] = useState<PredictedPlayerMotion | null>(null);
  const [initialSceneAssetsReady, setInitialSceneAssetsReady] = useState(false);
  const [isClientReady, setIsClientReady] = useState(false);
  const t = buildTranslator(language);
  const locale = languageLocale(language);

  const loggedSceneResourceErrorsRef = useRef(new Set<string>());

  useEffect(() => {
    if (typeof window === "undefined") return;
    markMir2CacheMilestone("htmlReady", {
      href: window.location.href,
    });
    setLanguage(normalizeLanguage(window.localStorage.getItem("mir2-language")));
    setIsClientReady(true);
  }, []);

  useEffect(() => {
    accountIdRef.current = accountId;
    passwordRef.current = password;
    charactersRef.current = characters;
    selectedCharacterIndexRef.current = selectedCharacterIndex;
  }, [accountId, characters, password, selectedCharacterIndex]);

  useEffect(() => {
    if (typeof window === "undefined") return;
    window.localStorage.setItem("mir2-language", language);
    setCharacters((current) =>
      current.length === 1 && isFallbackCharacter(current[0]) ? [fallbackCharacter(language)] : current,
    );
  }, [language]);

  useEffect(() => {
    if (typeof window === "undefined") return;
    const refreshWallets = () => setSuiWallets(getSuiWalletSummaries());
    refreshWallets();
    const delayedRefresh = window.setTimeout(refreshWallets, 500);
    const unsubscribe = subscribeToSuiWalletChanges(refreshWallets);
    return () => {
      window.clearTimeout(delayedRefresh);
      unsubscribe();
    };
  }, []);

  const self = world.entities.find((entity) => entity.objectId === world.playerObjectId) ?? null;
  const selfPredictionAnchor = self ? authoritativeSelfForPrediction(self) : null;
  const localSelfRenderPosition = preserveCrystalSelfRenderPosition(
    selfPredictionAnchor,
    chooseCrystalSelfRenderPosition(
      selfPredictionAnchor,
      renderableSelfPrediction(selfPredictionAnchor, predictedPlayerPosition),
    ),
  );
  const activePredictedPlayerPosition =
    self &&
    localSelfRenderPosition &&
    self.x === localSelfRenderPosition.x &&
    self.y === localSelfRenderPosition.y &&
    (!localSelfRenderPosition.direction || self.direction === localSelfRenderPosition.direction) &&
    !hasSelfMovementTransportEvidence()
      ? null
      : localSelfRenderPosition;
  const predictedSelf = useMemo(
    () =>
      self &&
      activePredictedPlayerPosition &&
      selfPredictionAnchor &&
      Math.max(
        Math.abs(selfPredictionAnchor.x - activePredictedPlayerPosition.x),
        Math.abs(selfPredictionAnchor.y - activePredictedPlayerPosition.y),
      ) <= MOVEMENT_LOCAL_RENDER_MAX_LEAD_TILES
        ? {
            ...self,
            x: activePredictedPlayerPosition.x,
            y: activePredictedPlayerPosition.y,
            direction: activePredictedPlayerPosition.direction ?? self.direction,
            ...localSelfMovementAnimationForPosition(activePredictedPlayerPosition),
          }
        : self,
    [self, selfPredictionAnchor, activePredictedPlayerPosition],
  );
  const displayEntities = useMemo(() => {
    if (!predictedSelf || !self || entityMovementRenderFieldsMatch(predictedSelf, self)) {
      return world.entities;
    }

    return world.entities.map((entity) =>
      entity.objectId === world.playerObjectId
        ? { ...entity, ...predictedSelf }
        : entity,
    );
  }, [predictedSelf, self, world.entities, world.playerObjectId]);
  const selectedEntity =
    displayEntities.find((entity) => entity.objectId === world.selectedObjectId) ?? null;

  useEffect(() => {
    predictedPlayerPositionRef.current = predictedPlayerPosition;
  }, [predictedPlayerPosition]);

  useEffect(() => {
    screenRef.current = screen;
    if (screen !== "game") {
      firstPlayableFrameMarkedRef.current = false;
      initialSceneAssetsReadyRef.current = false;
      sceneAssetReadinessRef.current = null;
      lastSceneAssetMilestoneKeyRef.current = null;
      setInitialSceneAssetsReady(false);
    }
  }, [screen]);

  function setInitialSceneAssetsReadyState(ready: boolean) {
    initialSceneAssetsReadyRef.current = ready;
    setInitialSceneAssetsReady(ready);
  }

  function handleSceneAssetReadinessChange(readiness: SceneAssetReadiness) {
    sceneAssetReadinessRef.current = readiness;
    const milestoneKey = `${readiness.key}:${readiness.status}:${readiness.loaded}:${readiness.failed}`;
    if (lastSceneAssetMilestoneKeyRef.current !== milestoneKey) {
      lastSceneAssetMilestoneKeyRef.current = milestoneKey;
      markMir2CacheMilestone(readiness.ready ? "sceneAssetsReady" : "sceneAssetsStart", {
        key: readiness.key,
        status: readiness.status,
        total: readiness.total,
        loaded: readiness.loaded,
        failed: readiness.failed,
        pending: readiness.pending,
        interactionReady: readiness.interactionReady ?? readiness.ready,
        visualReady: readiness.visualReady ?? readiness.ready,
        durationMs: readiness.durationMs,
        failedUrls: readiness.failedUrls.slice(0, 5),
      });
    }

    if (screenRef.current !== "game" || firstPlayableFrameMarkedRef.current) {
      return;
    }

    const nextSceneInteractionReady = readiness.interactionReady ?? readiness.ready;
    if (nextSceneInteractionReady && !initialSceneAssetsReadyRef.current) {
      setInitialSceneAssetsReadyState(true);
    }
  }

  function handleBevyEntityRenderStateChange(state: BevyEntityRenderState) {
    const runtime = runtimeRef.current;
    if (!runtime) return;

    for (const atlas of state.atlasImages ?? []) {
      if (!atlas.pixels) {
        continue;
      }
      if (uploadedBevyEntityAtlasKeysRef.current.has(atlas.key)) {
        continue;
      }
      runtime.setMir2EntityRenderAtlas?.(atlas.key, atlas.width, atlas.height, atlas.pixels);
      uploadedBevyEntityAtlasKeysRef.current.add(atlas.key);
    }

    const { atlasImages: _atlasImages, ...serializableState } = state;
    const serializableStateJson = JSON.stringify(serializableState);
    if (serializableStateJson === lastBevyEntityRenderStateJsonRef.current) {
      return;
    }
    lastBevyEntityRenderStateJsonRef.current = serializableStateJson;
    runtime.setMir2EntityRenderState?.(serializableStateJson);
  }

  function sceneInputDeferredForInitialAssets() {
    if (
      screenRef.current !== "game" ||
      firstPlayableFrameMarkedRef.current ||
      initialSceneAssetsReadyRef.current
    ) {
      return false;
    }

    const now = Date.now();
    if (now - lastDeferredSceneInputAtRef.current > 600) {
      lastDeferredSceneInputAtRef.current = now;
      markMir2CacheMilestone("sceneInputDeferred", {
        readiness: sceneAssetReadinessRef.current,
      });
    }
    return true;
  }

  function setPredictedPlayerMotion(position: PredictedPlayerMotion | null, holdUntil = 0) {
    if (position) {
      if (holdUntil > 0) {
        predictedPlayerHoldUntilRef.current = Math.max(predictedPlayerHoldUntilRef.current, holdUntil);
      } else if (predictedPlayerHoldUntilRef.current < Date.now()) {
        predictedPlayerHoldUntilRef.current = 0;
      }
    } else {
      if (predictedPlayerPositionRef.current && predictedPlayerHoldUntilRef.current > Date.now()) {
        return;
      }
      predictedPlayerHoldUntilRef.current = 0;
    }
    predictedPlayerPositionRef.current = position;
    const updateSeq = ++predictedPlayerUpdateSeqRef.current;
    queueMicrotask(() => {
      if (predictedPlayerUpdateSeqRef.current !== updateSeq) {
        return;
      }
      flushSync(() => {
        setPredictedPlayerPosition(predictedPlayerPositionRef.current);
      });
    });
  }

  useEffect(() => {
    const params = new URLSearchParams(window.location.search);
    const enabled =
      params.get("movementDiag") === "1" ||
      params.get("moveDiag") === "1" ||
      params.get("movementDiagnostics") === "1";
    if (!enabled) {
      return;
    }

    const startedAt = Date.now();
    const sessionId = `manual-${startedAt.toString(36)}-${Math.random().toString(36).slice(2, 8)}`;
    movementDiagnosticsRef.current = {
      enabled: true,
      sessionId,
      startedAt,
      events: [],
      pendingFlush: false,
      lastSample: null,
      lastMovementCommand: null,
    };

    const diagnosticWindow = window as typeof window & {
      __mir2MovementDiagnostics?: Record<string, unknown>;
    };
    diagnosticWindow.__mir2MovementDiagnostics = {
      sessionId,
      startedAt,
      flush: () => flushMovementDiagnostics("manual"),
      latestFile: "/api/movement-diagnostics",
    };

    appendLog(`Movement diagnostics enabled: ${sessionId}`, "system");
    recordMovementDiagnostic("diag:start", {
      url: window.location.href,
      userAgent: navigator.userAgent,
    });

    const sampleTimer = window.setInterval(() => {
      const sample = captureMovementDiagnosticSample();
      const anomalies = movementDiagnosticAnomalies(sample, movementDiagnosticsRef.current?.lastSample ?? null);
      recordMovementDiagnostic("sample", sample, anomalies);
      if (anomalies.length > 0) {
        recordMovementDiagnostic("anomaly", { sample, anomalies });
      }
      if (movementDiagnosticsRef.current) {
        movementDiagnosticsRef.current.lastSample = sample;
      }
    }, 100);
    const flushTimer = window.setInterval(() => flushMovementDiagnostics("interval"), 2500);
    const handlePageHide = () => flushMovementDiagnostics("pagehide", true);
    window.addEventListener("pagehide", handlePageHide);

    return () => {
      window.clearInterval(sampleTimer);
      window.clearInterval(flushTimer);
      window.removeEventListener("pagehide", handlePageHide);
      recordMovementDiagnostic("diag:stop", { url: window.location.href });
      flushMovementDiagnostics("unmount", true);
      delete diagnosticWindow.__mir2MovementDiagnostics;
    };
  }, []);

  function recordMovementDiagnostic(
    type: string,
    payload?: unknown,
    anomalies: string[] = [],
    at = Date.now(),
  ) {
    const state = movementDiagnosticsRef.current;
    if (!state?.enabled) {
      return;
    }

    const event: Record<string, unknown> = { type, at };
    if (payload !== undefined) {
      event.payload = payload;
    }
    if (anomalies.length > 0) {
      event.anomalies = anomalies;
    }

    if (type === "tx:movementCommand" && payload && typeof payload === "object") {
      const command = (payload as { command?: Record<string, unknown> }).command;
      if (command) {
        state.lastMovementCommand = {
          type: command.type,
          direction: command.direction,
          x: command.x,
          y: command.y,
          at,
        };
      }
    }

    state.events.push(event);
    if (state.events.length > 1200) {
      state.events.splice(0, state.events.length - 1200);
    }

    const diagnosticWindow = window as typeof window & {
      __mir2MovementDiagnostics?: Record<string, unknown>;
    };
    if (diagnosticWindow.__mir2MovementDiagnostics) {
      diagnosticWindow.__mir2MovementDiagnostics.pendingEvents = state.events.length;
      diagnosticWindow.__mir2MovementDiagnostics.lastEvent = event;
    }

    if (state.events.length >= 300) {
      flushMovementDiagnostics("buffer");
    }
  }

  function captureMovementConsoleState(now = Date.now()) {
    const currentWorld = worldRef.current;
    const currentSelf = currentWorld.entities.find((entity) => entity.objectId === currentWorld.playerObjectId) ?? null;
    const queuedDirectionStep = queuedDirectionStepRef.current;
    const directionStepPending = directionStepPendingRef.current;
    const movementPlan = movementPlanRef.current;
    const debugWindow = window as MovementConsoleWindow;

    return {
      screen: screenRef.current,
      wsState,
      self: currentSelf
        ? { x: currentSelf.x, y: currentSelf.y, direction: currentSelf.direction }
        : null,
      predicted: predictedPlayerPositionRef.current,
      pendingSelfMove: pendingSelfMoveRef.current,
      queuedMoveIntent: queuedMoveIntentRef.current,
      nextMoveWaitMs: Math.max(0, nextMoveSendAtRef.current - now),
      movementPlan: movementPlan
        ? {
            mode: movementPlan.mode,
            packetMode: movementPlan.packetMode ?? "direction",
            targetX: movementPlan.targetX,
            targetY: movementPlan.targetY,
            pendingX: movementPlan.pendingX ?? null,
            pendingY: movementPlan.pendingY ?? null,
            waitMs: Math.max(0, movementPlan.nextStepAt - now),
            pendingAgeMs: movementPlan.pendingSentAt ? Math.max(0, now - movementPlan.pendingSentAt) : null,
          }
        : null,
      queuedDirectionStep: queuedDirectionStep
        ? {
            direction: queuedDirectionStep.direction ?? null,
            mode: queuedDirectionStep.mode,
            requestedAgeMs: Math.max(0, now - queuedDirectionStep.requestedAt),
            repeatCount: queuedDirectionStep.repeatCount ?? 1,
          }
        : null,
      directionStepPending: directionStepPending
        ? {
            direction: directionStepPending.direction ?? null,
            mode: directionStepPending.mode,
            sentAgeMs: Math.max(0, now - directionStepPending.sentAt),
          }
        : null,
      directionStepPendingQueueLength: directionStepPendingQueueRef.current.length,
      crystalSelfActionFeedLength: crystalSelfActionFeedRef.current.length,
      outstandingSelfMovementActionsLength: outstandingSelfMovementActionsRef.current.length,
      pendingConsoleCommandCount: debugWindow.__mir2PendingMovementConsoleCommands?.length ?? 0,
    };
  }

  function flushMovementDiagnostics(reason = "manual", useBeacon = false) {
    const state = movementDiagnosticsRef.current;
    if (!state?.enabled || state.pendingFlush || state.events.length === 0) {
      return;
    }

    const events = state.events.splice(0, state.events.length);
    const body = JSON.stringify({
      sessionId: state.sessionId,
      startedAt: state.startedAt,
      reason,
      pageUrl: window.location.href,
      userAgent: navigator.userAgent,
      events,
    });

    if (useBeacon && navigator.sendBeacon) {
      const ok = navigator.sendBeacon(
        "/api/movement-diagnostics",
        new Blob([body], { type: "application/json" }),
      );
      if (ok) {
        return;
      }
    }

    state.pendingFlush = true;
    fetch("/api/movement-diagnostics", {
      method: "POST",
      headers: { "content-type": "application/json" },
      body,
      keepalive: useBeacon,
    })
      .catch(() => {
        state.events = [...events, ...state.events].slice(-1200);
      })
      .finally(() => {
        state.pendingFlush = false;
      });
  }

  function captureMovementDiagnosticSample(now = Date.now()): MovementDiagnosticSample {
    const currentWorld = worldRef.current;
    const currentSelf =
      currentWorld.entities.find((entity) => entity.objectId === currentWorld.playerObjectId) ?? null;
    const debugWindow = window as typeof window & {
      __mir2SceneMotionDebug?: {
        motionNow?: number;
        renderPlayer?: (MovementDiagnosticPoint & {
          movementAnimation?: string;
          movementStartedAt?: number;
          movementUntil?: number;
        }) | null;
        playerCameraMotionOffset?: unknown;
        playerMotionSnapshot?: unknown;
      };
    };
    const sceneDebug = debugWindow.__mir2SceneMotionDebug;
    const renderPlayer = sceneDebug?.renderPlayer ?? null;

    return {
      screen: screenRef.current,
      mapFileName: currentWorld.mapFileName,
      mapTitle: currentWorld.mapTitle,
      worldTick: currentWorld.worldTick,
      worldSnapshotVersion: worldSnapshotVersionRef.current,
      worldSnapshotRealtimeMode: packetRuntimeSnapshotModeRef.current,
      self: currentSelf
        ? {
            x: currentSelf.x,
            y: currentSelf.y,
            direction: currentSelf.direction,
            movementAnimation: currentSelf.movementAnimation,
            movementStartedAt: currentSelf.movementStartedAt,
            movementUntil: currentSelf.movementUntil,
          }
        : null,
      render: renderPlayer
        ? {
            x: renderPlayer.x,
            y: renderPlayer.y,
            direction: renderPlayer.direction,
          }
        : null,
      predicted: predictedPlayerPositionRef.current,
      scene: sceneDebug
        ? {
            motionNow: sceneDebug.motionNow,
            playerCameraMotionOffset: sceneDebug.playerCameraMotionOffset,
            playerMotionSnapshot: sceneDebug.playerMotionSnapshot,
          }
        : null,
      queues: {
        movementPlan: movementPlanRef.current,
        pendingSelfMove: pendingSelfMoveRef.current,
        queuedMoveIntent: queuedMoveIntentRef.current,
        nextMoveWaitMs: Math.max(0, nextMoveSendAtRef.current - now),
        queuedDirectionStep: queuedDirectionStepRef.current,
        directionStepPending: directionStepPendingRef.current,
        directionStepPendingQueueLength: directionStepPendingQueueRef.current.length,
        crystalSelfActionFeedLength: crystalSelfActionFeedRef.current.length,
        outstandingSelfMovementActionsLength: outstandingSelfMovementActionsRef.current.length,
        movementInputBlockedForMs: Math.max(0, movementInputBlockedUntilRef.current - now),
      },
      transport: {
        lastMovementCommand: movementDiagnosticsRef.current?.lastMovementCommand ?? null,
        lastSelfMovementAck: lastSelfMovementAckRef.current,
        lastSelfNoProgressAck: lastSelfNoProgressAckRef.current,
      },
    };
  }

  function prunePacketRuntimeObjectTombstones(now = Date.now()) {
    const tombstones = packetRuntimeObjectTombstonesRef.current;
    for (const [objectId, removedAt] of tombstones) {
      if (now - removedAt > PACKET_RUNTIME_SNAPSHOT_TOMBSTONE_MS) {
        tombstones.delete(objectId);
      }
    }
  }

  function rememberPacketRuntimeObjectRemoved(objectId: string, now = Date.now()) {
    if (!objectId || objectId === "0") return;
    prunePacketRuntimeObjectTombstones(now);
    packetRuntimeObjectTombstonesRef.current.set(objectId, now);
  }

  function clearPacketRuntimeObjectTombstone(objectId: string) {
    if (!objectId || objectId === "0") return;
    packetRuntimeObjectTombstonesRef.current.delete(objectId);
  }

  function isPacketRuntimeObjectTombstoned(objectId: string, now = Date.now()) {
    prunePacketRuntimeObjectTombstones(now);
    const removedAt = packetRuntimeObjectTombstonesRef.current.get(objectId);
    return typeof removedAt === "number" && now - removedAt <= PACKET_RUNTIME_SNAPSHOT_TOMBSTONE_MS;
  }

  function classifyWorldSnapshotRealtimeMode(
    snapshot: GatewayWorldSnapshot,
    current: WorldState,
    playerObjectId: string | null,
    snapshotMapChanged: boolean,
  ): WorldSnapshotRealtimeMode {
    if (reconnectSnapshotRef.current || reconnectStatusRef.current.mode !== "idle") {
      return "reconnect";
    }
    if (snapshotMapChanged) {
      return "mapChange";
    }
    if (screenRef.current !== "game" || !playerObjectId || !current.playerObjectId) {
      return "bootstrap";
    }
    const currentSelf = current.entities.find((entity) => entity.objectId === playerObjectId);
    if (!currentSelf) {
      return "bootstrap";
    }
    const snapshotMapFileName = snapshot.mapFileName ?? current.mapFileName;
    const hasLoadedScene =
      current.sceneView !== null &&
      current.originalMapRegion !== null &&
      normalizeMapFileName(current.originalMapRegion.mapFileName) === normalizeMapFileName(snapshotMapFileName);
    if (!hasLoadedScene || current.entities.length <= 1) {
      return "sceneBootstrap";
    }
    return "packetRefresh";
  }

  function mergeSnapshotEntityIntoPacketRuntime(
    currentEntity: WorldEntity,
    snapshotEntity: WorldEntity,
    now: number,
  ): WorldEntity {
    const movementActive =
      currentEntity.movementAnimation &&
      typeof currentEntity.movementUntil === "number" &&
      currentEntity.movementUntil > now;
    const attackActive =
      currentEntity.attackAnimation &&
      typeof currentEntity.attackUntil === "number" &&
      currentEntity.attackUntil > now;
    const struckActive =
      typeof currentEntity.struckUntil === "number" && currentEntity.struckUntil > now;
    const dieActive = typeof currentEntity.dieUntil === "number" && currentEntity.dieUntil > now;
    const reviveActive = typeof currentEntity.reviveUntil === "number" && currentEntity.reviveUntil > now;

    return {
      ...snapshotEntity,
      movementAnimation: movementActive ? currentEntity.movementAnimation : undefined,
      movementStartedAt: movementActive ? currentEntity.movementStartedAt : undefined,
      movementUntil: movementActive ? currentEntity.movementUntil : undefined,
      attackAnimation: attackActive ? currentEntity.attackAnimation : undefined,
      attackStartedAt: attackActive ? currentEntity.attackStartedAt : undefined,
      attackUntil: attackActive ? currentEntity.attackUntil : undefined,
      struckStartedAt: struckActive ? currentEntity.struckStartedAt : undefined,
      struckUntil: struckActive ? currentEntity.struckUntil : undefined,
      dieStartedAt: dieActive ? currentEntity.dieStartedAt : undefined,
      dieUntil: dieActive ? currentEntity.dieUntil : undefined,
      reviveStartedAt: reviveActive ? currentEntity.reviveStartedAt : undefined,
      reviveUntil: reviveActive ? currentEntity.reviveUntil : undefined,
    };
  }

  function mergePacketFirstSnapshotEntities(
    currentEntities: WorldEntity[],
    snapshotEntities: WorldEntity[],
    now: number,
  ) {
    const snapshotByObjectId = new Map(snapshotEntities.map((entity) => [entity.objectId, entity]));
    const currentObjectIds = new Set(currentEntities.map((entity) => entity.objectId));
    const mergedEntities = currentEntities.map((entity) => {
      const snapshotEntity = snapshotByObjectId.get(entity.objectId);
      return snapshotEntity ? mergeSnapshotEntityIntoPacketRuntime(entity, snapshotEntity, now) : entity;
    });

    for (const snapshotEntity of snapshotEntities) {
      if (currentObjectIds.has(snapshotEntity.objectId)) continue;
      if (isPacketRuntimeObjectTombstoned(snapshotEntity.objectId, now)) continue;
      mergedEntities.push(snapshotEntity);
    }

    return mergedEntities;
  }

  function mergePacketFirstSnapshotGroundDrops(
    currentDrops: GroundDrop[],
    snapshotDrops: GroundDrop[],
    now: number,
  ) {
    const snapshotByObjectId = new Map(snapshotDrops.map((drop) => [drop.objectId, drop]));
    const currentObjectIds = new Set(currentDrops.map((drop) => drop.objectId));
    const mergedDrops = currentDrops.map((drop) => {
      const snapshotDrop = snapshotByObjectId.get(drop.objectId);
      return snapshotDrop
        ? {
            ...snapshotDrop,
            x: drop.x,
            y: drop.y,
            quantity: drop.quantity,
          }
        : drop;
    });

    for (const snapshotDrop of snapshotDrops) {
      if (currentObjectIds.has(snapshotDrop.objectId)) continue;
      if (isPacketRuntimeObjectTombstoned(snapshotDrop.objectId, now)) continue;
      mergedDrops.push(snapshotDrop);
    }

    return mergedDrops;
  }

  function movementDiagnosticAnomalies(
    sample: MovementDiagnosticSample,
    previous: MovementDiagnosticSample | null,
  ) {
    const anomalies: string[] = [];
    const lastCommand = sample.transport.lastMovementCommand;
    const commandDirection =
      typeof lastCommand?.direction === "string" ? lastCommand.direction : undefined;
    const commandAt = typeof lastCommand?.at === "number" ? lastCommand.at : 0;
    const commandVector = directionVector(commandDirection);
    if (previous && commandVector && Date.now() - commandAt <= MOVEMENT_SERVER_CORRECTION_GRACE_MS) {
      const selfDelta = movementDiagnosticDelta(previous.self, sample.self);
      if (selfDelta && selfDelta.x * commandVector.x + selfDelta.y * commandVector.y < 0) {
        anomalies.push("worldTileMovedAgainstLastCommand");
      }

      const renderDelta = movementDiagnosticDelta(previous.render, sample.render);
      if (renderDelta && renderDelta.x * commandVector.x + renderDelta.y * commandVector.y < 0) {
        anomalies.push("renderTileMovedAgainstLastCommand");
      }
    }

    if (sample.self && sample.render) {
      const renderLead = Math.max(Math.abs(sample.render.x - sample.self.x), Math.abs(sample.render.y - sample.self.y));
      if (renderLead > MOVEMENT_LOCAL_RENDER_MAX_LEAD_TILES) {
        anomalies.push("renderLeadTooLarge");
      }
    }

    const cameraOffset = movementDiagnosticCameraOffset(sample);
    const previousCameraOffset = previous ? movementDiagnosticCameraOffset(previous) : null;
    if (
      cameraOffset &&
      previousCameraOffset &&
      Math.max(Math.abs(cameraOffset.x - previousCameraOffset.x), Math.abs(cameraOffset.y - previousCameraOffset.y)) >
        Math.max(VIEWPORT_CELL_WIDTH, VIEWPORT_CELL_HEIGHT)
    ) {
      anomalies.push("cameraOffsetJump");
    }

    if (
      sample.queues.directionStepPendingQueueLength > MOVEMENT_DIRECTION_PENDING_MAX ||
      sample.queues.outstandingSelfMovementActionsLength > MOVEMENT_LOCAL_ACTION_MAX_LEAD_TILES + 1
    ) {
      anomalies.push("movementQueueBacklog");
    }

    return anomalies;
  }

  function movementDiagnosticDelta(
    previous: MovementDiagnosticPoint | null,
    current: MovementDiagnosticPoint | null,
  ) {
    if (!previous || !current) {
      return null;
    }
    const x = current.x - previous.x;
    const y = current.y - previous.y;
    return x === 0 && y === 0 ? null : { x, y };
  }

  function movementDiagnosticCameraOffset(sample: MovementDiagnosticSample) {
    const offset = sample.scene?.playerCameraMotionOffset;
    if (!offset || typeof offset !== "object") {
      return null;
    }
    const x = (offset as { x?: unknown }).x;
    const y = (offset as { y?: unknown }).y;
    return typeof x === "number" && typeof y === "number" ? { x, y } : null;
  }

  function clearLocalSelfPrediction() {
    pendingSelfTurnRef.current = null;
    predictedPlayerHoldUntilRef.current = 0;
    localMovementAnchorRef.current = null;
    lastCrystalSelfRenderPositionRef.current = null;
    setPredictedPlayerMotion(null);
  }

  function clearLegacySelfMovementCoordinateSources() {
    movementPlanRef.current = null;
    queuedDirectionStepRef.current = null;
    queuedDirectionStepBacklogRef.current = [];
    directionStepPendingRef.current = null;
    pendingSelfTurnRef.current = null;
    clearDirectionStepPendingQueue();
    clearCrystalSelfActionFeed();
    clearOutstandingSelfMovementActions();
    clearLocalMovementAnchor();
    lastCrystalSelfRenderPositionRef.current = null;
  }

  function readSelfMovementControllerState(): MovementControllerState {
    return {
      pending: pendingSelfMoveRef.current,
      prediction: predictedPlayerPositionRef.current,
      nextMoveSendAt: nextMoveSendAtRef.current,
      runPrimedUntil: crystalRunPrimedUntilRef.current,
      inputBlockedUntil: movementInputBlockedUntilRef.current,
    };
  }

  function applySelfMovementControllerState(state: MovementControllerState) {
    pendingSelfMoveRef.current = state.pending;
    nextMoveSendAtRef.current = state.nextMoveSendAt;
    crystalRunPrimedUntilRef.current = state.runPrimedUntil;
    movementInputBlockedUntilRef.current = state.inputBlockedUntil;
    if (state.prediction) {
      setPredictedPlayerMotion(state.prediction, state.pending?.visualUntil ?? Date.now() + CRYSTAL_ENTITY_MOVE_ACTION_MS);
    } else {
      clearLocalSelfPrediction();
    }
  }

  function currentAuthoritativeSelf(currentWorld = worldRef.current) {
    return currentWorld.entities.find((entity) => entity.objectId === currentWorld.playerObjectId) ?? null;
  }

  function classifySelfMovementAckDisposition(
    point: { x: number; y: number; direction?: string },
    packetName: string,
  ): CrystalSelfAckDisposition {
    const pending = pendingSelfMoveRef.current;
    if (!pending) {
      return "none";
    }
    if (packetName === "UserDashFail") {
      return "correction";
    }
    return movementPointMatches(point, pending.to) ? "confirmed" : "correction";
  }

  function reconcileSelfMovementAck(
    point: { x: number; y: number; direction?: string },
    packetName: string,
    now: number,
  ) {
    const hadPending = pendingSelfMoveRef.current !== null;
    const result = reconcileMovementAck({
      state: readSelfMovementControllerState(),
      ack: point,
      packetName,
      now,
    });
    applySelfMovementControllerState(result.state);
    lastSelfMovementAckRef.current = { x: point.x, y: point.y, direction: point.direction, at: now };
    if (result.outcome === "correction") {
      queuedMoveIntentRef.current = null;
      crystalRunPrimedUntilRef.current = 0;
      movementPredictionBlockedUntilRef.current = Math.max(
        movementPredictionBlockedUntilRef.current,
        now + CRYSTAL_CORRECTION_BLOCK_MS,
      );
      clearLegacySelfMovementCoordinateSources();
      clearLocalSelfPrediction();
      recordMovementConsoleEvent("correction", {
        packet: packetName,
        point,
        pending: hadPending,
        state: captureMovementConsoleState(now),
      });
      return result.outcome;
    }
    if (result.outcome === "confirmed") {
      crystalRunPrimedUntilRef.current = now + CRYSTAL_RUN_PRIME_MS;
      clearLegacySelfMovementCoordinateSources();
      clearLocalSelfPrediction();
      scheduleMovementConfirmTick();
      return result.outcome;
    }
    if (!hadPending) {
      clearLegacySelfMovementCoordinateSources();
      clearLocalSelfPrediction();
    }
    return result.outcome;
  }

  function reconcileSelfMovementSnapshot(
    point: { x: number; y: number; direction?: string },
    now: number,
  ) {
    const result = reconcileMovementSnapshot({
      state: readSelfMovementControllerState(),
      snapshot: point,
      now,
    });
    if (!result.corrected) {
      return false;
    }
    applySelfMovementControllerState(result.state);
    queuedMoveIntentRef.current = null;
    clearLegacySelfMovementCoordinateSources();
    clearLocalSelfPrediction();
    return true;
  }

  function rememberLocalMovementAnchor(position: PredictedPlayerMotion, now: number, holdUntil = 0) {
    const requestedUntil = holdUntil > 0 ? holdUntil : now + MOVEMENT_PENDING_ACTION_MAX_AGE_MS;
    const nextUntil = Math.max(
      localMovementAnchorRef.current?.until ?? 0,
      requestedUntil,
    );
    const current = localMovementAnchorRef.current;
    if (
      current &&
      now <= current.until &&
      current.direction === position.direction &&
      predictedPlayerAheadOfServer(position, current, current.direction ?? position.direction)
    ) {
      localMovementAnchorRef.current = {
        ...current,
        until: nextUntil,
      };
      return;
    }
    localMovementAnchorRef.current = {
      ...position,
      until: nextUntil,
    };
  }

  function clearLocalMovementAnchor() {
    localMovementAnchorRef.current = null;
  }

  function pruneCrystalSelfActionFeed(now: number) {
    const keepAfter = now - MOVEMENT_PENDING_ACTION_MAX_AGE_MS;
    crystalSelfActionFeedRef.current = crystalSelfActionFeedRef.current.filter(
      (entry) => entry.sentAt >= keepAfter,
    );
    pruneSettledOutstandingSelfMovementActions(now, keepAfter);
    recentSelfMovementActionHistoryRef.current = recentSelfMovementActionHistoryRef.current.filter(
      (entry) => entry.sentAt >= keepAfter,
    );
  }

  function pruneSettledOutstandingSelfMovementActions(now: number, keepAfter = now - MOVEMENT_PENDING_ACTION_MAX_AGE_MS) {
    const ack = lastSelfMovementAckRef.current;
    const movementIdle =
      !movementPlanRef.current &&
      !directionStepPendingRef.current &&
      directionStepPendingQueueRef.current.length === 0 &&
      !predictedPlayerPositionRef.current;
    outstandingSelfMovementActionsRef.current = outstandingSelfMovementActionsRef.current.filter((entry) => {
      if (entry.sentAt < keepAfter) {
        return false;
      }
      if (now < entry.visualUntil + MOVEMENT_OUTSTANDING_ACTION_SETTLE_GRACE_MS) {
        return true;
      }
      if (
        ack &&
        crystalActionMatchesAck(entry, ack.x, ack.y, ack.direction)
      ) {
        return false;
      }
      return !movementIdle;
    });
  }

  function pruneLocallySettledDirectionStepPending(now: number) {
    const queue = directionStepPendingQueueRef.current;
    if (queue.length === 0) {
      return false;
    }

    let settledThrough = -1;
    for (let index = queue.length - 1; index >= 0; index -= 1) {
      if (directionStepPendingLocallySettled(queue[index], now)) {
        settledThrough = index;
        break;
      }
    }
    if (settledThrough < 0) {
      return false;
    }

    setDirectionStepPendingQueue(queue.slice(settledThrough + 1));
    return true;
  }

  function directionStepPendingLocallySettled(pending: DirectionStepPending, now: number) {
    if (
      !pending.direction ||
      now < pending.sentAt + movementStepIntervalMs(pending.mode) + MOVEMENT_CONFIRM_TICK_DELAY_MS
    ) {
      return false;
    }

    const currentWorld = worldRef.current;
    const serverSelf = currentWorld.entities.find((entity) => entity.objectId === currentWorld.playerObjectId) ?? null;
    const authoritativeSelf = authoritativeSelfForMovementSettlement(serverSelf, now);
    const candidates = [
      authoritativeSelf
        ? { x: authoritativeSelf.x, y: authoritativeSelf.y, direction: authoritativeSelf.direction }
        : null,
    ];

    return candidates.some(
      (candidate) =>
        candidate &&
        (!candidate.direction || candidate.direction === pending.direction) &&
        directionStepReachedOrPassed(pending, candidate.x, candidate.y),
    );
  }

  function rememberCrystalSelfAction(
    source: { x: number; y: number },
    action: { point: { x: number; y: number }; direction: string; mode: "walk" | "run" },
    now: number,
  ) {
    if (action.point.x === source.x && action.point.y === source.y) {
      return;
    }
    pruneCrystalSelfActionFeed(now);
    const entry: CrystalSelfActionFeedEntry = {
      fromX: source.x,
      fromY: source.y,
      x: action.point.x,
      y: action.point.y,
      direction: action.direction,
      mode: action.mode,
      sentAt: now,
      visualUntil: now + movementStepIntervalMs(action.mode),
    };
    crystalSelfActionFeedRef.current = [...crystalSelfActionFeedRef.current, entry].slice(-8);
    outstandingSelfMovementActionsRef.current = [
      ...outstandingSelfMovementActionsRef.current,
      entry,
    ].slice(-16);
    recentSelfMovementActionHistoryRef.current = [
      ...recentSelfMovementActionHistoryRef.current,
      entry,
    ].slice(-32);
  }

  function clearCrystalSelfActionFeed() {
    crystalSelfActionFeedRef.current = [];
  }

  function clearOutstandingSelfMovementActions() {
    outstandingSelfMovementActionsRef.current = [];
  }

  function clearRecentSelfMovementActionHistory() {
    recentSelfMovementActionHistoryRef.current = [];
  }

  function selfLocalMovementActive() {
    pruneCrystalSelfActionFeed(Date.now());
    return (
      hasSelfMovementTransportEvidence() ||
      crystalSelfActionFeedRef.current.length > 0 ||
      Boolean(predictedPlayerPositionRef.current)
    );
  }

  function hasSelfMovementTransportEvidence(now = Date.now()) {
    pruneCrystalSelfActionFeed(now);
    const anchor = localMovementAnchorRef.current;
    const pendingTurn = pendingSelfTurnRef.current;
    return (
      Boolean(pendingSelfMoveRef.current) ||
      Boolean(pendingTurn && now <= pendingTurn.visualUntil) ||
      Boolean(movementPlanRef.current) ||
      Boolean(directionStepPendingRef.current) ||
      directionStepPendingQueueRef.current.length > 0 ||
      outstandingSelfMovementActionsRef.current.length > 0 ||
      Boolean(anchor && now <= anchor.until)
    );
  }

  function hasSelfMovementAckInFlight() {
    pruneCrystalSelfActionFeed(Date.now());
    const plan = movementPlanRef.current;
    return (
      Boolean(
        pendingSelfMoveRef.current ||
        plan &&
          plan.pendingX !== undefined &&
          plan.pendingY !== undefined &&
          plan.pendingSentAt !== undefined,
      ) ||
      Boolean(directionStepPendingRef.current) ||
      directionStepPendingQueueRef.current.length > 0 ||
      outstandingSelfMovementActionsRef.current.length > 0
    );
  }

  function authoritativeSelfForPrediction(serverSelf: WorldEntity | null) {
    return serverSelf;
  }

  function authoritativeSelfForMovementSettlement(serverSelf: WorldEntity | null, now = Date.now()) {
    const ack = lastSelfMovementAckRef.current;
    if (ack) {
      return {
        x: ack.x,
        y: ack.y,
        direction: ack.direction ?? serverSelf?.direction,
      };
    }

    return hasSelfMovementTransportEvidence(now) ? null : serverSelf;
  }

  function rememberSnapshotSelfAck(
    entity: { x: number; y: number; direction?: string },
    now: number,
    force = false,
  ) {
    const confirmsLocalMovement = selfAckConfirmsLocalMovement(entity);
    if (force || confirmsLocalMovement || !lastSelfMovementAckRef.current || !selfLocalMovementActive()) {
      lastSelfMovementAckRef.current = {
        x: entity.x,
        y: entity.y,
        direction: entity.direction,
        at: now,
      };
    }
    if (confirmsLocalMovement) {
      lastSelfNoProgressAckRef.current = null;
      clearSettledSelfActionsAt(entity.x, entity.y, entity.direction);
      reconcileDirectionStepQueueWithServer(entity.x, entity.y, now, entity.direction, false, false);
    }
  }

  function activeCrystalSelfActionSource(
    serverSelf: { x: number; y: number; direction?: string } | null,
    now = Date.now(),
  ) {
    pruneCrystalSelfActionFeed(now);
    const feed = serverSelf
      ? crystalSelfActionFeedRef.current.filter((entry) =>
          crystalMovementCandidateNotBehindServer(serverSelf, entry, entry.direction),
        )
      : crystalSelfActionFeedRef.current;
    if (feed.length === 0) {
      return null;
    }
    const latest = feed[feed.length - 1];
    if (
      serverSelf &&
      !hasSelfMovementTransportEvidence(now) &&
      (latest.x !== serverSelf.x || latest.y !== serverSelf.y)
    ) {
      return null;
    }
    return {
      x: latest.x,
      y: latest.y,
      direction: latest.direction,
    };
  }

  function activeOutstandingSelfActionSource(
    serverSelf: { x: number; y: number; direction?: string } | null,
    now = Date.now(),
  ) {
    pruneCrystalSelfActionFeed(now);
    const actions = serverSelf
      ? outstandingSelfMovementActionsRef.current.filter((entry) =>
          crystalMovementCandidateNotBehindServer(serverSelf, entry, entry.direction),
        )
      : outstandingSelfMovementActionsRef.current;
    if (actions.length === 0) {
      return null;
    }
    const latest = actions[actions.length - 1];
    return {
      x: latest.x,
      y: latest.y,
      direction: latest.direction,
    };
  }

  function renderableSelfPrediction(
    serverSelf: { x: number; y: number; direction?: string } | null,
    candidate: PredictedPlayerMotion | null,
    now = Date.now(),
  ) {
    if (!serverSelf || !candidate) {
      return candidate;
    }
    if (candidate.x === serverSelf.x && candidate.y === serverSelf.y) {
      return candidate;
    }
    return hasSelfMovementTransportEvidence(now) ? candidate : null;
  }

  function localSelfMovementAnimationForPosition(position: PredictedPlayerMotion) {
    const action = [
      ...crystalSelfActionFeedRef.current,
      ...outstandingSelfMovementActionsRef.current,
    ]
      .slice()
      .reverse()
      .find((entry) => entry.x === position.x && entry.y === position.y);
    const pending =
      directionStepPendingRef.current &&
      directionStepPendingRef.current.x === position.x &&
      directionStepPendingRef.current.y === position.y
        ? directionStepPendingRef.current
        : null;
    const source = action ?? pending;
    if (!source) {
      return {};
    }
    const movementStartedAt = Math.max(source.sentAt, Date.now() - CRYSTAL_MOVE_FRAME_INTERVAL_MS);
    return {
      movementAnimation: source.mode === "run" ? "running" : "walking",
      movementStartedAt,
      movementUntil: movementStartedAt + movementStepIntervalMs(source.mode),
    } satisfies Pick<WorldEntity, "movementAnimation" | "movementStartedAt" | "movementUntil">;
  }

  function hasOpposingOutstandingSelfMovement(
    serverSelf: WorldEntity,
    requestedDirection: string,
    now = Date.now(),
  ) {
    pruneCrystalSelfActionFeed(now);
    const hasLead = (candidate: { x: number; y: number }) =>
      candidate.x !== serverSelf.x || candidate.y !== serverSelf.y;

    if (
      directionStepPendingQueueRef.current.some(
        (pending) =>
          movementDirectionsOppose(pending.direction, requestedDirection) &&
          (hasLead(pending) ||
            pending.sentFromX !== serverSelf.x ||
            pending.sentFromY !== serverSelf.y),
      )
    ) {
      return true;
    }

    if (
      directionStepPendingRef.current &&
      movementDirectionsOppose(directionStepPendingRef.current.direction, requestedDirection) &&
      (hasLead(directionStepPendingRef.current) ||
        directionStepPendingRef.current.sentFromX !== serverSelf.x ||
        directionStepPendingRef.current.sentFromY !== serverSelf.y)
    ) {
      return true;
    }

    if (
      crystalSelfActionFeedRef.current.some(
        (entry) => movementDirectionsOppose(entry.direction, requestedDirection) && hasLead(entry),
      )
    ) {
      return true;
    }

    if (
      outstandingSelfMovementActionsRef.current.some(
        (entry) => movementDirectionsOppose(entry.direction, requestedDirection) && hasLead(entry),
      )
    ) {
      return true;
    }

    const predicted = predictedPlayerPositionRef.current;
    if (
      predicted &&
      movementDirectionsOppose(predicted.direction, requestedDirection) &&
      hasLead(predicted) &&
      crystalMovementCandidateNotBehindServer(serverSelf, predicted, predicted.direction)
    ) {
      return true;
    }

    const anchor = localMovementAnchorRef.current;
    return Boolean(
      anchor &&
        now <= anchor.until &&
        movementDirectionsOppose(anchor.direction, requestedDirection) &&
        hasLead(anchor) &&
        crystalMovementCandidateNotBehindServer(serverSelf, anchor, anchor.direction),
    );
  }

  function crystalActionReachedOrPassed(entry: CrystalSelfActionFeedEntry, x: number, y: number) {
    const vectorX = Math.sign(entry.x - entry.fromX);
    const vectorY = Math.sign(entry.y - entry.fromY);
    if (vectorX === 0 && x !== entry.x) {
      return false;
    }
    if (vectorY === 0 && y !== entry.y) {
      return false;
    }
    if (vectorX !== 0 && Math.sign(x - entry.fromX) !== vectorX) {
      return false;
    }
    if (vectorY !== 0 && Math.sign(y - entry.fromY) !== vectorY) {
      return false;
    }
    const movedX = vectorX === 0 ? 0 : Math.abs(x - entry.fromX);
    const movedY = vectorY === 0 ? 0 : Math.abs(y - entry.fromY);
    const targetX = Math.abs(entry.x - entry.fromX);
    const targetY = Math.abs(entry.y - entry.fromY);
    return movedX >= targetX && movedY >= targetY;
  }

  function crystalActionMatchesAck(
    entry: CrystalSelfActionFeedEntry,
    x: number,
    y: number,
    direction: string | undefined,
  ) {
    if (direction && entry.direction !== direction) {
      return false;
    }
    if (entry.x === x && entry.y === y) {
      return true;
    }
    if (entry.mode !== "run") {
      return false;
    }
    const intermediate = pointMoveInDirection(
      { x: entry.fromX, y: entry.fromY },
      entry.direction,
      1,
    );
    return intermediate.x === x && intermediate.y === y;
  }

  function selfAckConfirmsLocalMovement(entity: { x: number; y: number; direction?: string }) {
    const directionMatches = (direction?: string) =>
      !direction || !entity.direction || direction === entity.direction;
    if (
      directionStepPendingQueueRef.current.some(
        (pending) =>
          directionMatches(pending.direction) &&
          directionStepReachedOrPassed(pending, entity.x, entity.y),
      )
    ) {
      return true;
    }
    if (
      directionStepPendingRef.current &&
      directionMatches(directionStepPendingRef.current.direction) &&
      directionStepReachedOrPassed(directionStepPendingRef.current, entity.x, entity.y)
    ) {
      return true;
    }
    if (
      crystalSelfActionFeedRef.current.some((entry) =>
        crystalActionMatchesAck(entry, entity.x, entity.y, entity.direction),
      )
    ) {
      return true;
    }
    if (
      outstandingSelfMovementActionsRef.current.some((entry) =>
        crystalActionMatchesAck(entry, entity.x, entity.y, entity.direction),
      )
    ) {
      return true;
    }

    const predicted = predictedPlayerPositionRef.current;
    if (
      predicted &&
      predicted.x === entity.x &&
      predicted.y === entity.y &&
      directionMatches(predicted.direction)
    ) {
      return true;
    }

    const anchor = localMovementAnchorRef.current;
    return Boolean(
      anchor &&
        anchor.x === entity.x &&
        anchor.y === entity.y &&
        directionMatches(anchor.direction),
    );
  }

  function staleSelfAckOverrideFromRecentActions(
    x: number,
    y: number,
    direction: string | undefined,
    currentSelf: { x: number; y: number; direction?: string },
    now: number,
  ): PredictedPlayerMotion | null {
    pruneCrystalSelfActionFeed(now);
    const history = recentSelfMovementActionHistoryRef.current;
    let matchedIndex = -1;
    for (let index = history.length - 1; index >= 0; index -= 1) {
      if (crystalActionMatchesAck(history[index], x, y, direction)) {
        matchedIndex = index;
        break;
      }
    }
    if (matchedIndex < 0) {
      return null;
    }

    const matched = history[matchedIndex];
    const newerAction = history
      .slice(matchedIndex + 1)
      .find(
        (entry) =>
          entry.sentAt > matched.sentAt &&
          (crystalActionReachedOrPassed(entry, currentSelf.x, currentSelf.y) ||
            (entry.x === currentSelf.x && entry.y === currentSelf.y)),
      );
    if (!newerAction || (currentSelf.x === x && currentSelf.y === y)) {
      return null;
    }

    return {
      x: currentSelf.x,
      y: currentSelf.y,
      direction: currentSelf.direction ?? newerAction.direction,
    };
  }

  function reconcileOutstandingSelfMovementActionsWithServer(
    x: number,
    y: number,
    direction: string | undefined,
  ) {
    const actions = outstandingSelfMovementActionsRef.current;
    if (actions.length === 0) {
      return;
    }

    let reachedIndex = -1;
    for (let index = actions.length - 1; index >= 0; index -= 1) {
      const action = actions[index];
      if (
        crystalActionReachedOrPassed(action, x, y) ||
        (action.x === x && action.y === y && (!direction || action.direction === direction))
      ) {
        reachedIndex = index;
        break;
      }
    if (action.mode === "run") {
      const intermediate = pointMoveInDirection(
        { x: action.fromX, y: action.fromY },
          action.direction,
          1,
        );
        if (
          intermediate.x === x &&
          intermediate.y === y &&
          (!direction || action.direction === direction)
        ) {
          reachedIndex = index;
          break;
        }
      }
    }

    if (reachedIndex >= 0) {
      outstandingSelfMovementActionsRef.current = actions.slice(reachedIndex + 1);
    }
  }

  function clearSettledSelfActionsAt(x: number, y: number, direction: string | undefined) {
    reconcileOutstandingSelfMovementActionsWithServer(x, y, direction);
    crystalSelfActionFeedRef.current = crystalSelfActionFeedRef.current.filter((entry) => {
      if (crystalActionReachedOrPassed(entry, x, y)) {
        return false;
      }
      if (entry.mode === "run") {
        const intermediate = pointMoveInDirection(
          { x: entry.fromX, y: entry.fromY },
          entry.direction,
          1,
        );
        if (
          intermediate.x === x &&
          intermediate.y === y &&
          (!direction || entry.direction === direction)
        ) {
          return false;
        }
      }
      return !(entry.x === x && entry.y === y && (!direction || entry.direction === direction));
    });
  }

  function reconcileCrystalSelfActionFeedWithServer(
    x: number,
    y: number,
    direction: string | undefined,
    packet: string,
    now: number,
  ): CrystalSelfAckDisposition {
    pruneCrystalSelfActionFeed(now);
    reconcileOutstandingSelfMovementActionsWithServer(x, y, direction);
    const feed = crystalSelfActionFeedRef.current;
    if (feed.length === 0) {
      return "none";
    }

    let passedIndex = -1;
    for (let index = feed.length - 1; index >= 0; index -= 1) {
      if (crystalActionReachedOrPassed(feed[index], x, y)) {
        passedIndex = index;
        break;
      }
    }
    if (passedIndex >= 0) {
      crystalSelfActionFeedRef.current = feed.slice(passedIndex + 1);
      return "confirmed";
    }

    const matchedIndex = feed.findIndex(
      (entry) => entry.x === x && entry.y === y && (!direction || entry.direction === direction),
    );
    if (matchedIndex >= 0) {
      const matched = feed[matchedIndex];
      crystalSelfActionFeedRef.current = feed.slice(matchedIndex + 1);
      directionStepNextAtRef.current = Math.max(
        directionStepNextAtRef.current,
        matched.sentAt + movementCommandDelayMs(matched.mode),
      );
      if (directionStepPendingRef.current?.x === x && directionStepPendingRef.current.y === y) {
        clearDirectionStepPendingQueue();
      }
      return "confirmed";
    }

    const partialRunIndex = feed.findIndex((entry) => {
      if (entry.mode !== "run") {
        return false;
      }
      const intermediate = pointMoveInDirection(
        { x: entry.fromX, y: entry.fromY },
        entry.direction,
        1,
      );
      return intermediate.x === x && intermediate.y === y && (!direction || entry.direction === direction);
    });
    if (partialRunIndex >= 0) {
      const partial = feed[partialRunIndex];
      crystalSelfActionFeedRef.current = feed.slice(partialRunIndex + 1);
      movementBlockedStepsRef.current = [
        ...recentMovementBlockedSteps(movementBlockedStepsRef.current, now),
        {
          fromX: x,
          fromY: y,
          direction: partial.direction,
          mode: partial.mode,
          at: now,
        },
      ].slice(-MOVEMENT_ROUTE_MAX_BLOCKED_STEPS);
      return "confirmed";
    }

    const oldest = feed[0];
    const correctedToKnownSource = feed.some((entry) => entry.fromX === x && entry.fromY === y);
    const isHardFailurePacket = packet === "UserDashFail";
    if (!isHardFailurePacket && correctedToKnownSource) {
      scheduleMovementConfirmTick();
      return "staleEcho";
    }

    return "correction";
  }

  function activeLocalMovementAnchor(
    serverSelf: { x: number; y: number; direction?: string } | null,
    now = Date.now(),
  ) {
    const anchor = localMovementAnchorRef.current;
    if (!anchor) {
      return null;
    }
    if (now > anchor.until) {
      localMovementAnchorRef.current = null;
      return null;
    }
    if (!serverSelf) {
      return anchor;
    }
    const lead = Math.max(Math.abs(anchor.x - serverSelf.x), Math.abs(anchor.y - serverSelf.y));
    if (lead > MOVEMENT_LOCAL_ACTION_MAX_LEAD_TILES) {
      localMovementAnchorRef.current = null;
      return null;
    }
    if (!crystalMovementCandidateNotBehindServer(serverSelf, anchor, anchor.direction ?? serverSelf.direction)) {
      localMovementAnchorRef.current = null;
      return null;
    }
    return anchor;
  }

  function activeCrystalLocalCurrentLocation(
    authoritativeSelf: { x: number; y: number; direction?: string } | null,
    now = Date.now(),
  ) {
    if (!authoritativeSelf) {
      return null;
    }
    const candidate = chooseCrystalSelfRenderPosition(
      authoritativeSelf,
      activeCrystalSelfActionSource(authoritativeSelf, now),
      activeOutstandingSelfActionSource(authoritativeSelf, now),
      activeLocalMovementAnchor(authoritativeSelf, now),
      renderableSelfPrediction(authoritativeSelf, predictedPlayerPositionRef.current, now),
      activeDirectionStepVisualSource(authoritativeSelf),
    );
    if (!candidate || (candidate.x === authoritativeSelf.x && candidate.y === authoritativeSelf.y)) {
      return null;
    }
    return hasSelfMovementTransportEvidence(now) ? candidate : null;
  }

  function preserveCrystalSelfRenderPosition(
    serverSelf: { x: number; y: number; direction?: string } | null,
    candidate: PredictedPlayerMotion | null,
  ) {
    if (!serverSelf) {
      lastCrystalSelfRenderPositionRef.current = candidate;
      return candidate;
    }

    let next = candidate;
    if (next) {
      const lead = Math.max(Math.abs(next.x - serverSelf.x), Math.abs(next.y - serverSelf.y));
      if (
        lead > MOVEMENT_LOCAL_RENDER_MAX_LEAD_TILES ||
        !crystalMovementCandidateNotBehindServer(serverSelf, next, next.direction ?? serverSelf.direction)
      ) {
        next = null;
      }
    }

    if (next && next.x === serverSelf.x && next.y === serverSelf.y) {
      lastCrystalSelfRenderPositionRef.current = null;
      return null;
    }

    lastCrystalSelfRenderPositionRef.current = next;
    return next;
  }

  function crystalEffectiveMovementMode(requestedMode: "walk" | "run", now: number): "walk" | "run" {
    return effectiveCrystalMovementMode(requestedMode, now, crystalRunPrimedUntilRef.current);
  }

  const viewportCenter = predictedSelf ?? self;
  const sortCenter = viewportCenter ?? self;

  const sortedEntities = useMemo(
    () =>
      [...displayEntities].sort((left, right) => {
        const leftRank = entitySortRank(left, world.playerObjectId, world.selectedObjectId, sortCenter);
        const rightRank = entitySortRank(right, world.playerObjectId, world.selectedObjectId, sortCenter);
        if (leftRank !== rightRank) return leftRank - rightRank;

        const leftDistance = tileDistance(sortCenter, left);
        const rightDistance = tileDistance(sortCenter, right);
        if (leftDistance !== rightDistance) return leftDistance - rightDistance;

        return left.name.localeCompare(right.name);
      }),
    [displayEntities, sortCenter, world.playerObjectId, world.selectedObjectId],
  );

  const viewportEntities = useMemo(() => {
    if (!viewportCenter) return [];

    return sortedEntities
      .filter(
        (entity) =>
          Math.abs(entity.x - viewportCenter.x) <= VIEWPORT_RANGE_X &&
          Math.abs(entity.y - viewportCenter.y) <= VIEWPORT_RANGE_Y,
      )
      .map((entity) => ({
        ...entity,
        dx: entity.x - viewportCenter.x,
        dy: entity.y - viewportCenter.y,
      }));
  }, [viewportCenter, sortedEntities]);

  const viewportTiles = useMemo(() => {
    const center = viewportCenter ?? world.sceneView?.center;
    if (!center) return [];

    const tiles: Array<{ x: number; y: number; dx: number; dy: number }> = [];

    for (let dy = -VIEWPORT_RANGE_Y; dy <= VIEWPORT_RANGE_Y; dy += 1) {
      for (let dx = -VIEWPORT_RANGE_X; dx <= VIEWPORT_RANGE_X; dx += 1) {
        tiles.push({ x: center.x + dx, y: center.y + dy, dx, dy });
      }
    }

    return tiles;
  }, [viewportCenter, world.sceneView]);

  useEffect(() => {
    let disposed = false;

    async function bootRuntime() {
      try {
        markMir2CacheMilestone("bevyRuntimeStart");
        const runtimeWindow = window as typeof window & {
          __mir2BevyRuntime?: RuntimeModule;
          __mir2BevyRuntimeBooted?: boolean;
          __mir2BevyRuntimeBackend?: BevyRuntimeBackend;
          __mir2BevyRuntimeDebug?: BevyRuntimeDebug;
        };
        const params = new URLSearchParams(window.location.search);
        if (params.get("skipRuntime") === "1") {
          const message = "Bevy runtime skipped by query parameter.";
          setRuntimePhase("dom-only");
          setRuntimeMessage(message);
          setBevyEntityRendererReady(false);
          setBevyRuntimeBackend(null);
          appendLog(message, "network");
          markMir2CacheMilestone("bevyRuntimeSkipped", { reason: "skipRuntime" });
          return;
        }
        if (runtimeWindow.__mir2BevyRuntimeBooted && runtimeWindow.__mir2BevyRuntime) {
          runtimeRef.current = runtimeWindow.__mir2BevyRuntime;
          lastBevyEntityRenderStateJsonRef.current = null;
          runtimeWindow.__mir2BevyRuntime.setMir2WorldState?.(JSON.stringify(worldRef.current));
          setBevyEntityRendererReady(Boolean(runtimeWindow.__mir2BevyRuntime.setMir2EntityRenderState));
          setBevyRuntimeBackend(runtimeWindow.__mir2BevyRuntimeBackend ?? null);
          setRuntimePhase("running");
          setRuntimeMessage("Bevy runtime already booted.");
          markMir2CacheMilestone("bevyRuntimeReady", {
            reused: true,
            backend: runtimeWindow.__mir2BevyRuntimeBackend ?? null,
          });
          return;
        }
        const runtimeSupport = detectBevyRuntimeSupport();
        const requestedBackend = params.get("bevyBackend")?.trim().toLowerCase() ?? null;
        const selectedBackend = selectBevyRuntimeBackend(params, runtimeSupport);
        if (!selectedBackend) {
          const message = "Bevy runtime skipped because neither WebGPU nor WebGL2 is available.";
          setRuntimePhase("dom-only");
          setRuntimeMessage(message);
          setBevyEntityRendererReady(false);
          setBevyRuntimeBackend(null);
          appendLog(message, "network");
          markMir2CacheMilestone("bevyRuntimeSkipped", {
            reason: "gpu-backend-unavailable",
            requestedBackend,
            webgpuSupported: runtimeSupport.webgpu,
            webgl2Supported: runtimeSupport.webgl2,
          });
          return;
        }

        appendLog(t("runtime.loadingModule"), "network");
        let runtimeBackend = selectedBackend;
        let fallbackFrom: BevyRuntimeBackend | undefined;
        let runtime: RuntimeModule;
        try {
          runtime = await loadBevyRuntimeModule(runtimeBackend);
        } catch (error) {
          if (runtimeBackend === "webgpu" && runtimeSupport.webgl2) {
            fallbackFrom = runtimeBackend;
            runtimeBackend = "webgl2";
            markMir2CacheMilestone("bevyRuntimeFallback", {
              from: fallbackFrom,
              to: runtimeBackend,
              message: error instanceof Error ? error.message : String(error),
            });
            runtime = await loadBevyRuntimeModule(runtimeBackend);
          } else {
            throw error;
          }
        }
        if (disposed) return;
        const compiledBackend = runtime.getMir2RendererBackend?.() ?? null;
        markMir2CacheMilestone("bevyRuntimeModuleReady", {
          backend: runtimeBackend,
          compiledBackend,
          fallbackFrom: fallbackFrom ?? null,
        });

        runtime.setMir2StatusSink?.((status) => {
          setRuntimePhase(status.phase);
          setRuntimeMessage(status.message);
        });

        runtime.setMir2WorldState?.(JSON.stringify(DEFAULT_WORLD_STATE));
        runtimeRef.current = runtime;
        lastBevyEntityRenderStateJsonRef.current = null;
        runtimeWindow.__mir2BevyRuntime = runtime;
        runtimeWindow.__mir2BevyRuntimeBooted = true;
        runtimeWindow.__mir2BevyRuntimeBackend = runtimeBackend;
        runtimeWindow.__mir2BevyRuntimeDebug = {
          requestedBackend,
          selectedBackend: runtimeBackend,
          compiledBackend,
          fallbackFrom,
          webgpuSupported: runtimeSupport.webgpu,
          webgl2Supported: runtimeSupport.webgl2,
          runtimeVersion: BEVY_RUNTIME_VERSION,
        };
        setBevyEntityRendererReady(Boolean(runtime.setMir2EntityRenderState));
        setBevyRuntimeBackend(runtimeBackend);
        runtime.bootMir2Runtime?.();
        markMir2CacheMilestone("bevyRuntimeReady", {
          reused: false,
          backend: runtimeBackend,
          compiledBackend,
          fallbackFrom: fallbackFrom ?? null,
        });
      } catch (error) {
        const message = error instanceof Error ? error.message : String(error);
        setRuntimePhase("boot-error");
        setRuntimeMessage(message);
        setBevyEntityRendererReady(false);
        setBevyRuntimeBackend(null);
        appendLog(t("runtime.bootFailed", [message]));
        markMir2CacheMilestone("bevyRuntimeError", { message });
        if (scheduleBevyRuntimeCacheRecovery(message)) {
          appendLog("Runtime cache mismatch detected; refreshing runtime files once.");
        }
      }
    }

    void bootRuntime();

    return () => {
      disposed = true;
      socketRef.current?.close();
      socketRef.current = null;
    };
  }, []);

  useEffect(() => {
    const center = self ?? world.sceneView?.center ?? { x: 330, y: 270 };
    const normalizedMapFileName = normalizeMapFileName(world.mapFileName);
    const sceneKey = `${normalizedMapFileName}:${Math.floor(center.x / SCENE_CHUNK_WIDTH)}:${Math.floor(
      center.y / SCENE_CHUNK_HEIGHT,
    )}`;
    if (loadingSceneKeyRef.current === sceneKey) {
      return;
    }
    if (!shouldReloadCrystalScene(world.originalMapRegion, normalizedMapFileName, center, sceneKey, loadedSceneKeyRef.current)) {
      return;
    }

    let disposed = false;

      async function loadSceneBlueprint() {
        function applySceneBlueprint(blueprint: SceneBlueprint, key: string) {
          markMir2CacheMilestone("sceneBlueprintReady", {
            sceneKey,
            sceneCache: key,
            spriteCount: blueprint.originalMapRegion ? Object.keys(blueprint.originalMapRegion.sprites).length : 0,
            cellCount: blueprint.originalMapRegion?.cells.length ?? 0,
          });
          loadedSceneKeyRef.current = sceneKey;
          loadingSceneKeyRef.current = null;
          setWorld((current) => ({
            ...current,
            mapTitle: blueprint.mapTitle ?? current.mapTitle,
            mapFileName: current.mapFileName ?? normalizedMapFileName,
            miniMapIndex: blueprint.miniMapIndex ?? current.miniMapIndex,
            bigMapIndex: blueprint.bigMapIndex ?? current.bigMapIndex,
            sceneView: blueprint.sceneView,
            terrainPatches: blueprint.terrainPatches,
            decorObjects: blueprint.decorObjects,
            originalMapRegion: blueprint.originalMapRegion,
          }));
        }

        async function applyStarterFallback() {
          const fallbackResponse = await fetch("/api/scene/starter");
          if (!fallbackResponse.ok) {
            throw new Error(`scene starter route returned ${fallbackResponse.status}`);
          }
          const fallbackBlueprint = (await fallbackResponse.json()) as SceneBlueprint;
          if (disposed) return;
          if (!fallbackBlueprint?.originalMapRegion) {
            throw new Error("starter scene missing originalMapRegion");
          }
          applySceneBlueprint(fallbackBlueprint, "starter");
        }

        try {
          loadingSceneKeyRef.current = sceneKey;
          const params = new URLSearchParams({
            map: normalizedMapFileName,
            x: String(center.x),
          y: String(center.y),
          width: String(SCENE_REQUEST_WIDTH),
          height: String(SCENE_REQUEST_HEIGHT),
        });
        markMir2CacheMilestone("sceneBlueprintStart", {
          sceneKey,
          map: normalizedMapFileName,
          x: center.x,
          y: center.y,
          width: SCENE_REQUEST_WIDTH,
          height: SCENE_REQUEST_HEIGHT,
        });
        const response = await fetch(`/api/scene/crystal?${params.toString()}`);
        if (!response.ok) {
          if (response.status === 424) {
            try {
              const body = await response.clone().json();
              if (body?.error === "resource_missing") {
                const publicPath = String(body.publicPath || body.resource?.path || "unknown");
                const signature = `${body.assetVersion ?? ""}:${body.publicPath ?? publicPath}:${response.status}`;
                if (!loggedSceneResourceErrorsRef.current.has(signature)) {
                  loggedSceneResourceErrorsRef.current.add(signature);
                  appendLog(
                    `scene resource missing for ${publicPath}: ${JSON.stringify({
                      publicPath: body.publicPath ?? publicPath,
                      libraryKey: body.libraryKey ?? body.resource?.libraryKey,
                      frameIndex: body.frameIndex ?? body.resource?.frameIndex,
                      manifestAssetHash: body.manifestAssetHash,
                      assetVersion: body.assetVersion,
                    })}`,
                    "system",
                  );
                }

                const resourceMissingMapName = normalizeMapFileName(String(body?.mapFileName ?? body?.resource?.mapFileName ?? normalizedMapFileName));
                if (resourceMissingMapName === "0") {
                  await applyStarterFallback();
                  return;
                }
              }
            } catch {
              // Ignore parse errors and fall back to generic message.
            }
          }
          throw new Error(`scene route returned ${response.status}`);
        }

        const blueprint = (await response.json()) as SceneBlueprint;
        if (disposed) return;
        applySceneBlueprint(blueprint, response.headers.get("x-mir2-scene-cache") ?? "crystal");
      } catch (error) {
        if (!disposed) {
          appendLog(t("log.sceneLoadFailed", [error instanceof Error ? error.message : String(error)]));
        }
        if (loadingSceneKeyRef.current === sceneKey) {
          loadingSceneKeyRef.current = null;
        }
      }
    }

    void loadSceneBlueprint();
    return () => {
      disposed = true;
      if (loadingSceneKeyRef.current === sceneKey) {
        loadingSceneKeyRef.current = null;
      }
    };
  }, [self?.x, self?.y, world.mapFileName]);

  useEffect(() => {
    runtimeRef.current?.setMir2WorldState?.(JSON.stringify(world));
  }, [world]);

  useEffect(() => {
    if (!world.originalMapRegion) return;
    const sceneKey = `${normalizeMapFileName(world.originalMapRegion.mapFileName)}:${world.originalMapRegion.regionBounds.minX}:${world.originalMapRegion.regionBounds.minY}:${world.originalMapRegion.regionBounds.maxX}:${world.originalMapRegion.regionBounds.maxY}`;
    if (sceneSpritesReadyKeyRef.current === sceneKey) return;
    sceneSpritesReadyKeyRef.current = sceneKey;
    markMir2CacheMilestone("sceneSpritesReady", {
      sceneKey,
      spriteCount: Object.keys(world.originalMapRegion?.sprites ?? {}).length,
      cellCount: world.originalMapRegion?.cells.length ?? 0,
    });
  }, [world.originalMapRegion]);

  useEffect(() => {
    if (screen !== "game" || firstPlayableFrameMarkedRef.current) return;
    const currentSelf =
      world.entities.find((entity) => entity.objectId === world.playerObjectId) ?? null;
    if (!currentSelf || !world.originalMapRegion) return;
    const runtimeReady = runtimePhase === "running" || runtimePhase === "dom-only";
    if (!runtimeReady) return;
    if (!initialSceneAssetsReady) return;
    markMir2CacheMilestone("gameScreenReady", {
      mapFileName: world.mapFileName,
      player: { x: currentSelf.x, y: currentSelf.y },
      runtimePhase,
      sceneAssets: sceneAssetReadinessRef.current,
    });
    firstPlayableFrameMarkedRef.current = true;
    markMir2CacheMilestone("firstPlayableFrame", {
      mapFileName: world.mapFileName,
      playerObjectId: world.playerObjectId,
      worldTick: world.worldTick,
      runtimePhase,
      sceneAssets: sceneAssetReadinessRef.current,
    });
  }, [
    screen,
    runtimePhase,
    initialSceneAssetsReady,
    world.entities,
    world.originalMapRegion,
    world.playerObjectId,
    world.mapFileName,
  ]);

  useEffect(() => {
    worldRef.current = world;
  }, [world]);

  useEffect(() => {
    if (!self || !predictedPlayerPosition) return;
    const visualUntil = movementPlanRef.current?.visualUntil ?? 0;
    const directionVisualUntil = directionStepVisualUntilRef.current;
    const predictedHoldUntil = predictedPlayerHoldUntilRef.current;
    const directionPending = directionStepPendingRef.current;
    if (
      directionPending &&
      self.x === directionPending.x &&
      self.y === directionPending.y &&
      (!directionPending.direction || self.direction === directionPending.direction)
    ) {
      clearDirectionStepPendingQueue();
      clearPredictedPlayerAfterDirectionVisual(
        self.x,
        self.y,
        Date.now(),
        directionStepVisualUntil(directionPending),
      );
      return;
    }
    if (self.x !== predictedPlayerPosition.x || self.y !== predictedPlayerPosition.y) {
      if (
        Math.max(Math.abs(self.x - predictedPlayerPosition.x), Math.abs(self.y - predictedPlayerPosition.y)) >
          MOVEMENT_LOCAL_ACTION_MAX_LEAD_TILES ||
        Date.now() >
          Math.max(
            visualUntil + MOVEMENT_PENDING_ACTION_MAX_AGE_MS,
            directionVisualUntil + MOVEMENT_PENDING_ACTION_MAX_AGE_MS,
            predictedHoldUntil,
          )
      ) {
        setPredictedPlayerMotion(null);
      }
      return;
    }
    if (
      !movementPlanRef.current &&
      !directionStepPendingRef.current &&
      crystalSelfActionFeedRef.current.length === 0
    ) {
      const now = Date.now();
      const visualUntil = directionStepVisualUntilRef.current;
      if (now >= visualUntil) {
        clearPredictedPlayerAfterDirectionVisual(self.x, self.y, now);
      } else {
        const timer = window.setTimeout(() => {
          clearPredictedPlayerAfterDirectionVisual(self.x, self.y, Date.now(), visualUntil);
        }, visualUntil - now + 16);
        return () => window.clearTimeout(timer);
      }
    }
  }, [self, predictedPlayerPosition]);

  useEffect(() => {
    if (!world.selectedObjectId) return;
    if (!world.entities.some((entity) => entity.objectId === world.selectedObjectId)) {
      setWorld((current) => ({ ...current, selectedObjectId: null }));
    }
  }, [world.entities, world.selectedObjectId]);

  useEffect(() => {
    if (!world.connected || wsState !== "open") return;
    const keepAliveTimer = window.setInterval(() => {
      if (socketRef.current?.readyState === WebSocket.OPEN) {
        socketRef.current.send(JSON.stringify({ type: "keepAlive", time: Date.now() }));
      }
    }, CRYSTAL_KEEPALIVE_INTERVAL_MS);

    return () => {
      window.clearInterval(keepAliveTimer);
    };
  }, [world.connected, wsState]);

  useEffect(() => {
    if (!world.connected || wsState !== "open") return;
    const params = new URLSearchParams(window.location.search);
    if (params.get("autoTick") !== "1") return;

    const tickTimer = window.setInterval(() => {
      if (!isMovementBusy()) {
        sendGatewayTick();
      }
    }, CRYSTAL_GAMEPLAY_TICK_MS);

    return () => {
      window.clearInterval(tickTimer);
    };
  }, [world.connected, wsState]);

  useEffect(() => {
    if (screen !== "game" || wsState !== "open") {
      movementPlanRef.current = null;
      movementBlockedStepsRef.current = [];
      queuedDirectionStepRef.current = null;
      queuedMoveIntentRef.current = null;
      pendingSelfMoveRef.current = null;
      pendingSelfTurnRef.current = null;
      nextMoveSendAtRef.current = 0;
      movementInputBlockedUntilRef.current = 0;
      crystalRunPrimedUntilRef.current = 0;
      predictedPlayerHoldUntilRef.current = 0;
      movementPredictionBlockedUntilRef.current = 0;
      lastSelfNoProgressAckRef.current = null;
      heldDirectionBlockedUntilRef.current = null;
      clearCrystalSelfActionFeed();
      clearOutstandingSelfMovementActions();
      clearRecentSelfMovementActionHistory();
      clearLocalMovementAnchor();
      lastCrystalSelfRenderPositionRef.current = null;
      clearDirectionStepPendingQueue();
      clearLocalSelfPrediction();
      return;
    }

    let animationFrame = 0;
    const tickMovementPlan = () => {
      const tickNow = Date.now();
      pruneCrystalSelfActionFeed(tickNow);
      pruneLocallySettledDirectionStepPending(tickNow);
      clearSettledPredictedPlayer(tickNow);
      movementPlanRef.current = null;
      queuedDirectionStepRef.current = null;
      queuedDirectionStepBacklogRef.current = [];
      if (directionStepPendingRef.current || directionStepPendingQueueRef.current.length > 0) {
        clearDirectionStepPendingQueue();
      }
      void trySendQueuedCrystalMove(tickNow);
      animationFrame = window.requestAnimationFrame(tickMovementPlan);
    };

    animationFrame = window.requestAnimationFrame(tickMovementPlan);

    return () => window.cancelAnimationFrame(animationFrame);
  }, [screen, wsState]);

  useEffect(() => {
    if (screen !== "game" || wsState !== "open") return;

    const pendingNpcInteractObjectId = pendingNpcInteractRef.current;
    if (!pendingNpcInteractObjectId) return;

    const pendingNpc = world.entities.find((entity) => entity.objectId === pendingNpcInteractObjectId);
    const nextSelf = world.entities.find((entity) => entity.objectId === world.playerObjectId) ?? null;
    if (!pendingNpc || pendingNpc.dead || pendingNpc.kind !== "npc") {
      pendingNpcInteractRef.current = null;
      return;
    }

    if (tileDistance(nextSelf, pendingNpc) <= 1) {
      pendingNpcInteractRef.current = null;
      interactTarget(pendingNpcInteractObjectId);
    }
  }, [screen, wsState, world.entities, world.playerObjectId]);

  function appendLog(
    text: string,
    tone: UiLogTone = "system",
    channel: UiLogChannel = defaultLogChannel(tone),
  ) {
    if (tone === "network") return;

    setLogs((current) =>
      [
        createLogLine(text, tone, channel, locale),
        ...current,
      ].slice(0, 24),
    );
  }

  function appendCrystalGameEntryChat() {
    if (gameEntryChatSeededRef.current) return;
    gameEntryChatSeededRef.current = true;

    const lines = [
      t("server.Welcome", [t("server.GameName", [], "Legend of Mir 2")], "Welcome to the Legend of Mir 2 Server."),
      t("client.AttackMode_Peace", [], "[Mode: Peaceful]"),
      t("client.PetMode_Both", [], "[Pet: Attack and Move]"),
      t("server.OnlinePlayers", [1], "Online Players: 1"),
    ];

    setLogs((current) => {
      const existing = new Set(current.map((line) => trimLogTimestamp(line.text)));
      const missing = lines.filter((line) => !existing.has(line));
      if (!missing.length) {
        return current;
      }

      const seeded = [...missing]
        .reverse()
        .map((line) => createLogLine(line, "chat", "server", locale));
      return [...seeded, ...current].slice(0, 24);
    });
  }

  function send(command: Record<string, unknown>, options?: { quiet?: boolean }) {
    if (socketRef.current?.readyState !== WebSocket.OPEN) return false;
    lastCommandRef.current = command;
    const commandNow = Date.now();
    if (isMovementPredictionBlockingCommand(command)) {
      movementPredictionBlockedUntilRef.current = Math.max(
        movementPredictionBlockedUntilRef.current,
        commandNow + MOVEMENT_ACTION_PREDICTION_BLOCK_MS,
      );
    }
    const debugWindow = window as typeof window & {
      __mir2LastCommand?: Record<string, unknown>;
      __mir2CommandHistory?: Array<Record<string, unknown>>;
      __mir2MovementSentCommands?: Array<Record<string, unknown>>;
    };
    const debugCommand = isMovementConsoleCommand(command)
      ? assignMovementConsoleSequence({ ...command, at: commandNow })
      : { ...command, at: commandNow };
    debugWindow.__mir2LastCommand = command;
    debugWindow.__mir2CommandHistory = [debugCommand, ...(debugWindow.__mir2CommandHistory ?? [])].slice(0, 50);
    if (command.type === "startGame") {
      markMir2CacheMilestone("startGameSubmit", {
        characterIndex: command.characterIndex,
      });
    }
    if (command.type === "getRanking") {
      lastRankingRequestRef.current = {
        rankType: numberOrUndefined(command.rankType) ?? 0,
        rankIndex: numberOrUndefined(command.rankIndex) ?? 0,
        onlineOnly: command.onlineOnly === true,
      };
    }
    if (isMovementCommand(command)) {
      debugWindow.__mir2MovementSentCommands = [
        debugCommand,
        ...(debugWindow.__mir2MovementSentCommands ?? []),
      ].slice(0, 50);
      recordMovementDiagnostic("tx:movementCommand", {
        command: debugCommand,
        sample: captureMovementDiagnosticSample(commandNow),
      });
    }
    if (isMovementConsoleCommand(command)) {
      rememberMovementConsoleCommand(debugCommand);
      recordMovementConsoleEvent("send", {
        command: debugCommand,
        state: captureMovementConsoleState(commandNow),
      });
    }
    recordDebugEvent("packet-out", "net", {
      type: typeof command.type === "string" ? command.type : "?",
    });
    // Feed the beginner-tutorial state machine (additive; the overlay listens for
    // these and ignores everything it doesn't care about). See
    // app/components/original-client-tutorial-overlay.tsx.
    if (typeof command.type === "string" && typeof window !== "undefined") {
      window.dispatchEvent(new CustomEvent("mir2:action", { detail: { type: command.type } }));
    }
    socketRef.current.send(JSON.stringify(command));
    if (isMovementCommand(command)) {
      scheduleMovementConfirmTick();
    }
    if (!options?.quiet) appendLog(t("log.sent", [JSON.stringify(command)]), "network");
    return true;
  }

  function sendGatewayTick() {
    if (socketRef.current?.readyState === WebSocket.OPEN) {
      socketRef.current.send(JSON.stringify({ type: "tick" }));
    }
  }

  function isMovementBusy() {
    pruneLocallySettledDirectionStepPending(Date.now());
    return (
      Boolean(pendingSelfMoveRef.current) ||
      Boolean(queuedMoveIntentRef.current) ||
      Boolean(movementPlanRef.current) ||
      crystalSelfActionFeedRef.current.length > 0 ||
      directionStepPendingQueueRef.current.length > 0 ||
      Boolean(directionStepPendingRef.current) ||
      Boolean(queuedDirectionStepRef.current) ||
      queuedDirectionStepBacklogRef.current.length > 0
    );
  }

  function scheduleMovementConfirmTick() {
    if (movementConfirmTickTimerRef.current !== null) {
      return;
    }
    movementConfirmTickTimerRef.current = window.setTimeout(() => {
      movementConfirmTickTimerRef.current = null;
      if (!isMovementBusy() && !predictedPlayerPositionRef.current) {
        return;
      }
      const now = Date.now();
      pruneLocallySettledDirectionStepPending(now);
      const currentWorld = worldRef.current;
      const serverSelf = currentWorld.entities.find((entity) => entity.objectId === currentWorld.playerObjectId);
      if (serverSelf) {
        recoverStaleMovementPlanFromServer(serverSelf.x, serverSelf.y, serverSelf.direction, now);
        clearVisuallySettledDirectionStepPending(now);
        clearSettledPredictedPlayer(now);
      }
      trySendQueuedCrystalMove(now);
      if (isMovementBusy() || predictedPlayerPositionRef.current) {
        scheduleMovementConfirmTick();
      }
    }, MOVEMENT_CONFIRM_TICK_DELAY_MS);
  }

  function isMovementCommand(command: Record<string, unknown>) {
    return command.type === "walk" || command.type === "run" || command.type === "turn";
  }

  function isMovementPredictionBlockingCommand(command: Record<string, unknown>) {
    return command.type === "attack" || command.type === "rangeAttack" || command.type === "castSkill";
  }

  function isMovementPacketName(packet: string) {
    return (
      packet === "UserLocation" ||
      packet === "Pushed" ||
      packet === "UserDash" ||
      packet === "UserDashFail" ||
      packet === "UserDashAttack" ||
      packet === "UserAttackMove" ||
      packet === "ObjectTurn" ||
      packet === "ObjectWalk" ||
      packet === "ObjectRun" ||
      packet === "ObjectPushed" ||
      packet === "ObjectDash" ||
      packet === "ObjectDashFail" ||
      packet === "ObjectDashAttack" ||
      packet === "ObjectBackStep" ||
      packet === "ObjectSitDown"
    );
  }

  useEffect(() => {
    const stage5Window = window as typeof window & {
      __mir2Stage5?: {
        send: (command: Record<string, unknown>) => boolean;
        closeGatewayForReconnectSmoke: () => boolean;
        state: {
          screen: ClientScreen;
          language: Mir2Language;
          accountId: string;
          wsState: string;
          reconnectStatus: ReconnectStatus;
          loginBusy: boolean;
          selectedCharacterIndex: number;
          characters: SelectCharacterEntry[];
          mapFileName: string | null;
          mapTitle: string | null;
          playerObjectId: string | null;
          player: { x: number; y: number } | null;
          predictedPlayer: PredictedPlayerMotion | null;
          movementPlan: MovementPlan | null;
          directionStepPending: DirectionStepPending | null;
          directionStepPendingQueue: DirectionStepPending[];
          crystalSelfActionFeed: CrystalSelfActionFeedEntry[];
          outstandingSelfMovementActions: CrystalSelfActionFeedEntry[];
          movementInputBlockedUntil: number;
          playerHp: number | undefined;
          playerMaxHp: number | undefined;
          playerMp: number | undefined;
          sceneTerrainKinds: string[];
          originalMapRegionSummary: {
            mapFileName: string;
            cellCount: number;
            spriteCount: number;
            regionBounds: OriginalMapRegion["regionBounds"];
            playBounds: OriginalMapRegion["playBounds"];
          } | null;
          sceneInteractionReady: boolean;
          sceneAssetReadiness: SceneAssetReadiness | null;
          resourceMetrics: {
            domImageCount: number;
            originalMapRegionSpriteCount: number;
            originalMapRegionCellCount: number;
            bevyEntityRenderer: unknown;
          };
          selectedObjectId: string | null;
          logs: UiLogLine[];
          entities: WorldEntity[];
          groundDrops: GroundDrop[];
          beltItems: WorldItem[];
          inventoryItems: WorldItem[];
          storageItems: WorldItem[];
          equipmentItems: EquipmentItem[];
          questLog: QuestEntry[];
          activeNpcDialog: NpcDialog | null;
          gold: number;
          activeInventoryTab: "bag1" | "bag2" | "quest";
          activeCharacterTab: "char" | "stats1" | "stats2" | "spells";
          hasExpandedStorage: boolean;
          hasStoragePassword: boolean;
          requireStoragePassword: boolean;
          storageSessionUnlocked: boolean;
          storagePasswordLastSetBinaryDatetime: number;
          knownSkills: KnownSkill[];
          activeBuffs: ActiveBuff[];
          stage5Systems: Stage5SystemsState;
          credit: number;
          lastCommand: Record<string, unknown> | null;
          worldSnapshotVersion: number;
          worldSnapshotRealtimeMode: WorldSnapshotRealtimeMode;
          worldTick: number;
        };
      };
    };
    stage5Window.__mir2Stage5 = {
      send: (command) => send(command),
      closeGatewayForReconnectSmoke: () => {
        const socket = socketRef.current;
        if (!socket || socket.readyState === WebSocket.CLOSED || socket.readyState === WebSocket.CLOSING) {
          return false;
        }
        manualSocketCloseRef.current = false;
        socket.close();
        return true;
      },
      state: {
        screen,
        language,
        accountId,
        wsState,
        reconnectStatus,
        loginBusy,
        selectedCharacterIndex,
        characters,
        mapFileName: world.mapFileName,
        mapTitle: world.mapTitle,
        playerObjectId: world.playerObjectId,
        get player() {
          const currentWorld = worldRef.current;
          const currentSelf =
            currentWorld.entities.find((entity) => entity.objectId === currentWorld.playerObjectId) ?? null;
          const predicted = preserveCrystalSelfRenderPosition(
            currentSelf,
            chooseCrystalSelfRenderPosition(
              currentSelf,
              renderableSelfPrediction(currentSelf, predictedPlayerPositionRef.current),
            ),
          );
          if (currentSelf && predicted) {
            const lead = Math.max(Math.abs(predicted.x - currentSelf.x), Math.abs(predicted.y - currentSelf.y));
            if (lead <= MOVEMENT_LOCAL_RENDER_MAX_LEAD_TILES) {
              return { x: predicted.x, y: predicted.y };
            }
          }
          return currentSelf ? { x: currentSelf.x, y: currentSelf.y } : null;
        },
        get predictedPlayer() {
          const currentWorld = worldRef.current;
          const currentSelf =
            currentWorld.entities.find((entity) => entity.objectId === currentWorld.playerObjectId) ?? null;
          const predicted = preserveCrystalSelfRenderPosition(
            currentSelf,
            chooseCrystalSelfRenderPosition(
              currentSelf,
              renderableSelfPrediction(currentSelf, predictedPlayerPositionRef.current),
            ),
          );
          if (
            currentSelf &&
            predicted &&
            predicted.x === currentSelf.x &&
            predicted.y === currentSelf.y &&
            !hasSelfMovementTransportEvidence()
          ) {
            return null;
          }
          return predicted;
        },
        get movementPlan() {
          return movementPlanRef.current;
        },
        get directionStepPending() {
          return directionStepPendingRef.current;
        },
        get directionStepPendingQueue() {
          return directionStepPendingQueueRef.current;
        },
        get crystalSelfActionFeed() {
          return crystalSelfActionFeedRef.current;
        },
        get outstandingSelfMovementActions() {
          return outstandingSelfMovementActionsRef.current;
        },
        get movementInputBlockedUntil() {
          return movementInputBlockedUntilRef.current;
        },
        playerHp: world.playerHp,
        playerMaxHp: world.playerMaxHp,
        playerMp: world.playerMp,
        sceneTerrainKinds: world.terrainPatches.map((patch) => patch.kind),
        originalMapRegionSummary: world.originalMapRegion
          ? {
              mapFileName: world.originalMapRegion.mapFileName,
              cellCount: world.originalMapRegion.cells.length,
              spriteCount: Object.keys(world.originalMapRegion.sprites).length,
              regionBounds: world.originalMapRegion.regionBounds,
              playBounds: world.originalMapRegion.playBounds,
            }
          : null,
        sceneInteractionReady: initialSceneAssetsReady,
        get sceneAssetReadiness() {
          return sceneAssetReadinessRef.current;
        },
        get resourceMetrics() {
          const browserWindow = typeof window !== "undefined"
            ? window as typeof window & { __mir2BevyEntityRendererDebug?: unknown }
            : null;
          return {
            domImageCount: typeof document !== "undefined" ? document.images.length : 0,
            originalMapRegionSpriteCount: world.originalMapRegion
              ? Object.keys(world.originalMapRegion.sprites).length
              : 0,
            originalMapRegionCellCount: world.originalMapRegion?.cells.length ?? 0,
            bevyEntityRenderer: browserWindow?.__mir2BevyEntityRendererDebug ?? null,
          };
        },
        selectedObjectId: world.selectedObjectId,
        logs,
        entities: world.entities,
        groundDrops: world.groundDrops,
        beltItems: world.beltItems,
        inventoryItems: world.inventoryItems,
        storageItems: world.storageItems,
        equipmentItems: world.equipmentItems,
        questLog: world.questLog,
        activeNpcDialog: world.activeNpcDialog,
        gold: world.gold,
        activeInventoryTab,
        activeCharacterTab,
        hasExpandedStorage: world.hasExpandedStorage,
        hasStoragePassword: world.hasStoragePassword,
        requireStoragePassword: world.requireStoragePassword,
        storageSessionUnlocked: world.storageSessionUnlocked,
        storagePasswordLastSetBinaryDatetime: world.storagePasswordLastSetBinaryDatetime,
        knownSkills: world.knownSkills,
        activeBuffs: world.activeBuffs,
        stage5Systems: world.stage5Systems,
        credit: world.credit,
        lastCommand: lastCommandRef.current,
        worldSnapshotVersion: worldSnapshotVersionRef.current,
        worldSnapshotRealtimeMode: packetRuntimeSnapshotModeRef.current,
        worldTick: world.worldTick,
      },
    };
    return () => {
      delete stage5Window.__mir2Stage5;
    };
  });

  function setGatewayReconnectStatus(nextStatus: ReconnectStatus) {
    reconnectStatusRef.current = nextStatus;
    setReconnectStatus(nextStatus);
  }

  function clearGatewayReconnectTimer() {
    if (reconnectTimerRef.current === null) {
      return;
    }
    window.clearTimeout(reconnectTimerRef.current);
    reconnectTimerRef.current = null;
  }

  function resetGatewayReconnectState() {
    clearGatewayReconnectTimer();
    reconnectAttemptRef.current = 0;
    reconnectSnapshotRef.current = null;
    setGatewayReconnectStatus(createIdleReconnectStatus());
  }

  function captureGatewayReconnectSnapshot(): ReconnectSnapshot | null {
    if (screenRef.current !== "game") {
      return null;
    }

    const fallbackAccountId = accountIdRef.current.trim();
    const auth = activeReconnectAuthRef.current ?? {
      kind: "password" as const,
      accountId: fallbackAccountId,
      password: passwordRef.current,
    };
    if (!auth.accountId.trim()) {
      return null;
    }
    if (auth.kind === "sui" && auth.expiresAt <= Date.now() + 5000) {
      return null;
    }

    const reconnectCharacters = charactersRef.current;
    const selected =
      reconnectCharacters[selectedCharacterIndexRef.current] ??
      reconnectCharacters.find((character) => !isFallbackCharacter(character)) ??
      reconnectCharacters[0] ??
      null;
    return {
      auth,
      characterIndex: selected?.index ?? selectedCharacterIndexRef.current ?? 0,
      characterName: selected?.name ?? null,
    };
  }

  function sendGatewayReconnectSequence(snapshot: ReconnectSnapshot) {
    if (snapshot.auth.kind === "sui") {
      sendSuiLoginCommand(send, snapshot.auth.accountId, snapshot.auth.token);
    } else {
      send({ type: "clientVersion" }, { quiet: true });
      send(
        { type: "login", accountId: snapshot.auth.accountId, password: snapshot.auth.password },
        { quiet: true },
      );
    }
    send({ type: "startGame", characterIndex: snapshot.characterIndex }, { quiet: true });
  }

  function completeGatewayReconnect() {
    if (reconnectStatusRef.current.mode === "idle" && reconnectSnapshotRef.current === null) {
      return;
    }

    const attempt = reconnectStatusRef.current.attempt || reconnectAttemptRef.current;
    resetGatewayReconnectState();
    appendLog(t("ui.reconnected", [], "Connection restored."), "system");
    markMir2CacheMilestone("gatewayReconnected", { attempt });
  }

  function failGatewayReconnect() {
    clearGatewayReconnectTimer();
    const attempt = reconnectStatusRef.current.attempt || reconnectAttemptRef.current;
    reconnectAttemptRef.current = 0;
    reconnectSnapshotRef.current = null;
    setGatewayReconnectStatus({ mode: "failed", attempt, nextAttemptAt: null });
    appendLog(t("ui.reconnectFailed", [], "Connection lost. Please log in again."), "system");
  }

  function scheduleGatewayReconnect(snapshot: ReconnectSnapshot | null) {
    if (!snapshot) {
      if (screenRef.current === "game") {
        failGatewayReconnect();
      }
      return;
    }

    clearGatewayReconnectTimer();
    reconnectSnapshotRef.current = snapshot;
    const nextAttempt = reconnectAttemptRef.current + 1;
    if (nextAttempt > MAX_RECONNECT_ATTEMPTS) {
      failGatewayReconnect();
      return;
    }

    reconnectAttemptRef.current = nextAttempt;
    const delayMs = RECONNECT_DELAYS_MS[Math.min(nextAttempt - 1, RECONNECT_DELAYS_MS.length - 1)];
    const nextAttemptAt = Date.now() + delayMs;
    setGatewayReconnectStatus({ mode: "scheduled", attempt: nextAttempt, nextAttemptAt });
    appendLog(
      t("ui.reconnectScheduled", [Math.ceil(delayMs / 1000)], "Connection lost. Reconnecting in {0}s."),
      "system",
    );
    markMir2CacheMilestone("gatewayReconnectScheduled", {
      attempt: nextAttempt,
      delayMs,
      characterIndex: snapshot.characterIndex,
      characterName: snapshot.characterName,
    });
    reconnectTimerRef.current = window.setTimeout(() => {
      reconnectTimerRef.current = null;
      if (!reconnectSnapshotRef.current) {
        return;
      }
      setGatewayReconnectStatus({ mode: "connecting", attempt: nextAttempt, nextAttemptAt: null });
      connectGateway();
    }, delayMs);
  }

  useEffect(() => {
    return () => {
      if (reconnectTimerRef.current !== null) {
        window.clearTimeout(reconnectTimerRef.current);
        reconnectTimerRef.current = null;
      }
    };
  }, []);

  function connectGateway(bootstrapAfterOpen = false) {
    if (socketRef.current?.readyState === WebSocket.OPEN) {
      if (bootstrapAfterOpen) sendGatewayBootstrapSequence(send, accountIdRef.current, passwordRef.current);
      return;
    }
    if (socketRef.current?.readyState === WebSocket.CONNECTING) {
      return;
    }

    markMir2CacheMilestone("gatewayConnectStart", {
      url: resolveGatewayWebSocketUrl(),
      bootstrapAfterOpen,
    });
    const socket = new WebSocket(resolveGatewayWebSocketUrl());
    socketRef.current = socket;
    setWsState("connecting");

    socket.addEventListener("open", () => {
      setWsState("open");
      markMir2CacheMilestone("gatewayConnected");
      setWorld((current) => ({ ...current, connected: true }));
      appendLog(t("log.gatewayWsOpen"), "network");
      send({ type: "setLanguage", language }, { quiet: true });
      const reconnectSnapshot = reconnectSnapshotRef.current;
      if (reconnectSnapshot && reconnectAttemptRef.current > 0) {
        setGatewayReconnectStatus({
          mode: "resuming",
          attempt: reconnectStatusRef.current.attempt || reconnectAttemptRef.current,
          nextAttemptAt: null,
        });
        markMir2CacheMilestone("gatewayReconnectResuming", {
          attempt: reconnectStatusRef.current.attempt || reconnectAttemptRef.current,
          characterIndex: reconnectSnapshot.characterIndex,
          characterName: reconnectSnapshot.characterName,
        });
        sendGatewayReconnectSequence(reconnectSnapshot);
        return;
      }
      if (pendingSuiLoginRef.current) {
        const pending = pendingSuiLoginRef.current;
        pendingSuiLoginRef.current = null;
        sendSuiLoginCommand(send, pending.accountId, pending.token);
        return;
      }
      if (pendingNewAccountRef.current) {
        pendingNewAccountRef.current = false;
        sendGatewayNewAccountCommand(send, accountIdRef.current, passwordRef.current);
        return;
      }
      if (pendingLoginRef.current) {
        pendingLoginRef.current = false;
        sendPasswordLoginCommand(send, accountIdRef.current, passwordRef.current, { quietClientVersion: true });
        return;
      }
      if (bootstrapAfterOpen) sendGatewayBootstrapSequence(send, accountIdRef.current, passwordRef.current);
    });

    socket.addEventListener("close", () => {
      const isCurrentSocket = socketRef.current === socket;
      const closedManually = manualSocketCloseRef.current;
      if (isCurrentSocket) {
        socketRef.current = null;
      }
      manualSocketCloseRef.current = false;
      pendingLoginRef.current = false;
      pendingNewAccountRef.current = false;
      pendingSuiLoginRef.current = null;
      setLoginBusy(false);
      setWsState("closed");
      markMir2CacheMilestone("gatewayClosed");
      setWorld((current) => ({ ...current, connected: false }));
      appendLog(t("log.gatewayWsClosed"), "network");
      if (isCurrentSocket && !closedManually && reconnectStatusRef.current.mode !== "failed") {
        scheduleGatewayReconnect(reconnectSnapshotRef.current ?? captureGatewayReconnectSnapshot());
      }
    });

    socket.addEventListener("error", () => {
      if (reconnectSnapshotRef.current) {
        markMir2CacheMilestone("gatewayReconnectSocketError", {
          attempt: reconnectStatusRef.current.attempt || reconnectAttemptRef.current,
        });
      }
    });

    socket.addEventListener("message", (event) => {
      try {
        handleGatewayEvent(JSON.parse(event.data as string) as GatewayEvent);
      } catch (error) {
        appendLog(t("log.invalidGatewayPayload", [String(error)]), "system");
      }
    });
  }

  useEffect(() => {
    if (socketRef.current?.readyState === WebSocket.OPEN) {
      send({ type: "setLanguage", language }, { quiet: true });
    }
  }, [language]);

  function createAccount() {
    resetGatewayReconnectState();
    setLoginBusy(false);
    setLoginErrorKey(null);

    if (socketRef.current?.readyState !== WebSocket.OPEN) {
      pendingNewAccountRef.current = true;
      connectGateway();
      return;
    }

    sendGatewayNewAccountCommand(send, accountId, password);
  }

  function submitLogin() {
    resetGatewayReconnectState();
    activeReconnectAuthRef.current = {
      kind: "password",
      accountId: accountId.trim(),
      password,
    };
    setLoginBusy(true);
    setLoginErrorKey(null);
    markMir2CacheMilestone("loginSubmit", { method: "password" });

    if (socketRef.current?.readyState !== WebSocket.OPEN) {
      pendingLoginRef.current = true;
      connectGateway();
      return;
    }

    sendPasswordLoginCommand(send, accountId, password);
  }

  async function submitSuiLogin(kind: SuiLoginKind, walletId?: string) {
    resetGatewayReconnectState();
    setWalletPickerOpen(false);
    setLoginBusy(true);
    setLoginErrorKey(null);

    try {
      const login = await requestSuiLoginToken(kind, walletId);
      activeReconnectAuthRef.current = {
        kind: "sui",
        accountId: login.accountId,
        token: login.token,
        expiresAt: login.expiresAt,
      };
      setAccountId(login.accountId);
      if (socketRef.current?.readyState !== WebSocket.OPEN) {
        pendingSuiLoginRef.current = login;
        connectGateway();
        return;
      }
      sendSuiLoginCommand(send, login.accountId, login.token);
    } catch (error) {
      setLoginBusy(false);
      const label = kind === "passkey" ? "Passkey" : "Wallet";
      setLoginErrorKey(`${label} login failed: ${error instanceof Error ? error.message : String(error)}`);
    }
  }

  function submitPasskeyLogin() {
    void submitSuiLogin("passkey");
  }

  function toggleWalletPicker() {
    setSuiWallets(getSuiWalletSummaries());
    setWalletPickerOpen((current) => !current);
  }

  function submitWalletLogin(walletId: string) {
    setWalletPickerOpen(false);
    void submitSuiLogin("wallet", walletId);
  }

  function startSelectedCharacter() {
    const selected = characters[selectedCharacterIndex] ?? characters[0];
    markMir2CacheMilestone("startGameSubmit", {
      characterIndex: selected?.index ?? 0,
    });
    send({ type: "startGame", characterIndex: selected?.index ?? 0 });
  }

  function quickEnterWorld() {
    resetGatewayReconnectState();
    activeReconnectAuthRef.current = {
      kind: "password",
      accountId: accountId.trim(),
      password,
    };
    markMir2CacheMilestone("quickEnterSubmit");
    if (socketRef.current?.readyState !== WebSocket.OPEN) {
      connectGateway(true);
      return;
    }
    sendGatewayBootstrapSequence(send, accountId, password);
  }

  function resetClient() {
    const socketToClose = socketRef.current;
    manualSocketCloseRef.current = Boolean(
      socketToClose &&
        socketToClose.readyState !== WebSocket.CLOSED &&
        socketToClose.readyState !== WebSocket.CLOSING,
    );
    resetGatewayReconnectState();
    activeReconnectAuthRef.current = null;
    pendingLoginRef.current = false;
    pendingNewAccountRef.current = false;
    pendingSuiLoginRef.current = null;
    pendingTransferRef.current = null;
    pendingNpcInteractRef.current = null;
    npcCallGuardRef.current = null;
    movementPlanRef.current = null;
    movementBlockedStepsRef.current = [];
    queuedDirectionStepRef.current = null;
    queuedMoveIntentRef.current = null;
    pendingSelfMoveRef.current = null;
    pendingSelfTurnRef.current = null;
    nextMoveSendAtRef.current = 0;
    crystalRunPrimedUntilRef.current = 0;
    predictedPlayerHoldUntilRef.current = 0;
    lastSelfMovementAckRef.current = null;
    lastSelfNoProgressAckRef.current = null;
    heldDirectionBlockedUntilRef.current = null;
    clearCrystalSelfActionFeed();
    clearOutstandingSelfMovementActions();
    clearLocalMovementAnchor();
    lastCrystalSelfRenderPositionRef.current = null;
    clearDirectionStepPendingQueue();
    if (movementConfirmTickTimerRef.current !== null) {
      window.clearTimeout(movementConfirmTickTimerRef.current);
      movementConfirmTickTimerRef.current = null;
    }
    gameEntryChatSeededRef.current = false;
    if (socketRef.current?.readyState === WebSocket.OPEN) {
      send({ type: "disconnect" });
    }
    socketToClose?.close();
    socketRef.current = null;
    if (!socketToClose || socketToClose.readyState === WebSocket.CLOSED) {
      manualSocketCloseRef.current = false;
    }
    setWsState("closed");
    setScreen("login");
    setLoginBusy(false);
    setLoginErrorKey(null);
    setChatMessage("");
    setSelectedCharacterIndex(0);
    setShowInventory(false);
    setShowCharacter(false);
    setActiveInventoryTab("bag1");
    setActiveCharacterTab("char");
    setCharacters([fallbackCharacter(language, accountId)]);
    setLogs([]);
    setWorld((current) => ({
      ...DEFAULT_WORLD_STATE,
        mapTitle: current.mapTitle,
        mapFileName: current.mapFileName,
      inSafeZone: current.inSafeZone,
      miniMapIndex: current.miniMapIndex,
      bigMapIndex: current.bigMapIndex,
      sceneView: current.sceneView,
      terrainPatches: current.terrainPatches,
      decorObjects: current.decorObjects,
        originalMapRegion: current.originalMapRegion,
      }));
  }

  function queueCrystalMoveIntent(intent: QueuedMoveIntent) {
    queuedMoveIntentRef.current = intent;
    movementPlanRef.current = null;
    queuedDirectionStepRef.current = null;
    queuedDirectionStepBacklogRef.current = [];
    clearDirectionStepPendingQueue();
    void trySendQueuedCrystalMove();
  }

  function sendCrystalTurn(direction: string) {
    const now = Date.now();
    if (!canSendMovement(readSelfMovementControllerState(), now)) {
      scheduleMovementConfirmTick();
      return false;
    }
    const serverSelf = currentAuthoritativeSelf();
    const visualUntil = now + MOVEMENT_TURN_VISUAL_HOLD_MS;
    if (serverSelf) {
      pendingSelfTurnRef.current = { direction, sentAt: now, visualUntil };
      directionStepVisualUntilRef.current = Math.max(directionStepVisualUntilRef.current, visualUntil);
      setPredictedPlayerMotion({ x: serverSelf.x, y: serverSelf.y, direction }, visualUntil);
    }
    nextMoveSendAtRef.current = now + movementCommandDelayMs("walk");
    const sent = send({ type: "turn", direction });
    if (!sent) {
      nextMoveSendAtRef.current = now;
      clearLocalSelfPrediction();
      return false;
    }
    scheduleMovementConfirmTick();
    return true;
  }

  function trySendQueuedCrystalMove(now = Date.now()) {
    const queued = queuedMoveIntentRef.current;
    if (!queued) {
      return false;
    }
    if (sceneInputDeferredForInitialAssets()) {
      return false;
    }
    const currentWorld = worldRef.current;
    const serverSelf = currentAuthoritativeSelf(currentWorld);
    if (!serverSelf) {
      queuedMoveIntentRef.current = null;
      return false;
    }
    const pending = pendingSelfMoveRef.current;
    if (pending && now - pending.sentAt > MOVEMENT_PENDING_ACTION_MAX_AGE_MS) {
      pendingSelfMoveRef.current = null;
      queuedMoveIntentRef.current = null;
      crystalRunPrimedUntilRef.current = 0;
      movementInputBlockedUntilRef.current = Math.max(
        movementInputBlockedUntilRef.current,
        now + CRYSTAL_CORRECTION_BLOCK_MS,
      );
      clearLegacySelfMovementCoordinateSources();
      clearLocalSelfPrediction();
      return false;
    }
    if (!canSendMovement(readSelfMovementControllerState(), now)) {
      scheduleMovementConfirmTick();
      return false;
    }

    const intentAge = now - queued.requestedAt;
    const maxIntentAge =
      queued.kind === "target" ? MOVEMENT_PENDING_ACTION_MAX_AGE_MS * 4 : MOVEMENT_QUEUED_DIRECTION_MAX_AGE_MS;
    if (intentAge > maxIntentAge) {
      queuedMoveIntentRef.current = null;
      return false;
    }

    const blockedSteps = recentMovementBlockedSteps(movementBlockedStepsRef.current, now);
    const requestedMode = queued.requestedMode;
    const effectiveMode = crystalEffectiveMovementMode(requestedMode, now);
    const nextAction =
      queued.kind === "direction" && queued.direction
        ? crystalMovementActionForDirection(serverSelf, queued.direction, effectiveMode, blockedSteps, currentWorld)
        : queued.kind === "target" &&
            queued.targetX !== undefined &&
            queued.targetY !== undefined &&
            (serverSelf.x !== queued.targetX || serverSelf.y !== queued.targetY)
          ? crystalMovementActionTowardWithRouteHints(
              serverSelf,
              { x: queued.targetX, y: queued.targetY },
              effectiveMode,
              blockedSteps,
              currentWorld,
            )
          : null;

    if (!nextAction) {
      queuedMoveIntentRef.current = null;
      return false;
    }

    const nextPoint = nextAction.point;
    if (nextPoint.x === serverSelf.x && nextPoint.y === serverSelf.y) {
      if (nextAction.direction && nextAction.direction !== serverSelf.direction) {
        if (queued.consumeAfterSend || queued.kind === "target") {
          queuedMoveIntentRef.current = null;
        }
        return sendCrystalTurn(nextAction.direction);
      }
      if (nextAction.direction) {
        rememberBlockedDirectionAtSource(serverSelf.x, serverSelf.y, nextAction.direction, now);
      }
      queuedMoveIntentRef.current = null;
      clearLocalSelfPrediction();
      return false;
    }

    const pendingMove = createPendingSelfMove({
      from: { x: serverSelf.x, y: serverSelf.y, direction: serverSelf.direction },
      direction: nextAction.direction,
      requestedMode: nextAction.mode,
      now,
      runPrimedUntil: crystalRunPrimedUntilRef.current,
    });
    const alignedPending: PendingSelfMove = {
      ...pendingMove,
      to: { x: nextPoint.x, y: nextPoint.y, direction: nextAction.direction },
      mode: nextAction.mode,
      visualUntil: now + movementStepIntervalMs(nextAction.mode),
    };
    pendingSelfMoveRef.current = alignedPending;
    pendingSelfTurnRef.current = null;
    nextMoveSendAtRef.current = now + movementCommandDelayMs(alignedPending.mode);
    directionStepNextAtRef.current = nextMoveSendAtRef.current;
    directionStepVisualUntilRef.current = alignedPending.visualUntil;
    clearLegacySelfMovementCoordinateSources();
    setPredictedPlayerMotion(alignedPending.to, alignedPending.visualUntil);

    if (queued.consumeAfterSend) {
      queuedMoveIntentRef.current = null;
    }
    const sent = send({ type: alignedPending.mode === "run" ? "run" : "walk", direction: alignedPending.direction });
    if (!sent) {
      pendingSelfMoveRef.current = null;
      nextMoveSendAtRef.current = now;
      clearLocalSelfPrediction();
      return false;
    }
    scheduleMovementConfirmTick();
    return true;
  }

  function moveToTile(x: number, y: number, mode: "walk" | "run", _packetMode: "target" | "direction" = "direction") {
    queueCrystalMoveIntent({
      kind: "target",
      targetX: x,
      targetY: y,
      requestedMode: mode,
      requestedAt: Date.now(),
      consumeAfterSend: false,
    });
  }

  function attackTarget(objectId: string) {
    send({ type: "attack", objectId: Number(objectId) });
  }

  // Harvest a corpse/resource in the given facing direction (ClientPacket::Harvest).
  function harvestToward(direction: string) {
    send({ type: "harvest", direction });
  }

  // Pick up whatever item is on the local player's own tile (ClientPacket::PickUp,
  // which carries no location — the server uses the player's current position).
  function pickUpUnderfoot() {
    send({ type: "pickUpTile" });
  }

  function createCharacter(draft: {
    name: string;
    classKey: SelectCharacterEntry["classKey"];
    gender: SelectCharacterEntry["gender"];
  }) {
    send({
      type: "newCharacter",
      name: draft.name,
      gender: draft.gender,
      class: draft.classKey,
    });
  }

  function deleteSelectedCharacter() {
    const selected = characters[selectedCharacterIndex] ?? characters[0];
    if (!selected) return;
    send({ type: "deleteCharacter", characterIndex: selected.index });
  }

  function useItem(item: ItemCommandRef) {
    (window as typeof window & { __mir2LastUseItem?: Record<string, unknown> }).__mir2LastUseItem = {
      key: item.key,
      uniqueId: item.uniqueId,
      slot: item.slot,
      container: item.container,
      at: Date.now(),
    };
    send({
      type: "useItem",
      key: item.key,
      uniqueId: item.uniqueId,
      slot: item.slot,
      grid: item.container === "belt" ? "belt" : item.container === "quest" ? "questInventory" : "inventory",
    });
  }

  function dropItem(item: ItemCommandRef) {
    send({
      type: "dropItem",
      key: item.key,
      uniqueId: item.uniqueId,
      count: 1,
      heroInventory: false,
    });
  }

  function equipItem(item: ItemCommandRef, slot: EquipmentSlot) {
    send({
      type: "equipItem",
      uniqueId: item.uniqueId,
      grid:
        item.container === "belt"
          ? "belt"
          : item.container === "quest"
            ? "questInventory"
            : "inventory",
      to: equipmentSlotIndex(slot),
    });
  }

  function removeItem(item: EquipmentCommandRef) {
    const occupiedBagSlots = new Set(
      world.inventoryItems.filter((entry) => entry.container === "bag1").map((entry) => entry.slot),
    );
    const targetSlot =
      Array.from({ length: Math.max(world.maxBagSlots, 1) }, (_, slot) => slot).find(
        (slot) => !occupiedBagSlots.has(slot),
      ) ?? 0;
    send({
      type: "removeItem",
      uniqueId: equipmentSlotIndex(item.slot),
      grid: "inventory",
      to: targetSlot,
    });
  }

  function moveItem(item: ItemMoveRef, toSlot: number) {
    const from =
      item.container === "bag1" || item.container === "bag2" || item.container === "quest"
        ? (item.uniqueId ?? (item.container === "bag2" ? 40 + item.slot : item.slot))
        : item.slot;
    send({
      type: "moveItem",
      grid:
        item.container === "belt"
          ? "belt"
          : item.container === "storage"
            ? "storage"
          : item.container === "quest"
            ? "questInventory"
            : "inventory",
      from,
      to: toSlot,
    });
  }

  function mergeItem(from: ItemMergeRef, to: ItemMergeRef) {
    send({
      type: "mergeItem",
      gridFrom:
        from.container === "belt"
          ? "belt"
          : from.container === "storage"
            ? "storage"
          : from.container === "quest"
            ? "questInventory"
            : "inventory",
      gridTo:
        to.container === "belt"
          ? "belt"
          : to.container === "storage"
            ? "storage"
          : to.container === "quest"
            ? "questInventory"
            : "inventory",
      idFrom: from.uniqueId,
      idTo: to.uniqueId,
    });
  }

  function splitItem(item: ItemCommandRef, count: number) {
    send({
      type: "splitItem",
      uniqueId: item.uniqueId,
      grid:
        item.container === "belt"
          ? "belt"
          : item.container === "storage"
            ? "storage"
          : item.container === "quest"
            ? "questInventory"
            : "inventory",
      count,
    });
  }

  function storeItem(item: ItemMoveRef, toSlot: number) {
    send({
      type: "storeItem",
      from: item.slot,
      to: toSlot,
    });
  }

  function takeBackItem(item: ItemMoveRef, toSlot: number) {
    send({
      type: "takeBackItem",
      from: item.slot,
      to: toSlot,
    });
  }

  function unlockStorage(storagePassword: string) {
    send({
      type: "unlockStorage",
      password: storagePassword,
    });
  }

  function setStoragePassword(currentPassword: string, newPassword: string) {
    send({
      type: "setStoragePassword",
      currentPassword,
      newPassword,
    });
  }

  function removeStoragePassword(currentPassword: string) {
    send({
      type: "removeStoragePassword",
      currentPassword,
    });
  }

  function rentExpandedStorage() {
    send({
      type: "chat",
      message: "@ADDSTORAGE",
    });
  }

  function sellItem(item: ItemCommandRef, count: number) {
    send({
      type: "sellItem",
      uniqueId: item.uniqueId,
      count,
    });
  }

  function dropGold(amount: number) {
    send({
      type: "dropGold",
      amount,
    });
  }

  function repairItem(item: EquipmentCommandRef) {
    send({
      type: "repairItem",
      uniqueId: equipmentSlotIndex(item.slot),
    });
  }

  function specialRepairItem(item: EquipmentCommandRef) {
    send({
      type: "specialRepairItem",
      uniqueId: equipmentSlotIndex(item.slot),
    });
  }

  function spellNameForSkill(skill: KnownSkill) {
    return skill.spell || skill.name || skill.key;
  }

  function sendMagicSkill(skill: KnownSkill, target: { x: number; y: number; objectId?: string } | null, direction?: string) {
    const origin = self ?? world.entities.find((entity) => entity.objectId === world.playerObjectId) ?? null;
    const castTarget = target ?? origin;
    if (!origin || !castTarget) {
      return false;
    }
    return send({
      type: "magic",
      spell: spellNameForSkill(skill),
      direction: direction ?? origin.direction ?? "Down",
      targetId: castTarget.objectId ? Number(castTarget.objectId) : 0,
      x: castTarget.x,
      y: castTarget.y,
      spellTargetLock: Boolean(castTarget.objectId),
    });
  }

  function castSkillAtTile(skill: KnownSkill, x: number, y: number) {
    const origin = self ?? world.entities.find((entity) => entity.objectId === world.playerObjectId) ?? null;
    if (!origin) return;
    sendMagicSkill(skill, { x, y }, directionFromPoint(origin, { x, y }, origin.direction ?? "Down"));
  }

  function castSkill(skillKey: string) {
    const skill = world.knownSkills.find((entry) => entry.key === skillKey);
    if (!skill) {
      send({ type: "castSkill", key: skillKey });
      return;
    }
    if (skill.castKind === "passive") {
      appendLog(`${skill.name} is passive.`, "system");
      return;
    }
    if (!skill.spell) {
      send({ type: "castSkill", key: skillKey });
      return;
    }
    if (skill.castKind === "ground") {
      pendingGroundSkillRef.current = skill;
      appendLog(`Select a ground tile for ${skill.name}.`, "system");
      return;
    }
    if (skill.castKind === "toggle" && skill.spell) {
      send({ type: "spellToggle", spell: skill.spell, toggleState: 1 });
      return;
    }
    if (skill.castKind === "direction") {
      const origin = self ?? world.entities.find((entity) => entity.objectId === world.playerObjectId) ?? null;
      sendMagicSkill(skill, origin ? { x: origin.x, y: origin.y } : null, origin?.direction);
      return;
    }
    if (skill.castKind === "target" || (!skill.castKind && skill.offensive)) {
      const target = selectedEntity && !selectedEntity.dead ? selectedEntity : null;
      if (!target || (skill.offensive && target.kind !== "monster")) {
        appendLog(`Select a target for ${skill.name}.`, "system");
        return;
      }
      sendMagicSkill(skill, target, directionToward(self, target));
      return;
    }
    sendMagicSkill(skill, self ? { x: self.x, y: self.y, objectId: self.objectId } : null, self?.direction);
  }

  function transferMap(key: string) {
    pendingTransferRef.current = null;
    crystalRunPrimedUntilRef.current = 0;
    send({ type: "transferMap", key });
  }

  function claimMail(mailId: number) {
    // Real protocol packets first (ReadMail marks the mail opened, CollectParcel
    // pulls the gold/items), then the stage5 action channel as a fallback for the
    // dev gateway's in-process mailbox. Field shapes match BrowserCommand::ReadMail
    // / CollectParcel (`mailId`) in apps/gateway/src/web.rs.
    send({ type: "readMail", mailId }, { quiet: true });
    send({ type: "collectParcel", mailId }, { quiet: true });
    send({ type: "stage5Command", action: "mail.claim", args: [String(mailId)] });
  }

  function deleteMail(mailId: number) {
    send({ type: "deleteMail", mailId }, { quiet: true });
    send({ type: "stage5Command", action: "mail.delete", args: [String(mailId)] });
  }

  function buyGameShopItem(gameShopIndex: number, quantity: number, paymentType: "gold" | "credit") {
    send({
      type: "stage5Command",
      action: paymentType === "credit" ? "gameShop.buyCredit" : "gameShop.buyGold",
      args: [String(gameShopIndex), String(quantity)],
    });
  }

  function runStage5Command(action: string, args: string[] = []) {
    send({ type: "stage5Command", action, args });
  }

  function sendClientCommand(command: Record<string, unknown>) {
    send(command);
  }

  // ---------------------------------------------------------------------------
  // ExtraWindows action handlers
  //
  // Each handler maps a Crystal UI window button to an outbound BrowserCommand
  // whose field shapes match `BrowserCommand` in apps/gateway/src/web.rs (which
  // forwards to the `ClientPacket` enum in packages/protocol/src/packets.rs).
  // Window actions that have no real ClientPacket / BrowserCommand are left
  // unwired so the window keeps the button disabled (see the <ExtraWindows>
  // mount + the task report for the omitted list).
  // ---------------------------------------------------------------------------

  // Ranking: a window tab maps to a `(rankType, onlineOnly)` pair, matching the
  // `rankingTabKey` adapter. `getRanking` -> ClientPacket::GetRanking; rankIndex
  // 0 requests the first page.
  function rankingRequestForTab(tab: RankingTabKey): { rankType: number; onlineOnly: boolean } {
    switch (tab) {
      case "warrior":
        return { rankType: 1, onlineOnly: false };
      case "wizard":
        return { rankType: 2, onlineOnly: false };
      case "taoist":
        return { rankType: 3, onlineOnly: false };
      case "assassin":
        return { rankType: 4, onlineOnly: false };
      case "archer":
        return { rankType: 5, onlineOnly: false };
      case "online":
        return { rankType: 0, onlineOnly: true };
      case "overall":
      default:
        return { rankType: 0, onlineOnly: false };
    }
  }

  function requestRanking(tab: RankingTabKey) {
    const { rankType, onlineOnly } = rankingRequestForTab(tab);
    send({ type: "getRanking", rankType, rankIndex: 0, onlineOnly });
  }

  // Friends: the stage-5 social roster carries no character index, so the friend
  // index that ClientPacket::RemoveFriend expects is derived from the displayed
  // ordering (`friends` then `blocked`), matching the gateway's
  // `stage5_friend_entries` enumeration.
  function friendCharacterIndex(name: string): number | null {
    const social = world.stage5Systems.social;
    const friends = Array.isArray(social?.friends) ? social.friends : [];
    const blocked = Array.isArray(social?.blocked) ? social.blocked : [];
    // social entries may be bare names (legacy) or rich objects (B-wave); compare by name.
    const entryName = (entry: string | { name?: string }) =>
      (typeof entry === "string" ? entry : entry?.name ?? "").toLowerCase();
    const target = name.toLowerCase();
    const friendIdx = friends.findIndex((entry) => entryName(entry) === target);
    if (friendIdx >= 0) return friendIdx;
    const blockedIdx = blocked.findIndex((entry) => entryName(entry) === target);
    if (blockedIdx >= 0) return friends.length + blockedIdx;
    return null;
  }

  function addFriend(name: string) {
    const trimmed = name.trim();
    if (!trimmed) return;
    send({ type: "addFriend", name: trimmed, blocked: false });
  }

  function blockPlayer(name: string) {
    const trimmed = name.trim();
    if (!trimmed) return;
    send({ type: "addFriend", name: trimmed, blocked: true });
  }

  // RemoveFriend resolves the entry by index and drops it from both the friend
  // and block lists, so it covers "remove friend" and "unblock" alike.
  function removeFriendEntry(name: string) {
    const characterIndex = friendCharacterIndex(name);
    if (characterIndex === null) return;
    send({ type: "removeFriend", characterIndex });
  }

  // Market: stage-5 auction listing ids are surfaced as strings; the
  // MarketBuy / MarketGetBack packets key off the same numeric listing id.
  function marketBuyListing(listingId: string) {
    const auctionId = Number(listingId);
    if (!Number.isFinite(auctionId)) return;
    send({ type: "marketBuy", auctionId, bidPrice: 0 });
  }

  function marketCancelListing(listingId: string) {
    const auctionId = Number(listingId);
    if (!Number.isFinite(auctionId)) return;
    // mode 0 == "get back" (the gateway ignores the mode discriminator).
    send({ type: "marketGetBack", mode: 0, auctionId });
  }

  function marketSearch(query: string) {
    send({ type: "marketSearch", matchText: query.trim() });
  }

  function marketRefresh() {
    send({ type: "marketRefresh" });
  }

  // Bonds (relationship + mentor). MarriageRequest targets the faced player
  // server-side, so the window's typed name is not part of the packet.
  function proposeMarriage(_name: string) {
    send({ type: "marriageRequest" });
  }

  function divorce() {
    send({ type: "divorceRequest" });
  }

  // ChangeMarriage toggles whether the player accepts incoming proposals.
  function toggleAllowMarriage(_allow: boolean) {
    send({ type: "changeMarriage" });
  }

  function addMentor(name: string) {
    const trimmed = name.trim();
    if (!trimmed) return;
    send({ type: "addMentor", name: trimmed });
  }

  function allowMentor(_allow: boolean) {
    send({ type: "allowMentor" });
  }

  function cancelMentor() {
    send({ type: "cancelMentor" });
  }

  // Guild chat rides the real Chat packet: the "!~" prefix routes to guild chat
  // server-side (apps/simulation/.../zone/runtime.rs).
  function sendGuildChat(message: string) {
    const trimmed = message.trim();
    if (!trimmed) return;
    send({ type: "chat", message: `!~${trimmed}` });
  }

  // Guild notice editing -> ClientPacket::EditGuildNotice (the gateway forwards
  // BrowserCommand::EditGuildNotice). The notice is a list of lines, so the
  // multiline draft is split on newlines (matching the C# client's per-line
  // notice array); blank trailing lines are dropped.
  function editGuildNotice(notice: string) {
    const lines = notice.replace(/\r\n/g, "\n").split("\n");
    while (lines.length > 0 && lines[lines.length - 1].trim() === "") {
      lines.pop();
    }
    send({ type: "editGuildNotice", notice: lines });
  }

  // Guild recruit/kick both ride ClientPacket::EditGuildMember. `changeType` 0
  // recruits/adds the named player, 1 removes them (see
  // stage5_edit_guild_member_packet in apps/simulation/.../runtime/packets.rs).
  function inviteGuildMember(name: string) {
    const trimmed = name.trim();
    if (!trimmed) return;
    send({ type: "editGuildMember", changeType: 0, rankIndex: 0, name: trimmed, rankName: "" });
  }

  function kickGuildMember(name: string) {
    const trimmed = name.trim();
    if (!trimmed) return;
    send({ type: "editGuildMember", changeType: 1, rankIndex: 0, name: trimmed, rankName: "" });
  }

  // Whisper rides the real Chat packet: a "/name body" message routes to a
  // private whisper server-side (apps/simulation/.../zone/runtime.rs). The
  // friends window only supplies a target name, so seed the chat input with the
  // "/name " prefix and let the player type the body (matching the original
  // Crystal client's whisper-compose flow).
  function whisperPlayer(name: string) {
    const trimmed = name.trim();
    if (!trimmed) return;
    setChatMessage(`/${trimmed} `);
  }

  // Group invite rides the real protocol: AddMember requires grouping to be
  // enabled first, so SwitchGroup(true) precedes AddMember (matching
  // stage5_group_add_member_packet in apps/simulation/.../runtime/packets.rs).
  function groupInviteMember(name: string) {
    const trimmed = name.trim();
    if (!trimmed) return;
    send({ type: "switchGroup", allowGroup: true }, { quiet: true });
    send({ type: "addMember", name: trimmed });
  }

  // Kicking a member rides ClientPacket::DelMember.
  function kickGroupMember(name: string) {
    const trimmed = name.trim();
    if (!trimmed) return;
    send({ type: "delMember", name: trimmed });
  }

  // Leaving / disbanding the group rides ClientPacket::SwitchGroup with
  // allowGroup=false, which the simulation maps to clearing the roster +
  // DeleteGroup. The stage-5 group.leave channel is kept as a dev fallback.
  function groupLeave() {
    send({ type: "switchGroup", allowGroup: false }, { quiet: true });
    send({ type: "stage5Command", action: "group.leave" });
  }

  // Toggling whether the player accepts group invites rides SwitchGroup's
  // allowGroup flag directly.
  function groupToggleAllowInvites(allow: boolean) {
    send({ type: "switchGroup", allowGroup: allow });
  }

  // Loot mode has no dedicated ClientPacket; it stays on the stage-5 channel.
  function groupToggleLootMode() {
    const current = world.stage5Systems.group?.lootMode;
    const next = current === "group" ? "solo" : "group";
    send({ type: "stage5Command", action: "group.loot", args: [next] });
  }

  function conquestStartWar() {
    send({ type: "stage5Command", action: "conquest.start" });
  }

  // Quest log actions. The window's "track" button shares the quest with the
  // group (ClientPacket::ShareQuest); "abandon" drops it (AbandonQuest). Both
  // key off the same `questIndex` the simulation tracks.
  function shareQuest(questId: number) {
    send({ type: "shareQuest", questIndex: questId });
  }

  function abandonQuest(questId: number) {
    send({ type: "abandonQuest", questIndex: questId });
  }

  // Hero summon rides ClientPacket::ChangeHero, which spawns the recruited hero
  // beside the player (stage5 ChangeHero handler). There is no dedicated hero
  // "dismiss/recall" packet in the simulation yet, so onDismissHero is left
  // unwired (button stays disabled) — see the task report follow-up.
  function summonHero() {
    send({ type: "changeHero", listIndex: 0 });
  }

  // Intelligent-creature summon/dismiss/release ride ClientPacket::
  // UpdateIntelligentCreature. The packet echoes the full creature record, so
  // look it up from the stage-5 list by the slot index encoded in the window's
  // `creature-<slot>` id. summonMe sets petMode>=1, unsummonMe clears it,
  // releaseMe removes the creature entirely (see
  // stage5_update_intelligent_creature_packet in the simulation).
  function intelligentCreatureRecord(creatureId: string): Record<string, unknown> | null {
    const list = world.stage5Systems.intelligentCreatures;
    if (!Array.isArray(list)) return null;
    const slotText = creatureId.startsWith("creature-") ? creatureId.slice("creature-".length) : creatureId;
    const slot = Number(slotText);
    const match = Number.isFinite(slot)
      ? list.find((entry) => Number((entry as Record<string, unknown>)?.slotIndex) === slot)
      : undefined;
    return (match ?? null) as Record<string, unknown> | null;
  }

  function summonCreature(creatureId: string) {
    const creature = intelligentCreatureRecord(creatureId);
    if (!creature) return;
    send({ type: "updateIntelligentCreature", creature, summonMe: true, unsummonMe: false, releaseMe: false });
  }

  function releaseCreature(creatureId: string) {
    const creature = intelligentCreatureRecord(creatureId);
    if (!creature) return;
    send({ type: "updateIntelligentCreature", creature, summonMe: false, unsummonMe: false, releaseMe: true });
  }

  // Cycling the pet mode re-sends the creature with the next petMode (0..5,
  // matching the original PetMode enum) and no summon/release flags, so the
  // simulation just updates the stored record.
  function cycleCreaturePickupMode(creatureId: string) {
    const creature = intelligentCreatureRecord(creatureId);
    if (!creature) return;
    const currentMode = Number(creature.petMode ?? 0);
    const nextMode = Number.isFinite(currentMode) ? (currentMode + 1) % 6 : 1;
    send({
      type: "updateIntelligentCreature",
      creature: { ...creature, petMode: nextMode },
      summonMe: false,
      unsummonMe: false,
      releaseMe: false,
    });
  }

  // Trade window actions ride the real trade packets: accept an incoming invite
  // (TradeReply), lock in your side (TradeConfirm), or cancel the trade
  // (TradeCancel).
  function acceptTrade() {
    send({ type: "tradeReply", acceptInvite: true });
  }

  function confirmTrade() {
    send({ type: "tradeConfirm", locked: true });
  }

  function cancelTrade() {
    send({ type: "tradeCancel" });
  }

  // ---- wave-2: additional window action wiring ----

  // Hero AI behaviour -> ClientPacket::SetHeroBehaviour. HeroBehaviourKey maps
  // positionally onto Crystal's HeroBehaviour enum; the sim stores + echoes the
  // value, so the round-trip is faithful even if the ordinal differs.
  function setHeroBehaviour(behaviour: "attack" | "counterAttack" | "follow" | "custom") {
    const ordinal = { attack: 0, counterAttack: 1, follow: 2, custom: 3 } as const;
    send({ type: "setHeroBehaviour", behaviour: ordinal[behaviour] ?? 0 });
  }

  // Recall the active hero. Crystal has no dedicated recall packet; recall rides
  // ChangeHero (toggles the hero in/out beside the player), per the protocol audit.
  function recallHero() {
    send({ type: "changeHero", listIndex: 0 });
  }

  // Guild storage gold: GuildStorageGoldChange changeType 0 = deposit, 1 = withdraw.
  function guildDepositGold(amount: number) {
    if (!Number.isFinite(amount) || amount <= 0) return;
    send({ type: "guildStorageGoldChange", changeType: 0, amount: Math.floor(amount) });
  }
  function guildWithdrawGold(amount: number) {
    if (!Number.isFinite(amount) || amount <= 0) return;
    send({ type: "guildStorageGoldChange", changeType: 1, amount: Math.floor(amount) });
  }

  // Assign a member to a rank -> EditGuildMember changeType 4 (carries rankIndex).
  function changeGuildMemberRank(name: string, rankIndex: number) {
    const trimmed = name.trim();
    if (!trimmed) return;
    send({ type: "editGuildMember", changeType: 4, rankIndex, name: trimmed, rankName: "" });
  }

  // Save a guild rank: rename (changeType 2 carries rankName) then push each
  // permission flag (changeType 5: rankName = option index 0..7, name = "true"/"false").
  // permissions is the list of ENABLED permission keys for the rank; each of the
  // 8 Crystal options is pushed true/false via EditGuildMember changeType 5.
  function saveGuildRank(rankIndex: number, name: string, permissions: string[]) {
    const trimmed = name.trim();
    if (trimmed) {
      send(
        { type: "editGuildMember", changeType: 2, rankIndex, name: "", rankName: trimmed },
        { quiet: true },
      );
    }
    const optionByKey: Record<string, number> = {
      changeRank: 0,
      recruit: 1,
      kick: 2,
      storeItem: 3,
      retrieveItem: 4,
      alterAlliance: 5,
      changeNotice: 6,
      activateBuff: 7,
    };
    const enabled = new Set(permissions);
    for (const [key, option] of Object.entries(optionByKey)) {
      send(
        {
          type: "editGuildMember",
          changeType: 5,
          rankIndex,
          name: enabled.has(key) ? "true" : "false",
          rankName: String(option),
        },
        { quiet: true },
      );
    }
  }

  // Friend memo -> AddMemo, keyed by the same derived friend index as RemoveFriend.
  function editFriendMemo(name: string, memo: string) {
    const characterIndex = friendCharacterIndex(name);
    if (characterIndex === null) return;
    send({ type: "addMemo", characterIndex, memo });
  }

  // Ranking online-only toggle: re-request the active board with the flag.
  function setRankingOnlineOnly(onlineOnly: boolean) {
    send({ type: "getRanking", rankType: 0, rankIndex: 0, onlineOnly });
  }

  // Trade gold offer -> TradeGold.
  function setTradeGold(amount: number) {
    if (!Number.isFinite(amount) || amount < 0) return;
    send({ type: "tradeGold", amount: Math.floor(amount) });
  }

  // Mail actions (ReadMail / CollectParcel / DeleteMail / SendMail).
  function openMailMessage(mailId: number) {
    send({ type: "readMail", mailId });
  }
  function claimMailAttachment(mailId: number) {
    send({ type: "collectParcel", mailId });
  }
  function deleteMailMessage(mailId: number) {
    send({ type: "deleteMail", mailId });
  }
  function sendMailMessage(draft: MailComposeDraft) {
    const name = draft.to.trim();
    if (!name) return;
    send({
      type: "sendMail",
      name,
      message: draft.body ?? "",
      gold: Math.max(0, Math.floor(draft.gold ?? 0)),
      itemsIdx: [0, 0, 0, 0, 0],
      stamped: false,
    });
  }
  // Friend "mail" affordance just opens the mail window (compose recipient typed there).
  function openMailWindow(_name?: string) {
    setShowMail(true);
  }

  function transferKeyForTile(x: number, y: number) {
    return transferKeyForWorldTile(world.mapTransfers, world.mapFileName, x, y);
  }

  function interactTarget(objectId: string) {
    send({ type: "interact", objectId: Number(objectId) });
  }

  function pickGroundDrop(objectId: string) {
    send({ type: "pickUp", objectId: Number(objectId) });
  }

  function selectEntity(objectId: string) {
    setWorld((current) => ({
      ...current,
      selectedObjectId: current.selectedObjectId === objectId ? null : objectId,
    }));
  }

  function activateEntity(objectId: string) {
    const entity = world.entities.find((entry) => entry.objectId === objectId);
    setWorld((current) => ({
      ...current,
      selectedObjectId: objectId,
    }));

    if (!entity || entity.dead) {
      return;
    }

    if (entity.kind === "npc") {
      if (tileDistance(self, entity) > 1) {
        pendingNpcInteractRef.current = objectId;
        const destination = approachDestination(self, entity);
        moveToTile(destination.x, destination.y, "run");
        return;
      }

      const now = Date.now();
      const existingGuard = npcCallGuardRef.current;
      const sameDialogVisible = world.activeNpcDialog?.npcObjectId === objectId;
      if (existingGuard?.objectId === objectId && (sameDialogVisible || now <= existingGuard.until)) {
        return;
      }
      npcCallGuardRef.current = { objectId, until: now + 650 };
      interactTarget(objectId);
      return;
    }

    if (entity.kind === "monster") {
      attackTarget(objectId);
    }
  }

  function openCharacter(tab: "char" | "stats1" | "stats2" | "spells") {
    setActiveCharacterTab(tab);
    setShowCharacter(true);
  }

  function openInventory(tab: "bag1" | "bag2" | "quest") {
    setActiveInventoryTab(tab);
    setShowInventory(true);
  }

  function handleViewportTileAction(x: number, y: number, mode: "walk" | "run") {
    if (sceneInputDeferredForInitialAssets()) {
      return;
    }
    const groundSkill = pendingGroundSkillRef.current;
    if (groundSkill) {
      pendingGroundSkillRef.current = null;
      castSkillAtTile(groundSkill, x, y);
      return;
    }
    const occupant = world.entities.find(
      (entity) => entity.objectId !== world.playerObjectId && !entity.dead && entity.x === x && entity.y === y,
    );
    if (occupant) {
      activateEntity(occupant.objectId);
      return;
    }
    const drop = world.groundDrops.find((entry) => entry.x === x && entry.y === y);
    if (drop) {
      // If the drop is on the player's own tile, use the underfoot PickUp packet
      // (no location); otherwise target the specific ground-drop object.
      const standingSelf = world.entities.find((entity) => entity.objectId === world.playerObjectId);
      if (standingSelf && standingSelf.x === x && standingSelf.y === y) {
        pickUpUnderfoot();
      } else {
        pickGroundDrop(drop.objectId);
      }
      return;
    }
    const transferKey = transferKeyForTile(x, y);
    if (transferKey) {
      pendingTransferRef.current = transferKey;
      moveToTile(x, y, mode);
      return;
    }
    moveToTile(x, y, mode);
  }

  function handleViewportTileStepAction(x: number, y: number, mode: "walk" | "run") {
    if (sceneInputDeferredForInitialAssets()) {
      return;
    }
    const currentWorld = worldRef.current;
    const serverSelf = currentWorld.entities.find((entity) => entity.objectId === currentWorld.playerObjectId) ?? self;
    if (!serverSelf) return;
    const currentPlan = movementPlanRef.current;
    const now = Date.now();
    if (
      currentPlan?.packetMode === "direction" &&
      (movementPlanBlockedByActionCadence(currentPlan, serverSelf, now) || now < currentPlan.nextStepAt)
    ) {
      moveToTile(x, y, mode, "direction");
      return;
    }
    const currentSelf =
      currentPlan?.actionX !== undefined &&
      currentPlan.actionY !== undefined &&
      Math.max(Math.abs(serverSelf.x - currentPlan.actionX), Math.abs(serverSelf.y - currentPlan.actionY)) <=
        MOVEMENT_LOCAL_ACTION_MAX_LEAD_TILES
        ? { x: currentPlan.actionX, y: currentPlan.actionY }
        : serverSelf;

    const nextPoint = crystalMovementActionToward(currentSelf, { x, y }, mode).point;
    const occupant = currentWorld.entities.find(
      (entity) =>
        entity.objectId !== currentWorld.playerObjectId && !entity.dead && entity.x === nextPoint.x && entity.y === nextPoint.y,
    );
    if (occupant) {
      activateEntity(occupant.objectId);
      return;
    }
    const drop = currentWorld.groundDrops.find((entry) => entry.x === nextPoint.x && entry.y === nextPoint.y);
    if (drop) {
      pickGroundDrop(drop.objectId);
      return;
    }
    const transferKey = transferKeyForTile(nextPoint.x, nextPoint.y);
    if (transferKey) {
      pendingTransferRef.current = transferKey;
    }
    moveToTile(nextPoint.x, nextPoint.y, mode, "direction");
  }

  function handleViewportDirectionStep(x: number, y: number, mode: "walk" | "run") {
    if (sceneInputDeferredForInitialAssets()) {
      return;
    }
    queueCrystalMoveIntent({
      kind: "target",
      targetX: x,
      targetY: y,
      requestedMode: mode,
      requestedAt: Date.now(),
      consumeAfterSend: true,
    });
  }

  function handleViewportDirectionIntent(
    direction: string,
    mode: "walk" | "run",
    options?: { discrete?: boolean },
  ) {
    if (sceneInputDeferredForInitialAssets()) {
      return;
    }
    queueCrystalMoveIntent({
      kind: "direction",
      direction,
      requestedMode: mode,
      requestedAt: Date.now(),
      consumeAfterSend: options?.discrete === true,
    });
  }

  function handleViewportDirectionStop() {
    if (queuedMoveIntentRef.current?.kind === "direction") {
      queuedMoveIntentRef.current = null;
    }
  }

  function consumeQueuedDirectionStep() {
    if (!queuedDirectionStepRef.current && queuedDirectionStepBacklogRef.current.length === 0) {
      return false;
    }
    queuedDirectionStepRef.current = null;
    queuedDirectionStepBacklogRef.current = [];
    clearDirectionStepPendingQueue();
    return false;
  }

  function promoteQueuedDirectionStepBacklog(now = Date.now()) {
    while (queuedDirectionStepBacklogRef.current.length > 0) {
      const next = queuedDirectionStepBacklogRef.current.shift() ?? null;
      if (!next) continue;
      if (now - next.requestedAt <= queuedDirectionStepMaxAgeMs(next)) {
        queuedDirectionStepRef.current = next;
        return true;
      }
    }
    return false;
  }

  function queuedDirectionStepMaxAgeMs(queued: DirectionStepRequest) {
    return (queued.repeatCount ?? 1) > 1
      ? MOVEMENT_QUEUED_DIRECTION_REPEAT_MAX_AGE_MS
      : MOVEMENT_QUEUED_DIRECTION_MAX_AGE_MS;
  }

  function localMovementActionSource(serverSelf: WorldEntity | null, currentPlan: MovementPlan | null) {
    if (!serverSelf) {
      return null;
    }

    const planSource =
      currentPlan?.actionX !== undefined && currentPlan.actionY !== undefined
        ? { x: currentPlan.actionX, y: currentPlan.actionY, direction: predictedPlayerPositionRef.current?.direction }
        : null;
    const pendingQueue = directionStepPendingQueueRef.current;
    const queuedSource = pendingQueue[pendingQueue.length - 1] ?? null;
    const predictedSource = predictedPlayerPositionRef.current;
    const anchorSource = activeLocalMovementAnchor(serverSelf);
    const actionFeedSource = activeCrystalSelfActionSource(serverSelf);

    for (const candidate of [planSource, actionFeedSource, queuedSource, predictedSource, anchorSource]) {
      if (!candidate) {
        continue;
      }
      const lead = Math.max(Math.abs(candidate.x - serverSelf.x), Math.abs(candidate.y - serverSelf.y));
      if (
        lead > 0 &&
        lead <= MOVEMENT_LOCAL_ACTION_MAX_LEAD_TILES &&
        crystalMovementCandidateNotBehindServer(serverSelf, candidate, candidate.direction ?? serverSelf.direction)
      ) {
        return {
          x: candidate.x,
          y: candidate.y,
          direction: candidate.direction ?? serverSelf.direction,
        };
      }
    }

    return null;
  }

  function activeDirectionStepVisualSource(serverSelf: { x: number; y: number; direction?: string } | null) {
    if (!serverSelf) {
      return null;
    }
    const pending =
      directionStepPendingQueueRef.current[directionStepPendingQueueRef.current.length - 1] ??
      directionStepPendingRef.current;
    if (!pending) {
      return null;
    }
    const sentFrom =
      pending.sentFromX !== undefined && pending.sentFromY !== undefined
        ? {
            x: pending.sentFromX,
            y: pending.sentFromY,
            direction: pending.direction ?? serverSelf.direction,
          }
        : null;
    const pendingPoint = {
      x: pending.x,
      y: pending.y,
      direction: pending.direction ?? serverSelf.direction,
    };
    return chooseCrystalSelfRenderPosition(serverSelf, sentFrom, pendingPoint);
  }

  function movementPlanPendingStillAhead(plan: MovementPlan | null, serverSelf: WorldEntity | null) {
    if (!plan || plan.pendingX === undefined || plan.pendingY === undefined) {
      return false;
    }
    if (!serverSelf) {
      return true;
    }
    return plan.pendingX !== serverSelf.x || plan.pendingY !== serverSelf.y;
  }

  function lastSelfMovementReadyAt(serverSelf: { x: number; y: number } | null, mode: "walk" | "run") {
    const ack = lastSelfMovementAckRef.current;
    if (!ack || !serverSelf || ack.x !== serverSelf.x || ack.y !== serverSelf.y) {
      return 0;
    }
    return ack.at + MOVEMENT_CONFIRM_TICK_DELAY_MS;
  }

  function pendingDirectionStepStillAhead(serverSelf: WorldEntity | null) {
    const pending = directionStepPendingQueueRef.current[0] ?? directionStepPendingRef.current;
    if (!pending) {
      return null;
    }
    if (!serverSelf) {
      return pending;
    }
    if (pending.x !== serverSelf.x || pending.y !== serverSelf.y) {
      return pending;
    }
    if (pending.direction && serverSelf.direction && pending.direction !== serverSelf.direction) {
      return pending;
    }
    return null;
  }

  function pendingPredictedPlayerStepStillAhead(
    serverSelf: WorldEntity | null,
    mode: "walk" | "run",
    now: number,
  ) {
    const predicted = predictedPlayerPositionRef.current;
    if (!predicted || !serverSelf || directionStepVisualUntilRef.current <= now) {
      return null;
    }
    if (predicted.x === serverSelf.x && predicted.y === serverSelf.y) {
      return null;
    }
    const lead = Math.max(Math.abs(predicted.x - serverSelf.x), Math.abs(predicted.y - serverSelf.y));
    if (lead > MOVEMENT_LOCAL_ACTION_MAX_LEAD_TILES) {
      return null;
    }
    if (!crystalMovementCandidateNotBehindServer(serverSelf, predicted, predicted.direction ?? serverSelf.direction)) {
      return null;
    }
    return {
      x: predicted.x,
      y: predicted.y,
      direction: predicted.direction,
      mode,
      sentAt: Math.max(0, directionStepVisualUntilRef.current - movementStepIntervalMs(mode)),
      sentFromX: serverSelf.x,
      sentFromY: serverSelf.y,
    };
  }

  function movementPlanBlockedByActionCadence(plan: MovementPlan | null, serverSelf: WorldEntity | null, now: number) {
    if (!plan) {
      return false;
    }
    if (movementPlanPendingStillAhead(plan, serverSelf)) {
      return true;
    }
    const confirmedActionReadyAt = confirmedMovementActionReadyAt(plan);
    return confirmedActionReadyAt > 0 && now < confirmedActionReadyAt;
  }

  function handleGatewayEvent(event: GatewayEvent) {
    const debugWindow = window as typeof window & {
      __mir2LastGatewayEvent?: Record<string, unknown>;
      __mir2GatewayEventHistory?: Array<Record<string, unknown>>;
      __mir2MovementReceivedPackets?: Array<Record<string, unknown>>;
    };
    const debugEvent = {
      type: event.type,
      packet: "packet" in event ? event.packet ?? null : null,
      payload:
        event.type === "worldSnapshot"
          ? summarizeDebugWorldSnapshot(event.payload as GatewayWorldSnapshot)
          : "payload" in event
            ? event.payload ?? null
            : null,
      at: Date.now(),
    };
    debugWindow.__mir2LastGatewayEvent = debugEvent;
    debugWindow.__mir2GatewayEventHistory = [debugEvent, ...(debugWindow.__mir2GatewayEventHistory ?? [])].slice(0, 50);
    if (event.type === "error") {
      const message = event.message ?? t("error.unknown");
      pendingLoginRef.current = false;
      pendingNewAccountRef.current = false;
      pendingSuiLoginRef.current = null;
      setLoginBusy(false);
      if (reconnectSnapshotRef.current) {
        failGatewayReconnect();
      }
      if (screenRef.current === "login") {
        setLoginErrorKey(`Gateway login failed: ${message}`);
      }
      appendLog(t("log.gatewayError", [message]), "system");
      return;
    }
    if (event.type === "worldSnapshot") {
      worldSnapshotVersionRef.current += 1;
      const snapshot = event.payload as GatewayWorldSnapshot;
      const snapshotSelf = snapshot.entities.find(
        (entity) => String(entity.objectId) === String(snapshot.playerObjectId),
      );
      recordMovementDiagnostic("rx:worldSnapshot", {
        tick: snapshot.tick,
        mapFileName: snapshot.mapFileName ?? null,
        snapshotSelf: snapshotSelf
          ? {
              x: snapshotSelf.x,
              y: snapshotSelf.y,
              direction: snapshotSelf.direction,
            }
          : null,
        before: captureMovementDiagnosticSample(),
      });
      applyGatewayWorldSnapshot(event.payload as GatewayWorldSnapshot);
      return;
    }
    if (event.type !== "packet" || !event.packet) return;

    appendLog(t("log.recv", [event.packet]), "network");
    const payload = event.payload ?? {};
    if (isMovementPacketName(event.packet)) {
      debugWindow.__mir2MovementReceivedPackets = [
        {
          packet: event.packet,
          payload,
          at: Date.now(),
        },
        ...(debugWindow.__mir2MovementReceivedPackets ?? []),
      ].slice(0, 50);
      recordMovementDiagnostic("rx:movementPacket", {
        packet: event.packet,
        payload,
        before: captureMovementDiagnosticSample(),
      });
    }

    recordDebugEvent("packet-in", "net", { packet: event.packet });
    switch (event.packet) {
      case "Connected":
        setLoginErrorKey(null);
        break;
      case "ClientVersion":
        if (numberOrZero(payload.result) !== 1) {
          appendLog(t("log.gatewayError", [t("error.unknown")]), "system");
        }
        break;
      case "Disconnect":
        resetGatewayReconnectState();
        activeReconnectAuthRef.current = null;
        setLoginBusy(false);
        screenRef.current = "login";
        setScreen("login");
        setWorld((current) => ({
          ...DEFAULT_WORLD_STATE,
          connected: false,
          mapTitle: current.mapTitle,
          mapFileName: current.mapFileName,
          inSafeZone: current.inSafeZone,
          miniMapIndex: current.miniMapIndex,
          bigMapIndex: current.bigMapIndex,
          sceneView: current.sceneView,
          terrainPatches: current.terrainPatches,
          decorObjects: current.decorObjects,
          originalMapRegion: current.originalMapRegion,
        }));
        appendLog(
          t(
            "log.gatewayError",
            [stringOrFallback(payload.reason, t("error.unknown"))],
            `Gateway disconnected: ${stringOrFallback(payload.reason, t("error.unknown"))}.`,
          ),
          "system",
        );
        break;
      case "KeepAlive":
        break;
      case "NewAccount":
        appendLog(
          numberOrZero(payload.result) === 8
            ? t("client.AccountCreatedSuccessfully", [], "Your account was created successfully.")
            : t("client.AccountCreationDisabled", [], "Account creation is currently disabled."),
          "system",
        );
        break;
      case "Login":
        if (reconnectSnapshotRef.current) {
          failGatewayReconnect();
        }
        activeReconnectAuthRef.current = null;
        setLoginBusy(false);
        setLoginErrorKey("error.loginFailedCheckAccountPassword");
        screenRef.current = "login";
        setScreen("login");
        break;
      case "LoginBanned":
        if (reconnectSnapshotRef.current) {
          failGatewayReconnect();
        }
        activeReconnectAuthRef.current = null;
        setLoginBusy(false);
        setLoginErrorKey("error.loginFailedCheckAccountPassword");
        appendLog(
          t(
            "ui.loginBanned",
            [stringOrFallback(payload.reason, t("error.unknown"))],
            `Login banned: ${stringOrFallback(payload.reason, t("error.unknown"))}.`,
          ),
          "system",
        );
        screenRef.current = "login";
        setScreen("login");
        break;
      case "NewCharacterSuccess":
        setCharacters((current) => {
          const nextCharacters = parseCharacters({ characters: [...current, payload.character] }, accountId, language);
          const visibleCharacters = nextCharacters.slice(0, 4);
          const createdIndex = numberOrUndefined(
            (payload.character as Record<string, unknown> | undefined)?.index ??
              (payload.character as Record<string, unknown> | undefined)?.Index,
          );
          const visibleIndex = visibleCharacters.findIndex((character) => character.index === createdIndex);
          if (visibleIndex >= 0) {
            setSelectedCharacterIndex(visibleIndex);
          }
          return visibleCharacters;
        });
        appendLog(t("ui.newCharacterCreated", [], "Character created."), "system");
        playOriginalSoundEvent("characterCreated");
        break;
      case "NewCharacter":
        appendLog(t("ui.newCharacterFailed", [], "Character creation failed."), "system");
        break;
      case "DeleteCharacterSuccess":
        setCharacters((current) => {
          const removedIndex = numberOrUndefined(payload.characterIndex);
          const nextCharacters = current.filter((character) => character.index !== removedIndex);
          setSelectedCharacterIndex(Math.max(0, Math.min(selectedCharacterIndex, nextCharacters.length - 1)));
          return nextCharacters.length ? nextCharacters : [fallbackCharacter(language, accountId)];
        });
        appendLog(t("ui.characterDeleted", [], "Character deleted."), "system");
        break;
      case "DeleteCharacter":
        appendLog(t("ui.characterDeleteFailed", [], "Character deletion failed."), "system");
        break;
      case "LoginSuccess":
        setLoginBusy(false);
        setLoginErrorKey(null);
        {
          const nextCharacters = parseCharacters(payload, accountId, language);
          const reconnectSnapshot = reconnectSnapshotRef.current;
          setCharacters(nextCharacters);
          if (reconnectSnapshot) {
            const reconnectCharacterPosition = nextCharacters.findIndex(
              (character) => character.index === reconnectSnapshot.characterIndex,
            );
            setSelectedCharacterIndex(reconnectCharacterPosition >= 0 ? reconnectCharacterPosition : 0);
          } else {
            setSelectedCharacterIndex(0);
            screenRef.current = "select";
            setScreen("select");
          }
        }
        markMir2CacheMilestone("loginSuccess", {
          characterCount: Array.isArray(payload.characters) ? payload.characters.length : undefined,
        });
        if (!reconnectSnapshotRef.current) {
          markMir2CacheMilestone("selectReady");
        }
        break;
      case "StartGame":
        markMir2CacheMilestone("startGamePacket", {
          result: numberOrZero(payload.result),
        });
        if (numberOrZero(payload.result) !== 4) {
          if (reconnectSnapshotRef.current) {
            failGatewayReconnect();
          }
          setLoginBusy(false);
          appendLog(
            t(
              "log.gatewayError",
              [stringOrFallback(payload.result, t("error.unknown"))],
              `Start game failed with result ${stringOrFallback(payload.result, t("error.unknown"))}.`,
            ),
            "system",
          );
        }
        break;
      case "Rankings":
        applyRankingPacket(payload);
        break;
      case "MapInformation": {
        const miniMapIndex = numberOrUndefined(payload.miniMapIndex);
        const bigMapIndex = numberOrUndefined(payload.bigMapIndex);
        setWorld((current) => {
          const nextMapFileName = stringOrNull(payload.fileName) ?? current.mapFileName;
          const mapChanged =
            normalizeMapFileName(nextMapFileName) !== normalizeMapFileName(current.mapFileName);
          const preservedSelfEntity = current.playerObjectId
            ? current.entities.find((entity) => entity.objectId === current.playerObjectId)
            : undefined;
          const nextWorld = {
            ...current,
            mapTitle: stringOrNull(payload.title),
            mapFileName: nextMapFileName,
            miniMapIndex: miniMapIndex && miniMapIndex > 0 ? miniMapIndex : null,
            bigMapIndex: bigMapIndex && bigMapIndex > 0 ? bigMapIndex : null,
            selectedObjectId: mapChanged ? null : current.selectedObjectId,
            activeNpcDialog: mapChanged ? null : current.activeNpcDialog,
            entities: mapChanged && preservedSelfEntity ? [preservedSelfEntity] : mapChanged ? [] : current.entities,
            groundDrops: mapChanged ? [] : current.groundDrops,
            mineNodes: mapChanged ? [] : current.mineNodes,
            projectiles: mapChanged ? [] : current.projectiles,
            sceneView: mapChanged ? null : current.sceneView,
            terrainPatches: mapChanged ? [] : current.terrainPatches,
            decorObjects: mapChanged ? [] : current.decorObjects,
            originalMapRegion: mapChanged ? null : current.originalMapRegion,
          };
          worldRef.current = nextWorld;
          return nextWorld;
        });
        break;
      }
      case "UserInformation": {
        const objectId = stringifyId(payload.objectId);
        const location = payload.location as { x?: number; y?: number } | undefined;
        const userX = numberOrZero(location?.x);
        const userY = numberOrZero(location?.y);
        const userDirection = stringOrNull(payload.direction) ?? undefined;
        lastSelfMovementAckRef.current = {
          x: userX,
          y: userY,
          direction: userDirection,
          at: Date.now(),
        };
        setWorld((current) => ({
          ...current,
          playerObjectId: objectId,
          playerName: stringOrFallback(payload.name, t("ui.self")),
          playerHp: numberOrUndefined(payload.hp),
          playerMaxHp: numberOrUndefined(payload.hp),
          playerMp: numberOrUndefined(payload.mp),
          playerExperience: numberOrZero(payload.experience),
          playerMaxExperience: Math.max(numberOrZero(payload.maxExperience), 1),
          gold: numberOrZero(payload.gold),
          credit: numberOrZero(payload.credit),
          hasExpandedStorage: payload.hasExpandedStorage === true,
          hasStoragePassword: payload.hasStoragePassword === true,
          requireStoragePassword: payload.requireStoragePassword === true,
          storageSessionUnlocked: payload.requireStoragePassword !== true,
          storagePasswordLastSetBinaryDatetime: numberOrZero(payload.storagePasswordLastSetBinaryDatetime),
          expandedStorageExpiryTimeBinaryDatetime: numberOrZero(
            payload.expandedStorageExpiryTimeBinaryDatetime,
          ),
          entities: upsertEntityInList(current.entities, {
            objectId,
            kind: "selfPlayer",
            name: stringOrFallback(payload.name, t("ui.self")),
            x: userX,
            y: userY,
            direction: userDirection,
            classKey: mapClassKey(payload.class),
            genderKey: mapGenderKey(payload.gender),
            level: numberOrUndefined(payload.level),
            hp: numberOrUndefined(payload.hp),
            maxHp: numberOrUndefined(payload.hp),
            nameColourArgb: -1,
            disposition: "friendly",
            sprite: playerSpriteFromPacket(payload),
          }),
        }));
        screenRef.current = "game";
        setScreen("game");
        completeGatewayReconnect();
        markMir2CacheMilestone("userInformationReady", {
          objectId,
          x: userX,
          y: userY,
        });
        appendCrystalGameEntryChat();
        break;
      }
      case "UserLocation":
      case "Pushed":
      case "UserDash":
      case "UserDashFail":
      case "UserDashAttack":
      case "UserAttackMove":
      case "ObjectTurn":
      case "ObjectWalk":
      case "ObjectRun":
      case "ObjectPushed":
      case "ObjectDash":
      case "ObjectDashFail":
      case "ObjectDashAttack":
      case "ObjectBackStep":
      case "ObjectSitDown": {
        const selfMovementPacket =
          event.packet === "UserLocation" ||
          event.packet === "Pushed" ||
          event.packet === "UserDash" ||
          event.packet === "UserDashFail" ||
          event.packet === "UserDashAttack" ||
          event.packet === "UserAttackMove";
        const movementObjectId = selfMovementPacket
          ? worldRef.current.playerObjectId ?? "0"
          : stringifyId(payload.objectId);
        const packetPoint = movementPointFromPacketPayload(payload);
        const x = packetPoint.x;
        const y = packetPoint.y;
        const direction = stringOrNull(payload.direction) ?? undefined;
        const movementPacket = event.packet;
        const movementNow = Date.now();
        let selfPacketAckDisposition: CrystalSelfAckDisposition | null = null;
        let previousSelfForMovementDiagnostic: WorldEntity | null = null;
        let selfMovementAdvanced = false;
        if (movementObjectId === worldRef.current.playerObjectId) {
          previousSelfForMovementDiagnostic = worldRef.current.entities.find(
            (entity) => entity.objectId === worldRef.current.playerObjectId,
          ) ?? null;
          selfMovementAdvanced =
            !previousSelfForMovementDiagnostic ||
            previousSelfForMovementDiagnostic.x !== x ||
            previousSelfForMovementDiagnostic.y !== y ||
            (!!direction && previousSelfForMovementDiagnostic.direction !== direction);
          selfPacketAckDisposition = classifySelfMovementAckDisposition({ x, y, direction }, movementPacket);
          recordMovementDiagnostic("apply:selfMovementPacket", {
            packet: movementPacket,
            packetPoint: { x, y, direction },
            previousSelf: previousSelfForMovementDiagnostic
              ? {
                  x: previousSelfForMovementDiagnostic.x,
                  y: previousSelfForMovementDiagnostic.y,
                  direction: previousSelfForMovementDiagnostic.direction,
                }
              : null,
            selfMovementAdvanced,
            crystalAckDisposition: selfPacketAckDisposition,
            localOverride: null,
            sample: captureMovementDiagnosticSample(movementNow),
          });
          if (selfMovementPacket) {
            const ackedCommand = consumeMovementConsoleCommand();
            const commandAt = numberOrUndefined(ackedCommand?.at);
            recordMovementConsoleEvent(
              selfPacketAckDisposition === "correction" ? "correction" : "ack",
              {
                packet: movementPacket,
                point: { x, y, direction: direction ?? null },
                disposition: selfPacketAckDisposition,
                latencyMs: commandAt === undefined ? null : Math.max(0, movementNow - commandAt),
                command: ackedCommand,
                previousSelf: previousSelfForMovementDiagnostic
                  ? {
                      x: previousSelfForMovementDiagnostic.x,
                      y: previousSelfForMovementDiagnostic.y,
                      direction: previousSelfForMovementDiagnostic.direction,
                    }
                  : null,
                selfMovementAdvanced,
                localOverride: null,
                state: captureMovementConsoleState(movementNow),
              },
            );
          }
        }
        setWorld((current) => {
          const nextWorld = {
            ...current,
            entities: current.entities.map((entity) =>
              entity.objectId === movementObjectId
                ? entity.objectId === current.playerObjectId && selfPacketAckDisposition
	                  ? withCrystalSelfPacketMovement(
	                      {
	                        ...entity,
	                        x,
	                        y,
	                        direction,
	                      },
	                      entity,
	                      movementPacket,
                      selfPacketAckDisposition,
                      movementNow,
                    )
	                  : withPacketMovementAnimation(
	                      {
	                        ...entity,
	                        x,
	                        y,
	                        direction,
	                      },
	                      entity,
                      movementPacket,
                      movementNow,
                    )
                : entity,
            ),
          };
	          worldRef.current = nextWorld;
	          return nextWorld;
	        });
        if (movementObjectId === worldRef.current.playerObjectId) {
          const outcome = reconcileSelfMovementAck({ x, y, direction }, movementPacket, movementNow);
          if (outcome === "confirmed") {
            void trySendQueuedCrystalMove();
          }
        }
        break;
      }
      case "ObjectPlayer":
        setWorldEntityFromPacket(payload, "player", "friendly");
        break;
      case "ObjectHero":
        setWorldEntityFromPacket(payload, "player", "friendly");
        break;
      case "NewMonsterInfo":
        setWorldEntityFromPacket(payload, "monster", "hostile");
        break;
      case "ObjectMonster":
        setWorldEntityFromPacket(payload, "monster", "hostile");
        break;
      case "NewNpcInfo":
        setWorldEntityFromPacket(payload, "npc", "neutral");
        break;
      case "ObjectNpc":
        setWorldEntityFromPacket(payload, "npc", "neutral");
        break;
      case "ObjectRemove":
      case "ObjectHide":
        removeObjectFromWorld(stringifyId(payload.objectId));
        break;
      case "ObjectShow":
        break;
      case "ObjectItem":
        setWorldGroundDropFromPacket(payload, stringOrFallback(payload.name, t("ui.item", [], "Item")));
        break;
      case "ObjectGold":
        setWorldGroundDropFromPacket(
          payload,
          typeof payload.gold === "number" ? `${payload.gold} ${t("ui.gold", [], "Gold")}` : t("ui.gold", [], "Gold"),
        );
        break;
      case "LoseGold": {
        const gold = numberOrZero(payload.gold);
        setWorld((current) => ({
          ...current,
          gold: Math.max(0, current.gold - gold),
        }));
        break;
      }
      case "GainedGold": {
        const gold = numberOrZero(payload.gold);
        setWorld((current) => ({
          ...current,
          gold: current.gold + gold,
        }));
        break;
      }
      case "ObjectAttack":
        updateWorldEntityFromLocationPacket(payload);
        markWorldEntityAttack(payload);
        break;
      case "ObjectHarvest":
      case "ObjectHarvested":
        updateWorldEntityFromLocationPacket(payload);
        break;
      case "ObjectStruck":
        updateWorldEntityFromLocationPacket(payload);
        markWorldEntityStruck(payload);
        break;
      case "ObjectRangeAttack":
        updateWorldEntityFromLocationPacket(payload);
        spawnRangeProjectile(payload);
        restoreObjectSelection(stringifyId(payload.targetId));
        break;
      case "ObjectProjectile":
        spawnRangeProjectile(payload);
        restoreObjectSelection(stringifyId(payload.destinationId));
        break;
      case "Magic":
      case "MagicCast":
        markPlayerMagic(payload);
        restoreObjectSelection(stringifyId(payload.targetId));
        break;
      case "MagicDelay":
        applyMagicDelayPacket(payload);
        break;
      case "MagicLeveled":
        applyMagicLeveledPacket(payload);
        break;
      case "ObjectSpell":
      case "ObjectMagic":
        updateWorldEntityFromLocationPacket(payload);
        markWorldEntityMagic(payload);
        if (stringifyId(payload.targetId) !== "0") {
          spawnRangeProjectile(payload);
          restoreObjectSelection(stringifyId(payload.targetId));
        }
        break;
      case "MapEffect":
        appendLog(
          t("ui.mapEffect", [String(numberOrZero(payload.effect))], `Map effect ${numberOrZero(payload.effect)}`),
          "system",
        );
        break;
      case "MineNodeState": {
        // Server-authoritative depletion stage for a mineable cell. The in-world
        // vein sprite is rendered by the Bevy runtime; here we surface the stage
        // change in the message log so "ore depletes as you mine" is observable.
        const mineStage = numberOrZero(payload.stage);
        const mineStageLabel =
          mineStage >= 2 ? "full vein" : mineStage === 1 ? "cracked" : "depleted";
        const mineLoc = payload.location as { x?: number; y?: number } | undefined;
        const mineX = numberOrZero(mineLoc?.x);
        const mineY = numberOrZero(mineLoc?.y);
        setWorld((current) => {
          const others = current.mineNodes.filter(
            (node) => node.x !== mineX || node.y !== mineY,
          );
          return {
            ...current,
            mineNodes: [...others, { x: mineX, y: mineY, stage: mineStage }],
          };
        });
        appendLog(
          t(
            "ui.mineNode",
            [String(mineX), String(mineY), mineStageLabel],
            `Mine node (${mineX}, ${mineY}) -> ${mineStageLabel}`,
          ),
          "system",
        );
        break;
      }
      case "PlaySound":
        playOriginalSoundId(numberOrZero(payload.sound));
        break;
      case "AddBuff":
        applyAddBuffPacket(payload);
        break;
      case "RemoveBuff":
        applyRemoveBuffPacket(payload);
        break;
      case "PauseBuff":
        applyPauseBuffPacket(payload);
        break;
      case "Struck":
        markPlayerStruck(payload);
        restoreObjectSelection(stringifyId(payload.attackerId));
        break;
      case "ObjectDied":
        markWorldEntityDead(payload);
        break;
      case "ObjectRevived":
        markWorldEntityRevived(payload);
        break;
      case "ObjectHealth":
        applyObjectHealthPacket(payload);
        break;
      case "ObjectMana":
        applyObjectManaPacket(payload);
        break;
      case "UseItem": {
        const uniqueId = numberOrUndefined(payload.uniqueId);
        const grid = stringOrFallback(payload.grid, "Inventory");
        if (payload.success === true && typeof uniqueId === "number") {
          setWorld((current) => ({
            ...current,
            inventoryItems: consumePacketItem(current.inventoryItems, grid, uniqueId, 1),
            beltItems: consumePacketItem(current.beltItems, grid, uniqueId, 1),
            storageItems: consumePacketItem(current.storageItems, grid, uniqueId, 1),
          }));
        }
        break;
      }
      case "DropItem": {
        const uniqueId = numberOrUndefined(payload.uniqueId);
        const count = numberOrZero(payload.count);
        if (payload.success === true && typeof uniqueId === "number") {
          setWorld((current) => ({
            ...current,
            inventoryItems: consumePacketItem(current.inventoryItems, "Inventory", uniqueId, Math.max(1, count)),
          }));
        }
        break;
      }
      case "DuraChanged": {
        const uniqueId = numberOrUndefined(payload.uniqueId);
        const currentDura = numberOrUndefined(payload.currentDura);
        if (typeof uniqueId === "number" && typeof currentDura === "number") {
          const equipmentSlot = equipmentSlotFromIndex(uniqueId);
          setWorld((current) => ({
            ...current,
            inventoryItems: current.inventoryItems.map((item) =>
              item.uniqueId === uniqueId ? { ...item, durabilityCurrent: currentDura } : item,
            ),
            beltItems: current.beltItems.map((item) =>
              item.uniqueId === uniqueId ? { ...item, durabilityCurrent: currentDura } : item,
            ),
            storageItems: current.storageItems.map((item) =>
              item.uniqueId === uniqueId ? { ...item, durabilityCurrent: currentDura } : item,
            ),
            equipmentItems: equipmentSlot
              ? current.equipmentItems.map((item) =>
                  item.slot === equipmentSlot ? { ...item, durabilityCurrent: currentDura } : item,
                )
              : current.equipmentItems,
          }));
        }
        break;
      }
      case "Chat":
        appendLog(
          stringOrFallback(payload.message, ""),
          gatewayChatTone(payload.chatType),
          gatewayChatChannel(payload.chatType),
        );
        break;
      case "ObjectChat":
        appendLog(
          stringOrFallback(payload.text, ""),
          gatewayChatTone(payload.chatType),
          gatewayChatChannel(payload.chatType),
        );
        break;
      case "StorageUnlockResult": {
        const result = numberOrZero(payload.result);
        const hasPassword = payload.hasPassword === true;
        setWorld((current) => ({
          ...current,
          hasStoragePassword: hasPassword,
          storageSessionUnlocked: result === 0 || result === 4 || !hasPassword,
        }));
        appendLog(storageUnlockResultMessage(result, hasPassword), "system");
        break;
      }
      case "UserStorage": {
        const storageEntries = Array.isArray(payload.storage) ? payload.storage : [];
        setWorld((current) => {
          const currentBySlot = new Map(current.storageItems.map((item) => [item.slot, item]));
          const storageItems = storageEntries.flatMap((entry, slot) => {
            if (!entry || typeof entry !== "object") {
              return [];
            }

            const currentEntry = currentBySlot.get(slot);
            const userItem = entry as {
              count?: unknown;
              current_dura?: unknown;
              max_dura?: unknown;
              unique_id?: unknown;
              uniqueId?: unknown;
            };
            return [
              {
                key: currentEntry?.key ?? `storage-slot-${slot}`,
                name: currentEntry?.name ?? `Storage Item ${slot}`,
                icon: currentEntry?.icon ?? 0,
                uniqueId:
                  numberOrUndefined(userItem.uniqueId) ??
                  numberOrUndefined(userItem.unique_id) ??
                  currentEntry?.uniqueId ??
                  slot,
                slot,
                container: currentEntry?.container ?? "storage",
                quantity: numberOrUndefined(userItem.count) ?? currentEntry?.quantity ?? 1,
                description: currentEntry?.description ?? "",
                durabilityCurrent:
                  numberOrUndefined(userItem.current_dura) ?? currentEntry?.durabilityCurrent,
                durabilityMax: numberOrUndefined(userItem.max_dura) ?? currentEntry?.durabilityMax,
              },
            ];
          });

          return {
            ...current,
            storageItems,
          };
        });
        break;
      }
      case "NPCStorage":
        setShowInventory(true);
        setActiveInventoryTab("bag1");
        setStorageServiceOpenVersion((current) => current + 1);
        break;
      case "StoragePasswordResult": {
        const result = numberOrZero(payload.result);
        const removing = payload.removing === true;
        const hasPassword = payload.hasPassword === true;
        setWorld((current) => ({
          ...current,
          hasStoragePassword: hasPassword,
          storageSessionUnlocked: removing ? !hasPassword : result === 4 || current.storageSessionUnlocked,
          storagePasswordLastSetBinaryDatetime:
            numberOrUndefined(payload.lastSetBinaryDatetime) ?? current.storagePasswordLastSetBinaryDatetime,
        }));
        appendLog(storagePasswordResultMessage(result, removing, hasPassword), "system");
        break;
      }
      case "ResizeStorage": {
        const size = numberOrZero(payload.size);
        const hasExpandedStorage = payload.hasExpandedStorage === true;
        setWorld((current) => ({
          ...current,
          storageSize: size > 0 ? size : current.storageSize,
          hasExpandedStorage,
          expandedStorageExpiryTimeBinaryDatetime:
            numberOrUndefined(payload.expiryTimeBinaryDatetime) ??
            current.expandedStorageExpiryTimeBinaryDatetime,
        }));
        appendLog(storageResizeMessage(size, hasExpandedStorage), "system");
        break;
      }
      case "LogOutSuccess":
        resetGatewayReconnectState();
        activeReconnectAuthRef.current = null;
        screenRef.current = "login";
        setScreen("login");
        setLoginBusy(false);
        setLoginErrorKey(null);
        {
          const nextCharacters = parseCharacters(payload, accountId, language);
          setCharacters(nextCharacters);
          setSelectedCharacterIndex(0);
        }
        setWorld((current) => ({
          ...DEFAULT_WORLD_STATE,
          connected: true,
          mapTitle: current.mapTitle,
          mapFileName: current.mapFileName,
          inSafeZone: current.inSafeZone,
          miniMapIndex: current.miniMapIndex,
          bigMapIndex: current.bigMapIndex,
          sceneView: current.sceneView,
          terrainPatches: current.terrainPatches,
          decorObjects: current.decorObjects,
          originalMapRegion: current.originalMapRegion,
        }));
        break;
      case "LogOutFailed":
        appendLog(t("ui.logoutFailed", [], "Log out failed."), "system");
        break;
      case "StartGameBanned":
        if (reconnectSnapshotRef.current) {
          failGatewayReconnect();
        }
        appendLog(
          t(
            "ui.startGameBanned",
            [stringOrFallback(payload.reason, t("error.unknown"))],
            `Start game banned: ${stringOrFallback(payload.reason, t("error.unknown"))}.`,
          ),
          "system",
        );
        screenRef.current = "select";
        setScreen("select");
        break;
      case "StartGameDelay":
        appendLog(
          t(
            "ui.startGameDelay",
            [numberOrZero(payload.milliseconds)],
            `Start game delayed: ${numberOrZero(payload.milliseconds)} ms.`,
          ),
          "system",
        );
        break;
      // --- [fe-packets] extended handlers ---
      // Additive gameplay-visible server->client packet handling. Field shapes
      // follow apps/gateway/src/web.rs `server_packet_to_event`.

      // Object world-state sync ------------------------------------------------
      case "ObjectTeleportOut":
        updateWorldEntityFromLocationPacket(payload);
        break;
      case "ObjectTeleportIn":
        updateWorldEntityFromLocationPacket(payload);
        break;
      case "ObjectHidden": {
        const objectId = stringifyId(payload.objectId);
        const hidden = payload.hidden === true;
        // Hidden mobs (e.g. sneaking/stealth) stay tracked but lose selection focus.
        setWorld((current) =>
          hidden && current.selectedObjectId === objectId
            ? { ...current, selectedObjectId: null }
            : current,
        );
        break;
      }
      case "ObjectSneaking":
        // Visual-only stealth toggle; location is unchanged. Restore selection sanity.
        restoreObjectSelection(stringifyId(payload.objectId));
        break;
      case "RemoveDelayedExplosion": {
        const objectId = stringifyId(payload.objectId);
        setWorld((current) => {
          const projectiles = current.projectiles.filter(
            (projectile) => projectile.attackerId !== objectId && projectile.targetId !== objectId,
          );
          if (projectiles.length === current.projectiles.length) {
            return current;
          }
          return { ...current, projectiles };
        });
        break;
      }
      case "RangeAttack": {
        // Self ranged attack: the source is the local player; reuse the projectile
        // spawner by supplying the player's object id as the attacker.
        const playerObjectId = worldRef.current.playerObjectId;
        if (playerObjectId) {
          spawnRangeProjectile({ ...payload, objectId: playerObjectId });
        }
        restoreObjectSelection(stringifyId(payload.targetId));
        break;
      }
      case "SetBindingShot":
        // Trap/binding marker on a target; surface location-only update if present.
        restoreObjectSelection(stringifyId(payload.objectId));
        break;
      case "MountUpdate":
        // Mount appearance is re-derived from the next world snapshot; clear any
        // stale walk/run animation on the affected entity so speed re-syncs.
        {
          const objectId = stringifyId(payload.objectId);
          setWorld((current) => ({
            ...current,
            entities: patchEntityInList(current.entities, objectId, (entity) => ({
              ...entity,
              movementAnimation: undefined,
              movementStartedAt: undefined,
              movementUntil: undefined,
            })),
          }));
        }
        break;
      case "FishingUpdate":
        if (payload.foundFish === true) {
          appendLog(t("ui.fishingBite", [], "A fish is biting!"), "system");
        }
        break;

      // Player / hero progression ---------------------------------------------
      case "GainExperience": {
        const amount = numberOrZero(payload.amount);
        if (amount > 0) {
          setWorld((current) => ({
            ...current,
            playerExperience: current.playerExperience + amount,
          }));
        }
        break;
      }
      case "LevelChanged":
        setWorld((current) => ({
          ...current,
          playerExperience: numberOrZero(payload.experience),
          playerMaxExperience: Math.max(numberOrZero(payload.maxExperience), 1),
        }));
        appendLog(
          t("server.LevelUp", [numberOrZero(payload.level)], `You reached level ${numberOrZero(payload.level)}.`),
          "system",
          "announcement",
        );
        break;
      case "HealthChanged": {
        const hp = numberOrUndefined(payload.hp);
        const mp = numberOrUndefined(payload.mp);
        setWorld((current) => ({
          ...current,
          playerHp: typeof hp === "number" ? Math.max(0, hp) : current.playerHp,
          playerMp: typeof mp === "number" ? Math.max(0, mp) : current.playerMp,
          entities: current.playerObjectId
            ? patchEntityInList(current.entities, current.playerObjectId, (entity) => ({
                ...entity,
                hp: typeof hp === "number" ? Math.max(0, hp) : entity.hp,
              }))
            : current.entities,
        }));
        break;
      }
      case "GainHeroExperience":
      case "HeroLevelChanged":
      case "HeroHealthChanged":
        // Hero (companion) stats are tracked in the stage5 hero slice rather than
        // the primary HUD; mirror them there so panels can read them.
        setWorld((current) => ({
          ...current,
          stage5Systems: {
            ...current.stage5Systems,
            hero: {
              ...(current.stage5Systems.hero ?? {}),
              ...(event.packet === "HeroHealthChanged"
                ? { hp: numberOrUndefined(payload.hp), mp: numberOrUndefined(payload.mp) }
                : {}),
              ...(event.packet === "HeroLevelChanged"
                ? {
                    level: numberOrUndefined(payload.level),
                    experience: numberOrUndefined(payload.experience),
                    maxExperience: numberOrUndefined(payload.maxExperience),
                  }
                : {}),
            },
          },
        }));
        break;
      case "GainedCredit": {
        const credit = numberOrZero(payload.credit);
        setWorld((current) => ({ ...current, credit: current.credit + credit }));
        break;
      }
      case "LoseCredit": {
        const credit = numberOrZero(payload.credit);
        setWorld((current) => ({ ...current, credit: Math.max(0, current.credit - credit) }));
        break;
      }
      case "Poisoned":
        appendLog(t("ui.poisoned", [], "You are poisoned."), "system");
        break;
      case "Death":
        setWorld((current) => ({
          ...current,
          playerHp: 0,
          entities: current.playerObjectId
            ? patchEntityInList(current.entities, current.playerObjectId, (entity) => ({
                ...entity,
                hp: 0,
                dead: true,
              }))
            : current.entities,
        }));
        break;
      case "Revived":
        setWorld((current) => ({
          ...current,
          playerHp:
            typeof current.playerMaxHp === "number" ? Math.max(1, current.playerMaxHp) : current.playerHp,
          entities: current.playerObjectId
            ? patchEntityInList(current.entities, current.playerObjectId, (entity) => ({
                ...entity,
                dead: false,
                hp: typeof entity.maxHp === "number" ? Math.max(1, entity.maxHp) : entity.hp,
              }))
            : current.entities,
        }));
        break;

      // Inventory / item lifecycle --------------------------------------------
      case "GainedItem":
      case "GainedQuestItem":
      case "NewChatItem": {
        const item = normalizeUserItem(payload.item);
        if (item) {
          appendLog(
            t("ui.gainedItem", [String(item.itemIndex ?? item.uniqueId)], "You gained an item."),
            "system",
          );
        }
        break;
      }
      case "RefreshItem":
      case "ItemUpgraded": {
        const item = normalizeUserItem(payload.item);
        if (item) {
          setWorld((current) => ({
            ...current,
            inventoryItems: patchItemsByUniqueId(current.inventoryItems, {
              uniqueId: item.uniqueId,
              quantity: item.count,
              durabilityCurrent: item.currentDura,
              durabilityMax: item.maxDura,
            }),
            beltItems: patchItemsByUniqueId(current.beltItems, {
              uniqueId: item.uniqueId,
              quantity: item.count,
              durabilityCurrent: item.currentDura,
              durabilityMax: item.maxDura,
            }),
            storageItems: patchItemsByUniqueId(current.storageItems, {
              uniqueId: item.uniqueId,
              quantity: item.count,
              durabilityCurrent: item.currentDura,
              durabilityMax: item.maxDura,
            }),
          }));
        }
        break;
      }
      case "DeleteItem":
      case "DeleteQuestItem": {
        const uniqueId = numberOrUndefined(payload.uniqueId);
        const count = numberOrZero(payload.count);
        if (typeof uniqueId === "number") {
          setWorld((current) => ({
            ...current,
            inventoryItems: removeItemByUniqueId(current.inventoryItems, uniqueId, count),
            beltItems: removeItemByUniqueId(current.beltItems, uniqueId, count),
            storageItems: removeItemByUniqueId(current.storageItems, uniqueId, count),
          }));
        }
        break;
      }
      case "SellItem": {
        const uniqueId = numberOrUndefined(payload.uniqueId);
        const count = numberOrZero(payload.count);
        if (payload.success === true && typeof uniqueId === "number") {
          setWorld((current) => ({
            ...current,
            inventoryItems: removeItemByUniqueId(current.inventoryItems, uniqueId, Math.max(1, count)),
          }));
        }
        break;
      }
      case "CombineItem": {
        const destroy = payload.destroy === true;
        const idFrom = numberOrUndefined(payload.idFrom);
        if (payload.success === true && destroy && typeof idFrom === "number") {
          setWorld((current) => ({
            ...current,
            inventoryItems: removeItemByUniqueId(current.inventoryItems, idFrom, 1),
          }));
        }
        break;
      }
      case "ItemRepaired": {
        const uniqueId = numberOrUndefined(payload.uniqueId);
        const currentDura = numberOrUndefined(payload.currentDura);
        const maxDura = numberOrUndefined(payload.maxDura);
        if (typeof uniqueId === "number") {
          setWorld((current) => ({
            ...current,
            inventoryItems: patchItemsByUniqueId(current.inventoryItems, {
              uniqueId,
              durabilityCurrent: currentDura,
              durabilityMax: maxDura,
            }),
            beltItems: patchItemsByUniqueId(current.beltItems, {
              uniqueId,
              durabilityCurrent: currentDura,
              durabilityMax: maxDura,
            }),
            storageItems: patchItemsByUniqueId(current.storageItems, {
              uniqueId,
              durabilityCurrent: currentDura,
              durabilityMax: maxDura,
            }),
          }));
        }
        break;
      }
      case "ResizeInventory": {
        const size = numberOrZero(payload.size);
        if (size > 0) {
          setWorld((current) => ({
            ...current,
            maxBagSlots: size,
          }));
        }
        break;
      }

      // Magic / skills ---------------------------------------------------------
      case "RemoveMagic": {
        const placeId = numberOrUndefined(payload.placeId);
        if (typeof placeId === "number") {
          setWorld((current) => ({
            ...current,
            knownSkills: current.knownSkills.filter((skill) => skill.hotkey !== placeId),
          }));
        }
        break;
      }
      case "SpellToggle": {
        const objectId = stringifyId(payload.objectId);
        if (objectId === "0" || objectId === worldRef.current.playerObjectId) {
          const spell = stringOrFallback(payload.spell, "");
          const canUse = payload.canUse === true;
          if (spell) {
            setWorld((current) => ({
              ...current,
              knownSkills: current.knownSkills.map((skill) =>
                skillMatchesCrystalSpell(skill, spell)
                  ? { ...skill, castKind: canUse ? skill.castKind ?? "toggle" : "passive" }
                  : skill,
              ),
            }));
          }
        }
        break;
      }

      // Group / party ----------------------------------------------------------
      case "SwitchGroup":
        setWorld((current) => ({
          ...current,
          stage5Systems: {
            ...current.stage5Systems,
            group: {
              ...(current.stage5Systems.group ?? {}),
              lootMode: payload.allowGroup === true ? "group" : "solo",
            },
          },
        }));
        break;
      case "AddMember":
      case "DeleteMember": {
        const name = stringOrFallback(payload.name, "");
        if (name) {
          setWorld((current) => ({
            ...current,
            stage5Systems: {
              ...current.stage5Systems,
              group: {
                ...(current.stage5Systems.group ?? {}),
                members: groupMembersAfterChange(
                  current.stage5Systems.group?.members,
                  event.packet === "AddMember" ? { add: name } : { remove: name },
                ),
              },
            },
          }));
        }
        break;
      }
      case "GroupMemberInfo": {
        // Enriched full party roster (level/class/hp/online + leader), emitted
        // alongside AddMember/DeleteMember. Stored in `memberInfos` (separate from
        // the incremental name list) so the existing path stays intact; adaptGroup
        // prefers memberInfos when present.
        const rawMembers = Array.isArray(payload.members) ? payload.members : [];
        const memberInfos = rawMembers.flatMap((entry) => {
          const record = (entry ?? {}) as Record<string, unknown>;
          const name = stringOrFallback(record.name, "");
          if (!name) return [];
          return [
            {
              name,
              level: numberOrUndefined(record.level),
              class: numberOrUndefined(record.class),
              hp: numberOrUndefined(record.hp),
              maxHp: numberOrUndefined(record.maxHp),
              online: record.online === true,
            },
          ];
        });
        const leaderName = stringOrFallback(payload.leaderName, "");
        setWorld((current) => ({
          ...current,
          stage5Systems: {
            ...current.stage5Systems,
            group: {
              ...(current.stage5Systems.group ?? {}),
              memberInfos,
              leaderName: leaderName || undefined,
            },
          },
        }));
        break;
      }
      case "DeleteGroup":
        setWorld((current) => ({
          ...current,
          stage5Systems: {
            ...current.stage5Systems,
            group: { ...(current.stage5Systems.group ?? {}), members: [], memberInfos: [], leaderName: undefined },
          },
        }));
        break;
      case "GroupInvite":
        appendLog(
          t(
            "server.GroupInviteFrom",
            [stringOrFallback(payload.name, "?")],
            `${stringOrFallback(payload.name, "Someone")} invites you to a group.`,
          ),
          "system",
          "group",
        );
        break;

      // Social: friends / mentor / relationship --------------------------------
      case "FriendUpdate": {
        // B-wave enrichment: keep the rich friend objects (name/online/memo) so the
        // friends window shows status, not just names. Blocked entries now arrive
        // under a separate `blocked` key; fall back to splitting the combined list.
        const friendsRaw = normalizeFriendList(payload.friends);
        const hasBlockedKey = payload.blocked !== undefined;
        const blockedRaw = hasBlockedKey
          ? normalizeFriendList(payload.blocked)
          : friendsRaw.filter((friend) => friend.blocked);
        const friendsList = hasBlockedKey ? friendsRaw : friendsRaw.filter((friend) => !friend.blocked);
        setWorld((current) => ({
          ...current,
          stage5Systems: {
            ...current.stage5Systems,
            social: {
              ...(current.stage5Systems.social ?? {}),
              // Names kept for existing consumers (friendCharacterIndex); rich detail
              // in the *Infos fields the adapter prefers (B-wave enrichment).
              friends: friendsList.map((friend) => friend.name),
              blocked: blockedRaw.map((friend) => friend.name),
              friendInfos: friendsList.map((friend) => ({ name: friend.name, online: friend.online, memo: friend.memo })),
              blockedInfos: blockedRaw.map((friend) => ({ name: friend.name, memo: friend.memo })),
            },
          },
        }));
        break;
      }
      case "MentorUpdate":
        setWorld((current) => ({
          ...current,
          stage5Systems: {
            ...current.stage5Systems,
            mentor: {
              ...(current.stage5Systems.mentor ?? {}),
              name: stringOrFallback(payload.name, ""),
              level: numberOrUndefined(payload.level),
              online: payload.online === true,
              menteeExp: numberOrUndefined(payload.menteeExp),
            },
          },
        }));
        break;
      case "LoverUpdate":
        setWorld((current) => ({
          ...current,
          stage5Systems: {
            ...current.stage5Systems,
            relationship: {
              ...(current.stage5Systems.relationship ?? {}),
              name: stringOrFallback(payload.name, ""),
              mapName: stringOrFallback(payload.mapName, ""),
              marriedDays: numberOrUndefined(payload.marriedDays),
            },
          },
        }));
        break;
      case "MarriageRequest":
      case "DivorceRequest":
      case "MentorRequest": {
        const name = stringOrFallback(payload.name, "?");
        const key =
          event.packet === "MarriageRequest"
            ? "server.MarriageRequestFrom"
            : event.packet === "DivorceRequest"
              ? "server.DivorceRequestFrom"
              : "server.MentorRequestFrom";
        appendLog(t(key, [name], `${name} sent you a ${event.packet}.`), "system", "relationship");
        break;
      }

      // Trade ------------------------------------------------------------------
      case "TradeRequest":
      case "TradeAccept": {
        const name = stringOrFallback(payload.name, "?");
        setWorld((current) => ({
          ...current,
          stage5Systems: {
            ...current.stage5Systems,
            trade: {
              ...(current.stage5Systems.trade ?? {}),
              partner: name,
              state: event.packet === "TradeAccept" ? "open" : "requested",
            },
          },
        }));
        appendLog(
          t("server.TradeRequestFrom", [name], `${name} wants to trade.`),
          "system",
          "trade",
        );
        break;
      }
      case "TradeGold":
        setWorld((current) => ({
          ...current,
          stage5Systems: {
            ...current.stage5Systems,
            trade: {
              ...(current.stage5Systems.trade ?? {}),
              partnerGold: numberOrZero(payload.amount),
            },
          },
        }));
        break;
      case "TradeItem":
        setWorld((current) => ({
          ...current,
          stage5Systems: {
            ...current.stage5Systems,
            trade: {
              ...(current.stage5Systems.trade ?? {}),
              partnerItemCount: Array.isArray(payload.partnerItems)
                ? payload.partnerItems.length
                : Array.isArray(payload.tradeItems)
                  ? payload.tradeItems.length
                  : 0,
              // B-wave-2: partner's offered item slots (adaptTrade reads these).
              ...(Array.isArray(payload.partnerItems) ? { partnerItems: payload.partnerItems } : {}),
            },
          },
        }));
        break;
      case "TradeConfirm":
        setWorld((current) => ({
          ...current,
          stage5Systems: {
            ...current.stage5Systems,
            trade: { ...(current.stage5Systems.trade ?? {}), confirmed: true },
          },
        }));
        break;
      case "TradeCancel":
        setWorld((current) => ({
          ...current,
          stage5Systems: { ...current.stage5Systems, trade: null },
        }));
        appendLog(t("server.TradeCancelled", [], "Trade cancelled."), "system", "trade");
        break;

      // Mail -------------------------------------------------------------------
      case "ReceiveMail": {
        const mail = normalizeMailList(payload.mail);
        setWorld((current) => ({
          ...current,
          stage5Systems: { ...current.stage5Systems, mail },
        }));
        break;
      }
      case "MailSent":
      case "ParcelCollected":
        appendLog(
          mailResultMessage(numberOrZero(payload.result), event.packet === "ParcelCollected"),
          "system",
        );
        break;
      case "MailCost":
        appendLog(
          t("ui.mailCost", [numberOrZero(payload.cost)], `Mail cost: ${numberOrZero(payload.cost)} gold.`),
          "system",
        );
        break;

      // Hero management --------------------------------------------------------
      case "NewHero":
        appendLog(heroCreateResultMessage(numberOrZero(payload.result)), "system");
        break;
      case "ManageHeroes":
        setWorld((current) => ({
          ...current,
          stage5Systems: {
            ...current.stage5Systems,
            hero: {
              ...(current.stage5Systems.hero ?? {}),
              maximumCount: numberOrUndefined(payload.maximumCount),
              currentHero: (payload.currentHero as Record<string, unknown> | null) ?? null,
              heroes: Array.isArray(payload.heroes) ? payload.heroes.length : 0,
            },
          },
        }));
        break;
      case "ChangeHero":
      case "SetHeroBehaviour":
      case "UpdateHeroSpawnState":
        setWorld((current) => ({
          ...current,
          stage5Systems: {
            ...current.stage5Systems,
            hero: {
              ...(current.stage5Systems.hero ?? {}),
              ...(event.packet === "SetHeroBehaviour"
                ? { behaviour: numberOrUndefined(payload.behaviour) }
                : {}),
              ...(event.packet === "UpdateHeroSpawnState"
                ? { spawnState: numberOrUndefined(payload.state) }
                : {}),
              ...(event.packet === "ChangeHero"
                ? { fromIndex: numberOrUndefined(payload.fromIndex) }
                : {}),
            },
          },
        }));
        break;

      // Intelligent creatures (pets) ------------------------------------------
      case "NewIntelligentCreature":
      case "UpdateIntelligentCreatureList":
        setWorld((current) => {
          const list = Array.isArray(payload.creatureList)
            ? (payload.creatureList as Array<Record<string, unknown>>)
            : current.stage5Systems.intelligentCreatures ?? [];
          return {
            ...current,
            stage5Systems: { ...current.stage5Systems, intelligentCreatures: list },
          };
        });
        break;

      // Item rental ------------------------------------------------------------
      case "ItemRentalRequest":
      case "ItemRentalFee":
      case "ItemRentalPeriod":
      case "CancelItemRental":
        setWorld((current) => ({
          ...current,
          stage5Systems: {
            ...current.stage5Systems,
            itemRental: {
              ...(current.stage5Systems.itemRental ?? {}),
              ...(event.packet === "ItemRentalRequest"
                ? {
                    partner: stringOrFallback(payload.name, ""),
                    renting: payload.renting === true,
                  }
                : {}),
              ...(event.packet === "ItemRentalFee" ? { fee: numberOrZero(payload.amount) } : {}),
              ...(event.packet === "ItemRentalPeriod" ? { days: numberOrZero(payload.days) } : {}),
              ...(event.packet === "CancelItemRental" ? { partner: null, renting: false } : {}),
            },
          },
        }));
        break;

      // NPC interaction surfaces ----------------------------------------------
      case "NPCGoods":
      case "NPCPearlGoods":
      case "NPCSell":
      case "NPCRepair":
      case "NPCSRepair":
      case "NPCRefine":
      case "NPCReplaceWedRing":
        // Opening an NPC service panel: reuse the inventory surface used by NPCStorage.
        setShowInventory(true);
        setActiveInventoryTab("bag1");
        break;
      case "NPCResponse": {
        const page = Array.isArray(payload.page)
          ? (payload.page as unknown[]).filter((line): line is string => typeof line === "string")
          : [];
        for (const line of page) {
          if (line.trim().length > 0) {
            appendLog(line, "chat", "server");
          }
        }
        break;
      }
      case "NPCCollectRefine":
        appendLog(
          payload.success === true
            ? t("ui.refineCollected", [], "Refined item collected.")
            : t("ui.refineFailed", [], "Refine failed."),
          "system",
        );
        break;

      // Output / misc gameplay messages ---------------------------------------
      case "SendOutputMessage": {
        const message = stringOrFallback(payload.message, "");
        if (message) {
          appendLog(message, "system", "server");
        }
        break;
      }
      case "Roll":
        appendLog(
          t(
            "ui.rollResult",
            [numberOrZero(payload.result)],
            `Roll result: ${numberOrZero(payload.result)}.`,
          ),
          "chat",
          "server",
        );
        break;
      case "OpenBrowser": {
        const url = stringOrFallback(payload.url, "");
        if (url) {
          appendLog(t("ui.openBrowser", [url], `Open link: ${url}`), "system", "server");
        }
        break;
      }
      case "ChangeAMode":
        appendLog(
          t(
            "client.AttackModeChanged",
            [attackModeLabel(numberOrZero(payload.mode))],
            `[Mode: ${attackModeLabel(numberOrZero(payload.mode))}]`,
          ),
          "system",
          "hint",
        );
        break;
      case "ChangePMode":
        appendLog(
          t(
            "client.PetModeChanged",
            [petModeLabel(numberOrZero(payload.mode))],
            `[Pet: ${petModeLabel(numberOrZero(payload.mode))}]`,
          ),
          "system",
          "hint",
        );
        break;
      case "MarketSuccess":
        appendLog(stringOrFallback(payload.message, t("ui.marketSuccess", [], "Market action succeeded.")), "system");
        break;
      case "MarketFail":
        appendLog(t("ui.marketFail", [numberOrZero(payload.reason)], "Market action failed."), "system");
        break;

      // Guild subsystem --------------------------------------------------------
      case "GuildStatus":
        setWorld((current) => ({
          ...current,
          stage5Systems: {
            ...current.stage5Systems,
            guild: {
              ...(current.stage5Systems.guild ?? {}),
              name: stringOrFallback(payload.guildName, current.stage5Systems.guild?.name ?? ""),
              rank: stringOrFallback(payload.guildRankName, current.stage5Systems.guild?.rank ?? ""),
            },
          },
        }));
        break;
      case "GuildMemberChange": {
        const name = stringOrFallback(payload.name, "");
        // change_type (status): 0 = add member, 1 = remove/kick member; other
        // values (2..5) are rank/option/notice edits that do not alter membership.
        const status = numberOrZero(payload.status);
        if (name && (status === 0 || status === 1)) {
          setWorld((current) => ({
            ...current,
            stage5Systems: {
              ...current.stage5Systems,
              guild: {
                ...(current.stage5Systems.guild ?? {}),
                members: groupMembersAfterChange(
                  current.stage5Systems.guild?.members,
                  status === 1 ? { remove: name } : { add: name },
                ),
              },
            },
          }));
        }
        break;
      }
      case "GuildNoticeChange": {
        const notice = Array.isArray(payload.notice)
          ? (payload.notice as unknown[]).filter((line): line is string => typeof line === "string")
          : [];
        if (notice.length > 0) {
          setWorld((current) => ({
            ...current,
            stage5Systems: {
              ...current.stage5Systems,
              guild: { ...(current.stage5Systems.guild ?? {}), chatLog: notice },
            },
          }));
        }
        break;
      }
      case "GuildInvite":
        appendLog(
          t(
            "server.GuildInviteFrom",
            [stringOrFallback(payload.name, "?")],
            `${stringOrFallback(payload.name, "A guild")} invites you to their guild.`,
          ),
          "system",
          "guild",
        );
        break;
      case "GuildExpGain":
        appendLog(
          t(
            "server.GuildExpGain",
            [numberOrZero(payload.amount)],
            `Guild gained ${numberOrZero(payload.amount)} experience.`,
          ),
          "system",
          "guild",
        );
        break;

      // Object appearance / identity sync -------------------------------------
      case "ColourChanged":
        setWorld((current) =>
          current.playerObjectId
            ? {
                ...current,
                entities: patchEntityInList(current.entities, current.playerObjectId, (entity) => ({
                  ...entity,
                  nameColourArgb: numberOrUndefined(payload.nameColourArgb) ?? entity.nameColourArgb,
                })),
              }
            : current,
        );
        break;
      case "ObjectColourChanged": {
        const objectId = stringifyId(payload.objectId);
        setWorld((current) => ({
          ...current,
          entities: patchEntityInList(current.entities, objectId, (entity) => ({
            ...entity,
            nameColourArgb: numberOrUndefined(payload.nameColourArgb) ?? entity.nameColourArgb,
          })),
        }));
        break;
      }
      case "ObjectName":
      case "UserName": {
        const objectId = stringifyId(payload.objectId ?? payload.id);
        const name = stringOrFallback(payload.name, "");
        if (name && objectId !== "0") {
          setWorld((current) => ({
            ...current,
            entities: patchEntityInList(current.entities, objectId, (entity) => ({
              ...entity,
              name,
            })),
          }));
        }
        break;
      }
      case "ObjectLeveled": {
        const objectId = stringifyId(payload.objectId);
        setWorld((current) => ({
          ...current,
          entities: patchEntityInList(current.entities, objectId, (entity) => ({
            ...entity,
            level: typeof entity.level === "number" ? entity.level + 1 : entity.level,
          })),
        }));
        break;
      }
      case "DamageIndicator": {
        // Floating combat text; mirror onto the entity HP where it is the self.
        const objectId = stringifyId(payload.objectId);
        const damage = numberOrZero(payload.damage);
        if (objectId !== "0" && objectId === worldRef.current.playerObjectId && damage > 0) {
          setWorld((current) => ({
            ...current,
            playerHp:
              typeof current.playerHp === "number"
                ? Math.max(0, current.playerHp - damage)
                : current.playerHp,
          }));
        }
        break;
      }
      case "ObjectEffect":
        // One-shot visual effect on an object; keep selection sane.
        restoreObjectSelection(stringifyId(payload.objectId));
        break;

      // Quests -----------------------------------------------------------------
      case "CompleteQuest": {
        const completed = Array.isArray(payload.completedQuests)
          ? (payload.completedQuests as unknown[]).filter(
              (id): id is number => typeof id === "number",
            )
          : [];
        if (completed.length > 0) {
          setWorld((current) => ({
            ...current,
            questLog: current.questLog.map((quest) =>
              completed.includes(quest.questId)
                ? { ...quest, stage: "completed" as QuestStage }
                : quest,
            ),
          }));
          appendLog(t("ui.questCompleted", [], "Quest completed."), "system");
        }
        break;
      }
      case "ChangeQuest": {
        const questId = numberOrUndefined(payload.questId) ?? numberOrUndefined(payload.id);
        const completed = payload.completed === true;
        if (typeof questId === "number") {
          // B-wave-2: thread the live per-task objectives + description.
          const objectives = parseQuestObjectives(payload.objectives);
          const descriptionLines = Array.isArray(payload.descriptionLines)
            ? (payload.descriptionLines as unknown[]).filter((line): line is string => typeof line === "string")
            : undefined;
          setWorld((current) => ({
            ...current,
            questLog: current.questLog.map((quest) =>
              quest.questId === questId
                ? {
                    ...quest,
                    stage: completed
                      ? ("completed" as QuestStage)
                      : payload.taken === true
                        ? ("inProgress" as QuestStage)
                        : quest.stage,
                    ...(objectives ? { objectives } : {}),
                    ...(descriptionLines && descriptionLines.length > 0 ? { descriptionLines } : {}),
                  }
                : quest,
            ),
          }));
        }
        break;
      }
      case "ShareQuest":
        appendLog(
          t(
            "server.QuestShared",
            [stringOrFallback(payload.sharerName, "?")],
            `${stringOrFallback(payload.sharerName, "A party member")} shared a quest.`,
          ),
          "system",
          "group",
        );
        break;

      // Map / navigation -------------------------------------------------------
      case "MapChanged": {
        const fileName = stringOrNull(payload.fileName);
        const miniMap = numberOrUndefined(payload.miniMap);
        const bigMap = numberOrUndefined(payload.bigMap);
        const location = payload.location as { x?: number; y?: number } | undefined;
        setWorld((current) => {
          const mapChanged =
            normalizeMapFileName(fileName) !== normalizeMapFileName(current.mapFileName);
          const preservedSelfEntity = current.playerObjectId
            ? current.entities.find((entity) => entity.objectId === current.playerObjectId)
            : undefined;
          const movedSelf =
            preservedSelfEntity && location
              ? {
                  ...preservedSelfEntity,
                  x: numberOrZero(location.x),
                  y: numberOrZero(location.y),
                  direction: stringOrNull(payload.direction) ?? preservedSelfEntity.direction,
                }
              : preservedSelfEntity;
          const nextWorld = {
            ...current,
            mapTitle: stringOrNull(payload.title) ?? current.mapTitle,
            mapFileName: fileName ?? current.mapFileName,
            miniMapIndex: typeof miniMap === "number" && miniMap > 0 ? miniMap : current.miniMapIndex,
            bigMapIndex: typeof bigMap === "number" && bigMap > 0 ? bigMap : current.bigMapIndex,
            selectedObjectId: mapChanged ? null : current.selectedObjectId,
            activeNpcDialog: mapChanged ? null : current.activeNpcDialog,
            entities: mapChanged && movedSelf ? [movedSelf] : mapChanged ? [] : current.entities,
            groundDrops: mapChanged ? [] : current.groundDrops,
            projectiles: mapChanged ? [] : current.projectiles,
            sceneView: mapChanged ? null : current.sceneView,
            terrainPatches: mapChanged ? [] : current.terrainPatches,
            decorObjects: mapChanged ? [] : current.decorObjects,
            originalMapRegion: mapChanged ? null : current.originalMapRegion,
          };
          worldRef.current = nextWorld;
          return nextWorld;
        });
        break;
      }
      case "SetCompass":
        // Quest/compass marker; surfaced as an interaction hint coordinate.
        {
          const location = payload.location as { x?: number; y?: number } | undefined;
          if (location && typeof location.x === "number" && typeof location.y === "number") {
            appendLog(
              t("ui.compassSet", [location.x, location.y], `Compass set to (${location.x}, ${location.y}).`),
              "system",
            );
          }
        }
        break;

      // Item move/equip acknowledgements (server grid indices) -----------------
      case "EquipItem":
      case "MoveItem":
      case "RemoveItem":
      case "RemoveSlotItem":
      case "StoreItem":
      case "TakeBackItem":
      case "DepositTradeItem":
      case "RetrieveTradeItem":
      case "DepositRefineItem":
      case "RetrieveRefineItem":
      case "TakeBackHeroItem":
      case "TransferHeroItem":
      case "DepositRentalItem":
      case "RetrieveRentalItem":
        // These confirm a slot operation by grid index. The authoritative item
        // layout is reconciled from the next world snapshot / UserSlotsRefresh;
        // surface only a failure note so the player gets feedback on rejection.
        if (payload.success === false) {
          appendLog(t("ui.itemActionFailed", [], "Item action failed."), "system");
        }
        break;
      case "UserSlotsRefresh":
        // Full inventory/equipment refresh handled by the snapshot pipeline; flag
        // the next snapshot as an in-place packet refresh so it merges cleanly.
        packetRuntimeSnapshotModeRef.current = "packetRefresh";
        break;

      // Misc world events ------------------------------------------------------
      case "InTrapRock":
        if (payload.trapped === true) {
          appendLog(t("ui.trappedInRock", [], "You are trapped in rock!"), "system");
        }
        break;
      case "ReturnToLogin":
        screenRef.current = "login";
        setScreen("login");
        setLoginBusy(false);
        break;

      // Skill book -------------------------------------------------------------
      case "NewMagic": {
        const magic = payload.magic as Record<string, unknown> | undefined;
        if (magic && payload.hero !== true) {
          const spell = stringOrFallback(magic.spell, "");
          const name = stringOrFallback(magic.name, spell || "Skill");
          const level = numberOrZero(magic.level);
          const key = spell ? `magic-${spell}` : `magic-${name}`;
          const nextSkill: KnownSkill = {
            key,
            name,
            description: `${name}${level > 0 ? ` (Lv. ${level})` : ""}`,
            spell: spell || null,
            hotkey: numberOrUndefined(magic.key),
            delayMs: numberOrUndefined(magic.delay),
            castTimeMs: numberOrUndefined(magic.cast_time) ?? numberOrUndefined(magic.castTime),
            cooldownRemainingTicks: 0,
          };
          setWorld((current) => ({
            ...current,
            knownSkills: [
              nextSkill,
              ...current.knownSkills.filter((skill) => skill.key !== key),
            ],
          }));
        }
        break;
      }

      // Item split (creates / decrements a stack) ------------------------------
      case "SplitItem": {
        const item = normalizeUserItem(payload.item);
        if (item) {
          appendLog(t("ui.itemSplit", [], "Item split."), "system");
        }
        break;
      }
      case "SplitItem1": {
        const uniqueId = numberOrUndefined(payload.uniqueId);
        const count = numberOrZero(payload.count);
        if (payload.success === true && typeof uniqueId === "number" && count > 0) {
          setWorld((current) => ({
            ...current,
            inventoryItems: patchItemsByUniqueId(current.inventoryItems, {
              uniqueId,
              // The source stack loses `count` units when split into a new slot.
              quantity: Math.max(
                0,
                (current.inventoryItems.find((entry) => entry.uniqueId === uniqueId)?.quantity ?? count) - count,
              ),
            }),
          }));
        }
        break;
      }
      case "CraftItem":
        appendLog(
          payload.success === true
            ? t("ui.craftSuccess", [], "Item crafted.")
            : t("ui.craftFailed", [], "Crafting failed."),
          "system",
        );
        break;
      case "ConsignItem":
        appendLog(
          payload.success === true
            ? t("ui.consignSuccess", [], "Item consigned to the market.")
            : t("ui.consignFailed", [], "Consignment failed."),
          "system",
          "trade",
        );
        break;

      // Item awakening ---------------------------------------------------------
      case "Awakening":
        appendLog(
          numberOrZero(payload.result) > 0
            ? t("ui.awakeningSuccess", [], "Awakening succeeded.")
            : t("ui.awakeningFailed", [], "Awakening failed."),
          "system",
        );
        break;

      // Hero (companion) info --------------------------------------------------
      case "HeroInformation":
      case "NewHeroInfo":
        setWorld((current) => ({
          ...current,
          stage5Systems: {
            ...current.stage5Systems,
            hero: {
              ...(current.stage5Systems.hero ?? {}),
              info: (payload.info as Record<string, unknown> | undefined) ?? current.stage5Systems.hero?.info,
            },
          },
        }));
        break;

      // Self knockback (back-step) --------------------------------------------
      case "UserBackStep": {
        const point = movementPointFromPacketPayload(payload);
        const direction = stringOrNull(payload.direction) ?? undefined;
        setWorld((current) =>
          current.playerObjectId
            ? {
                ...current,
                entities: patchEntityInList(current.entities, current.playerObjectId, (entity) => ({
                  ...entity,
                  x: point.x,
                  y: point.y,
                  direction: direction ?? entity.direction,
                })),
              }
            : current,
        );
        break;
      }

      // Pet pickup -------------------------------------------------------------
      case "IntelligentCreaturePickup":
        appendLog(t("ui.petPickup", [], "Your creature picked up an item."), "system");
        break;

      // Rental item detail -----------------------------------------------------
      case "UpdateRentalItem": {
        const loanItem = normalizeUserItem(payload.loanItem);
        setWorld((current) => ({
          ...current,
          stage5Systems: {
            ...current.stage5Systems,
            itemRental: {
              ...(current.stage5Systems.itemRental ?? {}),
              loanItemUniqueId: loanItem?.uniqueId ?? null,
            },
          },
        }));
        break;
      }

      // Quest definitions ------------------------------------------------------
      case "NewQuestInfo": {
        const info = payload.info as Record<string, unknown> | undefined;
        const questId = info ? numberOrUndefined(info.index) : undefined;
        if (info && typeof questId === "number") {
          const name = stringOrFallback(info.name, `Quest ${questId}`);
          const description = Array.isArray(info.description)
            ? (info.description as unknown[]).filter((line): line is string => typeof line === "string")
            : [];
          const taskDescription = Array.isArray(info.task_description)
            ? (info.task_description as unknown[]).filter((line): line is string => typeof line === "string")
            : Array.isArray(info.taskDescription)
              ? (info.taskDescription as unknown[]).filter((line): line is string => typeof line === "string")
              : [];
          setWorld((current) => {
            if (current.questLog.some((quest) => quest.questId === questId)) {
              return current;
            }
            // B-wave-2: structured description / objectives / rewards / time limit.
            const enrichedObjectives = parseQuestObjectives(payload.objectives);
            const enrichedRewards = parseQuestRewards(payload.rewards);
            const enrichedDescription = Array.isArray(payload.descriptionLines)
              ? (payload.descriptionLines as unknown[]).filter((line): line is string => typeof line === "string")
              : description;
            const nextEntry: QuestEntry = {
              questId,
              title: name,
              summary: description[0] ?? "",
              objective: taskDescription[0] ?? "",
              progressLabel: "",
              tracker: stringOrFallback(info.group, ""),
              stage: "available",
              current: 0,
              required: 0,
              rewardPreview: "",
              ...(enrichedDescription.length > 0 ? { descriptionLines: enrichedDescription } : {}),
              ...(enrichedObjectives ? { objectives: enrichedObjectives } : {}),
              ...(enrichedRewards ? { rewards: enrichedRewards } : {}),
              ...(typeof payload.timeLimit === "string" ? { timeLimit: payload.timeLimit } : {}),
            };
            return { ...current, questLog: [...current.questLog, nextEntry] };
          });
        }
        break;
      }

      // Refine / repair acknowledgements --------------------------------------
      case "RefineItem":
        appendLog(t("ui.refineStarted", [], "Item sent for refining."), "system");
        break;
      case "RepairItem":
        appendLog(t("ui.repairStarted", [], "Item repaired."), "system");
        break;
      case "GuildStorageGoldChange":
        appendLog(
          t(
            "ui.guildStorageGold",
            [numberOrZero(payload.amount)],
            `Guild storage gold changed by ${numberOrZero(payload.amount)}.`,
          ),
          "system",
          "guild",
        );
        break;
      case "AllowObserve":
        appendLog(
          payload.allow === true
            ? t("ui.observeAllowed", [], "Observers are now allowed.")
            : t("ui.observeBlocked", [], "Observers are now blocked."),
          "system",
        );
        break;

      // --- [fe2-packets] additional extended handlers ---
      // Second pass over apps/gateway/src/web.rs `server_packet_to_event`: variants
      // with an observable effect that the first pass left on the default branch.

      // Derived stat sheets ----------------------------------------------------
      case "HeroBaseStatsInfo":
        // Hero (companion) stat sheet -> stored on the stage5 hero slice for panels.
        setWorld((current) => ({
          ...current,
          stage5Systems: {
            ...current.stage5Systems,
            hero: {
              ...(current.stage5Systems.hero ?? {}),
              baseStats: (payload.stats as Record<string, unknown> | undefined) ?? null,
            },
          },
        }));
        break;
      case "BaseStatsInfo":
        // Player derived stats are reconciled from the world snapshot; the explicit
        // packet only needs to flag the next snapshot as an in-place refresh.
        packetRuntimeSnapshotModeRef.current = "packetRefresh";
        break;

      // Account security -------------------------------------------------------
      case "ChangePassword":
        appendLog(
          numberOrZero(payload.result) === 1
            ? t("ui.passwordChanged", [], "Password changed successfully.")
            : t("ui.passwordChangeFailed", [], "Password change failed."),
          "system",
        );
        break;
      case "ChangePasswordBanned":
        appendLog(
          t(
            "ui.passwordChangeBanned",
            [stringOrFallback(payload.reason, "")],
            "Password change is temporarily blocked.",
          ),
          "system",
        );
        break;

      // Item rental lock / confirmation flow -----------------------------------
      case "GetRentedItems": {
        const rentedItems = Array.isArray(payload.rentedItems) ? payload.rentedItems : [];
        setWorld((current) => ({
          ...current,
          stage5Systems: {
            ...current.stage5Systems,
            itemRental: {
              ...(current.stage5Systems.itemRental ?? {}),
              rentedItemCount: rentedItems.length,
            },
          },
        }));
        break;
      }
      case "ItemRentalLock":
        setWorld((current) => ({
          ...current,
          stage5Systems: {
            ...current.stage5Systems,
            itemRental: {
              ...(current.stage5Systems.itemRental ?? {}),
              lockSuccess: payload.success === true,
              goldLocked: payload.goldLocked === true,
              itemLocked: payload.itemLocked === true,
            },
          },
        }));
        break;
      case "ItemRentalPartnerLock":
        setWorld((current) => ({
          ...current,
          stage5Systems: {
            ...current.stage5Systems,
            itemRental: {
              ...(current.stage5Systems.itemRental ?? {}),
              partnerGoldLocked: payload.goldLocked === true,
              partnerItemLocked: payload.itemLocked === true,
            },
          },
        }));
        break;
      case "CanConfirmItemRental":
        setWorld((current) => ({
          ...current,
          stage5Systems: {
            ...current.stage5Systems,
            itemRental: {
              ...(current.stage5Systems.itemRental ?? {}),
              canConfirm: true,
            },
          },
        }));
        break;
      case "ConfirmItemRental":
        setWorld((current) => ({
          ...current,
          stage5Systems: {
            ...current.stage5Systems,
            itemRental: {
              ...(current.stage5Systems.itemRental ?? {}),
              confirmed: true,
            },
          },
        }));
        appendLog(t("ui.itemRentalConfirmed", [], "Item rental confirmed."), "system", "trade");
        break;

      // NPC service surfaces (open the relevant inventory-backed panel) --------
      case "DefaultNPC":
      case "NPCUpdate":
        // Acknowledge that an NPC interaction surface is active. The dialog body
        // arrives via NPCResponse / world snapshot; just keep selection coherent.
        restoreObjectSelection(stringifyId(payload.objectId ?? payload.npcId));
        break;
      case "NPCAwakening":
      case "NPCDisassemble":
      case "NPCDowngrade":
      case "NPCReset":
      case "NPCCheckRefine":
        // Opening an item-service NPC panel; reuse the inventory surface.
        setShowInventory(true);
        setActiveInventoryTab("bag1");
        break;

      // NPC market (consignment auction house) ---------------------------------
      case "NPCMarket":
      case "NPCMarketPage": {
        // B-wave-2: prefer the enriched `auctions` (type/level/expiry/auction) when
        // present, else the legacy `listings`.
        const listings = Array.isArray(payload.auctions)
          ? (payload.auctions as Array<Record<string, unknown>>)
          : Array.isArray(payload.listings)
            ? (payload.listings as Array<Record<string, unknown>>)
            : [];
        setWorld((current) => ({
          ...current,
          stage5Systems: { ...current.stage5Systems, auction: listings },
        }));
        break;
      }

      // Mail compose / lock acknowledgements -----------------------------------
      case "MailSendRequest":
        appendLog(t("ui.mailComposeReady", [], "You can compose mail now."), "system");
        break;
      case "MailLockedItem":
        // Toggle confirmation for an attachment lock in the mail compose window.
        appendLog(
          payload.locked === true
            ? t("ui.mailItemLocked", [], "Mail attachment locked.")
            : t("ui.mailItemUnlocked", [], "Mail attachment unlocked."),
          "system",
        );
        break;

      // Item state changes (seal / slot count / awakening lock) ----------------
      case "ItemSlotSizeChanged": {
        const uniqueId = numberOrUndefined(payload.uniqueId);
        if (typeof uniqueId === "number") {
          // Socketed slot count changed; the next snapshot carries the new sockets.
          packetRuntimeSnapshotModeRef.current = "packetRefresh";
        }
        break;
      }
      case "ItemSealChanged":
        appendLog(t("ui.itemSealChanged", [], "Item seal updated."), "system");
        break;
      case "AwakeningLockedItem":
        appendLog(
          payload.locked === true
            ? t("ui.awakeningItemLocked", [], "Awakening item locked.")
            : t("ui.awakeningItemUnlocked", [], "Awakening item unlocked."),
          "system",
        );
        break;

      // Item / recipe definition catalogues ------------------------------------
      case "NewItemInfo":
      case "NewRecipeInfo": {
        const info = payload.info as Record<string, unknown> | undefined;
        if (info) {
          const name = stringOrFallback(info.name, "") || stringOrFallback(info.itemName, "");
          if (name && event.packet === "NewRecipeInfo") {
            appendLog(t("ui.recipeLearned", [name], `Recipe available: ${name}.`), "system");
          }
        }
        break;
      }

      // Hero management acknowledgements ---------------------------------------
      case "HeroCreateRequest":
        setWorld((current) => ({
          ...current,
          stage5Systems: {
            ...current.stage5Systems,
            hero: {
              ...(current.stage5Systems.hero ?? {}),
              canCreateClass: Array.isArray(payload.canCreateClass) ? payload.canCreateClass : [],
            },
          },
        }));
        break;
      case "UnlockHeroAutoPot":
        setWorld((current) => ({
          ...current,
          stage5Systems: {
            ...current.stage5Systems,
            hero: { ...(current.stage5Systems.hero ?? {}), autoPotUnlocked: true },
          },
        }));
        break;
      case "IntelligentCreatureEnableRename":
        appendLog(t("ui.petRenameEnabled", [], "You can rename your creature now."), "system");
        break;

      // Group member positions (mini-map party markers) ------------------------
      case "GroupMembersMap":
      case "SendMemberLocation": {
        const memberName =
          stringOrFallback(payload.playerName, "") || stringOrFallback(payload.memberName, "");
        if (memberName) {
          const members =
            event.packet === "GroupMembersMap" && Array.isArray(payload.playerMap)
              ? (payload.playerMap as unknown[]).filter((line): line is string => typeof line === "string")
              : undefined;
          if (members && members.length > 0) {
            setWorld((current) => ({
              ...current,
              stage5Systems: {
                ...current.stage5Systems,
                group: { ...(current.stage5Systems.group ?? {}), members },
              },
            }));
          }
        }
        break;
      }

      // Guild buff catalogue ---------------------------------------------------
      case "GuildBuffList": {
        const activeCount = Array.isArray(payload.activeBuffs) ? payload.activeBuffs.length : 0;
        if (activeCount > 0 && payload.remove !== true) {
          appendLog(
            t("ui.guildBuffs", [activeCount], `Guild has ${activeCount} active buff(s).`),
            "system",
            "guild",
          );
        }
        break;
      }

      // Timers (expiring buffs / event countdowns) -----------------------------
      case "SetTimer": {
        const key = stringOrFallback(payload.key, "");
        const seconds = numberOrZero(payload.seconds);
        if (key && seconds > 0) {
          appendLog(
            t("ui.timerSet", [key, seconds], `Timer "${key}" set for ${seconds}s.`),
            "system",
          );
        }
        break;
      }
      case "ExpireTimer": {
        const key = stringOrFallback(payload.key, "");
        if (key) {
          appendLog(t("ui.timerExpired", [key], `Timer "${key}" expired.`), "system");
        }
        break;
      }

      // Decorative / level-up world effects ------------------------------------
      case "ObjectDeco":
        // Static map decoration spawn; harmless beyond keeping selection sane.
        restoreObjectSelection(stringifyId(payload.objectId));
        break;
      case "ObjectLevelEffects":
        // Level-up sparkle on another object; no state change beyond selection.
        restoreObjectSelection(stringifyId(payload.objectId));
        break;
      case "TeleportIn":
        // Self teleport-in flash; the destination is reconciled by the snapshot.
        break;
      case "RefineCancel":
        appendLog(t("ui.refineCancelled", [], "Refining cancelled."), "system");
        break;

      // World ambience ---------------------------------------------------------
      case "TimeOfDay": {
        const lights = numberOrUndefined(payload.lights);
        if (typeof lights === "number") {
          appendLog(
            t("ui.timeOfDay", [lights], `Time of day changed (light ${lights}).`),
            "system",
            "hint",
          );
        }
        break;
      }

      // Cash shop stock --------------------------------------------------------
      case "GameShopInfo": {
        const stockLevel = numberOrUndefined(payload.stockLevel);
        if (typeof stockLevel === "number") {
          appendLog(
            t("ui.gameShopStock", [stockLevel], `Game shop stock: ${stockLevel}.`),
            "system",
            "hint",
          );
        }
        break;
      }

      // Item merge acknowledgement (server grid op) ----------------------------
      case "MergeItem":
        if (payload.success === false) {
          appendLog(t("ui.itemActionFailed", [], "Item action failed."), "system");
        }
        break;

      // --- [fe2-packets] typed-fallback handlers ---
      // These ServerPacket variants have no bespoke arm in the gateway serializer;
      // they arrive via its `typed_packet_event_detail` wildcard, so the variant
      // name is PascalCase and every field is camelCase (rename_all_fields) plus a
      // `typed: true` marker (apps/gateway/src/web.rs).

      // Object appearance / status flags --------------------------------------
      case "ObjectPoisoned": {
        const objectId = stringifyId(payload.objectId);
        if (objectId !== "0" && objectId === worldRef.current.playerObjectId && numberOrZero(payload.poison) > 0) {
          appendLog(t("ui.poisoned", [], "You are poisoned."), "system");
        }
        restoreObjectSelection(objectId);
        break;
      }
      case "PlayerUpdate":
      case "TransformUpdate":
      case "NPCImageUpdate":
        // Equipment / transform / NPC sprite appearance change: the authoritative
        // visuals are re-derived from the next world snapshot.
        restoreObjectSelection(stringifyId(payload.objectId));
        packetRuntimeSnapshotModeRef.current = "packetRefresh";
        break;
      case "SetConcentration": {
        const objectId = stringifyId(payload.objectId);
        if (objectId === "0" || objectId === worldRef.current.playerObjectId) {
          appendLog(
            payload.enabled === true
              ? t("ui.concentrationOn", [], "Concentration active.")
              : t("ui.concentrationOff", [], "Concentration ended."),
            "system",
          );
        }
        break;
      }
      case "SetElemental": {
        const objectId = stringifyId(payload.objectId);
        if (objectId === "0" || objectId === worldRef.current.playerObjectId) {
          const value = numberOrZero(payload.value);
          appendLog(
            payload.enabled === true
              ? t("ui.elementalCharged", [value], `Elemental charge: ${value}.`)
              : t("ui.elementalReleased", [], "Elemental charge released."),
            "system",
          );
        }
        break;
      }
      case "ObjectGuildNameChanged":
        // Guild tag over another player's head; surface as a log line.
        {
          const guildName = stringOrFallback(payload.guildName, "");
          if (guildName) {
            appendLog(t("ui.guildNameChanged", [guildName], `Guild: ${guildName}.`), "system", "guild");
          }
        }
        break;

      // Doors ------------------------------------------------------------------
      case "Opendoor":
        // NB: the gateway emits this as "Opendoor" (lower-case d).
        appendLog(
          t("ui.doorOpened", [numberOrZero(payload.doorIndex)], `Door ${numberOrZero(payload.doorIndex)} opened.`),
          "system",
        );
        break;

      // Server notices ---------------------------------------------------------
      case "UpdateNotice": {
        const notice = payload.notice as Record<string, unknown> | undefined;
        const message = notice ? stringOrFallback(notice.message, "") || stringOrFallback(notice.text, "") : "";
        if (message) {
          appendLog(message, "system", "announcement");
        }
        break;
      }

      // Map search / map metadata ---------------------------------------------
      case "SearchMapResult":
        appendLog(
          t(
            "ui.searchMapResult",
            [numberOrZero(payload.mapIndex)],
            `Map search located map ${numberOrZero(payload.mapIndex)}.`,
          ),
          "system",
        );
        break;
      case "NewMapInfo":
        // Map definition prefetch; the active map is reconciled from the snapshot.
        packetRuntimeSnapshotModeRef.current = "packetRefresh";
        break;

      // Equip-to-slot acknowledgement (server grid op) -------------------------
      case "EquipSlotItem":
        if (payload.success === false) {
          appendLog(t("ui.itemActionFailed", [], "Item action failed."), "system");
        } else {
          packetRuntimeSnapshotModeRef.current = "packetRefresh";
        }
        break;

      // Cash-shop stock update -------------------------------------------------
      case "GameShopStock":
        appendLog(
          t(
            "ui.gameShopStock",
            [numberOrZero(payload.stockLevel)],
            `Game shop stock: ${numberOrZero(payload.stockLevel)}.`,
          ),
          "system",
          "hint",
        );
        break;

      // Auto-potion configuration echo -----------------------------------------
      case "SetAutoPotValue":
      case "SetAutoPotItem":
        setWorld((current) => ({
          ...current,
          stage5Systems: {
            ...current.stage5Systems,
            hero: {
              ...(current.stage5Systems.hero ?? {}),
              ...(event.packet === "SetAutoPotValue"
                ? { autoPotStat: numberOrUndefined(payload.stat), autoPotValue: numberOrUndefined(payload.value) }
                : { autoPotItemIndex: numberOrUndefined(payload.itemIndex) }),
            },
          },
        }));
        break;

      // NPC input / consign panels --------------------------------------------
      case "NPCRequestInput":
        // The dialog text input is surfaced through the snapshot's activeNpcDialog.
        restoreObjectSelection(stringifyId(payload.npcId));
        break;
      case "NPCConsign":
        setShowInventory(true);
        setActiveInventoryTab("bag1");
        break;

      // Awakening material requirement -----------------------------------------
      case "AwakeningNeedMaterials":
        appendLog(t("ui.awakeningNeedMaterials", [], "Awakening requires materials."), "system");
        break;

      // Guild storage / territory / war ----------------------------------------
      case "GuildStorageList": {
        const items = Array.isArray(payload.items) ? payload.items : [];
        const filled = items.filter((entry) => entry != null).length;
        appendLog(
          t("ui.guildStorageList", [filled], `Guild storage holds ${filled} item(s).`),
          "system",
          "guild",
        );
        break;
      }
      case "GuildStorageItemChange":
        if (payload.success === false) {
          appendLog(t("ui.itemActionFailed", [], "Item action failed."), "system");
        }
        break;
      case "GuildTerritoryPage":
        appendLog(
          t(
            "ui.guildTerritoryPage",
            [numberOrZero(payload.page)],
            `Guild territory page ${numberOrZero(payload.page)}.`,
          ),
          "system",
          "guild",
        );
        break;
      case "GuildNameRequest":
        appendLog(t("ui.guildNameRequest", [], "Enter a guild name."), "system", "guild");
        break;
      case "GuildRequestWar":
        appendLog(t("ui.guildRequestWar", [], "Enter a guild to declare war on."), "system", "guild");
        break;

      // Player inspection window ----------------------------------------------
      case "PlayerInspect": {
        const info = payload.info as Record<string, unknown> | undefined;
        const name = info ? stringOrFallback(info.name, "") : "";
        appendLog(
          t("ui.playerInspect", [name || "?"], `Inspecting ${name || "player"}.`),
          "system",
        );
        break;
      }

      // Reincarnation prompts --------------------------------------------------
      case "RequestReincarnation":
        appendLog(t("ui.reincarnationOffer", [], "A reincarnation has been offered to you."), "system");
        break;
      case "CancelReincarnation":
        appendLog(t("ui.reincarnationCancelled", [], "Reincarnation cancelled."), "system");
        break;
      default:
        break;
    }
  }

  function setWorldEntityFromPacket(payload: Record<string, unknown>, kind: EntityKind, disposition: EntityDisposition) {
    const location = payload.location as { x?: number; y?: number } | undefined;
    const objectId = stringifyId(payload.objectId);
    clearPacketRuntimeObjectTombstone(objectId);
    setWorld((current) => {
      const previousEntity = current.entities.find((entity) => entity.objectId === objectId);
      const sprite = spriteFromPacketOrExisting(
        payload,
        kind,
        previousEntity?.sprite ?? null,
        current.mapFileName,
      );
      const nextWorld = {
        ...current,
        entities: upsertEntityInList(current.entities, {
          objectId,
          kind,
          name: stringOrFallback(
            payload.name,
            kind === "npc" ? t("ui.npc") : kind === "monster" ? t("ui.monster") : t("ui.player"),
          ),
          ownerName: stringOrNull(payload.ownerName) ?? undefined,
          x: numberOrZero(location?.x),
          y: numberOrZero(location?.y),
          direction: stringOrNull(payload.direction) ?? undefined,
          classKey: kind === "player" || kind === "selfPlayer" ? mapClassKey(payload.class) : undefined,
          genderKey: kind === "player" || kind === "selfPlayer" ? mapGenderKey(payload.gender) : undefined,
          level: numberOrUndefined(payload.level),
          nameColourArgb: numberOrUndefined(payload.nameColourArgb) ?? (kind === "npc" ? -16_711_936 : -1),
          dead: payload.dead === true,
          disposition,
          sprite,
          bigMapIcon: numberOrUndefined(payload.bigMapIcon),
          showOnBigMap: payload.showOnBigMap === true ? true : payload.showOnBigMap === false ? false : undefined,
          canTeleportTo: payload.canTeleportTo === true ? true : payload.canTeleportTo === false ? false : undefined,
        }),
      };
      worldRef.current = nextWorld;
      return nextWorld;
    });
  }

  function setWorldGroundDropFromPacket(payload: Record<string, unknown>, fallbackName: string) {
    const location = payload.location as { x?: number; y?: number } | undefined;
    const objectId = stringifyId(payload.objectId);

    clearPacketRuntimeObjectTombstone(objectId);
    setWorld((current) => {
      const nextWorld = {
        ...current,
        groundDrops: upsertGroundDropInList(current.groundDrops, {
          objectId,
          name: stringOrFallback(payload.name, fallbackName),
          nameColourArgb: numberOrUndefined(payload.nameColourArgb),
          x: numberOrZero(location?.x),
          y: numberOrZero(location?.y),
          quantity: numberOrUndefined(payload.gold) ?? numberOrUndefined(payload.quantity) ?? 1,
          sourceMonster:
            current.groundDrops.find((drop) => drop.objectId === objectId)?.sourceMonster ??
            t("ui.unknown", [], "Unknown"),
        }),
      };
      worldRef.current = nextWorld;
      return nextWorld;
    });
  }

  function removeObjectFromWorld(objectId: string) {
    rememberPacketRuntimeObjectRemoved(objectId);
    setWorld((current) => {
      const nextWorld = {
        ...current,
        selectedObjectId: current.selectedObjectId === objectId ? null : current.selectedObjectId,
        activeNpcDialog: current.activeNpcDialog?.npcObjectId === objectId ? null : current.activeNpcDialog,
        entities: current.entities.filter((entity) => entity.objectId !== objectId),
        groundDrops: current.groundDrops.filter((drop) => drop.objectId !== objectId),
      };
      worldRef.current = nextWorld;
      return nextWorld;
    });
  }

  function restoreObjectSelection(objectId: string) {
    if (objectId === "0") return;

    setWorld((current) => ({
      ...current,
      selectedObjectId:
        current.selectedObjectId && current.selectedObjectId !== objectId ? current.selectedObjectId : objectId,
    }));
  }

  function updateWorldEntityFromLocationPacket(payload: Record<string, unknown>) {
    const location = payload.location as { x?: number; y?: number } | undefined;
    const objectId = stringifyId(payload.objectId);

    setWorld((current) => {
      const nextWorld = {
        ...current,
        entities: patchEntityInList(current.entities, objectId, (entity) => ({
          ...entity,
          x: numberOrZero(location?.x),
          y: numberOrZero(location?.y),
          direction: stringOrNull(payload.direction) ?? entity.direction,
        })),
      };
      worldRef.current = nextWorld;
      return nextWorld;
    });
  }

  function markWorldEntityAttack(payload: Record<string, unknown>) {
    const objectId = stringifyId(payload.objectId);
    const location = payload.location as { x?: number; y?: number } | undefined;
    const now = Date.now();

    setWorld((current) => ({
      ...current,
      entities: patchEntityInList(current.entities, objectId, (entity) => {
        const animation = attackAnimationVariant(payload);
        return {
          ...entity,
          x: typeof location?.x === "number" ? location.x : entity.x,
          y: typeof location?.y === "number" ? location.y : entity.y,
          direction: stringOrNull(payload.direction) ?? entity.direction,
          attackAnimation: animation,
          attackStartedAt: now,
          attackUntil: now + crystalAttackActionDurationMs(entity, animation),
        };
      }),
    }));
  }

  function markWorldEntityMagic(payload: Record<string, unknown>) {
    const objectId = stringifyId(payload.objectId);
    const location = payload.location as { x?: number; y?: number } | undefined;
    const now = Date.now();

    setWorld((current) => ({
      ...current,
      entities: patchEntityInList(current.entities, objectId, (entity) => ({
        ...entity,
        x: typeof location?.x === "number" ? location.x : entity.x,
        y: typeof location?.y === "number" ? location.y : entity.y,
        direction: stringOrNull(payload.direction) ?? entity.direction,
        attackAnimation: "range",
        attackStartedAt: now,
        attackUntil: now + crystalAttackActionDurationMs(entity, "range"),
      })),
    }));
  }

  function markPlayerMagic(payload: Record<string, unknown>) {
    const now = Date.now();

    setWorld((current) => {
      const playerObjectId = current.playerObjectId;
      if (!playerObjectId) {
        return current;
      }

      return {
        ...current,
        entities: patchEntityInList(current.entities, playerObjectId, (entity) => ({
          ...entity,
          attackAnimation: "range",
          attackStartedAt: now,
          attackUntil: now + crystalAttackActionDurationMs(entity, "range"),
        })),
      };
    });
  }

  function markWorldEntityStruck(payload: Record<string, unknown>) {
    const objectId = stringifyId(payload.objectId);
    const location = payload.location as { x?: number; y?: number } | undefined;
    const now = Date.now();

    setWorld((current) => ({
      ...current,
      entities: patchEntityInList(current.entities, objectId, (entity) => ({
        ...entity,
        x: typeof location?.x === "number" ? location.x : entity.x,
        y: typeof location?.y === "number" ? location.y : entity.y,
        direction: stringOrNull(payload.direction) ?? entity.direction,
        struckStartedAt: now,
        struckUntil: now + crystalStruckActionDurationMs(entity),
      })),
    }));
  }

  function markPlayerStruck(payload: Record<string, unknown>) {
    const now = Date.now();
    const attackerId = stringifyId(payload.attackerId);

    setWorld((current) => ({
      ...current,
      selectedObjectId:
        current.selectedObjectId && current.selectedObjectId !== attackerId ? current.selectedObjectId : attackerId,
      entities: current.playerObjectId
        ? patchEntityInList(current.entities, current.playerObjectId, (entity) => ({
            ...entity,
            struckStartedAt: now,
            struckUntil: now + crystalStruckActionDurationMs(entity),
          }))
        : current.entities,
    }));
  }

  function spawnRangeProjectile(payload: Record<string, unknown>) {
    const attackerId = stringifyId(payload.objectId ?? payload.sourceId);
    const targetId = stringifyId(payload.targetId ?? payload.destinationId);
    if (attackerId === "0" || targetId === "0") {
      return;
    }

    setWorld((current) => {
      const attacker = current.entities.find((entity) => entity.objectId === attackerId);
      const target = current.entities.find((entity) => entity.objectId === targetId);
      if (!attacker || !target) {
        return current;
      }

      const startedAt = Date.now();
      const animation = "range";
      const durationMs = crystalAttackActionDurationMs(attacker, animation);
      const projectile: ProjectileState = {
        key: `${attackerId}:${targetId}:${startedAt}`,
        attackerId,
        targetId,
        fromX: attacker.x,
        fromY: attacker.y,
        toX: target.x,
        toY: target.y,
        startedAt,
        expiresAt: startedAt + durationMs,
      };

      return {
        ...current,
        entities: patchEntityInList(current.entities, attackerId, (entity) => ({
          ...entity,
          direction: stringOrNull(payload.direction) ?? entity.direction,
          attackStartedAt: startedAt,
          attackAnimation: animation,
          attackUntil: startedAt + durationMs,
        })),
        projectiles: [...current.projectiles.filter((entry) => entry.expiresAt > startedAt), projectile],
      };
    });

  }

  function markWorldEntityDead(payload: Record<string, unknown>) {
    const location = payload.location as { x?: number; y?: number } | undefined;
    const objectId = stringifyId(payload.objectId);
    const now = Date.now();

    setWorld((current) => ({
      ...current,
      playerHp: current.playerObjectId === objectId ? 0 : current.playerHp,
      entities: patchEntityInList(current.entities, objectId, (entity) => ({
        ...entity,
        x: numberOrZero(location?.x),
        y: numberOrZero(location?.y),
        direction: stringOrNull(payload.direction) ?? entity.direction,
        hp: 0,
        dead: true,
        dieStartedAt: now,
        dieUntil: now + crystalDeathActionDurationMs(entity),
        attackAnimation: undefined,
        attackStartedAt: undefined,
        attackUntil: undefined,
        struckStartedAt: undefined,
        struckUntil: undefined,
        reviveStartedAt: undefined,
        reviveUntil: undefined,
      })),
    }));
  }

  function markWorldEntityRevived(payload: Record<string, unknown>) {
    const objectId = stringifyId(payload.objectId);
    const now = Date.now();

    setWorld((current) => ({
      ...current,
      playerHp:
        current.playerObjectId === objectId && typeof current.playerMaxHp === "number"
          ? Math.max(1, current.playerMaxHp)
          : current.playerHp,
      entities: patchEntityInList(current.entities, objectId, (entity) => ({
        ...entity,
        hp: typeof entity.maxHp === "number" ? Math.max(1, entity.maxHp) : entity.hp,
        dead: false,
        dieStartedAt: undefined,
        dieUntil: undefined,
        reviveStartedAt: now,
        reviveUntil: now + crystalDeathActionDurationMs(entity),
      })),
    }));
  }

  function applyMagicDelayPacket(payload: Record<string, unknown>) {
    const spell = stringOrFallback(payload.spell, "");
    const delay = numberOrZero(payload.delay);
    if (!spell) return;

    setWorld((current) => ({
      ...current,
      knownSkills: current.knownSkills.map((skill) =>
        skillMatchesCrystalSpell(skill, spell) ? { ...skill, cooldownRemainingTicks: delay } : skill,
      ),
    }));
  }

  function applyMagicLeveledPacket(payload: Record<string, unknown>) {
    const spell = stringOrFallback(payload.spell, "");
    const level = numberOrUndefined(payload.level);
    if (!spell || typeof level !== "number") return;

    setWorld((current) => ({
      ...current,
      knownSkills: current.knownSkills.map((skill) =>
        skillMatchesCrystalSpell(skill, spell)
          ? { ...skill, description: `${skill.description.replace(/\s*\(Lv\.\s*\d+\)$/i, "")} (Lv. ${level})` }
          : skill,
      ),
    }));
  }

  function applyAddBuffPacket(payload: Record<string, unknown>) {
    const buffType = numberOrUndefined(payload.buffType);
    const objectId = stringifyId(payload.objectId);
    if (typeof buffType !== "number" || payload.visible === false) return;

    setWorld((current) => {
      if (current.playerObjectId && objectId !== "0" && objectId !== current.playerObjectId) {
        return current;
      }

      const key = crystalBuffKey(objectId, buffType);
      const remainingTicks = payload.infinite === true ? 0 : crystalBuffRemainingTicks(numberOrUndefined(payload.expireTime));
      const nextBuff: ActiveBuff = {
        key,
        name: `Buff ${buffType}`,
        description: payload.paused === true ? "Paused Crystal buff" : "Crystal buff",
        remainingTicks,
        attackBonus: 0,
        defenceBonus: 0,
        // B-wave enriched fields.
        type: buffType,
        infinite: payload.infinite === true,
        paused: payload.paused === true,
        stats: Array.isArray(payload.stats)
          ? (payload.stats as Array<Record<string, unknown>>).map((entry) => ({
              label: typeof entry.label === "string" ? entry.label : undefined,
              value: numberOrUndefined(entry.value),
            }))
          : undefined,
      };

      return {
        ...current,
        activeBuffs: [nextBuff, ...current.activeBuffs.filter((buff) => buff.key !== key)],
      };
    });
  }

  function applyRemoveBuffPacket(payload: Record<string, unknown>) {
    const buffType = numberOrUndefined(payload.buffType);
    const objectId = stringifyId(payload.objectId);
    if (typeof buffType !== "number") return;
    const key = crystalBuffKey(objectId, buffType);

    setWorld((current) => ({
      ...current,
      activeBuffs: current.activeBuffs.filter((buff) => buff.key !== key),
    }));
  }

  function applyPauseBuffPacket(payload: Record<string, unknown>) {
    const buffType = numberOrUndefined(payload.buffType);
    const objectId = stringifyId(payload.objectId);
    const paused = payload.paused === true;
    if (typeof buffType !== "number") return;
    const key = crystalBuffKey(objectId, buffType);

    setWorld((current) => ({
      ...current,
      activeBuffs: current.activeBuffs.map((buff) =>
        buff.key === key ? { ...buff, description: paused ? "Paused Crystal buff" : "Crystal buff" } : buff,
      ),
    }));
  }

  function applyObjectHealthPacket(payload: Record<string, unknown>) {
    const objectId = stringifyId(payload.objectId);
    const percent = numberOrUndefined(payload.percent);
    const exactHp = numberOrUndefined(payload.hp);
    const exactMaxHp = numberOrUndefined(payload.maxHp);

    if (typeof percent !== "number") return;

    setWorld((current) => {
      const isSelf = current.playerObjectId === objectId;
      const selfMaxHp = exactMaxHp ?? current.playerMaxHp;
      const nextPlayerHp =
        isSelf && typeof exactHp === "number"
          ? Math.max(0, exactHp)
          : isSelf && typeof selfMaxHp === "number"
            ? Math.max(0, Math.round((selfMaxHp * percent) / 100))
            : isSelf && percent <= 0
              ? 0
              : current.playerHp;
      const now = Date.now();

      return {
        ...current,
        playerHp: nextPlayerHp,
        playerMaxHp: isSelf && typeof exactMaxHp === "number" ? exactMaxHp : current.playerMaxHp,
        entities: patchEntityInList(current.entities, objectId, (entity) => {
          const entityMaxHp = exactMaxHp ?? entity.maxHp;
          const nextHp = isSelf
            ? (nextPlayerHp ?? entity.hp)
            : typeof exactHp === "number"
              ? Math.max(0, exactHp)
              : typeof entityMaxHp === "number"
                ? Math.max(0, Math.round((entityMaxHp * percent) / 100))
                : percent <= 0
                  ? 0
                  : entity.hp;
          const nextMaxHp =
            typeof exactMaxHp === "number"
              ? exactMaxHp
              : typeof entity.maxHp === "number"
                ? entity.maxHp
                : undefined;
          const died = percent <= 0;
          const revived = percent > 0 && entity.dead;

          return {
            ...entity,
            hp: nextHp,
            maxHp: nextMaxHp,
            dead: died,
            dieStartedAt: died && !entity.dead ? now : entity.dieStartedAt,
            dieUntil: died && !entity.dead ? now + crystalDeathActionDurationMs(entity) : entity.dieUntil,
            reviveStartedAt: revived ? now : entity.reviveStartedAt,
            reviveUntil: revived ? now + 420 : entity.reviveUntil,
          };
        }),
      };
    });
  }

  function applyObjectManaPacket(payload: Record<string, unknown>) {
    const objectId = stringifyId(payload.objectId);
    const percent = numberOrUndefined(payload.percent);
    const exactMp = numberOrUndefined(payload.mp);

    if (typeof percent !== "number") return;

    setWorld((current) => ({
      ...current,
      playerMp:
        current.playerObjectId === objectId
          ? Math.max(0, Math.round(exactMp ?? percent))
          : current.playerMp,
    }));
  }

  function applyRankingPacket(payload: Record<string, unknown>) {
    const request = lastRankingRequestRef.current;
    const rankType = numberOrUndefined(payload.rankType) ?? request.rankType;
    const rankIndex = request.rankType === rankType ? request.rankIndex : 0;
    const onlineOnly = request.rankType === rankType ? request.onlineOnly : false;
    const listingDetails = Array.isArray(payload.listingDetails) ? payload.listingDetails : [];
    const listings = Array.isArray(payload.listings) ? payload.listings : [];
    const entries = listingDetails.flatMap((entry, index): RankingEntry[] => {
      if (!entry || typeof entry !== "object") return [];
      const record = entry as Record<string, unknown>;
      return [
        {
          rank: rankIndex + index + 1,
          playerId: numberOrUndefined(record.playerId ?? listings[index]) ?? 0,
          name: stringOrFallback(record.name, `Player ${rankIndex + index + 1}`),
          level: numberOrUndefined(record.level) ?? 0,
          classKey: mapClassKey(record.class),
        },
      ];
    });
    const page: RankingState = {
      rankType,
      rankIndex,
      onlineOnly,
      myRank: numberOrUndefined(payload.myRank) ?? 0,
      count: numberOrUndefined(payload.count) ?? entries.length,
      entries,
      updatedAt: Date.now(),
    };
    const key = rankingPageKey(rankType, onlineOnly);

    setWorld((current) => {
      const nextWorld = {
        ...current,
        rankings: {
          ...current.rankings,
          [key]: page,
        },
        rankingCurrentKey: key,
      };
      worldRef.current = nextWorld;
      return nextWorld;
    });
  }

  function applyGatewayWorldSnapshot(snapshot: GatewayWorldSnapshot) {
    const playerObjectId = snapshot.playerObjectId === null ? null : String(snapshot.playerObjectId);
    const previousEntitiesById = new Map(worldRef.current.entities.map((entity) => [entity.objectId, entity]));
    const snapshotNow = Date.now();
    const entities: WorldEntity[] = snapshot.entities.map((entity) => ({
      objectId: String(entity.objectId),
      kind: entity.kind,
      name: entity.name,
      ownerName: entity.ownerName ?? undefined,
      x: entity.x,
      y: entity.y,
      direction: entity.direction,
      classKey:
        entity.class === undefined || entity.class === null ? undefined : mapClassKey(entity.class),
      genderKey:
        entity.gender === undefined || entity.gender === null
          ? undefined
          : mapGenderKey(entity.gender),
      level: entity.level ?? undefined,
      hp: entity.hp ?? undefined,
      maxHp: entity.maxHp ?? undefined,
      nameColourArgb: entity.nameColourArgb ?? undefined,
      dead: entity.dead,
      disposition: entity.disposition,
      sprite: spriteFromSnapshotEntity(
        entity,
        snapshot.mapFileName,
        previousEntitiesById.get(String(entity.objectId))?.sprite ?? null,
      ),
      questIds: Array.isArray(entity.questIds) ? entity.questIds : [],
      bigMapIcon: entity.bigMapIcon ?? undefined,
      showOnBigMap: entity.showOnBigMap ?? undefined,
      canTeleportTo: entity.canTeleportTo ?? undefined,
      ...preservedMovementAnimation(
        previousEntitiesById.get(String(entity.objectId)),
        entity.x,
        entity.y,
        snapshotNow,
      ),
    }));
    const groundDrops: GroundDrop[] = (snapshot.groundDrops ?? []).map((drop) => ({
      objectId: String(drop.objectId),
      name: drop.name,
      nameColourArgb: drop.nameColourArgb ?? undefined,
      x: drop.x,
      y: drop.y,
      quantity: drop.quantity,
      sourceMonster: drop.sourceMonster,
    }));
    const inventoryItems = snapshot.inventoryItems.map((item) => ({
      key: item.key,
      name: item.name,
      icon: item.icon,
      uniqueId: item.uniqueId ?? (item.container === "bag2" ? 40 + item.slot : item.slot),
      slot: item.slot,
      container: item.container,
      quantity: item.quantity,
      description: item.description,
      durabilityCurrent: item.durabilityCurrent ?? undefined,
      durabilityMax: item.durabilityMax ?? undefined,
    }));
    const beltItems = snapshot.beltItems.map((item) => ({
      key: item.key,
      name: item.name,
      icon: item.icon,
      uniqueId: item.uniqueId ?? item.slot,
      slot: item.slot,
      container: item.container,
      quantity: item.quantity,
      description: item.description,
      durabilityCurrent: item.durabilityCurrent ?? undefined,
      durabilityMax: item.durabilityMax ?? undefined,
    }));
    const storageItems = (snapshot.storageItems ?? []).map((item) => ({
      key: item.key,
      name: item.name,
      icon: item.icon,
      uniqueId: item.uniqueId ?? item.slot,
      slot: item.slot,
      container: item.container,
      quantity: item.quantity,
      description: item.description,
      durabilityCurrent: item.durabilityCurrent ?? undefined,
      durabilityMax: item.durabilityMax ?? undefined,
    }));
    const equipmentItems = snapshot.equipmentItems.map((item) => ({
      slot: item.slot,
      name: item.name,
      icon: item.icon,
      shape: item.shape ?? undefined,
      description: item.description,
      durabilityCurrent: item.durabilityCurrent,
      durabilityMax: item.durabilityMax,
      attack: item.attack,
      defence: item.defence,
    }));
    const questLog = snapshot.questLog.map((quest) => ({
      questId: quest.questId,
      title: quest.title,
      summary: quest.summary,
      objective: quest.objective,
      progressLabel: quest.progressLabel,
      tracker: quest.tracker,
      stage: quest.stage,
      current: quest.current,
      required: quest.required,
      rewardPreview: quest.rewardPreview,
    }));
    const knownSkills = (snapshot.knownSkills ?? []).map((skill) => ({
      key: skill.key,
      name: skill.name,
      description: skill.description,
      spell: skill.spell ?? null,
      castKind: skill.castKind,
      offensive: skill.offensive,
      hotkey: skill.hotkey,
      delayMs: skill.delayMs,
      castTimeMs: skill.castTimeMs,
      cooldownRemainingTicks: skill.cooldownRemainingTicks,
    }));
    const activeBuffs = (snapshot.activeBuffs ?? []).map((buff) => ({
      key: buff.key,
      name: buff.name,
      description: buff.description,
      remainingTicks: buff.remainingTicks,
      attackBonus: buff.attackBonus,
      defenceBonus: buff.defenceBonus,
      // B-wave enriched fields (Crystal buff type + stat lines + flags).
      type: buff.buffType,
      infinite: buff.infinite,
      paused: buff.paused,
      stats: buff.stats,
    }));
    const mapTransfers = (snapshot.mapTransfers ?? []).map((transfer) => ({
      key: transfer.key,
      mapFileName: transfer.mapFileName,
      minX: transfer.bounds.minX,
      maxX: transfer.bounds.maxX,
      minY: transfer.bounds.minY,
      maxY: transfer.bounds.maxY,
      toMapFileName: transfer.toMapFileName,
      toMapTitle: transfer.toMapTitle,
    }));
    const activeNpcDialog = snapshot.activeNpcDialog
      ? {
          npcObjectId: String(snapshot.activeNpcDialog.npcObjectId),
          npcName: snapshot.activeNpcDialog.npcName,
          title: snapshot.activeNpcDialog.title,
          body: snapshot.activeNpcDialog.body,
          footer: snapshot.activeNpcDialog.footer,
          links: Array.isArray(snapshot.activeNpcDialog.links)
            ? snapshot.activeNpcDialog.links.flatMap((link) => {
                if (!link || typeof link !== "object") return [];
                const value = link as { text?: unknown; target?: unknown };
                return [
                  {
                    text: stringOrFallback(value.text, ""),
                    target: stringOrFallback(value.target, ""),
                  },
                ].filter((entry) => entry.text && entry.target);
              })
            : [],
          input: snapshot.activeNpcDialog.input ?? null,
        }
      : null;

    let followUpEntities = entities;
    let followUpMapTransfers = mapTransfers;

    setWorld((current) => {
      const currentTime = Date.now();
      const transientByObjectId = new Map(
        current.entities.map((entity) => [
          entity.objectId,
          {
            attackAnimation: entity.attackAnimation,
            attackStartedAt: entity.attackStartedAt,
            attackUntil: entity.attackUntil,
            struckStartedAt: entity.struckStartedAt,
            struckUntil: entity.struckUntil,
            dieStartedAt: entity.dieStartedAt,
            dieUntil: entity.dieUntil,
            reviveStartedAt: entity.reviveStartedAt,
            reviveUntil: entity.reviveUntil,
          },
        ]),
      );
      const mergedEntities = entities.map((entity) => {
        const baseEntity = entity;
        const transient = transientByObjectId.get(entity.objectId);
        if (!transient) {
          return baseEntity;
        }

        return {
          ...baseEntity,
          attackAnimation:
            typeof transient.attackUntil === "number" && transient.attackUntil > currentTime
              ? transient.attackAnimation
              : undefined,
          attackStartedAt:
            typeof transient.attackUntil === "number" && transient.attackUntil > currentTime
              ? transient.attackStartedAt
              : undefined,
          attackUntil:
            typeof transient.attackUntil === "number" && transient.attackUntil > currentTime
              ? transient.attackUntil
              : undefined,
          struckStartedAt:
            typeof transient.struckUntil === "number" && transient.struckUntil > currentTime
              ? transient.struckStartedAt
              : undefined,
          struckUntil:
            typeof transient.struckUntil === "number" && transient.struckUntil > currentTime
              ? transient.struckUntil
              : undefined,
          dieStartedAt:
            typeof transient.dieUntil === "number" && transient.dieUntil > currentTime
              ? transient.dieStartedAt
              : undefined,
          dieUntil:
            typeof transient.dieUntil === "number" && transient.dieUntil > currentTime
              ? transient.dieUntil
              : undefined,
          reviveStartedAt:
            typeof transient.reviveUntil === "number" && transient.reviveUntil > currentTime
              ? transient.reviveStartedAt
              : undefined,
          reviveUntil:
            typeof transient.reviveUntil === "number" && transient.reviveUntil > currentTime
              ? transient.reviveUntil
              : undefined,
        };
      });
      const snapshotSelfEntity = mergedEntities.find((entity) => entity.objectId === playerObjectId) ?? null;
      const snapshotMapFileName = snapshot.mapFileName ?? current.mapFileName;
      const snapshotMapChanged =
        normalizeMapFileName(snapshotMapFileName) !== normalizeMapFileName(current.mapFileName);
      const snapshotRealtimeMode = classifyWorldSnapshotRealtimeMode(
        snapshot,
        current,
        playerObjectId,
        snapshotMapChanged,
      );
      const packetRuntimeRefresh = snapshotRealtimeMode === "packetRefresh";
      packetRuntimeSnapshotModeRef.current = snapshotRealtimeMode;
      if (!packetRuntimeRefresh) {
        packetRuntimeObjectTombstonesRef.current.clear();
      }
      const packetRuntimeDebugWindow = window as typeof window & {
        __mir2PacketRuntime?: Record<string, unknown>;
      };
      packetRuntimeDebugWindow.__mir2PacketRuntime = {
        snapshotMode: snapshotRealtimeMode,
        packetRuntimeRefresh,
        lastSnapshotAt: currentTime,
        tick: snapshot.tick,
        mapFileName: snapshotMapFileName,
        mapChanged: snapshotMapChanged,
        snapshotEntityCount: entities.length,
        currentEntityCount: current.entities.length,
        tombstoneCount: packetRuntimeObjectTombstonesRef.current.size,
      };
      if (snapshotSelfEntity) {
        if (!packetRuntimeRefresh) {
          rememberSnapshotSelfAck(snapshotSelfEntity, currentTime, snapshotMapChanged);
        }
        reconcileSelfMovementSnapshot(snapshotSelfEntity, currentTime);
      }
      const hasCurrentSceneForSnapshot =
        current.originalMapRegion !== null &&
        normalizeMapFileName(current.originalMapRegion.mapFileName) === normalizeMapFileName(snapshotMapFileName);
      if (snapshotSelfEntity) {
        recordMovementDiagnostic("apply:worldSnapshotSelf", {
          tick: snapshot.tick,
          mapChanged: snapshotMapChanged,
          realtimeMode: snapshotRealtimeMode,
          snapshotSelf: {
            x: snapshotSelfEntity.x,
            y: snapshotSelfEntity.y,
            direction: snapshotSelfEntity.direction,
          },
          localOverride: null,
          beforeSelf:
            current.entities.find((entity) => entity.objectId === playerObjectId) ??
            null,
        });
      }
      const mergedEntitiesForWorld = packetRuntimeRefresh
        ? mergePacketFirstSnapshotEntities(current.entities, mergedEntities, currentTime)
        : mergedEntities;
      const mergedGroundDropsForWorld = packetRuntimeRefresh
        ? mergePacketFirstSnapshotGroundDrops(current.groundDrops, groundDrops, currentTime)
        : groundDrops;
      const effectiveSelfEntity =
        mergedEntitiesForWorld.find((entity) => entity.objectId === playerObjectId) ?? snapshotSelfEntity;

      if (packetRuntimeRefresh) {
        recordMovementDiagnostic("apply:worldSnapshotPacketRefresh", {
          tick: snapshot.tick,
          snapshotEntityCount: entities.length,
          currentEntityCount: current.entities.length,
          mergedEntityCount: mergedEntitiesForWorld.length,
          snapshotGroundDropCount: groundDrops.length,
          currentGroundDropCount: current.groundDrops.length,
          mergedGroundDropCount: mergedGroundDropsForWorld.length,
          snapshotSelf: snapshotSelfEntity
            ? {
                x: snapshotSelfEntity.x,
                y: snapshotSelfEntity.y,
                direction: snapshotSelfEntity.direction,
              }
            : null,
          currentSelf:
            current.entities.find((entity) => entity.objectId === playerObjectId) ??
            null,
        });
      }

      const nextWorld = {
        ...current,
        mapTitle: snapshot.mapTitle ?? current.mapTitle,
        mapFileName: snapshotMapFileName,
        inSafeZone: snapshot.inSafeZone ?? current.inSafeZone,
        playerObjectId,
        playerName: effectiveSelfEntity?.name ?? current.playerName,
        playerHp: snapshot.playerHp ?? undefined,
        playerMaxHp: snapshot.playerMaxHp ?? undefined,
        playerMp: snapshot.playerMp ?? undefined,
        playerExperience: snapshot.playerExperience,
        playerMaxExperience: Math.max(snapshot.playerMaxExperience, 1),
        gold: snapshot.gold,
        credit: snapshot.credit,
        cityCurrencies: snapshot.cityCurrencies ?? current.cityCurrencies,
        currentWeight: snapshot.currentWeight,
        maxWeight: snapshot.maxWeight,
        freeBagSlots: snapshot.freeBagSlots,
        maxBagSlots: snapshot.maxBagSlots,
        storageSize: snapshot.storageSize ?? current.storageSize,
        hasExpandedStorage: snapshot.hasExpandedStorage ?? current.hasExpandedStorage,
        hasStoragePassword: snapshot.hasStoragePassword ?? current.hasStoragePassword,
        requireStoragePassword: snapshot.requireStoragePassword ?? current.requireStoragePassword,
        storageSessionUnlocked:
          snapshot.requireStoragePassword === undefined
            ? current.storageSessionUnlocked
            : snapshot.requireStoragePassword !== true,
        storagePasswordLastSetBinaryDatetime:
          snapshot.storagePasswordLastSetBinaryDatetime ?? current.storagePasswordLastSetBinaryDatetime,
        expandedStorageExpiryTimeBinaryDatetime:
          snapshot.expandedStorageExpiryTimeBinaryDatetime ??
          current.expandedStorageExpiryTimeBinaryDatetime,
        worldTick: snapshot.tick,
        sceneView: snapshot.sceneView ?? current.sceneView,
        terrainPatches:
          hasCurrentSceneForSnapshot && current.terrainPatches.length
            ? current.terrainPatches
            : snapshot.terrainPatches.length
              ? snapshot.terrainPatches
              : current.terrainPatches,
        decorObjects:
          hasCurrentSceneForSnapshot && current.decorObjects.length
            ? current.decorObjects
            : snapshot.decorObjects.length
              ? snapshot.decorObjects
              : current.decorObjects,
        originalMapRegion: current.originalMapRegion,
        entities: mergedEntitiesForWorld,
        groundDrops: mergedGroundDropsForWorld,
        mineNodes: current.mineNodes,
        beltItems,
        inventoryItems,
        storageItems,
        equipmentItems,
        questLog,
        activeNpcDialog,
        knownSkills,
        activeBuffs,
        stage5Systems: snapshot.stage5Systems ?? current.stage5Systems,
        mapTransfers,
        interactionHints: snapshot.interactionHints,
        projectiles: current.projectiles.filter((projectile) => projectile.expiresAt > currentTime),
      };
      followUpEntities = mergedEntitiesForWorld;
      followUpMapTransfers = mapTransfers;
      worldRef.current = nextWorld;
      return nextWorld;
    });
    const pendingTransferKey = pendingTransferRef.current;
    const selfOnTransfer = followUpEntities.some(
      (entity) =>
        entity.objectId === playerObjectId &&
        pendingTransferKey ===
          transferKeyForWorldTile(followUpMapTransfers, snapshot.mapFileName ?? null, entity.x, entity.y),
    );
    if (pendingTransferKey && selfOnTransfer) {
      transferMap(pendingTransferKey);
    }
    const pendingNpcInteractObjectId = pendingNpcInteractRef.current;
    if (pendingNpcInteractObjectId) {
      const pendingNpc = followUpEntities.find((entity) => entity.objectId === pendingNpcInteractObjectId);
      const nextSelf = followUpEntities.find((entity) => entity.objectId === playerObjectId) ?? null;
      if (!pendingNpc || pendingNpc.dead || pendingNpc.kind !== "npc") {
        pendingNpcInteractRef.current = null;
      } else if (tileDistance(nextSelf, pendingNpc) <= 1) {
        pendingNpcInteractRef.current = null;
        interactTarget(pendingNpcInteractObjectId);
      }
    }
    if (playerObjectId) {
      setScreen("game");
    }
  }

  function reconcileMovementPlanWithServer(
    x: number,
    y: number,
    direction?: string,
    hardCorrection = false,
    selfMovementAdvanced = true,
  ) {
    const plan = movementPlanRef.current;
    if (!plan) {
      return;
    }

    const now = Date.now();
    if (plan.pendingX === undefined || plan.pendingY === undefined) {
      if (x === plan.actionX && y === plan.actionY) {
        if (x === plan.targetX && y === plan.targetY) {
          movementPlanRef.current = null;
          setPredictedPlayerMotion(null);
          return;
        }
        movementPlanRef.current = {
          ...plan,
          actionX: x,
          actionY: y,
          nextStepAt: Math.max(
            plan.nextStepAt,
            selfMovementAdvanced ? now + MOVEMENT_CONFIRM_TICK_DELAY_MS : lastSelfMovementReadyAt({ x, y }, plan.mode),
          ),
          blockedSteps: recentMovementBlockedSteps(plan.blockedSteps, now),
        };
        movementBlockedStepsRef.current = movementPlanRef.current.blockedSteps ?? [];
        return;
      }
      if (hardCorrection) {
        movementPlanRef.current = null;
        applyCrystalInputCorrection(
          x,
          y,
          now,
          direction,
          Math.max(plan.visualUntil ?? 0, directionStepVisualUntilRef.current),
        );
      }
      return;
    }

    if (x === plan.pendingX && y === plan.pendingY) {
      if (x === plan.targetX && y === plan.targetY) {
        movementPlanRef.current = null;
        setPredictedPlayerMotion(null);
        return;
      }
      movementPlanRef.current = {
        ...plan,
        actionX: x,
        actionY: y,
        pendingX: undefined,
        pendingY: undefined,
        pendingSentAt: undefined,
        sentFromX: undefined,
        sentFromY: undefined,
        sentDirection: undefined,
        sentMode: undefined,
        nextStepAt: Math.max(
          plan.nextStepAt,
          (plan.pendingSentAt ?? now) + movementCommandDelayMs(plan.sentMode ?? plan.mode),
        ),
        blockedSteps: recentMovementBlockedSteps(plan.blockedSteps, now),
      };
      movementBlockedStepsRef.current = movementPlanRef.current.blockedSteps ?? [];
      return;
    }

    const sentAt = plan.pendingSentAt ?? plan.nextStepAt;
    const correctedToSentSource = x === plan.sentFromX && y === plan.sentFromY;
    const correctedAwayFromTarget =
      plan.sentFromX !== undefined &&
      plan.sentFromY !== undefined &&
      pointTileDistance({ x, y }, { x: plan.targetX, y: plan.targetY }) >=
        pointTileDistance({ x: plan.sentFromX, y: plan.sentFromY }, { x: plan.targetX, y: plan.targetY });
    if (
      correctedToSentSource &&
      !hardCorrection &&
      !selfMovementAdvanced &&
      now - sentAt < MOVEMENT_ROUTE_REROUTE_AFTER_MS
    ) {
      scheduleMovementConfirmTick();
      return;
    }
    if (hardCorrection) {
      movementBlockedStepsRef.current = movementBlockedStepsAfterCorrection(plan, x, y, now, direction);
      movementPlanRef.current = null;
      applyCrystalInputCorrection(
        x,
        y,
        now,
        plan.sentDirection ?? direction,
        Math.max(plan.visualUntil ?? 0, directionStepVisualUntilRef.current),
      );
      return;
    }
    if (correctedToSentSource && now - sentAt < MOVEMENT_ROUTE_REROUTE_AFTER_MS) {
      retryMovementPlanAfterEarlyServerEcho(plan, now);
      return;
    }
    if (!correctedToSentSource && !correctedAwayFromTarget && now - sentAt < MOVEMENT_ROUTE_REROUTE_AFTER_MS) {
      return;
    }

    const blockedSteps = movementBlockedStepsAfterCorrection(plan, x, y, now, direction);
    movementBlockedStepsRef.current = blockedSteps;
    const blockedAtSource = countMovementBlockedStepsAtSource(blockedSteps, x, y);
    const visualUntil = Math.max(
      plan.visualUntil ?? 0,
      directionStepVisualUntilRef.current,
      now + CRYSTAL_ENTITY_MOVE_ACTION_MS,
    );
    if (
      plan.sentFromX !== undefined &&
      plan.sentFromY !== undefined &&
      pointTileDistance({ x, y }, { x: plan.targetX, y: plan.targetY }) >
        pointTileDistance({ x: plan.sentFromX, y: plan.sentFromY }, { x: plan.targetX, y: plan.targetY })
    ) {
      movementPlanRef.current = null;
      clearPredictedPlayerAfterRouteCorrection(x, y, now, visualUntil, plan.sentDirection ?? direction);
      return;
    }
    if (blockedAtSource >= MOVEMENT_ROUTE_MAX_BLOCKED_STEPS) {
      movementPlanRef.current = null;
      clearPredictedPlayerAfterRouteCorrection(x, y, now, visualUntil, plan.sentDirection ?? direction);
      return;
    }

    movementPlanRef.current = {
      ...plan,
      actionX: x,
      actionY: y,
      pendingX: undefined,
      pendingY: undefined,
      pendingSentAt: undefined,
      sentFromX: undefined,
      sentFromY: undefined,
      sentDirection: undefined,
      sentMode: undefined,
      nextStepAt: Math.max(now + MOVEMENT_ROUTE_RETRY_DELAY_MS, visualUntil),
      blockedSteps,
    };
    clearPredictedPlayerAfterRouteCorrection(x, y, now, visualUntil, plan.sentDirection ?? direction);
  }

  function retryMovementPlanAfterEarlyServerEcho(plan: MovementPlan, now: number) {
    const retrySourceX = plan.sentFromX ?? plan.actionX;
    const retrySourceY = plan.sentFromY ?? plan.actionY;
    if (retrySourceX === undefined || retrySourceY === undefined) {
      scheduleMovementConfirmTick();
      return;
    }

    const visualUntil = Math.max(plan.visualUntil ?? 0, directionStepVisualUntilRef.current);
    movementPlanRef.current = {
      ...plan,
      actionX: retrySourceX,
      actionY: retrySourceY,
      pendingX: undefined,
      pendingY: undefined,
      pendingSentAt: undefined,
      sentFromX: undefined,
      sentFromY: undefined,
      sentDirection: undefined,
      sentMode: undefined,
      nextStepAt: Math.max(now + MOVEMENT_CONFIRM_TICK_DELAY_MS, movementInputBlockedUntilRef.current),
      visualUntil,
      blockedSteps: recentMovementBlockedSteps(plan.blockedSteps, now),
    };
    const predicted = predictedPlayerPositionRef.current;
    if (predicted) {
      setPredictedPlayerMotion(predicted, visualUntil);
    }
    scheduleMovementConfirmTick();
  }

  function recoverStaleMovementPlanFromServer(x: number, y: number, direction: string | undefined, now: number) {
    const plan = movementPlanRef.current;
    if (!plan || plan.pendingX === undefined || plan.pendingY === undefined) {
      return;
    }
    const sentAt = plan.pendingSentAt ?? plan.nextStepAt;
    if (now - sentAt < MOVEMENT_PENDING_ACTION_RECOVERY_MS) {
      return;
    }

    const blockedSteps = recentMovementBlockedSteps(plan.blockedSteps, now);
    const visualUntil = Math.max(plan.visualUntil ?? 0, directionStepVisualUntilRef.current);
    movementBlockedStepsRef.current = blockedSteps;
    if (x === plan.targetX && y === plan.targetY) {
      movementPlanRef.current = null;
      clearPredictedPlayerAfterRouteCorrection(x, y, now, visualUntil, direction);
      return;
    }

    movementPlanRef.current = {
      ...plan,
      actionX: x,
      actionY: y,
      pendingX: undefined,
      pendingY: undefined,
      pendingSentAt: undefined,
      sentFromX: undefined,
      sentFromY: undefined,
      sentDirection: undefined,
      sentMode: undefined,
      nextStepAt: Math.max(now + MOVEMENT_ROUTE_RETRY_DELAY_MS, movementInputBlockedUntilRef.current),
      blockedSteps,
    };
    if (predictedPlayerPositionRef.current?.x !== x || predictedPlayerPositionRef.current?.y !== y) {
      predictedPlayerHoldUntilRef.current = 0;
      setPredictedPlayerMotion({ x, y, direction });
    } else {
      clearPredictedPlayerAfterRouteCorrection(x, y, now, visualUntil, direction);
    }
  }

  function confirmedMovementActionReadyAt(plan: MovementPlan) {
    if (plan.pendingX !== undefined || plan.pendingY !== undefined) {
      return 0;
    }
    const ack = lastSelfMovementAckRef.current;
    if (!ack || plan.actionX === undefined || plan.actionY === undefined) {
      return 0;
    }
    if (ack.x !== plan.actionX || ack.y !== plan.actionY) {
      return 0;
    }
    return ack.at + MOVEMENT_CONFIRM_TICK_DELAY_MS;
  }

  function reconcileDirectionStepWithServer(
    x: number,
    y: number,
    direction?: string,
    hardCorrection = false,
    allowRecovery = true,
  ) {
    reconcileDirectionStepQueueWithServer(x, y, Date.now(), direction, hardCorrection, allowRecovery);
  }

  function reconcileDirectionStepQueueWithServer(
    x: number,
    y: number,
    now: number,
    direction?: string,
    hardCorrection = false,
    allowRecovery = true,
  ) {
    const queue = directionStepPendingQueueRef.current;
    if (queue.length === 0) {
      const predicted = predictedPlayerPositionRef.current;
      if (hardCorrection && predicted && (predicted.x !== x || predicted.y !== y)) {
        applyCrystalInputCorrection(x, y, now, direction);
      }
      return;
    }

    let matchedIndex = -1;
    for (let index = queue.length - 1; index >= 0; index -= 1) {
      const pending = queue[index];
      const directionMatches = !pending.direction || !direction || pending.direction === direction;
      if (directionMatches && directionStepReachedOrPassed(pending, x, y)) {
        matchedIndex = index;
        break;
      }
    }
    if (matchedIndex >= 0) {
      const matchedPending = queue[matchedIndex];
      const nextQueue = queue.slice(matchedIndex + 1);
      setDirectionStepPendingQueue(nextQueue);
      if (nextQueue.length === 0) {
        directionStepNextAtRef.current = Math.max(
          directionStepNextAtRef.current,
          matchedPending.sentAt + movementCommandDelayMs(matchedPending.mode),
        );
        scheduleMovementConfirmTick();
      }
      if (nextQueue.length === 0 && predictedPlayerPositionRef.current?.x === x && predictedPlayerPositionRef.current?.y === y) {
        clearPredictedPlayerAfterDirectionVisual(x, y, now, directionStepVisualUntil(matchedPending));
      }
      return;
    }

    const oldestPending = queue[0];
    const correctedToSentSource =
      oldestPending.sentFromX !== undefined &&
      oldestPending.sentFromY !== undefined &&
      x === oldestPending.sentFromX &&
      y === oldestPending.sentFromY;
    if (!hardCorrection && correctedToSentSource && now - oldestPending.sentAt < MOVEMENT_ROUTE_REROUTE_AFTER_MS) {
      scheduleMovementConfirmTick();
      return;
    }
    if (directionStepPartiallyAdvanced(oldestPending, x, y, direction)) {
      clearSettledSelfActionsAt(x, y, direction);
      clearLocalMovementAnchor();
      setDirectionStepPendingQueue(queue.slice(1));
      directionStepNextAtRef.current = Math.max(
        directionStepNextAtRef.current,
        oldestPending.sentAt + movementCommandDelayMs("walk"),
      );
      scheduleMovementConfirmTick();
      return;
    }

    if (hardCorrection || (allowRecovery && now - oldestPending.sentAt >= MOVEMENT_PENDING_ACTION_RECOVERY_MS)) {
      movementBlockedStepsRef.current = movementBlockedStepsAfterDirectionCorrection(oldestPending, x, y, now);
      applyCrystalInputCorrection(x, y, now, direction);
    } else {
      scheduleMovementConfirmTick();
    }
  }

  function setDirectionStepPendingQueue(queue: DirectionStepPending[]) {
    directionStepPendingQueueRef.current = queue.slice(-MOVEMENT_DIRECTION_PENDING_MAX);
    directionStepPendingRef.current =
      directionStepPendingQueueRef.current[directionStepPendingQueueRef.current.length - 1] ?? null;
  }

  function clearDirectionStepPendingQueue() {
    directionStepPendingQueueRef.current = [];
    directionStepPendingRef.current = null;
  }

  function applyCrystalInputCorrection(
    x: number,
    y: number,
    now: number,
    direction?: string,
    visualUntil = directionStepVisualUntilRef.current,
  ) {
    movementInputBlockedUntilRef.current = now + CRYSTAL_INPUT_CORRECTION_DELAY_MS;
    crystalRunPrimedUntilRef.current = 0;
    queuedMoveIntentRef.current = null;
    pendingSelfMoveRef.current = null;
    nextMoveSendAtRef.current = movementInputBlockedUntilRef.current;
    directionStepNextAtRef.current = movementInputBlockedUntilRef.current;
    directionStepVisualUntilRef.current = Math.min(directionStepVisualUntilRef.current, now);
    queuedDirectionStepRef.current = null;
    clearLocalMovementAnchor();
    clearOutstandingSelfMovementActions();
    clearDirectionStepPendingQueue();
    if (predictedPlayerPositionRef.current && now < visualUntil) {
      setPredictedPlayerMotion({ x, y, direction: direction ?? predictedPlayerPositionRef.current.direction });
      return;
    }
    predictedPlayerHoldUntilRef.current = 0;
    clearLocalSelfPrediction();
  }

  function directionStepVisualUntil(step: DirectionStepPending) {
    return Math.max(directionStepVisualUntilRef.current, step.sentAt + movementStepIntervalMs(step.mode));
  }

  function directionStepPartiallyAdvanced(
    pending: DirectionStepPending,
    x: number,
    y: number,
    direction?: string,
  ) {
    if (
      pending.mode !== "run" ||
      !pending.direction ||
      (direction && direction !== pending.direction) ||
      pending.sentFromX === undefined ||
      pending.sentFromY === undefined
    ) {
      return false;
    }
    if (x === pending.x && y === pending.y) {
      return false;
    }

    const intermediate = pointMoveInDirection(
      { x: pending.sentFromX, y: pending.sentFromY },
      pending.direction,
      1,
    );
    return x === intermediate.x && y === intermediate.y;
  }

  function directionStepReachedOrPassed(pending: DirectionStepPending, x: number, y: number) {
    if (pending.x === x && pending.y === y) {
      return true;
    }
    if (!pending.direction || pending.sentFromX === undefined || pending.sentFromY === undefined) {
      return false;
    }
    return crystalActionReachedOrPassed(
      {
        fromX: pending.sentFromX,
        fromY: pending.sentFromY,
        x: pending.x,
        y: pending.y,
        direction: pending.direction,
        mode: pending.mode,
        sentAt: pending.sentAt,
        visualUntil: pending.sentAt + movementStepIntervalMs(pending.mode),
      },
      x,
      y,
    );
  }

  function movementBlockedStepsAfterDirectionCorrection(
    pending: DirectionStepPending,
    serverX: number,
    serverY: number,
    now: number,
  ) {
    const recentSteps = recentMovementBlockedSteps(movementBlockedStepsRef.current, now);
    if (!pending.direction) {
      return recentSteps;
    }
    if (
      recentSteps.some(
        (step) =>
          step.fromX === serverX &&
          step.fromY === serverY &&
          step.direction === pending.direction &&
          step.mode === pending.mode,
      )
    ) {
      return recentSteps;
    }
    return [
      ...recentSteps,
      {
        fromX: serverX,
        fromY: serverY,
        direction: pending.direction,
        mode: pending.mode,
        at: now,
      },
    ].slice(-MOVEMENT_ROUTE_MAX_BLOCKED_STEPS);
  }

  function recentMovementBlockedSteps(steps: MovementBlockedStep[] | undefined, now: number) {
    return (steps ?? [])
      .filter((step) => now - step.at <= MOVEMENT_ROUTE_BLOCK_MEMORY_MS)
      .slice(-MOVEMENT_ROUTE_MAX_BLOCKED_STEPS);
  }

  function countMovementBlockedStepsAtSource(steps: MovementBlockedStep[], x: number, y: number) {
    return steps.filter((step) => step.fromX === x && step.fromY === y).length;
  }

  function recordSelfNoProgressAck(x: number, y: number, direction: string | undefined, now: number) {
    const previous = lastSelfNoProgressAckRef.current;
    const sameTile =
      previous &&
      previous.x === x &&
      previous.y === y &&
      (!direction || !previous.direction || previous.direction === direction) &&
      now - previous.at <= MOVEMENT_ROUTE_BLOCK_MEMORY_MS;
    const next = {
      x,
      y,
      direction: direction ?? previous?.direction,
      at: now,
      count: sameTile ? previous.count + 1 : 1,
    };
    lastSelfNoProgressAckRef.current = next;
    return next;
  }

  function outstandingSelfMovementDirectionFromSource(x: number, y: number, fallbackDirection?: string) {
    for (let index = crystalSelfActionFeedRef.current.length - 1; index >= 0; index -= 1) {
      const entry = crystalSelfActionFeedRef.current[index];
      if (entry.fromX === x && entry.fromY === y) {
        return entry.direction;
      }
    }
    for (let index = directionStepPendingQueueRef.current.length - 1; index >= 0; index -= 1) {
      const pending = directionStepPendingQueueRef.current[index];
      if (pending.sentFromX === x && pending.sentFromY === y && pending.direction) {
        return pending.direction;
      }
    }
    if (directionStepPendingRef.current?.sentFromX === x && directionStepPendingRef.current.sentFromY === y) {
      return directionStepPendingRef.current.direction ?? fallbackDirection;
    }
    return queuedDirectionStepRef.current?.direction ?? fallbackDirection;
  }

  function rememberBlockedDirectionAtSource(x: number, y: number, direction: string, now: number) {
    const recentSteps = recentMovementBlockedSteps(movementBlockedStepsRef.current, now);
    const nextSteps = [...recentSteps];
    for (const mode of ["walk", "run"] as const) {
      const exists = nextSteps.some(
        (step) => step.fromX === x && step.fromY === y && step.direction === direction && step.mode === mode,
      );
      if (!exists) {
        nextSteps.push({ fromX: x, fromY: y, direction, mode, at: now });
      }
    }
    movementBlockedStepsRef.current = nextSteps.slice(-MOVEMENT_ROUTE_MAX_BLOCKED_STEPS);
    heldDirectionBlockedUntilRef.current = {
      x,
      y,
      direction,
      until: now + MOVEMENT_HELD_BLOCKED_DIRECTION_SUPPRESS_MS,
    };
  }

  function crystalMovementActionTowardWithRouteHints(
    source: { x: number; y: number; direction?: string },
    target: { x: number; y: number },
    requestedMode: "walk" | "run",
    blockedSteps: MovementBlockedStep[],
    currentWorld: WorldState,
  ): { point: { x: number; y: number }; direction: string; mode: "walk" | "run" } {
    const direct = crystalMovementActionToward(source, target, requestedMode);
    if (direct.point.x === source.x && direct.point.y === source.y) {
      return direct;
    }
    if (
      !movementStepBlockedByRecentCorrection(source, direct.direction, direct.mode, blockedSteps) &&
      !movementStepBlockedByVisibleEntity(source, direct.direction, direct.mode, currentWorld) &&
      !movementStepBlockedByVisibleMapCell(source, direct.direction, direct.mode, currentWorld)
    ) {
      return direct;
    }

    const sourceDistance = pointTileDistance(source, target);
    if (direct.mode === "run") {
      const walkPoint = pointMoveInDirection(source, direct.direction, 1);
      if (
        (walkPoint.x !== source.x || walkPoint.y !== source.y) &&
        pointTileDistance(walkPoint, target) <= sourceDistance &&
        !movementStepBlockedByRecentCorrection(source, direct.direction, "walk", blockedSteps) &&
        !movementStepBlockedByVisibleEntity(source, direct.direction, "walk", currentWorld) &&
        !movementStepBlockedByVisibleMapCell(source, direct.direction, "walk", currentWorld)
      ) {
        return { point: walkPoint, direction: direct.direction, mode: "walk" };
      }
    }

    const routed = crystalMovementRouteAroundObstacles(source, target, requestedMode, blockedSteps, currentWorld);
    if (routed) {
      return routed;
    }

    for (const direction of movementRerouteDirections(direct.direction)) {
      const point = pointMoveInDirection(source, direction, direct.mode === "run" ? 2 : 1);
      if (point.x === source.x && point.y === source.y) {
        continue;
      }
      if (pointTileDistance(point, target) > sourceDistance) {
        continue;
      }
      if (movementStepBlockedByRecentCorrection(source, direction, direct.mode, blockedSteps)) {
        continue;
      }
      if (movementStepBlockedByVisibleEntity(source, direction, direct.mode, currentWorld)) {
        continue;
      }
      if (movementStepBlockedByVisibleMapCell(source, direction, direct.mode, currentWorld)) {
        continue;
      }
      return { point, direction, mode: direct.mode };
    }

    return { point: { x: source.x, y: source.y }, direction: direct.direction, mode: direct.mode };
  }

  function crystalMovementRouteAroundObstacles(
    source: { x: number; y: number; direction?: string },
    target: { x: number; y: number },
    requestedMode: "walk" | "run",
    blockedSteps: MovementBlockedStep[],
    currentWorld: WorldState,
  ): { point: { x: number; y: number }; direction: string; mode: "walk" | "run" } | null {
    const route = crystalMovementWalkRoute(source, target, blockedSteps, currentWorld);
    if (!route || route.length < 2) {
      return null;
    }

    const firstStep = route[1];
    const direction = directionFromPoint(source, firstStep, source.direction ?? "Down");
    let point = firstStep;
    let mode: "walk" | "run" = "walk";

    if (requestedMode === "run" && route.length >= 3) {
      const secondStep = route[2];
      const secondDirection = directionFromPoint(firstStep, secondStep, direction);
      const runPoint = pointMoveInDirection(source, direction, 2);
      if (
        secondDirection === direction &&
        runPoint.x === secondStep.x &&
        runPoint.y === secondStep.y &&
        !movementStepBlocked(source, direction, "run", blockedSteps, currentWorld)
      ) {
        point = secondStep;
        mode = "run";
      }
    }

    return { point, direction, mode };
  }

  function crystalMovementWalkRoute(
    source: { x: number; y: number },
    target: { x: number; y: number },
    blockedSteps: MovementBlockedStep[],
    currentWorld: WorldState,
  ): Array<{ x: number; y: number }> | null {
    const regionBounds = currentWorld.originalMapRegion?.regionBounds ?? null;
    const minX = Math.max(
      regionBounds?.minX ?? Number.NEGATIVE_INFINITY,
      Math.min(source.x, target.x) - MOVEMENT_ROUTE_SEARCH_MARGIN,
    );
    const maxX = Math.min(
      regionBounds?.maxX ?? Number.POSITIVE_INFINITY,
      Math.max(source.x, target.x) + MOVEMENT_ROUTE_SEARCH_MARGIN,
    );
    const minY = Math.max(
      regionBounds?.minY ?? Number.NEGATIVE_INFINITY,
      Math.min(source.y, target.y) - MOVEMENT_ROUTE_SEARCH_MARGIN,
    );
    const maxY = Math.min(
      regionBounds?.maxY ?? Number.POSITIVE_INFINITY,
      Math.max(source.y, target.y) + MOVEMENT_ROUTE_SEARCH_MARGIN,
    );
    const insideBounds = (point: { x: number; y: number }) =>
      point.x >= minX && point.x <= maxX && point.y >= minY && point.y <= maxY;
    if (!insideBounds(source)) {
      return null;
    }

    type RouteNode = {
      x: number;
      y: number;
      cost: number;
      score: number;
      from: string | null;
    };
    const startKey = movementRouteKey(source);
    const startNode: RouteNode = {
      x: source.x,
      y: source.y,
      cost: 0,
      score: pointTileDistance(source, target),
      from: null,
    };
    const open = new Map<string, RouteNode>([[startKey, startNode]]);
    const nodes = new Map<string, RouteNode>([[startKey, startNode]]);
    const closed = new Set<string>();
    let bestKey = startKey;
    let searched = 0;

    while (open.size > 0 && searched < MOVEMENT_ROUTE_SEARCH_MAX_NODES) {
      searched += 1;
      const current = movementRouteBestOpenNode(open, target);
      const currentKey = movementRouteKey(current);
      open.delete(currentKey);
      closed.add(currentKey);
      const best = nodes.get(bestKey)!;
      if (
        movementRouteNodeBetterForTarget(current, best, target) &&
        !(current.x === source.x && current.y === source.y)
      ) {
        bestKey = currentKey;
      }
      if (current.x === target.x && current.y === target.y) {
        bestKey = currentKey;
        break;
      }

      for (const direction of movementRouteDirectionsToward(current, target)) {
        if (movementStepBlocked(current, direction, "walk", blockedSteps, currentWorld)) {
          continue;
        }
        const next = pointMoveInDirection(current, direction, 1);
        if (!insideBounds(next)) {
          continue;
        }
        const nextKey = movementRouteKey(next);
        if (closed.has(nextKey)) {
          continue;
        }
        const nextCost = current.cost + 1;
        const existing = nodes.get(nextKey);
        if (existing && existing.cost <= nextCost) {
          continue;
        }
        const nextNode: RouteNode = {
          x: next.x,
          y: next.y,
          cost: nextCost,
          score: nextCost + pointTileDistance(next, target),
          from: currentKey,
        };
        nodes.set(nextKey, nextNode);
        open.set(nextKey, nextNode);
      }
    }

    if (bestKey === startKey) {
      return null;
    }
    const sourceDistance = pointTileDistance(source, target);
    const best = nodes.get(bestKey);
    if (!best || pointTileDistance(best, target) >= sourceDistance) {
      return null;
    }
    return movementReconstructRoute(nodes, bestKey);
  }

  function movementRouteKey(point: { x: number; y: number }) {
    return `${point.x}:${point.y}`;
  }

  function movementRouteBestOpenNode(
    open: Map<string, { x: number; y: number; cost: number; score: number }>,
    target: { x: number; y: number },
  ) {
    return [...open.values()].reduce((best, candidate) =>
      candidate.score < best.score ||
      (candidate.score === best.score && pointTileDistance(candidate, target) < pointTileDistance(best, target))
        ? candidate
        : best,
    );
  }

  function movementRouteNodeBetterForTarget(
    candidate: { x: number; y: number; cost: number },
    currentBest: { x: number; y: number; cost: number },
    target: { x: number; y: number },
  ) {
    const candidateDistance = pointTileDistance(candidate, target);
    const bestDistance = pointTileDistance(currentBest, target);
    return candidateDistance < bestDistance || (candidateDistance === bestDistance && candidate.cost < currentBest.cost);
  }

  function movementRouteDirectionsToward(source: { x: number; y: number }, target: { x: number; y: number }) {
    return [...CRYSTAL_MOVEMENT_DIRECTIONS].sort((left, right) => {
      const leftPoint = pointMoveInDirection(source, left, 1);
      const rightPoint = pointMoveInDirection(source, right, 1);
      const leftDistance = pointTileDistance(leftPoint, target);
      const rightDistance = pointTileDistance(rightPoint, target);
      if (leftDistance !== rightDistance) return leftDistance - rightDistance;
      return CRYSTAL_MOVEMENT_DIRECTIONS.indexOf(left) - CRYSTAL_MOVEMENT_DIRECTIONS.indexOf(right);
    });
  }

  function movementReconstructRoute(
    nodes: Map<string, { x: number; y: number; from: string | null }>,
    endKey: string,
  ) {
    const route: Array<{ x: number; y: number }> = [];
    let currentKey: string | null = endKey;
    while (currentKey) {
      const node = nodes.get(currentKey);
      if (!node) break;
      route.push({ x: node.x, y: node.y });
      currentKey = node.from;
    }
    return route.reverse();
  }

  function crystalMovementActionForDirection(
    source: { x: number; y: number; direction?: string },
    direction: string,
    requestedMode: "walk" | "run",
    blockedSteps: MovementBlockedStep[],
    currentWorld: WorldState,
  ): { point: { x: number; y: number }; direction: string; mode: "walk" | "run" } {
    const directMode = requestedMode;
    const directBlocked =
      movementStepBlockedByRecentCorrection(source, direction, directMode, blockedSteps) ||
      movementStepBlockedByVisibleEntity(source, direction, directMode, currentWorld) ||
      movementStepBlockedByVisibleMapCell(source, direction, directMode, currentWorld);

    if (!directBlocked) {
      return {
        point: pointMoveInDirection(source, direction, directMode === "run" ? 2 : 1),
        direction,
        mode: directMode,
      };
    }

    if (
      directMode === "run" &&
      !movementStepBlockedByRecentCorrection(source, direction, "walk", blockedSteps) &&
      !movementStepBlockedByVisibleEntity(source, direction, "walk", currentWorld) &&
      !movementStepBlockedByVisibleMapCell(source, direction, "walk", currentWorld)
    ) {
      return { point: pointMoveInDirection(source, direction, 1), direction, mode: "walk" };
    }

    return { point: { x: source.x, y: source.y }, direction, mode: "walk" };
  }

  function movementStepBlocked(
    source: { x: number; y: number },
    direction: string,
    mode: "walk" | "run",
    blockedSteps: MovementBlockedStep[],
    currentWorld: WorldState,
  ) {
    return (
      movementStepBlockedByRecentCorrection(source, direction, mode, blockedSteps) ||
      movementStepBlockedByVisibleEntity(source, direction, mode, currentWorld) ||
      movementStepBlockedByVisibleMapCell(source, direction, mode, currentWorld)
    );
  }

  function movementStepBlockedByRecentCorrection(
    source: { x: number; y: number },
    direction: string,
    mode: "walk" | "run",
    blockedSteps: MovementBlockedStep[],
  ) {
    return blockedSteps.some(
      (step) => step.fromX === source.x && step.fromY === source.y && step.direction === direction && step.mode === mode,
    );
  }

  function movementStepBlockedByVisibleEntity(
    source: { x: number; y: number },
    direction: string,
    mode: "walk" | "run",
    currentWorld: WorldState,
  ) {
    const distance = mode === "run" ? 2 : 1;
    for (let step = 1; step <= distance; step += 1) {
      const point = pointMoveInDirection(source, direction, step);
      const occupied = currentWorld.entities.some(
        (entity) =>
          entity.objectId !== currentWorld.playerObjectId &&
          !entity.dead &&
          entity.x === point.x &&
          entity.y === point.y,
      );
      if (occupied) {
        return true;
      }
    }
    return false;
  }

  function movementStepBlockedByVisibleMapCell(
    source: { x: number; y: number },
    direction: string,
    mode: "walk" | "run",
    currentWorld: WorldState,
  ) {
    const distance = mode === "run" ? 2 : 1;
    for (let step = 1; step <= distance; step += 1) {
      const point = pointMoveInDirection(source, direction, step);
      if (originalMapCellBlocksMovement(currentWorld.originalMapRegion, point.x, point.y)) {
        return true;
      }
    }
    return false;
  }

  function movementPredictionShouldWaitForServer(
    source: { x: number; y: number },
    action: { point: { x: number; y: number }; direction: string; mode: "walk" | "run" },
    currentWorld: WorldState,
  ) {
    if (action.point.x === source.x && action.point.y === source.y) {
      return false;
    }
    if (Date.now() < movementPredictionBlockedUntilRef.current) {
      return true;
    }
    if (!currentWorld.originalMapRegion) {
      return true;
    }
    const distance = action.mode === "run" ? 2 : 1;
    for (let step = 1; step <= distance; step += 1) {
      const point = pointMoveInDirection(source, action.direction, step);
      if (
        !originalMapRegionContainsTile(currentWorld.originalMapRegion, point.x, point.y) ||
        originalMapCellBlocksMovement(currentWorld.originalMapRegion, point.x, point.y)
      ) {
        return true;
      }
      const dynamicBlockerOnPath = currentWorld.entities.some(
        (entity) =>
          entity.objectId !== currentWorld.playerObjectId &&
          !entity.dead &&
          entity.x === point.x &&
          entity.y === point.y,
      );
      if (dynamicBlockerOnPath) {
        return true;
      }
    }
    return false;
  }

  function movementRerouteDirections(direction: string) {
    return [-1, 1, -2, 2, -3, 3, 4]
      .map((offset) => rotatedMovementDirection(direction, offset))
      .filter((value): value is string => Boolean(value));
  }

  function rotatedMovementDirection(direction: string, offset: number) {
    const directions = ["Up", "UpRight", "Right", "DownRight", "Down", "DownLeft", "Left", "UpLeft"];
    const index = directions.indexOf(direction);
    if (index < 0) return null;
    return directions[(index + offset + directions.length) % directions.length];
  }

  function movementBlockedStepsAfterCorrection(
    plan: MovementPlan,
    serverX: number,
    serverY: number,
    now: number,
    direction?: string,
  ) {
    const recentSteps = recentMovementBlockedSteps(plan.blockedSteps, now);
    if (
      plan.sentFromX === undefined ||
      plan.sentFromY === undefined ||
      !plan.sentDirection
    ) {
      return recentSteps;
    }

    const correctedToSource = serverX === plan.sentFromX && serverY === plan.sentFromY;
    const correctedShortOfTarget = serverX !== plan.pendingX || serverY !== plan.pendingY;
    if (!correctedToSource && !correctedShortOfTarget) {
      return recentSteps;
    }

    if (
      recentSteps.some(
        (step) =>
          step.fromX === serverX &&
          step.fromY === serverY &&
          step.direction === plan.sentDirection &&
          step.mode === (plan.sentMode ?? plan.mode),
      )
    ) {
      return recentSteps;
    }

    return [
      ...recentSteps,
      {
        fromX: serverX,
        fromY: serverY,
        direction: plan.sentDirection,
        mode: plan.sentMode ?? plan.mode,
        at: now,
      },
    ].slice(-MOVEMENT_ROUTE_MAX_BLOCKED_STEPS);
  }

  function clearPredictedPlayerAfterDirectionVisual(x: number, y: number, now: number, visualUntilOverride?: number) {
    if (movementPlanRef.current) {
      return;
    }
    const predicted = predictedPlayerPositionRef.current;
    if (predicted && (predicted.x !== x || predicted.y !== y)) {
      return;
    }

    const visualUntil = visualUntilOverride ?? directionStepVisualUntilRef.current;
    if (now < visualUntil) {
      return;
    }

    clearLocalSelfPrediction();
  }

  function clearPredictedPlayerAfterRouteCorrection(
    x: number,
    y: number,
    now: number,
    visualUntil: number,
    direction?: string,
  ) {
    const predicted = predictedPlayerPositionRef.current;
    if (!predicted) {
      return;
    }
    if (predicted.x === x && predicted.y === y) {
      clearPredictedPlayerAfterDirectionVisual(x, y, now, visualUntil);
      return;
    }
    const holdUntil = visualUntil + MOVEMENT_PREDICTED_CORRECTION_HOLD_MS;
    if (now < holdUntil && predictedPlayerAheadOfServer({ x, y }, predicted, direction)) {
      predictedPlayerHoldUntilRef.current = Math.max(predictedPlayerHoldUntilRef.current, holdUntil);
      return;
    }
    if (now >= visualUntil) {
      setPredictedPlayerMotion({ x, y, direction: direction ?? predicted.direction });
    }
  }

  function clearVisuallySettledDirectionStepPending(now: number) {
    const pending = directionStepPendingRef.current;
    if (!pending) {
      return;
    }
    if (now < pending.sentAt + movementStepIntervalMs(pending.mode)) {
      return;
    }
    const currentWorld = worldRef.current;
    const serverSelf = currentWorld.entities.find((entity) => entity.objectId === currentWorld.playerObjectId) ?? null;
    const authoritativeSelf = authoritativeSelfForMovementSettlement(serverSelf, now);
    const candidateReached = (candidate: PredictedPlayerMotion | null | undefined) =>
      Boolean(candidate && candidate.x === pending.x && candidate.y === pending.y);
    if (candidateReached(authoritativeSelf)) {
      clearSettledSelfActionsAt(pending.x, pending.y, pending.direction);
      clearDirectionStepPendingQueue();
      clearLocalSelfPrediction();
    }
  }

  function clearSettledPredictedPlayer(now: number) {
    const predicted = predictedPlayerPositionRef.current;
    if (!predicted) {
      return;
    }
    const currentWorld = worldRef.current;
    const serverSelf = currentWorld.entities.find((entity) => entity.objectId === currentWorld.playerObjectId);
    const authoritativeSelf = authoritativeSelfForMovementSettlement(serverSelf ?? null, now);
    if (authoritativeSelf?.x === predicted.x && authoritativeSelf.y === predicted.y) {
      const pendingTurn = pendingSelfTurnRef.current;
      if (
        pendingTurn &&
        now < pendingTurn.visualUntil &&
        !movementTransformMatches(authoritativeSelf, predicted)
      ) {
        return;
      }
      clearSettledSelfActionsAt(predicted.x, predicted.y, predicted.direction ?? authoritativeSelf.direction);
      const plan = movementPlanRef.current;
      if (plan?.pendingX === predicted.x && plan.pendingY === predicted.y) {
        movementPlanRef.current = {
          ...plan,
          actionX: authoritativeSelf.x,
          actionY: authoritativeSelf.y,
          pendingX: undefined,
          pendingY: undefined,
          pendingSentAt: undefined,
          sentFromX: undefined,
          sentFromY: undefined,
          sentDirection: undefined,
          sentMode: undefined,
        };
      }
      if (directionStepPendingRef.current?.x === predicted.x && directionStepPendingRef.current.y === predicted.y) {
        clearDirectionStepPendingQueue();
      }
      clearLocalSelfPrediction();
      return;
    }
    const movementIdle = !hasSelfMovementTransportEvidence(now);
    if (!movementIdle) {
      return;
    } else if (serverSelf) {
      if (serverSelf.x !== predicted.x || serverSelf.y !== predicted.y) {
        predictedPlayerHoldUntilRef.current = 0;
        lastCrystalSelfRenderPositionRef.current = null;
        clearLocalSelfPrediction();
        return;
      }
      if (now < Math.max(directionStepVisualUntilRef.current, predictedPlayerHoldUntilRef.current)) {
        return;
      }
      if (
        now < predictedPlayerHoldUntilRef.current &&
        predictedPlayerAheadOfServer(serverSelf, predicted, serverSelf.direction ?? predicted.direction)
      ) {
        return;
      }
      clearLocalSelfPrediction();
    }
  }

  return (
    !isClientReady ? null :
    <>
    <OriginalClientShell
      language={language}
      screen={screen}
      runtimePhase={runtimePhase}
      runtimeMessage={runtimeMessage}
      wsState={wsState}
      reconnectStatus={reconnectStatus}
      world={world}
      player={self}
      predictedPlayerPosition={null}
      sceneInteractionReady={screen !== "game" || initialSceneAssetsReady}
      bevyEntityRendererReady={bevyEntityRendererReady}
      bevyRuntimeBackend={bevyRuntimeBackend}
      onSceneAssetReadinessChange={handleSceneAssetReadinessChange}
      onBevyEntityRenderStateChange={handleBevyEntityRenderStateChange}
      getLivePlayerRenderPosition={() => {
        const currentWorld = worldRef.current;
        const currentSelf =
          currentWorld.entities.find((entity) => entity.objectId === currentWorld.playerObjectId) ?? null;
        const predicted = preserveCrystalSelfRenderPosition(
          currentSelf,
            chooseCrystalSelfRenderPosition(
              currentSelf,
              renderableSelfPrediction(currentSelf, predictedPlayerPositionRef.current),
          ),
        );
        if (!currentSelf || !predicted) {
          return null;
        }
        const lead = Math.max(Math.abs(predicted.x - currentSelf.x), Math.abs(predicted.y - currentSelf.y));
        return lead <= MOVEMENT_LOCAL_RENDER_MAX_LEAD_TILES ? predicted : null;
      }}
      selectedEntity={selectedEntity}
      sortedEntities={sortedEntities}
      viewportEntities={viewportEntities}
      viewportTiles={viewportTiles}
      logs={logs}
      accountId={accountId}
      password={password}
      chatMessage={chatMessage}
      loginBusy={loginBusy}
      loginError={loginErrorKey ? t(loginErrorKey) : null}
      suiWallets={suiWallets}
      walletPickerOpen={walletPickerOpen}
      dubheWalletUrl={DUBHE_WALLET_URL}
      characters={characters}
      selectedCharacterIndex={selectedCharacterIndex}
      showInventory={showInventory}
      showCharacter={showCharacter}
      activeInventoryTab={activeInventoryTab}
      activeCharacterTab={activeCharacterTab}
      storageServiceOpenVersion={storageServiceOpenVersion}
      onAccountIdChange={setAccountId}
      onPasswordChange={setPassword}
      onLanguageChange={setLanguage}
      onChatMessageChange={setChatMessage}
      onCreateAccount={createAccount}
      onSubmitLogin={submitLogin}
      onPasskeyLogin={submitPasskeyLogin}
      onWalletPickerToggle={toggleWalletPicker}
      onWalletLogin={submitWalletLogin}
      onQuickEnter={quickEnterWorld}
      onResetClient={resetClient}
      onSendChat={(message) => send({ type: "chat", message })}
      onRequestTrade={() => send({ type: "tradeRequest" })}
      onRentExpandedStorage={rentExpandedStorage}
      onLogout={() => send({ type: "logOut" })}
      onCreateCharacter={createCharacter}
      onDeleteCharacter={deleteSelectedCharacter}
      onExitSelect={() => setScreen("login")}
      onUseItem={useItem}
      onDropItem={dropItem}
      onEquipItem={equipItem}
      onRemoveItem={removeItem}
      onMoveItem={moveItem}
      onMergeItem={mergeItem}
      onSplitItem={splitItem}
      onStoreItem={storeItem}
      onTakeBackItem={takeBackItem}
      onUnlockStorage={unlockStorage}
      onSetStoragePassword={setStoragePassword}
      onRemoveStoragePassword={removeStoragePassword}
      onSellItem={sellItem}
      onDropGold={dropGold}
      onRepairItem={repairItem}
      onSpecialRepairItem={specialRepairItem}
      onCastSkill={castSkill}
      onTransferMap={transferMap}
      onClaimMail={claimMail}
      onDeleteMail={deleteMail}
      onBuyGameShopItem={buyGameShopItem}
      onRunStage5Command={runStage5Command}
      onSendClientCommand={sendClientCommand}
      transferOptions={QUICK_TRANSFER_OPTIONS}
      onToggleCharacter={() => setShowCharacter((current) => !current)}
      onToggleInventory={() => setShowInventory((current) => !current)}
      onCloseCharacter={() => setShowCharacter(false)}
      onCloseInventory={() => setShowInventory(false)}
      onOpenCharacterTab={openCharacter}
      onOpenInventoryTab={openInventory}
      onViewportTileClick={(x, y) => handleViewportTileAction(x, y, "walk")}
      onViewportTileSecondaryAction={(x, y) => handleViewportTileAction(x, y, "run")}
      onViewportTileStepClick={(x, y) => handleViewportTileStepAction(x, y, "walk")}
      onViewportTileStepSecondaryAction={(x, y) => handleViewportTileStepAction(x, y, "run")}
      onViewportDirectionStep={handleViewportDirectionStep}
      onViewportDirectionIntent={handleViewportDirectionIntent}
      onViewportDirectionStop={handleViewportDirectionStop}
      onPickGroundDrop={pickGroundDrop}
      onSelectEntity={selectEntity}
      onActivateEntity={activateEntity}
      onApproachTarget={() => {
        if (!selectedEntity) return;
        const destination = approachDestination(self, selectedEntity);
        moveToTile(destination.x, destination.y, "run");
      }}
      onPrimaryTargetAction={() => {
        if (!selectedEntity) return;
        if (selectedEntity.kind === "monster") {
          // A dead monster is a harvestable corpse; live ones are attacked.
          if (selectedEntity.dead) return harvestToward(directionToward(self, selectedEntity));
          return attackTarget(selectedEntity.objectId);
        }
        if (selectedEntity.kind === "npc") return activateEntity(selectedEntity.objectId);
        sendCrystalTurn(directionToward(self, selectedEntity));
      }}
      onSelectNpcDialogTarget={(target) => send({ type: "selectNpcDialog", target })}
      onSubmitNpcInput={(value) => send({ type: "submitNpcInput", value })}
      onSelectCharacter={setSelectedCharacterIndex}
      onEnterWorld={startSelectedCharacter}
      targetDistance={selectedEntity ? tileDistance(self, selectedEntity) : null}
      entityKindClassName={entityKindClassName}
    />
    <ExtraWindows
      t={t}
      questLog={{ open: showQuestLog, onClose: () => setShowQuestLog(false), quests: world.questLog, onTrackQuest: shareQuest, onAbandonQuest: abandonQuest, onShareQuest: shareQuest }}
      heroPet={{ open: showHeroPet, onClose: () => setShowHeroPet(false), hero: adaptHero(world.stage5Systems.hero), creatures: adaptCreatures(world.stage5Systems.intelligentCreatures), onSummonHero: summonHero, onSummonCreature: summonCreature, onReleaseCreature: releaseCreature, onCyclePickupMode: cycleCreaturePickupMode, onSetHeroBehaviour: setHeroBehaviour, onRecallHero: recallHero }}
      guild={{ open: showGuild, onClose: () => setShowGuild(false), guild: world.stage5Systems?.guild ?? null, playerName: self?.name ?? null, onEditNotice: editGuildNotice, onInviteMember: inviteGuildMember, onKickMember: kickGuildMember, onSendGuildChat: sendGuildChat, onChangeMemberRank: changeGuildMemberRank, onSaveRank: saveGuildRank, onDepositGold: guildDepositGold, onWithdrawGold: guildWithdrawGold }}
      group={{ open: showGroup, onClose: () => setShowGroup(false), group: adaptGroup(world.stage5Systems.group), playerName: self?.name ?? null, onInviteMember: groupInviteMember, onKickMember: kickGroupMember, onLeaveGroup: groupLeave, onToggleLootMode: groupToggleLootMode, onToggleAllowInvites: groupToggleAllowInvites }}
      friends={{ open: showFriends, onClose: () => setShowFriends(false), social: adaptFriends(world.stage5Systems.social), onAddFriend: addFriend, onBlockPlayer: blockPlayer, onRemoveFriend: removeFriendEntry, onUnblockPlayer: removeFriendEntry, onWhisper: whisperPlayer, onMail: openMailWindow, onEditMemo: editFriendMemo }}
      bonds={{ open: showBonds, onClose: () => setShowBonds(false), relationship: adaptRelationship(world.stage5Systems.relationship), mentor: adaptMentor(world.stage5Systems.mentor), onProposeMarriage: proposeMarriage, onDivorce: divorce, onAllowMarriage: toggleAllowMarriage, onAddMentor: addMentor, onAllowMentor: allowMentor, onCancelMentor: cancelMentor }}
      ranking={{ open: showRanking, onClose: () => setShowRanking(false), activeTab: adaptActiveRankingPage(world.rankings, world.rankingCurrentKey).tab, page: adaptActiveRankingPage(world.rankings, world.rankingCurrentKey).page, playerName: self?.name ?? null, onSelectTab: requestRanking, onRefresh: requestRanking, onToggleOnlineOnly: setRankingOnlineOnly }}
      market={{ open: showMarket, onClose: () => setShowMarket(false), listings: adaptMarketListings(world.stage5Systems.auction), gold: world.gold, cityCurrencies: world.cityCurrencies, onBuy: marketBuyListing, onCancel: marketCancelListing, onSearch: marketSearch, onRefresh: marketRefresh, onCollect: marketCancelListing }}
      conquest={{ open: showConquest, onClose: () => setShowConquest(false), conquest: adaptConquest(world.stage5Systems.conquest), territory: adaptGuildTerritory(world.stage5Systems.guildTerritory), guildName: world.stage5Systems?.guild?.name ?? null, onStartWar: conquestStartWar }}
      trade={{ open: showTrade, onClose: () => setShowTrade(false), trade: adaptTrade(world.stage5Systems.trade), myGold: world.gold, onAccept: acceptTrade, onConfirm: confirmTrade, onCancel: cancelTrade, onSetGold: setTradeGold }}
      buffs={{ open: showBuffs, onClose: () => setShowBuffs(false), buffs: adaptBuffs(world.activeBuffs) }}
      mail={{ open: showMail, onClose: () => setShowMail(false), mail: adaptMailMessages(world.stage5Systems.mail), gold: world.gold, onOpen: openMailMessage, onClaimAttachment: claimMailAttachment, onDeleteMail: deleteMailMessage, onSendMail: sendMailMessage }}
      worldMap={{ open: showWorldMap, onClose: () => setShowWorldMap(false), currentMap: world.mapTitle, markers: adaptWorldMapMarkers(world.mapTransfers) }}
      help={{ open: showHelp, onClose: () => setShowHelp(false) }}
      hotkeys={{ open: showHotkeys, onClose: () => setShowHotkeys(false) }}
      chatSettings={{ open: showChatSettings, onClose: () => setShowChatSettings(false) }}
    />
    {debugSnapshotNotice ? (
      <div className={`debug-snapshot-toast ${debugSnapshotNotice.status}`} role="status" aria-live="polite">
        <span>{debugSnapshotNotice.message}</span>
        {debugSnapshotNotice.sessionId ? <code>{debugSnapshotNotice.sessionId}</code> : null}
      </div>
    ) : null}
    {screen === "game" && showTutorial ? (
      <OriginalClientTutorialOverlay
        language={language}
        windows={{
          inventory: showInventory,
          character: showCharacter,
          // The quest log is reachable two ways: the standalone Alt+Q window
          // (showQuestLog) and the HUD quest button, which opens the inventory
          // on its "quest" tab. Treat either as "quests viewed".
          questLog: showQuestLog || (showInventory && activeInventoryTab === "quest"),
        }}
        onClose={() => setShowTutorial(false)}
      />
    ) : null}
    </>
  );
}

// Maps the normalized mail records (extended-server-packets normalizeMailList)
// to the Mail window's MailMessageSummary shape. The list packet only carries a
// item *count*, so attachments are surfaced as placeholder slots for the parcel
// badge; full item detail arrives when the message is opened (ReadMail).
function adaptMailMessages(raw: Array<Record<string, unknown>> | undefined): MailMessageSummary[] {
  if (!Array.isArray(raw)) return [];
  return raw.map((entry) => {
    const itemCount = Number(entry.itemCount ?? 0);
    return {
      id: Number(entry.mailId ?? 0),
      from: typeof entry.senderName === "string" ? entry.senderName : "",
      body: typeof entry.message === "string" ? entry.message : "",
      gold: Number(entry.gold ?? 0),
      read: Boolean(entry.opened),
      locked: Boolean(entry.locked),
      claimed: Boolean(entry.collected),
      items: itemCount > 0 ? Array.from({ length: itemCount }, (_, index) => `#${index + 1}`) : undefined,
    };
  });
}

// B-wave-2: parse the enriched quest objectives/rewards the gateway now emits.
// The backend uses `text` for an objective; the window wants `label`.
function questNumber(value: unknown): number | undefined {
  const parsed = typeof value === "number" ? value : typeof value === "string" ? Number(value) : NaN;
  return Number.isFinite(parsed) ? parsed : undefined;
}

function parseQuestObjectives(
  raw: unknown,
): Array<{ label: string; current?: number; required?: number }> | undefined {
  if (!Array.isArray(raw)) return undefined;
  const list = raw.flatMap((entry) => {
    const record = (entry ?? {}) as Record<string, unknown>;
    const label =
      typeof record.text === "string" ? record.text : typeof record.label === "string" ? record.label : "";
    if (!label) return [];
    return [{ label, current: questNumber(record.current), required: questNumber(record.required) }];
  });
  return list.length > 0 ? list : undefined;
}

function parseQuestRewards(
  raw: unknown,
): { gold?: number; experience?: number; credit?: number; items?: Array<{ name: string; count?: number }> } | undefined {
  if (!raw || typeof raw !== "object" || Array.isArray(raw)) return undefined;
  const record = raw as Record<string, unknown>;
  const items = Array.isArray(record.items)
    ? (record.items as unknown[]).flatMap((entry) => {
        const itemRecord = (entry ?? {}) as Record<string, unknown>;
        const name = typeof itemRecord.name === "string" ? itemRecord.name : "";
        return name ? [{ name, count: questNumber(itemRecord.count) }] : [];
      })
    : undefined;
  const gold = questNumber(record.gold);
  const experience = questNumber(record.experience);
  const credit = questNumber(record.credit);
  if (gold === undefined && experience === undefined && credit === undefined && !items) return undefined;
  return { gold, experience, credit, items };
}

function upsertEntityInList(list: WorldEntity[], nextEntity: WorldEntity) {
  return list.some((entity) => entity.objectId === nextEntity.objectId)
    ? list.map((entity) => (entity.objectId === nextEntity.objectId ? { ...entity, ...nextEntity } : entity))
    : [...list, nextEntity];
}

function patchEntityInList(
  list: WorldEntity[],
  objectId: string,
  updater: (entity: WorldEntity) => WorldEntity,
) {
  return list.map((entity) => (entity.objectId === objectId ? updater(entity) : entity));
}

function withCrystalSelfPacketMovement(
  nextEntity: WorldEntity,
  previousEntity: WorldEntity,
  packet: string,
  disposition: CrystalSelfAckDisposition,
  now: number,
): WorldEntity {
  if (packet === "UserLocation") {
    if (disposition === "confirmed" || disposition === "staleEcho") {
      return {
        ...nextEntity,
        ...preservedMovementAnimation(previousEntity, nextEntity.x, nextEntity.y, now),
      };
    }

    return {
      ...nextEntity,
      movementAnimation: undefined,
      movementStartedAt: undefined,
      movementUntil: undefined,
    };
  }

  return withPacketMovementAnimation(nextEntity, previousEntity, packet, now);
}

function withPacketMovementAnimation(
  nextEntity: WorldEntity,
  previousEntity: WorldEntity,
  packet: string,
  now: number,
): WorldEntity {
  const animation = packetMovementAnimation(packet, previousEntity, nextEntity);
  if (!animation) {
    return {
      ...nextEntity,
      movementAnimation: undefined,
      movementStartedAt: undefined,
      movementUntil: undefined,
    };
  }

  return {
    ...nextEntity,
    movementAnimation: animation,
    movementStartedAt: now,
    movementUntil: now + CRYSTAL_ENTITY_MOVE_ACTION_MS,
  };
}

function packetMovementAnimation(
  packet: string,
  previousEntity: WorldEntity,
  nextEntity: WorldEntity,
): "walking" | "running" | null {
  switch (packet) {
    case "ObjectRun":
    case "ObjectDash":
    case "UserDash":
    case "ObjectDashAttack":
    case "UserDashAttack":
    case "UserAttackMove":
      return "running";
    case "ObjectWalk":
    case "ObjectBackStep":
    case "ObjectPushed":
    case "Pushed":
      return "walking";
    case "ObjectDashFail":
    case "UserDashFail":
      return null;
    case "UserLocation": {
      const distance = Math.max(
        Math.abs(nextEntity.x - previousEntity.x),
        Math.abs(nextEntity.y - previousEntity.y),
      );
      if (distance <= 0) return null;
      return distance > 1 ? "running" : "walking";
    }
    default:
      return null;
  }
}

function preservedMovementAnimation(
  previousEntity: WorldEntity | undefined,
  x: number,
  y: number,
  now: number,
): Pick<WorldEntity, "movementAnimation" | "movementStartedAt" | "movementUntil"> {
  if (
    previousEntity?.movementAnimation &&
    previousEntity.movementStartedAt !== undefined &&
    previousEntity.movementUntil !== undefined &&
    previousEntity.movementUntil > now &&
    previousEntity.x === x &&
    previousEntity.y === y
  ) {
    return {
      movementAnimation: previousEntity.movementAnimation,
      movementStartedAt: previousEntity.movementStartedAt,
      movementUntil: previousEntity.movementUntil,
    };
  }

  return {};
}

function upsertGroundDropInList(list: GroundDrop[], nextDrop: GroundDrop) {
  return list.some((drop) => drop.objectId === nextDrop.objectId)
    ? list.map((drop) => (drop.objectId === nextDrop.objectId ? { ...drop, ...nextDrop } : drop))
    : [...list, nextDrop];
}

/**
 * Resolve once the Bevy canvas (`#mir2-web3-canvas`) is ready to attach to — either it's
 * already in the DOM, the shell has signalled readiness on mount (`window.__mir2BevyCanvasReady`
 * / the `"mir2:bevy-canvas-ready"` event it dispatches), or a MutationObserver sees it inserted.
 * Rejects after `timeoutMs`.
 *
 * The runtime attaches to an existing `#mir2-web3-canvas` (apps/game-client/runtime/src/lib.rs),
 * rendered inside the lazily-mounted (dynamic, ssr:false) OriginalClientShell — booting the WASM
 * before it exists makes bevy_winit panic "Cannot find element: #mir2-web3-canvas".
 */
async function waitForBevyCanvas(timeoutMs = 15000): Promise<void> {
  if (typeof document === "undefined") return;
  const w = window as Window & { __mir2BevyCanvasReady?: boolean };
  const isReady = () =>
    w.__mir2BevyCanvasReady === true || document.querySelector("#mir2-web3-canvas") !== null;
  if (isReady()) return;
  await new Promise<void>((resolve, reject) => {
    let settled = false;
    const finish = (ok: boolean) => {
      if (settled) return;
      settled = true;
      window.clearTimeout(timer);
      observer.disconnect();
      window.removeEventListener("mir2:bevy-canvas-ready", onSignal);
      if (ok) resolve();
      else reject(new Error("timed out waiting for #mir2-web3-canvas"));
    };
    const onSignal = () => {
      if (isReady()) finish(true);
    };
    const observer = new MutationObserver(onSignal);
    const timer = window.setTimeout(() => finish(false), timeoutMs);
    window.addEventListener("mir2:bevy-canvas-ready", onSignal);
    observer.observe(document.documentElement, { childList: true, subtree: true });
    // Re-check in case it mounted between the initial check and listener/observer setup.
    if (isReady()) finish(true);
  });
}

async function loadBevyRuntimeModule(backend: BevyRuntimeBackend): Promise<RuntimeModule> {
  const runtimeVersionQuery = encodeURIComponent(BEVY_RUNTIME_VERSION);
  const runtimePackageDir = bevyRuntimePackageDir(backend);
  const runtimePath = `/bevy-runtime/${runtimePackageDir}/mir2_bevy_runtime.js?v=${runtimeVersionQuery}`;
  const runtimeWasmPath = `/bevy-runtime/${runtimePackageDir}/mir2_bevy_runtime_bg.wasm?v=${runtimeVersionQuery}`;
  const runtime = (await import(
    /* webpackIgnore: true */ runtimePath
  )) as RuntimeModule;

  if (typeof runtime.default === "function") {
    // The runtime attaches to #mir2-web3-canvas on boot; make sure it exists first
    // (it lives in the lazily-mounted OriginalClientShell) to avoid a bevy_winit panic.
    await waitForBevyCanvas();
    await runtime.default({ module_or_path: runtimeWasmPath });
  }

  return runtime;
}

function bevyRuntimePackageDir(backend: BevyRuntimeBackend) {
  return backend === "webgpu" ? "pkg-webgpu" : "pkg-webgl2";
}

function selectBevyRuntimeBackend(
  params: URLSearchParams,
  support: BevyRuntimeSupport,
): BevyRuntimeBackend | null {
  const requestedBackend = normalizeBevyRuntimeBackendOverride(
    params.get("bevyBackend") ?? safeLocalStorageGet("mir2-bevy-backend"),
  );

  if (requestedBackend === "webgl2") {
    return support.webgl2 ? "webgl2" : null;
  }

  if (requestedBackend === "webgpu") {
    return support.webgpu ? "webgpu" : support.webgl2 ? "webgl2" : null;
  }

  if (support.webgpu) return "webgpu";
  if (support.webgl2) return "webgl2";
  return null;
}

function normalizeBevyRuntimeBackendOverride(value: string | null): BevyRuntimeBackend | null {
  const normalized = value?.trim().toLowerCase();
  if (normalized === "webgpu" || normalized === "gpu") return "webgpu";
  if (normalized === "webgl2" || normalized === "webgl" || normalized === "gl") return "webgl2";
  return null;
}

function detectBevyRuntimeSupport(): BevyRuntimeSupport {
  return {
    webgpu: hasWebGpuSupport(),
    webgl2: hasWebGl2Support(),
  };
}

function hasWebGpuSupport() {
  try {
    const maybeNavigator = navigator as Navigator & { gpu?: unknown };
    return Boolean(window.isSecureContext && maybeNavigator.gpu);
  } catch {
    return false;
  }
}

function hasWebGl2Support() {
  try {
    const canvas = document.createElement("canvas");
    return Boolean(canvas.getContext("webgl2"));
  } catch {
    return false;
  }
}

function safeLocalStorageGet(key: string) {
  try {
    return window.localStorage.getItem(key);
  } catch {
    return null;
  }
}

function createLogLine(
  text: string,
  tone: UiLogTone,
  channel: UiLogChannel,
  locale: string,
): UiLogLine {
  return {
    text: `[${new Date().toLocaleTimeString(locale)}] ${text}`,
    tone,
    channel,
  };
}

function trimLogTimestamp(text: string) {
  return text.replace(/^\[\d{1,2}:\d{2}:\d{2}(?:\s?[AP]M)?\]\s*/i, "");
}

function defaultLogChannel(tone: UiLogTone): UiLogChannel {
  if (tone === "network") {
    return "network";
  }

  return tone === "chat" ? "normal" : "system";
}

function gatewayChatChannel(value: unknown): UiLogChannel {
  if (typeof value !== "string") {
    return "normal";
  }

  switch (value.toLowerCase()) {
    case "shout":
    case "shout2":
    case "shout3":
      return "shout";
    case "trade":
      return "trade";
    case "whisperin":
    case "whisperout":
      return "whisper";
    case "group":
      return "group";
    case "guild":
      return "guild";
    case "mentor":
      return "mentor";
    case "relationship":
      return "relationship";
    case "system":
    case "system2":
      return "system";
    case "hint":
      return "hint";
    case "server":
      return "server";
    case "announcement":
    case "levelup":
    case "linemessage":
      return "announcement";
    default:
      return "normal";
  }
}

function gatewayChatTone(value: unknown): UiLogTone {
  const channel = gatewayChatChannel(value);
  return channel === "system" || channel === "hint" || channel === "announcement" ? "system" : "chat";
}

function storageUnlockResultMessage(result: number, hasPassword: boolean) {
  switch (result) {
    case 0:
      return "Storage unlocked.";
    case 1:
      return "Storage password is required.";
    case 2:
      return "Storage password is incorrect.";
    case 3:
      return "Storage is not available.";
    case 4:
      return hasPassword ? "Storage unlock state is invalid." : "Storage password has not been set.";
    default:
      return `Storage unlock returned result ${result}.`;
  }
}

function storagePasswordResultMessage(result: number, removing: boolean, hasPassword: boolean) {
  switch (result) {
    case 0:
      return "Storage password service is not available.";
    case 1:
      return removing ? "Current storage password is required to remove it." : "Current storage password is required.";
    case 2:
      return "Current storage password is incorrect.";
    case 3:
      return "New storage password is invalid.";
    case 4:
      return removing
        ? "Storage password removed."
        : hasPassword
          ? "Storage password updated."
          : "Storage password cleared.";
    case 5:
      return "Storage password has not been set.";
    default:
      return `Storage password operation returned result ${result}.`;
  }
}

function storageResizeMessage(size: number, hasExpandedStorage: boolean) {
  if (size <= 0) {
    return "Storage layout updated.";
  }
  return hasExpandedStorage
    ? `Storage expanded to ${size} slots.`
    : `Storage layout updated: ${size} slots.`;
}

function equipmentSlotIndex(slot: EquipmentSlot): number {
  switch (slot) {
    case "weapon":
      return 0;
    case "armour":
      return 1;
    case "helmet":
      return 2;
    case "torch":
      return 3;
    case "necklace":
      return 4;
    case "braceletLeft":
      return 5;
    case "braceletRight":
      return 6;
    case "ringLeft":
      return 7;
    case "ringRight":
      return 8;
    case "amulet":
      return 9;
    case "belt":
      return 10;
    case "boots":
      return 11;
    case "stone":
      return 12;
    case "mount":
      return 13;
  }
}

function equipmentSlotFromIndex(index: number): EquipmentSlot | null {
  switch (index) {
    case 0:
      return "weapon";
    case 1:
      return "armour";
    case 2:
      return "helmet";
    case 3:
      return "torch";
    case 4:
      return "necklace";
    case 5:
      return "braceletLeft";
    case 6:
      return "braceletRight";
    case 7:
      return "ringLeft";
    case 8:
      return "ringRight";
    case 9:
      return "amulet";
    case 10:
      return "belt";
    case 11:
      return "boots";
    case 12:
      return "stone";
    case 13:
      return "mount";
    default:
      return null;
  }
}

function summarizeDebugWorldSnapshot(snapshot: GatewayWorldSnapshot) {
  return {
    tick: snapshot.tick,
    gold: snapshot.gold,
    playerHp: snapshot.playerHp,
    playerMaxHp: snapshot.playerMaxHp,
    inSafeZone: snapshot.inSafeZone,
    inventoryItems: (snapshot.inventoryItems ?? []).map((item) => ({
      key: item.key,
      uniqueId: item.uniqueId,
      slot: item.slot,
      container: item.container,
      quantity: item.quantity,
    })),
    beltItems: (snapshot.beltItems ?? []).map((item) => ({
      key: item.key,
      uniqueId: item.uniqueId,
      slot: item.slot,
      quantity: item.quantity,
    })),
    storageItems: (snapshot.storageItems ?? []).map((item) => ({
      key: item.key,
      uniqueId: item.uniqueId,
      slot: item.slot,
      quantity: item.quantity,
    })),
    groundDrops: (snapshot.groundDrops ?? []).map((drop) => ({
      name: drop.name,
      objectId: drop.objectId,
      x: drop.x,
      y: drop.y,
      quantity: drop.quantity,
    })),
    npcScriptDiagnostics: (snapshot.npcScriptDiagnostics ?? []).map((diagnostic) => ({
      scriptKey: diagnostic.scriptKey,
      label: diagnostic.label,
      lineNumber: diagnostic.lineNumber,
      command: diagnostic.command,
      message: diagnostic.message,
    })),
  };
}

function itemClientReference(item: WorldItem) {
  if (typeof item.uniqueId === "number") {
    return item.uniqueId;
  }
  switch (item.container) {
    case "bag2":
      return 40 + item.slot;
    default:
      return item.slot;
  }
}

function itemMatchesPacketGrid(item: WorldItem, grid: string, uniqueId: number) {
  const normalizedGrid = grid.toLowerCase();
  if (normalizedGrid === "belt") {
    return item.container === "belt" && itemClientReference(item) === uniqueId;
  }
  if (normalizedGrid === "questinventory" || normalizedGrid === "quest_inventory") {
    return item.container === "quest" && itemClientReference(item) === uniqueId;
  }
  if (normalizedGrid === "storage") {
    return item.container === "storage" && itemClientReference(item) === uniqueId;
  }
  return (item.container === "bag1" || item.container === "bag2") && itemClientReference(item) === uniqueId;
}

function consumePacketItem(items: WorldItem[], grid: string, uniqueId: number, count: number) {
  let consumed = false;
  const nextItems = items.flatMap((item) => {
    if (consumed || !itemMatchesPacketGrid(item, grid, uniqueId)) {
      return [item];
    }
    consumed = true;
    if (item.quantity > count) {
      return [{ ...item, quantity: item.quantity - count }];
    }
    return [];
  });
  return consumed ? nextItems : items;
}

function stringifyId(value: unknown) {
  return typeof value === "number" ? String(value) : typeof value === "string" ? value : "0";
}

function numberOrZero(value: unknown) {
  return typeof value === "number" ? value : 0;
}

function numberOrUndefined(value: unknown) {
  return typeof value === "number" ? value : undefined;
}

function movementPointFromPacketPayload(payload: Record<string, unknown>) {
  const location = payload.location as { x?: unknown; y?: unknown } | undefined;
  return {
    x: numberOrZero(payload.x ?? location?.x),
    y: numberOrZero(payload.y ?? location?.y),
  };
}

function spriteFromPacket(payload: Record<string, unknown>, kind: EntityKind): GatewayWorldEntitySprite | null {
  if (kind === "player" || kind === "selfPlayer") {
    return playerSpriteFromPacket(payload);
  }

  if (kind === "monster") {
    return simpleSpriteFromPacket("Monster", numberOrUndefined(payload.image), 3);
  }

  if (kind === "npc") {
    return simpleSpriteFromPacket("NPC", numberOrUndefined(payload.image), 2);
  }

  return null;
}

function spriteFromSnapshotEntity(
  entity: GatewayWorldEntity,
  mapFileName: string | null | undefined,
  existingSprite: GatewayWorldEntitySprite | null,
): GatewayWorldEntitySprite | null {
  if (entity.sprite) {
    return entity.sprite;
  }
  if (existingSprite) {
    return existingSprite;
  }
  return fallbackCrystalActorSprite(
    entity.kind,
    entity.name,
    entity.objectId,
    entity.x,
    entity.y,
    mapFileName,
  );
}

function spriteFromPacketOrExisting(
  payload: Record<string, unknown>,
  kind: EntityKind,
  existingSprite: GatewayWorldEntitySprite | null,
  mapFileName: string | null | undefined,
): GatewayWorldEntitySprite | null {
  const sprite = spriteFromPacket(payload, kind);
  const location = payload.location as { x?: number; y?: number } | undefined;
  const packetImage = numberOrUndefined(payload.image);
  const fallbackSprite =
    kind === "npc" || kind === "monster"
      ? fallbackCrystalActorSprite(
          kind,
          stringOrFallback(payload.name, ""),
          numberOrUndefined(payload.objectId),
          numberOrUndefined(location?.x),
          numberOrUndefined(location?.y),
          mapFileName,
        )
      : null;

  if (!existingSprite || (kind !== "npc" && kind !== "monster")) {
    if ((kind === "npc" || kind === "monster") && packetImage === 0 && fallbackSprite) {
      return fallbackSprite;
    }
    return sprite ?? fallbackSprite;
  }
  if (!sprite) {
    return existingSprite;
  }

  if (packetImage === 0 && sprite.bodyLibrary !== existingSprite.bodyLibrary) {
    return existingSprite;
  }

  return sprite;
}

function fallbackCrystalActorSprite(
  kind: EntityKind,
  name: string,
  objectId: number | undefined,
  x: number | undefined,
  y: number | undefined,
  mapFileName: string | null | undefined,
): GatewayWorldEntitySprite | null {
  if (kind === "npc") {
    const byLocation =
      mapFileName && typeof x === "number" && typeof y === "number"
        ? CRYSTAL_NPC_SPRITE_BY_LOCATION.get(crystalActorLocationKey(mapFileName, name, x, y))
        : undefined;
    const byObjectId =
      typeof objectId === "number" ? CRYSTAL_NPC_SPRITE_BY_OBJECT_ID.get(objectId) : undefined;
    const npc = byLocation ?? (byObjectId?.map === mapFileName ? byObjectId : undefined) ?? byObjectId;
    return npc ? simpleSpriteFromPacket("NPC", npc.image, 2) : null;
  }

  if (kind === "monster") {
    const monster = CRYSTAL_MONSTER_SPRITE_BY_NAME.get(normalizeCrystalActorName(name));
    return monster ? simpleSpriteFromPacket("Monster", monster.image, 3) : null;
  }

  return null;
}

function crystalActorLocationKey(mapFileName: string, name: string, x: number, y: number) {
  return `${mapFileName}:${normalizeCrystalActorName(name)}:${Math.trunc(x)}:${Math.trunc(y)}`;
}

function normalizeCrystalActorName(value: string) {
  return value.trim().toLowerCase().replace(/[\s_]+/g, "_");
}

function playerSpriteFromPacket(payload: Record<string, unknown>): GatewayWorldEntitySprite {
  const armourShape = numberOrUndefined(payload.armour) ?? 0;
  const hairShape = numberOrUndefined(payload.hair) ?? 0;
  const weaponShape = numberOrUndefined(payload.weapon);
  const classKey = mapClassKey(payload.class);
  const genderKey = mapGenderKey(payload.gender);
  const usesAssassinWeapon = typeof weaponShape === "number" && weaponShape >= 100 && weaponShape < 200;
  const usesArcherWeapon = typeof weaponShape === "number" && weaponShape >= 200;
  const bodyLibrary = `CArmour/${paddedLibraryIndex(armourShape, 2)}`;
  const hairLibrary = `CHair/${paddedLibraryIndex(hairShape, 2)}`;
  const weaponLibraries = playerWeaponLibraries(weaponShape);
  const frameBaseOffset = genderKey === "female" ? 808 : 0;
  const altFrameBaseOffset =
    classKey === "archer" && usesArcherWeapon
      ? genderKey === "female"
        ? 352
        : 0
      : classKey === "assassin" && usesAssassinWeapon
        ? genderKey === "female"
          ? 512
          : 0
        : null;

  return {
    bodyLibrary,
    hairLibrary,
    weaponLibrary: weaponLibraries.weaponLibrary,
    weaponLibrarySecondary: weaponLibraries.weaponLibrarySecondary,
    frameBaseOffset,
    weaponFrameOffset: weaponLibraries.weaponLibrary ? (genderKey === "female" ? 416 : 0) : null,
    altBodyLibrary:
      classKey === "archer" && usesArcherWeapon
        ? `ARArmour/${paddedLibraryIndex(armourShape, 2)}`
        : classKey === "assassin" && usesAssassinWeapon
          ? `AArmour/${paddedLibraryIndex(armourShape, 2)}`
          : null,
    altHairLibrary:
      classKey === "archer" && usesArcherWeapon
        ? `ARHair/${paddedLibraryIndex(hairShape, 2)}`
        : classKey === "assassin" && usesAssassinWeapon
          ? `AHair/${paddedLibraryIndex(hairShape, 2)}`
          : null,
    altWeaponLibrary: weaponLibraries.altWeaponLibrary,
    altWeaponLibrarySecondary: weaponLibraries.altWeaponLibrarySecondary,
    altFrameBaseOffset,
    altWeaponFrameOffset: altFrameBaseOffset,
    frameCount: 4,
    directionStride: 4,
  };
}

function playerWeaponLibraries(weaponShape: number | undefined) {
  if (typeof weaponShape !== "number") {
    return {
      weaponLibrary: null,
      weaponLibrarySecondary: null,
      altWeaponLibrary: null,
      altWeaponLibrarySecondary: null,
    };
  }

  if (weaponShape >= 100 && weaponShape < 200) {
    const index = paddedLibraryIndex(weaponShape - 100, 2);
    return {
      weaponLibrary: null,
      weaponLibrarySecondary: null,
      altWeaponLibrary: `AWeapon/${index} R`,
      altWeaponLibrarySecondary: `AWeapon/${index} L`,
    };
  }

  if (weaponShape >= 200) {
    const index = paddedLibraryIndex(weaponShape - 200, 2);
    return {
      weaponLibrary: `ARWeapon/${index}`,
      weaponLibrarySecondary: null,
      altWeaponLibrary: `ARWeapon/${index} S`,
      altWeaponLibrarySecondary: null,
    };
  }

  return {
    weaponLibrary: `CWeapon/${paddedLibraryIndex(weaponShape, 2)}`,
    weaponLibrarySecondary: null,
    altWeaponLibrary: null,
    altWeaponLibrarySecondary: null,
  };
}

function simpleSpriteFromPacket(
  libraryRoot: "Monster" | "NPC",
  image: number | undefined,
  padding: number,
): GatewayWorldEntitySprite | null {
  if (typeof image !== "number") {
    return null;
  }

  return {
    bodyLibrary: `${libraryRoot}/${paddedLibraryIndex(image, padding)}`,
    hairLibrary: null,
    weaponLibrary: null,
    weaponLibrarySecondary: null,
    frameBaseOffset: 0,
    weaponFrameOffset: null,
    altBodyLibrary: null,
    altHairLibrary: null,
    altWeaponLibrary: null,
    altWeaponLibrarySecondary: null,
    altFrameBaseOffset: null,
    altWeaponFrameOffset: null,
    frameCount: 4,
    directionStride: 4,
  };
}

function paddedLibraryIndex(value: number, width: number) {
  return Math.max(0, Math.trunc(value)).toString().padStart(width, "0");
}

function transferKeyForWorldTile(
  transfers: MapTransferArea[],
  mapFileName: string | null,
  x: number,
  y: number,
) {
  const normalizedMap = mapFileName?.trim().toLowerCase().replace(/\.map$/, "") ?? null;
  return (
    transfers.find(
      (transfer) =>
        normalizedMap === transfer.mapFileName &&
        x >= transfer.minX &&
        x <= transfer.maxX &&
        y >= transfer.minY &&
        y <= transfer.maxY,
    )?.key ?? null
  );
}

function normalizeMapFileName(mapFileName: string | null) {
  return (mapFileName ?? "0").trim().replace(/\.map$/i, "").toLowerCase() || "0";
}

function shouldReloadCrystalScene(
  region: OriginalMapRegion | null,
  mapFileName: string,
  center: { x: number; y: number },
  sceneKey: string,
  loadedSceneKey: string | null,
) {
  if (loadedSceneKey !== sceneKey) {
    return true;
  }

  if (!region) {
    return true;
  }

  if (normalizeMapFileName(region.mapFileName) !== mapFileName) {
    return true;
  }

  return (
    center.x <= region.playBounds.minX + SCENE_RELOAD_MARGIN_X ||
    center.x >= region.playBounds.maxX - SCENE_RELOAD_MARGIN_X ||
    center.y <= region.playBounds.minY + SCENE_RELOAD_MARGIN_Y ||
    center.y >= region.playBounds.maxY - SCENE_RELOAD_MARGIN_Y
  );
}

function originalMapRegionContainsTile(region: OriginalMapRegion | null, x: number, y: number) {
  if (!region) {
    return false;
  }
  return (
    x >= region.regionBounds.minX &&
    x <= region.regionBounds.maxX &&
    y >= region.regionBounds.minY &&
    y <= region.regionBounds.maxY
  );
}

function originalMapCellBlocksMovement(region: OriginalMapRegion | null, x: number, y: number) {
  if (!region) {
    return false;
  }
  const cell = region.cells.find((entry) => entry.x === x && entry.y === y);
  return Boolean(cell?.blocked || cell?.closedDoor);
}

function attackAnimationVariant(
  payload: Record<string, unknown>,
): "melee1" | "melee2" | "melee3" | "melee4" | "range" {
  if (typeof payload.spell === "string") {
    return "range";
  }

  switch (numberOrUndefined(payload.attackType)) {
    case 1:
      return "melee2";
    case 2:
      return "melee3";
    case 3:
      return "melee4";
    default:
      return "melee1";
  }
}

function crystalAttackActionDurationMs(
  entity: WorldEntity,
  animation: NonNullable<WorldEntity["attackAnimation"]>,
) {
  if (animation === "range" || animation === "melee3") {
    return 800;
  }

  return entity.kind === "monster" ? 600 : 600;
}

function crystalStruckActionDurationMs(entity: WorldEntity) {
  return entity.kind === "monster" ? 400 : 300;
}

function crystalDeathActionDurationMs(entity: WorldEntity) {
  return entity.kind === "monster" ? 1000 : 400;
}

function skillMatchesCrystalSpell(skill: KnownSkill, spell: string) {
  const normalizedSpell = normalizeCrystalToken(spell);
  return normalizeCrystalToken(skill.key) === normalizedSpell || normalizeCrystalToken(skill.name) === normalizedSpell;
}

function crystalBuffKey(objectId: string, buffType: number) {
  return `crystal-buff-${objectId}-${buffType}`;
}

function crystalBuffRemainingTicks(expireTime: number | undefined) {
  if (typeof expireTime !== "number" || expireTime <= 0) {
    return 0;
  }

  const now = Date.now();
  return expireTime > now ? Math.max(0, Math.ceil((expireTime - now) / 1000)) : Math.max(0, Math.ceil(expireTime));
}

function normalizeCrystalToken(value: string) {
  return value.replace(/[^a-z0-9]/gi, "").toLowerCase();
}

function stringOrNull(value: unknown) {
  return typeof value === "string" ? value : null;
}

function stringOrFallback(value: unknown, fallback: string) {
  return typeof value === "string" ? value : fallback;
}

function tileDistance(source: WorldEntity | null, target: WorldEntity) {
  if (!source) return 0;
  return pointTileDistance(source, target);
}

function pointTileDistance(source: { x: number; y: number }, target: { x: number; y: number }) {
  return Math.max(Math.abs(source.x - target.x), Math.abs(source.y - target.y));
}

function entitySortRank(entity: WorldEntity, playerObjectId: string | null, selectedObjectId: string | null, self: WorldEntity | null) {
  if (entity.objectId === selectedObjectId) return 0;
  if (entity.objectId === playerObjectId) return 1;
  if (entity.dead) return 100 + tileDistance(self, entity);
  if (entity.kind === "monster") return 2 + tileDistance(self, entity);
  if (entity.kind === "player") return 20 + tileDistance(self, entity);
  return 40 + tileDistance(self, entity);
}

function directionToward(source: WorldEntity | null, target: WorldEntity) {
  if (!source) return target.direction ?? "Down";
  return directionFromPoint(source, target, target.direction ?? "Down");
}

function chooseCrystalSelfRenderPosition(
  serverSelf: { x: number; y: number } | null,
  ...candidates: Array<PredictedPlayerMotion | null>
) {
  if (!serverSelf) return candidates.find(Boolean) ?? null;
  return candidates
    .filter(
      (candidate): candidate is PredictedPlayerMotion =>
        candidate !== null &&
        crystalMovementCandidateNotBehindServer(serverSelf, candidate, candidate.direction) &&
        Math.max(Math.abs(candidate.x - serverSelf.x), Math.abs(candidate.y - serverSelf.y)) <=
          MOVEMENT_LOCAL_RENDER_MAX_LEAD_TILES,
    )
    .reduce<PredictedPlayerMotion | null>((best, candidate) => {
      if (!best) return candidate;
      const bestLead = Math.max(Math.abs(best.x - serverSelf.x), Math.abs(best.y - serverSelf.y));
      const candidateLead = Math.max(Math.abs(candidate.x - serverSelf.x), Math.abs(candidate.y - serverSelf.y));
      return candidateLead >= bestLead ? candidate : best;
    }, null);
}

function movementStepIntervalMs(mode: "walk" | "run") {
  return mode === "run" ? RUN_STEP_INTERVAL_MS : WALK_STEP_INTERVAL_MS;
}

function movementCommandDelayMs(mode: "walk" | "run") {
  return Math.max(100, movementStepIntervalMs(mode) - MOVEMENT_QUEUE_INPUT_LEAD_MS);
}

function crystalMovementActionToward(
  source: { x: number; y: number; direction?: string },
  target: { x: number; y: number },
  requestedMode: "walk" | "run",
): { point: { x: number; y: number }; direction: string; mode: "walk" | "run" } {
  if (source.x === target.x && source.y === target.y) {
    return {
      point: { x: source.x, y: source.y },
      direction: source.direction ?? "Down",
      mode: "walk",
    };
  }

  const remainingDistance = Math.max(Math.abs(target.x - source.x), Math.abs(target.y - source.y));
  const mode = requestedMode === "run" && remainingDistance > 1 ? "run" : "walk";
  const direction = directionFromPoint(source, target, source.direction ?? "Down");
  return {
    point: pointMoveInDirection(source, direction, mode === "run" ? 2 : 1),
    direction,
    mode,
  };
}

function pointMoveInDirection(source: { x: number; y: number }, direction: string, distance: number) {
  switch (direction) {
    case "Up":
      return { x: source.x, y: source.y - distance };
    case "UpRight":
      return { x: source.x + distance, y: source.y - distance };
    case "Right":
      return { x: source.x + distance, y: source.y };
    case "DownRight":
      return { x: source.x + distance, y: source.y + distance };
    case "Down":
      return { x: source.x, y: source.y + distance };
    case "DownLeft":
      return { x: source.x - distance, y: source.y + distance };
    case "Left":
      return { x: source.x - distance, y: source.y };
    case "UpLeft":
      return { x: source.x - distance, y: source.y - distance };
    default:
      return { x: source.x, y: source.y };
  }
}

function directionVector(direction: string | undefined) {
  switch (direction) {
    case "Up":
      return { x: 0, y: -1 };
    case "UpRight":
      return { x: 1, y: -1 };
    case "Right":
      return { x: 1, y: 0 };
    case "DownRight":
      return { x: 1, y: 1 };
    case "Down":
      return { x: 0, y: 1 };
    case "DownLeft":
      return { x: -1, y: 1 };
    case "Left":
      return { x: -1, y: 0 };
    case "UpLeft":
      return { x: -1, y: -1 };
    default:
      return null;
  }
}

function movementDirectionsOppose(left: string | undefined, right: string | undefined) {
  const leftVector = directionVector(left);
  const rightVector = directionVector(right);
  if (!leftVector || !rightVector) {
    return false;
  }
  return leftVector.x * rightVector.x + leftVector.y * rightVector.y < 0;
}

function predictedPlayerAheadOfServer(
  server: { x: number; y: number },
  predicted: PredictedPlayerMotion,
  direction?: string,
) {
  const predictedDirection = predicted.direction ?? direction;
  if (!predictedDirection) return false;

  for (let distance = 1; distance <= MOVEMENT_LOCAL_ACTION_MAX_LEAD_TILES; distance += 1) {
    const point = pointMoveInDirection(server, predictedDirection, distance);
    if (point.x === predicted.x && point.y === predicted.y) {
      return true;
    }
  }

  return false;
}

function crystalMovementCandidateNotBehindServer(
  server: { x: number; y: number },
  candidate: PredictedPlayerMotion,
  direction?: string,
) {
  if (candidate.x === server.x && candidate.y === server.y) {
    return true;
  }
  return predictedPlayerAheadOfServer(server, candidate, direction);
}

function directionFromPoint(
  source: { x: number; y: number },
  target: { x: number; y: number },
  fallback = "Down",
) {
  const dx = Math.sign(target.x - source.x);
  const dy = Math.sign(target.y - source.y);
  if (dx === 0 && dy < 0) return "Up";
  if (dx > 0 && dy < 0) return "UpRight";
  if (dx > 0 && dy === 0) return "Right";
  if (dx > 0 && dy > 0) return "DownRight";
  if (dx === 0 && dy > 0) return "Down";
  if (dx < 0 && dy > 0) return "DownLeft";
  if (dx < 0 && dy === 0) return "Left";
  if (dx < 0 && dy < 0) return "UpLeft";
  return fallback;
}

function approachDestination(source: WorldEntity | null, target: WorldEntity) {
  if (!source) return { x: target.x, y: target.y };
  if (tileDistance(source, target) <= 1) return { x: source.x, y: source.y };

  const dx = Math.sign(target.x - source.x);
  const dy = Math.sign(target.y - source.y);
  const absX = Math.abs(target.x - source.x);
  const absY = Math.abs(target.y - source.y);

  if (dx !== 0 && absX >= absY) {
    return { x: target.x - dx, y: target.y };
  }
  if (dy !== 0) {
    return { x: target.x, y: target.y - dy };
  }
  return { x: target.x - dx, y: target.y - dy };
}

function entityKindClassName(kind: EntityKind) {
  return kind === "selfPlayer" ? "self" : kind === "player" ? "player" : kind;
}

function parseCharacters(
  payload: Record<string, unknown>,
  fallbackName: string,
  language: Mir2Language,
): SelectCharacterEntry[] {
  const candidates = Array.isArray(payload.characters)
    ? payload.characters
    : Array.isArray(payload.Characters)
      ? payload.Characters
      : [];

  const parsed = candidates
    .map((entry, index) => toCharacterEntry(entry, index, language))
    .filter((entry): entry is SelectCharacterEntry => entry !== null);

  if (parsed.length > 0) {
    return parsed;
  }

  return [fallbackCharacter(language, fallbackName)];
}

function toCharacterEntry(
  entry: unknown,
  fallbackIndex: number,
  language: Mir2Language,
): SelectCharacterEntry | null {
  if (!entry || typeof entry !== "object") {
    return null;
  }

  const value = entry as Record<string, unknown>;
  const translator = buildTranslator(language);
  const locale = languageLocale(language);
  return {
    index: typeof value.index === "number" ? value.index : typeof value.Index === "number" ? value.Index : fallbackIndex,
    name: stringOrFallback(value.name ?? value.Name, translator("ui.characterSlotFallback", [fallbackIndex + 1])),
    level: numberOrZero(value.level ?? value.Level) || 1,
    classKey: mapClassKey(value.class ?? value.Class),
    gender: mapGenderKey(value.gender ?? value.Gender),
    lastAccess: stringOrFallback(
      value.lastAccess ?? value.LastAccess,
      new Date().toLocaleString(locale),
    ),
  };
}

function fallbackCharacter(language: Mir2Language, fallbackName = ""): SelectCharacterEntry {
  const translator = buildTranslator(language);
  return {
    index: 0,
    name: fallbackName || translator("ui.characterFallbackName"),
    level: 1,
    classKey: "warrior",
    gender: "male",
    lastAccess: translator("client.Never", [], "Never"),
  };
}

function isFallbackCharacter(entry: SelectCharacterEntry) {
  return entry.index === 0 && entry.level === 1 && entry.classKey === "warrior";
}

function mapClassKey(value: unknown): SelectCharacterEntry["classKey"] {
  if (typeof value === "string") {
    const normalized = value.toLowerCase();
    if (normalized.includes("wizard")) return "wizard";
    if (normalized.includes("tao")) return "taoist";
    if (normalized.includes("assassin")) return "assassin";
    if (normalized.includes("archer")) return "archer";
    return "warrior";
  }

  if (typeof value === "number") {
    return value === 1 ? "wizard" : value === 2 ? "taoist" : value === 3 ? "assassin" : value === 4 ? "archer" : "warrior";
  }

  return "warrior";
}

function rankingPageKey(rankType: number, onlineOnly: boolean) {
  return `${rankType}:${onlineOnly ? "online" : "all"}`;
}

function mapGenderKey(value: unknown): SelectCharacterEntry["gender"] {
  if (typeof value === "string") {
    return value.toLowerCase().includes("female") ? "female" : "male";
  }

  if (typeof value === "number") {
    return value === 1 ? "female" : "male";
  }

  return "male";
}
