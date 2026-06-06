"use client";

import { useCallback, useEffect, useMemo, useRef, useState, type CSSProperties, type MouseEvent } from "react";

import {
  ORIGINAL_UI,
  type ClientScreen,
  type CharacterTabKey,
  type InventoryTabKey,
} from "../lib/original-ui";
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
import { type MapAtlasIndex, loadMapAtlasIndex } from "../lib/map-atlas-manifest";

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
const bevyEntityAtlasCache = new Map<string, BevyEntityAtlasSnapshot>();
const bevyEntityAtlasImageCache = new Map<string, Promise<HTMLImageElement>>();
const bevyEntityAtlasPrebuiltPixelsCache = new Map<string, Promise<Uint8Array | null>>();
let bevyEntityAtlasLatestSnapshot: BevyEntityAtlasSnapshot | null = null;
let bevyEntityAtlasManifestPromise: Promise<BevyEntityAtlasManifest | null> | null = null;
let bevyEntityAtlasDbPromise: Promise<IDBDatabase | null> | null = null;
const bevyEntityAtlasStats = {
  requests: 0,
  builds: 0,
  cacheHits: 0,
  prebuiltHits: 0,
  persistentHits: 0,
  persistentWrites: 0,
  failures: 0,
  lastBuildMs: 0,
  lastKey: null as string | null,
  lastSource: null as "memory" | "prebuilt" | "persistent" | "live" | null,
  lastPrebuiltKey: null as string | null,
  lastSourceCount: 0,
};

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
  const t = buildTranslator(language);
  const locale = languageLocale(language);
  const runtimePhaseLabel = formatRuntimePhase(language, runtimePhase);
  const runtimeMessageLabel = formatRuntimeMessage(language, runtimeMessage);
  const [loginTransitionFrame, setLoginTransitionFrame] = useState<number | null>(null);
  const [selectPortraitFrameIndex, setSelectPortraitFrameIndex] = useState(0);
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
  const entityMotionSnapshotsRef = useRef<Record<string, EntityMotionSnapshot>>({});
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
  const stageScaleStyle = useMemo(
    () => ({ "--mir-stage-scale": stageScale }) as CSSProperties,
    [stageScale],
  );
  const sceneAssetReadinessCallbackRef = useRef(onSceneAssetReadinessChange);
  sceneAssetReadinessCallbackRef.current = onSceneAssetReadinessChange;

  const selectedCharacter = characters[selectedCharacterIndex] ?? null;
  const selectedPortraitFrames = selectedCharacter ? portraitFramesForCharacter(selectedCharacter) : [];
  const activeSelectPortraitFrame =
    selectedPortraitFrames[selectPortraitFrameIndex % Math.max(selectedPortraitFrames.length, 1)] ?? null;
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

  useEffect(() => {
    setSelectPortraitFrameIndex(0);
  }, [screen, selectedCharacterIndex, selectedCharacter?.classKey, selectedCharacter?.gender]);

  useEffect(() => {
    if (screen !== "select" || selectedPortraitFrames.length <= 1) {
      return;
    }

    const timer = window.setInterval(() => {
      setSelectPortraitFrameIndex((current) => (current + 1) % selectedPortraitFrames.length);
    }, 120);

    return () => window.clearInterval(timer);
  }, [screen, selectedPortraitFrames.length]);

  useEffect(() => {
    if (screen !== "game") {
      return;
    }

    const timer = window.setInterval(() => {
      setSceneSpriteFrameIndex((current) => current + 1);
    }, 120);

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

  useEffect(() => {
    if (screen !== "game") {
      return;
    }

    setMotionNow(Date.now());
    let animationFrame = 0;
    const fallbackTimer = window.setInterval(() => setMotionNow(Date.now()), 100);
    const updateMotionClock = () => {
      setMotionNow(Date.now());
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
    const pendingLibraries = missingLibraries.filter(
      (libraryKey) =>
        originalSceneSpriteLibraryExists(libraryKey) &&
        !missingSceneSpriteLibrariesRef.current.has(libraryKey),
    );
    if (!pendingLibraries.length) {
      return;
    }

    let disposed = false;
    void Promise.all(
      pendingLibraries.map(async (libraryKey) => {
        try {
          return [libraryKey, await loadOriginalSceneSpriteLibrary(libraryKey)] as const;
        } catch {
          return null;
        }
      }),
    )
      .then((loadedLibraries) => {
        if (disposed) {
          return;
        }

        setSceneSpriteLibraries((current) => {
          const next = { ...current };
          for (const entry of loadedLibraries) {
            if (!entry) {
              continue;
            }
            const [libraryKey, libraryMeta] = entry;
            next[libraryKey] = libraryMeta;
          }
          return next;
        });
      })
      .catch(() => undefined);

    return () => {
      disposed = true;
    };
  }, [desiredSceneSpriteLibraryKey, desiredSceneSpriteLibraryKeys, sceneSpriteLibraries, screen]);

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
  if (typeof window !== "undefined") {
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
  const viewportEntitySprites = player
    ? viewportEntities.map((entity) => {
        const motionSnapshot = entityMotionSnapshotsRef.current[entity.objectId];
        const animationState = entityAnimationStateForEntity(entity, entityMotionSnapshotsRef.current, sceneNow);
        return {
          entity,
          sprite: buildViewportEntitySprite(
            entity,
            sceneSpriteLibraries,
            sceneSpriteFrameIndex,
            sceneNow,
            animationState,
            motionSnapshot,
          ),
        };
      })
    : [];
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

  // GPU map-atlas rendering (opt-in via ?mapAtlas=1). Loads the packed atlas manifest once;
  // when present, map tiles render from a few resident atlas textures on the WebGl2MapAtlasLayer
  // instead of ~450-510 per-frame DOM <img>/R2 GETs. Default OFF -> DOM path unchanged.
  const mapAtlasRequested = useMemo(() => {
    if (typeof window === "undefined") return false;
    return new URLSearchParams(window.location.search).get("mapAtlas") === "1";
  }, []);
  const [mapAtlasIndex, setMapAtlasIndex] = useState<MapAtlasIndex | null>(null);
  useEffect(() => {
    if (!mapAtlasRequested) return;
    let cancelled = false;
    void loadMapAtlasIndex().then((index) => {
      if (!cancelled) setMapAtlasIndex(index);
    });
    return () => {
      cancelled = true;
    };
  }, [mapAtlasRequested]);
  const mapGpuActive = mapAtlasRequested && Boolean(mapAtlasIndex) && screen === "game";
  const mapTileDrawList = useMemo(
    () =>
      mapGpuActive && mapAtlasIndex && renderPlayer
        ? buildMapTileDrawList(viewportMapSprites, mapAtlasIndex, playerCameraMotionOffset)
        : EMPTY_MAP_TILE_DRAW_LIST,
    [mapGpuActive, mapAtlasIndex, renderPlayer, viewportMapSprites, playerCameraMotionOffset],
  );
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
  const hideBevyCanvasForDomEntityFallback =
    screen === "game" && (bevyRuntimeBackend === "webgl2" || domEntityFallbackRequested);
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
  const cachedBevyEntityAtlas =
    bevyEntityAtlasKey ? bevyEntityAtlasCache.get(bevyEntityAtlasKey) ?? null : null;
  const latestBevyEntityAtlas =
    bevyEntityAtlasKey && bevyEntityAtlasLatestSnapshot?.key === bevyEntityAtlasKey
      ? bevyEntityAtlasLatestSnapshot
      : null;
  const activeBevyEntityAtlas =
    bevyEntityAtlasKey && bevyEntityAtlas?.key === bevyEntityAtlasKey
      ? bevyEntityAtlas
      : cachedBevyEntityAtlas ?? latestBevyEntityAtlas;
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
  });
  const bevyEntityRenderState = useBevyEntityRenderer
    ? entityRenderState
    : disabledEntityRenderState(entityRenderState);

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
      atlasCacheSize: bevyEntityAtlasCache.size,
      atlasCurrentKey: bevyEntityAtlasKey,
      atlasPendingKey: bevyEntityAtlasRequestRef.current?.key ?? null,
      atlasCachedCurrent: Boolean(cachedBevyEntityAtlas),
      atlasLatestKey: bevyEntityAtlasLatestSnapshot?.key ?? null,
      atlasLatestCurrent: Boolean(latestBevyEntityAtlas),
      domEntityFallback: useBevyEntityRenderer && !hideDomEntitySpritesForBevy,
      canvasHidden: hideBevyCanvasForDomEntityFallback,
      atlasStats: { ...bevyEntityAtlasStats },
      spriteLibraryCache: originalSceneSpriteLibraryCacheStats(),
      sceneAssetRuntime: sceneAssetRuntimeStats(),
      domImageCount: document.images.length,
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

    const cachedAtlas = getCachedBevyEntityAtlas(bevyEntityAtlasKey);
    if (cachedAtlas) {
      if (bevyEntityAtlasRequestRef.current?.key === bevyEntityAtlasKey) {
        bevyEntityAtlasRequestRef.current = null;
      }
      bevyEntityAtlasStats.cacheHits += 1;
      bevyEntityAtlasStats.lastKey = bevyEntityAtlasKey;
      bevyEntityAtlasStats.lastSource = "memory";
      bevyEntityAtlasStats.lastPrebuiltKey = null;
      bevyEntityAtlasStats.lastSourceCount = bevyEntityAtlasSources.length;
      setBevyEntityAtlas(cachedAtlas);
      return;
    }

    if (bevyEntityAtlasRequestRef.current?.key === bevyEntityAtlasKey) {
      return;
    }

    const requestId = (bevyEntityAtlasRequestRef.current?.requestId ?? 0) + 1;
    bevyEntityAtlasRequestRef.current = { key: bevyEntityAtlasKey, requestId };
    bevyEntityAtlasStats.requests += 1;
    bevyEntityAtlasStats.lastKey = bevyEntityAtlasKey;
    bevyEntityAtlasStats.lastSourceCount = bevyEntityAtlasSources.length;
    const buildStartedAt = performance.now();
    let disposed = false;

    resolveBevyEntityAtlasSnapshot(bevyEntityAtlasSources, bevyEntityAtlasKey)
      .then(({ atlas, source, prebuiltKey }) => {
        if (disposed || bevyEntityAtlasRequestRef.current?.requestId !== requestId) {
          return;
        }
        if (source === "live") {
          bevyEntityAtlasStats.builds += 1;
        } else if (source === "prebuilt") {
          bevyEntityAtlasStats.prebuiltHits += 1;
        } else {
          bevyEntityAtlasStats.persistentHits += 1;
        }
        bevyEntityAtlasStats.lastBuildMs = Math.round(performance.now() - buildStartedAt);
        bevyEntityAtlasStats.lastSource = source;
        bevyEntityAtlasStats.lastPrebuiltKey = prebuiltKey ?? null;
        cacheBevyEntityAtlas(atlas);
        bevyEntityAtlasRequestRef.current = null;
        setBevyEntityAtlas(atlas);
      })
      .catch(() => {
        bevyEntityAtlasStats.failures += 1;
        if (bevyEntityAtlasRequestRef.current?.requestId === requestId) {
          bevyEntityAtlasRequestRef.current = null;
        }
      });

    return () => {
      disposed = true;
    };
  }, [useGpuEntityRenderer, useBevyEntityAtlas, bevyEntityAtlasKey, bevyEntityAtlas?.key]);

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
    void preloadSceneAssetUrls(urls, 5_000, { allowPartialReady: true }).then((readiness) => {
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
              floorSprites={mapGpuActive ? EMPTY_VIEWPORT_MAP_SPRITES.floor : viewportMapSprites.floor}
              cameraOffset={playerCameraMotionOffset}
            />
          ) : null}

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

          <OriginalClientSceneVisualLayers
            screen={screen}
            t={t}
            world={world}
            player={player}
            selectedEntity={selectedEntity}
            viewportGroundDrops={viewportGroundDrops}
            viewportMapSprites={mapGpuActive ? EMPTY_VIEWPORT_MAP_SPRITES : viewportMapSprites}
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
              selectedPortraitFrame={activeSelectPortraitFrame}
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
const EMPTY_MAP_TILE_DRAW_LIST: MapTileDraw[] = [];
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
}: {
  enabled: boolean;
  player: DisplayEntity | null;
  viewportEntitySprites: Array<{
    entity: DisplayEntity & { dx: number; dy: number };
    sprite: {
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
      const entityMotionOffset = isPlayer
        ? EMPTY_VIEWPORT_OFFSET
        : entityMotionOffsetForEntity(entity, entityMotionSnapshots, motionNow);
      const cameraOffset = isPlayer ? EMPTY_VIEWPORT_OFFSET : playerCameraMotionOffset;
      const rootLeft =
        VIEWPORT_ENTITY_LEFT_ORIGIN + entity.dx * VIEWPORT_CELL_WIDTH + cameraOffset.x + entityMotionOffset.x;
      const rootTop =
        VIEWPORT_ENTITY_TOP_ORIGIN + entity.dy * VIEWPORT_CELL_HEIGHT + cameraOffset.y + entityMotionOffset.y;
      const depth = viewportDepthForCell(entity.x, entity.y, viewportDepthPlayer, 64);
      const layers = sprite
        ? [
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

function cacheBevyEntityAtlas(atlas: BevyEntityAtlasSnapshot) {
  bevyEntityAtlasLatestSnapshot = atlas;
  bevyEntityAtlasCache.set(atlas.key, atlas);
  while (bevyEntityAtlasCache.size > BEVY_ENTITY_ATLAS_CACHE_LIMIT) {
    const oldest = bevyEntityAtlasCache.keys().next().value;
    if (!oldest) break;
    bevyEntityAtlasCache.delete(oldest);
  }
}

function getCachedBevyEntityAtlas(key: string) {
  const cached = bevyEntityAtlasCache.get(key);
  if (!cached) {
    return null;
  }
  bevyEntityAtlasCache.delete(key);
  bevyEntityAtlasCache.set(key, cached);
  return cached;
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
    bevyEntityAtlasStats.persistentWrites += 1;
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
  const results = await Promise.all(urls.map((url) => preloadSceneImage(url, timeoutMs)));
  const loaded = results.filter((result) => result.loaded).length;
  const failedUrls = results.filter((result) => !result.loaded).map((result) => result.url);
  const minimumLoaded = Math.min(urls.length, options.minLoaded ?? SCENE_INTERACTION_MIN_PRELOADED_URLS);
  const partialReady = options.allowPartialReady === true && minimumLoaded > 0 && loaded >= minimumLoaded;
  const ready = failedUrls.length === 0 || partialReady;
  const timedOut = performance.now() - startedAt >= timeoutMs - 16 && failedUrls.length > 0;

  return {
    key: "scene-assets",
    ready,
    interactionReady: ready,
    visualReady: failedUrls.length === 0,
    status: ready ? "ready" : timedOut ? "timeout" : "loading",
    total: urls.length,
    loaded,
    failed: failedUrls.length,
    pending: 0,
    durationMs: Math.round(performance.now() - startedAt),
    failedUrls: failedUrls.slice(0, 20),
  };
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
