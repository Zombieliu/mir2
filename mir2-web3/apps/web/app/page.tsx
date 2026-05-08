"use client";

import { useEffect, useMemo, useRef, useState } from "react";

import { OriginalClientShell } from "./original-client-shell";
import {
  buildTranslator,
  formatRuntimeMessage,
  languageLocale,
  normalizeLanguage,
  type Mir2Language,
} from "../lib/localization";
import type {
  DecorObject,
  OriginalMapRegion,
  SceneBlueprint,
  SceneView,
  TerrainPatch,
} from "../lib/scene-types";
import type { ClientScreen } from "../lib/original-ui";

type RuntimeStatus = {
  phase: string;
  message: string;
};

type RuntimeModule = {
  default?: (input?: string | URL | Request) => Promise<unknown>;
  bootMir2Runtime?: () => void;
  setMir2WorldState?: (snapshotJson: string) => void;
  setMir2StatusSink?: (callback: (payload: RuntimeStatus) => void) => void;
};

type UiLogTone = "chat" | "system" | "network";
type UiLogChannel =
  | "normal"
  | "shout"
  | "whisper"
  | "trade"
  | "group"
  | "guild"
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
const WALK_STEP_INTERVAL_MS = 600;
const RUN_STEP_INTERVAL_MS = 600;
const CRYSTAL_ENTITY_MOVE_ACTION_MS = 600;
const MOVEMENT_SERVER_CORRECTION_GRACE_MS = 1800;
const MOVEMENT_LOCAL_ACTION_MAX_LEAD_TILES = 10;
const DEFAULT_GATEWAY_WS_URL =
  process.env.NEXT_PUBLIC_MIR2_GATEWAY_WS_URL ?? "ws://127.0.0.1:7110/ws";
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

function resolveGatewayWebSocketUrl() {
  if (typeof window === "undefined") return DEFAULT_GATEWAY_WS_URL;
  const queryValue = new URLSearchParams(window.location.search).get("gatewayWs");
  return queryValue && /^wss?:\/\//.test(queryValue) ? queryValue : DEFAULT_GATEWAY_WS_URL;
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
};

type DirectionStepRequest = {
  x: number;
  y: number;
  mode: "walk" | "run";
  requestedAt: number;
};

type DirectionStepPending = {
  x: number;
  y: number;
  mode: "walk" | "run";
  sentAt: number;
};

export default function HomePage() {
  const runtimeRef = useRef<RuntimeModule | null>(null);
  const socketRef = useRef<WebSocket | null>(null);
  const worldRef = useRef<WorldState>(DEFAULT_WORLD_STATE);
  const pendingLoginRef = useRef(false);
  const pendingNewAccountRef = useRef(false);
  const pendingTransferRef = useRef<string | null>(null);
  const pendingNpcInteractRef = useRef<string | null>(null);
  const gameEntryChatSeededRef = useRef(false);
  const movementPlanRef = useRef<MovementPlan | null>(null);
  const directionStepNextAtRef = useRef(0);
  const directionStepVisualUntilRef = useRef(0);
  const queuedDirectionStepRef = useRef<DirectionStepRequest | null>(null);
  const directionStepPendingRef = useRef<DirectionStepPending | null>(null);
  const predictedPlayerPositionRef = useRef<{ x: number; y: number } | null>(null);
  const loadedSceneKeyRef = useRef<string | null>(null);
  const loadingSceneKeyRef = useRef<string | null>(null);
  const lastCommandRef = useRef<Record<string, unknown> | null>(null);
  const worldSnapshotVersionRef = useRef(0);

  const [language, setLanguage] = useState<Mir2Language>("en");
  const [runtimePhase, setRuntimePhase] = useState("idle");
  const [runtimeMessage, setRuntimeMessage] = useState("Runtime not booted");
  const [screen, setScreen] = useState<ClientScreen>("login");
  const [world, setWorld] = useState<WorldState>(DEFAULT_WORLD_STATE);
  const [logs, setLogs] = useState<UiLogLine[]>([]);
  const [accountId, setAccountId] = useState("demo");
  const [password, setPassword] = useState("demo");
  const [chatMessage, setChatMessage] = useState("");
  const [loginBusy, setLoginBusy] = useState(false);
  const [loginErrorKey, setLoginErrorKey] = useState<string | null>(null);
  const [characters, setCharacters] = useState<SelectCharacterEntry[]>(() => [fallbackCharacter("en")]);
  const [selectedCharacterIndex, setSelectedCharacterIndex] = useState(0);
  const [wsState, setWsState] = useState("closed");
  const [showInventory, setShowInventory] = useState(false);
  const [showCharacter, setShowCharacter] = useState(false);
  const [activeInventoryTab, setActiveInventoryTab] = useState<"bag1" | "bag2" | "quest">("bag1");
  const [activeCharacterTab, setActiveCharacterTab] = useState<"char" | "stats1" | "stats2" | "spells">("char");
  const [storageServiceOpenVersion, setStorageServiceOpenVersion] = useState(0);
  const [predictedPlayerPosition, setPredictedPlayerPosition] = useState<{ x: number; y: number } | null>(null);
  const t = buildTranslator(language);
  const locale = languageLocale(language);

  useEffect(() => {
    if (typeof window === "undefined") return;
    setLanguage(normalizeLanguage(window.localStorage.getItem("mir2-language")));
  }, []);

  useEffect(() => {
    if (typeof window === "undefined") return;
    window.localStorage.setItem("mir2-language", language);
    setCharacters((current) =>
      current.length === 1 && isFallbackCharacter(current[0]) ? [fallbackCharacter(language)] : current,
    );
  }, [language]);

  const self = world.entities.find((entity) => entity.objectId === world.playerObjectId) ?? null;
  const predictedSelf = useMemo(
    () =>
      self &&
      predictedPlayerPosition &&
      (self.x !== predictedPlayerPosition.x || self.y !== predictedPlayerPosition.y) &&
      Math.max(Math.abs(self.x - predictedPlayerPosition.x), Math.abs(self.y - predictedPlayerPosition.y)) <=
        MOVEMENT_LOCAL_ACTION_MAX_LEAD_TILES
        ? { ...self, x: predictedPlayerPosition.x, y: predictedPlayerPosition.y }
        : self,
    [self, predictedPlayerPosition],
  );
  const displayEntities = useMemo(() => {
    if (!predictedSelf || !self || (predictedSelf.x === self.x && predictedSelf.y === self.y)) {
      return world.entities;
    }

    return world.entities.map((entity) =>
      entity.objectId === world.playerObjectId ? { ...entity, x: predictedSelf.x, y: predictedSelf.y } : entity,
    );
  }, [predictedSelf, self, world.entities, world.playerObjectId]);
  const selectedEntity =
    displayEntities.find((entity) => entity.objectId === world.selectedObjectId) ?? null;

  useEffect(() => {
    predictedPlayerPositionRef.current = predictedPlayerPosition;
  }, [predictedPlayerPosition]);

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
        if (!hasWebGl2Support()) {
          const message = "Bevy runtime skipped because WebGL2 is unavailable.";
          setRuntimePhase("dom-only");
          setRuntimeMessage(message);
          appendLog(message, "network");
          return;
        }

        appendLog(t("runtime.loadingModule"), "network");
        const runtimePath = "/bevy-runtime/pkg/mir2_bevy_runtime.js";
        const runtime = (await import(
          /* webpackIgnore: true */ runtimePath
        )) as RuntimeModule;
        if (disposed) return;

        if (typeof runtime.default === "function") {
          await runtime.default();
        }

        runtime.setMir2StatusSink?.((status) => {
          setRuntimePhase(status.phase);
          setRuntimeMessage(status.message);
        });

        runtime.bootMir2Runtime?.();
        runtime.setMir2WorldState?.(JSON.stringify(DEFAULT_WORLD_STATE));
        runtimeRef.current = runtime;
      } catch (error) {
        const message = error instanceof Error ? error.message : String(error);
        setRuntimePhase("boot-error");
        setRuntimeMessage(message);
        appendLog(t("runtime.bootFailed", [message]));
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
          width: String(VIEWPORT_RANGE_X * 2),
          height: String(VIEWPORT_RANGE_Y * 2),
        });
        const response = await fetch(`/api/scene/crystal?${params.toString()}`);
        if (!response.ok) throw new Error(`scene route returned ${response.status}`);

        const blueprint = (await response.json()) as SceneBlueprint;
        if (disposed) return;

        loadedSceneKeyRef.current = sceneKey;
        loadingSceneKeyRef.current = null;
        setWorld((current) => ({
          ...current,
          mapTitle: blueprint.mapTitle ?? current.mapTitle,
          miniMapIndex: blueprint.miniMapIndex,
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
    worldRef.current = world;
  }, [world]);

  useEffect(() => {
    if (!self || !predictedPlayerPosition) return;
    const visualUntil = movementPlanRef.current?.visualUntil ?? 0;
    const directionVisualUntil = directionStepVisualUntilRef.current;
    const directionPending = directionStepPendingRef.current;
    if (directionPending && self.x === directionPending.x && self.y === directionPending.y) {
      directionStepPendingRef.current = null;
      setPredictedPlayerPosition(null);
      return;
    }
    if (self.x !== predictedPlayerPosition.x || self.y !== predictedPlayerPosition.y) {
      if (
        Math.max(Math.abs(self.x - predictedPlayerPosition.x), Math.abs(self.y - predictedPlayerPosition.y)) >
          MOVEMENT_LOCAL_ACTION_MAX_LEAD_TILES ||
        Date.now() > Math.max(visualUntil, directionVisualUntil) + MOVEMENT_SERVER_CORRECTION_GRACE_MS
      ) {
        setPredictedPlayerPosition(null);
      }
      return;
    }
    if (!movementPlanRef.current && !directionStepPendingRef.current) {
      setPredictedPlayerPosition(null);
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
    const params = new URLSearchParams(window.location.search);
    if (params.get("autoTick") === "0") return;

    const timer = window.setInterval(() => {
      if (socketRef.current?.readyState === WebSocket.OPEN) {
        socketRef.current.send(JSON.stringify({ type: "keepAlive", time: Date.now() }));
        socketRef.current.send(JSON.stringify({ type: "tick" }));
      }
    }, 1200);

    return () => window.clearInterval(timer);
  }, [world.connected, wsState]);

  useEffect(() => {
    if (screen !== "game" || wsState !== "open") {
      movementPlanRef.current = null;
      queuedDirectionStepRef.current = null;
      directionStepPendingRef.current = null;
      setPredictedPlayerPosition(null);
      return;
    }

    let animationFrame = 0;
    const tickMovementPlan = () => {
      consumeQueuedDirectionStep();
      const plan = movementPlanRef.current;
      const currentWorld = worldRef.current;
      const player = currentWorld.entities.find((entity) => entity.objectId === currentWorld.playerObjectId);
      if (!plan || !player) {
        animationFrame = window.requestAnimationFrame(tickMovementPlan);
        return;
      }
      if (plan.packetMode === "direction") {
        animationFrame = window.requestAnimationFrame(tickMovementPlan);
        return;
      }
      if (plan.pendingX !== undefined && plan.pendingY !== undefined) {
        if (Date.now() - (plan.pendingSentAt ?? plan.nextStepAt) >= MOVEMENT_SERVER_CORRECTION_GRACE_MS) {
          movementPlanRef.current = null;
          setPredictedPlayerPosition(null);
        }
        animationFrame = window.requestAnimationFrame(tickMovementPlan);
        return;
      }

      const server = { x: player.x, y: player.y };
      const source =
        plan.actionX !== undefined &&
        plan.actionY !== undefined &&
        Math.max(Math.abs(server.x - plan.actionX), Math.abs(server.y - plan.actionY)) <=
          MOVEMENT_LOCAL_ACTION_MAX_LEAD_TILES
          ? { x: plan.actionX, y: plan.actionY }
          : server;

      if (Date.now() >= plan.nextStepAt) {
        if (source.x === plan.targetX && source.y === plan.targetY) {
          movementPlanRef.current = null;
          setPredictedPlayerPosition(null);
        } else {
          const nextPoint = stepPointTowardBy(
            source,
            { x: plan.targetX, y: plan.targetY },
            plan.mode === "run" ? 2 : 1,
          );

          if (nextPoint.x === source.x && nextPoint.y === source.y) {
            movementPlanRef.current = null;
            setPredictedPlayerPosition(null);
          } else {
            movementPlanRef.current = {
              ...plan,
              actionX: nextPoint.x,
              actionY: nextPoint.y,
              pendingX: nextPoint.x,
              pendingY: nextPoint.y,
              pendingSentAt: Date.now(),
              nextStepAt: Date.now() + (plan.mode === "run" ? RUN_STEP_INTERVAL_MS : WALK_STEP_INTERVAL_MS),
              visualUntil: Date.now() + (plan.mode === "run" ? RUN_STEP_INTERVAL_MS : WALK_STEP_INTERVAL_MS),
            };
            setPredictedPlayerPosition(nextPoint);
            send({ type: "moveTo", x: nextPoint.x, y: nextPoint.y, mode: plan.mode });
          }
        }
      }

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
    const debugWindow = window as typeof window & {
      __mir2LastCommand?: Record<string, unknown>;
      __mir2CommandHistory?: Array<Record<string, unknown>>;
    };
    const debugCommand = { ...command, at: Date.now() };
    debugWindow.__mir2LastCommand = command;
    debugWindow.__mir2CommandHistory = [debugCommand, ...(debugWindow.__mir2CommandHistory ?? [])].slice(0, 50);
    socketRef.current.send(JSON.stringify(command));
    if (!options?.quiet) appendLog(t("log.sent", [JSON.stringify(command)]), "network");
    return true;
  }

  useEffect(() => {
    const stage5Window = window as typeof window & {
      __mir2Stage5?: {
        send: (command: Record<string, unknown>) => boolean;
        state: {
          screen: ClientScreen;
          language: Mir2Language;
          accountId: string;
          wsState: string;
          loginBusy: boolean;
          selectedCharacterIndex: number;
          characters: SelectCharacterEntry[];
          mapFileName: string | null;
          mapTitle: string | null;
          player: { x: number; y: number } | null;
          predictedPlayer: { x: number; y: number } | null;
          movementPlan: MovementPlan | null;
          directionStepPending: DirectionStepPending | null;
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
          worldTick: number;
        };
      };
    };
    stage5Window.__mir2Stage5 = {
      send: (command) => send(command),
      state: {
        screen,
        language,
        accountId,
        wsState,
        loginBusy,
        selectedCharacterIndex,
        characters,
        mapFileName: world.mapFileName,
        mapTitle: world.mapTitle,
        player: self ? { x: self.x, y: self.y } : null,
        predictedPlayer: predictedPlayerPosition,
        movementPlan: movementPlanRef.current,
        directionStepPending: directionStepPendingRef.current,
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
        worldTick: world.worldTick,
      },
    };
    return () => {
      delete stage5Window.__mir2Stage5;
    };
  });

  function connectGateway(bootstrapAfterOpen = false) {
    if (socketRef.current?.readyState === WebSocket.OPEN) {
      if (bootstrapAfterOpen) sendBootstrapSequence(accountId, password);
      return;
    }

    const socket = new WebSocket(resolveGatewayWebSocketUrl());
    socketRef.current = socket;
    setWsState("connecting");

    socket.addEventListener("open", () => {
      setWsState("open");
      setWorld((current) => ({ ...current, connected: true }));
      appendLog(t("log.gatewayWsOpen"), "network");
      send({ type: "setLanguage", language }, { quiet: true });
      if (pendingNewAccountRef.current) {
        pendingNewAccountRef.current = false;
        sendNewAccountCommand();
        return;
      }
      if (pendingLoginRef.current) {
        pendingLoginRef.current = false;
        send({ type: "clientVersion" }, { quiet: true });
        send({ type: "login", accountId, password });
        return;
      }
      if (bootstrapAfterOpen) sendBootstrapSequence(accountId, password);
    });

    socket.addEventListener("close", () => {
      setWsState("closed");
      setWorld((current) => ({ ...current, connected: false }));
      appendLog(t("log.gatewayWsClosed"), "network");
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

  function sendBootstrapSequence(nextAccountId: string, nextPassword: string) {
    send({ type: "clientVersion" });
    send({ type: "login", accountId: nextAccountId, password: nextPassword });
    send({ type: "startGame", characterIndex: 0 });
  }

  function sendNewAccountCommand() {
    send({ type: "clientVersion" }, { quiet: true });
    send({
      type: "newAccount",
      accountId,
      password,
      birthDateBinary: 0,
      userName: accountId,
      secretQuestion: "",
      secretAnswer: "",
      emailAddress: "",
    });
  }

  function createAccount() {
    setLoginBusy(false);
    setLoginErrorKey(null);

    if (socketRef.current?.readyState !== WebSocket.OPEN) {
      pendingNewAccountRef.current = true;
      connectGateway();
      return;
    }

    sendNewAccountCommand();
  }

  function submitLogin() {
    setLoginBusy(true);
    setLoginErrorKey(null);

    if (socketRef.current?.readyState !== WebSocket.OPEN) {
      pendingLoginRef.current = true;
      connectGateway();
      return;
    }

    send({ type: "clientVersion" });
    send({ type: "login", accountId, password });
  }

  function startSelectedCharacter() {
    const selected = characters[selectedCharacterIndex] ?? characters[0];
    send({ type: "startGame", characterIndex: selected?.index ?? 0 });
  }

  function quickEnterWorld() {
    if (socketRef.current?.readyState !== WebSocket.OPEN) {
      connectGateway(true);
      return;
    }
    sendBootstrapSequence(accountId, password);
  }

  function resetClient() {
    pendingLoginRef.current = false;
    pendingNewAccountRef.current = false;
    pendingTransferRef.current = null;
    pendingNpcInteractRef.current = null;
    movementPlanRef.current = null;
    queuedDirectionStepRef.current = null;
    gameEntryChatSeededRef.current = false;
    if (socketRef.current?.readyState === WebSocket.OPEN) {
      send({ type: "disconnect" });
    }
    socketRef.current?.close();
    socketRef.current = null;
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

  function moveToTile(x: number, y: number, mode: "walk" | "run", packetMode: "target" | "direction" = "target") {
    queuedDirectionStepRef.current = null;
    const currentPlan = movementPlanRef.current;
    movementPlanRef.current = currentPlan
      ? {
          ...currentPlan,
          targetX: x,
          targetY: y,
          mode,
          packetMode,
          actionX: currentPlan.actionX,
          actionY: currentPlan.actionY,
          pendingX: currentPlan.pendingX,
          pendingY: currentPlan.pendingY,
          pendingSentAt: currentPlan.pendingSentAt,
          nextStepAt: currentPlan.nextStepAt || Date.now(),
        }
      : {
          targetX: x,
          targetY: y,
          mode,
          packetMode,
          nextStepAt: 0,
        };
  }

  function attackTarget(objectId: string) {
    send({ type: "attack", objectId: Number(objectId) });
  }

  function createCharacter() {
    const suffix = Math.random().toString(36).slice(2, 6).toUpperCase();
    send({
      type: "newCharacter",
      name: `MIR${suffix}`,
      gender: "male",
      class: "warrior",
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
        const destination = approachDestination(self, entity);
        pendingNpcInteractRef.current = objectId;
        moveToTile(destination.x, destination.y, "run");
        return;
      }
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
    const currentWorld = worldRef.current;
    const serverSelf = currentWorld.entities.find((entity) => entity.objectId === currentWorld.playerObjectId) ?? self;
    if (!serverSelf) return;
    const currentPlan = movementPlanRef.current;
    if (currentPlan?.packetMode === "direction" && Date.now() < currentPlan.nextStepAt) {
      return;
    }
    const currentSelf =
      currentPlan?.actionX !== undefined &&
      currentPlan.actionY !== undefined &&
      Math.max(Math.abs(serverSelf.x - currentPlan.actionX), Math.abs(serverSelf.y - currentPlan.actionY)) <=
        MOVEMENT_LOCAL_ACTION_MAX_LEAD_TILES
        ? { x: currentPlan.actionX, y: currentPlan.actionY }
        : serverSelf;

    const nextPoint = stepPointTowardBy(currentSelf, { x, y }, mode === "run" ? 2 : 1);
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
    queuedDirectionStepRef.current = { x, y, mode, requestedAt: Date.now() };
    consumeQueuedDirectionStep();
  }

  function consumeQueuedDirectionStep() {
    const queued = queuedDirectionStepRef.current;
    if (!queued) return false;
    const now = Date.now();
    if (now < directionStepNextAtRef.current) return false;

    const currentWorld = worldRef.current;
    const serverSelf = currentWorld.entities.find((entity) => entity.objectId === currentWorld.playerObjectId) ?? self;
    if (!serverSelf) {
      queuedDirectionStepRef.current = null;
      return false;
    }

    const pending = directionStepPendingRef.current;
    if (pending) {
      if (serverSelf.x === pending.x && serverSelf.y === pending.y) {
        directionStepPendingRef.current = null;
        if (
          predictedPlayerPositionRef.current?.x === pending.x &&
          predictedPlayerPositionRef.current?.y === pending.y
        ) {
          setPredictedPlayerPosition(null);
        }
      } else if (now - pending.sentAt < MOVEMENT_SERVER_CORRECTION_GRACE_MS) {
        return false;
      } else {
        directionStepPendingRef.current = null;
        setPredictedPlayerPosition(null);
      }
    }

    queuedDirectionStepRef.current = null;
    const actionSelf = serverSelf;
    const direction = directionFromPoint(actionSelf, { x: queued.x, y: queued.y }, actionSelf.direction ?? "Down");
    const nextPoint = stepPointTowardBy(actionSelf, { x: queued.x, y: queued.y }, queued.mode === "run" ? 2 : 1);
    directionStepNextAtRef.current = now + (queued.mode === "run" ? RUN_STEP_INTERVAL_MS : WALK_STEP_INTERVAL_MS);
    directionStepVisualUntilRef.current = directionStepNextAtRef.current;
    movementPlanRef.current = null;
    if (nextPoint.x !== actionSelf.x || nextPoint.y !== actionSelf.y) {
      directionStepPendingRef.current = {
        x: nextPoint.x,
        y: nextPoint.y,
        mode: queued.mode,
        sentAt: now,
      };
      setPredictedPlayerPosition(nextPoint);
    } else {
      directionStepPendingRef.current = null;
      setPredictedPlayerPosition(null);
    }
    send({ type: queued.mode === "run" ? "run" : "walk", direction });
    return true;
  }

  function handleGatewayEvent(event: GatewayEvent) {
    const debugWindow = window as typeof window & {
      __mir2LastGatewayEvent?: Record<string, unknown>;
      __mir2GatewayEventHistory?: Array<Record<string, unknown>>;
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
      appendLog(t("log.gatewayError", [event.message ?? t("error.unknown")]), "system");
      return;
    }
    if (event.type === "worldSnapshot") {
      worldSnapshotVersionRef.current += 1;
      applyGatewayWorldSnapshot(event.payload as GatewayWorldSnapshot);
      return;
    }
    if (event.type !== "packet" || !event.packet) return;

    appendLog(t("log.recv", [event.packet]), "network");
    const payload = event.payload ?? {};

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
        setLoginBusy(false);
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
        setLoginBusy(false);
        setLoginErrorKey("error.loginFailedCheckAccountPassword");
        setScreen("login");
        break;
      case "LoginBanned":
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
        setScreen("login");
        break;
      case "NewCharacterSuccess":
        setCharacters((current) => {
          const nextCharacters = parseCharacters({ characters: [...current, payload.character] }, accountId, language);
          return nextCharacters.slice(0, 4);
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
        setCharacters(parseCharacters(payload, accountId, language));
        setSelectedCharacterIndex(0);
        setScreen("select");
        break;
      case "StartGame":
        if (numberOrZero(payload.result) !== 4) {
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
      case "MapInformation": {
        const miniMapIndex = numberOrUndefined(payload.miniMapIndex);
        const bigMapIndex = numberOrUndefined(payload.bigMapIndex);
        setWorld((current) => ({
          ...current,
          mapTitle: stringOrNull(payload.title),
          mapFileName: stringOrNull(payload.fileName) ?? current.mapFileName,
          miniMapIndex: miniMapIndex && miniMapIndex > 0 ? miniMapIndex : null,
          bigMapIndex: bigMapIndex && bigMapIndex > 0 ? bigMapIndex : null,
        }));
        break;
      }
      case "UserInformation": {
        const objectId = stringifyId(payload.objectId);
        const location = payload.location as { x?: number; y?: number } | undefined;
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
            x: numberOrZero(location?.x),
            y: numberOrZero(location?.y),
            direction: stringOrNull(payload.direction) ?? undefined,
            level: numberOrUndefined(payload.level),
            hp: numberOrUndefined(payload.hp),
            maxHp: numberOrUndefined(payload.hp),
            nameColourArgb: -1,
            disposition: "friendly",
          }),
        }));
        setScreen("game");
        appendCrystalGameEntryChat();
        break;
      }
      case "UserLocation":
      case "ObjectTurn":
      case "ObjectWalk":
      case "ObjectRun":
      case "ObjectBackStep":
      case "ObjectSitDown": {
        const movementObjectId =
          event.packet === "UserLocation" ? worldRef.current.playerObjectId ?? "0" : stringifyId(payload.objectId);
        const x = numberOrZero(payload.x);
        const y = numberOrZero(payload.y);
        const movementPacket = event.packet;
        const movementNow = Date.now();
        if (movementObjectId === worldRef.current.playerObjectId) {
          reconcileMovementPlanWithServer(x, y);
          reconcileDirectionStepWithServer(x, y);
        }
        setWorld((current) => ({
          ...current,
          entities: current.entities.map((entity) =>
            entity.objectId === movementObjectId
              ? withPacketMovementAnimation(
                  {
                    ...entity,
                    x,
                    y,
                    direction: stringOrNull(payload.direction) ?? undefined,
                  },
                  entity,
                  movementPacket,
                  movementNow,
                )
              : entity,
          ),
        }));
        break;
      }
      case "ObjectPlayer":
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
        appendLog(
          t(
            "ui.startGameBanned",
            [stringOrFallback(payload.reason, t("error.unknown"))],
            `Start game banned: ${stringOrFallback(payload.reason, t("error.unknown"))}.`,
          ),
          "system",
        );
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
    setWorld((current) => ({
      ...current,
      entities: upsertEntityInList(current.entities, {
        objectId: stringifyId(payload.objectId),
        kind,
        name: stringOrFallback(
          payload.name,
          kind === "npc" ? t("ui.npc") : kind === "monster" ? t("ui.monster") : t("ui.player"),
        ),
        x: numberOrZero(location?.x),
        y: numberOrZero(location?.y),
        direction: stringOrNull(payload.direction) ?? undefined,
        classKey: kind === "player" || kind === "selfPlayer" ? mapClassKey(payload.class) : undefined,
        genderKey: kind === "player" || kind === "selfPlayer" ? mapGenderKey(payload.gender) : undefined,
        level: numberOrUndefined(payload.level),
        nameColourArgb: numberOrUndefined(payload.nameColourArgb) ?? (kind === "npc" ? -16_711_936 : -1),
        dead: payload.dead === true,
        disposition,
        sprite: spriteFromPacket(payload, kind),
        bigMapIcon: numberOrUndefined(payload.bigMapIcon),
        showOnBigMap: payload.showOnBigMap === true ? true : payload.showOnBigMap === false ? false : undefined,
        canTeleportTo: payload.canTeleportTo === true ? true : payload.canTeleportTo === false ? false : undefined,
      }),
    }));
  }

  function setWorldGroundDropFromPacket(payload: Record<string, unknown>, fallbackName: string) {
    const location = payload.location as { x?: number; y?: number } | undefined;
    const objectId = stringifyId(payload.objectId);

    setWorld((current) => ({
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
    }));
  }

  function removeObjectFromWorld(objectId: string) {
    setWorld((current) => ({
      ...current,
      selectedObjectId: current.selectedObjectId === objectId ? null : current.selectedObjectId,
      activeNpcDialog: current.activeNpcDialog?.npcObjectId === objectId ? null : current.activeNpcDialog,
      entities: current.entities.filter((entity) => entity.objectId !== objectId),
      groundDrops: current.groundDrops.filter((drop) => drop.objectId !== objectId),
    }));
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

    setWorld((current) => ({
      ...current,
      entities: patchEntityInList(current.entities, objectId, (entity) => ({
        ...entity,
        x: numberOrZero(location?.x),
        y: numberOrZero(location?.y),
        direction: stringOrNull(payload.direction) ?? entity.direction,
      })),
    }));
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

  function applyGatewayWorldSnapshot(snapshot: GatewayWorldSnapshot) {
    const playerObjectId = snapshot.playerObjectId === null ? null : String(snapshot.playerObjectId);
    const previousEntitiesById = new Map(worldRef.current.entities.map((entity) => [entity.objectId, entity]));
    const snapshotNow = Date.now();
    const entities = snapshot.entities.map((entity) => ({
      objectId: String(entity.objectId),
      kind: entity.kind,
      name: entity.name,
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
      sprite: entity.sprite ?? null,
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
    const groundDrops = (snapshot.groundDrops ?? []).map((drop) => ({
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
        const transient = transientByObjectId.get(entity.objectId);
        if (!transient) {
          return entity;
        }

        return {
          ...entity,
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
      const selfEntity = mergedEntities.find((entity) => entity.objectId === playerObjectId) ?? null;
      if (selfEntity) {
        reconcileMovementPlanWithServer(selfEntity.x, selfEntity.y);
      }
      const snapshotMapFileName = snapshot.mapFileName ?? current.mapFileName;
      const hasCurrentSceneForSnapshot =
        current.originalMapRegion !== null &&
        normalizeMapFileName(current.originalMapRegion.mapFileName) === normalizeMapFileName(snapshotMapFileName);

      return {
        ...current,
        mapTitle: snapshot.mapTitle ?? current.mapTitle,
        mapFileName: snapshotMapFileName,
        inSafeZone: snapshot.inSafeZone ?? current.inSafeZone,
        playerObjectId,
        playerName: selfEntity?.name ?? current.playerName,
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
        entities: mergedEntities,
        groundDrops,
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
    });
    const pendingTransferKey = pendingTransferRef.current;
    const selfOnTransfer = entities.some(
      (entity) =>
        entity.objectId === playerObjectId &&
        pendingTransferKey === transferKeyForWorldTile(mapTransfers, snapshot.mapFileName ?? null, entity.x, entity.y),
    );
    if (pendingTransferKey && selfOnTransfer) {
      transferMap(pendingTransferKey);
    }
    const pendingNpcInteractObjectId = pendingNpcInteractRef.current;
    if (pendingNpcInteractObjectId) {
      const pendingNpc = entities.find((entity) => entity.objectId === pendingNpcInteractObjectId);
      const nextSelf = entities.find((entity) => entity.objectId === playerObjectId) ?? null;
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

  function reconcileMovementPlanWithServer(x: number, y: number) {
    const plan = movementPlanRef.current;
    if (!plan || plan.pendingX === undefined || plan.pendingY === undefined) {
      return;
    }

    if (x === plan.pendingX && y === plan.pendingY) {
      movementPlanRef.current = {
        ...plan,
        actionX: x,
        actionY: y,
        pendingX: undefined,
        pendingY: undefined,
        pendingSentAt: undefined,
      };
      return;
    }

    const sentAt = plan.pendingSentAt ?? plan.nextStepAt;
    if (Date.now() - sentAt < MOVEMENT_SERVER_CORRECTION_GRACE_MS) {
      return;
    }

    movementPlanRef.current = null;
    setPredictedPlayerPosition(null);
  }

  function reconcileDirectionStepWithServer(x: number, y: number) {
    const pending = directionStepPendingRef.current;
    if (!pending) {
      return;
    }

    if (x === pending.x && y === pending.y) {
      directionStepPendingRef.current = null;
      if (predictedPlayerPositionRef.current?.x === x && predictedPlayerPositionRef.current?.y === y) {
        setPredictedPlayerPosition(null);
      }
      return;
    }

    if (Date.now() - pending.sentAt >= MOVEMENT_SERVER_CORRECTION_GRACE_MS) {
      directionStepPendingRef.current = null;
      setPredictedPlayerPosition(null);
    }
  }

  return (
    <OriginalClientShell
      language={language}
      screen={screen}
      runtimePhase={runtimePhase}
      runtimeMessage={runtimeMessage}
      wsState={wsState}
      world={world}
      player={self}
      predictedPlayerPosition={predictedPlayerPosition}
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
      onQuickEnter={quickEnterWorld}
      onResetClient={resetClient}
      onSendChat={() => send({ type: "chat", message: chatMessage })}
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
        send({ type: "turn", direction: directionToward(self, selectedEntity) });
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
      return "running";
    case "ObjectWalk":
    case "ObjectBackStep":
      return "walking";
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

function hasWebGl2Support() {
  try {
    const canvas = document.createElement("canvas");
    return Boolean(canvas.getContext("webgl2"));
  } catch {
    return false;
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
    case "system":
      return "system";
    case "hint":
      return "server";
    case "server":
      return "server";
    case "announcement":
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

  const marginX = Math.max(2, Math.floor(VIEWPORT_RANGE_X / 3));
  const marginY = Math.max(2, Math.floor(VIEWPORT_RANGE_Y / 3));
  return (
    center.x <= region.playBounds.minX + marginX ||
    center.x >= region.playBounds.maxX - marginX ||
    center.y <= region.playBounds.minY + marginY ||
    center.y >= region.playBounds.maxY - marginY
  );
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
  const dx = Math.sign(target.x - source.x);
  const dy = Math.sign(target.y - source.y);
  if (tileDistance(source, target) <= 1) return { x: source.x, y: source.y };
  return { x: target.x - dx, y: target.y - dy };
}

function stepPointTowardBy(
  source: { x: number; y: number },
  target: { x: number; y: number },
  distance: number,
) {
  const dx = target.x - source.x;
  const dy = target.y - source.y;
  const stepX = Math.sign(dx) * Math.min(Math.abs(dx), distance);
  const stepY = Math.sign(dy) * Math.min(Math.abs(dy), distance);
  return {
    x: source.x + stepX,
    y: source.y + stepY,
  };
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

function mapGenderKey(value: unknown): SelectCharacterEntry["gender"] {
  if (typeof value === "string") {
    return value.toLowerCase().includes("female") ? "female" : "male";
  }

  if (typeof value === "number") {
    return value === 1 ? "female" : "male";
  }

  return "male";
}
