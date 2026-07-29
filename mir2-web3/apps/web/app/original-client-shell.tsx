"use client";

import {
  memo,
  useCallback,
  useEffect,
  useLayoutEffect,
  useMemo,
  useRef,
  useState,
  type CSSProperties,
  type MouseEvent,
} from "react";

import {
  ORIGINAL_UI,
  type ClientScreen,
  type CharacterTabKey,
  type InventoryTabKey,
} from "../lib/original-ui";
import { createAssetResidency } from "../lib/asset-residency";
import { createBrowserAtlasFetcher } from "../lib/asset-residency/browser-adapters";
import type { AtlasPagePayload, PersistentStore } from "../lib/asset-residency/types";
import { buildCrystalFullPackAtlasSnapshot } from "../lib/crystal-full-pack-bevy";
import { loadCrystalFullPackIndex } from "../lib/crystal-full-pack-index";
import { normalizeDeviceMemoryGiB, resolveRenderTier } from "../lib/render-tier";
import {
  loadOriginalSceneSpriteLibrary,
  normalizeSceneSpriteLibraryKey,
  originalSceneSpriteLibraryExists,
  originalSceneSpriteLibraryCacheStats,
  type OriginalSceneSpriteLibraryMeta,
} from "../lib/original-scene-sprite-meta";
import {
  buildTranslator,
  formatRuntimeMessage,
  formatRuntimePhase,
  languageLocale,
  type Mir2Language,
} from "../lib/localization";
import {
  playOriginalSoundPath,
  setOriginalMusic,
  stopOriginalAudio,
  unlockOriginalAudio,
} from "../lib/original-audio";
import {
  LanguageSelector,
  LoginOverlay,
  SelectOverlay,
} from "./components/original-client-overlays";
import { GameUiScene, GameUiSceneStoreBound } from "./components/original-client-game-ui-scene";
import {
  calculateMir2StagePresentation,
  calculateMir2TouchControlDeck,
} from "./components/original-client-stage-presentation";
import type {
  BevyEntityRenderState,
  BevyMapRenderState,
  OriginalClientShellProps,
  SceneAssetReadiness,
} from "./components/original-client-shell-types";
import {
  LOGIN_STATIC_BACKGROUND_FRAME,
  LOGIN_TRANSITION_FRAME_MS,
  ORIGINAL_AUDIO,
  ORIGINAL_EFFECT_VOLUME,
  desiredMusicForScreen,
  entityKindLabelKey,
  selectedTargetActionLabel,
} from "./components/original-client-shell-flow";
import type {
  DisplayEntity,
  DisplayLogLine,
  DisplayWorld,
  EntityKind,
  EntityMotionSnapshot,
  EquipmentActionRef,
  EquipmentSlot,
  ItemActionRef,
  MergeItemRef,
  MoveItemRef,
  PredictedPlayerMotion,
  SelectCharacterEntry,
  TranslateFn,
} from "./components/original-client-types";
import {
  CRYSTAL_MOVE_INPUT_INTERVAL_MS,
  EMPTY_VIEWPORT_MAP_SPRITES,
  EMPTY_VIEWPORT_OFFSET,
  GameSceneBackdrop,
  MAX_PREDICTED_PLAYER_LEAD_TILES,
  VIEWPORT_CELL_HEIGHT,
  VIEWPORT_CELL_WIDTH,
  VIEWPORT_ENTITY_LEFT_ORIGIN,
  VIEWPORT_ENTITY_TOP_ORIGIN,
  VIEWPORT_MOUSE_TILE_CENTER_X,
  VIEWPORT_MOUSE_TILE_CENTER_Y,
  VIEWPORT_OFFSET_X,
  VIEWPORT_OFFSET_Y,
  VIEWPORT_RANGE_X,
  VIEWPORT_RANGE_Y,
  buildViewportEntitySprite,
  buildViewportMapSprites,
  cameraMotionOffsetForEntity,
  entityAnimationStateForEntity,
  entityMotionOffsetForEntity,
  mapSpriteRenderPath,
  portraitFramesForCharacter,
  projectileProgress,
  rebaseViewportEntitiesToRenderPlayer,
  resolvedMapSpriteBlendMode,
  refreshEntityMotionSnapshots,
  rescueStalledSceneAssetImages,
  sceneAssetCandidateUrls,
  sceneAssetRuntimeStats,
  viewportDepthForCell,
  type ViewportMapSprites,
  type ViewportOffset,
} from "./components/original-client-scene-rendering";
import { OriginalClientSceneVisualLayers } from "./components/original-client-scene-visual-layers";
import {
  useSceneCameraMotionDriver,
  type ScenePresentationContext,
} from "./components/original-client-scene-camera-motion-driver";
import type { BevyPresentationEntityMotion } from "./components/original-client-presentation-pose";
import {
  OriginalClientSceneOverlays,
  type SceneChatBubble,
} from "./components/original-client-scene-overlays";
import { OriginalClientMobileControls } from "./components/original-client-mobile-controls";
import { OriginalClientGamepadControls } from "./components/original-client-gamepad-controls";
import { useOriginalClientDeviceProfile } from "./components/use-original-client-device-profile";
import {
  createCrystalAnimationWorldSeed,
  entityAnimationRuntimeFromWindow,
  resolveCrystalEntityAnimationPoses,
} from "./components/original-client-entity-animation-runtime";
import { WebGl2EntityAtlasLayer, type WebGl2EntityAtlasDebug } from "./components/webgl2-entity-atlas-layer";
import {
  WebGl2MapAtlasLayer,
  type MapStandaloneTileDraw,
  type MapTileDraw,
} from "./components/webgl2-map-atlas-layer";
import {
  buildMapTileDrawList,
  buildStandaloneMapTiles,
  type MapStandaloneTileImageSource,
} from "./components/original-client-scene-map-rendering";
import { type MapAtlasIndex, loadMapAtlasIndex } from "../lib/map-atlas-manifest";
import { isCompleteBevyMapImageFamilyResident } from "../lib/bevy-map-image-residency";
import {
  decodeStandaloneTilePixels,
  evictStandaloneTilePixels,
} from "../lib/standalone-tile-decode";

type HeldScenePointer = {
  button: 0 | 2;
  sceneX: number;
  sceneY: number;
  startedAt: number;
  dispatched: boolean;
  tileX?: number;
  tileY?: number;
};

type ChatBubbleRecord = {
  speaker: string;
  text: string;
  channel: string;
  firstSeenAt: number;
};

type DecodedStandaloneMapImage = {
  width: number;
  height: number;
  pixels: Uint8Array;
  sourceSignature: string;
};

type StandaloneMapImageDecodeRequest = {
  sourceSignature: string;
  generation: number;
};

type FailedStandaloneMapImage = {
  sourceSignature: string;
  retryAt: number;
};

type StandaloneMapImageResidency = {
  runtimeGeneration: number;
  keys: ReadonlySet<string>;
};

// How long a freshly-seen chat line floats over its speaker before it is dropped.
const CHAT_BUBBLE_TTL_MS = 6_000;
// Channels worth surfacing as over-head speech (local say-style chatter). Global/system channels
// such as trade, server, announcement and system stay in the chat log only.
const CHAT_BUBBLE_CHANNELS = new Set(["normal", "shout", "whisper", "group", "guild"]);

type BevyEntityAtlasRect = {
  key: string;
  x: number;
  y: number;
  width: number;
  height: number;
  /**
   * Index of the page (in BevyEntityAtlasSnapshot.pages) this rect lives on.
   * Absent ⇒ page 0, so single-page snapshots are unchanged.
   */
  pageIndex?: number;
};

/** One texture page of a (possibly multi-page) entity atlas. */
type BevyEntityAtlasPage = {
  key: string;
  width: number;
  height: number;
  imageUrl?: string;
  pixels?: Uint8Array;
  rectList: BevyEntityAtlasRect[];
};

type BevyEntityAtlasSnapshot = {
  key: string;
  sourceKey?: string;
  width: number;
  height: number;
  imageUrl?: string;
  rects: Record<string, BevyEntityAtlasRect>;
  rectList: BevyEntityAtlasRect[];
  pixels?: Uint8Array;
  /**
   * Multi-page atlases list every page here; the top-level
   * width/height/imageUrl/pixels/rectList mirror page 0 for backward
   * compatibility. Absent or length-1 ⇒ single-page (unchanged behaviour).
   */
  pages?: BevyEntityAtlasPage[];
};

type BevyEntityAtlasSource = {
  key: string;
  path: string;
  width: number;
  height: number;
};

type BevyEntityAtlasBudgetProfile = {
  tier: "low" | "medium" | "high";
  memoryEntries: number;
  memoryBytes: number;
  persistentEntries: number;
  persistentBytes: number;
  deviceMemoryGiB: number | null;
};

const BEVY_ENTITY_ATLAS_PADDING = 1;
const BEVY_ENTITY_ATLAS_INITIAL_WIDTH = 512;
const BEVY_ENTITY_ATLAS_MAX_SIZE = 4096;
const BEVY_ENTITY_ATLAS_BUDGET_PROFILE = resolveBevyEntityAtlasBudgetProfile();
const BEVY_ENTITY_ATLAS_CACHE_LIMIT = BEVY_ENTITY_ATLAS_BUDGET_PROFILE.memoryEntries;
const BEVY_ENTITY_ATLAS_MEMORY_BUDGET_BYTES = BEVY_ENTITY_ATLAS_BUDGET_PROFILE.memoryBytes;
const BEVY_ENTITY_ATLAS_MANIFEST_URL = "/bevy-entity-atlases/manifest.json";
const BEVY_ENTITY_ATLAS_IDB_NAME = "mir2-bevy-entity-atlas-cache";
const BEVY_ENTITY_ATLAS_IDB_STORE = "atlases";
const BEVY_ENTITY_ATLAS_IDB_VERSION = 1;
const BEVY_ENTITY_ATLAS_PERSISTENT_LIMIT = BEVY_ENTITY_ATLAS_BUDGET_PROFILE.persistentEntries;
const BEVY_ENTITY_ATLAS_PERSISTENT_BUDGET_BYTES = BEVY_ENTITY_ATLAS_BUDGET_PROFILE.persistentBytes;
const BEVY_ENTITY_ATLAS_CACHE_NAMESPACE = "bevy-entity-atlas-v2-full-pack";
const bevyEntityAtlasImageCache = new Map<string, Promise<HTMLImageElement>>();
const bevyEntityAtlasPrebuiltPixelsCache = new Map<string, Promise<Uint8Array | null>>();
let bevyEntityAtlasLatestSnapshot: BevyEntityAtlasSnapshot | null = null;
let bevyEntityAtlasManifestPromise: Promise<BevyEntityAtlasManifest | null> | null = null;
let bevyEntityAtlasDbPromise: Promise<IDBDatabase | null> | null = null;

// Resolve-side stats (prebuilt/persistent/live breakdown + build timing) that
// the in-memory residency manager cannot see. Merged with bevyAtlasResidency
// .stats() for the Alt+D debug panel.
const bevyEntityAtlasResolveStats = {
  builds: 0,
  prebuiltHits: 0,
  persistentHits: 0,
  persistentWrites: 0,
  lastBuildMs: 0,
  lastSource: null as "prebuilt" | "persistent" | "live" | null,
  lastPrebuiltKey: null as string | null,
  lastSourceCount: 0,
};

// The residency fetcher's resolveFn only receives a key; the acquire effect
// stashes the sources for that key here just before calling acquire(key).
const bevyEntityAtlasSourcesByKey = new Map<string, BevyEntityAtlasSource[]>();

// AtlasPagePayload (residency manager) <-> BevyEntityAtlasSnapshot (renderer).
// imageUrl-only prebuilt atlases carry no pixels; round-trip the empty buffer
// back to `undefined` so the renderer's `pixels?` checks behave as before.
function payloadToAtlasSnapshot(payload: AtlasPagePayload): BevyEntityAtlasSnapshot {
  return {
    key: payload.key,
    sourceKey: payload.sourceKey,
    width: payload.width,
    height: payload.height,
    imageUrl: payload.imageUrl,
    rects: bevyEntityAtlasRectMap(payload.rectList),
    rectList: payload.rectList,
    pixels: payload.pixels.byteLength > 0 ? payload.pixels : undefined,
    pages: payload.pages?.map((page) => ({
      key: page.key,
      width: page.width,
      height: page.height,
      imageUrl: page.imageUrl,
      pixels: page.pixels && page.pixels.byteLength > 0 ? page.pixels : undefined,
      rectList: page.rectList,
    })),
  };
}

function atlasSnapshotToPayload(atlas: BevyEntityAtlasSnapshot): AtlasPagePayload {
  return {
    key: atlas.key,
    sourceKey: atlas.sourceKey,
    width: atlas.width,
    height: atlas.height,
    imageUrl: atlas.imageUrl,
    rectList: atlas.rectList,
    pixels: atlas.pixels ?? new Uint8Array(0),
    pages: atlas.pages?.map((page) => ({
      key: page.key,
      width: page.width,
      height: page.height,
      imageUrl: page.imageUrl,
      pixels: page.pixels ?? new Uint8Array(0),
      rectList: page.rectList,
    })),
  };
}

// No-op persistent store: the residency manager owns the in-memory (hot) tier +
// LRU eviction, while resolveBevyEntityAtlasSnapshot keeps its existing IDB
// persistent tier (prebuilt -> persistent -> live). A real persistent store here
// would double-cache IDB, so persistence stays in resolve for now.
function createNullPersistentStore(): PersistentStore {
  return {
    get: async () => null,
    put: async () => {},
    delete: async () => {},
    listByAge: async () => [],
  };
}

// In-memory atlas residency — replaces the old bevyEntityAtlasCache Map + manual
// LRU loop. Same budget (24); the cold path stays in the resolve fetcher.
const bevyAtlasResidency = createAssetResidency({
  memoryBudget: BEVY_ENTITY_ATLAS_CACHE_LIMIT,
  persistentBudget: BEVY_ENTITY_ATLAS_PERSISTENT_LIMIT,
  memoryBudgetBytes: BEVY_ENTITY_ATLAS_MEMORY_BUDGET_BYTES,
  persistentBudgetBytes: BEVY_ENTITY_ATLAS_PERSISTENT_BUDGET_BYTES,
  persistent: createNullPersistentStore(),
  fetcher: createBrowserAtlasFetcher({
    resolveFn: async (key: string): Promise<AtlasPagePayload> => {
      const sources = bevyEntityAtlasSourcesByKey.get(key) ?? [];
      const startedAt = performance.now();
      const { atlas, source, prebuiltKey } = await resolveBevyEntityAtlasSnapshot(sources, key);
      bevyEntityAtlasResolveStats.lastBuildMs = Math.round(performance.now() - startedAt);
      bevyEntityAtlasResolveStats.lastSource = source;
      bevyEntityAtlasResolveStats.lastPrebuiltKey = prebuiltKey ?? null;
      bevyEntityAtlasResolveStats.lastSourceCount = sources.length;
      if (source === "live") {
        bevyEntityAtlasResolveStats.builds += 1;
      } else if (source === "prebuilt") {
        bevyEntityAtlasResolveStats.prebuiltHits += 1;
      } else {
        bevyEntityAtlasResolveStats.persistentHits += 1;
      }
      return atlasSnapshotToPayload(atlas);
    },
  }),
});

type BevyEntityAtlasManifest = {
  schemaVersion?: number;
  generatedAt?: string;
  atlases?: PrebuiltBevyEntityAtlasRecord[];
};

type PrebuiltBevyEntityAtlasRecord = {
  key: string;
  label?: string;
  width: number;
  height: number;
  sourceCount?: number;
  imageBytes?: number;
  rgbaBytes?: number;
  roots?: string[];
  imageUrl?: string;
  pixelsUrl?: string;
  rects: BevyEntityAtlasRect[];
  /** Multi-page atlases describe each texture page here (manifest schemaVersion≥2). */
  pages?: PrebuiltBevyEntityAtlasPage[];
};

type PrebuiltBevyEntityAtlasPage = {
  imageFile?: string;
  imageUrl?: string;
  width: number;
  height: number;
  sha256?: string;
};

type BevyEntityAtlasResolveResult = {
  atlas: BevyEntityAtlasSnapshot;
  source: "prebuilt" | "persistent" | "live";
  prebuiltKey?: string | null;
};

type KeyboardMoveDirection = "up" | "down" | "left" | "right";

const KEYBOARD_MOVE_KEYS = new Set(["w", "a", "s", "d", "arrowup", "arrowdown", "arrowleft", "arrowright"]);

function keyboardMoveDirectionForKey(key: string): KeyboardMoveDirection | null {
  switch (key.toLowerCase()) {
    case "w":
    case "arrowup":
      return "up";
    case "s":
    case "arrowdown":
      return "down";
    case "a":
    case "arrowleft":
      return "left";
    case "d":
    case "arrowright":
      return "right";
    default:
      return null;
  }
}

function crystalDirectionFromKeyboardVector(dx: number, dy: number): string | null {
  if (dx === 0 && dy < 0) return "Up";
  if (dx > 0 && dy < 0) return "UpRight";
  if (dx > 0 && dy === 0) return "Right";
  if (dx > 0 && dy > 0) return "DownRight";
  if (dx === 0 && dy > 0) return "Down";
  if (dx < 0 && dy > 0) return "DownLeft";
  if (dx < 0 && dy === 0) return "Left";
  if (dx < 0 && dy < 0) return "UpLeft";
  return null;
}

function isKeyboardMoveKey(key: string) {
  return KEYBOARD_MOVE_KEYS.has(key.toLowerCase());
}

function keyboardInputTargetIsEditable(target: EventTarget | null) {
  return (
    target instanceof HTMLInputElement ||
    target instanceof HTMLTextAreaElement ||
    target instanceof HTMLSelectElement ||
    (target instanceof HTMLElement && target.isContentEditable)
  );
}

export function OriginalClientShell({
  language,
  screen,
  runtimePhase,
  runtimeMessage,
  wsState,
  reconnectStatus,
  world,
  worldStore,
  selectorHud,
  player,
  predictedPlayerPosition,
  getLivePlayerRenderPosition,
  selectedEntity,
  sortedEntities,
  viewportEntities: sourceViewportEntities,
  viewportTiles,
  sceneInteractionReady,
  bevyEntityRendererReady,
  bevyRuntimeBackend,
  bevyMapRuntimeGeneration,
  bevyMapRuntimeReady,
  bevyMapPresentedImageKeys,
  bevyMapImageResidencyVersion,
  onSceneAssetReadinessChange,
  onBevyEntityRenderStateChange,
  onBevyMapRenderStateChange,
  onBevyMapImagesEvicted,
  logs,
  accountId,
  password,
  chatMessage,
  loginBusy,
  loginError,
  suiWallets,
  walletPickerOpen,
  dubheWalletUrl,
  characters,
  selectedCharacterIndex,
  showInventory,
  showCharacter,
  activeInventoryTab,
  activeCharacterTab,
  storageServiceOpenVersion,
  onLanguageChange,
  onAccountIdChange,
  onPasswordChange,
  onChatMessageChange,
  onCreateAccount,
  onSubmitLogin,
  onPasskeyLogin,
  onWalletPickerToggle,
  onWalletLogin,
  onQuickEnter,
  onResetClient,
  onExitSelect,
  onSendChat,
  onRequestTrade,
  onRentExpandedStorage,
  onLogout,
  onCreateCharacter,
  onDeleteCharacter,
  onUseItem,
  onDropItem,
  onEquipItem,
  onRemoveItem,
  onMoveItem,
  onMergeItem,
  onSplitItem,
  onStoreItem,
  onTakeBackItem,
  onUnlockStorage,
  onSetStoragePassword,
  onRemoveStoragePassword,
  onSellItem,
  onDropGold,
  onRepairItem,
  onSpecialRepairItem,
  onCastSkill,
  onTransferMap,
  onClaimMail,
  onDeleteMail,
  onBuyGameShopItem,
  onRunStage5Command,
  onSendClientCommand,
  transferOptions,
  onToggleCharacter,
  onToggleInventory,
  onCloseCharacter,
  onCloseInventory,
  onOpenCharacterTab,
  onOpenInventoryTab,
  onViewportTileClick,
  onViewportTileSecondaryAction,
  onViewportTileStepClick,
  onViewportTileStepSecondaryAction,
  onViewportDirectionStep,
  onViewportDirectionIntent,
  onViewportDirectionStop,
  onPickGroundDrop,
  onSelectEntity,
  onActivateEntity,
  onApproachTarget,
  onPrimaryTargetAction,
  onSelectNpcDialogTarget,
  onSubmitNpcInput,
  onSelectCharacter,
  onEnterWorld,
  targetDistance,
  entityKindClassName,
}: OriginalClientShellProps) {
  // Memoize t and locale so stable references are passed to memo'd child components
  // (GameUiScene, LoginOverlay, SelectOverlay, OriginalClientMobileControls). Without
  // this, buildTranslator returns a new function on every render — invalidating the
  // memo on each 30 Hz motionNow tick. Language changes only on user action (rare).
  const t = useMemo(() => buildTranslator(language), [language]);
  const locale = useMemo(() => languageLocale(language), [language]);
  const runtimePhaseLabel = formatRuntimePhase(language, runtimePhase);
  const runtimeMessageLabel = formatRuntimeMessage(language, runtimeMessage);
  // Shown over the (black) stage while the map tiles preload after entering the world. The
  // condition reuses sceneInteractionReady, which preloadSceneAssetUrls() flips true on
  // success or within its 5s partial-ready timeout, so this overlay can never stick.
  const sceneLoadingLabel = t(
    "scene.loadingMap",
    [],
    language === "zh-CN"
      ? "地图加载中…"
      : language === "es"
        ? "Cargando mapa…"
        : language === "pt-BR"
          ? "Carregando mapa…"
          : "Loading map…",
  );
  const [loginTransitionFrame, setLoginTransitionFrame] = useState<number | null>(null);
  const [entityAnimationWorldSeed] = useState(createCrystalAnimationWorldSeed);
  const [sceneSpriteFrameIndex, setSceneSpriteFrameIndex] = useState(0);
  const [motionNow, setMotionNow] = useState(0);
  const [sceneSpriteLibraries, setSceneSpriteLibraries] = useState<Record<string, OriginalSceneSpriteLibraryMeta>>({});
  const clientProfile = useOriginalClientDeviceProfile();
  const [stagePresentation, setStagePresentation] = useState({
    scale: 1,
    left: 0,
    top: 0,
  });
  const [bevyEntityAtlas, setBevyEntityAtlas] = useState<BevyEntityAtlasSnapshot | null>(null);
  const [bevyLocalSelfMotion, setBevyLocalSelfMotion] =
    useState<BevyPresentationEntityMotion | null>(null);
  const [webGl2EntityTextureReadyKey, setWebGl2EntityTextureReadyKey] = useState<string | null>(null);
  const [webGl2EntityAtlasFailedKey, setWebGl2EntityAtlasFailedKey] = useState<string | null>(null);
  const previousScreenRef = useRef<ClientScreen>(screen);
  const bevyEntityAtlasRequestRef = useRef<{ key: string; requestId: number } | null>(null);
  const reconnectSeconds =
    reconnectStatus.nextAttemptAt === null
      ? null
      : Math.max(1, Math.ceil((reconnectStatus.nextAttemptAt - motionNow) / 1000));
  const reconnectMessage =
    reconnectStatus.mode === "scheduled"
      ? t("ui.reconnectScheduled", [reconnectSeconds ?? 1], "Connection lost. Reconnecting in {0}s.")
      : reconnectStatus.mode === "connecting"
        ? t("ui.reconnectConnecting", [], "Reconnecting...")
        : reconnectStatus.mode === "resuming"
          ? t("ui.reconnectRestoring", [], "Restoring character...")
          : reconnectStatus.mode === "failed"
            ? t("ui.reconnectFailed", [], "Connection lost. Please log in again.")
            : null;
  const missingSceneSpriteLibrariesRef = useRef<Set<string>>(new Set());
  // Sprite-library keys whose load is currently in flight — prevents duplicate
  // fetches when the load effect re-runs (it re-runs on every world.entities change).
  const sceneSpriteLibraryInFlightRef = useRef<Set<string>>(new Set());
  const entityMotionSnapshotsRef = useRef<Record<string, EntityMotionSnapshot>>({});
  const bevyMapRenderRevisionRef = useRef(0);
  const submittedPresentationContextRef = useRef<ScenePresentationContext>({
    mapRevision: null,
    mapCenter: null,
    entityCenter: null,
  });
  const lastSceneMapSubmissionRef = useRef<{
    state: BevyMapRenderState | null;
    imageResidencyVersion: number;
    runtimeGeneration: number;
    runtimeReady: boolean;
    submitted: boolean;
    revision: number | null;
    center: { x: number; y: number } | null;
  }>({
    state: null,
    imageResidencyVersion: -1,
    runtimeGeneration: -1,
    runtimeReady: false,
    submitted: false,
    revision: null,
    center: null,
  });
  useEffect(() => {
    submittedPresentationContextRef.current = {
      mapRevision: null,
      mapCenter: null,
      entityCenter: null,
    };
  }, [bevyMapRuntimeGeneration]);
  // Motion-clock cadence (ms between setMotionNow). 30 ms (~33 Hz) drives smooth JS
  // motion on the default/DOM path; in the imperative path (Bevy interpolates motion +
  // the scene-motion driver tracks DOM overlays) it drops to ~10 Hz — just enough for
  // the reconnect countdown + bubble/floater/projectile expiry — which is the perf win.
  const motionClockIntervalMsRef = useRef(30);
  // Over-head chat bubble bookkeeping. Keyed by speaker name (the only entity reference a chat log
  // line carries), each record remembers when the line first appeared so bubbles can expire on the
  // shell's existing motion clock without any dedicated timer.
  const chatBubbleStateRef = useRef<Map<string, ChatBubbleRecord>>(new Map());
  const stageFrameRef = useRef<HTMLDivElement | null>(null);
  const heldScenePointerRef = useRef<HeldScenePointer | null>(null);
  const heldKeyboardMoveKeysRef = useRef<Set<KeyboardMoveDirection>>(new Set());
  const heldKeyboardRunModeRef = useRef(false);
  const latestMoveInputRef = useRef<{
    screen: ClientScreen;
    player: DisplayEntity | null;
    renderPlayer: DisplayEntity | null;
    playerCameraMotionOffset: ViewportOffset;
  }>({
    screen,
    player,
    renderPlayer: player,
    playerCameraMotionOffset: EMPTY_VIEWPORT_OFFSET,
  });
  const stageScaleStyle = useMemo(() => {
    const touchControlDeck = calculateMir2TouchControlDeck(stagePresentation);
    return {
      "--mir-stage-scale": stagePresentation.scale,
      "--mir-stage-left": `${stagePresentation.left}px`,
      "--mir-stage-top": `${stagePresentation.top}px`,
      "--mir-control-deck-left": `${touchControlDeck.left}px`,
      "--mir-control-deck-width": `${touchControlDeck.width}px`,
    } as CSSProperties;
  }, [stagePresentation]);
  const sceneAssetReadinessCallbackRef = useRef(onSceneAssetReadinessChange);
  sceneAssetReadinessCallbackRef.current = onSceneAssetReadinessChange;

  const selectedCharacter = characters[selectedCharacterIndex] ?? null;
  const selectedPortraitFrames = selectedCharacter ? portraitFramesForCharacter(selectedCharacter) : [];
  const loginBackgroundFrame =
    ORIGINAL_UI.login.backgroundFrames[LOGIN_STATIC_BACKGROUND_FRAME] ?? ORIGINAL_UI.login.backgroundFrames[0];
  const loginTransitionBackground =
    screen !== "select" || loginTransitionFrame === null
      ? null
      : ORIGINAL_UI.login.backgroundFrames[
          Math.min(loginTransitionFrame, ORIGINAL_UI.login.backgroundFrames.length - 1)
        ] ?? loginBackgroundFrame;
  const loginTransitionAudioActive = screen === "select" && loginTransitionFrame !== null;

  useEffect(() => {
    setMotionNow(Date.now());
  }, []);

  useEffect(() => {
    const renderGameToText = () =>
      JSON.stringify({
        coordinateSystem: "tile coordinates; origin top-left; x right; y down",
        screen,
        layout: clientProfile.layout,
        input: clientProfile.input,
        sceneInteractionReady,
        player: player
          ? {
              objectId: player.objectId,
              x: player.x,
              y: player.y,
              direction: player.direction ?? null,
              hp: player.hp ?? null,
              maxHp: player.maxHp ?? null,
            }
          : null,
        selectedEntity: selectedEntity
          ? {
              objectId: selectedEntity.objectId,
              kind: selectedEntity.kind,
              name: selectedEntity.name,
              x: selectedEntity.x,
              y: selectedEntity.y,
              dead: Boolean(selectedEntity.dead),
            }
          : null,
        visibleEntities: sourceViewportEntities.slice(0, 32).map((entity) => ({
          objectId: entity.objectId,
          kind: entity.kind,
          name: entity.name,
          x: entity.x,
          y: entity.y,
          dead: Boolean(entity.dead),
        })),
        groundDrops: world.groundDrops.slice(0, 16).map((drop) => ({
          objectId: drop.objectId,
          name: drop.name,
          x: drop.x,
          y: drop.y,
        })),
        panels: {
          inventory: showInventory,
          character: showCharacter,
        },
      });
    const gameWindow = window as typeof window & {
      render_game_to_text?: () => string;
    };
    gameWindow.render_game_to_text = renderGameToText;
    return () => {
      if (gameWindow.render_game_to_text === renderGameToText) {
        delete gameWindow.render_game_to_text;
      }
    };
  }, [
    clientProfile.input,
    clientProfile.layout,
    player,
    sceneInteractionReady,
    screen,
    selectedEntity,
    showCharacter,
    showInventory,
    sourceViewportEntities,
    world.groundDrops,
  ]);

  // Announce that #mir2-web3-canvas is mounted so the Bevy runtime can boot against it.
  // This shell is lazily mounted (dynamic, ssr:false); the runtime attaches to this canvas
  // on boot, so booting before it exists panics bevy_winit ("Cannot find element"). This
  // mount effect runs after the canvas is committed to the DOM.
  useEffect(() => {
    if (typeof window === "undefined") return;
    const w = window as Window & { __mir2BevyCanvasReady?: boolean };
    w.__mir2BevyCanvasReady = true;
    window.dispatchEvent(new Event("mir2:bevy-canvas-ready"));
    return () => {
      w.__mir2BevyCanvasReady = false;
    };
  }, []);

  useEffect(() => {
    if (screen === "game") {
      stageFrameRef.current?.focus({ preventScroll: true });
    }
  }, [screen]);

  const syncMusic = useCallback((src: string | null) => {
    setOriginalMusic(src);
  }, []);

  const playLoginEffect = useCallback(() => {
    playOriginalSoundPath(ORIGINAL_AUDIO.loginEffect, ORIGINAL_EFFECT_VOLUME);
  }, []);

  useEffect(() => {
    const handleUserAudioGesture = () => unlockOriginalAudio();

    window.addEventListener("pointerdown", handleUserAudioGesture, true);
    window.addEventListener("keydown", handleUserAudioGesture, true);

    return () => {
      window.removeEventListener("pointerdown", handleUserAudioGesture, true);
      window.removeEventListener("keydown", handleUserAudioGesture, true);
      stopOriginalAudio();
    };
  }, []);

  useEffect(() => {
    const previousScreen = previousScreenRef.current;
    previousScreenRef.current = screen;

    if (
      previousScreen !== screen &&
      (previousScreen === "game" || screen === "game")
    ) {
      entityAnimationRuntimeFromWindow()?.resetMir2EntityAnimations?.();
    }

    if (previousScreen === "login" && screen === "select") {
      setLoginTransitionFrame(0);
      return;
    }

    if (screen !== "select") {
      setLoginTransitionFrame(null);
    }
  }, [screen]);

  useEffect(() => {
    if (loginTransitionFrame === null) {
      return;
    }

    const timer = window.setTimeout(() => {
      setLoginTransitionFrame((current) => {
        if (current === null) {
          return null;
        }

        const nextFrame = current + 1;
        return nextFrame >= ORIGINAL_UI.login.backgroundFrames.length ? null : nextFrame;
      });
    }, LOGIN_TRANSITION_FRAME_MS);

    return () => window.clearTimeout(timer);
  }, [loginTransitionFrame]);

  useEffect(() => {
    syncMusic(desiredMusicForScreen(screen, loginTransitionAudioActive));
  }, [loginTransitionAudioActive, screen, syncMusic]);

  useEffect(() => {
    if (loginTransitionFrame === 0) {
      playLoginEffect();
    }
  }, [loginTransitionFrame, playLoginEffect]);

  // Portrait idle animation moved into SelectOverlay so its 120ms tick re-renders
  // only that overlay, not this ~3000-line shell.

  useEffect(() => {
    if (screen !== "game") {
      return;
    }

    const timer = window.setInterval(() => {
      setSceneSpriteFrameIndex((current) => current + 1);
    }, 100);

    return () => window.clearInterval(timer);
  }, [screen]);

  useEffect(() => {
    if (screen !== "game") {
      return;
    }

    function suppressBrowserContextMenu(event: globalThis.MouseEvent) {
      const target = event.target;
      if (target instanceof Element && target.closest(".client-stage-frame")) {
        event.preventDefault();
      }
    }

    function suppressRightMouseDefault(event: globalThis.MouseEvent) {
      if (event.button !== 2) {
        return;
      }

      const target = event.target;
      if (target instanceof Element && target.closest(".client-stage-frame")) {
        event.preventDefault();
      }
    }

    window.addEventListener("contextmenu", suppressBrowserContextMenu, true);
    window.addEventListener("mousedown", suppressRightMouseDefault, true);

    return () => {
      window.removeEventListener("contextmenu", suppressBrowserContextMenu, true);
      window.removeEventListener("mousedown", suppressRightMouseDefault, true);
    };
  }, [screen]);

  useEffect(() => {
    if (screen !== "game") {
      return;
    }

    function handleShortcutKey(event: KeyboardEvent) {
      if (event.altKey || event.ctrlKey || event.metaKey || event.shiftKey) {
        return;
      }

      if (keyboardInputTargetIsEditable(event.target)) {
        return;
      }

      if (isKeyboardMoveKey(event.key)) {
        return;
      }

      if (selectedEntity) {
        if (event.key === " " || event.key === "Enter") {
          event.preventDefault();
          onPrimaryTargetAction();
          return;
        }

        if (!selectedEntity.dead && event.key.toLowerCase() === "f") {
          event.preventDefault();
          onApproachTarget();
          return;
        }
      }

      // F1–F8 cast the skill in that primary skill-bar slot. Crystal maps
      // KeybindOptions.Bar1Skill1..8 to Keys.F1..F8 (KeyBindSettings.cs:242) and
      // stores the slot in each spell's `Magic.Key` (mirrored onto `skill.hotkey`).
      // Prefer an explicit binding; otherwise fall back to the spell's position in
      // the known-skills list (the order shown in the character window's spell tab),
      // so the bar is usable before any slots are explicitly assigned.
      const skillBarMatch = /^F([1-8])$/.exec(event.key);
      if (skillBarMatch) {
        const slot = Number.parseInt(skillBarMatch[1], 10);
        const skill =
          world.knownSkills.find((entry) => entry.hotkey === slot) ??
          world.knownSkills[slot - 1] ??
          null;
        if (skill) {
          event.preventDefault();
          onCastSkill(skill.key);
        }
        return;
      }

      const slotIndex = Number.parseInt(event.key, 10);
      if (!Number.isFinite(slotIndex) || slotIndex < 1 || slotIndex > 6) {
        return;
      }

      const item = world.beltItems.find((entry) => entry.slot === slotIndex - 1);
      if (!item) {
        return;
      }

      event.preventDefault();
      onUseItem({
        key: item.key,
        uniqueId: item.uniqueId,
        slot: item.slot,
        container: item.container,
      });
    }

    window.addEventListener("keydown", handleShortcutKey);
    return () => window.removeEventListener("keydown", handleShortcutKey);
  }, [
    screen,
    selectedEntity,
    world.beltItems,
    world.knownSkills,
    onApproachTarget,
    onPrimaryTargetAction,
    onCastSkill,
    onUseItem,
  ]);

  function dispatchKeyboardMoveInput(source: "edge" | "held" = "held") {
    const latest = latestMoveInputRef.current;
    if (latest.screen !== "game") return;
    if (!sceneInteractionReady) return;
    if (!latest.renderPlayer && !latest.player) return;

    const heldKeys = heldKeyboardMoveKeysRef.current;
    let dx = 0;
    let dy = 0;
    if (heldKeys.has("left")) dx -= 1;
    if (heldKeys.has("right")) dx += 1;
    if (heldKeys.has("up")) dy -= 1;
    if (heldKeys.has("down")) dy += 1;
    if (dx === 0 && dy === 0) return;

    const direction = crystalDirectionFromKeyboardVector(dx, dy);
    if (!direction) return;
    onViewportDirectionIntent(direction, heldKeyboardRunModeRef.current ? "run" : "walk", {
      discrete: source === "edge",
    });
  }

  useEffect(() => {
    if (screen !== "game") {
      onViewportDirectionStop();
      heldKeyboardMoveKeysRef.current.clear();
      heldKeyboardRunModeRef.current = false;
      return;
    }

    function handleKeyboardMoveDown(event: KeyboardEvent) {
      if (event.altKey || event.ctrlKey || event.metaKey || keyboardInputTargetIsEditable(event.target)) {
        return;
      }

      if (event.key === "Shift") {
        if (!sceneInteractionReady) {
          return;
        }
        heldKeyboardRunModeRef.current = true;
        dispatchKeyboardMoveInput("held");
        return;
      }

      const direction = keyboardMoveDirectionForKey(event.key);
      if (!direction) {
        return;
      }

      event.preventDefault();
      if (!sceneInteractionReady) {
        return;
      }
      heldKeyboardRunModeRef.current = event.shiftKey || heldKeyboardRunModeRef.current;
      const alreadyHeld = heldKeyboardMoveKeysRef.current.has(direction);
      heldKeyboardMoveKeysRef.current.add(direction);
      if (!alreadyHeld && !event.repeat) {
        dispatchKeyboardMoveInput("edge");
      }
    }

    function handleKeyboardMoveUp(event: KeyboardEvent) {
      if (event.key === "Shift") {
        heldKeyboardRunModeRef.current = false;
        if (heldKeyboardMoveKeysRef.current.size === 0) {
          onViewportDirectionStop();
          return;
        }
        dispatchKeyboardMoveInput("held");
        return;
      }

      const direction = keyboardMoveDirectionForKey(event.key);
      if (direction) {
        event.preventDefault();
        heldKeyboardMoveKeysRef.current.delete(direction);
        heldKeyboardRunModeRef.current = event.shiftKey || heldKeyboardRunModeRef.current;
        if (heldKeyboardMoveKeysRef.current.size === 0) {
          onViewportDirectionStop();
          return;
        }
        dispatchKeyboardMoveInput("edge");
      }
    }

    const timer = window.setInterval(() => dispatchKeyboardMoveInput("held"), CRYSTAL_MOVE_INPUT_INTERVAL_MS);
    const stop = () => {
      heldKeyboardMoveKeysRef.current.clear();
      heldKeyboardRunModeRef.current = false;
      onViewportDirectionStop();
    };

    window.addEventListener("keydown", handleKeyboardMoveDown);
    window.addEventListener("keyup", handleKeyboardMoveUp);
    window.addEventListener("blur", stop);

    return () => {
      window.clearInterval(timer);
      window.removeEventListener("keydown", handleKeyboardMoveDown);
      window.removeEventListener("keyup", handleKeyboardMoveUp);
      window.removeEventListener("blur", stop);
    };
  }, [screen, sceneInteractionReady, onViewportDirectionIntent, onViewportDirectionStop]);

  const lastMotionNowRef = useRef(0);
  useEffect(() => {
    if (screen !== "game") {
      return;
    }

    const now = Date.now();
    lastMotionNowRef.current = now;
    setMotionNow(now);
    let animationFrame = 0;
    // Fallback 100ms timer keeps the reconnect countdown and bubble expiry ticking
    // when rAF is suppressed (background tab, etc.).
    const fallbackTimer = window.setInterval(() => {
      const t = Date.now();
      lastMotionNowRef.current = t;
      setMotionNow(t);
    }, 100);
    // Throttle the rAF to an adaptive cadence (motionClockIntervalMsRef): ~33 ms in the
    // DOM-entity fallback (where this clock drives the JS motion interpolation), or
    // ~100 ms in the imperative path (Bevy interpolates motion at display Hz + the
    // scene-motion driver tracks DOM overlays, so this clock only needs to advance the
    // reconnect countdown + bubble/floater/projectile expiry). Dropping 33→10 Hz there
    // is the bulk of the scene-render perf win (the React tree stops re-creating 30×/s).
    const updateMotionClock = () => {
      const t = Date.now();
      if (t - lastMotionNowRef.current >= motionClockIntervalMsRef.current) {
        lastMotionNowRef.current = t;
        setMotionNow(t);
      }
      animationFrame = window.requestAnimationFrame(updateMotionClock);
    };
    animationFrame = window.requestAnimationFrame(updateMotionClock);

    return () => {
      window.clearInterval(fallbackTimer);
      window.cancelAnimationFrame(animationFrame);
    };
  }, [screen]);

  useLayoutEffect(() => {
    const updateStageScale = () => {
      const viewport = window.visualViewport;
      const cssWidth = Math.max(1, viewport?.width ?? window.innerWidth);
      const cssHeight = Math.max(1, viewport?.height ?? window.innerHeight);
      const next = calculateMir2StagePresentation({
        cssWidth,
        cssHeight,
        devicePixelRatio: window.devicePixelRatio || 1,
        layout: clientProfile.layout,
        input: clientProfile.input,
        screen,
      });
      setStagePresentation((current) =>
        current.scale === next.scale && current.left === next.left && current.top === next.top
          ? current
          : next,
      );
    };

    updateStageScale();
    window.addEventListener("resize", updateStageScale);
    window.visualViewport?.addEventListener("resize", updateStageScale);

    return () => {
      window.removeEventListener("resize", updateStageScale);
      window.visualViewport?.removeEventListener("resize", updateStageScale);
    };
  }, [clientProfile.input, clientProfile.layout, screen]);

  const presentationOwnsPlayerInterpolation =
    screen === "game" &&
    bevyEntityRendererReady &&
    bevyMapRuntimeReady &&
    Boolean(bevyRuntimeBackend) &&
    runtimePhase !== "dom-only" &&
    runtimePhase !== "boot-error" &&
    shouldUseBevyEntityRenderer() &&
    BEVY_SELF_CAMERA_REQUESTED &&
    BEVY_ENTITY_INTERP_REQUESTED &&
    BEVY_PRESENTATION_POSE_REQUESTED &&
    BEVY_LOCAL_MOTION_REQUESTED;
  const livePlayerRenderPosition =
    getLivePlayerRenderPosition?.({
      presentationOwnsInterpolation: presentationOwnsPlayerInterpolation,
    }) ?? predictedPlayerPosition;
  const renderPlayer =
    player &&
    livePlayerRenderPosition &&
    Math.max(Math.abs(player.x - livePlayerRenderPosition.x), Math.abs(player.y - livePlayerRenderPosition.y)) <=
      MAX_PREDICTED_PLAYER_LEAD_TILES
      ? {
          ...player,
          ...livePlayerRenderPosition,
          direction: livePlayerRenderPosition.direction ?? player.direction,
        }
      : player;
  const viewportEntities = useMemo(
    () => rebaseViewportEntitiesToRenderPlayer(sourceViewportEntities, renderPlayer),
    [
      sourceViewportEntities,
      renderPlayer?.objectId,
      renderPlayer?.x,
      renderPlayer?.y,
      renderPlayer?.direction,
      renderPlayer?.movementAnimation,
      renderPlayer?.movementStartedAt,
      renderPlayer?.movementUntil,
    ],
  );

  const desiredSceneSpriteLibraryKeys = useMemo(() => {
    const libraries = new Set<string>();
    if (screen !== "game") {
      return [];
    }

    for (const entity of world.entities) {
      if (entity.sprite?.bodyLibrary) {
        libraries.add(normalizeSceneSpriteLibraryKey(entity.sprite.bodyLibrary));
      }
      if (entity.sprite?.hairLibrary) {
        libraries.add(normalizeSceneSpriteLibraryKey(entity.sprite.hairLibrary));
      }
      if (entity.sprite?.weaponLibrary) {
        libraries.add(normalizeSceneSpriteLibraryKey(entity.sprite.weaponLibrary));
      }
      if (entity.sprite?.mountLibrary) {
        libraries.add(normalizeSceneSpriteLibraryKey(entity.sprite.mountLibrary));
      }
      if (entity.sprite?.altBodyLibrary) {
        libraries.add(normalizeSceneSpriteLibraryKey(entity.sprite.altBodyLibrary));
      }
      if (entity.sprite?.altHairLibrary) {
        libraries.add(normalizeSceneSpriteLibraryKey(entity.sprite.altHairLibrary));
      }
      if (entity.sprite?.altWeaponLibrary) {
        libraries.add(normalizeSceneSpriteLibraryKey(entity.sprite.altWeaponLibrary));
      }
      if (entity.sprite?.altWeaponLibrarySecondary) {
        libraries.add(normalizeSceneSpriteLibraryKey(entity.sprite.altWeaponLibrarySecondary));
      }
      if (entity.kind === "selfPlayer" || entity.kind === "player") {
        libraries.add("CArmour/00");
        libraries.add("CHair/00");
        libraries.add("CWeapon/00");
        if (entity.classKey === "archer") {
          libraries.add("ARArmour/00");
          libraries.add("ARHair/00");
          libraries.add("ARWeapon/00");
          libraries.add("ARWeapon/00 S");
        }
        if (entity.classKey === "assassin") {
          libraries.add("AArmour/00");
          libraries.add("AHair/00");
          libraries.add("AWeapon/00 L");
          libraries.add("AWeapon/00 R");
        }
      }
    }

    return [...libraries].sort();
  }, [screen, world.entities]);
  const desiredSceneSpriteLibraryKey = desiredSceneSpriteLibraryKeys.join("|");
  const pendingSceneSpriteLibraryKeys = desiredSceneSpriteLibraryKeys.filter(
    (libraryKey) =>
      !(libraryKey in sceneSpriteLibraries) &&
      originalSceneSpriteLibraryExists(libraryKey) &&
      !missingSceneSpriteLibrariesRef.current.has(libraryKey),
  );
  const sceneSpriteLibrariesReady = pendingSceneSpriteLibraryKeys.length === 0;

  useEffect(() => {
    if (screen !== "game") {
      return;
    }

    const missingLibraries = desiredSceneSpriteLibraryKeys.filter(
      (libraryKey) => !(libraryKey in sceneSpriteLibraries),
    );
    for (const libraryKey of missingLibraries) {
      if (!originalSceneSpriteLibraryExists(libraryKey)) {
        missingSceneSpriteLibrariesRef.current.add(libraryKey);
      }
    }
    const toLoad = missingLibraries.filter(
      (libraryKey) =>
        originalSceneSpriteLibraryExists(libraryKey) &&
        !missingSceneSpriteLibrariesRef.current.has(libraryKey) &&
        !sceneSpriteLibraryInFlightRef.current.has(libraryKey),
    );
    if (!toLoad.length) {
      return;
    }

    // Load each library INDEPENDENTLY and ALWAYS cache a completed load. The old
    // Promise.all + `disposed` guard abandoned in-flight loads whenever this effect
    // re-ran — and it re-ran on every world.entities change (e.g. an NPC moving),
    // so on a fast gateway the loads were perpetually dropped before
    // setSceneSpriteLibraries, sceneSpriteLibrariesReady never flipped, and the
    // "Loading map…" overlay hung forever. A loaded sprite library is always valid
    // to cache; the in-flight set prevents duplicate fetches across re-runs.
    for (const libraryKey of toLoad) {
      sceneSpriteLibraryInFlightRef.current.add(libraryKey);
      void loadOriginalSceneSpriteLibrary(libraryKey)
        .then((libraryMeta) => {
          setSceneSpriteLibraries((current) =>
            libraryKey in current ? current : { ...current, [libraryKey]: libraryMeta },
          );
        })
        .catch(() => {
          // Permanently unavailable at runtime (e.g. 404 for a source-only library):
          // record it so pendingSceneSpriteLibraryKeys excludes it and readiness can
          // resolve, then nudge a re-render so the calculation recomputes.
          missingSceneSpriteLibrariesRef.current.add(libraryKey);
          setSceneSpriteLibraries((current) => ({ ...current }));
        })
        .finally(() => {
          sceneSpriteLibraryInFlightRef.current.delete(libraryKey);
        });
    }
  }, [desiredSceneSpriteLibraryKey, sceneSpriteLibraries, screen]);

  const sceneNow = motionNow;
  entityMotionSnapshotsRef.current = refreshEntityMotionSnapshots(
    screen,
    world.entities,
    renderPlayer,
    entityMotionSnapshotsRef.current,
    sceneNow,
  );
  const playerCameraMotionOffset = renderPlayer
    ? cameraMotionOffsetForEntity(renderPlayer, entityMotionSnapshotsRef.current, motionNow)
    : EMPTY_VIEWPORT_OFFSET;
  if (isSceneMotionDebugMode && typeof window !== "undefined") {
    (
      window as typeof window & {
        __mir2SceneMotionDebug?: unknown;
      }
    ).__mir2SceneMotionDebug = {
      motionNow,
      renderPlayer: renderPlayer
        ? {
            objectId: renderPlayer.objectId,
            x: renderPlayer.x,
            y: renderPlayer.y,
            direction: renderPlayer.direction,
            movementAnimation: renderPlayer.movementAnimation,
            movementStartedAt: renderPlayer.movementStartedAt,
            movementUntil: renderPlayer.movementUntil,
          }
        : null,
      playerCameraMotionOffset,
      playerMotionSnapshot: renderPlayer
        ? entityMotionSnapshotsRef.current[renderPlayer.objectId] ?? null
        : null,
    };
  }
  useLayoutEffect(() => {
    // Input must observe the same player/camera state that React has committed.
    // Writing this ref during render can expose an uncommitted concurrent tree.
    latestMoveInputRef.current = {
      screen,
      player,
      renderPlayer,
      playerCameraMotionOffset,
    };
  }, [screen, player, renderPlayer, playerCameraMotionOffset]);
  // Sprite *frame data* (which body/hair/weapon frame to draw) only changes on entity updates from
  // the server, sprite-library loads, or the 120ms animation tick — NOT on the 60fps motion clock.
  // Memoising it off `sceneSpriteFrameIndex` instead of `motionNow` stops every monster/NPC sprite
  // from being rebuilt 60×/sec (the smooth per-frame pixel offset is applied downstream from
  // `motionNow` on the wrapper element). Transient attack/struck frames quantise to the 120ms tick,
  // which is imperceptible. This is the bulk of the "running is janky / NPCs flicker" fix: stable
  // sprite refs let the memoised <EntitySpriteLayers> skip its per-frame DOM restyle.
  const viewportEntitySprites = useMemo(() => {
    if (!player) {
      return [];
    }
    const spriteNow = Date.now();
    const snapshots = entityMotionSnapshotsRef.current;
    const animationInputs = viewportEntities.map((entity) => {
      const motionSnapshot = snapshots[entity.objectId];
      const presentationMotion = entity.objectId === player.objectId ? bevyLocalSelfMotion : null;
      const packetAnimationState = entityAnimationStateForEntity(entity, snapshots, spriteNow);
      // A latched local movement pose must not mask a newer attack/struck/death
      // packet. It only owns the sprite phase while the semantic state is still
      // standing or moving.
      const activePresentationMotion =
        presentationMotion &&
        (packetAnimationState === "standing" ||
          packetAnimationState === "walking" ||
          packetAnimationState === "running")
          ? presentationMotion
          : null;
      const legacyAnimationState = activePresentationMotion
        ? activePresentationMotion.mode === "run"
          ? "running"
          : "walking"
        : packetAnimationState;
      return {
        entity,
        motionSnapshot,
        activePresentationMotion,
        legacyAnimationState,
      };
    });
    const animationPoses = resolveCrystalEntityAnimationPoses({
      runtime: entityAnimationRuntimeFromWindow(),
      worldKey: `${world.mapFileName ?? "none"}:${player.objectId}`,
      worldSeed: entityAnimationWorldSeed,
      now: spriteNow,
      entities: animationInputs.map(({ entity, legacyAnimationState, motionSnapshot }) => ({
        entity,
        state: legacyAnimationState,
        motionSnapshot,
      })),
    });

    return animationInputs.map(({
      entity,
      motionSnapshot,
      activePresentationMotion,
      legacyAnimationState,
    }) => {
      const animationPose = animationPoses[entity.objectId] ?? null;
      const animationState = animationPose?.animationState ?? legacyAnimationState;
      return {
        entity,
        animationPose,
        sprite: buildViewportEntitySprite(
          entity,
          sceneSpriteLibraries,
          sceneSpriteFrameIndex,
          spriteNow,
          animationState,
          motionSnapshot,
          activePresentationMotion,
          animationPose,
        ),
      };
    });
  }, [
    bevyLocalSelfMotion,
    bevyMapRuntimeGeneration,
    entityAnimationWorldSeed,
    player,
    sceneSpriteFrameIndex,
    sceneSpriteLibraries,
    viewportEntities,
    world.mapFileName,
  ]);
  const viewportGroundDrops = player
    ? world.groundDrops
        .filter(
          (drop) =>
            Math.abs(drop.x - (renderPlayer ?? player).x) <= VIEWPORT_RANGE_X &&
            Math.abs(drop.y - (renderPlayer ?? player).y) <= VIEWPORT_RANGE_Y,
        )
        .map((drop) => ({
          ...drop,
          dx: drop.x - (renderPlayer ?? player).x,
          dy: drop.y - (renderPlayer ?? player).y,
        }))
    : [];
  // Static layer (single-frame sprites = the ~500-tile bulk) rebuilds only when the
  // player's cell or the map region changes — NOT on the 120ms animation tick.
  const staticViewportMapSprites = useMemo(
    () =>
      renderPlayer
        ? buildViewportMapSprites(world, renderPlayer, 0, "static")
        : EMPTY_VIEWPORT_MAP_SPRITES,
    [renderPlayer?.x, renderPlayer?.y, world.originalMapRegion],
  );
  // Animated layer (multi-frame sprites only) tracks the frame index; the per-tick pass
  // skips all single-frame sprites early, so on maps without animated tiles it is ~free.
  const animatedViewportMapSprites = useMemo(
    () =>
      renderPlayer
        ? buildViewportMapSprites(world, renderPlayer, sceneSpriteFrameIndex, "animated")
        : EMPTY_VIEWPORT_MAP_SPRITES,
    [renderPlayer?.x, renderPlayer?.y, sceneSpriteFrameIndex, world.originalMapRegion],
  );
  const viewportMapSprites = useMemo(
    () => ({
      floor: [...staticViewportMapSprites.floor, ...animatedViewportMapSprites.floor],
      objects: [...staticViewportMapSprites.objects, ...animatedViewportMapSprites.objects],
    }),
    [staticViewportMapSprites, animatedViewportMapSprites],
  );

  // GPU map-atlas rendering (DEFAULT ON; escape hatch ?mapAtlas=0 or localStorage mir2-map-atlas=0).
  // Loads the packed atlas manifest once; when present, map tiles render from a few resident atlas
  // textures on the WebGl2MapAtlasLayer instead of ~450-510 per-frame DOM <img>/R2 GETs. The atlas
  // pages ship same-origin in the Vercel output (not pruned), so this needs no R2. If the manifest
  // is absent (404) or WebGL2 can't draw, mapGpuFailed/empty index force the DOM tile path — the map
  // is never left blank.
  const mapAtlasRequested = useMemo(() => {
    if (typeof window === "undefined") return false;
    const params = new URLSearchParams(window.location.search);
    if (params.get("mapAtlas") === "0") return false;
    if (params.get("mapAtlas") === "1") return true;
    if (window.localStorage.getItem("mir2-map-atlas") === "0") return false;
    return true;
  }, []);
  // Bevy-native map renderer (Stages 1-3, DEFAULT ON). The same packed map-atlas
  // tiles the DOM WebGl2MapAtlasLayer would draw are pushed into the Bevy runtime
  // and rendered in a UNIFIED y-sort band with the entities (map objects occlude
  // actors by cell row — Crystal's single sorted band); the DOM GPU layer + DOM
  // map sprites are disabled so the map is never drawn twice. Mirrors the
  // foldWebgl2ToBevy / mapAtlas flags.
  const bevyMapRequested = useMemo(() => {
    if (typeof window === "undefined") return false;
    const params = new URLSearchParams(window.location.search);
    // Stage 3: Bevy is the DEFAULT map renderer (it is already the default ENTITY
    // renderer, #119, and the map is the same runtime + sprite path — verified
    // rendering on both the WebGPU and WebGL2 Bevy backends). The standalone
    // WebGl2MapAtlasLayer is kept as an inert escape-hatch reachable via
    // `?bevyMap=0` (or localStorage "mir2-bevy-map"="0"), NOT deleted, pending the
    // cross-browser (Firefox/Safari) verification a single-Chrome env can't do.
    if (params.get("bevyMap") === "0") return false;
    if (params.get("bevyMap") === "1") return true;
    return window.localStorage.getItem("mir2-bevy-map") !== "0";
  }, []);
  const [mapAtlasIndex, setMapAtlasIndex] = useState<MapAtlasIndex | null>(null);
  const [mapGpuFailed, setMapGpuFailed] = useState(false);
  useEffect(() => {
    if (!mapAtlasRequested && !bevyMapRequested) return;
    let cancelled = false;
    void loadMapAtlasIndex().then((index) => {
      if (!cancelled) setMapAtlasIndex(index);
    });
    return () => {
      cancelled = true;
    };
  }, [mapAtlasRequested, bevyMapRequested]);
  const decodedStandaloneMapImagesRef = useRef<Map<string, DecodedStandaloneMapImage>>(new Map());
  const standaloneMapImageDecodeRequestedRef = useRef<
    Map<string, StandaloneMapImageDecodeRequest>
  >(new Map());
  const failedStandaloneMapImagesRef = useRef<Map<string, FailedStandaloneMapImage>>(new Map());
  const latestStandaloneMapImageSourcesRef = useRef<Map<string, MapStandaloneTileImageSource>>(
    new Map(),
  );
  const latestBevyMapActiveRef = useRef(false);
  const standaloneMapDecodeGenerationRef = useRef(0);
  const standaloneMapImagesFlushFrameRef = useRef<number | null>(null);
  const standaloneMapResidencyFlushFrameRef = useRef<number | null>(null);
  const pendingStandaloneMapImageResidencyRef = useRef<StandaloneMapImageResidency | null>(null);
  const onBevyMapImagesEvictedRef = useRef(onBevyMapImagesEvicted);
  onBevyMapImagesEvictedRef.current = onBevyMapImagesEvicted;
  const [standaloneMapImagesDecodedVersion, setStandaloneMapImagesDecodedVersion] = useState(0);
  const [standaloneMapImageResidency, setStandaloneMapImageResidency] =
    useState<StandaloneMapImageResidency>({
      runtimeGeneration: -1,
      keys: EMPTY_STRING_SET,
    });
  const scheduleStandaloneMapImagesFlush = useCallback(() => {
    if (standaloneMapImagesFlushFrameRef.current !== null) {
      return;
    }
    standaloneMapImagesFlushFrameRef.current = window.requestAnimationFrame(() => {
      standaloneMapImagesFlushFrameRef.current = null;
      setStandaloneMapImagesDecodedVersion((version) => version + 1);
    });
  }, []);
  const releaseStandaloneMapImages = useCallback((imageKeys: string[]) => {
    if (imageKeys.length === 0) {
      return;
    }
    evictStandaloneTilePixels(imageKeys);
    onBevyMapImagesEvictedRef.current?.(imageKeys);
    const releasedKeys = new Set(imageKeys);
    const pendingResidency = pendingStandaloneMapImageResidencyRef.current;
    if (pendingResidency && setIntersects(pendingResidency.keys, releasedKeys)) {
      pendingStandaloneMapImageResidencyRef.current = {
        runtimeGeneration: pendingResidency.runtimeGeneration,
        keys: new Set(
          Array.from(pendingResidency.keys).filter((key) => !releasedKeys.has(key)),
        ),
      };
    }
    setStandaloneMapImageResidency((current) => {
      if (!setIntersects(current.keys, releasedKeys)) {
        return current;
      }
      return {
        runtimeGeneration: current.runtimeGeneration,
        keys: new Set(Array.from(current.keys).filter((key) => !releasedKeys.has(key))),
      };
    });
  }, []);
  const scheduleStandaloneMapImageResidency = useCallback((residency: StandaloneMapImageResidency) => {
    pendingStandaloneMapImageResidencyRef.current = residency;
    if (standaloneMapResidencyFlushFrameRef.current !== null) {
      return;
    }
    standaloneMapResidencyFlushFrameRef.current = window.requestAnimationFrame(() => {
      standaloneMapResidencyFlushFrameRef.current = null;
      const pending = pendingStandaloneMapImageResidencyRef.current;
      pendingStandaloneMapImageResidencyRef.current = null;
      if (!pending) {
        return;
      }
      setStandaloneMapImageResidency((current) =>
        current.runtimeGeneration === pending.runtimeGeneration &&
        setsEqual(current.keys, pending.keys)
          ? current
          : pending,
      );
    });
  }, []);
  useEffect(
    () => () => {
      if (standaloneMapImagesFlushFrameRef.current !== null) {
        window.cancelAnimationFrame(standaloneMapImagesFlushFrameRef.current);
        standaloneMapImagesFlushFrameRef.current = null;
      }
      if (standaloneMapResidencyFlushFrameRef.current !== null) {
        window.cancelAnimationFrame(standaloneMapResidencyFlushFrameRef.current);
        standaloneMapResidencyFlushFrameRef.current = null;
      }
      pendingStandaloneMapImageResidencyRef.current = null;
      latestBevyMapActiveRef.current = false;
      standaloneMapDecodeGenerationRef.current += 1;
      const pendingDecodeKeys = Array.from(standaloneMapImageDecodeRequestedRef.current.keys());
      standaloneMapImageDecodeRequestedRef.current.clear();
      evictStandaloneTilePixels(pendingDecodeKeys);
    },
    [],
  );
  // Mirror the entity renderer's WebGL2 failure handling: if the atlas layer reports it can't draw
  // (no WebGL2 context, or a GL/texture error), latch the failure so the DOM tile path takes over
  // instead of leaving a blank scene on devices without WebGL2.
  const handleMapAtlasDebug = useCallback((debug: Record<string, unknown>) => {
    const reason = typeof debug.reason === "string" ? debug.reason : null;
    if (debug.supported === false || reason === "no-webgl2" || reason === "error") {
      setMapGpuFailed(true);
    }
  }, []);
  // The packed atlas is usable (manifest present, WebGL2 not failed, in-game).
  // This drives BOTH the DOM GPU layer and the Bevy-native map renderer; the
  // tile draw list is the same for either consumer.
  const mapAtlasUsable =
    mapAtlasRequested && Boolean(mapAtlasIndex) && !mapGpuFailed && screen === "game";
  const viewportProjectiles = renderPlayer
    ? world.projectiles
        .filter((projectile) => projectile.expiresAt > motionNow)
        .map((projectile) => ({
          ...projectile,
          fromDx: projectile.fromX - renderPlayer.x,
          fromDy: projectile.fromY - renderPlayer.y,
          toDx: projectile.toX - renderPlayer.x,
          toDy: projectile.toY - renderPlayer.y,
          progress: projectileProgress(projectile, motionNow),
        }))
    : [];
  const viewportDepthPlayer = renderPlayer ?? player ?? { x: 0, y: 0 };
  // Over-head chat bubbles + selected-target action label for the DOM scene overlays. Bubbles reuse
  // the chat log the shell already receives and expire on `motionNow` (the existing render tick);
  // the action label reuses the helper the shell already imported for keyboard target actions.
  const sceneChatBubbles =
    screen === "game"
      ? deriveChatBubbles(logs, viewportEntities, chatBubbleStateRef.current, motionNow)
      : [];
  const selectedTargetReadoutLabel = selectedEntity
    ? selectedTargetActionLabel(t, selectedEntity, targetDistance)
    : null;
  const domEntityFallbackRequested = shouldUseDomEntityFallback(clientProfile.layout === "touch");
  // Phase 3b (renderer consolidation), default ON. On the webgl2 backend the Bevy
  // canvas stays VISIBLE and transparent (the wasm build is transparent for both
  // backends) and draws entities itself; the DOM WebGl2EntityAtlasLayer
  // self-disables and the DOM map layer (z1) shows through the transparent Bevy
  // canvas (z2). So Bevy is the sole entity renderer on webgl2 too. Verified in a
  // production build on Chrome (webgpu + webgl2). The DOM layer is retained as a
  // fallback: `?bevyFoldWebgl2=0` (or localStorage mir2-bevy-fold-webgl2=0)
  // restores it — the escape hatch if a non-WebGPU browser's transparent webgl2
  // compositing misbehaves. (WebGPU is unaffected — the fold only gates webgl2.)
  const foldWebgl2ToBevy = useMemo(() => {
    if (typeof window === "undefined") return true;
    const params = new URLSearchParams(window.location.search);
    if (params.get("bevyFoldWebgl2") === "0") return false;
    if (params.get("bevyFoldWebgl2") === "1") return true;
    return window.localStorage.getItem("mir2-bevy-fold-webgl2") !== "0";
  }, []);
  const hideBevyCanvasForDomEntityFallback =
    screen === "game" &&
    ((bevyRuntimeBackend === "webgl2" && !foldWebgl2ToBevy) || domEntityFallbackRequested);
  const bevyEntityRendererSuppressed =
    domEntityFallbackRequested || runtimePhase === "dom-only" || runtimePhase === "boot-error";
  const bevyEntityRendererWanted =
    screen === "game" && !bevyEntityRendererSuppressed && shouldUseBevyEntityRenderer();
  const gpuEntityRendererRuntimePending =
    bevyEntityRendererWanted && (!bevyEntityRendererReady || !bevyRuntimeBackend);
  const entityRendererRequested =
    bevyEntityRendererWanted && bevyEntityRendererReady && Boolean(bevyRuntimeBackend);
  const useBevyEntityRenderer =
    entityRendererRequested && !hideBevyCanvasForDomEntityFallback;
  const bevySelfCameraActive =
    BEVY_SELF_CAMERA_REQUESTED &&
    useBevyEntityRenderer &&
    bevyMapRequested &&
    mapAtlasUsable &&
    bevyMapRuntimeReady;
  const bevyEntityInterpActive = BEVY_ENTITY_INTERP_REQUESTED && useBevyEntityRenderer;
  const bevyRemoteMotionActive =
    BEVY_REMOTE_MOTION_REQUESTED && bevyEntityInterpActive;
  const mapTileCameraOffset = bevySelfCameraActive ? EMPTY_VIEWPORT_OFFSET : playerCameraMotionOffset;
  const hasRenderPlayer = Boolean(renderPlayer);
  const mapDrawPlan = useMemo(
    () =>
      mapAtlasUsable && mapAtlasIndex && hasRenderPlayer
        ? buildMapTileDrawList(viewportMapSprites, mapAtlasIndex, mapTileCameraOffset)
        : null,
    [
      mapAtlasUsable,
      mapAtlasIndex,
      hasRenderPlayer,
      viewportMapSprites,
      mapTileCameraOffset.x,
      mapTileCameraOffset.y,
    ],
  );
  const mapTileDrawListCandidate = mapDrawPlan?.tiles ?? EMPTY_MAP_TILE_DRAW_LIST;
  const retainedMapTileDrawListRef = useRef<MapTileDraw[]>(EMPTY_MAP_TILE_DRAW_LIST);
  if (!mapTileDrawListsEqual(retainedMapTileDrawListRef.current, mapTileDrawListCandidate)) {
    retainedMapTileDrawListRef.current = mapTileDrawListCandidate;
  }
  const mapTileDrawList = retainedMapTileDrawListRef.current;
  const standaloneMapPlan = useMemo(
    () => (mapDrawPlan ? buildStandaloneMapTiles(mapDrawPlan.uncovered, mapTileCameraOffset) : null),
    [mapDrawPlan, mapTileCameraOffset.x, mapTileCameraOffset.y],
  );
  const standaloneMapTileDrawListCandidate =
    standaloneMapPlan?.tiles ?? EMPTY_STANDALONE_MAP_TILE_DRAW_LIST;
  const retainedStandaloneMapTileDrawListRef = useRef<MapStandaloneTileDraw[]>(
    EMPTY_STANDALONE_MAP_TILE_DRAW_LIST,
  );
  if (
    !standaloneMapTileDrawListsEqual(
      retainedStandaloneMapTileDrawListRef.current,
      standaloneMapTileDrawListCandidate,
    )
  ) {
    retainedStandaloneMapTileDrawListRef.current = standaloneMapTileDrawListCandidate;
  }
  const standaloneMapTileDrawList = retainedStandaloneMapTileDrawListRef.current;
  const standaloneMapImageSourcesCandidate =
    standaloneMapPlan?.images ?? EMPTY_STANDALONE_MAP_IMAGE_SOURCES;
  const retainedStandaloneMapImageSourcesRef = useRef<MapStandaloneTileImageSource[]>(
    EMPTY_STANDALONE_MAP_IMAGE_SOURCES,
  );
  if (
    !standaloneMapImageSourcesEqual(
      retainedStandaloneMapImageSourcesRef.current,
      standaloneMapImageSourcesCandidate,
    )
  ) {
    retainedStandaloneMapImageSourcesRef.current = standaloneMapImageSourcesCandidate;
  }
  const standaloneMapImageSources = retainedStandaloneMapImageSourcesRef.current;
  const standaloneMapImageSourcesByKey = useMemo(
    () => new Map(standaloneMapImageSources.map((source) => [source.imageKey, source])),
    [standaloneMapImageSources],
  );
  const residentStandaloneMapImageKeys =
    bevyMapRuntimeReady &&
    standaloneMapImageResidency.runtimeGeneration === bevyMapRuntimeGeneration
      ? standaloneMapImageResidency.keys
      : EMPTY_STRING_SET;
  const standaloneDomFallbackSprites = useMemo(() => {
    if (!standaloneMapPlan) {
      return EMPTY_VIEWPORT_MAP_SPRITES;
    }
    const keepInDom = (sprite: ViewportMapSprites["floor"][number]) => {
      const imageKey = standaloneMapPlan.imageKeyBySpriteKey.get(sprite.key);
      const requiredImageKeys =
        standaloneMapPlan.requiredImageKeysBySpriteKey.get(sprite.key) ??
        (imageKey ? [imageKey] : []);
      return (
        requiredImageKeys.length === 0 ||
        !isCompleteBevyMapImageFamilyResident(
          residentStandaloneMapImageKeys,
          requiredImageKeys,
        )
      );
    };
    return {
      floor: standaloneMapPlan.domFallback.floor.filter(keepInDom),
      objects: standaloneMapPlan.domFallback.objects.filter(keepInDom),
    };
  }, [residentStandaloneMapImageKeys, standaloneMapPlan]);
  // Imperative scene motion (perf): when Bevy renders entities AND interpolates the
  // self-camera + monsters at display Hz (both flags on), the ~33 Hz `motionNow` React
  // fold is redundant — it only re-created the scene tree. In that path the camera/glide
  // for the residual DOM overlays is driven imperatively (useSceneCameraMotionDriver) and
  // the React clock drops to ~10 Hz. Escape hatch: ?bevySelfCamera=0 / ?bevyEntityInterp=0.
  const imperativeSceneMotion = bevySelfCameraActive && bevyEntityInterpActive;
  const bevyPresentationPoseActive =
    BEVY_PRESENTATION_POSE_REQUESTED && imperativeSceneMotion;
  const bevyLocalMotionActive =
    BEVY_LOCAL_MOTION_REQUESTED && bevyPresentationPoseActive;
  const bevyPoseCommitActive =
    BEVY_POSE_COMMIT_REQUESTED && bevyPresentationPoseActive;
  // In the NON-imperative path (the default — Bevy renders sprites but the self-camera
  // scroll is folded through this React clock), the ~33 Hz cadence was measured as the
  // dominant run "judder": the map/camera scroll only advanced every ~30 ms, so on a
  // 120 Hz display the scroll sat still ~89 % of frames and lurched in 33 Hz steps —
  // very visible while RUNNING (2 tiles/600 ms = a big per-step displacement), barely
  // visible while walking. While the self-camera is actually gliding, tighten the clock
  // to ~60 Hz so the scroll keeps up with the display; fall back to 30 Hz when idle so
  // the scene tree is not re-created 60×/s during normal standing play (the perf win the
  // throttle exists for). The imperative path stays at its slow expiry cadence — Bevy
  // owns the scroll there.
  const selfCameraGliding =
    (renderPlayer?.movementUntil ?? 0) > Date.now() ||
    playerCameraMotionOffset.x !== 0 ||
    playerCameraMotionOffset.y !== 0;
  motionClockIntervalMsRef.current = imperativeSceneMotion ? 100 : selfCameraGliding ? 16 : 30;
  // In the imperative path the DOM world layers get a zero camera offset (the driver
  // pans them via a compositor transform at display Hz); otherwise they fold the React
  // `motionNow` camera offset exactly as before.
  const effectiveCameraOffset = imperativeSceneMotion
    ? EMPTY_VIEWPORT_OFFSET
    : playerCameraMotionOffset;
  const sceneMotionDriver = useSceneCameraMotionDriver(
    imperativeSceneMotion,
    () => latestMoveInputRef.current.renderPlayer,
    entityMotionSnapshotsRef,
    bevyPresentationPoseActive,
    bevyPoseCommitActive,
    bevyMapRuntimeGeneration,
    () => submittedPresentationContextRef.current,
    setBevyLocalSelfMotion,
  );
  const useWebGl2EntityAtlasRenderer =
    entityRendererRequested &&
    !foldWebgl2ToBevy &&
    bevyRuntimeBackend === "webgl2" &&
    shouldUseBevyEntityAtlas() &&
    shouldUseRawWebGl2EntityRenderer();
  const useGpuEntityRenderer = useBevyEntityRenderer || useWebGl2EntityAtlasRenderer;
  const useBevyEntityAtlas = useGpuEntityRenderer && shouldUseBevyEntityAtlas();
  const bevyEntityAtlasSources =
    useGpuEntityRenderer && useBevyEntityAtlas
      ? collectBevyEntityAtlasSources(viewportEntitySprites)
      : [];
  const bevyEntityAtlasKey = bevyEntityAtlasSources.length
    ? bevyEntityAtlasKeyForSources(bevyEntityAtlasSources)
    : null;
  // Synchronous in-memory residency read for the active atlas, so a cached-key
  // transition shows the packed atlas in the SAME frame (acquire()'s state set
  // is a microtask later). Conversion to a snapshot only runs in the fallback
  // branch below (i.e. when the React atlas state does not yet match the key).
  const peekedBevyEntityAtlasPayload =
    bevyEntityAtlasKey ? bevyAtlasResidency.peek(bevyEntityAtlasKey) : null;
  const latestBevyEntityAtlas =
    bevyEntityAtlasKey && bevyEntityAtlasLatestSnapshot?.key === bevyEntityAtlasKey
      ? bevyEntityAtlasLatestSnapshot
      : null;
  const activeBevyEntityAtlas =
    bevyEntityAtlasKey && bevyEntityAtlas?.key === bevyEntityAtlasKey
      ? bevyEntityAtlas
      : (peekedBevyEntityAtlasPayload ? payloadToAtlasSnapshot(peekedBevyEntityAtlasPayload) : null) ??
        latestBevyEntityAtlas;
  const webGl2EntityTextureReady =
    !useWebGl2EntityAtlasRenderer ||
    !activeBevyEntityAtlas ||
    webGl2EntityTextureReadyKey === activeBevyEntityAtlas.key;
  const handleWebGl2EntityAtlasDebug = useCallback((debug: WebGl2EntityAtlasDebug) => {
    const atlasKey = typeof debug.atlasKey === "string" ? debug.atlasKey : null;
    if (
      atlasKey &&
      debug.enabled === true &&
      debug.supported === true &&
      debug.textureReady === true &&
      debug.reason === "rendered" &&
      typeof debug.renderedLayers === "number" &&
      debug.renderedLayers > 0
    ) {
      setWebGl2EntityTextureReadyKey(atlasKey);
      setWebGl2EntityAtlasFailedKey((current) => (current === atlasKey ? null : current));
      return;
    }
    if (debug.reason === "error") {
      // The WebGL2 atlas renderer could not draw (e.g. an atlas texture failed to fetch or the
      // GL context was lost). Record the failed atlas key so DOM entity sprites are shown as a
      // fallback instead of leaving the player with invisible entities.
      setWebGl2EntityAtlasFailedKey(atlasKey ?? "__webgl2_entity_error__");
    }
  }, []);
  const webGl2EntityAtlasFailed =
    useWebGl2EntityAtlasRenderer &&
    webGl2EntityAtlasFailedKey !== null &&
    (webGl2EntityAtlasFailedKey === "__webgl2_entity_error__" ||
      webGl2EntityAtlasFailedKey === (activeBevyEntityAtlas?.key ?? null));
  const hideDomEntitySpritesForBevy =
    useGpuEntityRenderer &&
    !webGl2EntityAtlasFailed &&
    (!useBevyEntityAtlas || (Boolean(activeBevyEntityAtlas) && webGl2EntityTextureReady));
  const entityRenderState = buildBevyEntityRenderState({
    enabled: useGpuEntityRenderer,
    player,
    viewportEntitySprites,
    viewportDepthPlayer,
    playerCameraMotionOffset,
    entityMotionSnapshots: entityMotionSnapshotsRef.current,
    motionNow,
    atlas: useBevyEntityAtlas ? activeBevyEntityAtlas : null,
    bevySelfCameraActive,
    bevyEntityInterpActive,
  });
  const bevyEntityRenderState = useBevyEntityRenderer
    ? entityRenderState
    : disabledEntityRenderState(entityRenderState);

  // Start the Bevy map pipeline as soon as the runtime and packed atlas are usable,
  // but keep WebGL2/DOM ownership until Rust confirms a complete map sync for every
  // atlas page required by this viewport. This overlap avoids a cold-start clear-color
  // frame while page pixels cross the async decode -> WASM -> Bevy asset boundary.
  const bevyMapActive =
    bevyMapRequested && useBevyEntityRenderer && mapAtlasUsable && bevyMapRuntimeReady;
  const requiredBevyMapAtlasPageKeys = new Set(mapTileDrawList.map((tile) => tile.atlasKey));
  const bevyMapOwnershipActive =
    bevyMapActive &&
    Array.from(requiredBevyMapAtlasPageKeys).every((key) => bevyMapPresentedImageKeys.has(key));
  useEffect(() => {
    latestStandaloneMapImageSourcesRef.current = standaloneMapImageSourcesByKey;
    latestBevyMapActiveRef.current = bevyMapActive;
  }, [bevyMapActive, standaloneMapImageSourcesByKey]);
  // DOM GPU atlas layer draws until Bevy has acknowledged the complete viewport.
  const mapGpuActive = mapAtlasUsable && !bevyMapOwnershipActive;
  // Cell ownership: covered atlas tiles go to Bevy/WebGL2. Atlas misses,
  // including additive glows, stay in DOM until their standalone Bevy image is
  // decoded and acknowledged; when Bevy is disabled the legacy fallbacks stay intact.
  const mapDomSprites = bevyMapOwnershipActive
    ? standaloneDomFallbackSprites
    : mapGpuActive
      ? mapDrawPlan?.uncovered ?? EMPTY_VIEWPORT_MAP_SPRITES
      : viewportMapSprites;
  const mapDomBlendSpriteCount = countBlendMapSprites(mapDomSprites);

  useEffect(() => {
    const generation = standaloneMapDecodeGenerationRef.current + 1;
    standaloneMapDecodeGenerationRef.current = generation;
    const invalidateGeneration = () => {
      if (standaloneMapDecodeGenerationRef.current === generation) {
        standaloneMapDecodeGenerationRef.current = generation + 1;
      }
    };
    if (!bevyMapActive) {
      return invalidateGeneration;
    }

    const latestSources = latestStandaloneMapImageSourcesRef.current;
    for (const [imageKey] of standaloneMapImageDecodeRequestedRef.current) {
      if (latestSources.has(imageKey)) {
        continue;
      }
      standaloneMapImageDecodeRequestedRef.current.delete(imageKey);
      evictStandaloneTilePixels([imageKey]);
    }

    const now = Date.now();
    for (const source of standaloneMapImageSources) {
      const sourceSignature = standaloneMapImageSourceSignature(source);
      const failed = failedStandaloneMapImagesRef.current.get(source.imageKey);
      if (failed) {
        if (failed.sourceSignature === sourceSignature && failed.retryAt > now) {
          continue;
        }
        failedStandaloneMapImagesRef.current.delete(source.imageKey);
      }

      const decodedImage = decodedStandaloneMapImagesRef.current.get(source.imageKey);
      if (decodedImage?.sourceSignature === sourceSignature) {
        continue;
      }
      if (decodedImage) {
        decodedStandaloneMapImagesRef.current.delete(source.imageKey);
        releaseStandaloneMapImages([source.imageKey]);
      }

      const existingRequest = standaloneMapImageDecodeRequestedRef.current.get(source.imageKey);
      if (existingRequest?.sourceSignature === sourceSignature) {
        // Re-associate a still-relevant in-flight decode with this newest effect
        // generation. Its completion reads the latest source/keep set via refs.
        existingRequest.generation = generation;
        continue;
      }
      if (existingRequest) {
        standaloneMapImageDecodeRequestedRef.current.delete(source.imageKey);
        evictStandaloneTilePixels([source.imageKey]);
      }

      const request: StandaloneMapImageDecodeRequest = { sourceSignature, generation };
      standaloneMapImageDecodeRequestedRef.current.set(source.imageKey, request);
      void decodeStandaloneTilePixels(source)
        .then((decoded) => {
          if (standaloneMapImageDecodeRequestedRef.current.get(source.imageKey) !== request) {
            return;
          }
          standaloneMapImageDecodeRequestedRef.current.delete(source.imageKey);
          const latestSource = latestStandaloneMapImageSourcesRef.current.get(source.imageKey);
          const requestIsCurrent =
            request.generation === standaloneMapDecodeGenerationRef.current &&
            latestBevyMapActiveRef.current &&
            latestSource !== undefined &&
            standaloneMapImageSourceSignature(latestSource) === request.sourceSignature;
          if (!requestIsCurrent) {
            return;
          }
          if (!decoded) {
            failedStandaloneMapImagesRef.current.set(
              source.imageKey,
              {
                sourceSignature: request.sourceSignature,
                retryAt: Date.now() + STANDALONE_MAP_IMAGE_NEGATIVE_CACHE_MS,
              },
            );
            scheduleStandaloneMapImagesFlush();
            return;
          }
          failedStandaloneMapImagesRef.current.delete(source.imageKey);
          decodedStandaloneMapImagesRef.current.set(source.imageKey, {
            width: decoded.width,
            height: decoded.height,
            pixels: decoded.pixels,
            sourceSignature: request.sourceSignature,
          });
          const evicted = trimStandaloneMapImageCache(
            decodedStandaloneMapImagesRef.current,
            new Set(latestStandaloneMapImageSourcesRef.current.keys()),
          );
          if (evicted.length > 0) {
            releaseStandaloneMapImages(evicted);
          }
          scheduleStandaloneMapImagesFlush();
        })
        .catch(() => {
          if (standaloneMapImageDecodeRequestedRef.current.get(source.imageKey) !== request) {
            return;
          }
          standaloneMapImageDecodeRequestedRef.current.delete(source.imageKey);
          const latestSource = latestStandaloneMapImageSourcesRef.current.get(source.imageKey);
          if (
            request.generation !== standaloneMapDecodeGenerationRef.current ||
            !latestBevyMapActiveRef.current ||
            !latestSource ||
            standaloneMapImageSourceSignature(latestSource) !== request.sourceSignature
          ) {
            return;
          }
          failedStandaloneMapImagesRef.current.set(
            source.imageKey,
            {
              sourceSignature: request.sourceSignature,
              retryAt: Date.now() + STANDALONE_MAP_IMAGE_NEGATIVE_CACHE_MS,
            },
          );
          scheduleStandaloneMapImagesFlush();
        });
    }
    return invalidateGeneration;
  }, [
    bevyMapActive,
    releaseStandaloneMapImages,
    scheduleStandaloneMapImagesFlush,
    standaloneMapImageSources,
  ]);

  const bevyMapRenderState = useMemo<BevyMapRenderState>(() => {
    const revision = ++bevyMapRenderRevisionRef.current;
    const ackKey = `g${bevyMapRuntimeGeneration}:r${revision}`;
    const center = renderPlayer ? { centerX: renderPlayer.x, centerY: renderPlayer.y } : {};
    if (!bevyMapActive || !mapAtlasIndex) {
      return {
        enabled: false,
        stageWidth: ORIGINAL_UI.game.sceneWidth,
        stageHeight: ORIGINAL_UI.game.sceneHeight,
        ackKey,
        revision,
        ...center,
        atlases: [],
        atlasImages: [],
        tiles: [],
        standaloneTiles: [],
        retainedImageKeys: [],
        cameraOffset: EMPTY_VIEWPORT_OFFSET,
      };
    }
    const pageKeys = new Set(mapTileDrawList.map((tile) => tile.atlasKey));
    const atlases: NonNullable<BevyMapRenderState["atlases"]> = [];
    const atlasImages: NonNullable<BevyMapRenderState["atlasImages"]> = [];
    const imageReady = (imageKey: string) => {
      const source = standaloneMapImageSourcesByKey.get(imageKey);
      const decoded = decodedStandaloneMapImagesRef.current.get(imageKey);
      return (
        source !== undefined &&
        decoded?.sourceSignature === standaloneMapImageSourceSignature(source)
      );
    };
    const standaloneTiles = standaloneMapTileDrawList.filter((tile) => {
      const requiredImageKeys =
        standaloneMapPlan?.requiredImageKeysByTileKey.get(tile.key) ?? [tile.imageKey];
      // Do not expose a partly decoded animation family to Rust. The current
      // frame remains in DOM until every family image can be uploaded together.
      return requiredImageKeys.every(imageReady);
    });
    for (const pageKey of pageKeys) {
      const page = mapAtlasIndex.pages.get(pageKey);
      if (!page) {
        continue;
      }
      atlases.push({
        key: page.key,
        width: page.width,
        height: page.height,
        imageUrl: page.imageUrl,
        rects: page.rects.map((rect) => ({
          key: rect.key,
          x: rect.x,
          y: rect.y,
          width: rect.width,
          height: rect.height,
        })),
      });
    }
    const retainedImageKeys = Array.from(
      new Set(
        standaloneTiles.flatMap((tile) =>
          Array.from(
            standaloneMapPlan?.requiredImageKeysByTileKey.get(tile.key) ?? [tile.imageKey],
          ),
        ),
      ),
    );
    for (const imageKey of retainedImageKeys) {
      const decoded = decodedStandaloneMapImagesRef.current.get(imageKey);
      if (!decoded) {
        continue;
      }
      atlasImages.push({
        key: imageKey,
        width: decoded.width,
        height: decoded.height,
        pixels: decoded.pixels,
      });
    }
    return {
      enabled: true,
      stageWidth: ORIGINAL_UI.game.sceneWidth,
      stageHeight: ORIGINAL_UI.game.sceneHeight,
      ackKey,
      revision,
      ...center,
      atlases,
      atlasImages,
      // EXACTLY buildMapTileDrawList's output (camera offset folded into left/top).
      tiles: mapTileDrawList,
      standaloneTiles,
      retainedImageKeys,
      // Fold-in model: offset already baked into the tiles, so the root stays put.
      cameraOffset: EMPTY_VIEWPORT_OFFSET,
    };
  }, [
    bevyMapActive,
    mapAtlasIndex,
    mapTileDrawList,
    standaloneMapTileDrawList,
    standaloneMapPlan,
    standaloneMapImageSourcesByKey,
    standaloneMapImagesDecodedVersion,
    bevyMapRuntimeGeneration,
    renderPlayer?.x,
    renderPlayer?.y,
  ]);

  useLayoutEffect(() => {
    const previousMap = lastSceneMapSubmissionRef.current;
    const mapSubmissionChanged =
      previousMap.state !== bevyMapRenderState ||
      previousMap.imageResidencyVersion !== bevyMapImageResidencyVersion ||
      previousMap.runtimeGeneration !== bevyMapRuntimeGeneration ||
      previousMap.runtimeReady !== bevyMapRuntimeReady;
    let mapSubmitted = previousMap.submitted;
    let mapRevision = previousMap.revision;
    let mapCenter = previousMap.center;
    let entitySubmitted = false;
    if (mapSubmissionChanged) {
      let residentKeys: ReadonlySet<string> = EMPTY_STRING_SET;
      mapSubmitted = false;
      mapRevision = null;
      mapCenter = null;
      if (bevyMapRuntimeReady) {
        try {
          const uploadableKeys = new Set(
            (bevyMapRenderState.standaloneTiles ?? []).map((tile) => tile.imageKey),
          );
          for (const key of bevyMapRenderState.retainedImageKeys ?? []) {
            uploadableKeys.add(key);
          }
          const presentedKeys = onBevyMapRenderStateChange(bevyMapRenderState);
          residentKeys = new Set(
            presentedKeys.filter((key) => uploadableKeys.has(key)),
          );
          mapSubmitted = true;
          mapRevision = bevyMapRenderState.enabled
            ? bevyMapRenderState.revision ?? null
            : null;
          mapCenter =
            bevyMapRenderState.enabled &&
            bevyMapRenderState.centerX !== undefined &&
            bevyMapRenderState.centerY !== undefined
              ? { x: bevyMapRenderState.centerX, y: bevyMapRenderState.centerY }
              : null;
        } catch {
          // Keep the DOM fallback visible and retry on the next state/runtime generation.
        }
      }
      lastSceneMapSubmissionRef.current = {
        state: bevyMapRenderState,
        imageResidencyVersion: bevyMapImageResidencyVersion,
        runtimeGeneration: bevyMapRuntimeGeneration,
        runtimeReady: bevyMapRuntimeReady,
        submitted: mapSubmitted,
        revision: mapRevision,
        center: mapCenter,
      };
      // The page returns only keys acknowledged by a completed Rust map sync.
      // Batch the resulting DOM handoff onto one animation frame.
      scheduleStandaloneMapImageResidency({
        runtimeGeneration: bevyMapRuntimeGeneration,
        keys: residentKeys,
      });
    }
    if (bevyEntityRendererReady) {
      try {
        onBevyEntityRenderStateChange(bevyEntityRenderState);
        entitySubmitted = true;
      } catch {
        // Keep the last complete scene until both producers can commit together.
      }
    }

    const entityCenter =
      entitySubmitted &&
      bevyEntityRenderState.enabled &&
      bevyEntityRenderState.centerX !== undefined &&
      bevyEntityRenderState.centerY !== undefined
        ? { x: bevyEntityRenderState.centerX, y: bevyEntityRenderState.centerY }
        : null;
    const bothDisabled =
      mapSubmitted &&
      entitySubmitted &&
      !bevyMapRenderState.enabled &&
      !bevyEntityRenderState.enabled;
    const completeScene =
      mapRevision !== null &&
      mapCenter !== null &&
      entityCenter !== null &&
      mapCenter.x === entityCenter.x &&
      mapCenter.y === entityCenter.y;
    if (completeScene) {
      submittedPresentationContextRef.current = {
        mapRevision,
        mapCenter,
        entityCenter,
      };
    } else if (bothDisabled) {
      submittedPresentationContextRef.current = {
        mapRevision: null,
        mapCenter: null,
        entityCenter: null,
      };
    }
  }, [
    bevyMapRenderState,
    bevyMapImageResidencyVersion,
    bevyMapRuntimeGeneration,
    bevyMapRuntimeReady,
    onBevyMapRenderStateChange,
    scheduleStandaloneMapImageResidency,
    bevyEntityRenderState,
    bevyEntityRendererReady,
    onBevyEntityRenderStateChange,
  ]);

  // Bevy-self-camera path: push the self motion window straight to the runtime each
  // motion tick so Bevy can interpolate the camera at display Hz. Pushing the
  // (step-stable) window at 33Hz is cheap (6 numbers into a Cell); Bevy reads it
  // every frame. When not moving (no snapshot) push an expired window → camera
  // returns to origin. Active-gated; the runtime fn is a no-op when absent.
  useEffect(() => {
    const push = (
      window as typeof window & {
        __mir2BevyRuntime?: {
          setMir2SelfCameraMotion?: (
            fromX: number,
            fromY: number,
            toX: number,
            toY: number,
            startedMs: number,
            expiresMs: number,
          ) => void;
        };
      }
    ).__mir2BevyRuntime?.setMir2SelfCameraMotion;
    if (!push) return;
    if (!bevySelfCameraActive) {
      push(0, 0, 0, 0, 0, 0);
      return;
    }
    const playerObjectId = renderPlayer?.objectId;
    const snapshot = playerObjectId ? entityMotionSnapshotsRef.current[playerObjectId] : null;
    if (snapshot) {
      push(snapshot.fromX, snapshot.fromY, snapshot.toX, snapshot.toY, snapshot.startedAt, snapshot.expiresAt);
    } else {
      push(0, 0, 0, 0, 0, 0);
    }
  }, [bevySelfCameraActive, motionNow, renderPlayer]);

  // Packet-driven remote presentation is additive: Rust uses it only when the
  // segment target matches the latest packed entity snapshot. Any mismatch,
  // missing API, or disabled flag falls back to the existing TS motion window.
  useEffect(() => {
    const setEnabled = (
      window as typeof window & {
        __mir2BevyRuntime?: {
          setMir2RemoteMotionPresentationEnabled?: (enabled: boolean) => void;
        };
      }
    ).__mir2BevyRuntime?.setMir2RemoteMotionPresentationEnabled;
    if (!setEnabled) return;
    setEnabled(bevyRemoteMotionActive);
    return () => setEnabled(false);
  }, [bevyMapRuntimeGeneration, bevyRemoteMotionActive]);

  useEffect(() => {
    const setEnabled = (
      window as typeof window & {
        __mir2BevyRuntime?: {
          setMir2PresentationPoseEnabled?: (enabled: boolean) => void;
        };
      }
    ).__mir2BevyRuntime?.setMir2PresentationPoseEnabled;
    if (!setEnabled) return;
    setEnabled(bevyPresentationPoseActive);
    return () => setEnabled(false);
  }, [bevyMapRuntimeGeneration, bevyPresentationPoseActive]);

  useEffect(() => {
    const setEnabled = (
      window as typeof window & {
        __mir2BevyRuntime?: {
          setMir2LocalMotionPresentationEnabled?: (enabled: boolean) => void;
        };
      }
    ).__mir2BevyRuntime?.setMir2LocalMotionPresentationEnabled;
    if (!setEnabled) return;
    setEnabled(bevyLocalMotionActive);
    return () => setEnabled(false);
  }, [bevyMapRuntimeGeneration, bevyLocalMotionActive]);

  useEffect(() => {
    const activeKey = useWebGl2EntityAtlasRenderer ? activeBevyEntityAtlas?.key ?? null : null;
    if (webGl2EntityTextureReadyKey !== activeKey) {
      setWebGl2EntityTextureReadyKey(null);
    }
    // Give a newly-active atlas a fresh attempt before treating WebGL2 as failed.
    if (
      webGl2EntityAtlasFailedKey !== null &&
      webGl2EntityAtlasFailedKey !== "__webgl2_entity_error__" &&
      webGl2EntityAtlasFailedKey !== activeKey
    ) {
      setWebGl2EntityAtlasFailedKey(null);
    }
  }, [activeBevyEntityAtlas?.key, useWebGl2EntityAtlasRenderer, webGl2EntityTextureReadyKey, webGl2EntityAtlasFailedKey]);
  if (typeof window !== "undefined") {
    (
      window as typeof window & {
        __mir2BevyEntityRendererDebug?: unknown;
        __mir2BevyRuntimeDebug?: unknown;
      }
    ).__mir2BevyEntityRendererDebug = {
      ready: bevyEntityRendererReady,
      runtime: (window as typeof window & { __mir2BevyRuntimeDebug?: unknown }).__mir2BevyRuntimeDebug ?? null,
      enabled: bevyEntityRenderState.enabled,
      remoteMotionRequested: BEVY_REMOTE_MOTION_REQUESTED,
      remoteMotionActive: bevyRemoteMotionActive,
      presentationPoseRequested: BEVY_PRESENTATION_POSE_REQUESTED,
      presentationPoseActive: bevyPresentationPoseActive,
      localMotionRequested: BEVY_LOCAL_MOTION_REQUESTED,
      localMotionActive: bevyLocalMotionActive,
      poseCommitRequested: BEVY_POSE_COMMIT_REQUESTED,
      poseCommitActive: bevyPoseCommitActive,
      submittedPresentationContext: submittedPresentationContextRef.current,
      presentationPoseBridge: sceneMotionDriver.getDiagnostics(),
      rawWebGl2Enabled: useWebGl2EntityAtlasRenderer && entityRenderState.enabled,
      rawWebGl2TextureReady: webGl2EntityTextureReady,
      rawWebGl2TextureReadyKey: webGl2EntityTextureReadyKey,
      rawWebGl2AtlasFailed: webGl2EntityAtlasFailed,
      rawWebGl2AtlasFailedKey: webGl2EntityAtlasFailedKey,
      entityCount: entityRenderState.entities.length,
      layerCount: entityRenderState.entities.reduce((count, entity) => count + entity.layers.length, 0),
      atlasMode: useBevyEntityAtlas
        ? activeBevyEntityAtlas
          ? "packed"
          : bevyEntityAtlasKey
            ? "warming"
            : "packing"
        : "single-image",
      atlasKey: entityRenderState.atlases?.[0]?.key ?? null,
      atlasCount: entityRenderState.atlases?.length ?? 0,
      atlasRectCount: (entityRenderState.atlases ?? []).reduce(
        (count, atlas) => count + atlas.rects.length,
        0,
      ),
      atlasPixelBytes: (entityRenderState.atlasImages ?? []).reduce(
        (sum, atlas) => sum + (atlas.pixels?.byteLength ?? atlas.width * atlas.height * 4),
        0,
      ),
      atlasSourceCount: bevyEntityAtlasSources.length,
      atlasCacheSize: bevyAtlasResidency.stats().memoryCacheSize,
      atlasBudgetProfile: BEVY_ENTITY_ATLAS_BUDGET_PROFILE,
      atlasCurrentKey: bevyEntityAtlasKey,
      atlasPendingKey: bevyEntityAtlasRequestRef.current?.key ?? null,
      atlasCachedCurrent: Boolean(peekedBevyEntityAtlasPayload),
      atlasLatestKey: bevyEntityAtlasLatestSnapshot?.key ?? null,
      atlasLatestCurrent: Boolean(latestBevyEntityAtlas),
      domEntityFallback: useBevyEntityRenderer && !hideDomEntitySpritesForBevy,
      canvasHidden: hideBevyCanvasForDomEntityFallback,
      foldWebgl2ToBevy,
      atlasStats: { ...bevyAtlasResidency.stats(), ...bevyEntityAtlasResolveStats },
      spriteLibraryCache: originalSceneSpriteLibraryCacheStats(),
      sceneAssetRuntime: sceneAssetRuntimeStats(),
      domImageCount: document.images.length,
    };
    (
      window as typeof window & { __mir2BevyMapRendererDebug?: unknown }
    ).__mir2BevyMapRendererDebug = {
      requested: bevyMapRequested,
      active: bevyMapOwnershipActive,
      pipelineActive: bevyMapActive,
      mapGpuActive,
      atlasUsable: mapAtlasUsable,
      atlasIndexReady: Boolean(mapAtlasIndex),
      enabled: bevyMapRenderState.enabled,
      tileCount: bevyMapRenderState.tiles.length,
      uncoveredFloorCount: mapDrawPlan?.uncovered.floor.length ?? 0,
      uncoveredObjectCount: mapDrawPlan?.uncovered.objects.length ?? 0,
      standaloneTileCount: bevyMapRenderState.standaloneTiles?.length ?? 0,
      standaloneAdditiveTileCount:
        bevyMapRenderState.standaloneTiles?.filter((tile) => tile.additive).length ?? 0,
      standaloneRetainedImageCount: bevyMapRenderState.retainedImageKeys?.length ?? 0,
      standaloneImageSourceCount: standaloneMapImageSources.length,
      standaloneDecodedImageCount: decodedStandaloneMapImagesRef.current.size,
      standaloneFailedImageCount: failedStandaloneMapImagesRef.current.size,
      atlasPageCount: bevyMapRenderState.atlases?.length ?? 0,
      atlasImageCount: bevyMapRenderState.atlasImages?.length ?? 0,
      packedPageTransport: "bevy-asset-server-url",
      ackKey: bevyMapRenderState.ackKey,
      domSpriteCount: mapDomSprites.floor.length + mapDomSprites.objects.length,
      domBlendSpriteCount: mapDomBlendSpriteCount,
      cameraOffset: bevyMapRenderState.cameraOffset ?? null,
    };
  }
  const showSyntheticScene = screen === "game" && !world.originalMapRegion;
  const sceneAssetUrlsRef = useRef<string[]>([]);
  sceneAssetUrlsRef.current = collectVisibleSceneAssetUrls(viewportMapSprites, viewportEntitySprites, {
    includeEntityPreloadPaths: !hideDomEntitySpritesForBevy,
  });

  // Prefetch a ring of static tiles just outside the visible viewport whenever the
  // player's cell changes, so walking does not pop-in cold tiles. Idle-scheduled and
  // deduped against the currently-visible set; preloadSceneAssetUrls warms the browser
  // (and SW) cache via Image() with no DOM/render impact.
  useEffect(() => {
    if (screen !== "game" || !renderPlayer || !world.originalMapRegion) {
      return;
    }
    let cancelled = false;
    const warmPrefetchRing = () => {
      if (cancelled || !renderPlayer) {
        return;
      }
      const ring = buildViewportMapSprites(world, renderPlayer, 0, "static", SCENE_TILE_PREFETCH_RING_CELLS);
      const visible = new Set(sceneAssetUrlsRef.current);
      const ringUrls = Array.from(
        new Set([
          ...ring.objects.map((sprite) => mapSpriteRenderPath(sprite.path)),
          ...ring.floor.map((sprite) => sprite.path),
        ]),
      ).filter((url) => !visible.has(url));
      if (ringUrls.length) {
        void preloadSceneAssetUrls(ringUrls, SCENE_TILE_PREFETCH_TIMEOUT_MS, { allowPartialReady: true });
      }
    };
    const idleWindow = window as typeof window & {
      requestIdleCallback?: (cb: () => void, opts?: { timeout: number }) => number;
      cancelIdleCallback?: (handle: number) => void;
    };
    const usingIdle = typeof idleWindow.requestIdleCallback === "function";
    const handle = usingIdle
      ? idleWindow.requestIdleCallback!(warmPrefetchRing, { timeout: 1200 })
      : window.setTimeout(warmPrefetchRing, 200);
    return () => {
      cancelled = true;
      if (usingIdle) {
        idleWindow.cancelIdleCallback?.(handle);
      } else {
        window.clearTimeout(handle);
      }
    };
    // Re-run only on player-cell / map / screen change (not every animation tick).
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [renderPlayer?.x, renderPlayer?.y, world.originalMapRegion, screen]);
  const [sceneAssetPreloadReadiness, setSceneAssetPreloadReadiness] =
    useState<SceneAssetReadiness | null>(null);
  // True once the safety-valve window elapses with the scene up but readiness unresolved.
  const [readinessSafetyExpired, setReadinessSafetyExpired] = useState(false);
  // Key ONLY on the map-region identity. It MUST stay stable while the player stands
  // still: the tile-preload readiness effect re-runs (and resets preloadStatus to
  // "loading", disposing the in-flight preload) whenever this key changes. The old key
  // mixed in sceneAssetUrlKey (which includes per-entity sprite URLs), viewportEntities
  // .length, and desiredSceneSpriteLibraryKey (derived from world.entities) — all of
  // which churn every frame in a populated map (BichonProvince guards/archers/monsters).
  // That churn restarted the 5s preload faster than it could finish, so preloadStatus
  // was stuck "loading" forever → sceneInteractionReady never became true → MOVEMENT
  // was permanently gated. Sprite-library readiness is already a separate effect dep
  // (sceneSpriteLibrariesReady), so dropping the entity-volatile parts is safe.
  const sceneAssetReadinessKey =
    screen === "game" && renderPlayer && world.originalMapRegion
      ? [
          world.originalMapRegion.mapFileName,
          world.originalMapRegion.regionBounds.minX,
          world.originalMapRegion.regionBounds.minY,
          world.originalMapRegion.regionBounds.maxX,
          world.originalMapRegion.regionBounds.maxY,
        ].join(":")
      : `idle:${screen}`;

  // Diagnostic: expose the scene/movement-readiness gate so an Alt+D snapshot reveals
  // exactly which factor is blocking movement (sceneInteractionReady is the actual gate
  // checked by the keyboard/click move handlers). No behavior change.
  useEffect(() => {
    if (typeof window === "undefined") return;
    (window as unknown as Record<string, unknown>).__mir2SceneGate = {
      screen,
      sceneInteractionReady,
      sceneSpriteLibrariesReady,
      pendingSceneSpriteLibraryCount: pendingSceneSpriteLibraryKeys.length,
      pendingSceneSpriteLibraryKeys: pendingSceneSpriteLibraryKeys.slice(0, 10),
      desiredSceneSpriteLibraryCount: desiredSceneSpriteLibraryKeys.length,
      preloadStatus: sceneAssetPreloadReadiness?.status ?? null,
      preloadInteractionReady: sceneAssetPreloadReadiness?.interactionReady ?? null,
      preloadReady: sceneAssetPreloadReadiness?.ready ?? null,
      preloadFailed: sceneAssetPreloadReadiness?.failed ?? null,
      hasRenderPlayer: Boolean(renderPlayer),
      hasMapRegion: Boolean(world.originalMapRegion),
      gpuEntityRendererRuntimePending,
      useGpuEntityRenderer,
    };
  }, [
    screen,
    sceneInteractionReady,
    sceneSpriteLibrariesReady,
    pendingSceneSpriteLibraryKeys.length,
    desiredSceneSpriteLibraryKeys.length,
    sceneAssetPreloadReadiness?.status,
    sceneAssetPreloadReadiness?.interactionReady,
    renderPlayer,
    world.originalMapRegion,
    gpuEntityRendererRuntimePending,
    useGpuEntityRenderer,
  ]);

  useEffect(() => {
    if (!useGpuEntityRenderer || !useBevyEntityAtlas || !entityRenderState.enabled) {
      if (bevyEntityAtlas) {
        setBevyEntityAtlas(null);
      }
      return;
    }

    if (!bevyEntityAtlasSources.length || !bevyEntityAtlasKey) {
      if (bevyEntityAtlas) {
        setBevyEntityAtlas(null);
      }
      return;
    }

    // A memory hit is served synchronously by the render-path peek
    // (peekedBevyEntityAtlasPayload); here we always acquire through the
    // residency manager (memory -> [null persistent] -> resolve fetcher) and
    // commit the React atlas state when it resolves. The manager owns the
    // in-memory tier + LRU; resolve owns the prebuilt/persistent/live cold path
    // and records its source breakdown into bevyEntityAtlasResolveStats.
    const requestId = (bevyEntityAtlasRequestRef.current?.requestId ?? 0) + 1;
    bevyEntityAtlasRequestRef.current = { key: bevyEntityAtlasKey, requestId };
    bevyEntityAtlasSourcesByKey.set(bevyEntityAtlasKey, bevyEntityAtlasSources);
    let disposed = false;
    let acquired = false;

    void bevyAtlasResidency
      .acquire(bevyEntityAtlasKey)
      .then((payload) => {
        acquired = true;
        if (disposed || bevyEntityAtlasRequestRef.current?.requestId !== requestId) {
          bevyAtlasResidency.release(bevyEntityAtlasKey);
          acquired = false;
          return;
        }
        const atlas = payloadToAtlasSnapshot(payload);
        bevyEntityAtlasLatestSnapshot = atlas;
        bevyEntityAtlasRequestRef.current = null;
        setBevyEntityAtlas(atlas);
      })
      .catch(() => {
        if (bevyEntityAtlasRequestRef.current?.requestId === requestId) {
          bevyEntityAtlasRequestRef.current = null;
        }
      })
      .finally(() => {
        if (bevyEntityAtlasSourcesByKey.get(bevyEntityAtlasKey) === bevyEntityAtlasSources) {
          bevyEntityAtlasSourcesByKey.delete(bevyEntityAtlasKey);
        }
      });

    return () => {
      disposed = true;
      if (acquired) {
        bevyAtlasResidency.release(bevyEntityAtlasKey);
        acquired = false;
      }
    };
  }, [useGpuEntityRenderer, useBevyEntityAtlas, bevyEntityAtlasKey]);

  useEffect(() => {
    if (screen !== "game" || !renderPlayer || !world.originalMapRegion || !sceneSpriteLibrariesReady) {
      setSceneAssetPreloadReadiness(null);
      return;
    }

    const urls = sceneAssetUrlsRef.current;
    if (!urls.length) {
      setSceneAssetPreloadReadiness(createSceneAssetReadiness(sceneAssetReadinessKey, true, "ready", 0));
      return;
    }

    let disposed = false;
    setSceneAssetPreloadReadiness(createSceneAssetReadiness(sceneAssetReadinessKey, false, "loading", urls.length));
    void preloadSceneAssetUrls(urls, 2_500, { allowPartialReady: true }).then((readiness) => {
      if (disposed) return;
      const visualReady = readiness.failed === 0;
      const interactionReady = readiness.ready;
      setSceneAssetPreloadReadiness({
        ...readiness,
        key: sceneAssetReadinessKey,
        ready: interactionReady,
        interactionReady,
        visualReady,
        status: visualReady ? "ready" : readiness.status === "ready" ? "timeout" : readiness.status,
      });
    });

    return () => {
      disposed = true;
    };
  }, [screen, sceneAssetReadinessKey, sceneSpriteLibrariesReady]);

  // Safety-valve timer: arm when the scene is up; if readiness hasn't cleared the
  // overlay within SCENE_READINESS_SAFETY_MS, force interaction-ready (see notifier).
  useEffect(() => {
    if (screen !== "game" || !renderPlayer || !world.originalMapRegion) {
      setReadinessSafetyExpired(false);
      return;
    }
    setReadinessSafetyExpired(false);
    const timer = window.setTimeout(() => setReadinessSafetyExpired(true), SCENE_READINESS_SAFETY_MS);
    return () => window.clearTimeout(timer);
  }, [screen, sceneAssetReadinessKey]);

  useEffect(() => {
    const notify = (readiness: SceneAssetReadiness) => {
      sceneAssetReadinessCallbackRef.current(readiness);
    };
    const entityAtlasPending =
      gpuEntityRendererRuntimePending ||
      (useGpuEntityRenderer &&
        useBevyEntityAtlas &&
        (Boolean(entityRenderState.enabled && !bevyEntityAtlasKey) ||
          (Boolean(bevyEntityAtlasKey) && (!activeBevyEntityAtlas || !webGl2EntityTextureReady))));
    const entityAtlasPendingCount = entityAtlasPending ? 1 : 0;

    if (screen !== "game") {
      notify(createSceneAssetReadiness(sceneAssetReadinessKey, true, "idle", 0));
      return;
    }

    if (!renderPlayer || !world.originalMapRegion) {
      notify(createSceneAssetReadiness(sceneAssetReadinessKey, false, "loading", 0));
      return;
    }

    if (readinessSafetyExpired) {
      // Safety valve elapsed — the scene is rendered; let the player interact even if
      // a sprite library / tile is still resolving (they keep loading in the background).
      notify({
        ...createSceneAssetReadiness(sceneAssetReadinessKey, true, "ready", 0),
        interactionReady: true,
      });
      return;
    }

    const urls = sceneAssetUrlsRef.current;
    if (!sceneSpriteLibrariesReady) {
      notify(
        createSceneAssetReadiness(
          sceneAssetReadinessKey,
          false,
          "loading",
          urls.length + pendingSceneSpriteLibraryKeys.length + entityAtlasPendingCount,
        ),
      );
      return;
    }
    const preloadReadiness =
      sceneAssetPreloadReadiness?.key === sceneAssetReadinessKey ? sceneAssetPreloadReadiness : null;
    if (!urls.length) {
      // Map + tile libraries are ready and there are no scene tiles to preload, so the
      // player can interact (move) NOW. entityAtlasPending (the GPU/Bevy entity atlas)
      // only reflects visual completeness and must NOT gate interaction — otherwise a
      // hung/slow Bevy runtime keeps interactionReady false and the player can never move.
      notify({
        ...createSceneAssetReadiness(
          sceneAssetReadinessKey,
          !entityAtlasPending,
          entityAtlasPending ? "loading" : "ready",
          entityAtlasPendingCount,
        ),
        interactionReady: true,
      });
      return;
    }

    if (!preloadReadiness || preloadReadiness.status === "loading") {
      notify(createSceneAssetReadiness(sceneAssetReadinessKey, false, "loading", urls.length + entityAtlasPendingCount));
      return;
    }

    if (entityAtlasPending) {
      // Tiles are fully preloaded here, so interaction (movement) is ready even though the
      // GPU/Bevy entity atlas is still loading. Decoupling interactionReady from the entity
      // atlas is part of the "can't move" fix: a Bevy runtime that never reports ready
      // (intermittent WebGPU/WebGL boot) used to force interactionReady:false forever.
      // visualReady/ready still wait on the atlas so the HUD can reflect full readiness.
      notify({
        ...preloadReadiness,
        ready: false,
        interactionReady: true,
        visualReady: preloadReadiness.visualReady ?? preloadReadiness.failed === 0,
        status: "loading",
        pending: preloadReadiness.pending + entityAtlasPendingCount,
        total: preloadReadiness.total + entityAtlasPendingCount,
      });
      return;
    }

    notify(preloadReadiness);
  }, [
    screen,
    sceneAssetReadinessKey,
    sceneAssetPreloadReadiness,
    sceneSpriteLibrariesReady,
    pendingSceneSpriteLibraryKeys.length,
    gpuEntityRendererRuntimePending,
    useGpuEntityRenderer,
    useBevyEntityAtlas,
    bevyEntityAtlasKey,
    activeBevyEntityAtlas?.key,
    webGl2EntityTextureReady,
    readinessSafetyExpired,
  ]);

  useEffect(() => {
    if (screen !== "game") {
      return;
    }

    rescueStalledSceneAssetImages();
    const interval = window.setInterval(() => {
      rescueStalledSceneAssetImages();
    }, 1500);

    return () => {
      window.clearInterval(interval);
    };
  }, [screen, sceneAssetReadinessKey]);

  function scenePointFromMouseEvent(event: MouseEvent<HTMLElement>) {
    const rect = stageFrameRef.current?.getBoundingClientRect() ?? event.currentTarget.getBoundingClientRect();
    const scaleX = ORIGINAL_UI.game.sceneWidth / Math.max(rect.width, 1);
    const scaleY = ORIGINAL_UI.game.sceneHeight / Math.max(rect.height, 1);
    return {
      sceneX: (event.clientX - rect.left) * scaleX,
      sceneY: (event.clientY - rect.top) * scaleY,
    };
  }

  function committedViewportPlayerPosition() {
    const stage = stageFrameRef.current;
    if (!stage) return null;
    const x = Number(stage.dataset.viewportPlayerX);
    const y = Number(stage.dataset.viewportPlayerY);
    return Number.isFinite(x) && Number.isFinite(y) ? { x, y } : null;
  }

  function tileFromScenePoint(sceneX: number, sceneY: number) {
    const latest = latestMoveInputRef.current;
    // Interpret the pointer against the same committed camera position the
    // player can see. A concurrent React render may update latestMoveInputRef
    // before its DOM commit, which otherwise turns horizontal clicks diagonal.
    const basePlayer = committedViewportPlayerPosition() ?? latest.renderPlayer ?? latest.player;
    if (!basePlayer) return null;
    return {
      x: basePlayer.x + Math.floor(sceneX / VIEWPORT_CELL_WIDTH) - VIEWPORT_OFFSET_X,
      y: basePlayer.y + Math.floor(sceneY / VIEWPORT_CELL_HEIGHT) - VIEWPORT_OFFSET_Y,
    };
  }

  function dispatchSceneMoveInput(pointer: HeldScenePointer) {
    if (latestMoveInputRef.current.screen !== "game") return;
    if (!sceneInteractionReady) return;
    const tile = tileFromScenePoint(pointer.sceneX, pointer.sceneY);
    if (!tile) return;

    if (pointer.button === 2) {
      onViewportDirectionStep(tile.x, tile.y, "run");
    } else {
      onViewportDirectionStep(tile.x, tile.y, "walk");
    }
  }

  function dispatchSceneClickInput(pointer: HeldScenePointer) {
    if (latestMoveInputRef.current.screen !== "game") return;
    if (!sceneInteractionReady) return;
    const tile =
      pointer.tileX !== undefined && pointer.tileY !== undefined
        ? { x: pointer.tileX, y: pointer.tileY }
        : tileFromScenePoint(pointer.sceneX, pointer.sceneY);
    if (!tile) return;

    if (pointer.button === 2) {
      onViewportTileSecondaryAction(tile.x, tile.y);
    } else {
      onViewportTileClick(tile.x, tile.y);
    }
  }

  function handleScenePointerAction(event: MouseEvent<HTMLDivElement>) {
    if (screen !== "game" || !player || !sceneInteractionReady) {
      return;
    }

    const target = event.target;
    if (
      target instanceof HTMLElement &&
      target.closest("[data-ui-interactive='true'], .game-ui-scene, .login-overlay, .select-overlay")
    ) {
      return;
    }

    if (event.button !== 0 && event.button !== 2) {
      return;
    }

    event.preventDefault();
    const point = scenePointFromMouseEvent(event);
    const pointer: HeldScenePointer = {
      button: event.button,
      sceneX: point.sceneX,
      sceneY: point.sceneY,
      startedAt: Date.now(),
      dispatched: false,
    };
    heldScenePointerRef.current = pointer;
  }

  function handleScenePointerMove(event: MouseEvent<HTMLDivElement>) {
    const held = heldScenePointerRef.current;
      if (!held || screen !== "game" || !sceneInteractionReady) {
        return;
      }

    const point = scenePointFromMouseEvent(event);
    heldScenePointerRef.current = {
      ...held,
      sceneX: point.sceneX,
      sceneY: point.sceneY,
    };
  }

  function stopHeldScenePointer() {
    const held = heldScenePointerRef.current;
    heldScenePointerRef.current = null;
    if (!held || held.dispatched) return;
    dispatchSceneClickInput(held);
  }

  useEffect(() => {
    if (screen !== "game") {
      heldScenePointerRef.current = null;
      return;
    }

    const timer = window.setInterval(() => {
      const held = heldScenePointerRef.current;
      if (!held) return;
      if (!held.dispatched && Date.now() - held.startedAt < CRYSTAL_MOVE_INPUT_INTERVAL_MS) {
        return;
      }
      held.dispatched = true;
      dispatchSceneMoveInput(held);
    }, CRYSTAL_MOVE_INPUT_INTERVAL_MS);

    const stop = () => {
      heldScenePointerRef.current = null;
      onViewportDirectionStop();
    };
    window.addEventListener("mouseup", stop);
    window.addEventListener("blur", stop);

    return () => {
      window.clearInterval(timer);
      window.removeEventListener("mouseup", stop);
      window.removeEventListener("blur", stop);
    };
  }, [screen, sceneInteractionReady, onViewportDirectionStep, onViewportDirectionStop]);

  // Normal play uses the stage-level coordinate handlers. Keep the 1,155-button
  // hit grid only for legacy QA scripts that still query per-tile aria labels.
  const legacyTileHitGridEnabled = useMemo(() => {
    const params = new URLSearchParams(window.location.search);
    const explicit = params.get("legacyTileGrid");
    return explicit === null ? params.get("mir2Debug") === "1" : explicit === "1";
  }, []);
  const tileHitGrid = useMemo(
    () => legacyTileHitGridEnabled ? (
      <div className={`viewport-grid-overlay ${screen !== "game" ? "hidden" : ""}`}>
        {viewportTiles.map((tile) => (
          <button
            key={`tile-${tile.x}-${tile.y}`}
            type="button"
            className="tile-hit"
            style={{
              left: `${VIEWPORT_MOUSE_TILE_CENTER_X + tile.dx * VIEWPORT_CELL_WIDTH}px`,
              top: `${VIEWPORT_MOUSE_TILE_CENTER_Y + tile.dy * VIEWPORT_CELL_HEIGHT}px`,
            }}
            data-ui-interactive="true"
            onMouseDown={(event) => {
              if (event.button !== 0 && event.button !== 2) {
                return;
              }

              event.stopPropagation();
              if (!sceneInteractionReady) {
                event.preventDefault();
                return;
              }
              const point = scenePointFromMouseEvent(event);
              const pointer: HeldScenePointer = {
                button: event.button,
                sceneX: point.sceneX,
                sceneY: point.sceneY,
                startedAt: Date.now(),
                dispatched: false,
                tileX: tile.x,
                tileY: tile.y,
              };
              heldScenePointerRef.current = pointer;
              if (event.button === 2) {
                event.preventDefault();
              }
            }}
            onMouseMove={(event) => {
              const held = heldScenePointerRef.current;
              if (!held) return;
              const point = scenePointFromMouseEvent(event);
              heldScenePointerRef.current = {
                ...held,
                sceneX: point.sceneX,
                sceneY: point.sceneY,
              };
            }}
            onMouseUp={stopHeldScenePointer}
            onClick={(event) => {
              event.preventDefault();
              event.stopPropagation();
            }}
            onContextMenu={(event) => {
              event.preventDefault();
              event.stopPropagation();
            }}
            aria-label={`tile ${tile.x}, ${tile.y}`}
          />
        ))}
      </div>
    ) : null,
    // eslint-disable-next-line react-hooks/exhaustive-deps -- handlers read refs/hoisted fns; only sceneInteractionReady is dynamic state
    [legacyTileHitGridEnabled, viewportTiles, screen, sceneInteractionReady],
  );

  return (
    <main
      className={`mir-client-page ${clientProfile.layout === "touch" && clientProfile.input === "touch" ? "force-mobile-controls" : ""}`}
      data-layout-profile={clientProfile.layout}
      data-input-profile={clientProfile.input}
      data-client-screen={screen}
      style={stageScaleStyle}
    >
      <section className="mir-stage">
        <div
          ref={stageFrameRef}
          className={`client-stage-frame ${screen === "game" && !sceneInteractionReady ? "scene-assets-pending" : ""}`}
          data-viewport-player-x={(renderPlayer ?? player)?.x}
          data-viewport-player-y={(renderPlayer ?? player)?.y}
          data-viewport-tile-center-x={VIEWPORT_MOUSE_TILE_CENTER_X}
          data-viewport-tile-center-y={VIEWPORT_MOUSE_TILE_CENTER_Y}
          data-viewport-cell-width={VIEWPORT_CELL_WIDTH}
          data-viewport-cell-height={VIEWPORT_CELL_HEIGHT}
          data-viewport-scene-width={ORIGINAL_UI.game.sceneWidth}
          data-viewport-scene-height={ORIGINAL_UI.game.sceneHeight}
          tabIndex={-1}
          onMouseDownCapture={(event) => {
            if (screen === "game") {
              stageFrameRef.current?.focus({ preventScroll: true });
            }
            if (screen === "game" && event.button === 2) {
              event.preventDefault();
            }
          }}
          onMouseDown={handleScenePointerAction}
          onMouseMove={handleScenePointerMove}
          onMouseUp={stopHeldScenePointer}
          onContextMenuCapture={(event) => {
            if (screen === "game") {
              event.preventDefault();
            }
          }}
          onContextMenu={(event) => {
            if (screen === "game") {
              event.preventDefault();
            }
          }}
        >
          {screen === "login" ? (
            <div className="client-scene-overlay">
              <img
                className="client-scene-background"
                src={loginBackgroundFrame}
                alt=""
                draggable={false}
              />
            </div>
          ) : null}
          {loginTransitionBackground ? (
            <div className="client-scene-overlay login-transition-overlay" aria-hidden="true">
              <img
                className="client-scene-background"
                src={loginTransitionBackground}
                alt=""
                draggable={false}
              />
            </div>
          ) : null}
          <div
            className={`game-world-composite ${screen === "game" ? "" : "inactive"}`}
            aria-hidden={screen !== "game"}
          >
            {showSyntheticScene ? <div className="game-scene-underlay" /> : null}
            <canvas
              id="mir2-web3-canvas"
              className={hideBevyCanvasForDomEntityFallback ? "bevy-canvas-hidden" : undefined}
            />
            <WebGl2MapAtlasLayer
              enabled={mapGpuActive}
              stageWidth={ORIGINAL_UI.game.sceneWidth}
              stageHeight={ORIGINAL_UI.game.sceneHeight}
              index={mapAtlasIndex}
              tiles={mapTileDrawList}
              onDebugChange={handleMapAtlasDebug}
            />
            <WebGl2EntityAtlasLayer
              enabled={useWebGl2EntityAtlasRenderer && Boolean(activeBevyEntityAtlas)}
              state={entityRenderState}
              onDebugChange={handleWebGl2EntityAtlasDebug}
            />
            {screen === "game" ? (
              <GameSceneBackdrop
                world={world}
                player={renderPlayer ?? player}
                floorSprites={mapDomSprites.floor}
                cameraOffset={effectiveCameraOffset}
                imperativeCamera={imperativeSceneMotion}
                registerCameraSurface={sceneMotionDriver.registerCameraSurface}
              />
            ) : null}

            {tileHitGrid}

            <OriginalClientSceneVisualLayers
              screen={screen}
              t={t}
              world={world}
              player={renderPlayer ?? player}
              selectedEntity={selectedEntity}
              viewportGroundDrops={viewportGroundDrops}
              viewportMapSprites={mapDomSprites}
              viewportEntitySprites={viewportEntitySprites}
              viewportProjectiles={viewportProjectiles}
              viewportDepthPlayer={viewportDepthPlayer}
              playerCameraMotionOffset={effectiveCameraOffset}
              entityMotionSnapshots={entityMotionSnapshotsRef.current}
              motionNow={motionNow}
              imperativeCamera={imperativeSceneMotion}
              registerCameraSurface={sceneMotionDriver.registerCameraSurface}
              registerEntityEl={sceneMotionDriver.registerEntityEl}
              sceneSpriteFrameIndex={sceneSpriteFrameIndex}
              useBevyEntityRenderer={hideDomEntitySpritesForBevy}
              entityKindClassName={entityKindClassName}
              onPickGroundDrop={onPickGroundDrop}
              onActivateEntity={onActivateEntity}
            />
          </div>
          <OriginalClientSceneOverlays
            screen={screen}
            t={t}
            player={renderPlayer ?? player}
            selectedEntity={selectedEntity}
            viewportEntitySprites={viewportEntitySprites}
            playerCameraMotionOffset={effectiveCameraOffset}
            entityMotionSnapshots={entityMotionSnapshotsRef.current}
            motionNow={motionNow}
            imperativeCamera={imperativeSceneMotion}
            registerCameraSurface={sceneMotionDriver.registerCameraSurface}
            registerEntityEl={sceneMotionDriver.registerEntityEl}
            chatBubbles={sceneChatBubbles}
            damageFloaters={world.damageFloaters}
            targetActionLabel={selectedTargetReadoutLabel}
            entityKindClassName={entityKindClassName}
          />
          {screen === "login" ? (
            <LoginOverlay
              language={language}
              t={t}
              runtimePhase={runtimePhaseLabel}
              runtimeMessage={runtimeMessageLabel}
              wsState={wsState}
              accountId={accountId}
              password={password}
              loginBusy={loginBusy}
              loginError={loginError}
              suiWallets={suiWallets}
              walletPickerOpen={walletPickerOpen}
              dubheWalletUrl={dubheWalletUrl}
              onLanguageChange={onLanguageChange}
              onAccountIdChange={onAccountIdChange}
              onPasswordChange={onPasswordChange}
              onCreateAccount={onCreateAccount}
              onSubmitLogin={onSubmitLogin}
              onPasskeyLogin={onPasskeyLogin}
              onWalletPickerToggle={onWalletPickerToggle}
              onWalletLogin={onWalletLogin}
              onQuickEnter={onQuickEnter}
              onResetClient={onResetClient}
            />
          ) : null}
          {screen === "select" ? (
            <SelectOverlay
              language={language}
              t={t}
              characters={characters}
              selectedCharacterIndex={selectedCharacterIndex}
              accountId={accountId}
              selectedPortraitFrames={selectedPortraitFrames}
              onLanguageChange={onLanguageChange}
              onSelectCharacter={onSelectCharacter}
              onEnterWorld={onEnterWorld}
              onCreateCharacter={onCreateCharacter}
              onDeleteCharacter={onDeleteCharacter}
              onExit={onExitSelect}
            />
          ) : null}
          {screen === "game"
            ? (() => {
                // Shared HUD props (everything except `world`). Declared once so the
                // legacy prop path and the Stage-5c store-bound path stay in lockstep.
                const gameUiSharedProps = {
                  t,
                  locale,
                  runtimeMessage: runtimeMessageLabel,
                  player,
                  logs,
                  chatMessage,
                  showInventory,
                  showCharacter,
                  activeInventoryTab,
                  activeCharacterTab,
                  storageServiceOpenVersion,
                  defaultChatExpanded: clientProfile.layout !== "touch",
                  onChatMessageChange,
                  onSendChat,
                  onRequestTrade,
                  onRentExpandedStorage,
                  onLogout,
                  onToggleCharacter,
                  onToggleInventory,
                  onCloseCharacter,
                  onCloseInventory,
                  onOpenCharacterTab,
                  onOpenInventoryTab,
                  onSelectNpcDialogTarget,
                  onSubmitNpcInput,
                  onUseItem,
                  onDropItem,
                  onEquipItem,
                  onRemoveItem,
                  onMoveItem,
                  onMergeItem,
                  onSplitItem,
                  onStoreItem,
                  onTakeBackItem,
                  onUnlockStorage,
                  onSetStoragePassword,
                  onRemoveStoragePassword,
                  onSellItem,
                  onDropGold,
                  onRepairItem,
                  onSpecialRepairItem,
                  onCastSkill,
                  onTransferMap,
                  onClaimMail,
                  onDeleteMail,
                  onBuyGameShopItem,
                  onRunStage5Command,
                  onSendClientCommand,
                  transferOptions,
                };
                // Stage 5c: opt-in store-bound HUD (subscribes to `world` slices via
                // useWorldSelector). Defaults OFF — when the flag is absent/false (or no
                // store was threaded) this is byte-identical to the legacy `world={world}`
                // prop path below.
                return selectorHud && worldStore ? (
                  <GameUiSceneStoreBound store={worldStore} {...gameUiSharedProps} />
                ) : (
                  <GameUiScene world={world} {...gameUiSharedProps} />
                );
              })()
            : null}
          {screen !== "login" && reconnectMessage ? (
            <div
              className={`gateway-reconnect-overlay ${reconnectStatus.mode}`}
              role="status"
              aria-live="polite"
              data-reconnect-mode={reconnectStatus.mode}
            >
              <span className="gateway-reconnect-dot" aria-hidden="true" />
              <span className="gateway-reconnect-text">{reconnectMessage}</span>
            </div>
          ) : null}
          {screen === "game" && !sceneInteractionReady ? (
            <div className="scene-loading-overlay" role="status" aria-live="polite">
              <span className="scene-loading-spinner" aria-hidden="true" />
              <span className="scene-loading-text">{sceneLoadingLabel}</span>
            </div>
          ) : null}
        </div>
      </section>
      {clientProfile.layout === "tv" && clientProfile.input === "gamepad" && screen !== "game" ? (
        <div className="mir-gamepad-hint" role="status">
          {t("ui.gamepadNavigationHint", [], "D-pad: Navigate · A: Select · B: Back")}
        </div>
      ) : null}
      <OriginalClientMobileControls
        enabled={screen === "game" && sceneInteractionReady}
        forceVisible={clientProfile.layout === "touch" && clientProfile.input === "touch"}
        t={t}
        world={world}
        player={player}
        selectedEntity={selectedEntity}
        onDirectionIntent={onViewportDirectionIntent}
        onDirectionStop={onViewportDirectionStop}
        onPrimaryTargetAction={onPrimaryTargetAction}
        onApproachTarget={onApproachTarget}
        onPickGroundDrop={onPickGroundDrop}
        onToggleInventory={onToggleInventory}
        onToggleCharacter={onToggleCharacter}
        onCastSkill={onCastSkill}
        onUseItem={onUseItem}
      />
      <OriginalClientGamepadControls
        enabled={clientProfile.input === "gamepad"}
        screen={screen}
        gameplayReady={screen === "game" && sceneInteractionReady}
        stageRootRef={stageFrameRef}
        world={world}
        player={player}
        onDirectionIntent={onViewportDirectionIntent}
        onDirectionStop={onViewportDirectionStop}
        onPrimaryTargetAction={onPrimaryTargetAction}
        onApproachTarget={onApproachTarget}
        onPickGroundDrop={onPickGroundDrop}
        onToggleInventory={onToggleInventory}
        onToggleCharacter={onToggleCharacter}
        onCastSkill={onCastSkill}
        onUseItem={onUseItem}
      />
    </main>
  );
}

// Strip the leading "[HH:MM:SS] " stamp createLogLine adds so the bubble shows only the speech.
function stripLogTimestamp(text: string): string {
  return text.replace(/^\[\d{1,2}:\d{2}:\d{2}(?:\s?[AP]M)?\]\s*/i, "");
}

// Pull "Speaker: words" out of a chat line. Mir chat lines are emitted as "<name>: <message>"; the
// portion before the first colon is the speaker we try to anchor the bubble to. Returns null when
// the line has no speaker prefix (server notices, hints, etc.).
function parseChatLine(text: string): { speaker: string; text: string } | null {
  const stripped = stripLogTimestamp(text).trim();
  const colon = stripped.indexOf(":");
  if (colon <= 0) {
    return null;
  }
  const speaker = stripped.slice(0, colon).trim();
  const message = stripped.slice(colon + 1).trim();
  // Reject prefixes that are clearly not a name (timestamps already stripped, but guard URLs etc.).
  if (!speaker || !message || speaker.length > 24 || /\s{2,}/.test(speaker)) {
    return null;
  }
  return { speaker, text: message };
}

// Derive the currently-visible over-head chat bubbles from the chat log + on-screen entities,
// mutating `state` so each (speaker,text) pair keeps a stable firstSeenAt across renders. Expiry is
// driven purely by `now` (the shell's motion clock), so no timer is required.
function deriveChatBubbles(
  logs: DisplayLogLine[],
  viewportEntities: Array<DisplayEntity & { dx: number; dy: number }>,
  state: Map<string, ChatBubbleRecord>,
  now: number,
): SceneChatBubble[] {
  // Index visible speakers by lower-cased name (names are unique on screen for our purposes).
  const entityByName = new Map<string, DisplayEntity & { dx: number; dy: number }>();
  for (const entity of viewportEntities) {
    if (entity.name) {
      entityByName.set(entity.name.toLowerCase(), entity);
    }
  }

  // Walk the chat-tone lines newest-first and remember the most recent line per speaker.
  const latestBySpeaker = new Map<string, { text: string; channel: string }>();
  for (const line of logs) {
    if (line.tone !== "chat" || !CHAT_BUBBLE_CHANNELS.has(line.channel)) {
      continue;
    }
    const parsed = parseChatLine(line.text);
    if (!parsed) {
      continue;
    }
    const key = parsed.speaker.toLowerCase();
    if (!entityByName.has(key) || latestBySpeaker.has(key)) {
      continue;
    }
    latestBySpeaker.set(key, { text: parsed.text, channel: line.channel });
  }

  // Reconcile records: stamp firstSeenAt for newly-said lines, refresh text for ongoing ones.
  for (const [key, { text, channel }] of latestBySpeaker) {
    const existing = state.get(key);
    if (existing && existing.text === text) {
      continue;
    }
    state.set(key, { speaker: key, text, channel, firstSeenAt: now });
  }

  // Emit active (non-expired) bubbles whose speaker is still on screen; prune the rest.
  const bubbles: SceneChatBubble[] = [];
  for (const [key, record] of state) {
    const entity = entityByName.get(key);
    if (!entity || now - record.firstSeenAt > CHAT_BUBBLE_TTL_MS) {
      state.delete(key);
      continue;
    }
    bubbles.push({
      objectId: entity.objectId,
      text: record.text,
      channel: record.channel,
      firstSeenAt: record.firstSeenAt,
    });
  }
  return bubbles;
}

const SCENE_INTERACTION_PRELOAD_URL_LIMIT = 512;
const SCENE_INTERACTION_ENTITY_PRELOAD_URL_LIMIT = 96;
const SCENE_INTERACTION_ENTITY_PRELOAD_PATHS_PER_SPRITE = 64;
const SCENE_INTERACTION_MIN_PRELOADED_URLS = 24;
// Safety valve: once the scene is rendered (map + player), the "Loading map…" overlay
// must clear within this window even if some sprite library / tile is still resolving.
// In a populated map world.entities streams in continuously, so the "all desired
// libraries ready" gate may never settle; without this bound the overlay can hang
// forever while the world is already drawn behind it. Remaining assets keep loading.
const SCENE_READINESS_SAFETY_MS = 3000;
const EMPTY_MAP_TILE_DRAW_LIST: MapTileDraw[] = [];
const EMPTY_STANDALONE_MAP_TILE_DRAW_LIST: MapStandaloneTileDraw[] = [];
const EMPTY_STANDALONE_MAP_IMAGE_SOURCES: MapStandaloneTileImageSource[] = [];
const EMPTY_STRING_SET: ReadonlySet<string> = new Set();
const STANDALONE_MAP_IMAGE_NEGATIVE_CACHE_MS = 30 * 1000;
const STANDALONE_MAP_IMAGE_CACHE_LIMIT = 192;

function mapTileDrawListsEqual(left: MapTileDraw[], right: MapTileDraw[]) {
  if (left === right) return true;
  if (left.length !== right.length) return false;
  return left.every((tile, index) => {
    const other = right[index];
    return (
      tile.key === other.key &&
      tile.atlasKey === other.atlasKey &&
      tile.rectKey === other.rectKey &&
      tile.left === other.left &&
      tile.top === other.top &&
      tile.width === other.width &&
      tile.height === other.height &&
      tile.z === other.z &&
      tile.opacity === other.opacity
    );
  });
}

function standaloneMapTileDrawListsEqual(
  left: MapStandaloneTileDraw[],
  right: MapStandaloneTileDraw[],
) {
  if (left === right) return true;
  if (left.length !== right.length) return false;
  return left.every((tile, index) => {
    const other = right[index];
    return (
      tile.key === other.key &&
      tile.imageKey === other.imageKey &&
      tile.left === other.left &&
      tile.top === other.top &&
      tile.width === other.width &&
      tile.height === other.height &&
      tile.z === other.z &&
      tile.opacity === other.opacity &&
      tile.additive === other.additive
    );
  });
}

function standaloneMapImageSourcesEqual(
  left: MapStandaloneTileImageSource[],
  right: MapStandaloneTileImageSource[],
) {
  if (left === right) return true;
  if (left.length !== right.length) return false;
  return left.every((source, index) => {
    const other = right[index];
    return (
      source.imageKey === other.imageKey &&
      source.fetchUrl === other.fetchUrl &&
      source.alphaKeyMapObject === other.alphaKeyMapObject
    );
  });
}

// Off-screen tile prefetch ring: how many cells beyond the visible viewport to warm.
const SCENE_TILE_PREFETCH_RING_CELLS = 6;
const SCENE_TILE_PREFETCH_TIMEOUT_MS = 8_000;

function trimStandaloneMapImageCache(
  cache: Map<string, DecodedStandaloneMapImage>,
  keepKeys: Set<string>,
) {
  if (cache.size <= STANDALONE_MAP_IMAGE_CACHE_LIMIT) {
    return [];
  }
  const evicted: string[] = [];
  for (const key of cache.keys()) {
    if (cache.size <= STANDALONE_MAP_IMAGE_CACHE_LIMIT) {
      break;
    }
    if (keepKeys.has(key)) {
      continue;
    }
    cache.delete(key);
    evicted.push(key);
  }
  return evicted;
}

function standaloneMapImageSourceSignature(source: MapStandaloneTileImageSource) {
  return `${source.fetchUrl}\0${source.alphaKeyMapObject ? "alpha-key" : "raw"}`;
}

function setsEqual(left: ReadonlySet<string>, right: ReadonlySet<string>) {
  if (left.size !== right.size) {
    return false;
  }
  for (const value of left) {
    if (!right.has(value)) {
      return false;
    }
  }
  return true;
}

function setIntersects(left: ReadonlySet<string>, right: ReadonlySet<string>) {
  for (const value of left) {
    if (right.has(value)) {
      return true;
    }
  }
  return false;
}

function countBlendMapSprites(sprites: ViewportMapSprites) {
  return (
    sprites.floor.reduce((count, sprite) => count + (resolvedMapSpriteBlendMode(sprite) ? 1 : 0), 0) +
    sprites.objects.reduce((count, sprite) => count + (resolvedMapSpriteBlendMode(sprite) ? 1 : 0), 0)
  );
}

function collectVisibleSceneAssetUrls(
  viewportMapSprites: {
    floor: Array<{ path: string; kind?: string; left: number; top: number; width: number; height: number }>;
    objects: Array<{ path: string; kind?: string; left: number; top: number; width: number; height: number }>;
  },
  viewportEntitySprites: Array<{
    sprite: {
      body: { path: string } | null;
      hair: { path: string } | null;
      rearWeapons: Array<{ path: string }>;
      frontWeapons: Array<{ path: string }>;
      preloadPaths?: string[];
    } | null;
  }>,
  options: { includeEntityPreloadPaths?: boolean } = {},
) {
  const sceneCenterX = ORIGINAL_UI.game.sceneWidth / 2;
  const sceneCenterY = ORIGINAL_UI.game.sceneHeight / 2;
  const rankedMapUrls = [
    ...viewportMapSprites.objects.map((sprite) => ({
      ...sprite,
      path: mapSpriteRenderPath(sprite.path),
      preloadPriority: 0,
    })),
    ...viewportMapSprites.floor.map((sprite) => ({
      ...sprite,
      path: sprite.path,
      preloadPriority: sceneMapSpritePreloadPriority(sprite.kind),
    })),
  ]
    .sort((a, b) => {
      const aCenterX = a.left + a.width / 2;
      const aCenterY = a.top + a.height / 2;
      const bCenterX = b.left + b.width / 2;
      const bCenterY = b.top + b.height / 2;
      return (
        a.preloadPriority - b.preloadPriority ||
        Math.hypot(aCenterX - sceneCenterX, aCenterY - sceneCenterY) -
        Math.hypot(bCenterX - sceneCenterX, bCenterY - sceneCenterY)
      );
    })
    .slice(0, SCENE_INTERACTION_PRELOAD_URL_LIMIT)
    .map((sprite) => sprite.path);
  const entityUrls: string[] = [];
  const addEntityUrl = (url: string | null | undefined) => {
    if (!url || entityUrls.includes(url)) {
      return false;
    }
    entityUrls.push(url);
    return true;
  };

  for (const { sprite } of viewportEntitySprites) {
    if (!sprite) continue;

    addEntityUrl(sprite.body?.path);
    addEntityUrl(sprite.hair?.path);
    for (const weapon of sprite.rearWeapons) addEntityUrl(weapon.path);
    for (const weapon of sprite.frontWeapons) addEntityUrl(weapon.path);

    if (!options.includeEntityPreloadPaths) {
      continue;
    }

    let spritePreloadCount = 0;
    for (const path of sprite.preloadPaths ?? []) {
      if (spritePreloadCount >= SCENE_INTERACTION_ENTITY_PRELOAD_PATHS_PER_SPRITE) break;
      if (entityUrls.length >= SCENE_INTERACTION_ENTITY_PRELOAD_URL_LIMIT) break;
      if (addEntityUrl(path)) spritePreloadCount += 1;
    }

    if (entityUrls.length >= SCENE_INTERACTION_ENTITY_PRELOAD_URL_LIMIT) {
      break;
    }
  }

  return [
    ...rankedMapUrls,
    ...entityUrls,
  ].filter((url, index, list): url is string => Boolean(url) && list.indexOf(url) === index);
}

function stableSceneAssetUrlKey(urls: string[]) {
  if (!urls.length) return "empty";
  let hash = 0x811c9dc5;
  const uniqueSorted = Array.from(new Set(urls)).sort();
  for (const url of uniqueSorted) {
    for (let index = 0; index < url.length; index += 1) {
      hash ^= url.charCodeAt(index);
      hash = Math.imul(hash, 0x01000193) >>> 0;
    }
    hash ^= 0xff;
    hash = Math.imul(hash, 0x01000193) >>> 0;
  }
  return `${uniqueSorted.length}:${hash.toString(16).padStart(8, "0")}`;
}

function sceneMapSpritePreloadPriority(kind: string | undefined) {
  if (kind === "front") return 1;
  if (kind === "middle" || kind === "tileAnimation") return 2;
  return 3;
}

function shouldUseBevyEntityRenderer() {
  if (typeof window === "undefined") {
    return false;
  }

  const params = new URLSearchParams(window.location.search);
  if (
    params.get("bevyEntities") === "0" ||
    params.get("domEntities") === "1" ||
    params.get("bevyCanvas") === "0" ||
    params.get("bevyCanvasHidden") === "1"
  ) {
    return false;
  }
  if (params.get("bevyEntities") === "1" || params.get("bevyCanvas") === "1") {
    return true;
  }

  return window.localStorage.getItem("mir2-dom-entities") !== "1";
}

function shouldUseDomEntityFallback(isTouchOrMobile: boolean) {
  if (typeof window === "undefined") {
    return false;
  }

  const params = new URLSearchParams(window.location.search);
  if (params.get("bevyEntities") === "1" || params.get("bevyCanvas") === "1") {
    return false;
  }
  if (
    params.get("bevyEntities") === "0" ||
    params.get("domEntities") === "1" ||
    params.get("bevyCanvas") === "0" ||
    params.get("bevyCanvasHidden") === "1"
  ) {
    return true;
  }
  if (
    window.localStorage.getItem("mir2-dom-entities") === "1" ||
    window.localStorage.getItem("mir2-bevy-canvas") === "0"
  ) {
    return true;
  }

  if (!isTouchOrMobile) {
    return false;
  }
  if (params.get("bevyMobileEntities") === "1") {
    return false;
  }
  if (params.get("bevyMobileEntities") === "0") {
    return true;
  }
  // Default for touch/mobile devices: use the GPU entity renderer when the device reports
  // enough capability (memory, CPU cores, WebGL2 support), otherwise keep the lighter DOM
  // sprite renderer. This replaces the previous blanket "mobile => DOM" default so capable
  // phones/tablets get the higher-fidelity renderer, while low-end devices stay protected.
  return !mobileDeviceCanRunGpuEntities();
}

// Capability probe for the GPU entity renderer on touch/mobile devices. Memoised because the
// WebGL2 support check allocates a GL context, and creating one per render would exhaust the
// browser's live-context budget and break the real renderer. Device capability is stable for
// the session, so a single probe is sufficient.
let cachedMobileGpuEntityCapability: boolean | null = null;
function mobileDeviceCanRunGpuEntities() {
  if (cachedMobileGpuEntityCapability !== null) {
    return cachedMobileGpuEntityCapability;
  }
  cachedMobileGpuEntityCapability = computeMobileGpuEntityCapability();
  return cachedMobileGpuEntityCapability;
}

function computeMobileGpuEntityCapability() {
  if (typeof navigator === "undefined" || typeof document === "undefined") {
    return false;
  }
  const deviceMemory = (navigator as Navigator & { deviceMemory?: number }).deviceMemory;
  if (typeof deviceMemory === "number" && deviceMemory > 0 && deviceMemory < 4) {
    return false;
  }
  const cores = navigator.hardwareConcurrency;
  if (typeof cores === "number" && cores > 0 && cores < 4) {
    return false;
  }
  try {
    const canvas = document.createElement("canvas");
    const gl = canvas.getContext("webgl2");
    if (!gl) {
      return false;
    }
    // Release the probe context immediately so it does not count against the live-context budget.
    gl.getExtension("WEBGL_lose_context")?.loseContext();
    return true;
  } catch {
    return false;
  }
}

function shouldUseBevyEntityAtlas() {
  if (typeof window === "undefined") {
    return false;
  }

  const params = new URLSearchParams(window.location.search);
  if (params.get("bevyAtlas") === "0" || window.localStorage.getItem("mir2-bevy-atlas") === "0") {
    return false;
  }
  return true;
}

function shouldUseRawWebGl2EntityRenderer() {
  if (typeof window === "undefined") {
    return false;
  }

  const params = new URLSearchParams(window.location.search);
  if (
    params.get("webgl2Entities") === "0" ||
    params.get("webgl2Atlas") === "0" ||
    window.localStorage.getItem("mir2-webgl2-entities") === "0"
  ) {
    return false;
  }
  return true;
}

// Whether per-render scene motion diagnostics are written to window.__mir2SceneMotionDebug.
// Enabled only with ?mir2Debug=1 so the object-allocation is never paid in normal play.
const isSceneMotionDebugMode: boolean =
  typeof window !== "undefined" && new URLSearchParams(window.location.search).get("mir2Debug") === "1";

// Default-on Crystal feel path: request Bevy self-camera + per-entity interpolation,
// but activate it only when the Bevy entity/map renderer is actually live. Escape
// hatches: ?bevySelfCamera=0 and ?bevyEntityInterp=0, or matching localStorage keys.
const BEVY_SELF_CAMERA_REQUESTED: boolean = clientFeatureFlagEnabled(
  "bevySelfCamera",
  "mir2-bevy-self-camera",
  true,
);
const BEVY_ENTITY_INTERP_REQUESTED: boolean = clientFeatureFlagEnabled(
  "bevyEntityInterp",
  "mir2-bevy-entity-interp",
  true,
);
const BEVY_REMOTE_MOTION_REQUESTED: boolean = clientFeatureFlagEnabled(
  "bevyRemoteMotion",
  "mir2-bevy-remote-motion",
  true,
);
const BEVY_PRESENTATION_POSE_REQUESTED: boolean = clientFeatureFlagEnabled(
  "bevyPresentationPose",
  "mir2-bevy-presentation-pose",
  true,
);
// Native-temporal parity candidate: matched local commands now use the same
// shared 100 ms Bevy scene pulse by default. Corrections, degraded runs and
// path/target mismatches remain TypeScript-owned, and both switches retain
// explicit query/localStorage rollback values.
const BEVY_LOCAL_MOTION_REQUESTED: boolean = clientFeatureFlagEnabled(
  "bevyLocalMotion",
  "mir2-bevy-local-motion",
  true,
);
const BEVY_POSE_COMMIT_REQUESTED: boolean = clientFeatureFlagEnabled(
  "bevyPoseCommit",
  "mir2-bevy-pose-commit",
  true,
);

function clientFeatureFlagEnabled(queryKey: string, storageKey: string, defaultValue: boolean) {
  if (typeof window === "undefined") {
    return false;
  }
  const params = new URLSearchParams(window.location.search);
  const queryValue = params.get(queryKey);
  if (queryValue === "0") return false;
  if (queryValue === "1") return true;
  const storedValue = window.localStorage.getItem(storageKey);
  if (storedValue === "0") return false;
  if (storedValue === "1") return true;
  return defaultValue;
}

function resolveBevyEntityAtlasBudgetProfile(): BevyEntityAtlasBudgetProfile {
  if (typeof window === "undefined") {
    return {
      tier: "medium",
      memoryEntries: 16,
      memoryBytes: 160 * 1024 * 1024,
      persistentEntries: 6,
      persistentBytes: 256 * 1024 * 1024,
      deviceMemoryGiB: null,
    };
  }

  const forcedTier = new URLSearchParams(window.location.search).get("renderTier");
  const deviceMemoryGiB = normalizeDeviceMemoryGiB(
    (navigator as Navigator & { deviceMemory?: number }).deviceMemory,
  );
  const coarsePointer = window.matchMedia?.("(pointer: coarse)").matches ?? false;
  const tier = resolveRenderTier({ forcedTier, deviceMemoryGiB, coarsePointer });

  if (tier === "low") {
    const memoryBytes = deviceMemoryGiB !== null && deviceMemoryGiB <= 2 ? 64 : 96;
    return {
      tier,
      memoryEntries: 8,
      memoryBytes: memoryBytes * 1024 * 1024,
      persistentEntries: 3,
      persistentBytes: 128 * 1024 * 1024,
      deviceMemoryGiB,
    };
  }
  if (tier === "high") {
    return {
      tier,
      memoryEntries: 24,
      memoryBytes: 256 * 1024 * 1024,
      persistentEntries: 8,
      persistentBytes: 512 * 1024 * 1024,
      deviceMemoryGiB,
    };
  }
  return {
    tier,
    memoryEntries: 16,
    memoryBytes: 160 * 1024 * 1024,
    persistentEntries: 6,
    persistentBytes: 256 * 1024 * 1024,
    deviceMemoryGiB,
  };
}

function disabledEntityRenderState(state: BevyEntityRenderState): BevyEntityRenderState {
  return {
    enabled: false,
    stageWidth: state.stageWidth,
    stageHeight: state.stageHeight,
    atlases: [],
    atlasImages: [],
    entities: [],
  };
}

function buildBevyEntityRenderState({
  enabled,
  player,
  viewportEntitySprites,
  viewportDepthPlayer,
  playerCameraMotionOffset,
  entityMotionSnapshots,
  motionNow,
  atlas,
  bevySelfCameraActive,
  bevyEntityInterpActive,
}: {
  enabled: boolean;
  player: DisplayEntity | null;
  viewportEntitySprites: Array<{
    entity: DisplayEntity & { dx: number; dy: number };
    sprite: {
      mount: { path: string; x: number; y: number; width: number; height: number } | null;
      body: { path: string; x: number; y: number; width: number; height: number } | null;
      hair: { path: string; x: number; y: number; width: number; height: number } | null;
      rearWeapons: Array<{ path: string; x: number; y: number; width: number; height: number }>;
      frontWeapons: Array<{ path: string; x: number; y: number; width: number; height: number }>;
    } | null;
  }>;
  viewportDepthPlayer: Pick<DisplayEntity, "x" | "y">;
  playerCameraMotionOffset: ViewportOffset;
  entityMotionSnapshots: Record<string, EntityMotionSnapshot>;
  motionNow: number;
  atlas: BevyEntityAtlasSnapshot | null;
  bevySelfCameraActive: boolean;
  bevyEntityInterpActive: boolean;
}): BevyEntityRenderState {
  if (!enabled || !player) {
    return {
      enabled: false,
      stageWidth: 1024,
      stageHeight: 768,
      atlases: [],
      entities: [],
    };
  }

  const centerAnchor =
    viewportEntitySprites.find(({ entity }) => entity.objectId === player.objectId)?.entity ??
    viewportEntitySprites[0]?.entity;
  const centerX = centerAnchor ? centerAnchor.x - centerAnchor.dx : player.x;
  const centerY = centerAnchor ? centerAnchor.y - centerAnchor.dy : player.y;

  // Normalise to a page list: real multi-page snapshots expose `pages`; single-
  // page (live / single-page prebuilt) snapshots synthesise one page from the
  // top-level fields, so the output below is identical to the pre-multi-page form.
  const atlasPages: BevyEntityAtlasPage[] = atlas
    ? atlas.pages && atlas.pages.length
      ? atlas.pages
      : [
          {
            key: atlas.key,
            width: atlas.width,
            height: atlas.height,
            imageUrl: atlas.imageUrl,
            pixels: atlas.pixels,
            rectList: atlas.rectList,
          },
        ]
    : [];

  return {
    enabled: true,
    stageWidth: 1024,
    stageHeight: 768,
    centerX,
    centerY,
    // One render-atlas per texture page; the runtime registers each by key and
    // resolves each layer to its page via the layer's atlasKey.
    atlases: atlasPages.map((page) => ({
      key: page.key,
      width: page.width,
      height: page.height,
      imageUrl: page.imageUrl,
      rects: page.rectList,
    })),
    // Live/persistent pages carry RGBA pixels; prebuilt pages carry an imageUrl
    // (above) and are loaded by the runtime, so they contribute no atlasImage.
    atlasImages: atlasPages.flatMap((page) =>
      page.pixels && page.pixels.byteLength > 0
        ? [
            {
              key: page.key,
              width: page.width,
              height: page.height,
              pixels: page.pixels,
            },
          ]
        : [],
    ),
    entities: viewportEntitySprites.map(({ entity, sprite }) => {
      const isPlayer = player.objectId === entity.objectId;
      // Bevy-entity-interp path: stop folding the sub-cell glide here (push cell-space)
      // and ship the motion window below so Bevy interpolates the glide at display Hz.
      // When the self-camera is on we ALSO interp the self sprite in Bevy (not just the
      // camera): the camera and the self sprite then share one display-Hz curve so the
      // player stays pinned to centre, and entityRenderState no longer depends on
      // `motionNow` — which is what lets the React motion clock drop to ~10 Hz.
      const interpEntityInBevy = bevyEntityInterpActive && (!isPlayer || bevySelfCameraActive);
      // Bevy-self-camera path: drop the JS camera fold (Bevy moves the camera at
      // display Hz) and let the self player glide via its own motion. Default path
      // is unchanged (player pinned via EMPTY, others fold playerCameraMotionOffset).
      const entityMotionOffset =
        (isPlayer && !bevySelfCameraActive) || interpEntityInBevy
          ? EMPTY_VIEWPORT_OFFSET
          : entityMotionOffsetForEntity(entity, entityMotionSnapshots, motionNow);
      const cameraOffset =
        bevySelfCameraActive || isPlayer ? EMPTY_VIEWPORT_OFFSET : playerCameraMotionOffset;
      // Step-stable window (changes only at step boundaries, not per motionNow
      // tick) handed to Bevy for the non-self interpolation. undefined ⇒ no fields
      // spread ⇒ serialized state byte-identical to today.
      const interpMotion = interpEntityInBevy ? entityMotionSnapshots[entity.objectId] : undefined;
      const rootLeft =
        VIEWPORT_ENTITY_LEFT_ORIGIN + entity.dx * VIEWPORT_CELL_WIDTH + cameraOffset.x + entityMotionOffset.x;
      const rootTop =
        VIEWPORT_ENTITY_TOP_ORIGIN + entity.dy * VIEWPORT_CELL_HEIGHT + cameraOffset.y + entityMotionOffset.y;
      const depth = viewportDepthForCell(entity.x, entity.y, viewportDepthPlayer, 64);
      const layers = sprite
        ? [
            // Crystal draws the mount first, beneath every other layer (`DrawMount`).
            ...(sprite.mount ? [{ layer: sprite.mount, role: "mount", index: 0 }] : []),
            ...sprite.rearWeapons.map((layer, index) => ({ layer, role: "rearWeapon", index })),
            ...(sprite.body ? [{ layer: sprite.body, role: "body", index: 0 }] : []),
            ...(sprite.hair ? [{ layer: sprite.hair, role: "hair", index: 0 }] : []),
            ...sprite.frontWeapons.map((layer, index) => ({ layer, role: "frontWeapon", index })),
          ].map(({ layer, role, index }, order) => {
            const atlasRectKey = bevyEntityAtlasRectKey(layer.path, layer.width, layer.height);
            const atlasRect = atlas?.rects[atlasRectKey];
            // Route the layer to the page its frame lives on (multi-page); for
            // single-page snapshots pageIndex is 0 ⇒ the sole page.
            const atlasPageKey = atlasRect ? atlasPages[atlasRect.pageIndex ?? 0]?.key : undefined;
            return {
              key: `${entity.objectId}:${role}:${index}`,
              path: layer.path,
              ...(atlasRect && atlasPageKey
                ? {
                    atlasKey: atlasPageKey,
                    atlasRectKey,
                  }
                : {}),
              left: rootLeft + layer.x,
              top: rootTop + layer.y,
              width: layer.width,
              height: layer.height,
              z: depth * 10 + order,
              opacity: entity.dead ? 0.45 : 1,
            };
          })
        : [];

      return {
        objectId: entity.objectId,
        dead: Boolean(entity.dead),
        isSelf: isPlayer,
        gridX: entity.x,
        gridY: entity.y,
        ...(interpMotion
          ? {
              motionFromX: interpMotion.fromX,
              motionFromY: interpMotion.fromY,
              motionToX: interpMotion.toX,
              motionToY: interpMotion.toY,
              motionStartedMs: interpMotion.startedAt,
              motionDurationMs: interpMotion.expiresAt - interpMotion.startedAt,
            }
          : {}),
        layers,
      };
    }),
  };
}

function collectBevyEntityAtlasSources(
  viewportEntitySprites: Array<{
    sprite: {
      mount: { path: string; width: number; height: number } | null;
      body: { path: string; width: number; height: number } | null;
      hair: { path: string; width: number; height: number } | null;
      rearWeapons: Array<{ path: string; width: number; height: number }>;
      frontWeapons: Array<{ path: string; width: number; height: number }>;
      preloadFrames?: Array<{ path: string; width: number; height: number }>;
    } | null;
  }>,
): BevyEntityAtlasSource[] {
  const sources = new Map<string, BevyEntityAtlasSource>();
  const addLayer = (layer: { path: string; width: number; height: number } | null | undefined) => {
    if (!layer?.path || layer.width <= 0 || layer.height <= 0) {
      return;
    }
    const key = bevyEntityAtlasRectKey(layer.path, layer.width, layer.height);
    if (!sources.has(key)) {
      sources.set(key, {
        key,
        path: layer.path,
        width: Math.max(1, Math.ceil(layer.width)),
        height: Math.max(1, Math.ceil(layer.height)),
      });
    }
  };

  for (const { sprite } of viewportEntitySprites) {
    if (!sprite) continue;
    addLayer(sprite.mount);
    sprite.rearWeapons.forEach(addLayer);
    addLayer(sprite.body);
    addLayer(sprite.hair);
    sprite.frontWeapons.forEach(addLayer);
    sprite.preloadFrames?.forEach(addLayer);
  }
  return [...sources.values()].sort((a, b) => a.key.localeCompare(b.key));
}

function bevyEntityAtlasRectKey(path: string, width: number, height: number) {
  return `${path}|${Math.max(1, Math.ceil(width))}x${Math.max(1, Math.ceil(height))}`;
}

function bevyEntityAtlasKeyForSources(sources: BevyEntityAtlasSource[]) {
  const sourceKey = sources.map((source) => `${source.key}`).join("\n");
  return `entity-atlas-${hashString(sourceKey)}`;
}

async function resolveBevyEntityAtlasSnapshot(
  sources: BevyEntityAtlasSource[],
  key: string,
): Promise<BevyEntityAtlasResolveResult> {
  const persistent = await loadPersistedBevyEntityAtlas(key);
  if (persistent) {
    return {
      atlas: persistent,
      source: "persistent",
    };
  }

  const prebuilt = await loadPrebuiltBevyEntityAtlasSnapshot(sources, key);
  if (prebuilt) {
    void persistBevyEntityAtlas(prebuilt);
    return {
      atlas: prebuilt,
      source: "prebuilt",
      prebuiltKey: prebuilt.sourceKey ?? null,
    };
  }

  const live = await buildBevyEntityAtlasSnapshot(sources, key);
  void persistBevyEntityAtlas(live);
  return {
    atlas: live,
    source: "live",
  };
}

async function loadPrebuiltBevyEntityAtlasSnapshot(
  sources: BevyEntityAtlasSource[],
  key: string,
): Promise<BevyEntityAtlasSnapshot | null> {
  const fullPack = await loadCrystalFullPackBevyEntityAtlasSnapshot(sources, key);
  if (fullPack) {
    return fullPack;
  }

  const manifest = await loadBevyEntityAtlasManifest();
  if (!manifest?.atlases?.length) {
    return null;
  }

  const sourceKeys = new Set(sources.map((source) => source.key));
  for (const candidate of manifest.atlases) {
    if (!prebuiltBevyEntityAtlasCoversSources(candidate, sourceKeys)) {
      continue;
    }

    // The prebuilt image can cover thousands of frames, but a render-state update
    // only needs metadata for the frames referenced by this scene snapshot. Keeping
    // the full manifest here made every animation phase stringify and deserialize
    // nearly 1 MB of immutable rect data on the main thread.
    const rectList = candidate.rects.filter((rect) => sourceKeys.has(rect.key));
    const rects = bevyEntityAtlasRectMap(rectList);

    // Multi-page candidate (manifest schemaVersion≥2): build one page per
    // texture page, grouping rects by pageIndex. Each page carries its own
    // imageUrl so the runtime loads pages independently (no pixel push).
    if (candidate.pages && candidate.pages.length > 1) {
      const multiPage = buildMultiPagePrebuiltSnapshot(candidate, key, rects, rectList);
      if (multiPage) {
        return multiPage;
      }
      continue;
    }

    if (candidate.imageUrl) {
      return {
        key,
        sourceKey: candidate.key,
        width: candidate.width,
        height: candidate.height,
        imageUrl: resolveBevyEntityAtlasAssetUrl(candidate.imageUrl),
        rects,
        rectList,
      };
    }

    const pixels = await loadPrebuiltBevyEntityAtlasCandidatePixels(candidate);
    if (pixels) {
      return {
        key,
        sourceKey: candidate.key,
        width: candidate.width,
        height: candidate.height,
        rects,
        rectList,
        pixels,
      };
    }
  }

  return null;
}

async function loadCrystalFullPackBevyEntityAtlasSnapshot(
  sources: BevyEntityAtlasSource[],
  key: string,
): Promise<BevyEntityAtlasSnapshot | null> {
  if (typeof window === "undefined") return null;
  if (new URLSearchParams(window.location.search).get("crystalFullPack") === "0") return null;
  try {
    const runtime = await loadCrystalFullPackIndex();
    return await buildCrystalFullPackAtlasSnapshot(runtime, sources, key);
  } catch {
    // Keep the existing starter/live path available during partial deployments.
    return null;
  }
}

// Page 0 keeps the atlas key (single-page convention); spill pages get a
// `#p<i>` suffix so the runtime registers each page under a distinct atlas key.
function bevyEntityAtlasPageKey(atlasKey: string, pageIndex: number) {
  return pageIndex === 0 ? atlasKey : `${atlasKey}#p${pageIndex}`;
}

// Build a multi-page snapshot from a prebuilt manifest candidate: one page per
// texture page, each rect routed to its page by `pageIndex`. Synchronous — pages
// carry imageUrls and the runtime loads them, so there is no fetch/decode here.
function buildMultiPagePrebuiltSnapshot(
  candidate: PrebuiltBevyEntityAtlasRecord,
  key: string,
  rects: Record<string, BevyEntityAtlasRect>,
  rectList: BevyEntityAtlasRect[],
): BevyEntityAtlasSnapshot | null {
  const pageDescriptors = candidate.pages ?? [];
  if (!pageDescriptors.length) {
    return null;
  }

  // Group rects by their pageIndex (absent ⇒ page 0).
  const rectsByPage = new Map<number, BevyEntityAtlasRect[]>();
  for (const rect of rectList) {
    const pageIndex = rect.pageIndex ?? 0;
    const list = rectsByPage.get(pageIndex);
    if (list) {
      list.push(rect);
    } else {
      rectsByPage.set(pageIndex, [rect]);
    }
  }

  const pages: BevyEntityAtlasPage[] = [];
  for (let pageIndex = 0; pageIndex < pageDescriptors.length; pageIndex += 1) {
    const descriptor = pageDescriptors[pageIndex];
    if (!descriptor?.imageUrl) {
      // Multi-page prebuilt requires a per-page image URL; bail to the next
      // candidate / live build rather than render a partial atlas.
      return null;
    }
    pages.push({
      key: bevyEntityAtlasPageKey(candidate.key, pageIndex),
      width: descriptor.width,
      height: descriptor.height,
      imageUrl: resolveBevyEntityAtlasAssetUrl(descriptor.imageUrl),
      rectList: rectsByPage.get(pageIndex) ?? [],
    });
  }

  const page0 = pages[0];
  return {
    key,
    sourceKey: candidate.key,
    width: page0.width,
    height: page0.height,
    imageUrl: page0.imageUrl,
    rects,
    rectList,
    pages,
  };
}

function loadPrebuiltBevyEntityAtlasCandidatePixels(candidate: PrebuiltBevyEntityAtlasRecord) {
  const cacheKey = `${candidate.key}:${candidate.pixelsUrl ?? candidate.imageUrl ?? ""}:${candidate.width}x${candidate.height}`;
  const cached = bevyEntityAtlasPrebuiltPixelsCache.get(cacheKey);
  if (cached) {
    return cached;
  }

  const promise = (async () => {
    try {
      if (candidate.pixelsUrl) {
        return await loadPrebuiltBevyEntityAtlasPixels(candidate.pixelsUrl, candidate.width, candidate.height);
      }
      if (candidate.imageUrl) {
        return await loadPrebuiltBevyEntityAtlasImagePixels(candidate.imageUrl, candidate.width, candidate.height);
      }
    } catch {
      return null;
    }
    return null;
  })();
  bevyEntityAtlasPrebuiltPixelsCache.set(cacheKey, promise);
  const release = () => {
    if (bevyEntityAtlasPrebuiltPixelsCache.get(cacheKey) === promise) {
      bevyEntityAtlasPrebuiltPixelsCache.delete(cacheKey);
    }
  };
  void promise.then(release, release);
  return promise;
}

function prebuiltBevyEntityAtlasCoversSources(candidate: PrebuiltBevyEntityAtlasRecord, sourceKeys: Set<string>) {
  if (!candidate.key || candidate.width <= 0 || candidate.height <= 0 || !Array.isArray(candidate.rects)) {
    return false;
  }
  if (!candidate.imageUrl && !candidate.pixelsUrl) {
    return false;
  }
  const rectKeys = new Set(candidate.rects.map((rect) => rect.key));
  for (const sourceKey of sourceKeys) {
    if (!rectKeys.has(sourceKey)) {
      return false;
    }
  }
  return true;
}

async function loadBevyEntityAtlasManifest() {
  if (typeof window === "undefined") {
    return null;
  }
  if (new URLSearchParams(window.location.search).get("bevyAtlasPrebuilt") === "0") {
    return null;
  }
  if (!bevyEntityAtlasManifestPromise) {
    // Revalidate the entity-atlas manifest instead of force-caching it: the URL is
    // constant but the file changes whenever the atlas is regenerated, so
    // "force-cache" pins a stale manifest and the prebuilt-coverage check runs
    // against old rects — the atlas silently never matches after a repack. See
    // docs/MOVEMENT-AND-ATLAS-INVESTIGATION-2026-06-24.md (Finding 1).
    bevyEntityAtlasManifestPromise = fetch(BEVY_ENTITY_ATLAS_MANIFEST_URL, {
      cache: "no-cache",
    })
      .then(async (response) => {
        if (!response.ok) {
          return null;
        }
        return (await response.json()) as BevyEntityAtlasManifest;
      })
      .catch(() => null);
  }
  return bevyEntityAtlasManifestPromise;
}

async function loadPrebuiltBevyEntityAtlasPixels(url: string, width: number, height: number) {
  const response = await fetch(resolveBevyEntityAtlasAssetUrl(url), {
    cache: "force-cache",
  });
  if (!response.ok) {
    return null;
  }
  const bytes = new Uint8Array(await response.arrayBuffer());
  return bytes.byteLength === width * height * 4 ? bytes : null;
}

async function loadPrebuiltBevyEntityAtlasImagePixels(url: string, width: number, height: number) {
  const image = await loadBevyEntityAtlasImage(resolveBevyEntityAtlasAssetUrl(url));
  const canvas = document.createElement("canvas");
  canvas.width = width;
  canvas.height = height;
  const context = canvas.getContext("2d", {
    alpha: true,
    willReadFrequently: true,
  });
  if (!context) {
    return null;
  }
  context.clearRect(0, 0, canvas.width, canvas.height);
  context.imageSmoothingEnabled = false;
  context.drawImage(image, 0, 0, width, height);
  const imageData = context.getImageData(0, 0, width, height);
  return new Uint8Array(
    imageData.data.buffer.slice(imageData.data.byteOffset, imageData.data.byteOffset + imageData.data.byteLength),
  );
}

function resolveBevyEntityAtlasAssetUrl(url: string) {
  if (url.startsWith("http://") || url.startsWith("https://") || url.startsWith("/")) {
    return url;
  }
  return new URL(url, new URL(BEVY_ENTITY_ATLAS_MANIFEST_URL, window.location.href)).toString();
}

function bevyEntityAtlasRectMap(rectList: BevyEntityAtlasRect[]) {
  return Object.fromEntries(rectList.map((rect) => [rect.key, rect])) as Record<string, BevyEntityAtlasRect>;
}

async function buildBevyEntityAtlasSnapshot(
  sources: BevyEntityAtlasSource[],
  key: string,
): Promise<BevyEntityAtlasSnapshot> {
  const images = await Promise.all(
    sources.map(async (source) => ({
      ...source,
      image: await loadBevyEntityAtlasImage(source.path),
    })),
  );
  const packed = packBevyEntityAtlas(images);
  const canvas = document.createElement("canvas");
  canvas.width = packed.width;
  canvas.height = packed.height;

  const context = canvas.getContext("2d", {
    alpha: true,
    willReadFrequently: true,
  });
  if (!context) {
    throw new Error("2d canvas unavailable for Bevy entity atlas");
  }
  context.clearRect(0, 0, canvas.width, canvas.height);
  context.imageSmoothingEnabled = false;

  for (const source of images) {
    const rect = packed.rects[source.key];
    if (!rect) continue;
    context.drawImage(source.image, rect.x, rect.y, rect.width, rect.height);
  }

  const imageData = context.getImageData(0, 0, canvas.width, canvas.height);
  const pixels = new Uint8Array(
    imageData.data.buffer.slice(imageData.data.byteOffset, imageData.data.byteOffset + imageData.data.byteLength),
  );
  const rectList = Object.values(packed.rects).sort((a, b) => a.key.localeCompare(b.key));
  return {
    key,
    width: canvas.width,
    height: canvas.height,
    rects: packed.rects,
    rectList,
    pixels,
  };
}

function loadBevyEntityAtlasImage(path: string) {
  const cached = bevyEntityAtlasImageCache.get(path);
  if (cached) {
    bevyEntityAtlasImageCache.delete(path);
    bevyEntityAtlasImageCache.set(path, cached);
    return cached;
  }

  const promise = new Promise<HTMLImageElement>((resolve, reject) => {
    const image = new Image();
    image.crossOrigin = "anonymous";
    image.decoding = "async";
    image.onload = () => resolve(image);
    image.onerror = () => reject(new Error(`Failed to load entity atlas image: ${path}`));
    image.src = path;
  });
  bevyEntityAtlasImageCache.set(path, promise);
  while (bevyEntityAtlasImageCache.size > BEVY_ENTITY_ATLAS_CACHE_LIMIT * 4) {
    const oldest = bevyEntityAtlasImageCache.keys().next().value;
    if (typeof oldest !== "string") break;
    bevyEntityAtlasImageCache.delete(oldest);
  }
  void promise.catch(() => {
    if (bevyEntityAtlasImageCache.get(path) === promise) {
      bevyEntityAtlasImageCache.delete(path);
    }
  });
  return promise;
}

function packBevyEntityAtlas(
  sources: Array<BevyEntityAtlasSource & { image: HTMLImageElement }>,
): { width: number; height: number; rects: Record<string, BevyEntityAtlasRect> } {
  const sortedSources = [...sources].sort((a, b) => b.height - a.height || b.width - a.width || a.key.localeCompare(b.key));
  const widest = sortedSources.reduce((max, source) => Math.max(max, source.width + BEVY_ENTITY_ATLAS_PADDING * 2), 1);
  let width = Math.max(BEVY_ENTITY_ATLAS_INITIAL_WIDTH, nextPowerOfTwo(widest));

  while (width <= BEVY_ENTITY_ATLAS_MAX_SIZE) {
    const rects: Record<string, BevyEntityAtlasRect> = {};
    let cursorX = BEVY_ENTITY_ATLAS_PADDING;
    let cursorY = BEVY_ENTITY_ATLAS_PADDING;
    let rowHeight = 0;

    for (const source of sortedSources) {
      if (cursorX + source.width + BEVY_ENTITY_ATLAS_PADDING > width) {
        cursorX = BEVY_ENTITY_ATLAS_PADDING;
        cursorY += rowHeight + BEVY_ENTITY_ATLAS_PADDING;
        rowHeight = 0;
      }

      rects[source.key] = {
        key: source.key,
        x: cursorX,
        y: cursorY,
        width: source.width,
        height: source.height,
      };
      cursorX += source.width + BEVY_ENTITY_ATLAS_PADDING;
      rowHeight = Math.max(rowHeight, source.height);
    }

    const height = nextPowerOfTwo(cursorY + rowHeight + BEVY_ENTITY_ATLAS_PADDING);
    if (height <= BEVY_ENTITY_ATLAS_MAX_SIZE) {
      return { width, height, rects };
    }
    width *= 2;
  }

  throw new Error("Visible entity sprites exceed Bevy atlas size budget");
}

async function loadPersistedBevyEntityAtlas(key: string): Promise<BevyEntityAtlasSnapshot | null> {
  if (!shouldUsePersistentBevyEntityAtlasCache()) {
    return null;
  }
  const db = await openBevyEntityAtlasDb();
  if (!db) {
    return null;
  }

  try {
    const transaction = db.transaction(BEVY_ENTITY_ATLAS_IDB_STORE, "readwrite");
    const store = transaction.objectStore(BEVY_ENTITY_ATLAS_IDB_STORE);
    const record = (await idbRequest(store.get(key))) as PersistedBevyEntityAtlasRecord | undefined;
    if (!record || record.namespace !== BEVY_ENTITY_ATLAS_CACHE_NAMESPACE) {
      await idbTransactionDone(transaction);
      return null;
    }

    const pixels = persistedBevyEntityAtlasPixels(record.pixels);
    if (!pixels || pixels.byteLength !== record.width * record.height * 4) {
      store.delete(key);
      await idbTransactionDone(transaction);
      return null;
    }

    record.lastUsedAt = Date.now();
    store.put(record);
    await idbTransactionDone(transaction);
    return {
      key: record.key,
      sourceKey: record.sourceKey,
      width: record.width,
      height: record.height,
      rects: bevyEntityAtlasRectMap(record.rectList),
      rectList: record.rectList,
      pixels,
    };
  } catch {
    return null;
  }
}

async function persistBevyEntityAtlas(atlas: BevyEntityAtlasSnapshot) {
  if (!shouldUsePersistentBevyEntityAtlasCache()) {
    return;
  }
  if (!atlas.pixels) {
    return;
  }
  const db = await openBevyEntityAtlasDb();
  if (!db) {
    return;
  }

  try {
    const now = Date.now();
    const pixels = new Uint8Array(atlas.pixels.byteLength);
    pixels.set(atlas.pixels);
    const transaction = db.transaction(BEVY_ENTITY_ATLAS_IDB_STORE, "readwrite");
    const store = transaction.objectStore(BEVY_ENTITY_ATLAS_IDB_STORE);
    store.put({
      namespace: BEVY_ENTITY_ATLAS_CACHE_NAMESPACE,
      key: atlas.key,
      sourceKey: atlas.sourceKey,
      width: atlas.width,
      height: atlas.height,
      rectList: atlas.rectList,
      pixels: pixels.buffer,
      storedAt: now,
      lastUsedAt: now,
    } satisfies PersistedBevyEntityAtlasRecord);
    await idbTransactionDone(transaction);
    bevyEntityAtlasResolveStats.persistentWrites += 1;
    await trimPersistedBevyEntityAtlases(db);
  } catch {
    // Persistent atlas cache is an optimization; live atlas rendering remains the fallback.
  }
}

function shouldUsePersistentBevyEntityAtlasCache() {
  if (typeof window === "undefined" || !("indexedDB" in window)) {
    return false;
  }
  return new URLSearchParams(window.location.search).get("bevyAtlasPersistent") !== "0";
}

function openBevyEntityAtlasDb() {
  if (!bevyEntityAtlasDbPromise) {
    bevyEntityAtlasDbPromise = new Promise<IDBDatabase | null>((resolve) => {
      if (typeof window === "undefined" || !("indexedDB" in window)) {
        resolve(null);
        return;
      }
      const request = window.indexedDB.open(BEVY_ENTITY_ATLAS_IDB_NAME, BEVY_ENTITY_ATLAS_IDB_VERSION);
      request.onupgradeneeded = () => {
        const db = request.result;
        if (!db.objectStoreNames.contains(BEVY_ENTITY_ATLAS_IDB_STORE)) {
          const store = db.createObjectStore(BEVY_ENTITY_ATLAS_IDB_STORE, { keyPath: "key" });
          store.createIndex("lastUsedAt", "lastUsedAt");
        }
      };
      request.onsuccess = () => resolve(request.result);
      request.onerror = () => resolve(null);
      request.onblocked = () => resolve(null);
    });
  }
  return bevyEntityAtlasDbPromise;
}

async function trimPersistedBevyEntityAtlases(db: IDBDatabase) {
  const transaction = db.transaction(BEVY_ENTITY_ATLAS_IDB_STORE, "readwrite");
  const store = transaction.objectStore(BEVY_ENTITY_ATLAS_IDB_STORE);
  const records = ((await idbRequest(store.getAll())) as PersistedBevyEntityAtlasRecord[])
    .filter((record) => record.namespace === BEVY_ENTITY_ATLAS_CACHE_NAMESPACE)
    .sort((a, b) => a.lastUsedAt - b.lastUsedAt);
  while (records.length > BEVY_ENTITY_ATLAS_PERSISTENT_LIMIT) {
    const record = records.shift();
    if (!record) break;
    store.delete(record.key);
  }
  await idbTransactionDone(transaction);
}

function persistedBevyEntityAtlasPixels(pixels: ArrayBuffer | Uint8Array | unknown) {
  if (pixels instanceof Uint8Array) {
    return pixels;
  }
  if (pixels instanceof ArrayBuffer) {
    return new Uint8Array(pixels);
  }
  return null;
}

function idbRequest<T>(request: IDBRequest<T>) {
  return new Promise<T>((resolve, reject) => {
    request.onsuccess = () => resolve(request.result);
    request.onerror = () => reject(request.error ?? new Error("IndexedDB request failed"));
  });
}

function idbTransactionDone(transaction: IDBTransaction) {
  return new Promise<void>((resolve, reject) => {
    transaction.oncomplete = () => resolve();
    transaction.onabort = () => reject(transaction.error ?? new Error("IndexedDB transaction aborted"));
    transaction.onerror = () => reject(transaction.error ?? new Error("IndexedDB transaction failed"));
  });
}

type PersistedBevyEntityAtlasRecord = {
  namespace: string;
  key: string;
  sourceKey?: string;
  width: number;
  height: number;
  rectList: BevyEntityAtlasRect[];
  pixels: ArrayBuffer;
  storedAt: number;
  lastUsedAt: number;
};

function nextPowerOfTwo(value: number) {
  return 2 ** Math.ceil(Math.log2(Math.max(1, value)));
}

function hashString(value: string) {
  let hash = 2166136261;
  for (let index = 0; index < value.length; index += 1) {
    hash ^= value.charCodeAt(index);
    hash = Math.imul(hash, 16777619);
  }
  return (hash >>> 0).toString(36);
}

function createSceneAssetReadiness(
  key: string,
  ready: boolean,
  status: SceneAssetReadiness["status"],
  total: number,
): SceneAssetReadiness {
  return {
    key,
    ready,
    interactionReady: ready,
    visualReady: ready,
    status,
    total,
    loaded: ready ? total : 0,
    failed: 0,
    pending: ready ? 0 : total,
    durationMs: 0,
    failedUrls: [],
  };
}

type SceneAssetPreloadOptions = {
  allowPartialReady?: boolean;
  minLoaded?: number;
};

async function preloadSceneAssetUrls(
  urls: string[],
  timeoutMs: number,
  options: SceneAssetPreloadOptions = {},
): Promise<SceneAssetReadiness> {
  const startedAt = performance.now();
  const minimumLoaded = Math.min(urls.length, options.minLoaded ?? SCENE_INTERACTION_MIN_PRELOADED_URLS);
  let loaded = 0;
  let pending = urls.length;
  const failedUrls: string[] = [];

  // Resolve as soon as enough tiles are ready for interaction (partialReady)
  // instead of awaiting every URL: a few slow/failed tiles must NOT hold the
  // "Loading map…" overlay for the full timeout. The timeout is only the hard
  // ceiling; remaining tiles keep loading in the background after we resolve.
  return await new Promise<SceneAssetReadiness>((resolve) => {
    let settled = false;
    const build = (timedOut: boolean): SceneAssetReadiness => {
      const partialReady = options.allowPartialReady === true && minimumLoaded > 0 && loaded >= minimumLoaded;
      const ready = failedUrls.length === 0 || partialReady;
      return {
        key: "scene-assets",
        ready,
        interactionReady: ready,
        visualReady: pending === 0 && failedUrls.length === 0,
        status: ready ? "ready" : timedOut ? "timeout" : "loading",
        total: urls.length,
        loaded,
        failed: failedUrls.length,
        pending,
        durationMs: Math.round(performance.now() - startedAt),
        failedUrls: failedUrls.slice(0, 20),
      };
    };
    const finish = (timedOut: boolean) => {
      if (settled) return;
      settled = true;
      clearTimeout(timer);
      resolve(build(timedOut));
    };
    const timer = setTimeout(() => finish(true), timeoutMs);
    if (urls.length === 0) {
      finish(false);
      return;
    }
    for (const url of urls) {
      void preloadSceneImage(url, timeoutMs).then((result) => {
        if (result.loaded) loaded += 1;
        else failedUrls.push(result.url);
        pending -= 1;
        // Early-out the moment we have enough loaded for interaction, else once all settle.
        if (options.allowPartialReady === true && minimumLoaded > 0 && loaded >= minimumLoaded) {
          finish(false);
        } else if (pending === 0) {
          finish(false);
        }
      });
    }
  });
}

async function preloadSceneImage(url: string, timeoutMs: number): Promise<{ url: string; loaded: boolean }> {
  const startedAt = performance.now();
  const candidates = sceneAssetCandidateUrls(url);

  for (const candidate of candidates) {
    const remainingMs = timeoutMs - (performance.now() - startedAt);
    if (remainingMs <= 0) {
      break;
    }
    const loaded = await preloadSceneImageCandidate(candidate, Math.max(250, remainingMs));
    if (loaded) {
      return { url, loaded: true };
    }
  }

  return { url, loaded: false };
}

function preloadSceneImageCandidate(url: string, timeoutMs: number): Promise<boolean> {
  return new Promise((resolve) => {
    const image = new Image();
    let settled = false;
    const timer = window.setTimeout(() => finish(false), timeoutMs);

    const finish = (loaded: boolean) => {
      if (settled) {
        return;
      }
      settled = true;
      window.clearTimeout(timer);
      resolve(loaded);
    };

    image.onload = () => {
      if (typeof image.decode === "function") {
        image
          .decode()
          .then(() => finish(image.naturalWidth > 0))
          .catch(() => finish(image.naturalWidth > 0));
        return;
      }
      finish(image.naturalWidth > 0);
    };
    image.onerror = () => finish(false);
    image.decoding = "async";
    image.src = url;
    if (image.complete) {
      finish(image.naturalWidth > 0);
    }
  });
}
