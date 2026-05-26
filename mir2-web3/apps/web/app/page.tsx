"use client";

import { useEffect, useMemo, useRef, useState } from "react";
import { flushSync } from "react-dom";

import { OriginalClientShell } from "./original-client-shell";
import {
  buildTranslator,
  formatRuntimeMessage,
  languageLocale,
  normalizeLanguage,
  type Mir2Language,
} from "../lib/localization";
import { playOriginalSoundId } from "../lib/original-audio";
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

type RuntimeStatus = {
  phase: string;
  message: string;
};

type RuntimeModule = {
  default?: (input?: string | URL | Request) => Promise<unknown>;
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
  cooldownRemainingTicks: number;
};

type GatewayActiveBuff = {
  key: string;
  name: string;
  description: string;
  remainingTicks: number;
  attackBonus: number;
  defenceBonus: number;
};

type Stage5SystemsState = {
  group?: { members?: string[]; lootMode?: string };
  guild?: { name?: string; members?: string[]; rank?: string; permissions?: string[]; chatLog?: string[] };
  social?: { friends?: string[]; blocked?: string[] };
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
  cooldownRemainingTicks: number;
};

type ActiveBuff = {
  key: string;
  name: string;
  description: string;
  remainingTicks: number;
  attackBonus: number;
  defenceBonus: number;
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
  return command.type === "walk" || command.type === "run" || command.type === "moveTo" || command.type === "turn";
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
  const [activeInventoryTab, setActiveInventoryTab] = useState<"bag1" | "bag2" | "quest">("bag1");
  const [activeCharacterTab, setActiveCharacterTab] = useState<"char" | "stats1" | "stats2" | "spells">("char");
  const [storageServiceOpenVersion, setStorageServiceOpenVersion] = useState(0);
  const [predictedPlayerPosition, setPredictedPlayerPosition] = useState<PredictedPlayerMotion | null>(null);
  const [initialSceneAssetsReady, setInitialSceneAssetsReady] = useState(false);
  const t = buildTranslator(language);
  const locale = languageLocale(language);

  useEffect(() => {
    if (typeof window === "undefined") return;
    markMir2CacheMilestone("htmlReady", {
      href: window.location.href,
    });
    setLanguage(normalizeLanguage(window.localStorage.getItem("mir2-language")));
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

    // Keep movement unlocked after the first playable scene; before that, later renderer readiness can still tighten the gate.
    const nextSceneInteractionReady = readiness.interactionReady ?? readiness.ready;
    if (initialSceneAssetsReadyRef.current !== nextSceneInteractionReady) {
      setInitialSceneAssetsReadyState(nextSceneInteractionReady);
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
        if (!response.ok) throw new Error(`scene route returned ${response.status}`);

        const blueprint = (await response.json()) as SceneBlueprint;
        if (disposed) return;
        markMir2CacheMilestone("sceneBlueprintReady", {
          sceneKey,
          sceneCache: response.headers.get("x-mir2-scene-cache"),
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
    return command.type === "walk" || command.type === "run" || command.type === "moveTo";
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

  function castSkill(skillKey: string) {
    send({ type: "castSkill", key: skillKey });
  }

  function transferMap(key: string) {
    pendingTransferRef.current = null;
    crystalRunPrimedUntilRef.current = 0;
    send({ type: "transferMap", key });
  }

  function claimMail(mailId: number) {
    send({ type: "stage5Command", action: "mail.claim", args: [String(mailId)] });
  }

  function deleteMail(mailId: number) {
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
    const occupant = world.entities.find(
      (entity) => entity.objectId !== world.playerObjectId && !entity.dead && entity.x === x && entity.y === y,
    );
    if (occupant) {
      activateEntity(occupant.objectId);
      return;
    }
    const drop = world.groundDrops.find((entry) => entry.x === x && entry.y === y);
    if (drop) {
      pickGroundDrop(drop.objectId);
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

    if (typeof percent !== "number") return;

    setWorld((current) => {
      const nextPlayerHp =
        current.playerObjectId === objectId && typeof current.playerMaxHp === "number"
          ? Math.max(0, Math.round((current.playerMaxHp * percent) / 100))
          : current.playerHp;

      return {
        ...current,
        playerHp: nextPlayerHp,
        entities: patchEntityInList(current.entities, objectId, (entity) => {
          const nextHp =
            typeof entity.maxHp === "number" ? Math.max(0, Math.round((entity.maxHp * percent) / 100)) : entity.hp;

          return {
            ...entity,
            hp: nextHp,
            dead: percent <= 0,
            reviveStartedAt: percent > 0 && entity.dead ? Date.now() : entity.reviveStartedAt,
            reviveUntil: percent > 0 && entity.dead ? Date.now() + 420 : entity.reviveUntil,
          };
        }),
      };
    });
  }

  function applyObjectManaPacket(payload: Record<string, unknown>) {
    const objectId = stringifyId(payload.objectId);
    const percent = numberOrUndefined(payload.percent);

    if (typeof percent !== "number") return;

    setWorld((current) => ({
      ...current,
      playerMp:
        current.playerObjectId === objectId ? Math.max(0, Math.min(100, Math.round(percent))) : current.playerMp,
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
      cooldownRemainingTicks: skill.cooldownRemainingTicks,
    }));
    const activeBuffs = (snapshot.activeBuffs ?? []).map((buff) => ({
      key: buff.key,
      name: buff.name,
      description: buff.description,
      remainingTicks: buff.remainingTicks,
      attackBonus: buff.attackBonus,
      defenceBonus: buff.defenceBonus,
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
        if (selectedEntity.kind === "monster") return attackTarget(selectedEntity.objectId);
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
  );
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

async function loadBevyRuntimeModule(backend: BevyRuntimeBackend): Promise<RuntimeModule> {
  const runtimeVersionQuery = encodeURIComponent(BEVY_RUNTIME_VERSION);
  const runtimePackageDir = bevyRuntimePackageDir(backend);
  const runtimePath = `/bevy-runtime/${runtimePackageDir}/mir2_bevy_runtime.js?v=${runtimeVersionQuery}`;
  const runtimeWasmPath = `/bevy-runtime/${runtimePackageDir}/mir2_bevy_runtime_bg.wasm?v=${runtimeVersionQuery}`;
  const runtime = (await import(
    /* webpackIgnore: true */ runtimePath
  )) as RuntimeModule;

  if (typeof runtime.default === "function") {
    await runtime.default(runtimeWasmPath);
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
      return "server";
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
