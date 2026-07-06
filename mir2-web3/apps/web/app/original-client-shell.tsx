"use client";

import { memo, useCallback, useEffect, useLayoutEffect, useMemo, useRef, useState, type CSSProperties, type MouseEvent, type MutableRefObject } from "react";

import {
  ORIGINAL_UI,
  type ClientScreen,
  type CharacterTabKey,
  type InventoryTabKey,
} from "../lib/original-ui";
import { createAssetResidency } from "../lib/asset-residency";
import { createBrowserAtlasFetcher } from "../lib/asset-residency/browser-adapters";
import type { AtlasPagePayload, PersistentStore } from "../lib/asset-residency/types";
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
import { GameUiScene } from "./components/original-client-game-ui-scene";
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
  refreshEntityMotionSnapshots,
  rescueStalledSceneAssetImages,
  sceneAssetCandidateUrls,
  sceneAssetRuntimeStats,
  viewportDepthForCell,
  type ViewportOffset,
} from "./components/original-client-scene-rendering";
import { OriginalClientSceneVisualLayers } from "./components/original-client-scene-visual-layers";
import {
  OriginalClientSceneOverlays,
  type SceneChatBubble,
} from "./components/original-client-scene-overlays";
import { OriginalClientMobileControls } from "./components/original-client-mobile-controls";
import { WebGl2EntityAtlasLayer, type WebGl2EntityAtlasDebug } from "./components/webgl2-entity-atlas-layer";
import { WebGl2MapAtlasLayer, type MapTileDraw } from "./components/webgl2-map-atlas-layer";
import { buildMapTileDrawList } from "./components/original-client-scene-map-rendering";
import { type MapAtlasIndex, type MapAtlasPage, loadMapAtlasIndex } from "../lib/map-atlas-manifest";

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

type BevyMapAnchor = {
  regionKey: string;
  x: number;
  y: number;
};

// How long a freshly-seen chat line floats over its speaker before it is dropped.
const CHAT_BUBBLE_TTL_MS = 6_000;
const BEVY_MAP_ANCHOR_RECENTER_X = 12;
const BEVY_MAP_ANCHOR_RECENTER_Y = 10;
// Channels worth surfacing as over-head speech (local say-style chatter). Global/system channels
// such as trade, server, announcement and system stay in the chat log only.
const CHAT_BUBBLE_CHANNELS = new Set(["normal", "shout", "whisper", "group", "guild"]);

type BevyEntityAtlasRect = {
  key: string;
  x: number;
  y: number;
  width: number;
  height: number;
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
};

type BevyEntityAtlasSource = {
  key: string;
  path: string;
  width: number;
  height: number;
};

const BEVY_ENTITY_ATLAS_PADDING = 1;
const BEVY_ENTITY_ATLAS_INITIAL_WIDTH = 512;
const BEVY_ENTITY_ATLAS_MAX_SIZE = 4096;
const BEVY_ENTITY_ATLAS_CACHE_LIMIT = 24;
const BEVY_ENTITY_ATLAS_MANIFEST_URL = "/bevy-entity-atlases/manifest.json";
const BEVY_ENTITY_ATLAS_IDB_NAME = "mir2-bevy-entity-atlas-cache";
const BEVY_ENTITY_ATLAS_IDB_STORE = "atlases";
const BEVY_ENTITY_ATLAS_IDB_VERSION = 1;
const BEVY_ENTITY_ATLAS_PERSISTENT_LIMIT = 8;
const BEVY_ENTITY_ATLAS_CACHE_NAMESPACE = "bevy-entity-atlas-v1";
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

// mir2-probe L3 — expose the residency manager stats + IDB timings to the
// browser, so window.__mir2Probe.snapshot() can read them via the probe bus.
// Idempotent on re-load; uses a fresh closure over the live `bevyAtlasResidency`
// instance and the (module-local) idb-time probe in browser-adapters.ts.
if (typeof window !== "undefined") {
  (window as unknown as { __mir2Residency?: unknown }).__mir2Residency = {
    stats: () => bevyAtlasResidency.stats(),
    resolveStats: () => ({ ...bevyEntityAtlasResolveStats }),
    idbTimings: () => {
      try {
        // Lazy import shim: browser-adapters exports the reader; resolve the
        // function lazily so SSR / non-DOM environments do not crash.
        const mod = (window as unknown as { __mir2ResidencyIdbTimingsReader?: () => unknown }).__mir2ResidencyIdbTimingsReader;
        return typeof mod === "function" ? mod() : null;
      } catch {
        return null;
      }
    },
  };
}

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
};

type BevyEntityAtlasResolveResult = {
  atlas: BevyEntityAtlasSnapshot;
  source: "prebuilt" | "persistent" | "live";
  prebuiltKey?: string | null;
};

type KeyboardMoveDirection = "up" | "down" | "left" | "right";

const KEYBOARD_MOVE_KEYS = new Set(["w", "a", "s", "d", "arrowup", "arrowdown", "arrowleft", "arrowright"]);

function recordKeyboardMoveDebug(event: Record<string, unknown>) {
  if (typeof window === "undefined") return;
  const debugWindow = window as typeof window & {
    __mir2KeyboardMoveEvents?: Array<Record<string, unknown>>;
  };
  debugWindow.__mir2KeyboardMoveEvents = [
    { ...event, at: Date.now() },
    ...(debugWindow.__mir2KeyboardMoveEvents ?? []),
  ].slice(0, 200);
}

const isShellRenderPerfMode: boolean =
  typeof window !== "undefined" && new URLSearchParams(window.location.search).get("renderPerf") === "1";

function recordShellRenderPerf(event: Record<string, unknown>) {
  if (!isShellRenderPerfMode || typeof window === "undefined") return;
  const debugWindow = window as typeof window & {
    __mir2ShellRenderPerf?: Array<Record<string, unknown>>;
  };
  debugWindow.__mir2ShellRenderPerf = [
    ...(debugWindow.__mir2ShellRenderPerf ?? []),
    { ...event, at: Date.now() },
  ].slice(-120);
}

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
  player,
  predictedPlayerPosition,
  getLivePlayerRenderPosition,
  selectedEntity,
  sortedEntities,
  viewportEntities,
  viewportTiles,
  sceneInteractionReady,
  bevyEntityRendererReady,
  bevyRuntimeBackend,
  onSceneAssetReadinessChange,
  onBevyEntityRenderStateChange,
  onBevyMapRenderStateChange,
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
  const shellRenderPerfStartedAt = isShellRenderPerfMode ? performance.now() : 0;
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
    language === "zh-CN" ? "地图加载中…" : language === "es" ? "Cargando mapa…" : "Loading map…",
  );
  const [loginTransitionFrame, setLoginTransitionFrame] = useState<number | null>(null);
  const [sceneSpriteFrameIndex, setSceneSpriteFrameIndex] = useState(0);
  const [motionNow, setMotionNow] = useState(0);
  const [sceneSpriteLibraries, setSceneSpriteLibraries] = useState<Record<string, OriginalSceneSpriteLibraryMeta>>({});
  const [forceMobileControls, setForceMobileControls] = useState(false);
  const [touchPrimaryDevice, setTouchPrimaryDevice] = useState(false);
  const [stageScale, setStageScale] = useState(1);
  const [bevyEntityAtlas, setBevyEntityAtlas] = useState<BevyEntityAtlasSnapshot | null>(null);
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
  // Over-head chat bubble bookkeeping. Keyed by speaker name (the only entity reference a chat log
  // line carries), each record remembers when the line first appeared so bubbles can expire on the
  // shell's existing motion clock without any dedicated timer.
  const chatBubbleStateRef = useRef<Map<string, ChatBubbleRecord>>(new Map());
  const stageFrameRef = useRef<HTMLDivElement | null>(null);
  const heldScenePointerRef = useRef<HeldScenePointer | null>(null);
  const heldKeyboardMoveKeysRef = useRef<Set<KeyboardMoveDirection>>(new Set());
  const heldKeyboardRunModeRef = useRef(false);
  const bevyMapAnchorRef = useRef<BevyMapAnchor | null>(null);
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
  const stageScaleStyle = useMemo(
    () => ({ "--mir-stage-scale": stageScale }) as CSSProperties,
    [stageScale],
  );
  const sceneAssetReadinessCallbackRef = useRef(onSceneAssetReadinessChange);
  sceneAssetReadinessCallbackRef.current = onSceneAssetReadinessChange;
  const sceneInteractionReadyRef = useRef(sceneInteractionReady);
  sceneInteractionReadyRef.current = sceneInteractionReady;
  const viewportDirectionIntentRef = useRef(onViewportDirectionIntent);
  viewportDirectionIntentRef.current = onViewportDirectionIntent;
  const viewportDirectionStopRef = useRef(onViewportDirectionStop);
  viewportDirectionStopRef.current = onViewportDirectionStop;
  const sceneMotionClockActiveRef = useRef(false);

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
    const params = new URLSearchParams(window.location.search);
    setForceMobileControls(params.get("mobileControls") === "1" || params.get("mobile") === "1");
    setTouchPrimaryDevice(window.matchMedia("(pointer: coarse)").matches || navigator.maxTouchPoints > 0);
    setMotionNow(Date.now());
  }, []);

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

      if (selectedEntity && !selectedEntity.dead) {
        if (event.key === " " || event.key === "Enter") {
          event.preventDefault();
          onPrimaryTargetAction();
          return;
        }

        if (event.key.toLowerCase() === "f") {
          event.preventDefault();
          onApproachTarget();
          return;
        }
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
  }, [screen, selectedEntity, world.beltItems, onApproachTarget, onPrimaryTargetAction, onUseItem]);

  function dispatchKeyboardMoveInput(source: "edge" | "held" = "held") {
    const latest = latestMoveInputRef.current;
    const heldKeys = heldKeyboardMoveKeysRef.current;
    const held = [...heldKeys];
    if (latest.screen !== "game") {
      recordKeyboardMoveDebug({ type: "dispatch", source, held, skipped: "screen", screen: latest.screen });
      return;
    }
    if (!sceneInteractionReadyRef.current) {
      recordKeyboardMoveDebug({ type: "dispatch", source, held, skipped: "sceneInteractionReady" });
      return;
    }
    if (!latest.renderPlayer && !latest.player) {
      recordKeyboardMoveDebug({ type: "dispatch", source, held, skipped: "player" });
      return;
    }

    let dx = 0;
    let dy = 0;
    if (heldKeys.has("left")) dx -= 1;
    if (heldKeys.has("right")) dx += 1;
    if (heldKeys.has("up")) dy -= 1;
    if (heldKeys.has("down")) dy += 1;
    if (dx === 0 && dy === 0) {
      recordKeyboardMoveDebug({ type: "dispatch", source, held, skipped: "empty" });
      return;
    }

    const direction = crystalDirectionFromKeyboardVector(dx, dy);
    if (!direction) {
      recordKeyboardMoveDebug({ type: "dispatch", source, held, skipped: "direction" });
      return;
    }
    const mode = heldKeyboardRunModeRef.current ? "run" : "walk";
    recordKeyboardMoveDebug({
      type: "dispatch",
      source,
      held,
      direction,
      mode,
    });
    if (source === "edge") {
      viewportDirectionIntentRef.current(direction, mode, { discrete: true });
      viewportDirectionIntentRef.current(direction, mode, { discrete: false });
      return;
    }
    viewportDirectionIntentRef.current(direction, mode, { discrete: false });
  }

  useEffect(() => {
    if (screen !== "game") {
      viewportDirectionStopRef.current();
      heldKeyboardMoveKeysRef.current.clear();
      heldKeyboardRunModeRef.current = false;
      return;
    }

    function handleKeyboardMoveDown(event: KeyboardEvent) {
      if (event.altKey || event.ctrlKey || event.metaKey || keyboardInputTargetIsEditable(event.target)) {
        return;
      }

      if (event.key === "Shift") {
        if (!sceneInteractionReadyRef.current) {
          return;
        }
        heldKeyboardRunModeRef.current = true;
        recordKeyboardMoveDebug({ type: "keydown", key: event.key, shift: true, held: [...heldKeyboardMoveKeysRef.current] });
        dispatchKeyboardMoveInput("held");
        return;
      }

      const direction = keyboardMoveDirectionForKey(event.key);
      if (!direction) {
        return;
      }

      event.preventDefault();
      if (!sceneInteractionReadyRef.current) {
        return;
      }
      heldKeyboardRunModeRef.current = event.shiftKey || heldKeyboardRunModeRef.current;
      const alreadyHeld = heldKeyboardMoveKeysRef.current.has(direction);
      heldKeyboardMoveKeysRef.current.add(direction);
      recordKeyboardMoveDebug({
        type: "keydown",
        key: event.key,
        direction,
        repeat: event.repeat,
        alreadyHeld,
        shift: event.shiftKey,
        held: [...heldKeyboardMoveKeysRef.current],
      });
      if (!alreadyHeld && !event.repeat) {
        dispatchKeyboardMoveInput("edge");
      }
    }

    function handleKeyboardMoveUp(event: KeyboardEvent) {
      if (event.key === "Shift") {
        heldKeyboardRunModeRef.current = false;
        recordKeyboardMoveDebug({ type: "keyup", key: event.key, shift: false, held: [...heldKeyboardMoveKeysRef.current] });
        if (heldKeyboardMoveKeysRef.current.size === 0) {
          viewportDirectionStopRef.current();
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
        recordKeyboardMoveDebug({
          type: "keyup",
          key: event.key,
          direction,
          shift: event.shiftKey,
          held: [...heldKeyboardMoveKeysRef.current],
        });
        if (heldKeyboardMoveKeysRef.current.size === 0) {
          viewportDirectionStopRef.current();
          return;
        }
        dispatchKeyboardMoveInput("edge");
      }
    }

    const timer = window.setInterval(() => dispatchKeyboardMoveInput("held"), CRYSTAL_MOVE_INPUT_INTERVAL_MS);
    const stop = () => {
      recordKeyboardMoveDebug({ type: "blur", held: [...heldKeyboardMoveKeysRef.current] });
      heldKeyboardMoveKeysRef.current.clear();
      heldKeyboardRunModeRef.current = false;
      viewportDirectionStopRef.current();
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
  }, [screen]);

  const lastMotionNowRef = useRef(0);
  useEffect(() => {
    if (screen !== "game") {
      return;
    }

    const now = Date.now();
    lastMotionNowRef.current = now;
    setMotionNow(now);
    let animationFrame = 0;
    // Fallback timer keeps the reconnect countdown and bubble expiry ticking
    // when rAF is suppressed (background tab, etc.). In foreground it stays
    // quiet because rAF owns active motion and idle ticks are intentionally slow.
    const fallbackTimer = window.setInterval(() => {
      const t = Date.now();
      const intervalMs = sceneMotionClockActiveRef.current ? 250 : 500;
      if (t - lastMotionNowRef.current >= intervalMs) {
        lastMotionNowRef.current = t;
        setMotionNow(t);
      }
    }, 100);
    // Throttle the rAF to ~30 Hz (one render per ≥30 ms). The shell cannot usefully
    // process frames faster than 30 Hz — motion offsets are interpolated from timestamps
    // so smoothness is retained; the reconnect countdown and chat-bubble expiry both
    // operate on second/multi-second scales. This halves the React re-render rate vs the
    // previous 60 Hz clock, recovering ~9 % of main-thread time during idle gameplay.
    const MOTION_CLOCK_MIN_INTERVAL_MS = 30;
    const MOTION_CLOCK_IDLE_INTERVAL_MS = 500;
    const updateMotionClock = () => {
      const t = Date.now();
      const intervalMs = sceneMotionClockActiveRef.current
        ? MOTION_CLOCK_MIN_INTERVAL_MS
        : MOTION_CLOCK_IDLE_INTERVAL_MS;
      if (t - lastMotionNowRef.current >= intervalMs) {
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

  useEffect(() => {
    const updateStageScale = () => {
      const viewport = window.visualViewport;
      const width = Math.max(1, Math.floor(viewport?.width ?? window.innerWidth));
      const height = Math.max(1, Math.floor(viewport?.height ?? window.innerHeight));
      const nextScale = Math.min(1, width / 1024, height / 768);
      setStageScale(Number(nextScale.toFixed(4)));
    };

    updateStageScale();
    window.addEventListener("resize", updateStageScale);
    window.visualViewport?.addEventListener("resize", updateStageScale);

    return () => {
      window.removeEventListener("resize", updateStageScale);
      window.visualViewport?.removeEventListener("resize", updateStageScale);
    };
  }, []);

  const livePlayerRenderPosition = getLivePlayerRenderPosition?.() ?? predictedPlayerPosition;
  const renderPlayer =
    player &&
    livePlayerRenderPosition &&
    Math.max(Math.abs(player.x - livePlayerRenderPosition.x), Math.abs(player.y - livePlayerRenderPosition.y)) <=
      MAX_PREDICTED_PLAYER_LEAD_TILES
      ? {
          ...player,
          x: livePlayerRenderPosition.x,
          y: livePlayerRenderPosition.y,
          direction: livePlayerRenderPosition.direction ?? player.direction,
        }
      : player;
  const sceneMotionClockNow = Date.now();
  const sceneHasEntityMotion = world.entities.some(
    (entity) => typeof entity.movementUntil === "number" && entity.movementUntil > sceneMotionClockNow,
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
  latestMoveInputRef.current = {
    screen,
    player,
    renderPlayer,
    playerCameraMotionOffset,
  };
  useLayoutEffect(() => {
    if (!isShellRenderPerfMode) {
      return;
    }
    recordShellRenderPerf({
      durationMs: Math.round((performance.now() - shellRenderPerfStartedAt) * 10) / 10,
      screen,
      motionNow,
      sceneSpriteFrameIndex,
      cameraOffsetX: Math.round(playerCameraMotionOffset.x * 10) / 10,
      cameraOffsetY: Math.round(playerCameraMotionOffset.y * 10) / 10,
      entityCount: viewportEntities.length,
    });
  });
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
    return viewportEntities.map((entity) => {
      const motionSnapshot = snapshots[entity.objectId];
      const animationState = entityAnimationStateForEntity(entity, snapshots, spriteNow);
      return {
        entity,
        sprite: buildViewportEntitySprite(
          entity,
          sceneSpriteLibraries,
          sceneSpriteFrameIndex,
          spriteNow,
          animationState,
          motionSnapshot,
        ),
      };
    });
  }, [player, viewportEntities, sceneSpriteLibraries, sceneSpriteFrameIndex]);
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
  // Stage 1 Bevy-native map renderer (DEFAULT ON; escape hatch ?bevyMap=0 or
  // localStorage mir2-bevy-map=0). When on, the same packed map-atlas tiles that
  // the DOM WebGl2MapAtlasLayer would draw are pushed into the Bevy runtime and
  // rendered behind entities; the DOM GPU layer + DOM map sprites are disabled so
  // the map is never drawn twice. Mirrors the foldWebgl2ToBevy / mapAtlas flags.
  const bevyMapRequested = useMemo(() => {
    if (typeof window === "undefined") return false;
    const params = new URLSearchParams(window.location.search);
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
  // Decoded RGBA bytes for each map-atlas page consumed by the Bevy-native map
  // renderer, keyed by page key. Decoded ONCE per page via an offscreen canvas
  // (drawImage -> getImageData); the requested set dedupes in-flight decodes.
  const decodedMapAtlasPagesRef = useRef<
    Map<string, { width: number; height: number; pixels: Uint8Array }>
  >(new Map());
  const mapAtlasPageDecodeRequestedRef = useRef<Set<string>>(new Set());
  const [mapAtlasPagesDecodedVersion, setMapAtlasPagesDecodedVersion] = useState(0);
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
  const bevyMapAnchorPlayer =
    bevyMapRequested && mapAtlasUsable && renderPlayer
      ? bevyMapAnchorPlayerFor(bevyMapAnchorRef, world, renderPlayer)
      : null;
  const bevyMapCameraOffset =
    bevyMapAnchorPlayer && renderPlayer
      ? bevyMapCameraOffsetForAnchor(bevyMapAnchorPlayer, renderPlayer, playerCameraMotionOffset)
      : playerCameraMotionOffset;
  const staticMapDrawPlan = useMemo(
    () =>
      mapAtlasUsable && mapAtlasIndex && renderPlayer
        ? buildMapTileDrawList(staticViewportMapSprites, mapAtlasIndex)
        : null,
    [mapAtlasUsable, mapAtlasIndex, renderPlayer, staticViewportMapSprites],
  );
  const animatedMapDrawPlan = useMemo(
    () =>
      mapAtlasUsable && mapAtlasIndex && renderPlayer
        ? buildMapTileDrawList(animatedViewportMapSprites, mapAtlasIndex)
        : null,
    [mapAtlasUsable, mapAtlasIndex, renderPlayer, animatedViewportMapSprites],
  );
  const bevyStaticMapDrawPlan = useMemo(
    () =>
      mapAtlasUsable && mapAtlasIndex && bevyMapAnchorPlayer
        ? buildMapTileDrawList(
            buildViewportMapSprites(world, bevyMapAnchorPlayer, 0, "static"),
            mapAtlasIndex,
          )
        : null,
    [
      mapAtlasUsable,
      mapAtlasIndex,
      bevyMapAnchorPlayer?.x,
      bevyMapAnchorPlayer?.y,
      world.originalMapRegion,
    ],
  );
  const bevyAnimatedMapDrawPlan = useMemo(
    () =>
      mapAtlasUsable && mapAtlasIndex && bevyMapAnchorPlayer
        ? buildMapTileDrawList(
            buildViewportMapSprites(world, bevyMapAnchorPlayer, sceneSpriteFrameIndex, "animated"),
            mapAtlasIndex,
          )
        : null,
    [
      mapAtlasUsable,
      mapAtlasIndex,
      bevyMapAnchorPlayer?.x,
      bevyMapAnchorPlayer?.y,
      sceneSpriteFrameIndex,
      world.originalMapRegion,
    ],
  );
  const mapDrawPlan = useMemo(() => {
    if (!staticMapDrawPlan && !animatedMapDrawPlan) {
      return null;
    }
    return {
      tiles: [...(staticMapDrawPlan?.tiles ?? []), ...(animatedMapDrawPlan?.tiles ?? [])],
      uncovered: {
        floor: [
          ...(staticMapDrawPlan?.uncovered.floor ?? []),
          ...(animatedMapDrawPlan?.uncovered.floor ?? []),
        ],
        objects: [
          ...(staticMapDrawPlan?.uncovered.objects ?? []),
          ...(animatedMapDrawPlan?.uncovered.objects ?? []),
        ],
      },
    };
  }, [staticMapDrawPlan, animatedMapDrawPlan]);
  const mapTileDrawList = mapDrawPlan?.tiles ?? EMPTY_MAP_TILE_DRAW_LIST;
  const bevyMapDrawPlan = useMemo(() => {
    if (!bevyStaticMapDrawPlan && !bevyAnimatedMapDrawPlan) {
      return null;
    }
    return {
      tiles: [...(bevyStaticMapDrawPlan?.tiles ?? []), ...(bevyAnimatedMapDrawPlan?.tiles ?? [])],
      uncovered: {
        floor: [
          ...(bevyStaticMapDrawPlan?.uncovered.floor ?? []),
          ...(bevyAnimatedMapDrawPlan?.uncovered.floor ?? []),
        ],
        objects: [
          ...(bevyStaticMapDrawPlan?.uncovered.objects ?? []),
          ...(bevyAnimatedMapDrawPlan?.uncovered.objects ?? []),
        ],
      },
    };
  }, [bevyStaticMapDrawPlan, bevyAnimatedMapDrawPlan]);
  const bevyMapTileDrawList = bevyMapDrawPlan?.tiles ?? EMPTY_MAP_TILE_DRAW_LIST;
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
  const domEntityFallbackRequested = shouldUseDomEntityFallback(forceMobileControls || touchPrimaryDevice);
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
  const currentBevyEntityAtlasCoversSources =
    bevyEntityAtlas && bevyEntityAtlasSources.length > 0
      ? bevyEntityAtlasSnapshotCoversSources(bevyEntityAtlas, bevyEntityAtlasSources)
      : false;
  // Synchronous in-memory residency read for the active atlas, so a cached-key
  // transition shows the packed atlas in the SAME frame (acquire()'s state set
  // is a microtask later). Conversion to a snapshot only runs in the fallback
  // branch below (i.e. when the React atlas state does not yet match the key).
  const peekedBevyEntityAtlasPayload =
    bevyEntityAtlasKey ? bevyAtlasResidency.peek(bevyEntityAtlasKey) : null;
  const peekedBevyEntityAtlas = peekedBevyEntityAtlasPayload
    ? payloadToAtlasSnapshot(peekedBevyEntityAtlasPayload)
    : null;
  const latestBevyEntityAtlas =
    bevyEntityAtlasKey &&
    bevyEntityAtlasLatestSnapshot &&
    (bevyEntityAtlasLatestSnapshot.key === bevyEntityAtlasKey ||
      bevyEntityAtlasSnapshotCoversSources(bevyEntityAtlasLatestSnapshot, bevyEntityAtlasSources))
      ? bevyEntityAtlasLatestSnapshot
      : null;
  const activeBevyEntityAtlas =
    bevyEntityAtlasKey && bevyEntityAtlas?.key === bevyEntityAtlasKey
      ? bevyEntityAtlas
      : currentBevyEntityAtlasCoversSources
        ? bevyEntityAtlas
        : (peekedBevyEntityAtlas &&
              bevyEntityAtlasSnapshotCoversSources(peekedBevyEntityAtlas, bevyEntityAtlasSources)
            ? peekedBevyEntityAtlas
            : null) ?? latestBevyEntityAtlas;
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
  // Bevy-native map renderer is active only when the flag is on, the Bevy entity
  // renderer is the active renderer (so the canvas is composited and entities draw
  // there too), and the packed atlas is usable. It then becomes the SOLE map
  // renderer; the DOM GPU layer + DOM map sprites are gated off below.
  const bevyMapActive = bevyMapRequested && useBevyEntityRenderer && mapAtlasUsable;
  // DOM GPU atlas layer draws only when the Bevy-native map renderer is NOT active.
  const mapGpuActive = mapAtlasUsable && !bevyMapActive;
  const runtimeOwnsSceneMotion = useBevyEntityRenderer && bevyMapActive;
  const entityRenderState = buildBevyEntityRenderState({
    enabled: useGpuEntityRenderer,
    runtimeOwnsSceneMotion,
    player,
    viewportEntitySprites,
    viewportDepthPlayer,
    playerCameraMotionOffset,
    entityMotionSnapshots: entityMotionSnapshotsRef.current,
    motionNow,
    atlas: useBevyEntityAtlas ? activeBevyEntityAtlas : null,
  });
  const bevyEntityRenderState = useBevyEntityRenderer
    ? entityRenderState
    : disabledEntityRenderState(entityRenderState);

  // Cell ownership: when Bevy owns the map, the DOM draws nothing (covered tiles
  // go to Bevy; the small uncovered remainder is an accepted Stage 1 gap). When
  // the DOM GPU layer is active it draws covered tiles and the DOM renders only
  // the uncovered remainder. When neither is active the DOM draws the full set.
  const mapDomSprites = bevyMapActive
    ? EMPTY_VIEWPORT_MAP_SPRITES
    : mapGpuActive
      ? mapDrawPlan?.uncovered ?? EMPTY_VIEWPORT_MAP_SPRITES
      : viewportMapSprites;
  sceneMotionClockActiveRef.current =
    screen === "game" &&
    ((!runtimeOwnsSceneMotion && sceneHasEntityMotion) ||
      world.projectiles.some((projectile) => projectile.expiresAt > sceneMotionClockNow) ||
      world.damageFloaters.some((floater) => floater.expiresAt > sceneMotionClockNow));
  const sceneSpriteFrameTickActive =
    screen === "game" && (!useGpuEntityRenderer || (!mapGpuActive && !bevyMapActive));

  useEffect(() => {
    if (!sceneSpriteFrameTickActive) {
      return;
    }

    const timer = window.setInterval(() => {
      setSceneSpriteFrameIndex((current) => current + 1);
    }, 120);

    return () => window.clearInterval(timer);
  }, [sceneSpriteFrameTickActive]);

  // Decode each atlas page referenced by the current tiles to RGBA bytes once.
  useEffect(() => {
    if (!bevyMapActive || !mapAtlasIndex) {
      return;
    }
    const neededPageKeys = new Set(bevyMapTileDrawList.map((tile) => tile.atlasKey));
    for (const pageKey of neededPageKeys) {
      if (mapAtlasPageDecodeRequestedRef.current.has(pageKey)) {
        continue;
      }
      const page = mapAtlasIndex.pages.get(pageKey);
      if (!page) {
        continue;
      }
      mapAtlasPageDecodeRequestedRef.current.add(pageKey);
      // NOTE: do NOT gate the decode result on an effect-scoped `cancelled` flag.
      // This effect re-runs on every mapTileDrawList change (i.e. every move /
      // map animation frame); an effect-cleanup `cancelled=true` would
      // drop the in-flight decode before it commits, while the requestedRef key
      // blocks any retry — leaving the Bevy map permanently textureless
      // (atlasImageCount stuck at 0). The page-pixel cache + decodedMapAtlasPagesRef
      // are idempotent and module/ref-scoped, so committing the result on resolve
      // is always safe regardless of how many times the effect re-ran.
      void decodeMapAtlasPagePixels(page)
        .then((decoded) => {
          if (!decoded) {
            // Allow a later retry if the decode failed (e.g. transient load error).
            mapAtlasPageDecodeRequestedRef.current.delete(pageKey);
            return;
          }
          decodedMapAtlasPagesRef.current.set(pageKey, decoded);
          setMapAtlasPagesDecodedVersion((version) => version + 1);
        })
        .catch(() => {
          mapAtlasPageDecodeRequestedRef.current.delete(pageKey);
        });
    }
  }, [bevyMapActive, mapAtlasIndex, bevyMapTileDrawList]);

  const bevyMapBaseRenderState = useMemo<BevyMapRenderState>(() => {
    if (!bevyMapActive || !mapAtlasIndex) {
      return {
        enabled: false,
        stageWidth: ORIGINAL_UI.game.sceneWidth,
        stageHeight: ORIGINAL_UI.game.sceneHeight,
        atlases: [],
        atlasImages: [],
        tiles: [],
        cameraOffset: EMPTY_VIEWPORT_OFFSET,
      };
    }
    const pageKeys = new Set(bevyMapTileDrawList.map((tile) => tile.atlasKey));
    const atlases: NonNullable<BevyMapRenderState["atlases"]> = [];
    const atlasImages: NonNullable<BevyMapRenderState["atlasImages"]> = [];
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
      const decoded = decodedMapAtlasPagesRef.current.get(pageKey);
      if (decoded) {
        atlasImages.push({
          key: page.key,
          width: decoded.width,
          height: decoded.height,
          pixels: decoded.pixels,
        });
      }
    }
    return {
      enabled: true,
      stageWidth: ORIGINAL_UI.game.sceneWidth,
      stageHeight: ORIGINAL_UI.game.sceneHeight,
      atlases,
      atlasImages,
      // Tile coordinates stay in viewport space; sub-tile movement is sent as
      // cameraOffset so walking/running does not rebuild the full tile list.
      tiles: bevyMapTileDrawList,
      cameraOffset: EMPTY_VIEWPORT_OFFSET,
    };
    // mapAtlasPagesDecodedVersion forces a rebuild once a page's pixels decode.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [bevyMapActive, mapAtlasIndex, bevyMapTileDrawList, mapAtlasPagesDecodedVersion]);
  const bevyMapRenderState = useMemo<BevyMapRenderState>(
    () => ({
      ...bevyMapBaseRenderState,
      cameraOffset: bevyMapBaseRenderState.enabled ? bevyMapCameraOffset : EMPTY_VIEWPORT_OFFSET,
    }),
    [bevyMapBaseRenderState, bevyMapCameraOffset],
  );

  useEffect(() => {
    onBevyMapRenderStateChange(bevyMapRenderState);
  }, [bevyMapRenderState, onBevyMapRenderStateChange]);

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
      atlasPixelBytes: (entityRenderState.atlasImages ?? []).reduce(
        (sum, atlas) => sum + (atlas.pixels?.byteLength ?? atlas.width * atlas.height * 4),
        0,
      ),
      atlasSourceCount: bevyEntityAtlasSources.length,
      atlasCacheSize: bevyAtlasResidency.stats().memoryCacheSize,
      atlasCurrentKey: bevyEntityAtlasKey,
      atlasPendingKey: bevyEntityAtlasRequestRef.current?.key ?? null,
      atlasCachedCurrent: Boolean(peekedBevyEntityAtlasPayload),
      atlasLatestKey: bevyEntityAtlasLatestSnapshot?.key ?? null,
      atlasLatestCurrent: Boolean(latestBevyEntityAtlas),
      domEntityFallback: useBevyEntityRenderer && !hideDomEntitySpritesForBevy,
      runtimeOwnsSceneMotion,
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
      active: bevyMapActive,
      mapGpuActive,
      atlasUsable: mapAtlasUsable,
      atlasIndexReady: Boolean(mapAtlasIndex),
      enabled: bevyMapRenderState.enabled,
      tileCount: bevyMapRenderState.tiles.length,
      atlasPageCount: bevyMapRenderState.atlases?.length ?? 0,
      atlasImageCount: bevyMapRenderState.atlasImages?.length ?? 0,
      decodedPageCount: decodedMapAtlasPagesRef.current.size,
      domSpriteCount: mapDomSprites.floor.length + mapDomSprites.objects.length,
      cameraOffset: bevyMapRenderState.cameraOffset ?? null,
    };
  }
  const showSyntheticScene = screen === "game" && !world.originalMapRegion;
  const sceneAssetUrlsRef = useRef<string[]>([]);
  // Movement interaction is gated by terrain/map readiness only. Entity sprites
  // are either handled by the Bevy atlas path or loaded naturally by their DOM
  // <img> nodes; preloading every visible body/hair/weapon URL on each scene
  // settle can starve the movement/ack loop in crowded maps.
  sceneAssetUrlsRef.current = collectVisibleSceneAssetUrls(mapDomSprites, []);

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
      if (cancelled || !renderPlayer || mapGpuActive || bevyMapActive) {
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
  }, [renderPlayer?.x, renderPlayer?.y, world.originalMapRegion, screen, mapGpuActive, bevyMapActive]);
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

    if (bevyEntityAtlas?.key === bevyEntityAtlasKey) {
      return;
    }
    if (
      activeBevyEntityAtlas &&
      bevyEntityAtlasSnapshotCoversSources(activeBevyEntityAtlas, bevyEntityAtlasSources)
    ) {
      return;
    }

    // A memory hit is served synchronously by the render-path peek
    // (peekedBevyEntityAtlasPayload); here we always acquire through the
    // residency manager (memory -> [null persistent] -> resolve fetcher) and
    // commit the React atlas state when it resolves. The manager owns the
    // in-memory tier + LRU; resolve owns the prebuilt/persistent/live cold path
    // and records its source breakdown into bevyEntityAtlasResolveStats.
    if (bevyEntityAtlasRequestRef.current?.key === bevyEntityAtlasKey) {
      return;
    }

    const requestId = (bevyEntityAtlasRequestRef.current?.requestId ?? 0) + 1;
    bevyEntityAtlasRequestRef.current = { key: bevyEntityAtlasKey, requestId };
    bevyEntityAtlasSourcesByKey.set(bevyEntityAtlasKey, bevyEntityAtlasSources);
    let disposed = false;

    bevyAtlasResidency
      .acquire(bevyEntityAtlasKey)
      .then((payload) => {
        if (disposed || bevyEntityAtlasRequestRef.current?.requestId !== requestId) {
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
      });

    return () => {
      disposed = true;
    };
  }, [useGpuEntityRenderer, useBevyEntityAtlas, bevyEntityAtlasKey, bevyEntityAtlas?.key, activeBevyEntityAtlas?.key]);

  useEffect(() => {
    onBevyEntityRenderStateChange(bevyEntityRenderState);
  }, [bevyEntityRenderState, onBevyEntityRenderStateChange]);

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

  function tileFromScenePoint(sceneX: number, sceneY: number) {
    const latest = latestMoveInputRef.current;
    const basePlayer = latest.renderPlayer ?? latest.player;
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

  return (
    <main className={`mir-client-page ${forceMobileControls ? "force-mobile-controls" : ""}`} style={stageScaleStyle}>
      <section className="mir-stage">
        <div
          ref={stageFrameRef}
          className={`client-stage-frame ${screen === "game" && !sceneInteractionReady ? "scene-assets-pending" : ""}`}
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
            cameraOffset={playerCameraMotionOffset}
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
              player={player}
              floorSprites={mapDomSprites.floor}
              cameraOffset={playerCameraMotionOffset}
            />
          ) : null}

          <div className={`viewport-grid-overlay ${screen !== "game" ? "hidden" : ""}`}>
            {runtimeOwnsSceneMotion ? null : viewportTiles.map((tile) => (
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

          <OriginalClientSceneVisualLayers
            screen={screen}
            t={t}
            world={world}
            player={player}
            selectedEntity={selectedEntity}
            viewportGroundDrops={viewportGroundDrops}
            viewportMapSprites={mapDomSprites}
            viewportEntitySprites={viewportEntitySprites}
            viewportProjectiles={viewportProjectiles}
            viewportDepthPlayer={viewportDepthPlayer}
            playerCameraMotionOffset={playerCameraMotionOffset}
            entityMotionSnapshots={entityMotionSnapshotsRef.current}
            motionNow={motionNow}
            sceneSpriteFrameIndex={sceneSpriteFrameIndex}
            useBevyEntityRenderer={hideDomEntitySpritesForBevy}
            entityKindClassName={entityKindClassName}
            onPickGroundDrop={onPickGroundDrop}
            onActivateEntity={onActivateEntity}
          />
          <OriginalClientSceneOverlays
            screen={screen}
            t={t}
            player={player}
            selectedEntity={selectedEntity}
            viewportEntitySprites={viewportEntitySprites}
            playerCameraMotionOffset={playerCameraMotionOffset}
            entityMotionSnapshots={entityMotionSnapshotsRef.current}
            motionNow={motionNow}
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
          {screen === "game" ? (
            <GameUiScene
              t={t}
              locale={locale}
              runtimeMessage={runtimeMessageLabel}
              world={world}
              player={player}
              logs={logs}
              chatMessage={chatMessage}
              showInventory={showInventory}
              showCharacter={showCharacter}
              activeInventoryTab={activeInventoryTab}
              activeCharacterTab={activeCharacterTab}
              storageServiceOpenVersion={storageServiceOpenVersion}
              onChatMessageChange={onChatMessageChange}
              onSendChat={onSendChat}
              onRequestTrade={onRequestTrade}
              onRentExpandedStorage={onRentExpandedStorage}
              onLogout={onLogout}
              onToggleCharacter={onToggleCharacter}
              onToggleInventory={onToggleInventory}
              onCloseCharacter={onCloseCharacter}
              onCloseInventory={onCloseInventory}
              onOpenCharacterTab={onOpenCharacterTab}
              onOpenInventoryTab={onOpenInventoryTab}
              onSelectNpcDialogTarget={onSelectNpcDialogTarget}
              onSubmitNpcInput={onSubmitNpcInput}
              onUseItem={onUseItem}
              onDropItem={onDropItem}
              onEquipItem={onEquipItem}
              onRemoveItem={onRemoveItem}
              onMoveItem={onMoveItem}
              onMergeItem={onMergeItem}
              onSplitItem={onSplitItem}
              onStoreItem={onStoreItem}
              onTakeBackItem={onTakeBackItem}
              onUnlockStorage={onUnlockStorage}
              onSetStoragePassword={onSetStoragePassword}
              onRemoveStoragePassword={onRemoveStoragePassword}
              onSellItem={onSellItem}
              onDropGold={onDropGold}
              onRepairItem={onRepairItem}
              onSpecialRepairItem={onSpecialRepairItem}
              onCastSkill={onCastSkill}
              onTransferMap={onTransferMap}
              onClaimMail={onClaimMail}
              onDeleteMail={onDeleteMail}
              onBuyGameShopItem={onBuyGameShopItem}
              onRunStage5Command={onRunStage5Command}
              onSendClientCommand={onSendClientCommand}
              transferOptions={transferOptions}
            />
          ) : null}
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
      <OriginalClientMobileControls
        enabled={screen === "game" && sceneInteractionReady}
        forceVisible={forceMobileControls}
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

function bevyMapAnchorPlayerFor(
  anchorRef: MutableRefObject<BevyMapAnchor | null>,
  world: DisplayWorld,
  renderPlayer: DisplayEntity,
): DisplayEntity {
  const region = world.originalMapRegion;
  const regionKey = region
    ? [
        world.mapFileName ?? "",
        region.regionBounds.minX,
        region.regionBounds.minY,
        region.regionBounds.maxX,
        region.regionBounds.maxY,
      ].join(":")
    : "no-original-region";
  const current = anchorRef.current;
  const shouldRecenter =
    !current ||
    current.regionKey !== regionKey ||
    Math.abs(renderPlayer.x - current.x) > BEVY_MAP_ANCHOR_RECENTER_X ||
    Math.abs(renderPlayer.y - current.y) > BEVY_MAP_ANCHOR_RECENTER_Y;

  if (shouldRecenter) {
    anchorRef.current = {
      regionKey,
      x: renderPlayer.x,
      y: renderPlayer.y,
    };
  }

  const anchor = anchorRef.current ?? { regionKey, x: renderPlayer.x, y: renderPlayer.y };
  return {
    ...renderPlayer,
    x: anchor.x,
    y: anchor.y,
  };
}

function bevyMapCameraOffsetForAnchor(
  anchorPlayer: Pick<DisplayEntity, "x" | "y">,
  renderPlayer: Pick<DisplayEntity, "x" | "y">,
  playerCameraMotionOffset: ViewportOffset,
): ViewportOffset {
  return {
    x: playerCameraMotionOffset.x + (anchorPlayer.x - renderPlayer.x) * VIEWPORT_CELL_WIDTH,
    y: playerCameraMotionOffset.y + (anchorPlayer.y - renderPlayer.y) * VIEWPORT_CELL_HEIGHT,
  };
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

// Decoded RGBA bytes for a map-atlas page, cached by image URL so a page is only
// fetched + read back once across the session even if the renderer toggles.
const mapAtlasPagePixelCache = new Map<
  string,
  Promise<{ width: number; height: number; pixels: Uint8Array } | null>
>();

// Decode a packed map-atlas page PNG to raw RGBA bytes via an offscreen canvas
// (drawImage -> getImageData), matching set_mir2_map_render_atlas's expectation
// of width*height*4 bytes. Returns null when running without a DOM or when the
// image/canvas readback fails (the caller then leaves the page unbound for now).
function decodeMapAtlasPagePixels(
  page: MapAtlasPage,
): Promise<{ width: number; height: number; pixels: Uint8Array } | null> {
  if (typeof document === "undefined") {
    return Promise.resolve(null);
  }
  const cached = mapAtlasPagePixelCache.get(page.imageUrl);
  if (cached) {
    return cached;
  }
  const promise = new Promise<{ width: number; height: number; pixels: Uint8Array } | null>(
    (resolve) => {
      const image = new Image();
      image.decoding = "async";
      image.crossOrigin = "anonymous";
      image.onload = () => {
        try {
          const width = image.naturalWidth || page.width;
          const height = image.naturalHeight || page.height;
          if (width <= 0 || height <= 0) {
            resolve(null);
            return;
          }
          const canvas = document.createElement("canvas");
          canvas.width = width;
          canvas.height = height;
          const context = canvas.getContext("2d", { willReadFrequently: true });
          if (!context) {
            resolve(null);
            return;
          }
          context.drawImage(image, 0, 0, width, height);
          const imageData = context.getImageData(0, 0, width, height);
          resolve({ width, height, pixels: new Uint8Array(imageData.data.buffer.slice(0)) });
        } catch {
          resolve(null);
        }
      };
      image.onerror = () => resolve(null);
      image.src = page.imageUrl;
    },
  );
  mapAtlasPagePixelCache.set(page.imageUrl, promise);
  return promise;
}

// Off-screen tile prefetch ring: how many cells beyond the visible viewport to warm.
const SCENE_TILE_PREFETCH_RING_CELLS = 6;
const SCENE_TILE_PREFETCH_TIMEOUT_MS = 8_000;

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
  runtimeOwnsSceneMotion,
  player,
  viewportEntitySprites,
  viewportDepthPlayer,
  playerCameraMotionOffset,
  entityMotionSnapshots,
  motionNow,
  atlas,
}: {
  enabled: boolean;
  runtimeOwnsSceneMotion: boolean;
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

  return {
    enabled: true,
    stageWidth: 1024,
    stageHeight: 768,
    atlases: atlas
      ? [
          {
            key: atlas.key,
            width: atlas.width,
            height: atlas.height,
            imageUrl: atlas.imageUrl,
            rects: atlas.rectList,
          },
        ]
      : [],
    atlasImages: atlas?.pixels
      ? [
          {
            key: atlas.key,
            width: atlas.width,
            height: atlas.height,
            pixels: atlas.pixels,
          },
        ]
      : [],
    entities: viewportEntitySprites.map(({ entity, sprite }) => {
      const isPlayer = player.objectId === entity.objectId;
      const entityMotionOffset = runtimeOwnsSceneMotion || isPlayer
        ? EMPTY_VIEWPORT_OFFSET
        : entityMotionOffsetForEntity(entity, entityMotionSnapshots, motionNow);
      const cameraOffset = runtimeOwnsSceneMotion || isPlayer ? EMPTY_VIEWPORT_OFFSET : playerCameraMotionOffset;
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
            return {
              key: `${entity.objectId}:${role}:${index}`,
              path: layer.path,
              ...(atlasRect
                ? {
                    atlasKey: atlas.key,
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
        isPlayer,
        dead: Boolean(entity.dead),
        layers,
      };
    }),
  };
}

function collectBevyEntityAtlasSources(
  viewportEntitySprites: Array<{
    sprite: {
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

function bevyEntityAtlasSnapshotCoversSources(
  atlas: BevyEntityAtlasSnapshot | null,
  sources: BevyEntityAtlasSource[],
) {
  if (!atlas || sources.length === 0) return false;
  for (const source of sources) {
    if (!atlas.rects[source.key]) {
      return false;
    }
  }
  return true;
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
  const manifest = await loadBevyEntityAtlasManifest();
  if (!manifest?.atlases?.length) {
    return null;
  }

  const sourceKeys = new Set(sources.map((source) => source.key));
  for (const candidate of manifest.atlases) {
    if (!prebuiltBevyEntityAtlasCoversSources(candidate, sourceKeys)) {
      continue;
    }

    const rects = bevyEntityAtlasRectMap(candidate.rects);
    if (candidate.imageUrl) {
      return {
        key,
        sourceKey: candidate.key,
        width: candidate.width,
        height: candidate.height,
        imageUrl: resolveBevyEntityAtlasAssetUrl(candidate.imageUrl),
        rects,
        rectList: candidate.rects,
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
        rectList: candidate.rects,
        pixels,
      };
    }
  }

  return null;
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
    bevyEntityAtlasManifestPromise = fetch(BEVY_ENTITY_ATLAS_MANIFEST_URL, {
      cache: "force-cache",
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
