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
  | "group"
  | "guild"
  | "system"
  | "hint"
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
  dead: boolean;
  disposition: EntityDisposition;
  sprite?: GatewayWorldEntitySprite | null;
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
  mail?: Array<Record<string, unknown>>;
  trade?: Record<string, unknown> | null;
  auction?: Array<Record<string, unknown>>;
  conquest?: { castleOwner?: string; activeWars?: string[]; eventLog?: string[]; taxRatePercent?: number; gold?: number; guards?: number[]; walls?: number[]; gates?: number[]; openGates?: number[] };
  guildTerritory?: { owned?: boolean; mapFileName?: string; rentalDaysLeft?: number; recallLog?: string[] };
  hero?: Record<string, unknown> | null;
  profession?: { miningLevel?: number; ore?: number; craftedItems?: string[] };
  appearance?: { hair?: number };
  nameLists?: string[];
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
  dead?: boolean;
  disposition?: EntityDisposition;
  sprite?: GatewayWorldEntitySprite | null;
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
  slot: number;
  container: ItemContainer;
  quantity: number;
  description: string;
  durabilityCurrent?: number;
  durabilityMax?: number;
};

type ItemCommandRef = {
  key: string;
  slot: number;
  container: ItemContainer;
};

type EquipmentCommandRef = {
  slot: EquipmentSlot;
};

type ItemMoveRef = {
  slot: number;
  container: ItemContainer;
};

type ItemMergeRef = {
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

type MovementPlan = {
  targetX: number;
  targetY: number;
  mode: "walk" | "run";
  nextStepAt: number;
};

export default function HomePage() {
  const runtimeRef = useRef<RuntimeModule | null>(null);
  const socketRef = useRef<WebSocket | null>(null);
  const pendingLoginRef = useRef(false);
  const pendingNewAccountRef = useRef(false);
  const pendingTransferRef = useRef<string | null>(null);
  const pendingNpcInteractRef = useRef<string | null>(null);
  const movementPlanRef = useRef<MovementPlan | null>(null);
  const loadedSceneKeyRef = useRef<string | null>(null);

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
  const selectedEntity =
    world.entities.find((entity) => entity.objectId === world.selectedObjectId) ?? null;

  const sortedEntities = useMemo(
    () =>
      [...world.entities].sort((left, right) => {
        const leftRank = entitySortRank(left, world.playerObjectId, world.selectedObjectId, self);
        const rightRank = entitySortRank(right, world.playerObjectId, world.selectedObjectId, self);
        if (leftRank !== rightRank) return leftRank - rightRank;

        const leftDistance = tileDistance(self, left);
        const rightDistance = tileDistance(self, right);
        if (leftDistance !== rightDistance) return leftDistance - rightDistance;

        return left.name.localeCompare(right.name);
      }),
    [self, world.entities, world.playerObjectId, world.selectedObjectId],
  );

  const viewportEntities = useMemo(() => {
    if (!self) return [];

    return sortedEntities
      .filter(
        (entity) => Math.abs(entity.x - self.x) <= VIEWPORT_RANGE_X && Math.abs(entity.y - self.y) <= VIEWPORT_RANGE_Y,
      )
      .map((entity) => ({ ...entity, dx: entity.x - self.x, dy: entity.y - self.y }));
  }, [self, sortedEntities]);

  const viewportTiles = useMemo(() => {
    const center = self ?? world.sceneView?.center;
    if (!center) return [];

    const tiles: Array<{ x: number; y: number; dx: number; dy: number }> = [];

    for (let dy = -VIEWPORT_RANGE_Y; dy <= VIEWPORT_RANGE_Y; dy += 1) {
      for (let dx = -VIEWPORT_RANGE_X; dx <= VIEWPORT_RANGE_X; dx += 1) {
        tiles.push({ x: center.x + dx, y: center.y + dy, dx, dy });
      }
    }

    return tiles;
  }, [self, world.sceneView]);

  useEffect(() => {
    let disposed = false;

    async function bootRuntime() {
      try {
        appendLog(t("runtime.loadingModule"));
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
    if (!shouldReloadCrystalScene(world.originalMapRegion, normalizedMapFileName, center, sceneKey, loadedSceneKeyRef.current)) {
      return;
    }

    let disposed = false;

    async function loadSceneBlueprint() {
      try {
        appendLog(t("log.loadingSceneBlueprint"));
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
      }
    }

    void loadSceneBlueprint();
    return () => {
      disposed = true;
    };
  }, [self?.x, self?.y, world.mapFileName, world.originalMapRegion]);

  useEffect(() => {
    runtimeRef.current?.setMir2WorldState?.(JSON.stringify(world));
  }, [world]);

  useEffect(() => {
    if (!world.selectedObjectId) return;
    if (!world.entities.some((entity) => entity.objectId === world.selectedObjectId)) {
      setWorld((current) => ({ ...current, selectedObjectId: null }));
    }
  }, [world.entities, world.selectedObjectId]);

  useEffect(() => {
    if (!world.connected || wsState !== "open") return;

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
      return;
    }

    const timer = window.setInterval(() => {
      const plan = movementPlanRef.current;
      const player = world.entities.find((entity) => entity.objectId === world.playerObjectId);
      if (!plan || !player) return;
      if (Date.now() < plan.nextStepAt) return;

      if (player.x === plan.targetX && player.y === plan.targetY) {
        movementPlanRef.current = null;
        return;
      }

      const nextPoint = stepPointTowardBy(
        { x: player.x, y: player.y },
        { x: plan.targetX, y: plan.targetY },
        plan.mode === "run" ? 2 : 1,
      );

      if (nextPoint.x === player.x && nextPoint.y === player.y) {
        movementPlanRef.current = null;
        return;
      }

      movementPlanRef.current = {
        ...plan,
        nextStepAt: Date.now() + (plan.mode === "run" ? RUN_STEP_INTERVAL_MS : WALK_STEP_INTERVAL_MS),
      };
      send({ type: "moveTo", x: nextPoint.x, y: nextPoint.y, mode: plan.mode });
    }, 32);

    return () => window.clearInterval(timer);
  }, [screen, wsState, world.entities, world.playerObjectId]);

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
    setLogs((current) =>
      [
        {
          text: `[${new Date().toLocaleTimeString(locale)}] ${text}`,
          tone,
          channel,
        },
        ...current,
      ].slice(0, 24),
    );
  }

  function send(command: Record<string, unknown>, options?: { quiet?: boolean }) {
    if (socketRef.current?.readyState !== WebSocket.OPEN) return false;
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
          mapFileName: string | null;
          mapTitle: string | null;
          stage5Systems: Stage5SystemsState;
        };
      };
    };
    stage5Window.__mir2Stage5 = {
      send: (command) => send(command),
      state: {
        screen,
        mapFileName: world.mapFileName,
        mapTitle: world.mapTitle,
        stage5Systems: world.stage5Systems,
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

    const socket = new WebSocket("ws://127.0.0.1:7110/ws");
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
      sceneView: current.sceneView,
      terrainPatches: current.terrainPatches,
      decorObjects: current.decorObjects,
        originalMapRegion: current.originalMapRegion,
      }));
  }

  function moveToTile(x: number, y: number, mode: "walk" | "run") {
    movementPlanRef.current = {
      targetX: x,
      targetY: y,
      mode,
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
    send({
      type: "useItem",
      key: item.key,
      slot: item.slot,
      grid: item.container === "belt" ? "belt" : item.container === "quest" ? "questInventory" : "inventory",
    });
  }

  function dropItem(item: ItemCommandRef) {
    send({
      type: "dropItem",
      key: item.key,
      uniqueId: item.slot,
      count: 1,
      heroInventory: false,
    });
  }

  function equipItem(item: ItemCommandRef, slot: EquipmentSlot) {
    send({
      type: "equipItem",
      uniqueId: item.slot,
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
    send({
      type: "removeItem",
      uniqueId: equipmentSlotIndex(item.slot),
      grid: "equipment",
      to: 0,
    });
  }

  function moveItem(item: ItemMoveRef, toSlot: number) {
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
      from: item.slot,
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
      idFrom: from.slot,
      idTo: to.slot,
    });
  }

  function splitItem(item: ItemCommandRef, count: number) {
    send({
      type: "splitItem",
      uniqueId: item.slot,
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
      uniqueId: item.slot,
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

  function handleGatewayEvent(event: GatewayEvent) {
    if (event.type === "error") {
      appendLog(t("log.gatewayError", [event.message ?? t("error.unknown")]), "system");
      return;
    }
    if (event.type === "worldSnapshot") {
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
      case "MapInformation":
        setWorld((current) => ({
          ...current,
          mapTitle: stringOrNull(payload.title),
          mapFileName: stringOrNull(payload.fileName) ?? current.mapFileName,
          miniMapIndex: null,
        }));
        break;
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
            disposition: "friendly",
          }),
        }));
        setScreen("game");
        break;
      }
      case "UserLocation":
      case "ObjectTurn":
      case "ObjectWalk":
      case "ObjectRun":
      case "ObjectBackStep":
      case "ObjectSitDown":
        setWorld((current) => ({
          ...current,
          entities: current.entities.map((entity) =>
            entity.objectId ===
            (event.packet === "UserLocation" ? current.playerObjectId ?? "0" : stringifyId(payload.objectId))
              ? {
                  ...entity,
                  x: numberOrZero(payload.x),
                  y: numberOrZero(payload.y),
                  direction: stringOrNull(payload.direction) ?? undefined,
                }
              : entity,
          ),
        }));
        break;
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
        setWorldGroundDropFromPacket(payload, t("ui.gold", [], "Gold"));
        break;
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
      case "ObjectSpell":
        updateWorldEntityFromLocationPacket(payload);
        markWorldEntityAttack(payload);
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
        dead: payload.dead === true,
        disposition,
        sprite: spriteFromPacket(payload, kind),
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
      entities: patchEntityInList(current.entities, objectId, (entity) => ({
        ...entity,
        x: typeof location?.x === "number" ? location.x : entity.x,
        y: typeof location?.y === "number" ? location.y : entity.y,
        direction: stringOrNull(payload.direction) ?? entity.direction,
        attackAnimation: attackAnimationVariant(payload),
        attackStartedAt: now,
        attackUntil: now + 260,
      })),
    }));
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
        struckUntil: now + 220,
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
            struckUntil: now + 220,
          }))
        : current.entities,
    }));
  }

  function spawnRangeProjectile(payload: Record<string, unknown>) {
    const attackerId = stringifyId(payload.objectId);
    const targetId = stringifyId(payload.targetId);
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
      const projectile: ProjectileState = {
        key: `${attackerId}:${targetId}:${startedAt}`,
        attackerId,
        targetId,
        fromX: attacker.x,
        fromY: attacker.y,
        toX: target.x,
        toY: target.y,
        startedAt,
        expiresAt: startedAt + 280,
      };

      return {
        ...current,
        entities: patchEntityInList(current.entities, attackerId, (entity) => ({
          ...entity,
          direction: stringOrNull(payload.direction) ?? entity.direction,
          attackStartedAt: startedAt,
          attackAnimation: "range",
          attackUntil: startedAt + 260,
        })),
        projectiles: [...current.projectiles.filter((entry) => entry.expiresAt > startedAt), projectile],
      };
    });

  }

  function markWorldEntityDead(payload: Record<string, unknown>) {
    const location = payload.location as { x?: number; y?: number } | undefined;
    const objectId = stringifyId(payload.objectId);

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
        dieStartedAt: Date.now(),
        dieUntil: Date.now() + 420,
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
        reviveStartedAt: Date.now(),
        reviveUntil: Date.now() + 420,
      })),
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

  function applyGatewayWorldSnapshot(snapshot: GatewayWorldSnapshot) {
    const playerObjectId = snapshot.playerObjectId === null ? null : String(snapshot.playerObjectId);
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
      dead: entity.dead,
      disposition: entity.disposition,
      sprite: entity.sprite ?? null,
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

      return {
        ...current,
        mapTitle: snapshot.mapTitle ?? current.mapTitle,
        mapFileName: snapshot.mapFileName ?? current.mapFileName,
        inSafeZone: snapshot.inSafeZone ?? current.inSafeZone,
        playerObjectId,
        playerName: selfEntity?.name ?? current.playerName,
        playerHp: snapshot.playerHp ?? undefined,
        playerMaxHp: snapshot.playerMaxHp ?? undefined,
        playerMp: snapshot.playerMp ?? undefined,
        playerExperience: snapshot.playerExperience,
        playerMaxExperience: Math.max(snapshot.playerMaxExperience, 1),
        gold: snapshot.gold,
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
        terrainPatches: snapshot.terrainPatches.length ? snapshot.terrainPatches : current.terrainPatches,
        decorObjects: snapshot.decorObjects.length ? snapshot.decorObjects : current.decorObjects,
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

  return (
    <OriginalClientShell
      language={language}
      screen={screen}
      runtimePhase={runtimePhase}
      runtimeMessage={runtimeMessage}
      wsState={wsState}
      world={world}
      player={self}
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
      transferOptions={QUICK_TRANSFER_OPTIONS}
      onToggleCharacter={() => setShowCharacter((current) => !current)}
      onToggleInventory={() => setShowInventory((current) => !current)}
      onCloseCharacter={() => setShowCharacter(false)}
      onCloseInventory={() => setShowInventory(false)}
      onOpenCharacterTab={openCharacter}
      onOpenInventoryTab={openInventory}
      onViewportTileClick={(x, y) => handleViewportTileAction(x, y, "walk")}
      onViewportTileSecondaryAction={(x, y) => handleViewportTileAction(x, y, "run")}
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
        if (selectedEntity.kind === "npc") return interactTarget(selectedEntity.objectId);
        send({ type: "turn", direction: directionToward(self, selectedEntity) });
      }}
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

function upsertGroundDropInList(list: GroundDrop[], nextDrop: GroundDrop) {
  return list.some((drop) => drop.objectId === nextDrop.objectId)
    ? list.map((drop) => (drop.objectId === nextDrop.objectId ? { ...drop, ...nextDrop } : drop))
    : [...list, nextDrop];
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
      return "hint";
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
    case "boots":
      return 10;
    case "belt":
      return 11;
    case "stone":
      return 12;
    case "mount":
      return 13;
  }
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
  return target.direction ?? "Down";
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
