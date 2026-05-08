"use client";

import { useCallback, useEffect, useMemo, useRef, useState, type CSSProperties, type FormEvent, type MouseEvent } from "react";

import {
  ORIGINAL_UI,
  type ClientScreen,
  type CharacterTabKey,
  type InventoryTabKey,
  type SpriteState,
} from "../lib/original-ui";
import {
  frameMetaForIndex,
  loadOriginalSceneSpriteLibrary,
  normalizeSceneSpriteLibraryKey,
  originalSceneSpriteLibraryExists,
  type OriginalSceneSpriteFrameMeta,
  type OriginalSceneSpriteLibraryMeta,
} from "../lib/original-scene-sprite-meta";
import {
  SELECT_PORTRAIT_ANCHOR,
  SELECT_PORTRAIT_ANIMATIONS,
  type SelectPortraitFrame,
  type SelectPortraitKey,
} from "../lib/select-portraits";
import miniMapMeta from "../public/original-ui/MMap/meta.json";
import {
  buildTranslator,
  formatRuntimeMessage,
  formatRuntimePhase,
  languageLocale,
  languageNativeName,
  SUPPORTED_LANGUAGES,
  type Mir2Language,
} from "../lib/localization";
import type { OriginalMapRegion, OriginalMapSpriteFrame } from "../lib/scene-types";
import {
  CRYSTAL_GAME_SHOP_ITEM_INFO_BY_INDEX,
  CRYSTAL_GAME_SHOP_ITEMS,
} from "../lib/generated/crystal-game-shop-data";
import { CRYSTAL_BIG_MAP_NPCS } from "../lib/generated/crystal-npc-info-data";

type MiniMapLibraryMeta = {
  frames: Array<{
    index: number;
    width: number;
    height: number;
    path: string;
  }>;
};

const MINI_MAP_ASSETS = new Map(
  (miniMapMeta as MiniMapLibraryMeta).frames.map((frame) => [
    frame.index,
    { src: frame.path, width: frame.width, height: frame.height },
  ]),
);

type EntityKind = "selfPlayer" | "player" | "monster" | "npc";
type ItemContainer = "bag1" | "bag2" | "quest" | "belt" | "storage";
type EntityClassKey = "warrior" | "wizard" | "taoist" | "assassin" | "archer";
type EntityGenderKey = "male" | "female";
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
type EntitySprite = {
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

type EntitySpriteAnimationState =
  | "standing"
  | "walking"
  | "running"
  | "attackMelee"
  | "attackRange"
  | "struck"
  | "dying"
  | "dead"
  | "reviving";

type EntityMotionSnapshot = {
  fromX: number;
  fromY: number;
  toX: number;
  toY: number;
  animationState: EntitySpriteAnimationState;
  startedAt: number;
  expiresAt: number;
};

type DisplayEntity = {
  objectId: string;
  kind: EntityKind;
  name: string;
  x: number;
  y: number;
  direction?: string;
  classKey?: EntityClassKey;
  genderKey?: EntityGenderKey;
  level?: number;
  hp?: number;
  maxHp?: number;
  nameColourArgb?: number;
  dead?: boolean;
  sprite?: EntitySprite | null;
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

type DisplayProjectile = {
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

type DisplayItem = {
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

type ItemActionRef = Pick<DisplayItem, "key" | "uniqueId" | "slot" | "container">;
type EquipmentActionRef = Pick<DisplayEquipmentItem, "slot">;
type MoveItemRef = Pick<DisplayItem, "uniqueId" | "slot" | "container">;
type MergeItemRef = Pick<DisplayItem, "uniqueId" | "slot" | "container">;

type DisplayEquipmentItem = {
  slot: EquipmentSlot;
  name: string;
  icon: number;
  description: string;
  durabilityCurrent: number;
  durabilityMax: number;
  attack: number;
  defence: number;
};

type DisplayQuest = {
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

type DisplayKnownSkill = {
  key: string;
  name: string;
  description: string;
  cooldownRemainingTicks: number;
};

type DisplayActiveBuff = {
  key: string;
  name: string;
  description: string;
  remainingTicks: number;
  attackBonus: number;
  defenceBonus: number;
};

type DisplayNpcDialog = {
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

type DisplayLogLine = {
  text: string;
  tone: "chat" | "system" | "network";
  channel:
    | "normal"
    | "shout"
    | "trade"
    | "whisper"
    | "group"
    | "guild"
    | "system"
    | "hint"
    | "server"
    | "announcement"
    | "network";
};

type DisplayWorld = {
  connected: boolean;
  mapTitle: string | null;
  mapFileName: string | null;
  inSafeZone: boolean;
  miniMapIndex: number | null;
  bigMapIndex?: number | null;
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
};

type DisplayMailMessage = {
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

type SelectCharacterEntry = {
  index: number;
  name: string;
  level: number;
  classKey: "warrior" | "wizard" | "taoist" | "assassin" | "archer";
  gender: "male" | "female";
  lastAccess: string;
};

type OriginalClientShellProps = {
  language: Mir2Language;
  screen: ClientScreen;
  runtimePhase: string;
  runtimeMessage: string;
  wsState: string;
  world: DisplayWorld;
  player: DisplayEntity | null;
  predictedPlayerPosition: { x: number; y: number } | null;
  selectedEntity: DisplayEntity | null;
  sortedEntities: DisplayEntity[];
  viewportEntities: Array<DisplayEntity & { dx: number; dy: number }>;
  viewportTiles: Array<{ x: number; y: number; dx: number; dy: number }>;
  logs: DisplayLogLine[];
  accountId: string;
  password: string;
  chatMessage: string;
  loginBusy: boolean;
  loginError: string | null;
  characters: SelectCharacterEntry[];
  selectedCharacterIndex: number;
  showInventory: boolean;
  showCharacter: boolean;
  activeInventoryTab: InventoryTabKey;
  activeCharacterTab: CharacterTabKey;
  storageServiceOpenVersion: number;
  onLanguageChange: (language: Mir2Language) => void;
  onAccountIdChange: (value: string) => void;
  onPasswordChange: (value: string) => void;
  onChatMessageChange: (value: string) => void;
  onCreateAccount: () => void;
  onSubmitLogin: () => void;
  onQuickEnter: () => void;
  onResetClient: () => void;
  onExitSelect: () => void;
  onSendChat: () => void;
  onRentExpandedStorage: () => void;
  onLogout: () => void;
  onCreateCharacter: () => void;
  onDeleteCharacter: () => void;
  onUseItem: (item: ItemActionRef) => void;
  onDropItem: (item: ItemActionRef) => void;
  onEquipItem: (item: ItemActionRef, slot: EquipmentSlot) => void;
  onRemoveItem: (item: EquipmentActionRef) => void;
  onMoveItem: (item: MoveItemRef, toSlot: number) => void;
  onMergeItem: (from: MergeItemRef, to: MergeItemRef) => void;
  onSplitItem: (item: ItemActionRef, count: number) => void;
  onStoreItem: (item: MoveItemRef, toSlot: number) => void;
  onTakeBackItem: (item: MoveItemRef, toSlot: number) => void;
  onUnlockStorage: (password: string) => void;
  onSetStoragePassword: (currentPassword: string, newPassword: string) => void;
  onRemoveStoragePassword: (currentPassword: string) => void;
  onSellItem: (item: ItemActionRef, count: number) => void;
  onDropGold: (amount: number) => void;
  onRepairItem: (item: EquipmentActionRef) => void;
  onSpecialRepairItem: (item: EquipmentActionRef) => void;
  onCastSkill: (skillKey: string) => void;
  onTransferMap: (transferKey: string) => void;
  onClaimMail: (mailId: number) => void;
  onDeleteMail: (mailId: number) => void;
  onBuyGameShopItem: (gameShopIndex: number, quantity: number, paymentType: "gold" | "credit") => void;
  onRunStage5Command: (action: string, args?: string[]) => void;
  onSendClientCommand: (command: Record<string, unknown>) => void;
  transferOptions: SystemMenuTransferOption[];
  onToggleCharacter: () => void;
  onToggleInventory: () => void;
  onCloseCharacter: () => void;
  onCloseInventory: () => void;
  onOpenCharacterTab: (tab: CharacterTabKey) => void;
  onOpenInventoryTab: (tab: InventoryTabKey) => void;
  onViewportTileClick: (x: number, y: number) => void;
  onViewportTileSecondaryAction: (x: number, y: number) => void;
  onViewportTileStepClick: (x: number, y: number) => void;
  onViewportTileStepSecondaryAction: (x: number, y: number) => void;
  onViewportDirectionStep: (x: number, y: number, mode: "walk" | "run") => void;
  onPickGroundDrop: (objectId: string) => void;
  onSelectEntity: (objectId: string) => void;
  onActivateEntity: (objectId: string) => void;
  onApproachTarget: () => void;
  onPrimaryTargetAction: () => void;
  onSelectNpcDialogTarget: (target: string) => void;
  onSubmitNpcInput: (value: string) => void;
  onSelectCharacter: (index: number) => void;
  onEnterWorld: () => void;
  targetDistance: number | null;
  entityKindClassName: (kind: EntityKind) => string;
};

const VIEWPORT_CELL_WIDTH = 48;
const VIEWPORT_CELL_HEIGHT = 32;
const VIEWPORT_OFFSET_X = Math.floor(ORIGINAL_UI.game.sceneWidth / 2 / VIEWPORT_CELL_WIDTH);
const VIEWPORT_OFFSET_Y = Math.floor(ORIGINAL_UI.game.sceneHeight / 2 / VIEWPORT_CELL_HEIGHT) - 1;
const VIEWPORT_RANGE_X = VIEWPORT_OFFSET_X + 6;
const VIEWPORT_RANGE_Y = VIEWPORT_OFFSET_Y + 6;
const VIEWPORT_TILE_LEFT_ORIGIN = VIEWPORT_OFFSET_X * VIEWPORT_CELL_WIDTH - VIEWPORT_OFFSET_X;
const VIEWPORT_TILE_TOP_ORIGIN = VIEWPORT_OFFSET_Y * VIEWPORT_CELL_HEIGHT;
const VIEWPORT_TILE_CENTER_X = VIEWPORT_TILE_LEFT_ORIGIN + VIEWPORT_CELL_WIDTH / 2;
const VIEWPORT_TILE_CENTER_Y = VIEWPORT_TILE_TOP_ORIGIN + VIEWPORT_CELL_HEIGHT / 2;
const VIEWPORT_ENTITY_LEFT_ORIGIN = VIEWPORT_OFFSET_X * VIEWPORT_CELL_WIDTH;
const VIEWPORT_ENTITY_TOP_ORIGIN = VIEWPORT_OFFSET_Y * VIEWPORT_CELL_HEIGHT;
const CRYSTAL_MOVE_INPUT_INTERVAL_MS = 100;
const CRYSTAL_MOVE_FRAME_COUNT = 6;
const CRYSTAL_MOVE_FRAME_INTERVAL_MS = 100;
const MAX_PREDICTED_PLAYER_LEAD_TILES = 10;
const LOGIN_STATIC_BACKGROUND_FRAME = 0;
const LOGIN_TRANSITION_FRAME_MS = 180;
const ORIGINAL_AUDIO = {
  loginMusic: "/original-ui/Sound/Login2.wav",
  loginEffect: "/original-ui/Sound/100.wav",
  selectMusic: "/original-ui/Sound/Select2.wav",
} as const;
const ORIGINAL_MUSIC_VOLUME = 0.72;
const ORIGINAL_EFFECT_VOLUME = 0.86;

type TranslateFn = (
  key: string,
  args?: Array<string | number>,
  fallback?: string,
) => string;

type HeldScenePointer = {
  button: 0 | 2;
  sceneX: number;
  sceneY: number;
  startedAt: number;
  dispatched: boolean;
  tileX?: number;
  tileY?: number;
};

type CrystalGameShopEntry = {
  item_index: number;
  game_shop_index: number;
  item_name: string;
  gold_price: number;
  credit_price: number;
  count: number;
  class: string;
  category: string;
  stock: number;
  stock_level: number;
};

type CrystalItemEntry = {
  image: number;
  item_type: number;
};

type GameShopSectionFilter = "all" | "top" | "deals" | "new";
type GameShopClassFilter = "all" | EntityClassKey;
type GameShopPaymentType = "gold" | "credit";

const GAME_SHOP_ITEMS_PER_PAGE = 8;
const GAME_SHOP_CLASS_FILTERS: GameShopClassFilter[] = ["all", "warrior", "assassin", "taoist", "wizard", "archer"];
const GAME_SHOP_PREVIEW_ITEM_TYPES = new Set([1, 2, 19, 37]);
const BIG_MAP_NPC_INDEX = new Map(
  CRYSTAL_BIG_MAP_NPCS.map((npc) => [bigMapNpcKey(npc.map, npc.name, npc.x, npc.y), npc]),
);

type BigMapNpcRowView = {
  key: string;
  name: string;
  icon: number;
  x: number;
  y: number;
  canTeleportTo: boolean;
};

function selectedTargetActionLabel(
  t: TranslateFn,
  entity: DisplayEntity,
  targetDistance: number | null,
): string {
  const actionLabel =
    entity.kind === "monster"
      ? t("ui.attack", [], "Attack")
      : entity.kind === "npc"
        ? t("ui.talk", [], "Talk")
        : t("ui.approach", [], "Approach");

  if (targetDistance === null) {
    return `${t("ui.target", [], "Target")} · ${actionLabel}`;
  }

  return t("ui.targetDistance", [actionLabel, targetDistance], `${actionLabel} · ${targetDistance} tiles`);
}

function entityDisplayName(entity: DisplayEntity): string {
  return entity.name;
}

function entityDisplayLabelLines(entity: DisplayEntity): Array<{ text: string; role: "primary" | "secondary" }> {
  if (entity.kind !== "npc" && entity.kind !== "monster") {
    return [{ text: entity.name, role: "primary" }];
  }

  const parts = entity.name.split("_").filter(Boolean);
  if (parts.length <= 1) {
    return [{ text: entity.name.replace(/_/g, " "), role: "primary" }];
  }

  return parts.map((part, index) => ({ text: part, role: index === 0 ? "primary" : "secondary" }));
}

function desiredMusicForScreen(screen: ClientScreen, loginTransitionActive: boolean) {
  if (loginTransitionActive) {
    return ORIGINAL_AUDIO.loginMusic;
  }

  if (screen === "login") {
    return ORIGINAL_AUDIO.loginMusic;
  }

  if (screen === "select") {
    return ORIGINAL_AUDIO.selectMusic;
  }

  return null;
}

export function OriginalClientShell({
  language,
  screen,
  runtimePhase,
  runtimeMessage,
  wsState,
  world,
  player,
  predictedPlayerPosition,
  selectedEntity,
  sortedEntities,
  viewportEntities,
  viewportTiles,
  logs,
  accountId,
  password,
  chatMessage,
  loginBusy,
  loginError,
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
  onQuickEnter,
  onResetClient,
  onExitSelect,
  onSendChat,
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
  const [motionNow, setMotionNow] = useState(() => Date.now());
  const [sceneSpriteLibraries, setSceneSpriteLibraries] = useState<Record<string, OriginalSceneSpriteLibraryMeta>>({});
  const previousScreenRef = useRef<ClientScreen>(screen);
  const musicAudioRef = useRef<HTMLAudioElement | null>(null);
  const loginEffectAudioRef = useRef<HTMLAudioElement | null>(null);
  const activeMusicSrcRef = useRef<string | null>(null);
  const pendingMusicSrcRef = useRef<string | null>(null);
  const missingSceneSpriteLibrariesRef = useRef<Set<string>>(new Set());
  const entityMotionSnapshotsRef = useRef<Record<string, EntityMotionSnapshot>>({});
  const stageFrameRef = useRef<HTMLDivElement | null>(null);
  const heldScenePointerRef = useRef<HeldScenePointer | null>(null);
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

  const syncMusic = useCallback((src: string | null) => {
    pendingMusicSrcRef.current = src;

    if (!src) {
      musicAudioRef.current?.pause();
      activeMusicSrcRef.current = null;
      return;
    }

    const audio = musicAudioRef.current ?? new Audio();
    musicAudioRef.current = audio;

    if (activeMusicSrcRef.current !== src) {
      audio.pause();
      audio.src = src;
      audio.currentTime = 0;
      activeMusicSrcRef.current = src;
    }

    audio.loop = true;
    audio.volume = ORIGINAL_MUSIC_VOLUME;
    void audio.play().catch(() => undefined);
  }, []);

  const playLoginEffect = useCallback(() => {
    const audio = loginEffectAudioRef.current ?? new Audio();
    loginEffectAudioRef.current = audio;
    audio.src = ORIGINAL_AUDIO.loginEffect;
    audio.currentTime = 0;
    audio.volume = ORIGINAL_EFFECT_VOLUME;
    void audio.play().catch(() => undefined);
  }, []);

  useEffect(() => {
    const handleUserAudioGesture = () => syncMusic(pendingMusicSrcRef.current);

    window.addEventListener("pointerdown", handleUserAudioGesture, true);
    window.addEventListener("keydown", handleUserAudioGesture, true);

    return () => {
      window.removeEventListener("pointerdown", handleUserAudioGesture, true);
      window.removeEventListener("keydown", handleUserAudioGesture, true);
      musicAudioRef.current?.pause();
      loginEffectAudioRef.current?.pause();
    };
  }, [syncMusic]);

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

      const target = event.target;
      if (
        target instanceof HTMLInputElement ||
        target instanceof HTMLTextAreaElement ||
        target instanceof HTMLSelectElement ||
        (target instanceof HTMLElement && target.isContentEditable)
      ) {
        return;
      }

      if (selectedEntity && !selectedEntity.dead) {
        if (event.key === " " || event.key === "Enter") {
          event.preventDefault();
          onPrimaryTargetAction();
          return;
        }

        if (event.key.toLowerCase() === "a") {
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

  useEffect(() => {
    if (screen !== "game") {
      return;
    }

    let animationFrame = 0;
    const updateMotionClock = () => {
      setMotionNow(Date.now());
      animationFrame = window.requestAnimationFrame(updateMotionClock);
    };
    animationFrame = window.requestAnimationFrame(updateMotionClock);

    return () => window.cancelAnimationFrame(animationFrame);
  }, [screen]);

  const renderPlayer = useMemo(() => (
    player &&
    predictedPlayerPosition &&
    Math.max(Math.abs(player.x - predictedPlayerPosition.x), Math.abs(player.y - predictedPlayerPosition.y)) <=
      MAX_PREDICTED_PLAYER_LEAD_TILES
      ? { ...player, x: predictedPlayerPosition.x, y: predictedPlayerPosition.y }
      : player
  ), [player, predictedPlayerPosition]);

  useEffect(() => {
    if (screen !== "game") {
      return;
    }

    const libraries = new Set<string>();
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

    const missingLibraries = [...libraries].filter((libraryKey) => !(libraryKey in sceneSpriteLibraries));
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
          missingSceneSpriteLibrariesRef.current.add(libraryKey);
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
  }, [sceneSpriteLibraries, screen, world.entities]);

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
  const viewportMapSprites = renderPlayer
    ? buildViewportMapSprites(world, renderPlayer, sceneSpriteFrameIndex)
    : EMPTY_VIEWPORT_MAP_SPRITES;
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
  const showSyntheticScene =
    screen === "game" && !viewportMapSprites.floor.length && Boolean(world.originalMapRegion);

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
      x: Math.round(
        basePlayer.x +
          (sceneX - VIEWPORT_TILE_CENTER_X - latest.playerCameraMotionOffset.x) / VIEWPORT_CELL_WIDTH,
      ),
      y: Math.round(
        basePlayer.y +
          (sceneY - VIEWPORT_TILE_CENTER_Y - latest.playerCameraMotionOffset.y) / VIEWPORT_CELL_HEIGHT,
      ),
    };
  }

  function dispatchSceneMoveInput(pointer: HeldScenePointer) {
    if (latestMoveInputRef.current.screen !== "game") return;
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
    if (screen !== "game" || !player) {
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
    if (!held || screen !== "game") {
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
    };
    window.addEventListener("mouseup", stop);
    window.addEventListener("blur", stop);

    return () => {
      window.clearInterval(timer);
      window.removeEventListener("mouseup", stop);
      window.removeEventListener("blur", stop);
    };
  }, [screen, onViewportDirectionStep]);

  return (
    <main className="mir-client-page">
      <section className="mir-stage">
        <div
          ref={stageFrameRef}
          className="client-stage-frame"
          onMouseDownCapture={(event) => {
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
          <canvas id="mir2-web3-canvas" />
          {screen === "game" ? (
            <GameSceneBackdrop
              world={world}
              player={player}
              floorSprites={viewportMapSprites.floor}
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
                  left: `${VIEWPORT_TILE_CENTER_X + tile.dx * VIEWPORT_CELL_WIDTH + playerCameraMotionOffset.x}px`,
                  top: `${VIEWPORT_TILE_CENTER_Y + tile.dy * VIEWPORT_CELL_HEIGHT + playerCameraMotionOffset.y}px`,
                }}
                data-ui-interactive="true"
                onMouseDown={(event) => {
                  if (event.button !== 0 && event.button !== 2) {
                    return;
                  }

                  event.stopPropagation();
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

          <div className={`viewport-drop-overlay ${screen !== "game" ? "hidden" : ""}`}>
            {viewportGroundDrops.map((drop) => (
              <button
                key={`drop-${drop.objectId}`}
                type="button"
                className="ground-drop-marker"
                style={{
                  left: `${VIEWPORT_TILE_CENTER_X + drop.dx * VIEWPORT_CELL_WIDTH + playerCameraMotionOffset.x}px`,
                  top: `${VIEWPORT_TILE_CENTER_Y + drop.dy * VIEWPORT_CELL_HEIGHT + playerCameraMotionOffset.y - 12}px`,
                  zIndex: viewportDepthForCell(drop.x, drop.y, viewportDepthPlayer, 16),
                }}
                onClick={() => onPickGroundDrop(drop.objectId)}
                data-ui-interactive="true"
                title={`${drop.name} x${drop.quantity}`}
              >
                <span className="drop-dot" />
                <span className="drop-label" style={{ color: argbToCssColor(drop.nameColourArgb) }}>
                  {drop.quantity > 1 ? `${drop.name} x${drop.quantity}` : drop.name}
                </span>
              </button>
            ))}
          </div>

          <div className={`viewport-sprite-overlay ${screen !== "game" ? "hidden" : ""}`}>
            {viewportMapSprites.objects.map((sprite) => (
              <img
                key={sprite.key}
                className="scene-map-object-sprite"
                src={mapSpriteRenderPath(sprite.path)}
                alt=""
                draggable={false}
                data-map-sprite-path={sprite.path}
                data-map-render-path={mapSpriteRenderPath(sprite.path)}
                data-map-cell-x={sprite.cellX}
                data-map-cell-y={sprite.cellY}
                style={{
                  left: sprite.left + playerCameraMotionOffset.x,
                  top: sprite.top + playerCameraMotionOffset.y,
                  width: sprite.width,
                  height: sprite.height,
                  mixBlendMode: mapSpriteBlendMode(sprite.path),
                  zIndex: sprite.zIndex,
                }}
              />
            ))}
            {viewportEntitySprites.map(({ entity, sprite }) => {
              const isPlayer = player?.objectId === entity.objectId;
              const entityMotionOffset = isPlayer
                ? EMPTY_VIEWPORT_OFFSET
                : entityMotionOffsetForEntity(entity, entityMotionSnapshotsRef.current, motionNow);
              const cameraOffset = isPlayer ? EMPTY_VIEWPORT_OFFSET : playerCameraMotionOffset;
              const label = entityDisplayName(entity);
              const hitWidth = Math.max(sprite?.body?.width ?? 48, sprite?.hair?.width ?? 0, 48);
              const hitHeight = Math.max(sprite?.body?.height ?? 64, sprite?.hair?.height ?? 0, 64);
              const healthRatio =
                isPlayer && entity.hp !== undefined && entity.maxHp ? ratio(entity.hp, entity.maxHp) : null;

              return (
                <div
                  key={`sprite-${entity.objectId}`}
                  className={`entity-sprite-stack ${entityKindClassName(entity.kind)} ${entity.objectId === selectedEntity?.objectId ? "selected" : ""} ${entity.dead ? "dead" : ""} ${isEntityAttacking(entity, motionNow) ? "attacking" : ""} ${isEntityStruck(entity, motionNow) ? "struck" : ""} ${isEntityReviving(entity, motionNow) ? "reviving" : ""}`}
                  style={{
                    left: `${VIEWPORT_ENTITY_LEFT_ORIGIN + entity.dx * VIEWPORT_CELL_WIDTH + cameraOffset.x + entityMotionOffset.x}px`,
                    top: `${VIEWPORT_ENTITY_TOP_ORIGIN + entity.dy * VIEWPORT_CELL_HEIGHT + cameraOffset.y + entityMotionOffset.y}px`,
                    zIndex: viewportDepthForCell(entity.x, entity.y, viewportDepthPlayer, 64),
                  }}
                  data-ui-interactive="true"
                  onMouseDown={(event) => {
                    if (event.button !== 0 && event.button !== 2) {
                      return;
                    }
                    event.preventDefault();
                    event.stopPropagation();
                    onActivateEntity(entity.objectId);
                  }}
                  onContextMenu={(event) => {
                    event.preventDefault();
                    event.stopPropagation();
                    onActivateEntity(entity.objectId);
                  }}
                >
                  {healthRatio !== null ? (
                    <div className="entity-health-bar">
                      <span style={{ width: `${healthRatio * 100}%` }} />
                    </div>
                  ) : null}
                  <button
                    type="button"
                    className="entity-sprite-hit"
                    style={{
                      left: `${-hitWidth / 2}px`,
                      top: `${-hitHeight}px`,
                      width: `${hitWidth}px`,
                      height: `${hitHeight}px`,
                    }}
                    aria-label={label}
                  />
                  {sprite?.rearWeapons.map((weapon, index) => (
                    <img
                      key={`rear-${entity.objectId}-${index}-${weapon.path}`}
                      className="entity-sprite-layer weapon rear"
                      src={weapon.path}
                      alt=""
                      draggable={false}
                      style={{
                        left: weapon.x,
                        top: weapon.y,
                        width: weapon.width,
                        height: weapon.height,
                      }}
                    />
                  ))}
                  {sprite?.body ? (
                    <img
                      className="entity-sprite-layer body"
                      src={sprite.body.path}
                      alt=""
                      draggable={false}
                      style={{
                        left: sprite.body.x,
                        top: sprite.body.y,
                        width: sprite.body.width,
                        height: sprite.body.height,
                      }}
                    />
                  ) : null}
                  {sprite?.hair ? (
                    <img
                      className="entity-sprite-layer hair"
                      src={sprite.hair.path}
                      alt=""
                      draggable={false}
                      style={{
                        left: sprite.hair.x,
                        top: sprite.hair.y,
                        width: sprite.hair.width,
                        height: sprite.hair.height,
                      }}
                    />
                  ) : null}
                  {sprite?.frontWeapons.map((weapon, index) => (
                    <img
                      key={`front-${entity.objectId}-${index}-${weapon.path}`}
                      className="entity-sprite-layer weapon front"
                      src={weapon.path}
                      alt=""
                      draggable={false}
                      style={{
                        left: weapon.x,
                        top: weapon.y,
                        width: weapon.width,
                        height: weapon.height,
                      }}
                    />
                  ))}
                  {entity.kind === "npc" ? (
                    (() => {
                      const questIcon = questIconForEntity(entity, world.questLog, sceneSpriteFrameIndex);
                      return questIcon ? (
                        <img
                          className="entity-quest-icon"
                          src={questIcon}
                          alt=""
                          draggable={false}
                          style={{ top: nameplateTopOffset(sprite) - 30 }}
                        />
                      ) : null;
                    })()
                  ) : null}
                </div>
              );
            })}
            {viewportProjectiles.map((projectile) => {
              const currentLeft =
                VIEWPORT_TILE_CENTER_X +
                (projectile.fromDx + (projectile.toDx - projectile.fromDx) * projectile.progress) * VIEWPORT_CELL_WIDTH +
                playerCameraMotionOffset.x;
              const currentTop =
                VIEWPORT_TILE_CENTER_Y +
                (projectile.fromDy + (projectile.toDy - projectile.fromDy) * projectile.progress) * VIEWPORT_CELL_HEIGHT +
                playerCameraMotionOffset.y -
                28;
              const deltaX = (projectile.toDx - projectile.fromDx) * VIEWPORT_CELL_WIDTH;
              const deltaY = (projectile.toDy - projectile.fromDy) * VIEWPORT_CELL_HEIGHT;
              const angle = Math.atan2(deltaY, deltaX);

              return (
                <div
                  key={projectile.key}
                  className="viewport-projectile"
                  style={{
                    left: currentLeft,
                    top: currentTop,
                    transform: `translate(-50%, -50%) rotate(${angle}rad)`,
                    zIndex: viewportDepthForCell(projectile.toX, projectile.toY, viewportDepthPlayer, 80),
                  }}
                />
              );
            })}
          </div>

          <div className={`viewport-entity-overlay ${screen !== "game" ? "hidden" : ""}`}>
            {player
              ? viewportEntitySprites.map(({ entity, sprite }) => {
                  const isPlayer = player.objectId === entity.objectId;
                  const entityMotionOffset = isPlayer
                    ? EMPTY_VIEWPORT_OFFSET
                    : entityMotionOffsetForEntity(entity, entityMotionSnapshotsRef.current, motionNow);
                  const cameraOffset = isPlayer ? EMPTY_VIEWPORT_OFFSET : playerCameraMotionOffset;
                  const labelLines = entityDisplayLabelLines(entity);

                  return (
                    <button
                      key={`entity-${entity.objectId}`}
                      type="button"
                      className={`entity-nameplate ${entityKindClassName(entity.kind)} ${entity.objectId === selectedEntity?.objectId ? "selected" : ""}`}
                      style={{
                        left: `${VIEWPORT_ENTITY_LEFT_ORIGIN + entity.dx * VIEWPORT_CELL_WIDTH + cameraOffset.x + entityMotionOffset.x + 25}px`,
                        top: `${VIEWPORT_ENTITY_TOP_ORIGIN + entity.dy * VIEWPORT_CELL_HEIGHT + cameraOffset.y + entityMotionOffset.y + entityNameplateTopOffset(entity, sprite)}px`,
                        "--entity-name-color": entityNameplateColor(entity),
                      } as CSSProperties}
                      data-ui-interactive="true"
                      onClick={() => onActivateEntity(entity.objectId)}
                      onContextMenu={(event) => {
                        event.preventDefault();
                        event.stopPropagation();
                        onActivateEntity(entity.objectId);
                      }}
                    >
                      {labelLines.map((line, index) => (
                        <strong
                          key={`${entity.objectId}-label-${index}`}
                          className={line.role === "secondary" ? "entity-subname" : undefined}
                        >
                          {line.text}
                        </strong>
                      ))}
                      {entity.dead ? <strong className="entity-state-label">{t("ui.dead")}</strong> : null}
                    </button>
                  );
                })
              : null}
          </div>

          <div className={`viewport-vignette ${screen === "game" && viewportMapSprites.floor.length ? "hidden" : ""}`} />
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
              onLanguageChange={onLanguageChange}
              onAccountIdChange={onAccountIdChange}
              onPasswordChange={onPasswordChange}
              onCreateAccount={onCreateAccount}
              onSubmitLogin={onSubmitLogin}
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
        </div>
      </section>
    </main>
  );
}

function entityKindLabelKey(kind: EntityKind) {
  switch (kind) {
    case "selfPlayer":
      return "ui.self";
    case "player":
      return "ui.player";
    case "monster":
      return "ui.monster";
    case "npc":
      return "ui.npc";
  }
}

type LanguageSelectorProps = {
  language: Mir2Language;
  t: TranslateFn;
  onLanguageChange: (language: Mir2Language) => void;
  compact?: boolean;
  className?: string;
};

function LanguageSelector({
  language,
  t,
  onLanguageChange,
  compact = false,
  className = "",
}: LanguageSelectorProps) {
  const selectorClassName = ["language-selector", compact ? "compact" : "", className]
    .filter(Boolean)
    .join(" ");

  return (
    <section className={selectorClassName}>
      {compact ? null : (
        <div className="language-selector-copy">
          <strong>{t("ui.languageSettings")}</strong>
          <span>{t("ui.languageDescription")}</span>
        </div>
      )}
      <div className="language-selector-buttons">
        {SUPPORTED_LANGUAGES.map((option) => (
          <button
            key={option}
            type="button"
            className={`language-selector-button ${option === language ? "active" : ""}`}
            aria-pressed={option === language}
            onClick={() => onLanguageChange(option)}
          >
            {languageNativeName(option)}
          </button>
        ))}
      </div>
    </section>
  );
}


type GameUiSceneProps = {
  t: TranslateFn;
  locale: string;
  runtimeMessage: string;
  world: DisplayWorld;
  player: DisplayEntity | null;
  logs: DisplayLogLine[];
  chatMessage: string;
  showInventory: boolean;
  showCharacter: boolean;
  activeInventoryTab: InventoryTabKey;
  activeCharacterTab: CharacterTabKey;
  storageServiceOpenVersion: number;
  onChatMessageChange: (value: string) => void;
  onSendChat: () => void;
  onRentExpandedStorage: () => void;
  onLogout: () => void;
  onToggleCharacter: () => void;
  onToggleInventory: () => void;
  onCloseCharacter: () => void;
  onCloseInventory: () => void;
  onOpenCharacterTab: (tab: CharacterTabKey) => void;
  onOpenInventoryTab: (tab: InventoryTabKey) => void;
  onSelectNpcDialogTarget: (target: string) => void;
  onSubmitNpcInput: (value: string) => void;
  onUseItem: (item: ItemActionRef) => void;
  onDropItem: (item: ItemActionRef) => void;
  onEquipItem: (item: ItemActionRef, slot: EquipmentSlot) => void;
  onRemoveItem: (item: EquipmentActionRef) => void;
  onMoveItem: (item: MoveItemRef, toSlot: number) => void;
  onMergeItem: (from: MergeItemRef, to: MergeItemRef) => void;
  onSplitItem: (item: ItemActionRef, count: number) => void;
  onStoreItem: (item: MoveItemRef, toSlot: number) => void;
  onTakeBackItem: (item: MoveItemRef, toSlot: number) => void;
  onUnlockStorage: (password: string) => void;
  onSetStoragePassword: (currentPassword: string, newPassword: string) => void;
  onRemoveStoragePassword: (currentPassword: string) => void;
  onSellItem: (item: ItemActionRef, count: number) => void;
  onDropGold: (amount: number) => void;
  onRepairItem: (item: EquipmentActionRef) => void;
  onSpecialRepairItem: (item: EquipmentActionRef) => void;
  onCastSkill: (skillKey: string) => void;
  onTransferMap: (transferKey: string) => void;
  onClaimMail: (mailId: number) => void;
  onDeleteMail: (mailId: number) => void;
  onBuyGameShopItem: (gameShopIndex: number, quantity: number, paymentType: "gold" | "credit") => void;
  onRunStage5Command: (action: string, args?: string[]) => void;
  onSendClientCommand: (command: Record<string, unknown>) => void;
  transferOptions: SystemMenuTransferOption[];
};

function GameUiScene({
  t,
  locale,
  runtimeMessage,
  world,
  player,
  logs,
  chatMessage,
  showInventory,
  showCharacter,
  activeInventoryTab,
  activeCharacterTab,
  storageServiceOpenVersion,
  onChatMessageChange,
  onSendChat,
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
}: GameUiSceneProps) {
  const [showDuraPanel, setShowDuraPanel] = useState(false);
  const [showBelt, setShowBelt] = useState(true);
  const [beltVertical, setBeltVertical] = useState(false);
  const [activeChatFilter, setActiveChatFilter] = useState<ChatFilterKey>("all");
  const [chatExpanded, setChatExpanded] = useState(true);
  const [showChatSettings, setShowChatSettings] = useState(false);
  const [showMailPanel, setShowMailPanel] = useState(false);
  const [showBigMap, setShowBigMap] = useState(false);
  const [showReportPanel, setShowReportPanel] = useState(false);
  const [showSystemMenu, setShowSystemMenu] = useState(false);
  const [showGameShop, setShowGameShop] = useState(false);
  const [showSystemMenuFeaturePanel, setShowSystemMenuFeaturePanel] = useState<SystemMenuSurfacePanel | null>(null);
  const [dismissedDialogKey, setDismissedDialogKey] = useState<string | null>(null);

  const dialogKey = world.activeNpcDialog
    ? `${world.activeNpcDialog.npcObjectId}:${world.activeNpcDialog.title}:${world.worldTick}`
    : null;
  const visibleDialog =
    world.activeNpcDialog && dialogKey !== dismissedDialogKey ? world.activeNpcDialog : null;

  useEffect(() => {
    if (!dialogKey) {
      setDismissedDialogKey(null);
    } else if (dialogKey !== dismissedDialogKey) {
      setDismissedDialogKey(null);
    }
  }, [dialogKey, dismissedDialogKey]);

  return (
    <div className={`game-ui-scene ${originalMiniMapAssetPath(world.miniMapIndex) ? "with-mini-map" : "without-mini-map"}`}>
      <MiniMapPanel
        t={t}
        world={world}
        player={player}
        showMailPanel={showMailPanel}
        showBigMap={showBigMap}
        onToggleMail={() => setShowMailPanel((current) => !current)}
        onToggleBigMap={() => setShowBigMap((current) => !current)}
      />
      <DuraPanel
        t={t}
        visible={showDuraPanel}
        equipmentItems={world.equipmentItems}
        onToggle={() => setShowDuraPanel((current) => !current)}
      />
      {showBelt ? (
        <BeltDialog
          t={t}
          items={world.beltItems}
          vertical={beltVertical}
          onClose={() => setShowBelt(false)}
          onRotate={() => setBeltVertical((current) => !current)}
          onUseItem={onUseItem}
        />
      ) : null}
      <ChatFilterBar
        t={t}
        activeFilter={activeChatFilter}
        chatExpanded={chatExpanded}
        showSettings={showChatSettings}
        onSelectFilter={setActiveChatFilter}
        onSelectTrade={() => setActiveChatFilter("trade")}
        onToggleExpanded={() => setChatExpanded((current) => !current)}
        onToggleSettings={() => setShowChatSettings((current) => !current)}
        onToggleReport={() => setShowReportPanel((current) => !current)}
      />
      <ChatFrame
        t={t}
        runtimeMessage={runtimeMessage}
        logs={logs}
        chatMessage={chatMessage}
        hints={world.interactionHints}
        activeFilter={activeChatFilter}
        expanded={chatExpanded}
        showSettings={showChatSettings}
        onChatMessageChange={onChatMessageChange}
        onSendChat={onSendChat}
        onCloseSettings={() => setShowChatSettings(false)}
      />
      <MainHud
        t={t}
        connected={world.connected}
        mapTitle={world.mapTitle}
        player={player}
        world={world}
        showCharacter={showCharacter}
        showInventory={showInventory}
        activeCharacterTab={activeCharacterTab}
        activeInventoryTab={activeInventoryTab}
        onToggleCharacter={onToggleCharacter}
        onToggleInventory={onToggleInventory}
        onOpenCharacterTab={onOpenCharacterTab}
        onOpenInventoryTab={onOpenInventoryTab}
        onDropGold={() => onDropGold(100)}
        onLogout={onLogout}
        showGameShop={showGameShop}
        onToggleGameShop={() => setShowGameShop((current) => !current)}
        showMenu={showSystemMenu}
        onToggleMenu={() => setShowSystemMenu((current) => !current)}
      />
      {showMailPanel ? (
        <MailPanel
          t={t}
          mail={world.stage5Systems?.mail ?? []}
          onClaim={onClaimMail}
          onDelete={onDeleteMail}
          onClose={() => setShowMailPanel(false)}
        />
      ) : null}
      {showBigMap ? (
        <BigMapDialog
          t={t}
          world={world}
          player={player}
          onClose={() => setShowBigMap(false)}
        />
      ) : null}
      {showReportPanel ? <ReportPanel t={t} logs={logs} onClose={() => setShowReportPanel(false)} /> : null}
      {showSystemMenu ? (
        <SystemMenuPanel
          t={t}
          playerName={player?.name ?? null}
          playerPosition={player ? { x: player.x, y: player.y } : null}
          mapTitle={world.mapTitle}
          mapFileName={world.mapFileName}
          inSafeZone={world.inSafeZone}
          transferOptions={transferOptions}
          onOpenPanel={(panel) => {
            setShowSystemMenuFeaturePanel(panel);
            setShowSystemMenu(false);
          }}
          onClose={() => setShowSystemMenu(false)}
          onLogout={onLogout}
          onTransferMap={(transferKey) => {
            onTransferMap(transferKey);
            setShowSystemMenu(false);
          }}
        />
      ) : null}
      {showSystemMenuFeaturePanel ? (
        <SystemMenuFeaturePanel
          t={t}
          feature={showSystemMenuFeaturePanel}
          playerName={player?.name ?? null}
          world={world}
          onRunStage5Command={onRunStage5Command}
          onSendClientCommand={onSendClientCommand}
          onClose={() => {
            setShowSystemMenuFeaturePanel(null);
            setShowSystemMenu(true);
          }}
        />
      ) : null}
      {showGameShop ? (
        <GameShopWindow
          t={t}
          gold={world.gold}
          credits={world.credit}
          playerClass={player?.classKey ?? "warrior"}
          onBuy={onBuyGameShopItem}
          onClose={() => setShowGameShop(false)}
        />
      ) : null}
      {visibleDialog ? (
        <NpcDialogPanel
          t={t}
          dialog={visibleDialog}
          onClose={() => setDismissedDialogKey(dialogKey)}
          onSelectTarget={onSelectNpcDialogTarget}
          onSubmitInput={onSubmitNpcInput}
        />
      ) : null}
      {showInventory ? (
        <InventoryWindow
          t={t}
          locale={locale}
          activeTab={activeInventoryTab}
          world={world}
          storageServiceOpenVersion={storageServiceOpenVersion}
          onClose={onCloseInventory}
          onTabChange={onOpenInventoryTab}
          onUseItem={onUseItem}
          onDropItem={onDropItem}
          onEquipItem={onEquipItem}
          onMoveItem={onMoveItem}
          onMergeItem={onMergeItem}
          onSplitItem={onSplitItem}
          onStoreItem={onStoreItem}
          onTakeBackItem={onTakeBackItem}
          onRentExpandedStorage={onRentExpandedStorage}
          onUnlockStorage={onUnlockStorage}
          onSetStoragePassword={onSetStoragePassword}
          onRemoveStoragePassword={onRemoveStoragePassword}
          onSellItem={onSellItem}
          onDropGold={onDropGold}
        />
      ) : null}
      {showCharacter ? (
        <CharacterWindow
          t={t}
          activeTab={activeCharacterTab}
          onClose={onCloseCharacter}
          onTabChange={onOpenCharacterTab}
          player={player}
          world={world}
          onRemoveItem={onRemoveItem}
          onRepairItem={onRepairItem}
          onSpecialRepairItem={onSpecialRepairItem}
          onCastSkill={onCastSkill}
        />
      ) : null}
    </div>
  );
}

type ChatFilterKey = "all" | "shout" | "trade" | "whisper" | "lover" | "mentor" | "group" | "guild";

type SceneBackdropTile = {
  key: string;
  left: number;
  top: number;
  texture: string;
  tint: string;
};

type ViewportMapSprite = {
  key: string;
  path: string;
  cellX: number;
  cellY: number;
  left: number;
  top: number;
  width: number;
  height: number;
  zIndex: number;
};

type ViewportMapSprites = {
  floor: ViewportMapSprite[];
  objects: ViewportMapSprite[];
};

type ViewportOffset = {
  x: number;
  y: number;
};

type SystemMenuTransferOption = {
  key: string;
  label: string;
};

type SystemMenuButtonDefinition = {
  key: keyof typeof ORIGINAL_UI.menu.buttons;
  label: string;
  panel?: SystemMenuSurfacePanel;
  onClick?: () => void;
};

type SystemMenuFeaturePanel = "creature" | "mount" | "fishing";
type SystemMenuSocialPanel =
  | "ranking"
  | "friend"
  | "mentor"
  | "relationship"
  | "group"
  | "guild"
  | "trade"
  | "market"
  | "marriage"
  | "hero"
  | "itemRental";
type SystemMenuSurfacePanel = SystemMenuFeaturePanel | SystemMenuSocialPanel;

type SystemMenuSocialPanelMetric = {
  label: string;
  value: string;
};

type SystemMenuSocialPanelRow = {
  name: string;
  meta: string;
  note: string;
  metrics: SystemMenuSocialPanelMetric[];
};

type SystemMenuSocialPanelTab = {
  key: string;
  label: string;
  rows: SystemMenuSocialPanelRow[];
  actions: string[];
};

type SystemMenuSocialPanelDefinition = {
  subtitle: string;
  footer: string;
  tabs: SystemMenuSocialPanelTab[];
};

const SYSTEM_MENU_SOCIAL_PANEL_DEFINITIONS: Partial<Record<SystemMenuSocialPanel, SystemMenuSocialPanelDefinition>> = {
  ranking: {
    subtitle: "Crystal leaderboard for {player}",
    footer: "Compare, inspect, and whisper without leaving the menu.",
    tabs: [
      {
        key: "overall",
        label: "Overall",
        rows: [
          {
            name: "{player}",
            meta: "Current slot",
            note: "Safe-zone progress and route status are shown here.",
            metrics: [
              { label: "Rank", value: "01" },
              { label: "Score", value: "1,820" },
              { label: "Map", value: "Bichon" },
            ],
          },
          {
            name: "CrystalKnight",
            meta: "Front line",
            note: "Stable damage pressure and route clarity.",
            metrics: [
              { label: "Rank", value: "02" },
              { label: "Score", value: "1,774" },
              { label: "Map", value: "Border" },
            ],
          },
          {
            name: "MapScout",
            meta: "Traversal",
            note: "Fast map switching and arrival route status.",
            metrics: [
              { label: "Rank", value: "03" },
              { label: "Score", value: "1,709" },
              { label: "Map", value: "Arena" },
            ],
          },
        ],
        actions: ["Compare", "Inspect", "Whisper"],
      },
      {
        key: "class",
        label: "Class",
        rows: [
          {
            name: "Warrior",
            meta: "Melee ladder",
            note: "Heavy armor and steady damage still anchor the list.",
            metrics: [
              { label: "Best", value: "2,008" },
              { label: "Wins", value: "84" },
              { label: "Trend", value: "+12" },
            ],
          },
          {
            name: "Wizard",
            meta: "Burst ladder",
            note: "Magic burst remains the fastest route to the top.",
            metrics: [
              { label: "Best", value: "1,962" },
              { label: "Wins", value: "79" },
              { label: "Trend", value: "+8" },
            ],
          },
          {
            name: "Archer",
            meta: "Ranged ladder",
            note: "Long-range control is shown in the panel preview.",
            metrics: [
              { label: "Best", value: "1,955" },
              { label: "Wins", value: "88" },
              { label: "Trend", value: "+15" },
            ],
          },
        ],
        actions: ["Sort", "Filter", "Whisper"],
      },
      {
        key: "guild",
        label: "Guild",
        rows: [
          {
            name: "Obelisk",
            meta: "Prime guild",
            note: "Guild coordination, roster review, and notice checks.",
            metrics: [
              { label: "Members", value: "42" },
              { label: "Donation", value: "96%" },
              { label: "Status", value: "Open" },
            ],
          },
          {
            name: "Crystal",
            meta: "Support guild",
            note: "Guild searching and comparison details.",
            metrics: [
              { label: "Members", value: "31" },
              { label: "Donation", value: "88%" },
              { label: "Status", value: "Open" },
            ],
          },
          {
            name: "Mir2",
            meta: "Training guild",
            note: "Useful for quick roster and memo verification.",
            metrics: [
              { label: "Members", value: "27" },
              { label: "Donation", value: "74%" },
              { label: "Status", value: "Open" },
            ],
          },
        ],
        actions: ["Notice", "Inspect", "Chat"],
      },
    ],
  },
  friend: {
    subtitle: "Friends, block list, and memos for {player}",
    footer: "Whisper, memo, or inspect the current social list.",
    tabs: [
      {
        key: "friends",
        label: "Friends",
        rows: [
          {
            name: "Assistant_Jane",
            meta: "Online",
            note: "Helpful route checks and map labels are shared here.",
            metrics: [
              { label: "Map", value: "Bichon" },
              { label: "Mood", value: "Ready" },
              { label: "Note", value: "Escort" },
            ],
          },
          {
            name: "Merchant_Ruben",
            meta: "Away",
            note: "Inventory and trade context stay visible in the panel.",
            metrics: [
              { label: "Map", value: "Market" },
              { label: "Mood", value: "Away" },
              { label: "Note", value: "Mail" },
            ],
          },
          {
            name: "{player}",
            meta: "Local hero",
            note: "Your own row stays visible for quick status review.",
            metrics: [
              { label: "Map", value: "Bichon" },
              { label: "Mood", value: "Open" },
              { label: "Note", value: "Self" },
            ],
          },
        ],
        actions: ["Whisper", "Memo", "Inspect"],
      },
      {
        key: "blocks",
        label: "Block List",
        rows: [
          {
            name: "Spam_Filter",
            meta: "Muted",
            note: "Noise filtering is represented as a real row selection.",
            metrics: [
              { label: "Reason", value: "Spam" },
              { label: "Flag", value: "Muted" },
              { label: "Age", value: "12d" },
            ],
          },
          {
            name: "Trade_Auto",
            meta: "Muted",
            note: "Trade moderation list entries are kept here for visibility.",
            metrics: [
              { label: "Reason", value: "Spoof" },
              { label: "Flag", value: "Muted" },
              { label: "Age", value: "4d" },
            ],
          },
          {
            name: "Channel_Noise",
            meta: "Muted",
            note: "Channel list entry showing active moderation state.",
            metrics: [
              { label: "Reason", value: "Noise" },
              { label: "Flag", value: "Muted" },
              { label: "Age", value: "1d" },
            ],
          },
        ],
        actions: ["Unblock", "Memo", "Inspect"],
      },
      {
        key: "memo",
        label: "Memo",
        rows: [
          {
            name: "Bichon Route",
            meta: "Pinned memo",
            note: "The menu keeps saved route notes clickable.",
            metrics: [
              { label: "Tag", value: "Route" },
              { label: "State", value: "Pinned" },
              { label: "Age", value: "Today" },
            ],
          },
          {
            name: "Guild Invite",
            meta: "Pinned memo",
            note: "Stored invite memo with quick follow-up actions.",
            metrics: [
              { label: "Tag", value: "Invite" },
              { label: "State", value: "Pinned" },
              { label: "Age", value: "Today" },
            ],
          },
          {
            name: "Drop Check",
            meta: "Pinned memo",
            note: "Use this memo to keep route updates quick and visible.",
            metrics: [
              { label: "Tag", value: "Loot" },
              { label: "State", value: "Pinned" },
              { label: "Age", value: "Today" },
            ],
          },
        ],
        actions: ["Write", "Pin", "Inspect"],
      },
    ],
  },
  mentor: {
    subtitle: "Mentor and apprentice rollup for {player}",
    footer: "Review training rows or track a mentor request.",
    tabs: [
      {
        key: "mentor",
        label: "Mentor",
        rows: [
          {
            name: "Crystal_Sage",
            meta: "Mentor",
            note: "Guidance, tracks, and training notes stay visible.",
            metrics: [
              { label: "Rank", value: "S" },
              { label: "Focus", value: "Balance" },
              { label: "State", value: "Active" },
            ],
          },
          {
            name: "Field_Guide",
            meta: "Mentor",
            note: "Mentor contact state and availability.",
            metrics: [
              { label: "Rank", value: "A" },
              { label: "Focus", value: "Route" },
              { label: "State", value: "Active" },
            ],
          },
          {
            name: "{player}",
            meta: "Current trainee",
            note: "Your slot can still be clicked like a real mentor row.",
            metrics: [
              { label: "Rank", value: "B" },
              { label: "Focus", value: "Route" },
              { label: "State", value: "Active" },
            ],
          },
        ],
        actions: ["Track", "Teach", "Review"],
      },
      {
        key: "apprentices",
        label: "Apprentices",
        rows: [
          {
            name: "Rising_Hero",
            meta: "Level 24",
            note: "Apprentice roster rows can be selected and compared.",
            metrics: [
              { label: "Progress", value: "63%" },
              { label: "Focus", value: "Combat" },
              { label: "State", value: "Training" },
            ],
          },
          {
            name: "Map_Walker",
            meta: "Level 31",
            note: "Route familiarity and map travel are represented here.",
            metrics: [
              { label: "Progress", value: "71%" },
              { label: "Focus", value: "Travel" },
              { label: "State", value: "Training" },
            ],
          },
          {
            name: "Crystal_Reader",
            meta: "Level 19",
            note: "Level progression row with clear state updates.",
            metrics: [
              { label: "Progress", value: "42%" },
              { label: "Focus", value: "Info" },
              { label: "State", value: "Training" },
            ],
          },
        ],
        actions: ["Track", "Assign", "Review"],
      },
      {
        key: "requests",
        label: "Requests",
        rows: [
          {
            name: "Pending_Bond",
            meta: "Awaiting response",
            note: "Request rows mirror the kind of yes/no state Crystal uses.",
            metrics: [
              { label: "Age", value: "2h" },
              { label: "Type", value: "Mentor" },
              { label: "State", value: "Pending" },
            ],
          },
          {
            name: "Manual_Review",
            meta: "Awaiting response",
            note: "Alternate request row for mentor queue status checks.",
            metrics: [
              { label: "Age", value: "5h" },
              { label: "Type", value: "Train" },
              { label: "State", value: "Pending" },
            ],
          },
          {
            name: "Fallback_Pass",
            meta: "Awaiting response",
            note: "A pending request row for mentor actions.",
            metrics: [
              { label: "Age", value: "1d" },
              { label: "Type", value: "Trace" },
              { label: "State", value: "Pending" },
            ],
          },
        ],
        actions: ["Accept", "Track", "Review"],
      },
    ],
  },
  relationship: {
    subtitle: "Relationship, ring, and affinity for {player}",
    footer: "Review bond rows and inspect relationship status.",
    tabs: [
      {
        key: "lover",
        label: "Lover",
        rows: [
          {
            name: "Promise_Ring",
            meta: "Bonded",
            note: "Love-state rows turn into a real clickable selection.",
            metrics: [
              { label: "Affinity", value: "87%" },
              { label: "Gift", value: "Ready" },
              { label: "State", value: "Bonded" },
            ],
          },
          {
            name: "Shared_Route",
            meta: "Bonded",
            note: "Shared relationship status and route history.",
            metrics: [
              { label: "Affinity", value: "81%" },
              { label: "Gift", value: "Ready" },
              { label: "State", value: "Bonded" },
            ],
          },
          {
            name: "{player}",
            meta: "Bonded",
            note: "The player row can still be selected like a normal entry.",
            metrics: [
              { label: "Affinity", value: "90%" },
              { label: "Gift", value: "Ready" },
              { label: "State", value: "Bonded" },
            ],
          },
        ],
        actions: ["Gift", "Bond", "Inspect"],
      },
      {
        key: "affinity",
        label: "Affinity",
        rows: [
          {
            name: "Affinity_87",
            meta: "Gauge",
            note: "A progress-style row for the affinity tab.",
            metrics: [
              { label: "Level", value: "87" },
              { label: "Timer", value: "Ready" },
              { label: "State", value: "Warm" },
            ],
          },
          {
            name: "Gift_Cooldown",
            meta: "Gauge",
            note: "Gift cooldown meter with instant status updates.",
            metrics: [
              { label: "Level", value: "12" },
              { label: "Timer", value: "Ready" },
              { label: "State", value: "Warm" },
            ],
          },
          {
            name: "Ring_Bond",
            meta: "Gauge",
            note: "Another row to make the tab feel like a true management panel.",
            metrics: [
              { label: "Level", value: "44" },
              { label: "Timer", value: "Ready" },
              { label: "State", value: "Warm" },
            ],
          },
        ],
        actions: ["Trace", "Gift", "Inspect"],
      },
      {
        key: "history",
        label: "History",
        rows: [
          {
            name: "First_Meeting",
            meta: "Log",
            note: "History rows give the panel enough weight to feel present.",
            metrics: [
              { label: "Date", value: "Day 1" },
              { label: "Map", value: "Bichon" },
              { label: "State", value: "Saved" },
            ],
          },
          {
            name: "Last_Gift",
            meta: "Log",
            note: "Another static row makes selection changes easy to spot.",
            metrics: [
              { label: "Date", value: "Today" },
              { label: "Map", value: "Market" },
              { label: "State", value: "Saved" },
            ],
          },
          {
            name: "Travel_Log",
            meta: "Log",
            note: "Good for screenshoting a different selected row state.",
            metrics: [
              { label: "Date", value: "This week" },
              { label: "Map", value: "Arena" },
              { label: "State", value: "Saved" },
            ],
          },
        ],
        actions: ["Record", "Bond", "Inspect"],
      },
    ],
  },
  group: {
    subtitle: "Party roster and recruitment for {player}",
    footer: "Invite, assist, or inspect the group state.",
    tabs: [
      {
        key: "party",
        label: "Party",
        rows: [
          {
            name: "{player}",
            meta: "Leader",
            note: "The local player stays visible as a normal selected row.",
            metrics: [
              { label: "Role", value: "Leader" },
              { label: "Range", value: "Near" },
              { label: "State", value: "Ready" },
            ],
          },
          {
            name: "Field_Cleric",
            meta: "Support",
            note: "Support row with clear role visibility and status.",
            metrics: [
              { label: "Role", value: "Heal" },
              { label: "Range", value: "Mid" },
              { label: "State", value: "Ready" },
            ],
          },
          {
            name: "Frontline",
            meta: "Tank",
            note: "Front-line rows help the panel read like a real party list.",
            metrics: [
              { label: "Role", value: "Tank" },
              { label: "Range", value: "Front" },
              { label: "State", value: "Ready" },
            ],
          },
        ],
        actions: ["Invite", "Assist", "Share"],
      },
      {
        key: "recruit",
        label: "Recruit",
        rows: [
          {
            name: "Need_Tank",
            meta: "Recruit",
            note: "Recruit rows show available party slots.",
            metrics: [
              { label: "Role", value: "Tank" },
              { label: "Slots", value: "1" },
              { label: "State", value: "Open" },
            ],
          },
          {
            name: "Need_DPS",
            meta: "Recruit",
            note: "Another row keeps click selection obvious.",
            metrics: [
              { label: "Role", value: "DPS" },
              { label: "Slots", value: "2" },
              { label: "State", value: "Open" },
            ],
          },
          {
            name: "Need_Support",
            meta: "Recruit",
            note: "Recruiting state for the party list.",
            metrics: [
              { label: "Role", value: "Support" },
              { label: "Slots", value: "1" },
              { label: "State", value: "Open" },
            ],
          },
        ],
        actions: ["Invite", "Inspect", "Chat"],
      },
      {
        key: "loot",
        label: "Loot",
        rows: [
          {
            name: "Shard_Split",
            meta: "Share",
            note: "Group loot is ready to share.",
            metrics: [
              { label: "Split", value: "Equal" },
              { label: "Need", value: "Open" },
              { label: "State", value: "Ready" },
            ],
          },
          {
            name: "Gold_Share",
            meta: "Share",
            note: "A different row makes the click target unmistakable.",
            metrics: [
              { label: "Split", value: "Gold" },
              { label: "Need", value: "Open" },
              { label: "State", value: "Ready" },
            ],
          },
          {
            name: "Quest_Drop",
            meta: "Share",
            note: "Quest split row for quick loot visibility and handoff.",
            metrics: [
              { label: "Split", value: "Quest" },
              { label: "Need", value: "Open" },
              { label: "State", value: "Ready" },
            ],
          },
        ],
        actions: ["Share", "Inspect", "Chat"],
      },
    ],
  },
  guild: {
    subtitle: "Guild hall and member queue for {player}",
    footer: "Notice, roster, and member management are available.",
    tabs: [
      {
        key: "overview",
        label: "Overview",
        rows: [
          {
            name: "Obelisk",
            meta: "Guild hall",
            note: "Main guild summary with a real selectable row.",
            metrics: [
              { label: "Members", value: "42" },
              { label: "Rank", value: "A" },
              { label: "State", value: "Open" },
            ],
          },
          {
            name: "Banner_Room",
            meta: "Guild hall",
            note: "A second overview row for clearer panel screenshots.",
            metrics: [
              { label: "Members", value: "42" },
              { label: "Rank", value: "A" },
              { label: "State", value: "Open" },
            ],
          },
          {
            name: "Crystal_Notice",
            meta: "Guild hall",
            note: "Guild notice row for current hall updates.",
            metrics: [
              { label: "Members", value: "42" },
              { label: "Rank", value: "A" },
              { label: "State", value: "Open" },
            ],
          },
        ],
        actions: ["Notice", "Inspect", "Chat"],
      },
      {
        key: "members",
        label: "Members",
        rows: [
          {
            name: "Guild_Master",
            meta: "Leader",
            note: "Leader entries show current roster visibility and state.",
            metrics: [
              { label: "Role", value: "Leader" },
              { label: "Duty", value: "Manage" },
              { label: "State", value: "Online" },
            ],
          },
          {
            name: "Deputy",
            meta: "Officer",
            note: "Officer row for member management.",
            metrics: [
              { label: "Role", value: "Officer" },
              { label: "Duty", value: "Manage" },
              { label: "State", value: "Online" },
            ],
          },
          {
            name: "Recruit",
            meta: "Member",
            note: "Member row for roster review.",
            metrics: [
              { label: "Role", value: "Member" },
              { label: "Duty", value: "Train" },
              { label: "State", value: "Online" },
            ],
          },
        ],
        actions: ["Promote", "Inspect", "Chat"],
      },
      {
        key: "notice",
        label: "Notice",
        rows: [
          {
            name: "Raid_Night",
            meta: "Pinned",
            note: "Guild notice row for current hall updates.",
            metrics: [
              { label: "Time", value: "20:00" },
              { label: "Type", value: "Raid" },
              { label: "State", value: "Pinned" },
            ],
          },
          {
            name: "Bank_Lock",
            meta: "Pinned",
            note: "Lock rows are static but still clickable and selectable.",
            metrics: [
              { label: "Time", value: "Now" },
              { label: "Type", value: "Bank" },
              { label: "State", value: "Pinned" },
            ],
          },
          {
            name: "Contribution",
            meta: "Pinned",
            note: "Useful for demonstrating row state and action updates.",
            metrics: [
              { label: "Time", value: "Today" },
              { label: "Type", value: "Donate" },
              { label: "State", value: "Pinned" },
            ],
          },
        ],
        actions: ["Notice", "Inspect", "Chat"],
      },
    ],
  },
};

function resolveSystemMenuShellText(value: string, playerName: string | null) {
  return value.replace(/\{player\}/g, playerName ?? "-");
}

const EMPTY_VIEWPORT_MAP_SPRITES: ViewportMapSprites = {
  floor: [],
  objects: [],
};

const EMPTY_VIEWPORT_OFFSET: ViewportOffset = {
  x: 0,
  y: 0,
};

const VIEWPORT_ROW_Z_STRIDE = 128;
const VIEWPORT_BASE_Z = 4096;
const MINI_MAP_VIEW_WIDTH = 120;
const MINI_MAP_VIEW_HEIGHT = 108;

const CHAT_FILTER_BUTTONS: Array<{ key: ChatFilterKey; left: number; labelKey: string }> = [
  { key: "all", left: 12, labelKey: "client.Chat_All" },
  { key: "shout", left: 34, labelKey: "ui.shout" },
  { key: "whisper", left: 56, labelKey: "client.Chat_Whisper" },
  { key: "lover", left: 78, labelKey: "client.Chat_Lover" },
  { key: "mentor", left: 100, labelKey: "client.Chat_Mentor" },
  { key: "group", left: 122, labelKey: "client.Chat_Group" },
  { key: "guild", left: 144, labelKey: "client.Chat_Guild" },
];

function GameSceneBackdrop({
  world,
  player,
  floorSprites,
  cameraOffset,
}: {
  world: DisplayWorld;
  player: DisplayEntity | null;
  floorSprites: ViewportMapSprite[];
  cameraOffset: ViewportOffset;
}) {
  if (floorSprites.length) {
    return (
      <div className="game-scene-backdrop">
        {floorSprites.map((sprite) => (
          <img
            key={sprite.key}
            className="scene-backdrop-sprite"
            data-map-sprite-key={sprite.key}
            src={sprite.path}
            alt=""
            draggable={false}
            style={{
              left: sprite.left + cameraOffset.x,
              top: sprite.top + cameraOffset.y,
              width: sprite.width,
              height: sprite.height,
            }}
          />
        ))}
      </div>
    );
  }

  if (!world.originalMapRegion) {
    return null;
  }

  const tiles = buildSceneBackdropTiles(world, player);

  if (!tiles.length) {
    return null;
  }

  return (
    <div className="game-scene-backdrop">
      {tiles.map((tile) => (
        <div
          key={tile.key}
          className="scene-backdrop-tile"
          data-map-sprite-key={tile.key}
          style={{
            left: tile.left + cameraOffset.x,
            top: tile.top + cameraOffset.y,
            backgroundImage: `linear-gradient(${tile.tint}, ${tile.tint}), url("${tile.texture}")`,
          }}
        />
      ))}
    </div>
  );
}

function MailPanel({
  t,
  mail,
  onClaim,
  onDelete,
  onClose,
}: {
  t: TranslateFn;
  mail: DisplayMailMessage[];
  onClaim: (mailId: number) => void;
  onDelete: (mailId: number) => void;
  onClose: () => void;
}) {
  const entries = mail.filter((message) => !message.deleted);
  const visibleEntries = entries.slice(0, 10);
  const selectedEntry = visibleEntries.find((entry) => entry.id !== undefined) ?? visibleEntries[0] ?? null;
  const pageCount = Math.max(1, Math.ceil(entries.length / 10));

  return (
    <section className="mail-panel">
      <img className="mail-frame" src={ORIGINAL_UI.mail.frame} alt="" draggable={false} />
      <img className="mail-title-image" src={ORIGINAL_UI.mail.title} alt="" draggable={false} />
      <div className="mail-close">
        <SpriteButton sprite={ORIGINAL_UI.mail.closeButton} label={t("ui.close")} onClick={onClose} />
      </div>
      <div className="mail-help">
        <SpriteButton sprite={ORIGINAL_UI.mail.helpButton} label={t("ui.help", [], "Help")} onClick={() => undefined} />
      </div>
      <div className="mail-header type">{t("client.Type", [], "Type")}</div>
      <div className="mail-header sender">{t("client.Sender", [], "Sender")}</div>
      <div className="mail-header message">{t("client.Message", [], "Message")}</div>
      {visibleEntries.map((entry, index) => (
        <MailListRow
          key={`mail-row-${entry.id ?? index}`}
          entry={entry}
          index={index}
          selected={index === 0}
          onClaim={onClaim}
          onDelete={onDelete}
        />
      ))}
      <div className="mail-page-previous">
        <SpriteButton sprite={ORIGINAL_UI.mail.previousButton} label={t("ui.previous", [], "Previous")} onClick={() => undefined} />
      </div>
      <div className="overlay-panel-foot mail-page-label">{`1 / ${pageCount}`}</div>
      <div className="mail-page-next">
        <SpriteButton sprite={ORIGINAL_UI.mail.nextButton} label={t("ui.next", [], "Next")} onClick={() => undefined} />
      </div>
      <div className="mail-action send"><SpriteButton sprite={ORIGINAL_UI.mail.sendButton} label={t("client.Send", [], "Send")} onClick={() => undefined} /></div>
      <div className="mail-action reply"><SpriteButton sprite={ORIGINAL_UI.mail.replyButton} label={t("client.Reply", [], "Reply")} onClick={() => undefined} /></div>
      <div className="mail-action read">
        <SpriteButton
          sprite={ORIGINAL_UI.mail.readButton}
          label={t("client.Read", [], "Read")}
          onClick={() => selectedEntry?.id !== undefined && !selectedEntry.claimed ? onClaim(selectedEntry.id) : undefined}
        />
      </div>
      <div className="mail-action delete">
        <SpriteButton
          sprite={ORIGINAL_UI.mail.deleteButton}
          label={t("client.Delete", [], "Delete")}
          onClick={() => selectedEntry?.id !== undefined ? onDelete(selectedEntry.id) : undefined}
        />
      </div>
      <div className="mail-action block disabled"><SpriteButton sprite={ORIGINAL_UI.mail.blockListButton} label={t("client.BlockList", [], "Block List")} onClick={() => undefined} /></div>
      <div className="mail-action bug disabled"><SpriteButton sprite={ORIGINAL_UI.mail.bugReportButton} label={t("client.ReportBug", [], "Report Bug")} onClick={() => undefined} /></div>
      <div className="overlay-panel-list mail-legacy-list" hidden>
        {entries.length ? (
          entries.map((entry, index) => (
            <div key={`mail-${entry.id ?? index}`} className="overlay-panel-row">
              <strong>{entry.subject ?? t("client.Mail", [], "Mail")}</strong>
              <span>{`${entry.from ?? "System"} -> ${entry.to ?? "You"}`}</span>
              <span>{entry.body ?? ""}</span>
              <span>
                {[
                  entry.gold ? `${entry.gold} Gold` : null,
                  entry.items?.length ? `${entry.items.join(", ")}` : null,
                  entry.claimed ? "Claimed" : "Unclaimed",
                ]
                  .filter(Boolean)
                  .join(" · ")}
              </span>
              <div className="overlay-panel-actions">
                <button
                  type="button"
                  disabled={entry.claimed || entry.id === undefined}
                  onClick={() => entry.id !== undefined && onClaim(entry.id)}
                >
                  Claim
                </button>
                <button
                  type="button"
                  disabled={entry.id === undefined}
                  onClick={() => entry.id !== undefined && onDelete(entry.id)}
                >
                  Delete
                </button>
              </div>
            </div>
          ))
        ) : (
          <div className="overlay-panel-empty">No mail</div>
        )}
      </div>
      <div className="overlay-panel-foot mail-legacy-foot">{`${entries.length}/${mail.length}`}</div>
    </section>
  );
}

function MailListRow({
  entry,
  index,
  selected,
  onClaim,
  onDelete,
}: {
  entry: DisplayMailMessage;
  index: number;
  selected: boolean;
  onClaim: (mailId: number) => void;
  onDelete: (mailId: number) => void;
}) {
  const hasParcel = !entry.claimed && (Boolean(entry.gold) || Boolean(entry.items?.length));
  const icon = entry.gold && !entry.items?.length ? ORIGINAL_UI.mail.icons.gold : ORIGINAL_UI.mail.icons.letter;
  const sender = entry.from ?? "System";
  const message = (entry.body || entry.subject || "").replace(/\s+/g, " ");

  return (
    <div
      role="button"
      tabIndex={0}
      className="overlay-panel-row mail-row"
      style={{ top: 55 + index * 33 }}
      onDoubleClick={() => entry.id !== undefined && !entry.claimed && onClaim(entry.id)}
      onKeyDown={(event) => {
        if ((event.key === "Enter" || event.key === " ") && entry.id !== undefined && !entry.claimed) {
          event.preventDefault();
          onClaim(entry.id);
        }
      }}
    >
      {selected ? <img className="mail-row-selected" src={ORIGINAL_UI.mail.icons.selected} alt="" draggable={false} /> : null}
      <span className="mail-row-icon-area">
        <img className="mail-row-icon" src={icon} alt="" draggable={false} />
        {!entry.claimed ? <img className={`mail-row-flag unread ${hasParcel ? "second" : ""}`} src={ORIGINAL_UI.mail.icons.unread} alt="" draggable={false} /> : null}
        {hasParcel ? <img className="mail-row-flag parcel" src={ORIGINAL_UI.mail.icons.parcel} alt="" draggable={false} /> : null}
      </span>
      <span className="mail-row-sender">{sender}</span>
      <span className="mail-row-message">{message}</span>
      <span className="overlay-panel-actions mail-row-actions">
        <button
          type="button"
          disabled={entry.claimed || entry.id === undefined}
          onClick={(event) => {
            event.stopPropagation();
            if (entry.id !== undefined) onClaim(entry.id);
          }}
        >
          Claim
        </button>
        <button
          type="button"
          disabled={entry.id === undefined}
          onClick={(event) => {
            event.stopPropagation();
            if (entry.id !== undefined) onDelete(entry.id);
          }}
        >
          Delete
        </button>
      </span>
    </div>
  );
}

function BigMapDialog({
  t,
  world,
  player,
  onClose,
}: {
  t: TranslateFn;
  world: DisplayWorld;
  player: DisplayEntity | null;
  onClose: () => void;
}) {
  const [showWorldMap, setShowWorldMap] = useState(false);
  const bigMapAsset = originalBigMapAssetPath(world.bigMapIndex ?? world.miniMapIndex);
  const mapWidth = Math.max(world.originalMapRegion?.mapWidth ?? player?.x ?? 1, 1);
  const mapHeight = Math.max(world.originalMapRegion?.mapHeight ?? player?.y ?? 1, 1);
  const viewport = bigMapViewport(bigMapAsset);
  const scaleX = viewport.contentWidth / mapWidth;
  const scaleY = viewport.contentHeight / mapHeight;
  const coordinateLabel = player ? `[ ${player.x}, ${player.y} ]` : "[ 0, 0 ]";
  const npcRows = bigMapNpcRowsForWorld(world).slice(0, 18);

  return (
    <section className="big-map-dialog" aria-label={t("client.BigMapKey", ["M"], t("ui.map"))}>
      <img className="big-map-frame" src={ORIGINAL_UI.bigMap.frame} alt="" draggable={false} />
      <div className="big-map-title">{world.mapTitle ?? world.mapFileName ?? ""}</div>
      <div className="big-map-close"><SpriteButton sprite={ORIGINAL_UI.bigMap.closeButton} label={t("ui.close")} onClick={onClose} /></div>
      <div className="big-map-scroll up"><SpriteButton sprite={ORIGINAL_UI.bigMap.upButton} label={t("ui.up", [], "Up")} onClick={() => undefined} /></div>
      <div className="big-map-scroll thumb"><SpriteButton sprite={ORIGINAL_UI.bigMap.positionBar} label={t("ui.scroll", [], "Scroll")} onClick={() => undefined} /></div>
      <div className="big-map-scroll down"><SpriteButton sprite={ORIGINAL_UI.bigMap.downButton} label={t("ui.down", [], "Down")} onClick={() => undefined} /></div>
      <div className="big-map-viewport" style={{ left: viewport.left, top: viewport.top, width: viewport.width, height: viewport.height }}>
        {bigMapAsset ? (
          <img
            className="big-map-raster"
            src={bigMapAsset.src}
            alt=""
            draggable={false}
            style={{ width: viewport.contentWidth, height: viewport.contentHeight, left: viewport.imageLeft, top: viewport.imageTop }}
          />
        ) : (
          <div className="big-map-fallback" />
        )}
        {world.entities.map((entity) => {
          const left = viewport.imageLeft + entity.x * scaleX - 1;
          const top = viewport.imageTop + entity.y * scaleY - 1;
          return <span key={`big-map-dot-${entity.objectId}`} className={`big-map-dot ${entity.kind}`} style={{ left, top }} />;
        })}
        {player ? (
          <img
            className="big-map-user-dot"
            src={ORIGINAL_UI.bigMap.radarDot}
            alt=""
            draggable={false}
            style={{ left: viewport.imageLeft + player.x * scaleX - 6, top: viewport.imageTop + player.y * scaleY - 5 }}
          />
        ) : null}
      </div>
      <div className="big-map-coordinate">{coordinateLabel}</div>
      <div className="big-map-npc-list">
        {npcRows.map((entity, index) => (
          <button
            key={`big-map-npc-${entity.key}`}
            type="button"
            className="big-map-npc-row"
            style={{ top: index * 21 }}
          >
            <img
              className="big-map-npc-icon"
              src={originalMapLinkIconPath(entity.icon)}
              alt=""
              draggable={false}
            />
            <span className="big-map-npc-name">{bigMapNpcDisplayName(entity.name)}</span>
          </button>
        ))}
      </div>
      <div className="big-map-world-button"><SpriteButton sprite={ORIGINAL_UI.bigMap.worldButton} label={t("ui.world", [], "World")} onClick={() => setShowWorldMap(true)} /></div>
      <div className="big-map-my-location-button"><SpriteButton sprite={ORIGINAL_UI.bigMap.myLocationButton} label={t("ui.myLocation", [], "My Location")} onClick={() => setShowWorldMap(false)} /></div>
      <div className="big-map-teleport-button disabled"><SpriteButton sprite={ORIGINAL_UI.bigMap.teleportButton} label={t("ui.teleport", [], "Teleport")} onClick={() => undefined} active /></div>
      <div className="big-map-search-button"><SpriteButton sprite={ORIGINAL_UI.bigMap.searchButton} label={t("ui.search", [], "Search")} onClick={() => undefined} /></div>
      <input className="big-map-search-input" aria-label={t("ui.search", [], "Search")} readOnly />
      {showWorldMap ? (
        <div className="big-map-world-overlay">
          <img className="big-map-world-image" src={ORIGINAL_UI.bigMap.worldMap} alt="" draggable={false} />
          <img className="big-map-world-clouds" src={ORIGINAL_UI.bigMap.worldClouds} alt="" draggable={false} />
          <img className="big-map-world-border" src={ORIGINAL_UI.bigMap.worldBorder} alt="" draggable={false} />
        </div>
      ) : null}
    </section>
  );
}

function ReportPanel({
  t,
  logs,
  onClose,
}: {
  t: TranslateFn;
  logs: DisplayLogLine[];
  onClose: () => void;
}) {
  const lines = logs.filter((line) => line.tone !== "network").slice(0, 6);

  return (
    <section className="overlay-panel report-panel">
      <div className="overlay-panel-head">
        <strong>{t("ui.report")}</strong>
        <button type="button" onClick={onClose}>
          {t("ui.close")}
        </button>
      </div>
      <div className="overlay-panel-list">
        {lines.map((line, index) => (
          <div key={`report-${index}`} className="overlay-panel-row">
            {trimLogTimestamp(line.text)}
          </div>
        ))}
      </div>
      <div className="overlay-panel-foot">{`${lines.length}/6`}</div>
    </section>
  );
}

function SystemMenuPanel({
  t,
  playerName,
  playerPosition,
  mapTitle,
  mapFileName,
  inSafeZone,
  transferOptions,
  onOpenPanel,
  onClose,
  onLogout,
  onTransferMap,
}: {
  t: TranslateFn;
  playerName: string | null;
  playerPosition: { x: number; y: number } | null;
  mapTitle: string | null;
  mapFileName: string | null;
  inSafeZone: boolean;
  transferOptions: SystemMenuTransferOption[];
  onOpenPanel: (panel: SystemMenuSurfacePanel) => void;
  onClose: () => void;
  onLogout: () => void;
  onTransferMap: (transferKey: string) => void;
}) {
  const [qaMap, setQaMap] = useState(() => normalizeMapInput(mapFileName ?? "0"));
  const [qaX, setQaX] = useState(() => String(playerPosition?.x ?? 330));
  const [qaY, setQaY] = useState(() => String(playerPosition?.y ?? 270));
  const noop = () => undefined;
  const menuButtons: SystemMenuButtonDefinition[] = [
    { key: "exit", label: t("ui.exit"), onClick: onLogout },
    { key: "logout", label: t("ui.logout", [], "Log Out"), onClick: onLogout },
    { key: "help", label: t("ui.help", [], "Help"), onClick: noop },
    { key: "keyboard", label: t("ui.keyboard", [], "Keyboard"), onClick: noop },
    { key: "ranking", label: t("ui.ranking", [], "Ranking"), panel: "ranking" as const },
    { key: "creature", label: t("ui.creature", [], "Creature"), panel: "creature" as const },
    { key: "ride", label: t("ui.mount", [], "Mount"), panel: "mount" as const },
    { key: "fishing", label: t("ui.fishing", [], "Fishing"), panel: "fishing" as const },
    { key: "friend", label: t("ui.friends", [], "Friends"), panel: "friend" as const },
    { key: "mentor", label: t("ui.mentor", [], "Mentor"), panel: "mentor" as const },
    { key: "relationship", label: t("ui.relationship", [], "Relationship"), panel: "relationship" as const },
    { key: "group", label: t("ui.group", [], "Group"), panel: "group" as const },
    { key: "guild", label: t("ui.guild", [], "Guild"), panel: "guild" as const },
  ];
  const lateSystemButtons: Array<{ panel: SystemMenuSocialPanel; label: string }> = [
    { panel: "hero", label: t("ui.hero", [], "Hero") },
    { panel: "trade", label: t("ui.trade", [], "Trade") },
    { panel: "market", label: t("ui.market", [], "Market") },
    { panel: "marriage", label: t("ui.marriage", [], "Marriage") },
    { panel: "itemRental", label: t("ui.itemRental", [], "Item Rental") },
  ];

  function submitQaTransfer(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();

    const map = normalizeMapInput(qaMap);
    const x = parseFiniteInteger(qaX);
    const y = parseFiniteInteger(qaY);
    if (!map || x === null || y === null) {
      return;
    }

    onTransferMap(`crystal:${map}:${x}:${y}`);
  }

  return (
    <>
      <section className="system-menu-panel" aria-label={t("ui.menu")}>
        <img className="system-menu-frame" src={ORIGINAL_UI.menu.frame} alt="" draggable={false} />
        <section className="system-menu-actions" aria-label={t("ui.menu") + " actions"}>
          {menuButtons.map((button) => {
            const definition = ORIGINAL_UI.menu.buttons[button.key];
            const panel = button.panel;
            const handleClick = panel ? () => onOpenPanel(panel) : button.onClick ?? noop;

            return (
              <div
                key={button.key}
                className="system-menu-icon"
                data-system-menu-action={button.key}
                style={{ left: `${definition.x}px`, top: `${definition.y}px` }}
              >
                <SpriteButton sprite={definition.sprite} label={button.label} onClick={handleClick} />
              </div>
            );
          })}
        </section>
        <button type="button" className="system-menu-close-hit" onClick={onClose} aria-label={t("ui.close")} />
      </section>
      <section className="system-menu-qa-panel" aria-label={t("ui.transfer", [], "Transfer controls")}>
        <div className="system-menu-meta">
          <span>{playerName ?? "-"}</span>
          <span>{mapTitle ?? t("ui.map")}{mapFileName ? ` [${mapFileName}]` : ""}</span>
          <span>{inSafeZone ? t("ui.safeZone", [], "Safe Zone") : t("ui.combatZone", [], "Combat Zone")}</span>
        </div>
        <div className="system-menu-transfer-list">
          <div className="system-menu-transfer-title">{t("ui.transfer", [], "Transfer")}</div>
          {transferOptions.map((option) => (
            <button key={option.key} type="button" onClick={() => onTransferMap(option.key)}>
              {option.label}
            </button>
          ))}
        </div>
        <form className="system-menu-qa-transfer" onSubmit={submitQaTransfer}>
          <div className="system-menu-transfer-title">{t("ui.quickJump", [], "Quick Jump")}</div>
          <label>
            <span>{t("ui.map")}</span>
            <input
              value={qaMap}
              onChange={(event) => setQaMap(event.target.value)}
              autoComplete="off"
              spellCheck={false}
            />
          </label>
          <div className="system-menu-qa-transfer-coords">
            <label>
              <span>X</span>
              <input
                type="number"
                value={qaX}
                onChange={(event) => setQaX(event.target.value)}
                inputMode="numeric"
              />
            </label>
            <label>
              <span>Y</span>
              <input
                type="number"
                value={qaY}
                onChange={(event) => setQaY(event.target.value)}
                inputMode="numeric"
              />
            </label>
            <button type="submit">{t("ui.go", [], "Go")}</button>
          </div>
        </form>
        <div className="system-menu-late-actions" aria-label="Late systems">
          <div className="system-menu-transfer-title">{t("ui.lateSystems", [], "Systems")}</div>
          {lateSystemButtons.map((button) => (
            <button
              key={button.panel}
              type="button"
              data-system-menu-action={button.panel}
              onClick={() => onOpenPanel(button.panel)}
            >
              {button.label}
            </button>
          ))}
        </div>
      </section>
    </>
  );
}

function SystemMenuFeaturePanel({
  t,
  feature,
  playerName,
  world,
  onRunStage5Command,
  onSendClientCommand,
  onClose,
}: {
  t: TranslateFn;
  feature: SystemMenuSurfacePanel;
  playerName: string | null;
  world: DisplayWorld;
  onRunStage5Command: (action: string, args?: string[]) => void;
  onSendClientCommand: (command: Record<string, unknown>) => void;
  onClose: () => void;
}) {
  const featureTitle =
    feature === "creature"
      ? t("ui.creature", [], "Creature")
      : feature === "mount"
        ? t("ui.mount", [], "Mount")
        : feature === "fishing"
          ? t("ui.fishing", [], "Fishing")
          : feature === "ranking"
            ? t("ui.ranking", [], "Ranking")
            : feature === "friend"
              ? t("ui.friends", [], "Friends")
              : feature === "mentor"
                ? t("ui.mentor", [], "Mentor")
                : feature === "relationship"
                  ? t("ui.relationship", [], "Relationship")
                  : feature === "group"
                    ? t("ui.group", [], "Group")
                    : feature === "guild"
                      ? t("ui.guild", [], "Guild")
                      : feature === "trade"
                        ? t("ui.trade", [], "Trade")
                        : feature === "market"
                          ? t("ui.market", [], "Market")
                          : t("ui.marriage", [], "Marriage");
  const isSocialPanel = feature !== "creature" && feature !== "mount" && feature !== "fishing";

  return (
    <section
      className={`system-feature-panel system-feature-panel-${feature} ${isSocialPanel ? "system-feature-panel-social" : ""}`}
      aria-label={featureTitle}
      data-system-feature-panel={feature}
    >
      <button type="button" className="system-feature-close" onClick={onClose} aria-label={t("ui.close")} />
      {feature === "creature" ? (
        <CreatureSystemPanel t={t} world={world} onSendClientCommand={onSendClientCommand} />
      ) : feature === "mount" ? (
        <MountSystemPanel t={t} world={world} onSendClientCommand={onSendClientCommand} />
      ) : feature === "fishing" ? (
        <FishingSystemPanel t={t} onSendClientCommand={onSendClientCommand} />
      ) : (
        <SocialSystemPanel
          t={t}
          panel={feature}
          playerName={playerName}
          world={world}
          onRunStage5Command={onRunStage5Command}
          onSendClientCommand={onSendClientCommand}
        />
      )}
    </section>
  );
}

function SocialSystemPanel({
  t,
  panel,
  playerName,
  world,
  onRunStage5Command,
  onSendClientCommand,
}: {
  t: TranslateFn;
  panel: SystemMenuSocialPanel;
  playerName: string | null;
  world: DisplayWorld;
  onRunStage5Command: (action: string, args?: string[]) => void;
  onSendClientCommand: (command: Record<string, unknown>) => void;
}) {
  const definition = systemMenuSocialPanelDefinition(panel, playerName, world);
  const [activeTabIndex, setActiveTabIndex] = useState(0);
  const [selectedRowIndex, setSelectedRowIndex] = useState(0);
  const [statusLine, setStatusLine] = useState(() => definition.footer);

  const activeTab = definition.tabs[activeTabIndex] ?? definition.tabs[0];
  const selectedRow = activeTab.rows[selectedRowIndex] ?? activeTab.rows[0];

  useEffect(() => {
    setActiveTabIndex(0);
    setSelectedRowIndex(0);
    setStatusLine(definition.footer);
  }, [definition.footer, panel]);

  if (!activeTab || !selectedRow) {
    return null;
  }

  const resolvedSubtitle = resolveSystemMenuShellText(definition.subtitle, playerName);
  const resolvedTabLabel = resolveSystemMenuShellText(activeTab.label, playerName);
  const resolvedSelectedRowName = resolveSystemMenuShellText(selectedRow.name, playerName);
  const resolvedSelectedRowMeta = resolveSystemMenuShellText(selectedRow.meta, playerName);
  const resolvedSelectedRowNote = resolveSystemMenuShellText(selectedRow.note, playerName);

  return (
    <div
      className="system-social-panel"
      data-system-social-panel={panel}
      data-system-social-tab={activeTab.key}
      data-system-social-selected-row={resolvedSelectedRowName}
      data-system-social-status={statusLine}
    >
      <div className="system-social-subtitle">{resolvedSubtitle}</div>
      <div className="system-social-tabs" role="tablist" aria-label={featureTitleForSocialPanel(t, panel)}>
        {definition.tabs.map((tab, index) => {
          const resolvedLabel = resolveSystemMenuShellText(tab.label, playerName);
          return (
            <button
              key={tab.key}
              type="button"
              className={index === activeTabIndex ? "active" : ""}
              data-social-tab-key={tab.key}
              role="tab"
              aria-selected={index === activeTabIndex}
              onClick={() => {
                setActiveTabIndex(index);
                setSelectedRowIndex(0);
                setStatusLine(`${resolvedLabel} opened`);
              }}
            >
              {resolvedLabel}
            </button>
          );
        })}
      </div>
      <div className="system-social-body">
        <div className="system-social-list" aria-label={`${resolvedTabLabel} rows`}>
          {activeTab.rows.map((row, index) => {
            const resolvedRowName = resolveSystemMenuShellText(row.name, playerName);
            const resolvedRowMeta = resolveSystemMenuShellText(row.meta, playerName);
            return (
              <button
                key={`${panel}-${activeTab.key}-${row.name}`}
                type="button"
                className={`system-social-entry ${index === selectedRowIndex ? "selected" : ""}`}
                data-social-entry-name={resolvedRowName}
                aria-pressed={index === selectedRowIndex}
                onClick={() => {
                  setSelectedRowIndex(index);
                  setStatusLine(`${resolvedRowName} selected`);
                }}
              >
                <strong>{resolvedRowName}</strong>
                <span>{resolvedRowMeta}</span>
              </button>
            );
          })}
        </div>
        <div className="system-social-detail">
          <div className="system-social-detail-name">{resolvedSelectedRowName}</div>
          <div className="system-social-detail-meta">{resolvedSelectedRowMeta}</div>
          <div className="system-social-detail-note">{resolvedSelectedRowNote}</div>
          <div className="system-social-detail-metrics">
            {selectedRow.metrics.map((metric) => (
              <div key={`${panel}-${activeTab.key}-${selectedRow.name}-${metric.label}`} className="system-social-metric">
                <span className="label">{metric.label}</span>
                <span className="value">{resolveSystemMenuShellText(metric.value, playerName)}</span>
              </div>
            ))}
          </div>
        </div>
      </div>
      <div className="system-social-actions">
        {activeTab.actions.map((action) => {
          const resolvedAction = resolveSystemMenuShellText(action, playerName);
          return (
            <button
              key={`${panel}-${activeTab.key}-${action}`}
              type="button"
              data-social-action-label={resolvedAction}
              onClick={() => {
                const clientCommand = clientCommandForSocialAction(panel, activeTab.key, resolvedAction, resolvedSelectedRowName);
                if (clientCommand) {
                  onSendClientCommand(clientCommand);
                  setStatusLine(`${resolvedAction} -> ${resolvedSelectedRowName}`);
                  return;
                }
                const command = stage5CommandForSocialAction(panel, activeTab.key, resolvedAction, resolvedSelectedRowName);
                if (command) {
                  onRunStage5Command(command.action, command.args);
                  setStatusLine(`${resolvedAction} -> ${resolvedSelectedRowName}`);
                } else {
                  setStatusLine(`${resolvedAction} -> ${resolvedSelectedRowName}`);
                }
              }}
            >
              {resolvedAction}
            </button>
          );
        })}
      </div>
      <div className="system-social-footer">
        <span>{definition.footer}</span>
        <span>{statusLine}</span>
      </div>
      <div className="system-social-shell-tick" aria-hidden="true">
        {`${resolvedSelectedRowName} • ${resolvedSelectedRowMeta}`}
      </div>
    </div>
  );
}

function featureTitleForSocialPanel(t: TranslateFn, panel: SystemMenuSocialPanel) {
  switch (panel) {
    case "ranking":
      return t("ui.ranking", [], "Ranking");
    case "friend":
      return t("ui.friends", [], "Friends");
    case "mentor":
      return t("ui.mentor", [], "Mentor");
    case "relationship":
      return t("ui.relationship", [], "Relationship");
    case "group":
      return t("ui.group", [], "Group");
    case "guild":
      return t("ui.guild", [], "Guild");
    case "trade":
      return t("ui.trade", [], "Trade");
    case "market":
      return t("ui.market", [], "Market");
    case "marriage":
      return t("ui.marriage", [], "Marriage");
    case "hero":
      return t("ui.hero", [], "Hero");
    case "itemRental":
      return t("ui.itemRental", [], "Item Rental");
  }
}

function systemMenuSocialPanelDefinition(
  panel: SystemMenuSocialPanel,
  playerName: string | null,
  world: DisplayWorld,
): SystemMenuSocialPanelDefinition {
  const systems = world.stage5Systems ?? {};
  const player = playerName ?? "{player}";
  const selfEntity = world.entities.find((entity) => entity.kind === "selfPlayer");
  const emptyRow = (name: string, meta = "Empty") =>
    systemMenuRow(name, meta, "No live data is currently available for this slot.", [
      { label: "State", value: "None" },
      { label: "Player", value: player },
      { label: "Map", value: world.mapTitle ?? "-" },
    ]);

  switch (panel) {
    case "ranking":
      return {
        subtitle: "Current leaderboard context for {player}",
        footer: "Rows are derived from the current session instead of fixed sample names.",
        tabs: [
          {
            key: "overall",
            label: "Overall",
            rows: [
              systemMenuRow(player, "Current character", "Live character state from the active world snapshot.", [
                { label: "Level", value: String(selfEntity?.level ?? 1) },
                { label: "Gold", value: String(world.gold) },
                { label: "Map", value: world.mapTitle ?? "-" },
              ]),
              systemMenuRow("Visible monsters", "Current map", "Local combat density around the player.", [
                { label: "Count", value: String(world.entities.filter((entity) => entity.kind === "monster").length) },
                { label: "Safe", value: world.inSafeZone ? "Yes" : "No" },
                { label: "Buffs", value: String(world.activeBuffs.length) },
              ]),
            ],
            actions: ["Inspect", "Refresh"],
          },
        ],
      };
    case "friend": {
      const friends = systems.social?.friends ?? [];
      const blocked = systems.social?.blocked ?? [];
      return {
        subtitle: "Friends, block list, and memos for {player}",
        footer: "Friend list is loaded from the active runtime snapshot.",
        tabs: [
          {
            key: "friends",
            label: "Friends",
            rows: friends.length
              ? friends.map((name) =>
                  systemMenuRow(name, "Friend", "Friend entry synced from runtime social state.", socialMetrics("Online", "Friend")),
                )
              : [emptyRow("No friends")],
            actions: ["Add", "Memo", "Refresh"],
          },
          {
            key: "blocks",
            label: "Block List",
            rows: blocked.length
              ? blocked.map((name) =>
                  systemMenuRow(name, "Blocked", "Blocked entry synced from runtime social state.", socialMetrics("Muted", "Blocked")),
                )
              : [emptyRow("No blocked entries")],
            actions: ["Block", "Unblock", "Refresh"],
          },
        ],
      };
    }
    case "group": {
      const members = systems.group?.members ?? [];
      return {
        subtitle: "Group state for {player}",
        footer: `Loot mode: ${systems.group?.lootMode ?? "none"}`,
        tabs: [
          {
            key: "party",
            label: "Party",
            rows: members.length
              ? members.map((name) =>
                  systemMenuRow(name, "Member", "Active group member from the current session.", [
                    { label: "Loot", value: systems.group?.lootMode ?? "-" },
                    { label: "Count", value: String(members.length) },
                    { label: "Map", value: world.mapTitle ?? "-" },
                  ]),
                )
              : [emptyRow("No party")],
            actions: ["Create", "Loot", "Refresh"],
          },
        ],
      };
    }
    case "guild": {
      const members = systems.guild?.members ?? [];
      const chatLog = systems.guild?.chatLog ?? [];
      return {
        subtitle: `${systems.guild?.name ?? "No guild"} roster for {player}`,
        footer: `Rank: ${systems.guild?.rank ?? "-"}`,
        tabs: [
          {
            key: "members",
            label: "Members",
            rows: members.length
              ? members.map((name) =>
                  systemMenuRow(name, systems.guild?.rank ?? "Member", "Guild roster entry from live session data.", [
                    { label: "Guild", value: systems.guild?.name ?? "-" },
                    { label: "Rank", value: systems.guild?.rank ?? "-" },
                    { label: "Perms", value: String(systems.guild?.permissions?.length ?? 0) },
                  ]),
                )
              : [emptyRow("No guild members")],
            actions: ["Create", "Notice", "Chat"],
          },
          {
            key: "notice",
            label: "Notice",
            rows: chatLog.length
              ? chatLog.slice(-3).map((line) => systemMenuRow(line, "Guild chat", "Latest guild chat line from live traffic.", socialMetrics("Log", "Chat")))
              : [emptyRow("No guild notice")],
            actions: ["Chat", "Refresh"],
          },
        ],
      };
    }
    case "trade": {
      const trade = systems.trade ?? null;
      const partner = stringRecordValue(trade, ["partner", "partnerName", "name"]) ?? "No active trade";
      return {
        subtitle: "Trade exchange state for {player}",
        footer: trade ? "Trade session is open." : "No active trade session.",
        tabs: [
          {
            key: "session",
            label: "Session",
            rows: [
              systemMenuRow(partner, trade ? "Active" : "Idle", "Trade session state for this character.", [
                { label: "Gold", value: stringRecordValue(trade, ["gold", "offeredGold"]) ?? "0" },
                { label: "Accepted", value: stringRecordValue(trade, ["accepted", "confirmed"]) ?? "false" },
                { label: "Partner", value: partner },
              ]),
            ],
            actions: ["Start", "Offer 1g", "Accept", "Cancel"],
          },
        ],
      };
    }
    case "hero": {
      const hero = systems.hero ?? null;
      const heroName = stringRecordValue(hero, ["name"]) ?? "Aide";
      const heroClass = stringRecordValue(hero, ["class"]) ?? "warrior";
      const heroGender = stringRecordValue(hero, ["gender"]) ?? "female";
      const behaviour = stringRecordValue(hero, ["behaviour"]) ?? "0";
      const spawned = stringRecordValue(hero, ["spawned"]) ?? "false";
      return {
        subtitle: "Hero management for {player}",
        footer: hero ? "Hero state is loaded from the current runtime snapshot." : "No active hero is attached.",
        tabs: [
          {
            key: "status",
            label: "Status",
            rows: [
              systemMenuRow(heroName, hero ? "Active hero" : "Create hero", "Current hero slot and behaviour state.", [
                { label: "Class", value: heroClass },
                { label: "Gender", value: heroGender },
                { label: "Behaviour", value: behaviour },
              ]),
              systemMenuRow("Spawn", spawned === "true" ? "Spawned" : "Idle", "Hero spawn state reported by runtime.", [
                { label: "Level", value: stringRecordValue(hero, ["level"]) ?? "1" },
                { label: "Experience", value: stringRecordValue(hero, ["experience"]) ?? "0" },
                { label: "Player", value: player },
              ]),
            ],
            actions: ["Create", "Behaviour Guard", "Change"],
          },
        ],
      };
    }
    case "market": {
      const auctions = systems.auction ?? [];
      return {
        subtitle: "Market listings for {player}",
        footer: `${auctions.length} active listing${auctions.length === 1 ? "" : "s"}`,
        tabs: [
          {
            key: "listings",
            label: "Listings",
            rows: auctions.length
              ? auctions.map((listing, index) =>
                  systemMenuRow(
                    stringRecordValue(listing, ["item", "itemName", "name"]) ?? `Listing ${index + 1}`,
                    stringRecordValue(listing, ["seller", "owner"]) ?? "Market",
                    "Active market listing from live session data.",
                    [
                      { label: "Id", value: stringRecordValue(listing, ["id", "listingId"]) ?? String(index + 1) },
                      { label: "Price", value: stringRecordValue(listing, ["price", "gold"]) ?? "0" },
                      { label: "State", value: stringRecordValue(listing, ["state", "status"]) ?? "Open" },
                    ],
                  ),
                )
              : [emptyRow("No market listings")],
            actions: ["List", "Buy", "Cancel"],
          },
        ],
      };
    }
    case "itemRental": {
      const rental = systems.itemRental ?? {};
      const partner = stringRecordValue(rental, ["partnerName", "partner"]) ?? "Crystal Partner";
      const fee = stringRecordValue(rental, ["fee"]) ?? "100";
      const days = stringRecordValue(rental, ["days"]) ?? "7";
      const goldLocked = stringRecordValue(rental, ["goldLocked"]) ?? "false";
      const itemLocked = stringRecordValue(rental, ["itemLocked"]) ?? "false";
      return {
        subtitle: "Item rental session for {player}",
        footer: `Partner: ${partner}`,
        tabs: [
          {
            key: "session",
            label: "Session",
            rows: [
              systemMenuRow(partner, "Rental partner", "Current rental handshake and lock state.", [
                { label: "Fee", value: fee },
                { label: "Days", value: days },
                { label: "Gold lock", value: goldLocked },
              ]),
              systemMenuRow("Loan item", itemLocked === "true" ? "Locked" : "Open", "Loan item slot state.", [
                { label: "Item lock", value: itemLocked },
                { label: "Records", value: stringRecordValue(rental, ["recordCount"]) ?? "0" },
                { label: "Player", value: player },
              ]),
            ],
            actions: ["Request", "Fee 100", "Period 7", "Cancel", "List"],
          },
        ],
      };
    }
    case "marriage":
    case "relationship": {
      const relationship = systems.relationship ?? {};
      const partnerName = stringRecordValue(relationship, ["partnerName"]) ?? "";
      const allowMarriage = boolRecordValue(relationship, ["allowMarriage"], true);
      const requestFrom = stringRecordValue(relationship, ["pendingRequestFrom", "pendingDivorceFrom"]) ?? "None";
      return {
        subtitle: `${panel === "marriage" ? "Marriage" : "Relationship"} state for {player}`,
        footer: allowMarriage ? "Marriage requests are allowed." : "Marriage requests are blocked.",
        tabs: [
          {
            key: "status",
            label: "Status",
            rows: [
              systemMenuRow(partnerName || player, partnerName ? "Married" : "Single", "Current relationship state.", [
                { label: "Partner", value: partnerName || "-" },
                { label: "Ring", value: "-" },
                { label: "Request", value: requestFrom },
              ]),
            ],
            actions: ["Allow", "Request", "Divorce", "Refresh"],
          },
        ],
      };
    }
    case "mentor": {
      const mentor = systems.mentor ?? {};
      const mentorName = stringRecordValue(mentor, ["name"]) ?? "";
      const allowMentor = boolRecordValue(mentor, ["allowMentor"], true);
      const pending = stringRecordValue(mentor, ["pendingRequestFrom"]) ?? "";
      return {
        subtitle: "Mentor state for {player}",
        footer: allowMentor ? "Mentor requests are allowed." : "Mentor requests are blocked.",
        tabs: [
          {
            key: "requests",
            label: "Requests",
            rows: [
              systemMenuRow(mentorName || pending || "No mentor request", mentorName ? "Mentor" : "Open", "Current mentor state.", [
                { label: "Level", value: stringRecordValue(mentor, ["level"]) ?? "0" },
                { label: "Online", value: stringRecordValue(mentor, ["online"]) ?? "false" },
                { label: "Mentee EXP", value: stringRecordValue(mentor, ["menteeExp"]) ?? "0" },
              ]),
            ],
            actions: ["Allow", "Add", "Cancel", "Refresh"],
          },
        ],
      };
    }
  }

  return SYSTEM_MENU_SOCIAL_PANEL_DEFINITIONS[panel] ?? {
    subtitle: "System state for {player}",
    footer: "No state available.",
    tabs: [{ key: "state", label: "State", rows: [emptyRow("No rows")], actions: ["Refresh"] }],
  };
}

function systemMenuRow(
  name: string,
  meta: string,
  note: string,
  metrics: SystemMenuSocialPanelMetric[],
): SystemMenuSocialPanelRow {
  return { name, meta, note, metrics };
}

function socialMetrics(state: string, note: string): SystemMenuSocialPanelMetric[] {
  return [
    { label: "State", value: state },
    { label: "Note", value: note },
    { label: "Source", value: "Live" },
  ];
}

function stringRecordValue(record: Record<string, unknown> | null | undefined, keys: string[]) {
  if (!record) return null;
  for (const key of keys) {
    const value = record[key];
    if (typeof value === "string" || typeof value === "number" || typeof value === "boolean") {
      return String(value);
    }
  }
  return null;
}

function numberRecordValue(record: Record<string, unknown> | null | undefined, keys: string[], fallback = 0) {
  const value = stringRecordValue(record, keys);
  if (value === null) return fallback;
  const number = Number(value);
  return Number.isFinite(number) ? number : fallback;
}

function boolRecordValue(record: Record<string, unknown> | null | undefined, keys: string[], fallback = false) {
  if (!record) return fallback;
  for (const key of keys) {
    const value = record[key];
    if (typeof value === "boolean") return value;
  }
  return fallback;
}

function recordObjectValue(record: Record<string, unknown> | null | undefined, key: string) {
  const value = record?.[key];
  return value && typeof value === "object" && !Array.isArray(value) ? (value as Record<string, unknown>) : null;
}

function clientCommandForSocialAction(
  panel: SystemMenuSocialPanel,
  tab: string,
  action: string,
  rowName: string,
): Record<string, unknown> | null {
  const normalized = action.toLowerCase();
  if (panel === "friend" && normalized === "refresh") {
    return { type: "refreshFriends" };
  }
  if (panel === "friend" && normalized === "add" && rowName !== "No friends") {
    return { type: "addFriend", name: rowName, blocked: false };
  }
  if (panel === "friend" && normalized === "block" && rowName !== "No blocked entries") {
    return { type: "addFriend", name: rowName, blocked: true };
  }
  if (panel === "trade" && normalized === "start") {
    return { type: "tradeRequest" };
  }
  if (panel === "trade" && normalized === "offer 1g") {
    return { type: "tradeGold", amount: 1 };
  }
  if (panel === "trade" && normalized === "accept") {
    return { type: "tradeConfirm", locked: true };
  }
  if (panel === "trade" && normalized === "cancel") {
    return { type: "tradeCancel" };
  }
  if (panel === "hero" && normalized === "create") {
    return { type: "newHero", name: "Aide", gender: "female", class: "taoist" };
  }
  if (panel === "hero" && normalized === "behaviour guard") {
    return { type: "setHeroBehaviour", behaviour: 2 };
  }
  if (panel === "hero" && normalized === "change") {
    return { type: "changeHero", listIndex: 0 };
  }
  if (panel === "itemRental" && normalized === "request") {
    return { type: "itemRentalRequest" };
  }
  if (panel === "itemRental" && normalized === "fee 100") {
    return { type: "itemRentalFee", amount: 100 };
  }
  if (panel === "itemRental" && normalized === "period 7") {
    return { type: "itemRentalPeriod", days: 7 };
  }
  if (panel === "itemRental" && normalized === "cancel") {
    return { type: "cancelItemRental" };
  }
  if (panel === "itemRental" && normalized === "list") {
    return { type: "getRentedItems" };
  }
  if (panel === "mentor" && normalized === "allow") {
    return { type: "allowMentor" };
  }
  if (panel === "mentor" && normalized === "add") {
    return { type: "addMentor", name: rowName === "No mentor request" ? "Master" : rowName };
  }
  if (panel === "mentor" && normalized === "cancel") {
    return { type: "cancelMentor" };
  }
  if ((panel === "relationship" || panel === "marriage") && normalized === "allow") {
    return { type: "changeMarriage" };
  }
  if ((panel === "relationship" || panel === "marriage") && normalized === "request") {
    return { type: "marriageRequest" };
  }
  if ((panel === "relationship" || panel === "marriage") && normalized === "divorce") {
    return { type: "divorceRequest" };
  }
  if (panel === "market" && normalized === "refresh") {
    return { type: "marketRefresh" };
  }
  void tab;
  return null;
}

function stage5CommandForSocialAction(
  panel: SystemMenuSocialPanel,
  tab: string,
  action: string,
  rowName: string,
): { action: string; args: string[] } | null {
  const normalized = action.toLowerCase();
  if (panel === "friend" && normalized === "add" && rowName !== "No friends") {
    return { action: "social.friend", args: [rowName] };
  }
  if (panel === "friend" && normalized === "block" && rowName !== "No blocked entries") {
    return { action: "social.block", args: [rowName] };
  }
  if (panel === "group" && normalized === "create") {
    return { action: "group.create", args: ["Panel"] };
  }
  if (panel === "group" && normalized === "loot") {
    return { action: "group.loot", args: ["roundRobin"] };
  }
  if (panel === "guild" && normalized === "create") {
    return { action: "guild.create", args: ["PanelGuild"] };
  }
  if (panel === "guild" && normalized === "chat") {
    return { action: "guild.chat", args: ["Guild", "panel"] };
  }
  if (panel === "trade" && normalized === "start") {
    return { action: "trade.start", args: ["Trader"] };
  }
  if (panel === "trade" && normalized === "offer 1g") {
    return { action: "trade.offerGold", args: ["1"] };
  }
  if (panel === "trade" && normalized === "accept") {
    return { action: "trade.accept", args: [] };
  }
  if (panel === "trade" && normalized === "cancel") {
    return { action: "trade.cancel", args: [] };
  }
  if (panel === "market" && normalized === "list") {
    return { action: "auction.list", args: ["panel-listing", "35"] };
  }
  if (panel === "market" && normalized === "buy") {
    return { action: "auction.buy", args: ["1"] };
  }
  if (panel === "market" && normalized === "cancel") {
    return { action: "auction.cancel", args: ["1"] };
  }
  void tab;
  return null;
}

function defaultIntelligentCreaturePayload(source: Record<string, unknown> | null, summoned: boolean) {
  const rules = recordObjectValue(source, "creatureRules") ?? {};
  const filter = recordObjectValue(source, "filter") ?? {};
  return {
    petType: numberRecordValue(source, ["petType"], 1),
    icon: numberRecordValue(source, ["icon"], 44),
    customName: stringRecordValue(source, ["customName", "name"]) ?? "Buddy",
    fullness: numberRecordValue(source, ["fullness"], 50),
    slotIndex: numberRecordValue(source, ["slotIndex"], 0),
    expireBinaryDatetime: numberRecordValue(source, ["expireBinaryDatetime"], 638000000000000000),
    blackstoneTime: numberRecordValue(source, ["blackstoneTime"], 12000),
    petMode: summoned ? 1 : 0,
    creatureRules: {
      minimalFullness: numberRecordValue(rules, ["minimalFullness"], 1),
      mousePickupEnabled: boolRecordValue(rules, ["mousePickupEnabled"], true),
      mousePickupRange: numberRecordValue(rules, ["mousePickupRange"], 6),
      autoPickupEnabled: boolRecordValue(rules, ["autoPickupEnabled"], false),
      autoPickupRange: numberRecordValue(rules, ["autoPickupRange"], 0),
      semiAutoPickupEnabled: boolRecordValue(rules, ["semiAutoPickupEnabled"], true),
      semiAutoPickupRange: numberRecordValue(rules, ["semiAutoPickupRange"], 4),
      canProduceBlackstone: boolRecordValue(rules, ["canProduceBlackstone"], true),
    },
    filter: {
      petPickupAll: boolRecordValue(filter, ["petPickupAll"], false),
      petPickupGold: boolRecordValue(filter, ["petPickupGold"], true),
      petPickupWeapons: boolRecordValue(filter, ["petPickupWeapons"], false),
      petPickupArmours: boolRecordValue(filter, ["petPickupArmours"], false),
      petPickupHelmets: boolRecordValue(filter, ["petPickupHelmets"], false),
      petPickupBoots: boolRecordValue(filter, ["petPickupBoots"], false),
      petPickupBelts: boolRecordValue(filter, ["petPickupBelts"], false),
      petPickupAccessories: boolRecordValue(filter, ["petPickupAccessories"], false),
      petPickupOthers: boolRecordValue(filter, ["petPickupOthers"], true),
    },
    pickupGrade: numberRecordValue(source, ["pickupGrade"], 2),
    maintainFoodTime: numberRecordValue(source, ["maintainFoodTime"], 24000),
  };
}

function CreatureSystemPanel({
  t,
  world,
  onSendClientCommand,
}: {
  t: TranslateFn;
  world: DisplayWorld;
  onSendClientCommand: (command: Record<string, unknown>) => void;
}) {
  const creature = world.stage5Systems?.intelligentCreatures?.[0] ?? null;
  const fullness = clampNumber(numberRecordValue(creature, ["fullness"], 0), 0, 100);
  const blackstone = clampNumber(Math.round(numberRecordValue(creature, ["blackstoneTime"], 0) / 240), 0, 100);
  const creatureName = stringRecordValue(creature, ["customName", "name"]) ?? "-";
  const updateCreature = (summoned: boolean, releaseMe = false) => {
    onSendClientCommand({
      type: "updateIntelligentCreature",
      creature: defaultIntelligentCreaturePayload(creature, summoned),
      summonMe: summoned && !releaseMe,
      unsummonMe: !summoned && !releaseMe,
      releaseMe,
    });
  };
  return (
    <>
      <div className="system-feature-title">{t("ui.creature", [], "Creature")}</div>
      <div className="creature-feature-name">{creatureName}</div>
      <div className="creature-feature-gauge creature-feature-fullness">
        <span className="creature-feature-fill" style={{ width: `${fullness}%` }} />
      </div>
      <div className="creature-feature-gauge creature-feature-minimum">
        <span className="creature-feature-marker" style={{ left: "10%" }} />
      </div>
      <div className="creature-feature-blackstone">
        <span className="creature-feature-fill" style={{ width: `${blackstone}%` }} />
      </div>
      <div className="creature-feature-actions">
        <button type="button" onClick={() => updateCreature(true)}>{t("ui.summon", [], "Summon")}</button>
        <button type="button" onClick={() => updateCreature(false)}>{t("ui.dismiss", [], "Dismiss")}</button>
        <button type="button" onClick={() => updateCreature(false, true)}>{t("ui.release", [], "Release")}</button>
      </div>
      <div className="creature-feature-slots">
        {Array.from({ length: 10 }, (_, index) => (
          <span key={`creature-slot-${index}`} />
        ))}
      </div>
    </>
  );
}

function MountSystemPanel({
  t,
  world,
  onSendClientCommand,
}: {
  t: TranslateFn;
  world: DisplayWorld;
  onSendClientCommand: (command: Record<string, unknown>) => void;
}) {
  const slots = ["Reins", "Bells", "Saddle", "Ribbon", "Mask"];
  const mountItem = world.equipmentItems.find((item) => item.slot === "mount");
  return (
    <>
      <div className="system-feature-title">{t("ui.mount", [], "Mount")}</div>
      <div className="mount-feature-name">{mountItem?.name ?? "-"}</div>
      <div className="mount-feature-loyalty">
        {mountItem ? `${mountItem.durabilityCurrent} / ${mountItem.durabilityMax}` : "0 / 0"}
      </div>
      <div className="mount-feature-preview" />
      <button
        type="button"
        className="mount-feature-ride"
        onClick={() => onSendClientCommand({ type: "useItem", slot: 13, grid: "equipment" })}
      >
        {t("ui.mount", [], "Mount")}
      </button>
      <div className="mount-feature-slots">
        {slots.map((slot) => (
          <span key={slot} aria-label={slot} />
        ))}
      </div>
    </>
  );
}

function FishingSystemPanel({
  t,
  onSendClientCommand,
}: {
  t: TranslateFn;
  onSendClientCommand: (command: Record<string, unknown>) => void;
}) {
  const slots = ["Hook", "Float", "Bait", "Finder", "Reel"];
  return (
    <>
      <div className="system-feature-title">{t("ui.fishing", [], "Fishing")}</div>
      <div className="fishing-feature-water" />
      <div className="fishing-feature-slots">
        {slots.map((slot) => (
          <span key={slot} aria-label={slot} />
        ))}
      </div>
      <div className="fishing-feature-status">
        <button type="button" onClick={() => onSendClientCommand({ type: "fishingCast", castOut: true })}>
          {t("ui.cast", [], "Cast")}
        </button>
        <button type="button" onClick={() => onSendClientCommand({ type: "fishingChangeAutocast", autoCast: true })}>
          {t("ui.auto", [], "Auto")}
        </button>
        <label>
          <input
            type="checkbox"
            readOnly
            onClick={() => onSendClientCommand({ type: "fishingChangeAutocast", autoCast: true })}
          />
        </label>
      </div>
    </>
  );
}

function normalizeMapInput(value: string) {
  return value.trim().replace(/\.map$/i, "") || "0";
}

function parseFiniteInteger(value: string) {
  const parsed = Number.parseInt(value, 10);
  return Number.isFinite(parsed) ? parsed : null;
}

function GameShopWindow({
  t,
  gold,
  credits,
  playerClass,
  onBuy,
  onClose,
}: {
  t: TranslateFn;
  gold: number;
  credits: number;
  playerClass: EntityClassKey;
  onBuy: (gameShopIndex: number, quantity: number, paymentType: GameShopPaymentType) => void;
  onClose: () => void;
}) {
  const [sectionFilter, setSectionFilter] = useState<GameShopSectionFilter>("all");
  const [classFilter, setClassFilter] = useState<GameShopClassFilter>(playerClass);
  const [categoryFilter, setCategoryFilter] = useState("Show All");
  const [search, setSearch] = useState("");
  const [page, setPage] = useState(0);
  const [paymentType, setPaymentType] = useState<GameShopPaymentType>("gold");
  const [quantities, setQuantities] = useState<Record<number, number>>({});
  const [preview, setPreview] = useState<{ item: CrystalGameShopEntry; cellLeft: number } | null>(null);

  const sectionItems = useMemo(
    () => applyGameShopSectionFilter(CRYSTAL_GAME_SHOP_ITEMS, sectionFilter),
    [sectionFilter],
  );
  const classItems = useMemo(
    () => sectionItems.filter((item) => gameShopClassMatches(item.class, classFilter)),
    [sectionItems, classFilter],
  );
  const searchQuery = search.trim().toLowerCase();
  const searchedItems = useMemo(
    () =>
      classItems.filter((item) =>
        searchQuery ? item.item_name.toLowerCase().includes(searchQuery) : true,
      ),
    [classItems, searchQuery],
  );
  const categories = useMemo(
    () => [
      "Show All",
      ...Array.from(new Set(searchedItems.map((item) => item.category))).sort((left, right) =>
        left.localeCompare(right),
      ),
    ],
    [searchedItems],
  );
  const filteredItems = useMemo(
    () =>
      searchedItems
        .filter((item) => categoryFilter === "Show All" || item.category === categoryFilter)
        .slice()
        .sort(compareGameShopItems),
    [searchedItems, categoryFilter],
  );
  const pageCount = Math.max(1, Math.ceil(filteredItems.length / GAME_SHOP_ITEMS_PER_PAGE));
  const currentPage = Math.min(page, pageCount - 1);
  const visibleItems = filteredItems.slice(
    currentPage * GAME_SHOP_ITEMS_PER_PAGE,
    currentPage * GAME_SHOP_ITEMS_PER_PAGE + GAME_SHOP_ITEMS_PER_PAGE,
  );

  useEffect(() => {
    setClassFilter(playerClass);
  }, [playerClass]);

  useEffect(() => {
    setPage(0);
    if (!categories.includes(categoryFilter)) {
      setCategoryFilter("Show All");
    }
  }, [categories, categoryFilter]);

  useEffect(() => {
    if (page > pageCount - 1) {
      setPage(pageCount - 1);
    }
  }, [page, pageCount]);

  const setQuantity = (gameShopIndex: number, nextQuantity: number) => {
    setQuantities((current) => ({
      ...current,
      [gameShopIndex]: Math.max(1, Math.min(99, nextQuantity)),
    }));
  };

  const showPreview = (item: CrystalGameShopEntry, cellLeft: number) => {
    setPreview({ item, cellLeft });
  };

  return (
    <section className="game-shop-window" aria-label={t("ui.gameShop")}>
      <img className="game-shop-frame" src={ORIGINAL_UI.gameShop.frame} alt="" draggable={false} />
      <img className="game-shop-title" src={ORIGINAL_UI.gameShop.title} alt="GAMESHOP" draggable={false} />
      <div className="game-shop-close">
        <SpriteButton sprite={ORIGINAL_UI.gameShop.closeButton} label={t("ui.close")} onClick={onClose} />
      </div>
      <img className="game-shop-filter-bg" src={ORIGINAL_UI.gameShop.filterBackground} alt="" draggable={false} />
      <div className="game-shop-scroll up">
        <SpriteButton sprite={ORIGINAL_UI.gameShop.upButton} label={t("ui.up", [], "Up")} onClick={() => undefined} />
      </div>
      <div className="game-shop-scroll thumb">
        <SpriteButton sprite={ORIGINAL_UI.gameShop.positionBar} label={t("ui.scroll", [], "Scroll")} onClick={() => undefined} />
      </div>
      <div className="game-shop-scroll down">
        <SpriteButton sprite={ORIGINAL_UI.gameShop.downButton} label={t("ui.down", [], "Down")} onClick={() => undefined} />
      </div>
      <div className="game-shop-section all">
        <SpriteButton sprite={ORIGINAL_UI.gameShop.sectionTabs.all} label="All" active={sectionFilter === "all"} onClick={() => setSectionFilter("all")} />
      </div>
      <div className="game-shop-section top">
        <SpriteButton sprite={ORIGINAL_UI.gameShop.sectionTabs.top} label="Top" active={sectionFilter === "top"} onClick={() => setSectionFilter("top")} />
      </div>
      <div className="game-shop-section deals">
        <SpriteButton sprite={ORIGINAL_UI.gameShop.sectionTabs.deals} label="Deals" active={sectionFilter === "deals"} onClick={() => setSectionFilter("deals")} />
      </div>
      <div className="game-shop-section new">
        <SpriteButton sprite={ORIGINAL_UI.gameShop.sectionTabs.newItems} label="New" active={sectionFilter === "new"} onClick={() => setSectionFilter("new")} />
      </div>
      <div className="game-shop-class-tabs">
        {GAME_SHOP_CLASS_FILTERS.map((key, index) => (
          <div key={key} style={{ left: `${index === 0 ? 0 : 29 + (index - 1) * 23}px` }}>
            <SpriteButton
              sprite={ORIGINAL_UI.gameShop.classTabs[key]}
              label={key}
              active={classFilter === key}
              onClick={() => setClassFilter(key)}
            />
          </div>
        ))}
      </div>
      <input
        className="game-shop-search"
        aria-label="Search"
        value={search}
        onChange={(event) => setSearch(event.target.value)}
        spellCheck={false}
      />
      <div className="game-shop-categories">
        {categories.map((category) => (
          <button
            key={category}
            type="button"
            className={category === categoryFilter ? "active" : ""}
            onClick={() => setCategoryFilter(category)}
          >
            {category}
          </button>
        ))}
      </div>
      <div className="game-shop-cells">
        {visibleItems.map((item, index) => (
          <GameShopCell
            key={item.game_shop_index}
            item={item}
            index={index}
            quantity={quantities[item.game_shop_index] ?? 1}
            onQuantityChange={(nextQuantity) => setQuantity(item.game_shop_index, nextQuantity)}
            onBuy={() => onBuy(item.game_shop_index, quantities[item.game_shop_index] ?? 1, paymentType)}
            onPreview={(cellLeft) => showPreview(item, cellLeft)}
            t={t}
          />
        ))}
      </div>
      {preview ? (
        <GameShopViewer
          item={preview.item}
          left={preview.cellLeft < 350 ? 416 : 151}
          top={115}
          t={t}
          onClose={() => setPreview(null)}
        />
      ) : null}
      <div className="game-shop-total credits">{credits}</div>
      <div className="game-shop-total gold">{gold}</div>
      <button type="button" className="game-shop-payment gold" onClick={() => setPaymentType("gold")}>
        <img src={paymentType === "gold" ? ORIGINAL_UI.gameShop.paymentBox.checked : ORIGINAL_UI.gameShop.paymentBox.unchecked} alt="" draggable={false} />
        <span>Gold</span>
      </button>
      <button type="button" className="game-shop-payment credit" onClick={() => setPaymentType("credit")}>
        <img src={paymentType === "credit" ? ORIGINAL_UI.gameShop.paymentBox.checked : ORIGINAL_UI.gameShop.paymentBox.unchecked} alt="" draggable={false} />
        <span>Credits</span>
      </button>
      <div className="game-shop-page">{currentPage + 1} / {pageCount}</div>
      <div className="game-shop-page-button previous">
        <SpriteButton sprite={ORIGINAL_UI.gameShop.previousButton} label={t("ui.previous", [], "Previous")} onClick={() => setPage((current) => Math.max(0, current - 1))} />
      </div>
      <div className="game-shop-page-button next">
        <SpriteButton sprite={ORIGINAL_UI.gameShop.nextButton} label={t("ui.next", [], "Next")} onClick={() => setPage((current) => Math.min(pageCount - 1, current + 1))} />
      </div>
    </section>
  );
}

function GameShopCell({
  item,
  index,
  quantity,
  onQuantityChange,
  onBuy,
  onPreview,
  t,
}: {
  item: CrystalGameShopEntry;
  index: number;
  quantity: number;
  onQuantityChange: (quantity: number) => void;
  onBuy: () => void;
  onPreview: (cellLeft: number) => void;
  t: TranslateFn;
}) {
  const info = gameShopItemInfo(item.item_index);
  const left = index < 4 ? 152 + index * 132 : 152 + (index - 4) * 132;
  const top = index < 4 ? 115 : 275;
  const hasPreview = Boolean(info && GAME_SHOP_PREVIEW_ITEM_TYPES.has(info.item_type));
  const displayName = truncateGameShopName(item.item_name);

  return (
    <div className="game-shop-cell-frame" style={{ left, top }}>
      <img className="game-shop-cell-bg" src={ORIGINAL_UI.gameShop.cellFrame} alt="" draggable={false} />
      <div className="game-shop-cell-name" title={item.item_name}>{displayName}</div>
      {info ? (
        <img
          className="game-shop-cell-icon"
          src={originalItemIconPath(info.image)}
          alt=""
          draggable={false}
        />
      ) : null}
      <div className="game-shop-cell-stock-label">STOCK:</div>
      <div className="game-shop-cell-stock-value">{formatGameShopStock(item.stock)}</div>
      <div className="game-shop-cell-count">{item.count > 1 ? item.count : ""}</div>
      <div className="game-shop-cell-quantity-down">
        <SpriteButton sprite={ORIGINAL_UI.gameShop.previousButton} label={t("ui.down", [], "Down")} onClick={() => onQuantityChange(quantity - 1)} />
      </div>
      <div className="game-shop-cell-quantity">{quantity}</div>
      <div className="game-shop-cell-quantity-up">
        <SpriteButton sprite={ORIGINAL_UI.gameShop.nextButton} label={t("ui.up", [], "Up")} onClick={() => onQuantityChange(quantity + 1)} />
      </div>
      <div className="game-shop-cell-credit-price">{item.credit_price * quantity}</div>
      <div className="game-shop-cell-gold-price">{item.gold_price * quantity}</div>
      {hasPreview ? (
        <div className="game-shop-cell-preview">
          <SpriteButton sprite={ORIGINAL_UI.gameShop.previewButton} label={t("ui.preview", [], "Preview")} onClick={() => onPreview(left)} />
        </div>
      ) : null}
      <div className={hasPreview ? "game-shop-cell-buy with-preview" : "game-shop-cell-buy"}>
        <SpriteButton sprite={ORIGINAL_UI.gameShop.buyButton} label={t("ui.buy", [], "Buy")} onClick={onBuy} />
      </div>
    </div>
  );
}

function GameShopViewer({
  item,
  left,
  top,
  t,
  onClose,
}: {
  item: CrystalGameShopEntry;
  left: number;
  top: number;
  t: TranslateFn;
  onClose: () => void;
}) {
  const [direction, setDirection] = useState(6);
  const info = gameShopItemInfo(item.item_index);

  return (
    <div
      className="game-shop-viewer"
      style={{ left, top }}
      data-item-name={item.item_name}
      data-game-shop-index={item.game_shop_index}
      data-direction={direction}
    >
      <button type="button" className="game-shop-viewer-close" onClick={onClose} aria-label={t("ui.close")}>
        x
      </button>
      <div className="game-shop-viewer-stage">
        {info ? (
          <img
            className="game-shop-viewer-item-icon"
            src={originalItemIconPath(info.image)}
            alt=""
            draggable={false}
          />
        ) : null}
        <div className="game-shop-viewer-figure" data-direction={direction}>
          <div className="game-shop-viewer-head" />
          <div className="game-shop-viewer-body" />
          <div className="game-shop-viewer-item-glow" />
        </div>
      </div>
      <div className="game-shop-viewer-name">{truncateGameShopName(item.item_name)}</div>
      <div className="game-shop-viewer-controls">
        <div className="game-shop-viewer-left">
          <SpriteButton
            sprite={ORIGINAL_UI.gameShop.previousButton}
            label={t("ui.previous", [], "Previous")}
            onClick={() => setDirection((current) => (current === 1 ? 8 : current - 1))}
          />
        </div>
        <div className="game-shop-viewer-right">
          <SpriteButton
            sprite={ORIGINAL_UI.gameShop.nextButton}
            label={t("ui.next", [], "Next")}
            onClick={() => setDirection((current) => (current === 8 ? 1 : current + 1))}
          />
        </div>
      </div>
    </div>
  );
}

function applyGameShopSectionFilter(items: readonly CrystalGameShopEntry[], section: GameShopSectionFilter) {
  switch (section) {
    case "top":
      return items.slice(0, 24);
    case "deals":
      return items.filter((item) => item.gold_price > 0 && item.credit_price > 0);
    case "new":
      return [];
    case "all":
    default:
      return items;
  }
}

function gameShopClassMatches(itemClass: string, classFilter: GameShopClassFilter) {
  return classFilter === "all" || itemClass.toLowerCase() === "all" || itemClass.toLowerCase() === classFilter;
}

function gameShopItemInfo(itemIndex: number): CrystalItemEntry | undefined {
  return CRYSTAL_GAME_SHOP_ITEM_INFO_BY_INDEX[String(itemIndex) as keyof typeof CRYSTAL_GAME_SHOP_ITEM_INFO_BY_INDEX];
}

function compareGameShopItems(left: CrystalGameShopEntry, right: CrystalGameShopEntry) {
  return left.item_name.localeCompare(right.item_name) || left.game_shop_index - right.game_shop_index;
}

function truncateGameShopName(name: string) {
  return name.length > 17 ? `${name.slice(0, 17)}...` : name;
}

function formatGameShopStock(stock: number) {
  if (stock <= 0) return "\u221e";
  if (stock >= 99) return "99+";
  return String(stock);
}

function NpcDialogPanel({
  t,
  dialog,
  onClose,
  onSelectTarget,
  onSubmitInput,
}: {
  t: TranslateFn;
  dialog: DisplayNpcDialog;
  onClose: () => void;
  onSelectTarget: (target: string) => void;
  onSubmitInput: (value: string) => void;
}) {
  const [inputValue, setInputValue] = useState("");
  const bodyLines = dialog.body.map(stripCrystalDialogMarkup).filter((line) => line.trim().length > 0);
  const title = stripCrystalDialogMarkup(dialog.title || dialog.npcName);
  const footer = stripCrystalDialogMarkup(dialog.footer);

  return (
    <section className="npc-dialog-panel">
      <div className="npc-dialog-head">
        <strong>{title}</strong>
        <div className="npc-dialog-actions">
          <SpriteButton sprite={ORIGINAL_UI.mail.helpButton} label={t("ui.help", [], "Help")} onClick={() => undefined} />
          <SpriteButton sprite={ORIGINAL_UI.inventory.closeButton} label={t("ui.close")} onClick={onClose} />
        </div>
      </div>
      <div className="npc-dialog-body">
        {bodyLines.map((line, index) => (
          <p key={`${dialog.npcObjectId}-${index}`}>{line}</p>
        ))}
        {dialog.links.length ? (
          <div className="npc-dialog-links">
            {dialog.links.map((link, index) => (
              <button
                key={`${dialog.npcObjectId}-link-${index}-${link.target}`}
                type="button"
                data-target={link.target}
                onClick={() => onSelectTarget(link.target)}
              >
                {stripCrystalDialogMarkup(link.text)}
              </button>
            ))}
          </div>
        ) : null}
      </div>
      {dialog.input ? (
        <form
          className="npc-dialog-input-form"
          onSubmit={(event) => {
            event.preventDefault();
            onSubmitInput(inputValue);
            setInputValue("");
          }}
        >
          <label>
            <span>{stripCrystalDialogMarkup(dialog.input.prompt)}</span>
            <input
              value={inputValue}
              onChange={(event) => setInputValue(event.target.value)}
              autoComplete="off"
              autoFocus
            />
          </label>
          <button type="submit">{t("ui.confirm", [], "Confirm")}</button>
        </form>
      ) : null}
      {footer ? <div className="npc-dialog-footer">{footer}</div> : null}
    </section>
  );
}

function stripCrystalDialogMarkup(text: string) {
  return text
    .replace(/\{\/?[A-Z]+\}/gi, "")
    .replace(/<\$[^>]+>/g, "")
    .replace(/%[A-Z0-9_()]+/gi, "")
    .replace(/\s{2,}/g, " ")
    .trim();
}

type ChatFrameProps = {
  t: TranslateFn;
  runtimeMessage: string;
  logs: DisplayLogLine[];
  chatMessage: string;
  hints: string[];
  activeFilter: ChatFilterKey;
  expanded: boolean;
  showSettings: boolean;
  onChatMessageChange: (value: string) => void;
  onSendChat: () => void;
  onCloseSettings: () => void;
};

function ChatFrame({
  t,
  runtimeMessage,
  logs,
  chatMessage,
  hints,
  activeFilter,
  expanded,
  showSettings,
  onChatMessageChange,
  onSendChat,
  onCloseSettings,
}: ChatFrameProps) {
  const lines = playerFacingChatLines(logs, activeFilter);
  const [scrollOffset, setScrollOffset] = useState(0);
  const previousMaxScrollOffsetRef = useRef(0);
  const previousActiveFilterRef = useRef(activeFilter);
  const previousExpandedRef = useRef(expanded);
  const visibleLineCount = 4;
  const maxScrollOffset = Math.max(lines.length - visibleLineCount, 0);
  const visibleLines = lines.slice(scrollOffset, scrollOffset + visibleLineCount);
  const knobTop = maxScrollOffset === 0 ? 16 : 16 + Math.round((scrollOffset / maxScrollOffset) * 28);

  useEffect(() => {
    setScrollOffset((current) => {
      const previousMaxScrollOffset = previousMaxScrollOffsetRef.current;
      const filterChanged = previousActiveFilterRef.current !== activeFilter;
      const expandedChanged = previousExpandedRef.current !== expanded;
      previousMaxScrollOffsetRef.current = maxScrollOffset;
      previousActiveFilterRef.current = activeFilter;
      previousExpandedRef.current = expanded;

      if (filterChanged || expandedChanged || current >= previousMaxScrollOffset) {
        return maxScrollOffset;
      }

      return Math.min(current, maxScrollOffset);
    });
  }, [activeFilter, expanded, maxScrollOffset]);

  return (
    <section className={`chat-frame ${expanded ? "" : "collapsed"}`}>
      <img className="chat-frame-bg" src={ORIGINAL_UI.game.chatDialog} alt="" draggable={false} />
      <div className="chat-scroll-buttons">
        <SpriteButton sprite={ORIGINAL_UI.game.chatScrollButtons.home} label={t("ui.home")} onClick={() => setScrollOffset(0)} />
        <SpriteButton sprite={ORIGINAL_UI.game.chatScrollButtons.up} label={t("ui.up")} onClick={() => setScrollOffset((current) => Math.max(current - 1, 0))} />
        <SpriteButton sprite={ORIGINAL_UI.game.chatScrollButtons.down} label={t("ui.down")} onClick={() => setScrollOffset((current) => Math.min(current + 1, maxScrollOffset))} />
        <SpriteButton sprite={ORIGINAL_UI.game.chatScrollButtons.end} label={t("ui.end")} onClick={() => setScrollOffset(maxScrollOffset)} />
      </div>
      <img className="chat-count-bar" src={ORIGINAL_UI.game.chatCountBar} alt="" draggable={false} />
      <div className="chat-position-knob" style={{ top: knobTop }}>
        <img src={ORIGINAL_UI.game.chatScrollButtons.knob.base} alt="" draggable={false} />
      </div>
      <div className={`chat-feed ${expanded ? "" : "hidden"}`}>
        {visibleLines.map((line, index) => (
          <div
            key={`chat-line-${activeFilter}-${index}`}
            className={`chat-feed-line ${line.tone === "system" ? "system" : ""} channel-${line.channel}`}
          >
            {line.text}
          </div>
        ))}
      </div>
      {showSettings ? (
        <div className="chat-settings-panel">
          <div className="chat-settings-title">{t("ui.settings")}</div>
          <div className="chat-settings-copy">{t("ui.languageDescription")}</div>
          <div className="chat-settings-copy">{`${t("ui.size")}: ${expanded ? t("ui.down") : t("ui.up")}`}</div>
          <button type="button" className="chat-settings-close" onClick={onCloseSettings}>
            {t("ui.close")}
          </button>
        </div>
      ) : null}
      <input
        className="chat-textbox"
        value={chatMessage}
        aria-label={t("ui.worldChatPlaceholder")}
        onChange={(event) => onChatMessageChange(event.target.value)}
        onKeyDown={(event) => {
          if (event.key === "Enter") {
            onSendChat();
          }
        }}
      />
    </section>
  );
}

type ChatFilterBarProps = {
  t: TranslateFn;
  activeFilter: ChatFilterKey;
  chatExpanded: boolean;
  showSettings: boolean;
  onSelectFilter: (filter: ChatFilterKey) => void;
  onSelectTrade: () => void;
  onToggleExpanded: () => void;
  onToggleSettings: () => void;
  onToggleReport: () => void;
};

function ChatFilterBar({
  t,
  activeFilter,
  chatExpanded,
  showSettings,
  onSelectFilter,
  onSelectTrade,
  onToggleExpanded,
  onToggleSettings,
  onToggleReport,
}: ChatFilterBarProps) {
  return (
    <section className="chat-filter-bar">
      <img className="chat-filter-bg" src={ORIGINAL_UI.game.chatControlBar} alt="" draggable={false} />
      {CHAT_FILTER_BUTTONS.map(({ key, left, labelKey }) => (
        <div
          key={key}
          className="chat-filter-button"
          data-chat-filter-key={key}
          data-chat-filter-active={activeFilter === key}
          style={{ left }}
        >
          <SpriteButton
            sprite={ORIGINAL_UI.game.chatFilterButtons[key]}
            label={t(labelKey, [], labelKey)}
            onClick={() => onSelectFilter(key)}
            active={activeFilter === key}
          />
        </div>
      ))}
      <div className="chat-filter-button trade" data-chat-filter-key="trade" data-chat-filter-active={activeFilter === "trade"}>
        <SpriteButton
          sprite={ORIGINAL_UI.game.chatFilterButtons.trade}
          label={t("ui.trade")}
          onClick={onSelectTrade}
          active={activeFilter === "trade"}
        />
      </div>
      <div className="chat-filter-button size">
        <SpriteButton
          sprite={ORIGINAL_UI.game.chatFilterButtons.size}
          label={t("ui.size")}
          onClick={onToggleExpanded}
          active={!chatExpanded}
        />
      </div>
      <div className="chat-filter-button settings">
        <SpriteButton
          sprite={ORIGINAL_UI.game.chatFilterButtons.settings}
          label={t("ui.settings")}
          onClick={onToggleSettings}
          active={showSettings}
        />
      </div>
      <div className="chat-filter-button report">
        <SpriteButton
          sprite={ORIGINAL_UI.game.chatFilterButtons.report}
          label={t("ui.report")}
          onClick={onToggleReport}
        />
      </div>
    </section>
  );
}

type BeltDialogProps = {
  t: TranslateFn;
  items: DisplayItem[];
  vertical: boolean;
  onClose: () => void;
  onRotate: () => void;
  onUseItem: (item: ItemActionRef) => void;
};

function BeltDialog({ t, items, vertical, onClose, onRotate, onUseItem }: BeltDialogProps) {
  const itemBySlot = new Map(items.map((item) => [item.slot, item]));
  const useBeltItem = (item: DisplayItem) => {
    (window as typeof window & { __mir2LastBeltActivation?: Record<string, unknown> }).__mir2LastBeltActivation = {
      key: item.key,
      name: item.name,
      uniqueId: item.uniqueId,
      slot: item.slot,
      container: item.container,
      at: Date.now(),
    };
    onUseItem({
      key: item.key,
      uniqueId: item.uniqueId,
      slot: item.slot,
      container: item.container,
    });
  };

  return (
    <section className={`belt-dialog ${vertical ? "vertical" : "horizontal"}`}>
      <img
        className="belt-dialog-bg"
        src={vertical ? ORIGINAL_UI.game.belt.vertical : ORIGINAL_UI.game.belt.horizontal}
        alt=""
        draggable={false}
      />
      <img
        className="belt-dialog-bg belt-dialog-overlay"
        src={vertical ? ORIGINAL_UI.game.belt.verticalOverlay : ORIGINAL_UI.game.belt.horizontalOverlay}
        alt=""
        draggable={false}
      />
      {ORIGINAL_UI.game.belt.slots.map((slot, index) => {
        const item = itemBySlot.get(index) ?? null;

        return (
          <div
            key={slot.key}
            className="belt-slot"
            style={{
              left: vertical ? slot.verticalX : slot.horizontalX,
              top: vertical ? slot.verticalY : slot.horizontalY,
            }}
          >
            <span
              className="belt-slot-label"
              style={{
                left: vertical ? slot.verticalLabelX : slot.labelX,
                top: vertical ? slot.verticalLabelY : slot.labelY,
              }}
            >
              {index + 1}
            </span>
            {item ? (
              <button
                type="button"
                className={`belt-item ${vertical ? "vertical" : "horizontal"}`}
                title={item.name}
                onMouseDown={(event) => {
                  if (event.button !== 0) return;
                  event.preventDefault();
                  useBeltItem(item);
                }}
                onClick={(event) => {
                  if (event.detail !== 0) return;
                  useBeltItem(item);
                }}
              >
                <img
                  className="original-item-icon belt-item-icon"
                  src={originalItemIconPath(item.icon)}
                  alt=""
                  draggable={false}
                />
                {item.quantity > 1 ? <span className="item-stack-count belt-item-count">{item.quantity}</span> : null}
              </button>
            ) : null}
          </div>
        );
      })}
      <div className={`belt-button ${vertical ? "rotate-vertical" : "rotate-horizontal"}`}>
        <SpriteButton
          sprite={vertical ? ORIGINAL_UI.game.belt.rotateVertical : ORIGINAL_UI.game.belt.rotateHorizontal}
          label={t("ui.rotateBelt")}
          onClick={onRotate}
        />
      </div>
      <div className={`belt-button ${vertical ? "close-vertical" : "close-horizontal"}`}>
        <SpriteButton
          sprite={vertical ? ORIGINAL_UI.game.belt.closeVertical : ORIGINAL_UI.game.belt.closeHorizontal}
          label={t("ui.closeBelt")}
          onClick={onClose}
        />
      </div>
    </section>
  );
}

type MiniMapPanelProps = {
  t: TranslateFn;
  world: DisplayWorld;
  player: DisplayEntity | null;
  showMailPanel: boolean;
  showBigMap: boolean;
  onToggleMail: () => void;
  onToggleBigMap: () => void;
};

function MiniMapPanel({ t, world, player, showMailPanel, showBigMap, onToggleMail, onToggleBigMap }: MiniMapPanelProps) {
  const [collapsed, setCollapsed] = useState(false);
  const miniMapAsset = originalMiniMapAssetPath(world.miniMapIndex);
  const hasRasterMiniMap = Boolean(miniMapAsset);
  const panelFrame = hasRasterMiniMap ? ORIGINAL_UI.game.miniMap : ORIGINAL_UI.game.miniMapSmall;

  return (
    <section className={`mini-map-panel ${hasRasterMiniMap ? "large" : "small"}`}>
      <img className="mini-map-bg" src={panelFrame} alt="" draggable={false} />
      <div className={`mini-map-scene-shell ${collapsed || !hasRasterMiniMap ? "hidden" : ""}`}>
        <MiniMapScene world={world} player={player} />
      </div>
      {hasRasterMiniMap ? <div className="mini-map-name">
        <span>{world.mapTitle ?? t("content.scene.starterField.title")}</span>
        {world.inSafeZone ? <>
          {" "}
          <span className="mini-map-safe-zone">{t("ui.safeZone", [], "Safe Zone")}</span>
        </> : null}
      </div> : null}
      <div className="mini-map-coords">{player ? `${player.x}:${player.y}` : "--:--"}</div>
      <div className="mini-map-button mail">
        <SpriteButton
          sprite={ORIGINAL_UI.game.miniMapButtons.mail}
          label={t("client.Mail", [], "Mail")}
          onClick={onToggleMail}
          active={showMailPanel}
        />
      </div>
      <div className="mini-map-button bigmap">
        <SpriteButton sprite={ORIGINAL_UI.game.miniMapButtons.bigMap} label={t("client.BigMapKey", ["M"], t("ui.map"))} onClick={onToggleBigMap} active={showBigMap} />
      </div>
      {hasRasterMiniMap ? <div className="mini-map-button toggle">
        <SpriteButton sprite={ORIGINAL_UI.game.miniMapButtons.toggle} label={t("ui.toggleMiniMap")} onClick={() => setCollapsed((current) => !current)} />
      </div> : null}
      <img className="mini-map-light" src={ORIGINAL_UI.game.miniMapIcons.light} alt="" draggable={false} />
    </section>
  );
}

function MiniMapScene({
  world,
  player,
}: {
  world: DisplayWorld;
  player: DisplayEntity | null;
}) {
  const miniMapAssetPath = originalMiniMapAssetPath(world.miniMapIndex);
  const bounds = miniMapBounds(world, player, miniMapAssetPath);

  if (!bounds) {
    return null;
  }

  const radarDot = {
    width: (bounds.width / MINI_MAP_VIEW_WIDTH) * 2,
    height: (bounds.height / MINI_MAP_VIEW_HEIGHT) * 2,
  };

  return (
    <div className="mini-map-scene">
      {miniMapAssetPath && bounds.raster ? (
        <img
          className="mini-map-raster"
          src={miniMapAssetPath.src}
          alt=""
          draggable={false}
          style={miniMapRasterStyle(bounds.raster)}
        />
      ) : (
        <svg className="mini-map-patch-fallback" viewBox={`0 0 ${bounds.width} ${bounds.height}`} preserveAspectRatio="none">
          <rect x="0" y="0" width={bounds.width} height={bounds.height} fill="#090603" />
          {world.terrainPatches.map((patch) => (
            <rect
              key={`patch-${patch.x}-${patch.y}-${patch.kind}`}
              x={patch.x - bounds.minX}
              y={patch.y - bounds.minY}
              width={patch.width}
              height={patch.height}
              fill={miniMapTerrainColor(patch.kind)}
            />
          ))}
        </svg>
      )}
      <svg className="mini-map-overlay" viewBox={`0 0 ${bounds.width} ${bounds.height}`} preserveAspectRatio="none">
        {world.entities.map((entity) => (
          <rect
            key={`mini-${entity.objectId}`}
            x={entity.x - bounds.minX - radarDot.width / 2}
            y={entity.y - bounds.minY - radarDot.height / 2}
            width={radarDot.width}
            height={radarDot.height}
            fill={miniMapEntityColor(entity.kind)}
          />
        ))}
      </svg>
    </div>
  );
}

type DuraPanelProps = {
  t: TranslateFn;
  visible: boolean;
  equipmentItems: DisplayEquipmentItem[];
  onToggle: () => void;
};

const DURA_ICON_LAYOUT = [
  { className: "helmet", slot: "helmet" as EquipmentSlot },
  { className: "belt", slot: "belt" as EquipmentSlot },
  { className: "armour", slot: "armour" as EquipmentSlot },
  { className: "boots", slot: "boots" as EquipmentSlot },
  { className: "weapon", slot: "weapon" as EquipmentSlot },
  { className: "necklace", slot: "necklace" as EquipmentSlot },
  { className: "bracelet-left", slot: "braceletLeft" as EquipmentSlot },
  { className: "bracelet-right", slot: "braceletRight" as EquipmentSlot },
  { className: "ring-left", slot: "ringLeft" as EquipmentSlot },
  { className: "ring-right", slot: "ringRight" as EquipmentSlot },
  { className: "torch", slot: "torch" as EquipmentSlot },
  { className: "stone", slot: "stone" as EquipmentSlot },
  { className: "amulet", slot: "amulet" as EquipmentSlot },
  { className: "mount", slot: "mount" as EquipmentSlot },
];

function DuraPanel({ t, visible, equipmentItems, onToggle }: DuraPanelProps) {
  return (
    <>
      <section className="dura-status-panel">
        <div className="dura-button">
          <SpriteButton
            sprite={ORIGINAL_UI.game.miniMapButtons.dura}
            label={t("ui.duraPanel")}
            onClick={onToggle}
            active={visible}
          />
        </div>
      </section>
      {visible ? (
        <section className="dura-panel">
          <img className="dura-panel-bg" src={ORIGINAL_UI.game.duraPanel} alt="" draggable={false} />
          <img className="dura-panel-gray" src={ORIGINAL_UI.game.duraGray} alt="" draggable={false} />
          <img className="dura-panel-overlay" src={ORIGINAL_UI.game.duraBg} alt="" draggable={false} />
          {DURA_ICON_LAYOUT.map((icon) => (
            <img
              key={icon.className}
              className={`dura-piece ${icon.className}`}
              src={duraIconForSlot(icon.slot, equipmentItems)}
              alt=""
              draggable={false}
            />
          ))}
        </section>
      ) : null}
    </>
  );
}

type LoginOverlayProps = {
  language: Mir2Language;
  t: TranslateFn;
  runtimePhase: string;
  runtimeMessage: string;
  wsState: string;
  accountId: string;
  password: string;
  loginBusy: boolean;
  loginError: string | null;
  onLanguageChange: (language: Mir2Language) => void;
  onAccountIdChange: (value: string) => void;
  onPasswordChange: (value: string) => void;
  onCreateAccount: () => void;
  onSubmitLogin: () => void;
  onQuickEnter: () => void;
  onResetClient: () => void;
};

function LoginOverlay({
  language,
  t,
  runtimePhase,
  runtimeMessage,
  wsState,
  accountId,
  password,
  loginBusy,
  loginError,
  onLanguageChange,
  onAccountIdChange,
  onPasswordChange,
  onCreateAccount,
  onSubmitLogin,
  onQuickEnter,
  onResetClient,
}: LoginOverlayProps) {
  const loginNotice = loginError ?? (loginBusy ? t("ui.loggingIn") : null);
  const [showAccountPanel, setShowAccountPanel] = useState(false);

  return (
    <section className="login-overlay">
      <LanguageSelector
        language={language}
        t={t}
        compact
        className="login-language-selector"
        onLanguageChange={onLanguageChange}
      />
      <div className="login-dialog">
        <img className="login-panel" src={ORIGINAL_UI.login.dialog} alt="" draggable={false} />
        <img className="login-title" src={ORIGINAL_UI.login.title} alt="" draggable={false} />
        <img className="login-label account" src={ORIGINAL_UI.login.accountLabel} alt="" draggable={false} />
        <img className="login-label password" src={ORIGINAL_UI.login.passwordLabel} alt="" draggable={false} />
        <input
          className="login-input account"
          value={accountId}
          onChange={(event) => onAccountIdChange(event.target.value)}
          onKeyDown={(event) => {
            if (event.key === "Enter") {
              event.preventDefault();
              onSubmitLogin();
            }
          }}
          autoComplete="off"
        />
        <input
          className="login-input password"
          type="password"
          value={password}
          onChange={(event) => onPasswordChange(event.target.value)}
          onKeyDown={(event) => {
            if (event.key === "Enter") {
              event.preventDefault();
              onSubmitLogin();
            }
          }}
          autoComplete="off"
        />
        <div className="login-button ok">
          <SpriteButton sprite={ORIGINAL_UI.login.buttons.ok} label={t("ui.login")} onClick={onSubmitLogin} />
        </div>
        <div className="login-button account">
          <SpriteButton
            sprite={ORIGINAL_UI.login.buttons.newAccount}
            label={t("client.NewAccount", [], "New Account")}
            onClick={onCreateAccount}
          />
        </div>
        <div className="login-button password">
          <SpriteButton sprite={ORIGINAL_UI.login.buttons.changePassword} label={t("ui.quickEnter")} onClick={onQuickEnter} />
        </div>
        <div className="login-button view">
          <SpriteButton
            sprite={ORIGINAL_UI.login.buttons.viewKey}
            label={t("ui.viewKey")}
            onClick={() => setShowAccountPanel((current) => !current)}
          />
        </div>
        <div className="login-button close">
          <SpriteButton sprite={ORIGINAL_UI.login.buttons.close} label={t("ui.close")} onClick={onResetClient} />
        </div>
      </div>
      {showAccountPanel ? (
        <div className="login-account-panel">
          <strong>{t("ui.viewKey")}</strong>
          <span>{accountId || "-"}</span>
          <button type="button" onClick={() => setShowAccountPanel(false)}>
            {t("ui.close")}
          </button>
        </div>
      ) : null}
      {loginNotice ? <div className="login-feedback">{loginNotice}</div> : null}
      {runtimePhase === "boot-error" || wsState === "closed" ? (
        <div className="login-runtime-stamp" aria-hidden="true">{`${runtimePhase} / ${wsState} / ${runtimeMessage}`}</div>
      ) : null}
    </section>
  );
}

type SelectOverlayProps = {
  language: Mir2Language;
  t: TranslateFn;
  characters: SelectCharacterEntry[];
  selectedCharacterIndex: number;
  accountId: string;
  selectedPortraitFrame: SelectPortraitFrame | null;
  onLanguageChange: (language: Mir2Language) => void;
  onSelectCharacter: (index: number) => void;
  onEnterWorld: () => void;
  onCreateCharacter: () => void;
  onDeleteCharacter: () => void;
  onExit: () => void;
};

function SelectOverlay({
  language,
  t,
  characters,
  selectedCharacterIndex,
  accountId,
  selectedPortraitFrame,
  onLanguageChange,
  onSelectCharacter,
  onEnterWorld,
  onCreateCharacter,
  onDeleteCharacter,
  onExit,
}: SelectOverlayProps) {
  const selected = characters[selectedCharacterIndex] ?? null;
  const [showCreditsPanel, setShowCreditsPanel] = useState(false);
  const [showDeleteConfirm, setShowDeleteConfirm] = useState(false);

  return (
    <section className="select-overlay">
      <div className="select-scene">
        <LanguageSelector
          language={language}
          t={t}
          compact
          className="select-language-selector"
          onLanguageChange={onLanguageChange}
        />
        <img className="select-background-frame" src={ORIGINAL_UI.select.background} alt="" draggable={false} />
        <img className="select-title" src={ORIGINAL_UI.select.title} alt="" draggable={false} />
        <div className="select-server-name">{t("client.GameName", [], "Legend of Mir 2")}</div>

        <div
          className="select-portrait-anchor"
          style={{ left: SELECT_PORTRAIT_ANCHOR.x, top: SELECT_PORTRAIT_ANCHOR.y }}
        >
          {selectedPortraitFrame ? (
            <img
              className="select-portrait-frame"
              src={selectedPortraitFrame.path}
              alt=""
              draggable={false}
              style={{ left: selectedPortraitFrame.x, top: selectedPortraitFrame.y }}
            />
          ) : null}
        </div>

        <div className="select-last-access-label">{t("client.LastOnlineTitle", [], "Last Online:")}</div>
        <div className="select-last-access-value">{selected?.lastAccess ?? t("client.Never", [], "Never")}</div>

        {Array.from({ length: 4 }, (_, slotIndex) => {
          const character = characters[slotIndex] ?? null;
          return (
            <button
              key={`select-slot-${slotIndex}`}
              type="button"
              className={`select-character-slot-card row-${slotIndex + 1} ${character ? "" : "empty"} ${character && slotIndex === selectedCharacterIndex ? "selected" : ""}`}
              disabled={!character}
              onClick={() => {
                if (character) {
                  onSelectCharacter(slotIndex);
                }
              }}
            >
              {character ? (
                <>
                  <img
                    className="select-character-slot-frame"
                    src={classCardForCharacter(character, slotIndex === selectedCharacterIndex)}
                    alt=""
                    draggable={false}
                  />
                  <div className="select-character-slot-copy">
                    <strong className="name">{character.name}</strong>
                    <span className="level">{character.level}</span>
                    <span className="job">{selectClassLabel(t, character.classKey)}</span>
                  </div>
                </>
              ) : (
                <img className="select-character-slot-frame" src={ORIGINAL_UI.select.emptySlot} alt="" draggable={false} />
              )}
            </button>
          );
        })}

        <div className="select-action start"><SpriteButton sprite={ORIGINAL_UI.select.buttons.start} label={t("ui.startGame")} onClick={onEnterWorld} /></div>
        <div className="select-action new"><SpriteButton sprite={ORIGINAL_UI.select.buttons.newCharacter} label={t("ui.newCharacter")} onClick={onCreateCharacter} /></div>
        <div className="select-action delete">
          <SpriteButton
            sprite={ORIGINAL_UI.select.buttons.deleteCharacter}
            label={t("ui.deleteCharacter")}
            onClick={() => setShowDeleteConfirm((current) => !current)}
          />
        </div>
        <div className="select-action credits">
          <SpriteButton
            sprite={ORIGINAL_UI.select.buttons.credits}
            label={t("ui.credits")}
            onClick={() => setShowCreditsPanel((current) => !current)}
          />
        </div>
        <div className="select-action exit"><SpriteButton sprite={ORIGINAL_UI.select.buttons.exit} label={t("ui.exit")} onClick={onExit} /></div>
        {showCreditsPanel ? (
          <div className="select-credits-panel">
            <strong>{t("ui.credits")}</strong>
            <span>{t("client.GameName", [], "Legend of Mir 2")}</span>
            <span>{accountId}</span>
            <button type="button" onClick={() => setShowCreditsPanel(false)}>
              {t("ui.close")}
            </button>
          </div>
        ) : null}
        {showDeleteConfirm ? (
          <div className="select-delete-panel">
            <strong>{t("ui.deleteCharacter")}</strong>
            <span>{selected?.name ?? "-"}</span>
            <div className="select-delete-actions">
              <button
                type="button"
                onClick={() => {
                  onDeleteCharacter();
                  setShowDeleteConfirm(false);
                }}
              >
                {t("ui.confirm", [], "Confirm")}
              </button>
              <button type="button" onClick={() => setShowDeleteConfirm(false)}>
                {t("ui.close")}
              </button>
            </div>
          </div>
        ) : null}
      </div>
    </section>
  );
}

type MainHudProps = {
  t: TranslateFn;
  connected: boolean;
  mapTitle: string | null;
  player: DisplayEntity | null;
  world: DisplayWorld;
  showCharacter: boolean;
  showInventory: boolean;
  activeCharacterTab: CharacterTabKey;
  activeInventoryTab: InventoryTabKey;
  onToggleCharacter: () => void;
  onToggleInventory: () => void;
  onOpenCharacterTab: (tab: CharacterTabKey) => void;
  onOpenInventoryTab: (tab: InventoryTabKey) => void;
  onDropGold: () => void;
  onLogout: () => void;
  showGameShop: boolean;
  onToggleGameShop: () => void;
  showMenu: boolean;
  onToggleMenu: () => void;
};

function MainHud({
  t,
  connected,
  mapTitle,
  player,
  world,
  showCharacter,
  showInventory,
  activeCharacterTab,
  activeInventoryTab,
  onToggleCharacter,
  onToggleInventory,
  onOpenCharacterTab,
  onOpenInventoryTab,
  onDropGold,
  onLogout,
  showGameShop,
  onToggleGameShop,
  showMenu,
  onToggleMenu,
}: MainHudProps) {
  const healthRatio = ratio(world.playerHp, world.playerMaxHp);
  const manaRatio = ratio(world.playerMp, Math.max(world.playerMp ?? 0, 100));
  const experienceRatio = ratio(world.playerExperience, world.playerMaxExperience);
  const currentHp = world.playerHp ?? 0;
  const maxHp = world.playerMaxHp ?? 0;
  const currentMp = world.playerMp ?? 0;
  const maxMp = 100;
  const hpOnlyOrb = (player?.classKey ?? "warrior") === "warrior" && (player?.level ?? 1) < 26;
  const locationLabel = mapTitle ?? world.mapTitle ?? "";
  const buffLabel = world.activeBuffs
    .slice(0, 2)
    .map((buff) => `${buff.name}:${buff.remainingTicks}`)
    .join("  ");

  return (
    <div className="main-hud-shell">
      <div className="main-hud">
        <img className="hud-cap left" src={ORIGINAL_UI.hud.leftCap} alt="" draggable={false} />
        <img className="hud-base" src={ORIGINAL_UI.hud.base} alt="" draggable={false} />
        <img className="hud-cap right" src={ORIGINAL_UI.hud.rightCap} alt="" draggable={false} />
        <img className="hud-exp-bar" src={ORIGINAL_UI.hud.experienceBar} alt="" draggable={false} />
        <img className="hud-weight-bar" src={ORIGINAL_UI.hud.weightBar} alt="" draggable={false} />

        <div className={`hud-orb-fill hp ${hpOnlyOrb ? "hp-only" : ""}`} style={{ height: `${80 * healthRatio}px` }}>
          <img src={hpOnlyOrb ? ORIGINAL_UI.hud.healthOnlyOrb : ORIGINAL_UI.hud.healthManaOrb} alt="" draggable={false} />
        </div>
        <div className={`hud-orb-fill mp ${hpOnlyOrb ? "hidden" : ""}`} style={{ height: `${80 * manaRatio}px` }}>
          <img src={ORIGINAL_UI.hud.healthManaOrb} alt="" draggable={false} />
        </div>

        {hpOnlyOrb ? (
          <div className="hud-health-only-label">{`HP ${currentHp}/${maxHp}`}</div>
        ) : (
          <>
            <div className="hud-top-label">{`${currentHp}    ${currentMp}`}</div>
            <div className="hud-bottom-label">{`${maxHp}    ${maxMp}`}</div>
          </>
        )}
        <div className="hud-level-label">{player?.level ?? 1}</div>
        <div className="hud-name-label">{player?.name ?? ""}</div>
        <div className="hud-map-label">
          {locationLabel}
          {world.inSafeZone ? ` ${t("ui.safeZone", [], "Safe Zone")}` : ""}
        </div>
        {buffLabel ? <div className="hud-buff-label">{buffLabel}</div> : null}
        <div className="hud-exp-label">{`${experienceRatio.toFixed(2).replace(/^0/, "") === ".00" ? "0.00" : (experienceRatio * 100).toFixed(2)}%`}</div>
        <div className="hud-gold-label">{connected ? `${world.gold}` : "0"}</div>
        <div className="hud-weight-label">{`${world.freeBagSlots}/${world.maxBagSlots}`}</div>
        <div className="hud-space-label">{`${world.currentWeight}`}</div>

        <div className="hud-button shop">
          <SpriteButton sprite={ORIGINAL_UI.hud.buttons.gameShop} label={t("ui.gameShop")} onClick={onToggleGameShop} active={showGameShop} />
        </div>
        <div className="hud-button menu">
          <SpriteButton sprite={ORIGINAL_UI.hud.buttons.menu} label={t("ui.menu")} onClick={onToggleMenu} active={showMenu} />
        </div>
        <div className="hud-button character">
          <SpriteButton sprite={ORIGINAL_UI.hud.buttons.character} label={t("ui.character")} onClick={onToggleCharacter} active={showCharacter && activeCharacterTab === "char"} />
        </div>
        <div className="hud-button inventory">
          <SpriteButton sprite={ORIGINAL_UI.hud.buttons.inventory} label={t("ui.inventory")} onClick={onToggleInventory} active={showInventory && activeInventoryTab === "bag1"} />
        </div>
        <div className="hud-button skill">
          <SpriteButton sprite={ORIGINAL_UI.hud.buttons.skill} label={t("ui.skills")} onClick={() => onOpenCharacterTab("spells")} active={showCharacter && activeCharacterTab === "spells"} />
        </div>
        <div className="hud-button quest">
          <SpriteButton sprite={ORIGINAL_UI.hud.buttons.quest} label={t("ui.quest")} onClick={() => onOpenInventoryTab("quest")} active={showInventory && activeInventoryTab === "quest"} />
        </div>
        <div className="hud-button option">
          <SpriteButton sprite={ORIGINAL_UI.hud.buttons.option} label={t("ui.options")} onClick={() => onOpenCharacterTab("stats2")} active={showCharacter && activeCharacterTab === "stats2"} />
        </div>
      </div>
    </div>
  );
}

function ratio(value?: number, max?: number) {
  if (value === undefined || max === undefined || max <= 0) {
    return 0;
  }

  return Math.max(0, Math.min(1, value / max));
}

type InventoryWindowProps = {
  t: TranslateFn;
  locale: string;
  activeTab: InventoryTabKey;
  world: DisplayWorld;
  storageServiceOpenVersion: number;
  onClose: () => void;
  onTabChange: (tab: InventoryTabKey) => void;
  onUseItem: (item: ItemActionRef) => void;
  onDropItem: (item: ItemActionRef) => void;
  onEquipItem: (item: ItemActionRef, slot: EquipmentSlot) => void;
  onMoveItem: (item: MoveItemRef, toSlot: number) => void;
  onMergeItem: (from: MergeItemRef, to: MergeItemRef) => void;
  onSplitItem: (item: ItemActionRef, count: number) => void;
  onStoreItem: (item: MoveItemRef, toSlot: number) => void;
  onTakeBackItem: (item: MoveItemRef, toSlot: number) => void;
  onRentExpandedStorage: () => void;
  onUnlockStorage: (password: string) => void;
  onSetStoragePassword: (currentPassword: string, newPassword: string) => void;
  onRemoveStoragePassword: (currentPassword: string) => void;
  onSellItem: (item: ItemActionRef, count: number) => void;
  onDropGold: (amount: number) => void;
};

export function InventoryWindow({
  t,
  locale,
  activeTab,
  world,
  storageServiceOpenVersion,
  onClose,
  onTabChange,
  onUseItem,
  onDropItem,
  onEquipItem,
  onMoveItem,
  onMergeItem,
  onSplitItem,
  onStoreItem,
  onTakeBackItem,
  onRentExpandedStorage,
  onUnlockStorage,
  onSetStoragePassword,
  onRemoveStoragePassword,
  onSellItem,
  onDropGold,
}: InventoryWindowProps) {
  const [deleteMode, setDeleteMode] = useState(false);
  const [sellMode, setSellMode] = useState(false);
  const [storageMode, setStorageMode] = useState<"store" | "takeBack" | null>(null);
  const [storagePageIndex, setStoragePageIndex] = useState<0 | 1>(0);
  const [showStoragePasswordPanel, setShowStoragePasswordPanel] = useState(false);
  const [storagePasswordPanelMode, setStoragePasswordPanelMode] = useState<"unlock" | "set" | "change" | "remove">(
    "unlock",
  );
  const [storagePassword, setStoragePassword] = useState("");
  const [newStoragePassword, setNewStoragePassword] = useState("");
  const [confirmStoragePassword, setConfirmStoragePassword] = useState("");
  const storageProtectionEnabled = world.requireStoragePassword || world.hasStoragePassword;
  const storageLocked = storageProtectionEnabled && !world.storageSessionUnlocked;
  const visibleItems = world.inventoryItems.filter((item) => item.container === activeTab);
  const storagePageStart = storagePageIndex * 80;
  const storagePageEnd = storagePageStart + 80;
  const storagePageLocked = storagePageIndex === 1 && !world.hasExpandedStorage;
  const visibleStorageItems = storagePageLocked
    ? []
    : world.storageItems.filter((item) => item.slot >= storagePageStart && item.slot < storagePageEnd);
  const showStorageWindow = storageMode !== null;
  const [pendingDeleteItem, setPendingDeleteItem] = useState<DisplayItem | null>(null);
  const [pendingSellItem, setPendingSellItem] = useState<DisplayItem | null>(null);
  const [deleteFeedback, setDeleteFeedback] = useState<string | null>(null);
  const [pendingMoveItem, setPendingMoveItem] = useState<DisplayItem | null>(null);
  const [pendingSplitItem, setPendingSplitItem] = useState<DisplayItem | null>(null);
  const [splitCount, setSplitCount] = useState("1");
  const [pendingGoldDrop, setPendingGoldDrop] = useState(false);
  const [goldDropAmount, setGoldDropAmount] = useState("100");

  useEffect(() => {
    if (storageServiceOpenVersion <= 0) {
      return;
    }

    setStorageMode("takeBack");
    setDeleteMode(false);
    setSellMode(false);
    setPendingDeleteItem(null);
    setPendingSellItem(null);
    setPendingMoveItem(null);
    setPendingSplitItem(null);
    setPendingGoldDrop(false);
    setDeleteFeedback(null);

    if (storageLocked) {
      setShowStoragePasswordPanel(true);
      setStoragePasswordPanelMode("unlock");
      setStoragePassword("");
      setNewStoragePassword("");
      setConfirmStoragePassword("");
    } else if (world.requireStoragePassword && !world.hasStoragePassword) {
      setShowStoragePasswordPanel(true);
      setStoragePasswordPanelMode("set");
      setStoragePassword("");
      setNewStoragePassword("");
      setConfirmStoragePassword("");
    }
  }, [storageServiceOpenVersion, storageLocked, world.hasStoragePassword, world.requireStoragePassword]);

  useEffect(() => {
    if (!deleteFeedback) {
      return;
    }

    const timer = window.setTimeout(() => {
      setDeleteFeedback(null);
    }, 2200);

    return () => window.clearTimeout(timer);
  }, [deleteFeedback]);

  useEffect(() => {
    if (!showStorageWindow) {
      setStoragePageIndex(0);
      setShowStoragePasswordPanel(false);
      setStoragePasswordPanelMode("unlock");
      setStoragePassword("");
      setNewStoragePassword("");
      setConfirmStoragePassword("");
      return;
    }

    if (world.requireStoragePassword && !world.hasStoragePassword) {
      setShowStoragePasswordPanel(true);
      setStoragePasswordPanelMode("set");
      return;
    }

    if (storageLocked) {
      setShowStoragePasswordPanel(true);
      setStoragePasswordPanelMode("unlock");
    }
  }, [showStorageWindow, storageLocked, world.hasStoragePassword, world.requireStoragePassword]);

  const storageStatusText = storageLocked
    ? t("ui.storageLocked", [], "Storage is locked. Unlock to access warehouse items.")
    : storageProtectionEnabled
      ? t("ui.storageUnlocked", [], "Storage is unlocked for this session.")
      : t("ui.storageNoPassword", [], "Storage password is not set.");
  const storagePageText =
    storagePageIndex === 0
      ? t("ui.storagePageOne", [], "Storage Page 1")
      : t("ui.storagePageTwo", [], "Storage Page 2");
  const expandedStorageText =
    storagePageIndex === 1
      ? world.hasExpandedStorage
        ? t("ui.expandedStorageEnabled", [], "Expanded storage is active.")
        : t("ui.expandedStorageLocked", [], "Expanded storage is locked.")
      : world.hasExpandedStorage
        ? t("ui.expandedStorageEnabled", [], "Expanded storage is active.")
        : t("ui.expandedStorageHint", [], "Select page 2 to preview expanded storage.")
      ;
  const storageModeText =
    storageMode === "store"
      ? t("ui.storageStoreHint", [], "Choose an inventory item, then click a warehouse slot.")
      : storageMode === "takeBack"
        ? t("ui.storageTakeBackHint", [], "Choose a warehouse item, then click an inventory slot.")
        : pendingMoveItem?.container === "storage"
          ? `${t("ui.storageMode", [], "Storage items")}: ${pendingMoveItem.name}`
          : storageStatusText;
  const storageCapacityText = `${t("ui.storageCapacity", [], "Capacity")}: ${world.storageItems.length}/${world.storageSize}`;
  const storageProtectEnabled = !storagePageLocked;
  const storagePasswordLastSetText = formatBinaryDateTimeLabel(
    locale,
    world.storagePasswordLastSetBinaryDatetime,
    t("ui.storagePasswordLastSet", ["{0}"], "Last set: {0}"),
  );
  const storageRentalText =
    storagePageIndex === 1
      ? formatBinaryDateTimeLabel(
          locale,
          world.expandedStorageExpiryTimeBinaryDatetime,
          t("ui.expandedStorageExpiresOn", ["{0}"], "Expanded storage expires on {0}"),
        ) ?? expandedStorageText
      : null;
  const storageStatusLabelText =
    storageMode !== null || pendingMoveItem?.container === "storage" || storageLocked ? storageModeText : null;
  const effectiveStoragePasswordPanelMode = storageLocked ? "unlock" : storagePasswordPanelMode;
  const effectiveShowStoragePasswordPanel = showStoragePasswordPanel || (showStorageWindow && storageLocked);
  const storagePasswordPanelTitle =
    effectiveStoragePasswordPanelMode === "unlock"
      ? t("ui.unlockStorage", [], "Unlock Storage")
      : effectiveStoragePasswordPanelMode === "set"
        ? t("ui.setStoragePassword", [], "Set Storage Password")
        : effectiveStoragePasswordPanelMode === "change"
          ? t("ui.changeStoragePassword", [], "Change Storage Password")
          : t("ui.removeStoragePassword", [], "Remove Storage Password");
  const storagePasswordPanelPrompt =
    effectiveStoragePasswordPanelMode === "unlock"
      ? t("client.StoragePasswordPrompt", [], "Enter your storage password.")
      : effectiveStoragePasswordPanelMode === "set"
        ? t("client.StoragePasswordNewPrompt", [], "Enter a new storage password.")
        : effectiveStoragePasswordPanelMode === "change"
          ? t("client.StoragePasswordChangePrompt", [], "Change your storage password.")
          : t("ui.storagePasswordRemovePrompt", [], "Enter your current password to remove storage protection.");
  const storagePasswordRules = t("client.StoragePasswordRules", [4, 20], "Password must be between {0} and {1} characters.");
  const passwordConfirmationMismatch =
    (effectiveStoragePasswordPanelMode === "set" || effectiveStoragePasswordPanelMode === "change") &&
    confirmStoragePassword.length > 0 &&
    newStoragePassword !== confirmStoragePassword;

  function closeStorageWindow() {
    setStorageMode(null);
    setPendingMoveItem(null);
    setPendingSplitItem(null);
    setShowStoragePasswordPanel(false);
    if (activeTab === "quest") {
      onTabChange("bag1");
    }
  }

  function openStoragePasswordPanel() {
    setStoragePassword("");
    setNewStoragePassword("");
    setConfirmStoragePassword("");
    if (!world.hasStoragePassword) {
      setStoragePasswordPanelMode("set");
    } else if (storageLocked) {
      setStoragePasswordPanelMode("unlock");
    } else {
      setStoragePasswordPanelMode("change");
    }
    setShowStoragePasswordPanel(true);
  }

  function closeStoragePasswordPanel() {
    setShowStoragePasswordPanel(false);
    setStoragePassword("");
    setNewStoragePassword("");
    setConfirmStoragePassword("");
  }

  function submitStoragePasswordPanel() {
    if (effectiveStoragePasswordPanelMode === "unlock") {
      onUnlockStorage(storagePassword);
      return;
    }

    if (effectiveStoragePasswordPanelMode === "remove") {
      onRemoveStoragePassword(storagePassword);
      return;
    }

    if (newStoragePassword !== confirmStoragePassword) {
      setDeleteFeedback(t("client.StoragePasswordMismatch", [], "Storage password confirmation does not match."));
      return;
    }

    onSetStoragePassword(effectiveStoragePasswordPanelMode === "set" ? "" : storagePassword, newStoragePassword);
  }

  function storagePasswordPanelCanSubmit() {
    switch (effectiveStoragePasswordPanelMode) {
      case "unlock":
      case "remove":
        return storagePassword.trim().length > 0;
      case "set":
        return newStoragePassword.trim().length > 0 && newStoragePassword === confirmStoragePassword;
      case "change":
        return (
          storagePassword.trim().length > 0 &&
          newStoragePassword.trim().length > 0 &&
          newStoragePassword === confirmStoragePassword
        );
    }
  }

  function activateInventoryItem(item: DisplayItem) {
    (window as typeof window & { __mir2LastInventoryActivation?: Record<string, unknown> }).__mir2LastInventoryActivation = {
      key: item.key,
      name: item.name,
      slot: item.slot,
      container: item.container,
      storageMode,
      deleteMode,
      sellMode,
      hasPendingMoveItem: Boolean(pendingMoveItem),
      at: Date.now(),
    };
    if (storageMode === "store") {
      if (item.container === "storage") {
        return;
      }
      setPendingMoveItem(item);
      setPendingSplitItem(null);
      setDeleteFeedback(`${t("ui.storeItem", [], "Store Item")}: ${item.name}`);
      return;
    }
    if (storageMode === "takeBack") {
      if (item.container !== "storage") {
        return;
      }
      setPendingMoveItem(item);
      setPendingSplitItem(null);
      setDeleteFeedback(`${t("ui.takeBackItem", [], "Take Back")}: ${item.name}`);
      return;
    }
    if (deleteMode) {
      setPendingDeleteItem(item);
      return;
    }
    if (sellMode) {
      setPendingSellItem(item);
      return;
    }
    if (pendingMoveItem) {
      if (pendingMoveItem.slot === item.slot && pendingMoveItem.container === item.container) {
        setPendingMoveItem(null);
        return;
      }

      if (pendingMoveItem.key === item.key && pendingMoveItem.container === item.container) {
        onMergeItem(
          {
            uniqueId: pendingMoveItem.uniqueId,
            slot: pendingMoveItem.slot,
            container: pendingMoveItem.container,
          },
          {
            uniqueId: item.uniqueId,
            slot: item.slot,
            container: item.container,
          },
        );
      } else {
        onMoveItem(
          {
            uniqueId: pendingMoveItem.uniqueId,
            slot: pendingMoveItem.slot,
            container: pendingMoveItem.container,
          },
          item.slot,
        );
      }
      setDeleteFeedback(`${t("ui.inventory")}: ${pendingMoveItem.name} -> ${item.name}`);
      setPendingMoveItem(null);
      return;
    }

    const equipSlot = equipmentSlotForItemKey(item.key);
    if (equipSlot) {
      onEquipItem(
        {
          key: item.key,
          uniqueId: item.uniqueId,
          slot: item.slot,
          container: item.container,
        },
        equipSlot,
      );
    } else {
      onUseItem({
        key: item.key,
        uniqueId: item.uniqueId,
        slot: item.slot,
        container: item.container,
      });
    }
  }

  function confirmSellItem(item: DisplayItem) {
    (window as typeof window & { __mir2LastInventoryConfirmation?: Record<string, unknown> }).__mir2LastInventoryConfirmation = {
      action: "sell",
      key: item.key,
      name: item.name,
      uniqueId: item.uniqueId,
      slot: item.slot,
      container: item.container,
      at: Date.now(),
    };
    onSellItem(
      {
        key: item.key,
        uniqueId: item.uniqueId,
        slot: item.slot,
        container: item.container,
      },
      1,
    );
    setDeleteFeedback(`${t("ui.sellItem", [], "Sell Item")}: ${item.name}`);
    setPendingSellItem(null);
    setSellMode(false);
  }

  function confirmDeleteItem(item: DisplayItem) {
    (window as typeof window & { __mir2LastInventoryConfirmation?: Record<string, unknown> }).__mir2LastInventoryConfirmation = {
      action: "drop",
      key: item.key,
      name: item.name,
      uniqueId: item.uniqueId,
      slot: item.slot,
      container: item.container,
      at: Date.now(),
    };
    onDropItem({
      key: item.key,
      uniqueId: item.uniqueId,
      slot: item.slot,
      container: item.container,
    });
    setDeleteFeedback(`${t("ui.deleteItem")}: ${item.name}`);
    setPendingDeleteItem(null);
    setDeleteMode(false);
  }

  function confirmSplitItem(item: DisplayItem) {
    const count = Number.parseInt(splitCount, 10);
    if (!Number.isFinite(count) || count <= 0) {
      return;
    }
    (window as typeof window & { __mir2LastInventoryConfirmation?: Record<string, unknown> }).__mir2LastInventoryConfirmation = {
      action: "split",
      key: item.key,
      name: item.name,
      uniqueId: item.uniqueId,
      slot: item.slot,
      container: item.container,
      count,
      at: Date.now(),
    };
    onSplitItem(
      {
        key: item.key,
        uniqueId: item.uniqueId,
        slot: item.slot,
        container: item.container,
      },
      count,
    );
    setDeleteFeedback(`${t("ui.splitItem", [], "Split Item")}: ${item.name} x${count}`);
    setPendingSplitItem(null);
  }

  function confirmGoldDrop() {
    const amount = Number.parseInt(goldDropAmount, 10);
    if (!Number.isFinite(amount) || amount <= 0) {
      return;
    }
    (window as typeof window & { __mir2LastInventoryConfirmation?: Record<string, unknown> }).__mir2LastInventoryConfirmation = {
      action: "dropGold",
      amount,
      at: Date.now(),
    };
    onDropGold(amount);
    setDeleteFeedback(`${t("ui.dropGold", [], "Drop Gold")}: ${amount}`);
    setPendingGoldDrop(false);
  }

  function primaryMouseAction(event: MouseEvent, action: () => void) {
    if (event.button !== 0) return;
    event.preventDefault();
    action();
  }

  return (
    <div className={`window-shell inventory-window ${showStorageWindow ? "with-storage" : ""}`}>
      <img className="window-frame" src={ORIGINAL_UI.inventory.frame} alt="" draggable={false} />

      <div className="inventory-tab tab-one">
        <button type="button" className="window-tab-button" onClick={() => onTabChange("bag1")}>
          <img src={activeTab === "bag1" ? ORIGINAL_UI.inventory.tabs.bag1.active : ORIGINAL_UI.inventory.tabs.bag1.idle} alt={t("ui.bag1")} draggable={false} />
        </button>
      </div>
      <div className="inventory-tab tab-two">
        <button type="button" className="window-tab-button" onClick={() => onTabChange("bag2")}>
          <img src={activeTab === "bag2" ? ORIGINAL_UI.inventory.tabs.bag2.active : ORIGINAL_UI.inventory.tabs.bag2.idle} alt={t("ui.bag2")} draggable={false} />
        </button>
      </div>
      <div className="inventory-tab tab-three">
        <button
          type="button"
          className="window-tab-button"
          onClick={() => {
            onTabChange("quest");
            setStorageMode("takeBack");
            setPendingMoveItem(null);
            setPendingSplitItem(null);
            if (storageLocked) {
              setShowStoragePasswordPanel(true);
              setStoragePasswordPanelMode("unlock");
            } else {
              setShowStoragePasswordPanel(false);
            }
          }}
        >
          <img src={activeTab === "quest" ? ORIGINAL_UI.inventory.tabs.quest.active : ORIGINAL_UI.inventory.tabs.quest.idle} alt={t("ui.quest")} draggable={false} />
        </button>
      </div>

      <div className="inventory-close">
        <SpriteButton sprite={ORIGINAL_UI.inventory.closeButton} label={t("ui.closeInventory")} onClick={onClose} />
      </div>
      <div className="inventory-delete">
        <SpriteButton
          sprite={ORIGINAL_UI.inventory.deleteButton}
          label={t("ui.deleteItem")}
          onClick={() => {
            setDeleteMode((current) => !current);
            setSellMode(false);
            setPendingDeleteItem(null);
            setPendingSellItem(null);
            setPendingMoveItem(null);
            setPendingSplitItem(null);
            setPendingGoldDrop(false);
            setStorageMode(null);
            setDeleteFeedback(null);
          }}
          active={deleteMode}
        />
      </div>
      <div className="inventory-sell">
        <button
          type="button"
          aria-label={t("ui.sellItem", [], "Sell Item")}
          title={t("ui.sellItem", [], "Sell Item")}
          className={sellMode ? "active" : ""}
          onClick={() => {
            setSellMode((current) => !current);
            setDeleteMode(false);
            setPendingDeleteItem(null);
            setPendingSellItem(null);
            setPendingMoveItem(null);
            setPendingSplitItem(null);
            setPendingGoldDrop(false);
            setStorageMode(null);
            setDeleteFeedback(null);
          }}
        >
          {t("ui.sell", [], "Sell")}
        </button>
      </div>
      <div className="inventory-grid">
        {ORIGINAL_UI.inventory.slots.map((slot, slotIndex) => (
          <div
            key={slot.key}
            className={`inventory-slot ${activeTab === "quest" ? "quest" : ""}`}
            style={{ left: slot.x, top: slot.y }}
	            title={slot.key}
	            onClick={() => {
	              const takeBackItem =
	                pendingMoveItem?.container === "storage"
	                  ? pendingMoveItem
	                  : storageMode === "takeBack"
	                    ? (visibleStorageItems[0] ?? null)
	                    : null;
	              if (storageMode === "takeBack" && takeBackItem) {
	                onTakeBackItem(
	                  {
	                    uniqueId: takeBackItem.uniqueId,
	                    slot: takeBackItem.slot,
	                    container: takeBackItem.container,
	                  },
	                  slotIndex,
	                );
	                setDeleteFeedback(`${t("ui.takeBackItem", [], "Take Back")}: ${takeBackItem.name} -> ${slot.key}`);
	                setPendingMoveItem(null);
	                return;
	              }
              if (!pendingMoveItem) return;
              if (pendingMoveItem.slot === slotIndex && pendingMoveItem.container === activeTab) {
                setPendingMoveItem(null);
                return;
              }
              onMoveItem(
                {
                  uniqueId: pendingMoveItem.uniqueId,
                  slot: pendingMoveItem.slot,
                  container: pendingMoveItem.container,
                },
                slotIndex,
              );
              setDeleteFeedback(`${t("ui.inventory")}: ${pendingMoveItem.name} -> ${slot.key}`);
              setPendingMoveItem(null);
            }}
          />
        ))}
        {visibleItems.map((item) => {
          const slot = ORIGINAL_UI.inventory.slots[item.slot];
          if (!slot) return null;

          return (
            <button
              key={`${item.container}-${item.slot}-${item.uniqueId}-${item.key}`}
              type="button"
              className="inventory-item-card"
              style={{ left: slot.x, top: slot.y }}
              title={item.name}
              onMouseDown={(event) => {
                if (event.button !== 0) return;
                event.preventDefault();
                activateInventoryItem(item);
              }}
              onClick={(event) => {
                if (event.detail !== 0) return;
                activateInventoryItem(item);
              }}
	              onContextMenu={(event) => {
	                event.preventDefault();
	                if (storageMode === "store" && item.container !== "storage") {
	                  setPendingMoveItem(item);
	                  setPendingSplitItem(null);
	                  setDeleteFeedback(`${t("ui.storeItem", [], "Store Item")}: ${item.name}`);
	                  return;
	                }
	                if (storageMode === "takeBack" && item.container === "storage") {
	                  setPendingMoveItem(item);
	                  setPendingSplitItem(null);
	                  setDeleteFeedback(`${t("ui.takeBackItem", [], "Take Back")}: ${item.name}`);
	                  return;
	                }
	                if (item.quantity > 1) {
	                  setPendingSplitItem(item);
	                  setSplitCount("1");
                  setPendingMoveItem(null);
                  return;
                }
                setPendingMoveItem(item);
                setPendingSplitItem(null);
                setDeleteFeedback(`${t("ui.inventory")}: ${item.name}`);
              }}
            >
              <img
                className="original-item-icon inventory-item-icon"
                src={originalItemIconPath(item.icon)}
                alt=""
                draggable={false}
              />
              {item.quantity > 1 ? <span className="item-stack-count inventory-item-count">{item.quantity}</span> : null}
            </button>
          );
        })}
      </div>

      {activeTab === "quest" ? (
        <div className="inventory-quest-log">
          {storageLocked ? (
            <div className="inventory-delete-hint">
              {t("ui.storageLocked", [], "Storage is locked. Unlock to access warehouse items.")}
            </div>
          ) : storageMode ? (
            <div className="inventory-delete-hint">
              {storageMode === "store"
                ? t("ui.storageStoreHint", [], "Choose an inventory item, then click a warehouse slot.")
                : t("ui.storageTakeBackHint", [], "Choose a warehouse item, then click an inventory slot.")}
            </div>
          ) : (
            world.questLog.map((quest) => (
              <div
                key={quest.questId}
                className={`inventory-quest-entry ${quest.stage}`}
                data-quest-id={quest.questId}
                data-quest-stage={quest.stage}
              >
                <div className="inventory-quest-entry-head">
                  <strong>{quest.title}</strong>
                  <span>{quest.stage}</span>
                </div>
                <span className="inventory-quest-summary">{quest.summary}</span>
                <span className="inventory-quest-objective">{quest.objective}</span>
                <div className="inventory-quest-progress-row">
                  <span>{quest.progressLabel}</span>
                  <span>{quest.rewardPreview}</span>
                </div>
                <span
                  className="inventory-quest-progress-fill"
                  style={{ width: `${Math.min(100, Math.max(0, (quest.current / Math.max(quest.required, 1)) * 100))}%` }}
                />
              </div>
            ))
          )}
        </div>
      ) : null}

      <img className="inventory-weight-bar" src={ORIGINAL_UI.inventory.weightBar} alt="" draggable={false} />
      <button
        type="button"
        className="inventory-gold"
        aria-label={t("ui.dropGold", [], "Drop Gold")}
        title={t("ui.dropGold", [], "Drop Gold")}
        onClick={() => {
          setPendingGoldDrop(true);
          setDeleteMode(false);
          setSellMode(false);
          setStorageMode(null);
          setPendingDeleteItem(null);
          setPendingSellItem(null);
          setPendingSplitItem(null);
          setDeleteFeedback(null);
        }}
      >
        {world.gold}
      </button>
      <div className="inventory-weight">{world.freeBagSlots}</div>
      {deleteMode ? <div className="inventory-delete-hint">{`${t("ui.deleteItem")}...`}</div> : null}
      {showStorageWindow ? (
        <div className="window-shell storage-window">
          <div className="storage-window-title">{t("ui.storageWindow", [], "Storage")}</div>
          <div className="storage-window-subtitle">{`${storagePageText}  ${storageCapacityText}`}</div>
          <div className="storage-window-page-tabs">
            <button
              type="button"
              className={`storage-page-tab page-1 ${storagePageIndex === 0 ? "active" : ""}`}
              onClick={() => {
                setStoragePageIndex(0);
                setShowStoragePasswordPanel(false);
              }}
            >
              {t("ui.storagePageOneShort", [], "1")}
            </button>
            <button
              type="button"
              className={`storage-page-tab page-2 ${storagePageIndex === 1 ? "active" : ""}`}
              onClick={() => {
                setStoragePageIndex(1);
                setShowStoragePasswordPanel(false);
              }}
            >
              {t("ui.storagePageTwoShort", [], "2")}
            </button>
          </div>
	          <button
	            type="button"
	            className={`storage-action-button take-back ${storageMode === "takeBack" ? "active" : ""}`}
	            aria-label={t("ui.takeBackItem", [], "Take Back")}
	            title={t("ui.takeBackItem", [], "Take Back")}
	            onClick={() => {
	              const firstStorageItem = visibleStorageItems[0] ?? null;
	              setStorageMode("takeBack");
	              setSellMode(false);
	              setDeleteMode(false);
	              setPendingMoveItem(firstStorageItem);
	              setPendingSplitItem(null);
	              setPendingDeleteItem(null);
	              setPendingSellItem(null);
	              setPendingGoldDrop(false);
	              setShowStoragePasswordPanel(false);
	              setDeleteFeedback(
	                firstStorageItem ? `${t("ui.takeBackItem", [], "Take Back")}: ${firstStorageItem.name}` : null,
	              );
	              onTabChange("quest");
	            }}
	          >
	            {t("ui.takeBack", [], "Take Back")}
	          </button>
	          <button
	            type="button"
	            className={`storage-action-button store ${storageMode === "store" ? "active" : ""}`}
	            aria-label={t("ui.storeItem", [], "Store Item")}
	            title={t("ui.storeItem", [], "Store Item")}
	            onClick={() => {
	              setStorageMode("store");
	              setSellMode(false);
	              setDeleteMode(false);
	              setPendingMoveItem(null);
	              setPendingSplitItem(null);
	              setPendingDeleteItem(null);
	              setPendingSellItem(null);
	              setPendingGoldDrop(false);
	              setShowStoragePasswordPanel(false);
	              onTabChange("bag1");
	            }}
	          >
	            {t("ui.store", [], "Store")}
	          </button>
	          <button
	            type="button"
	            className="storage-action-button rent"
	            onClick={() => {
	              setStoragePageIndex(1);
              onRentExpandedStorage();
              setDeleteFeedback(
                world.hasExpandedStorage
                  ? t("ui.expandedStorageEnabled", [], "Expanded storage is active.")
                  : t("ui.expandedStorageRentRequested", [], "Requested expanded storage rental."),
              );
            }}
          >
            {t("ui.storageRent", [], "Rent")}
          </button>
          <button
            type="button"
            className={`storage-action-button protect ${effectiveShowStoragePasswordPanel ? "active" : ""}`}
            onClick={() => {
              if (showStoragePasswordPanel) {
                closeStoragePasswordPanel();
                return;
              }
              openStoragePasswordPanel();
            }}
            disabled={!storageProtectEnabled}
          >
            {t("ui.storageProtect", [], "Protect")}
          </button>
          <div className="storage-close">
            <SpriteButton sprite={ORIGINAL_UI.inventory.closeButton} label={t("ui.closeInventory")} onClick={closeStorageWindow} />
          </div>
          <div className={`storage-grid ${storagePageLocked ? "locked" : ""}`}>
            {ORIGINAL_UI.storage.slots.map((slot, slotIndex) => {
              const absoluteSlot = storagePageStart + slotIndex;
              return (
	                <button
	                  key={slot.key}
	                  type="button"
	                  className="storage-slot"
	                  style={{ left: slot.x, top: slot.y }}
	                  onClick={() => {
	                    const slotItem = visibleStorageItems.find((entry) => entry.slot === absoluteSlot);
	                    if (slotItem && storageMode === "takeBack" && !storageLocked && !storagePageLocked) {
	                      setPendingMoveItem(slotItem);
	                      setPendingSplitItem(null);
	                      setDeleteFeedback(`${t("ui.takeBackItem", [], "Take Back")}: ${slotItem.name}`);
	                      return;
	                    }
	                    if (
	                      slotItem &&
	                      storageMode === null &&
	                      !pendingMoveItem &&
	                      !storageLocked &&
	                      !storagePageLocked
	                    ) {
	                      setPendingMoveItem(slotItem);
	                      setPendingSplitItem(null);
	                      setDeleteFeedback(`${t("ui.storageMode", [], "Storage items")}: ${slotItem.name}`);
	                      return;
	                    }
	                    if (
	                      storageMode === "store" &&
                      pendingMoveItem &&
                      pendingMoveItem.container !== "storage" &&
                      !storageLocked &&
                      !storagePageLocked
                    ) {
                      onStoreItem(
                        {
                          uniqueId: pendingMoveItem.uniqueId,
                          slot: pendingMoveItem.slot,
                          container: pendingMoveItem.container,
                        },
                        absoluteSlot,
                      );
                      setDeleteFeedback(
                        `${t("ui.storeItem", [], "Store Item")}: ${pendingMoveItem.name} -> ${absoluteSlot + 1}`,
                      );
                      setPendingMoveItem(null);
                      return;
                    }

                    if (
                      storageMode === null &&
                      pendingMoveItem &&
                      pendingMoveItem.container === "storage" &&
                      !storageLocked &&
                      !storagePageLocked
                    ) {
                      if (pendingMoveItem.slot === absoluteSlot) {
                        setPendingMoveItem(null);
                        return;
                      }
                      onMoveItem(
                        {
                          uniqueId: pendingMoveItem.uniqueId,
                          slot: pendingMoveItem.slot,
                          container: pendingMoveItem.container,
                        },
                        absoluteSlot,
                      );
                      setDeleteFeedback(
                        `${t("ui.storageMode", [], "Storage items")}: ${pendingMoveItem.name} -> ${absoluteSlot + 1}`,
                      );
                      setPendingMoveItem(null);
                    }
                  }}
	                />
              );
            })}
            {visibleStorageItems.map((item) => {
              const slot = ORIGINAL_UI.storage.slots[item.slot - storagePageStart];
              if (!slot) return null;

              return (
	                      <button
	                        key={`storage-${item.container}-${item.slot}-${item.uniqueId}-${item.key}`}
                  type="button"
                  className={`storage-item-card ${
                    pendingMoveItem?.container === "storage" && pendingMoveItem.slot === item.slot
                      ? "selected"
                      : ""
                  }`}
                  style={{ left: slot.x, top: slot.y }}
                  title={item.name}
	                  onClick={() => {
	                    if (storageLocked || storagePageLocked) {
	                      return;
	                    }

	                    setPendingMoveItem(item);
	                    setPendingSplitItem(null);
	                    setDeleteFeedback(
	                      `${
	                        storageMode === "takeBack"
	                          ? t("ui.takeBackItem", [], "Take Back")
	                          : t("ui.storageMode", [], "Storage items")
	                      }: ${item.name}`,
	                    );
	                  }}
	                  onContextMenu={(event) => {
	                    event.preventDefault();
	                    if (storageLocked || storagePageLocked) {
	                      return;
	                    }
	                    if (storageMode === "takeBack") {
	                      setPendingMoveItem(item);
	                      setPendingSplitItem(null);
	                      setDeleteFeedback(`${t("ui.takeBackItem", [], "Take Back")}: ${item.name}`);
	                      return;
	                    }
	                    if (storageMode !== null) {
	                      return;
	                    }
	                    if (item.quantity > 1) {
	                      setPendingSplitItem(item);
                      setSplitCount("1");
                      setPendingMoveItem(null);
                      return;
                    }
                    setPendingMoveItem(item);
                    setPendingSplitItem(null);
                    setDeleteFeedback(`${t("ui.storageMode", [], "Storage items")}: ${item.name}`);
                  }}
                >
                  <img
                    className="original-item-icon storage-item-icon"
                    src={originalItemIconPath(item.icon)}
                    alt=""
                    draggable={false}
                  />
                  {item.quantity > 1 ? (
                    <span className="item-stack-count storage-item-count">{item.quantity}</span>
                  ) : null}
                </button>
              );
            })}
            {storagePageLocked ? (
              <div className="storage-page-locked">
                <strong>{t("ui.expandedStorageLocked", [], "Expanded storage is locked.")}</strong>
                <span>{t("ui.expandedStorageRentHint", [], "Enable expanded storage to use page 2.")}</span>
              </div>
            ) : null}
          </div>
          {effectiveShowStoragePasswordPanel ? (
            <div className="inventory-storage-panel storage-password-panel">
              <strong>{storagePasswordPanelTitle}</strong>
              <span>{storagePasswordPanelPrompt}</span>
              <span>{storageStatusText}</span>
              {storagePasswordLastSetText ? <span>{storagePasswordLastSetText}</span> : null}
              {effectiveStoragePasswordPanelMode !== "set" ? (
                <input
                  type="password"
                  value={storagePassword}
                  placeholder={t("client.StoragePasswordCurrentPrompt", [], "Current password")}
                  onChange={(event) => setStoragePassword(event.target.value)}
                />
              ) : null}
              {effectiveStoragePasswordPanelMode === "set" || effectiveStoragePasswordPanelMode === "change" ? (
                <>
                  <input
                    type="password"
                    value={newStoragePassword}
                    placeholder={t("client.StoragePasswordNewPrompt", [], "New password")}
                    onChange={(event) => setNewStoragePassword(event.target.value)}
                  />
                  <input
                    type="password"
                    value={confirmStoragePassword}
                    placeholder={t("client.StoragePasswordConfirmPrompt", [], "Confirm password")}
                    onChange={(event) => setConfirmStoragePassword(event.target.value)}
                  />
                  <span className={passwordConfirmationMismatch ? "storage-password-warning" : ""}>
                    {passwordConfirmationMismatch
                      ? t("client.StoragePasswordMismatch", [], "Storage password confirmation does not match.")
                      : storagePasswordRules}
                  </span>
                </>
              ) : null}
              {world.hasStoragePassword && !storageLocked ? (
                <div className="storage-password-mode-row">
                  <button
                    type="button"
                    className={storagePasswordPanelMode === "change" ? "active" : ""}
                    onClick={() => {
                      setStoragePasswordPanelMode("change");
                      setNewStoragePassword("");
                      setConfirmStoragePassword("");
                    }}
                  >
                    {t("ui.changePassword", [], "Change")}
                  </button>
                  <button
                    type="button"
                    className={storagePasswordPanelMode === "remove" ? "active" : ""}
                    onClick={() => {
                      setStoragePasswordPanelMode("remove");
                      setNewStoragePassword("");
                      setConfirmStoragePassword("");
                    }}
                  >
                    {t("ui.removePassword", [], "Remove")}
                  </button>
                </div>
              ) : null}
              <div className="inventory-delete-actions">
                <button
                  type="button"
                  onClick={submitStoragePasswordPanel}
                  disabled={!storagePasswordPanelCanSubmit()}
                >
                  {effectiveStoragePasswordPanelMode === "unlock"
                    ? t("ui.unlock", [], "Unlock")
                    : effectiveStoragePasswordPanelMode === "remove"
                      ? t("ui.removePassword", [], "Remove")
                      : t("ui.ok", [], "OK")}
                </button>
                <button type="button" onClick={closeStoragePasswordPanel}>
                  {t("ui.cancel", [], "Cancel")}
                </button>
              </div>
            </div>
          ) : null}
          {storageStatusLabelText ? (
            <div className={`storage-window-status ${storageLocked ? "locked" : ""}`}>{storageStatusLabelText}</div>
          ) : null}
          {storageRentalText ? (
            <div className={`storage-window-rental ${world.hasExpandedStorage ? "" : "locked"}`}>{storageRentalText}</div>
          ) : null}
        </div>
      ) : null}
      {pendingDeleteItem ? (
        <div className="inventory-delete-panel">
          <strong>{t("ui.deleteItem")}</strong>
          <div className="inventory-delete-preview">
            <img
              className="original-item-icon inventory-delete-icon"
              src={originalItemIconPath(pendingDeleteItem.icon)}
              alt=""
              draggable={false}
            />
          </div>
          <span>{pendingDeleteItem.name}</span>
          <div className="inventory-delete-actions">
            <button
              type="button"
              onMouseDown={(event) => primaryMouseAction(event, () => confirmDeleteItem(pendingDeleteItem))}
              onClick={(event) => {
                if (event.detail !== 0) return;
                confirmDeleteItem(pendingDeleteItem);
              }}
            >
              {t("ui.confirm", [], "Confirm")}
            </button>
            <button type="button" onClick={() => setPendingDeleteItem(null)}>
              {t("ui.close")}
            </button>
          </div>
        </div>
      ) : null}
      {pendingSellItem ? (
        <div className="inventory-delete-panel">
          <strong>{t("ui.sellItem", [], "Sell Item")}</strong>
          <div className="inventory-delete-preview">
            <img
              className="original-item-icon inventory-delete-icon"
              src={originalItemIconPath(pendingSellItem.icon)}
              alt=""
              draggable={false}
            />
          </div>
          <span>{pendingSellItem.name}</span>
          <div className="inventory-delete-actions">
            <button
              type="button"
              onMouseDown={(event) => primaryMouseAction(event, () => confirmSellItem(pendingSellItem))}
              onClick={(event) => {
                if (event.detail !== 0) return;
                confirmSellItem(pendingSellItem);
              }}
            >
              {t("ui.confirm", [], "Confirm")}
            </button>
            <button type="button" onClick={() => setPendingSellItem(null)}>
              {t("ui.close")}
            </button>
          </div>
        </div>
      ) : null}
      {pendingSplitItem ? (
        <div className="inventory-delete-panel">
          <strong>{t("ui.splitItem", [], "Split Item")}</strong>
          <div className="inventory-delete-preview">
            <img
              className="original-item-icon inventory-delete-icon"
              src={originalItemIconPath(pendingSplitItem.icon)}
              alt=""
              draggable={false}
            />
          </div>
          <span>{pendingSplitItem.name}</span>
          <input
            type="number"
            min="1"
            max={Math.max(1, pendingSplitItem.quantity - 1)}
            value={splitCount}
            onChange={(event) => setSplitCount(event.target.value)}
          />
          <div className="inventory-delete-actions">
            <button
              type="button"
              onMouseDown={(event) => primaryMouseAction(event, () => confirmSplitItem(pendingSplitItem))}
              onClick={(event) => {
                if (event.detail !== 0) return;
                confirmSplitItem(pendingSplitItem);
              }}
            >
              {t("ui.confirm", [], "Confirm")}
            </button>
            <button type="button" onClick={() => setPendingSplitItem(null)}>
              {t("ui.close")}
            </button>
          </div>
        </div>
      ) : null}
      {pendingGoldDrop ? (
        <div className="inventory-delete-panel">
          <strong>{t("ui.dropGold", [], "Drop Gold")}</strong>
          <input
            type="number"
            min="1"
            value={goldDropAmount}
            onChange={(event) => setGoldDropAmount(event.target.value)}
          />
          <div className="inventory-delete-actions">
            <button
              type="button"
              onMouseDown={(event) => primaryMouseAction(event, confirmGoldDrop)}
              onClick={(event) => {
                if (event.detail !== 0) return;
                confirmGoldDrop();
              }}
            >
              {t("ui.confirm", [], "Confirm")}
            </button>
            <button type="button" onClick={() => setPendingGoldDrop(false)}>
              {t("ui.close")}
            </button>
          </div>
        </div>
      ) : null}
      {deleteFeedback ? <div className="inventory-delete-feedback">{deleteFeedback}</div> : null}
    </div>
  );
}

type CharacterWindowProps = {
  t: TranslateFn;
  activeTab: CharacterTabKey;
  onClose: () => void;
  onTabChange: (tab: CharacterTabKey) => void;
  player: DisplayEntity | null;
  world: DisplayWorld;
  onRemoveItem: (item: EquipmentActionRef) => void;
  onRepairItem: (item: EquipmentActionRef) => void;
  onSpecialRepairItem: (item: EquipmentActionRef) => void;
  onCastSkill: (skillKey: string) => void;
};

export function CharacterWindow({
  t,
  activeTab,
  onClose,
  onTabChange,
  player,
  world,
  onRemoveItem,
  onRepairItem,
  onSpecialRepairItem,
  onCastSkill,
}: CharacterWindowProps) {
  const activePage = ORIGINAL_UI.character.pages[activeTab];
  const equipmentBySlot = new Map(world.equipmentItems.map((item) => [item.slot, item]));
  const totalAttack = world.equipmentItems.reduce((sum, item) => sum + item.attack, 0);
  const totalDefence = world.equipmentItems.reduce((sum, item) => sum + item.defence, 0);
  const [repairMode, setRepairMode] = useState<"normal" | "special" | null>(null);
  const repairModeLabel =
    repairMode === "normal"
      ? t("ui.repairItem", [], "Repair Item")
      : repairMode === "special"
        ? t("ui.specialRepairItem", [], "Special Repair")
        : "";
  const stats1Values = [
    displayFieldValue(world.playerHp, world.playerMaxHp),
    displayFieldValue(world.playerMp, 100),
    statNumber(totalDefence),
    "",
    statNumber(totalAttack),
    "",
    "",
    "",
    "",
    "",
    "",
  ];
  const stats2Values = [
    `${(ratio(world.playerExperience, world.playerMaxExperience) * 100).toFixed(2)}%`,
    statPair(world.freeBagSlots, world.maxBagSlots),
    statPair(world.currentWeight, world.maxWeight),
    "",
    "",
    "",
    "",
    "",
    "",
    "",
    "",
  ];
  const spellValues = [
    ...world.knownSkills.slice(0, 7).map((skill) => {
      const cooldownSuffix =
        skill.cooldownRemainingTicks > 0 ? ` (${skill.cooldownRemainingTicks})` : "";
      return `${skill.name}${cooldownSuffix}`;
    }),
  ];
  while (spellValues.length < 7) {
    spellValues.push("");
  }

  return (
    <div className="window-shell character-window">
      <img className="window-frame" src={ORIGINAL_UI.character.frame} alt="" draggable={false} />
      <img className="character-page" src={activePage} alt="" draggable={false} />
      <div className="character-close">
        <SpriteButton sprite={ORIGINAL_UI.character.closeButton} label={t("ui.closeCharacter")} onClick={onClose} />
      </div>

      <div className="character-tab char"><button type="button" className="window-tab-button" onClick={() => onTabChange("char")}><img src={ORIGINAL_UI.character.tabs.char} alt={t("ui.character")} draggable={false} /></button></div>
      <div className="character-tab stats1"><button type="button" className="window-tab-button" onClick={() => onTabChange("stats1")}><img src={ORIGINAL_UI.character.tabs.stats1} alt={t("ui.statsPrimary")} draggable={false} /></button></div>
      <div className="character-tab stats2"><button type="button" className="window-tab-button" onClick={() => onTabChange("stats2")}><img src={ORIGINAL_UI.character.tabs.stats2} alt={t("ui.statsSecondary")} draggable={false} /></button></div>
      <div className="character-tab spells"><button type="button" className="window-tab-button" onClick={() => onTabChange("spells")}><img src={ORIGINAL_UI.character.tabs.spells} alt={t("ui.spells")} draggable={false} /></button></div>

      <div className="character-name">{player?.name ?? ""}</div>
      <div className="character-guild" />

      {activeTab === "char" ? (
        <>
          {ORIGINAL_UI.character.equipmentSlots.map((slot) => {
            const item = equipmentBySlot.get(equipmentSlotFromLabel(slot.label));

            return (
              <div
                key={slot.label}
                className="character-slot"
                style={{ left: slot.x + 8, top: slot.y + 90 }}
                title={slot.label}
              >
	                {item ? (
	                  <button
	                    type="button"
	                    className="character-slot-card"
	                    title={item.name}
	                    onClick={() => {
	                      if (repairMode === "normal") {
	                        onRepairItem({ slot: item.slot });
	                        setRepairMode(null);
	                        return;
	                      }
	                      if (repairMode === "special") {
	                        onSpecialRepairItem({ slot: item.slot });
	                        setRepairMode(null);
	                        return;
	                      }
	                      onRemoveItem({ slot: item.slot });
	                    }}
	                  >
                    <img
                      className="original-item-icon character-item-icon"
                      src={originalItemIconPath(item.icon)}
                      alt=""
                      draggable={false}
                    />
                  </button>
                ) : null}
              </div>
            );
          })}
	        </>
	      ) : null}

	      {activeTab === "char" ? (
	        <>
	          {repairModeLabel ? <div className="inventory-delete-hint">{repairModeLabel}</div> : null}
	          <div className="character-repair-actions">
	            <button
	              type="button"
	              className={repairMode === "normal" ? "active" : ""}
	              onClick={() => setRepairMode((current) => (current === "normal" ? null : "normal"))}
	            >
	              {t("ui.repairItem", [], "Repair Item")}
	            </button>
	            <button
	              type="button"
	              className={repairMode === "special" ? "active" : ""}
	              onClick={() => setRepairMode((current) => (current === "special" ? null : "special"))}
	            >
	              {t("ui.specialRepairItem", [], "Special Repair")}
	            </button>
	          </div>
	        </>
	      ) : null}

      {activeTab === "stats1" ? (
        <div className="character-field-values stats1">
          {stats1Values.map((value, index) => (
            <div key={`stats1-${index}`} className="character-field-value">
              {value}
            </div>
          ))}
        </div>
      ) : null}

      {activeTab === "stats2" ? (
        <div className="character-field-values stats2">
          {stats2Values.map((value, index) => (
            <div key={`stats2-${index}`} className="character-field-value">
              {value}
            </div>
          ))}
        </div>
      ) : null}

      {activeTab === "spells" ? (
        <div className="character-spell-values">
          {spellValues.map((value, index) => {
            const skill = world.knownSkills[index] ?? null;
            if (skill) {
              return (
                <button
                  key={`spells-${skill.key}`}
                  type="button"
                  className="character-spell-value"
                  title={skill.description}
                  onClick={() => onCastSkill(skill.key)}
                >
                  {value}
                </button>
              );
            }

            return (
              <div key={`spells-${index}`} className="character-spell-value">
                {value}
              </div>
            );
          })}
        </div>
      ) : null}
    </div>
  );
}

type SpriteButtonProps = {
  sprite: SpriteState;
  label: string;
  onClick: () => void;
  active?: boolean;
};

function SpriteButton({ sprite, label, onClick, active = false }: SpriteButtonProps) {
  const [hovered, setHovered] = useState(false);
  const [pressed, setPressed] = useState(false);

  let source = sprite.base;
  if (pressed && sprite.pressed) {
    source = sprite.pressed;
  } else if (active && sprite.active) {
    source = sprite.active;
  } else if ((hovered || active) && sprite.hover) {
    source = sprite.hover;
  }

  return (
    <button
      type="button"
      className="sprite-button"
      onClick={onClick}
      onMouseEnter={() => setHovered(true)}
      onMouseLeave={() => {
        setHovered(false);
        setPressed(false);
      }}
      onMouseDown={() => setPressed(true)}
      onMouseUp={() => setPressed(false)}
      aria-label={label}
      title={label}
    >
      <img src={source} alt="" draggable={false} />
    </button>
  );
}

type ViewportSpriteLayer = Pick<
  OriginalSceneSpriteFrameMeta,
  "path" | "width" | "height" | "x" | "y"
>;

type ViewportEntitySprite = {
  rearWeapons: ViewportSpriteLayer[];
  body: ViewportSpriteLayer | null;
  hair: ViewportSpriteLayer | null;
  frontWeapons: ViewportSpriteLayer[];
  nameplateTop: number;
};

type QuestIconKey =
  | "questionWhite"
  | "exclamationYellow"
  | "questionYellow"
  | "exclamationGreen"
  | "questionGreen";

type ViewportSpriteAnimationMeta = {
  frameBaseOffset: number;
  weaponFrameOffset: number | null;
  frameCount: number;
  directionStride: number;
  frameIntervalMs?: number;
  reverse?: boolean;
};

function buildViewportEntitySprite(
  entity: DisplayEntity,
  libraries: Record<string, OriginalSceneSpriteLibraryMeta>,
  sceneFrameIndex: number,
  now: number,
  animationState: EntitySpriteAnimationState,
  motionSnapshot?: EntityMotionSnapshot,
): ViewportEntitySprite | null {
  const sprite = resolvedEntitySprite(entity, libraries, animationState);
  if (!sprite) {
    return null;
  }

  const animation = spriteAnimationMetaForEntity(entity, sprite, animationState);
  if (!animation) {
    return null;
  }

  const frameCycle = spriteFrameCycleForEntity(
    entity,
    sceneFrameIndex,
    now,
    animationState,
    animation,
    motionSnapshot,
  );
  const frameIndex =
    animation.frameBaseOffset +
    directionIndex(entity.direction) * animation.directionStride +
    frameCycle;
  const bodyLibraryKey = normalizeSceneSpriteLibraryKey(sprite.bodyLibrary);
  const hairLibraryKey = sprite.hairLibrary
    ? normalizeSceneSpriteLibraryKey(sprite.hairLibrary)
    : null;
  const weaponLibraryKey = sprite.weaponLibrary
    ? normalizeSceneSpriteLibraryKey(sprite.weaponLibrary)
    : null;
  const secondaryWeaponLibraryKey = sprite.weaponLibrarySecondary
    ? normalizeSceneSpriteLibraryKey(sprite.weaponLibrarySecondary)
    : null;
  const fallbackFrameIndex =
    sprite.frameBaseOffset +
    directionIndex(entity.direction) * Math.max(sprite.directionStride, 1);
  const bodyFrame = frameMetaForIndexWithFallback(libraries[bodyLibraryKey], frameIndex, fallbackFrameIndex);
  const hairFrame = hairLibraryKey
    ? frameMetaForIndexWithFallback(libraries[hairLibraryKey], frameIndex, fallbackFrameIndex)
    : null;
  const weaponFrameIndex =
    animation.weaponFrameOffset === null
      ? null
      : animation.weaponFrameOffset + directionIndex(entity.direction) * animation.directionStride + frameCycle;
  const fallbackWeaponFrameIndex =
    sprite.weaponFrameOffset === undefined || sprite.weaponFrameOffset === null
      ? null
      : sprite.weaponFrameOffset + directionIndex(entity.direction) * Math.max(sprite.directionStride, 1);
  const primaryWeaponFrame =
    weaponLibraryKey && weaponFrameIndex !== null
      ? frameMetaForIndexWithFallback(libraries[weaponLibraryKey], weaponFrameIndex, fallbackWeaponFrameIndex)
      : null;
  const secondaryWeaponFrame =
    secondaryWeaponLibraryKey && weaponFrameIndex !== null
      ? frameMetaForIndexWithFallback(libraries[secondaryWeaponLibraryKey], weaponFrameIndex, fallbackWeaponFrameIndex)
      : null;
  const weaponPlacement = weaponPlacementForDirection(entity.direction);
  const classKey = entityClassKey(entity);
  const primaryWeaponLayer = viewportSpriteLayer(primaryWeaponFrame);
  const secondaryWeaponLayer = viewportSpriteLayer(secondaryWeaponFrame);
  const rearWeapons =
    classKey === "assassin"
      ? assassinRearWeaponsForDirection(entity.direction, primaryWeaponLayer, secondaryWeaponLayer)
      : weaponPlacement === "rear"
        ? [primaryWeaponLayer, secondaryWeaponLayer].filter((layer): layer is ViewportSpriteLayer => Boolean(layer))
        : [];
  const frontWeapons =
    classKey === "assassin"
      ? assassinFrontWeaponsForDirection(entity.direction, primaryWeaponLayer, secondaryWeaponLayer)
      : weaponPlacement === "front"
        ? [primaryWeaponLayer, secondaryWeaponLayer].filter((layer): layer is ViewportSpriteLayer => Boolean(layer))
        : [];

  return {
    rearWeapons,
    body: viewportSpriteLayer(bodyFrame),
    hair: viewportSpriteLayer(hairFrame),
    frontWeapons,
    nameplateTop: computeNameplateTop(bodyFrame, hairFrame, primaryWeaponFrame ?? secondaryWeaponFrame),
  };
}

function entityClassKey(entity: DisplayEntity): EntityClassKey {
  return entity.classKey ?? "warrior";
}

function entityGenderKey(entity: DisplayEntity): EntityGenderKey {
  return entity.genderKey ?? "male";
}

function sceneLibraryExists(
  libraries: Record<string, OriginalSceneSpriteLibraryMeta>,
  libraryKey: string | null | undefined,
) {
  if (!libraryKey) {
    return false;
  }

  return normalizeSceneSpriteLibraryKey(libraryKey) in libraries;
}

function resolvedEntitySprite(
  entity: DisplayEntity,
  libraries: Record<string, OriginalSceneSpriteLibraryMeta>,
  animationState: EntitySpriteAnimationState,
): EntitySprite | null {
  const sprite = entity.sprite;
  if (!sprite) {
    return null;
  }

  if (entity.kind === "monster" || entity.kind === "npc") {
    return sprite;
  }

  const classKey = entityClassKey(entity);
  const genderKey = entityGenderKey(entity);
  const bodyBaseOffset = genderKey === "female" ? 808 : 0;
  const weaponBaseOffset = genderKey === "female" ? 416 : 0;
  const spriteBodyLibrary = sprite.bodyLibrary;
  const spriteHairLibrary = sprite.hairLibrary;
  const spriteWeaponLibrary = sprite.weaponLibrary;
  const spriteWeaponLibrarySecondary = sprite.weaponLibrarySecondary;
  const spriteAltBodyLibrary = sprite.altBodyLibrary;
  const spriteAltHairLibrary = sprite.altHairLibrary;
  const spriteAltWeaponLibrary = sprite.altWeaponLibrary;
  const spriteAltWeaponLibrarySecondary = sprite.altWeaponLibrarySecondary;
  const isArcherAlt = Boolean(spriteAltBodyLibrary?.startsWith("ARArmour/"));
  const isAssassinAlt = Boolean(spriteAltBodyLibrary?.startsWith("AArmour/"));

  if (
    isArcherAlt &&
    (animationState === "walking" || animationState === "running" || animationState === "attackRange")
  ) {
    const altWeaponLibrary =
      spriteAltWeaponLibrary && sceneLibraryExists(libraries, spriteAltWeaponLibrary)
        ? spriteAltWeaponLibrary
        : sprite.weaponLibrary;
    return {
      ...sprite,
      bodyLibrary:
        spriteAltBodyLibrary && sceneLibraryExists(libraries, spriteAltBodyLibrary)
          ? spriteAltBodyLibrary
          : sprite.bodyLibrary,
      hairLibrary:
        spriteAltHairLibrary && sceneLibraryExists(libraries, spriteAltHairLibrary)
          ? spriteAltHairLibrary
          : sprite.hairLibrary,
      weaponLibrary: altWeaponLibrary,
      weaponLibrarySecondary: null,
      frameBaseOffset:
        sprite.altFrameBaseOffset !== undefined && sprite.altFrameBaseOffset !== null
          ? sprite.altFrameBaseOffset
          : bodyBaseOffset,
      weaponFrameOffset:
        sprite.altWeaponFrameOffset !== undefined && sprite.altWeaponFrameOffset !== null
          ? sprite.altWeaponFrameOffset
          : sprite.weaponFrameOffset,
    };
  }

  if (
    isAssassinAlt &&
    (animationState === "standing" ||
      animationState === "walking" ||
      animationState === "running" ||
      animationState === "attackMelee" ||
      animationState === "struck" ||
      animationState === "dying" ||
      animationState === "dead" ||
      animationState === "reviving")
  ) {
    return {
      ...sprite,
      bodyLibrary:
        spriteAltBodyLibrary && sceneLibraryExists(libraries, spriteAltBodyLibrary)
          ? spriteAltBodyLibrary
          : sprite.bodyLibrary,
      hairLibrary:
        spriteAltHairLibrary && sceneLibraryExists(libraries, spriteAltHairLibrary)
          ? spriteAltHairLibrary
          : sprite.hairLibrary,
      weaponLibrary:
        spriteAltWeaponLibrary && sceneLibraryExists(libraries, spriteAltWeaponLibrary)
          ? spriteAltWeaponLibrary
          : sprite.weaponLibrary,
      weaponLibrarySecondary:
        spriteAltWeaponLibrarySecondary && sceneLibraryExists(libraries, spriteAltWeaponLibrarySecondary)
          ? spriteAltWeaponLibrarySecondary
          : sprite.weaponLibrarySecondary,
      frameBaseOffset:
        sprite.altFrameBaseOffset !== undefined && sprite.altFrameBaseOffset !== null
          ? sprite.altFrameBaseOffset
          : bodyBaseOffset,
      weaponFrameOffset:
        sprite.altWeaponFrameOffset !== undefined && sprite.altWeaponFrameOffset !== null
          ? sprite.altWeaponFrameOffset
          : spriteAltWeaponLibrary && sceneLibraryExists(libraries, spriteAltWeaponLibrary)
            ? weaponBaseOffset
          : sprite.weaponFrameOffset,
    };
  }

  return sprite;
}

function viewportSpriteLayer(frame: OriginalSceneSpriteFrameMeta | null): ViewportSpriteLayer | null {
  if (!frame) {
    return null;
  }

  return {
    path: frame.path,
    width: frame.width,
    height: frame.height,
    x: frame.x,
    y: frame.y,
  };
}

function frameMetaForIndexWithFallback(
  library: OriginalSceneSpriteLibraryMeta | null | undefined,
  frameIndex: number,
  fallbackFrameIndex: number | null,
) {
  return (
    frameMetaForIndex(library, frameIndex) ??
    (fallbackFrameIndex === null ? null : frameMetaForIndex(library, fallbackFrameIndex)) ??
    library?.frames[0] ??
    null
  );
}

function spriteFrameCycleForEntity(
  entity: DisplayEntity,
  sceneFrameIndex: number,
  now: number,
  animationState: EntitySpriteAnimationState,
  animation: ViewportSpriteAnimationMeta,
  motionSnapshot?: EntityMotionSnapshot,
) {
  const frameCount = Math.max(animation.frameCount, 1);
  if (frameCount <= 1) {
    return 0;
  }

  const frameIntervalMs = animation.frameIntervalMs ?? 100;
  let cycle = sceneFrameIndex % frameCount;

  switch (animationState) {
    case "standing": {
      const standingStartedAt =
        motionSnapshot && motionSnapshot.expiresAt > 0 && motionSnapshot.expiresAt <= now
          ? motionSnapshot.expiresAt
          : 0;
      const phaseMs =
        standingStartedAt === 0 && entity.kind === "monster"
          ? stableEntityAnimationPhaseMs(entity, frameIntervalMs, frameCount)
          : 0;
      cycle = loopingFrameCycle(
        now + phaseMs,
        standingStartedAt,
        frameIntervalMs,
        frameCount,
      );
      break;
    }
    case "dead":
      cycle = 0;
      break;
    case "walking":
    case "running":
      cycle = transientFrameCycle(
        now,
        motionSnapshot?.startedAt ?? entity.movementStartedAt,
        frameIntervalMs,
        frameCount,
      );
      break;
    case "attackMelee":
    case "attackRange":
      cycle = transientFrameCycle(now, entity.attackStartedAt, frameIntervalMs, frameCount);
      break;
    case "struck":
      cycle = transientFrameCycle(now, entity.struckStartedAt, frameIntervalMs, frameCount);
      break;
    case "dying":
      cycle = transientFrameCycle(now, entity.dieStartedAt, frameIntervalMs, frameCount);
      break;
    case "reviving":
      cycle = transientFrameCycle(now, entity.reviveStartedAt, frameIntervalMs, frameCount);
      break;
    default:
      break;
  }

  return animation.reverse ? frameCount - 1 - cycle : cycle;
}

function loopingFrameCycle(now: number, startedAt: number, frameIntervalMs: number, frameCount: number) {
  if (frameCount <= 1) {
    return 0;
  }

  const elapsed = Math.max(now - startedAt, 0);
  return Math.floor(elapsed / Math.max(frameIntervalMs, 1)) % frameCount;
}

function stableEntityAnimationPhaseMs(entity: DisplayEntity, frameIntervalMs: number, frameCount: number) {
  const numericId = Number(entity.objectId);
  const seed = Number.isFinite(numericId)
    ? numericId
    : Array.from(entity.objectId).reduce((total, char) => total + char.charCodeAt(0), 0);
  return (Math.abs(seed) % Math.max(frameCount, 1)) * Math.max(frameIntervalMs, 1);
}

function spriteAnimationMetaForEntity(
  entity: DisplayEntity,
  sprite: EntitySprite,
  animationState: EntitySpriteAnimationState,
): ViewportSpriteAnimationMeta | null {
  if (entity.kind === "npc") {
    return {
      frameBaseOffset: sprite.frameBaseOffset,
      weaponFrameOffset:
        sprite.weaponFrameOffset === undefined || sprite.weaponFrameOffset === null
          ? null
          : sprite.weaponFrameOffset,
      frameCount: Math.max(sprite.frameCount, 1),
      directionStride: Math.max(sprite.directionStride, 1),
      frameIntervalMs: 450,
    };
  }

  if (entity.kind === "monster") {
    switch (animationState) {
      case "walking":
        return {
          frameBaseOffset: sprite.frameBaseOffset + 32,
          weaponFrameOffset: null,
          frameCount: 6,
          directionStride: 6,
          frameIntervalMs: 100,
        };
      case "attackMelee":
      case "attackRange":
        return {
          frameBaseOffset: sprite.frameBaseOffset + 80,
          weaponFrameOffset: null,
          frameCount: 6,
          directionStride: 6,
          frameIntervalMs: 100,
        };
      case "struck":
        return {
          frameBaseOffset: sprite.frameBaseOffset + 128,
          weaponFrameOffset: null,
          frameCount: 2,
          directionStride: 2,
          frameIntervalMs: 200,
        };
      case "dying":
        return {
          frameBaseOffset: sprite.frameBaseOffset + 144,
          weaponFrameOffset: null,
          frameCount: 10,
          directionStride: 10,
          frameIntervalMs: 100,
        };
      case "reviving":
        return {
          frameBaseOffset: sprite.frameBaseOffset + 144,
          weaponFrameOffset: null,
          frameCount: 10,
          directionStride: 10,
          frameIntervalMs: 100,
          reverse: true,
        };
      case "dead":
        return {
          frameBaseOffset: sprite.frameBaseOffset + 153,
          weaponFrameOffset: null,
          frameCount: 1,
          directionStride: 1,
        };
      default:
        return {
          frameBaseOffset: sprite.frameBaseOffset,
          weaponFrameOffset: null,
          frameCount: Math.max(sprite.frameCount, 1),
          directionStride: Math.max(sprite.directionStride, 1),
          frameIntervalMs: 500,
        };
    }
  }

  const archerAlt =
    Boolean(sprite.bodyLibrary.startsWith("ARArmour/")) &&
    (animationState === "walking" || animationState === "running" || animationState === "attackRange");

  switch (animationState) {
    case "walking":
      return {
        frameBaseOffset: sprite.frameBaseOffset + (archerAlt ? 0 : 32),
        weaponFrameOffset:
          sprite.weaponFrameOffset === undefined || sprite.weaponFrameOffset === null
            ? null
            : sprite.weaponFrameOffset + (archerAlt ? 0 : 32),
        frameCount: 6,
        directionStride: 6,
        frameIntervalMs: 100,
      };
    case "running":
      return {
        frameBaseOffset: sprite.frameBaseOffset + (archerAlt ? 48 : 80),
        weaponFrameOffset:
          sprite.weaponFrameOffset === undefined || sprite.weaponFrameOffset === null
            ? null
            : sprite.weaponFrameOffset + (archerAlt ? 48 : 112),
        frameCount: 6,
        directionStride: 6,
        frameIntervalMs: 100,
      };
    case "attackMelee":
      return {
        frameBaseOffset:
          entity.attackAnimation === "melee2"
            ? sprite.frameBaseOffset + 184
            : entity.attackAnimation === "melee3"
              ? sprite.frameBaseOffset + 232
              : entity.attackAnimation === "melee4"
                ? sprite.frameBaseOffset + 416
                : sprite.frameBaseOffset + 136,
        weaponFrameOffset:
          sprite.weaponFrameOffset === undefined || sprite.weaponFrameOffset === null
            ? null
            : entity.attackAnimation === "melee2"
              ? sprite.weaponFrameOffset + 216
              : entity.attackAnimation === "melee3"
                ? sprite.weaponFrameOffset + 264
                : entity.attackAnimation === "melee4"
                  ? sprite.weaponFrameOffset + 448
                  : sprite.weaponFrameOffset + 168,
        frameCount: entity.attackAnimation === "melee3" ? 8 : 6,
        directionStride: entity.attackAnimation === "melee3" ? 8 : 6,
        frameIntervalMs: 100,
      };
    case "attackRange":
      return {
        frameBaseOffset: sprite.frameBaseOffset + 96,
        weaponFrameOffset:
          sprite.weaponFrameOffset === undefined || sprite.weaponFrameOffset === null
            ? null
            : sprite.weaponFrameOffset + 96,
        frameCount: 8,
        directionStride: 8,
        frameIntervalMs: 100,
      };
    case "struck":
      return {
        frameBaseOffset: sprite.frameBaseOffset + 360,
        weaponFrameOffset:
          sprite.weaponFrameOffset === undefined || sprite.weaponFrameOffset === null
            ? null
            : sprite.weaponFrameOffset + 392,
        frameCount: 3,
        directionStride: 3,
        frameIntervalMs: 100,
      };
    case "dying":
      return {
        frameBaseOffset: sprite.frameBaseOffset + 384,
        weaponFrameOffset:
          sprite.weaponFrameOffset === undefined || sprite.weaponFrameOffset === null
            ? null
            : sprite.weaponFrameOffset + 416,
        frameCount: 4,
        directionStride: 4,
        frameIntervalMs: 100,
      };
    case "dead":
      return {
        frameBaseOffset: sprite.frameBaseOffset + 387,
        weaponFrameOffset:
          sprite.weaponFrameOffset === undefined || sprite.weaponFrameOffset === null
            ? null
            : sprite.weaponFrameOffset + 419,
        frameCount: 1,
        directionStride: 1,
      };
    case "reviving":
      return {
        frameBaseOffset: sprite.frameBaseOffset + 384,
        weaponFrameOffset:
          sprite.weaponFrameOffset === undefined || sprite.weaponFrameOffset === null
            ? null
            : sprite.weaponFrameOffset + 416,
        frameCount: 4,
        directionStride: 4,
        frameIntervalMs: 100,
        reverse: true,
      };
    default:
      return {
        frameBaseOffset: sprite.frameBaseOffset,
        weaponFrameOffset:
          sprite.weaponFrameOffset === undefined || sprite.weaponFrameOffset === null
            ? null
            : sprite.weaponFrameOffset,
        frameCount: Math.max(sprite.frameCount, 1),
        directionStride: Math.max(sprite.directionStride, 1),
        frameIntervalMs: 500,
      };
  }
}

function animationStateForMovement(
  entity: DisplayEntity,
  tileDistance: number,
  now: number,
): EntitySpriteAnimationState {
  if (entity.dead || entity.kind === "npc") {
    return "standing";
  }

  if (isEntityMovementAnimationActive(entity, now)) {
    return entity.movementAnimation ?? "walking";
  }

  if (entity.kind === "monster") {
    return "walking";
  }

  return tileDistance > 1 ? "running" : "walking";
}

function animationStateLifetimeMs(animationState: EntitySpriteAnimationState, _tileDistance: number) {
  switch (animationState) {
    case "running":
    case "walking":
      return 600;
    default:
      return 0;
  }
}

function entityAnimationStateForEntity(
  entity: DisplayEntity,
  snapshots: Record<string, EntityMotionSnapshot>,
  now: number,
): EntitySpriteAnimationState {
  if (entity.kind === "npc") {
    return "standing";
  }

  if (isEntityReviving(entity, now)) {
    return "reviving";
  }

  if (isEntityDying(entity, now)) {
    return "dying";
  }

  if (entity.dead) {
    return "dead";
  }

  if (isEntityStruck(entity, now)) {
    return "struck";
  }

  if (isEntityAttacking(entity, now)) {
    return entity.attackAnimation === "range" ? "attackRange" : "attackMelee";
  }

  if (isEntityMovementAnimationActive(entity, now)) {
    return entity.movementAnimation ?? "walking";
  }

  const snapshot = snapshots[entity.objectId];
  if (!snapshot || snapshot.expiresAt <= now) {
    return "standing";
  }

  return snapshot.animationState;
}

function entityMotionOffsetForEntity(
  entity: DisplayEntity,
  snapshots: Record<string, EntityMotionSnapshot>,
  now: number,
): ViewportOffset {
  const snapshot = snapshots[entity.objectId];
  if (!snapshot) {
    return EMPTY_VIEWPORT_OFFSET;
  }

  const remaining = remainingMotionRatio(snapshot, now);
  if (remaining <= 0) {
    return EMPTY_VIEWPORT_OFFSET;
  }

  return {
    x: crystalMovementPixelOffset((snapshot.fromX - snapshot.toX) * VIEWPORT_CELL_WIDTH * remaining),
    y: crystalMovementPixelOffset((snapshot.fromY - snapshot.toY) * VIEWPORT_CELL_HEIGHT * remaining),
  };
}

function refreshEntityMotionSnapshots(
  screen: ClientScreen,
  entities: DisplayEntity[],
  renderPlayer: DisplayEntity | null,
  snapshots: Record<string, EntityMotionSnapshot>,
  now: number,
): Record<string, EntityMotionSnapshot> {
  if (screen !== "game") {
    return {};
  }

  const nextSnapshots: Record<string, EntityMotionSnapshot> = {};
  const motionEntities = entities.map((entity) =>
    renderPlayer && entity.objectId === renderPlayer.objectId
      ? { ...entity, x: renderPlayer.x, y: renderPlayer.y }
      : entity,
  );

  for (const entity of motionEntities) {
    const previous = snapshots[entity.objectId];
    if (previous && previous.toX === entity.x && previous.toY === entity.y) {
      nextSnapshots[entity.objectId] = previous;
      continue;
    }

    const previousX = previous ? currentMotionCoordinate(previous.fromX, previous.toX, previous, now) : entity.x;
    const previousY = previous ? currentMotionCoordinate(previous.fromY, previous.toY, previous, now) : entity.y;
    const tileDistance = Math.max(Math.abs(entity.x - previousX), Math.abs(entity.y - previousY));

    if (tileDistance > 3) {
      nextSnapshots[entity.objectId] = {
        fromX: entity.x,
        fromY: entity.y,
        toX: entity.x,
        toY: entity.y,
        animationState: "standing",
        startedAt: now,
        expiresAt: 0,
      };
      continue;
    }

    if (tileDistance > 0.001) {
      const animationState = animationStateForMovement(entity, tileDistance, now);
      const packetStartedAt =
        entity.movementStartedAt !== undefined &&
        entity.movementStartedAt <= now &&
        entity.movementUntil !== undefined &&
        entity.movementUntil > now
          ? entity.movementStartedAt
          : now;
      const packetExpiresAt =
        entity.movementUntil !== undefined && entity.movementUntil > now
          ? entity.movementUntil
          : now + animationStateLifetimeMs(animationState, tileDistance);
      nextSnapshots[entity.objectId] = {
        fromX: previousX,
        fromY: previousY,
        toX: entity.x,
        toY: entity.y,
        animationState,
        startedAt: packetStartedAt,
        expiresAt: packetExpiresAt,
      };
      continue;
    }

    nextSnapshots[entity.objectId] = previous
      ? {
          ...previous,
          toX: entity.x,
          toY: entity.y,
        }
      : {
          fromX: entity.x,
          fromY: entity.y,
          toX: entity.x,
          toY: entity.y,
          animationState: "standing",
          startedAt: now,
          expiresAt: 0,
        };
  }

  return nextSnapshots;
}

function cameraMotionOffsetForEntity(
  entity: DisplayEntity,
  snapshots: Record<string, EntityMotionSnapshot>,
  now: number,
): ViewportOffset {
  const snapshot = snapshots[entity.objectId];
  if (!snapshot) {
    return EMPTY_VIEWPORT_OFFSET;
  }

  const remaining = remainingMotionRatio(snapshot, now);
  if (remaining <= 0) {
    return EMPTY_VIEWPORT_OFFSET;
  }

  return {
    x: crystalMovementPixelOffset((snapshot.toX - snapshot.fromX) * VIEWPORT_CELL_WIDTH * remaining),
    y: crystalMovementPixelOffset((snapshot.toY - snapshot.fromY) * VIEWPORT_CELL_HEIGHT * remaining),
  };
}

function remainingMotionRatio(snapshot: EntityMotionSnapshot, now: number) {
  if (snapshot.expiresAt <= snapshot.startedAt) {
    return 0;
  }

  return 1 - movementProgressRatio(snapshot, now);
}

function movementProgressRatio(snapshot: EntityMotionSnapshot, now: number) {
  const duration = snapshot.expiresAt - snapshot.startedAt;
  const elapsed = Math.min(Math.max(now - snapshot.startedAt, 0), duration);

  if (elapsed >= duration) {
    return 1;
  }

  if (snapshot.animationState !== "walking" && snapshot.animationState !== "running") {
    return elapsed / duration;
  }

  // Crystal ties walk/run displacement to the same six 100ms frames used by the body sprite.
  const frameIndex = Math.min(
    Math.floor(elapsed / CRYSTAL_MOVE_FRAME_INTERVAL_MS),
    CRYSTAL_MOVE_FRAME_COUNT - 1,
  );
  return Math.min((frameIndex + 1) / CRYSTAL_MOVE_FRAME_COUNT, 1);
}

function crystalMovementPixelOffset(value: number) {
  if (!Number.isFinite(value) || Math.abs(value) < 0.001) {
    return 0;
  }

  const integer = value < 0 ? Math.ceil(value) : Math.floor(value);
  const even = integer + (integer % 2);
  return Object.is(even, -0) ? 0 : even;
}

function currentMotionCoordinate(from: number, to: number, snapshot: EntityMotionSnapshot, now: number) {
  const remaining = remainingMotionRatio(snapshot, now);
  if (remaining <= 0) {
    return to;
  }

  return to + (from - to) * remaining;
}

function projectileProgress(projectile: DisplayProjectile, now: number) {
  if (projectile.expiresAt <= projectile.startedAt) {
    return 1;
  }

  const duration = projectile.expiresAt - projectile.startedAt;
  const elapsed = Math.min(Math.max(now - projectile.startedAt, 0), duration);
  return elapsed / duration;
}

function transientFrameCycle(
  now: number,
  startedAt: number | undefined,
  frameIntervalMs: number,
  frameCount: number,
) {
  if (typeof startedAt !== "number" || frameIntervalMs <= 0 || frameCount <= 1) {
    return 0;
  }

  const raw = Math.floor(Math.max(now - startedAt, 0) / frameIntervalMs);
  return Math.min(raw, frameCount - 1);
}

function isEntityMovementAnimationActive(entity: DisplayEntity, now: number) {
  return (
    (entity.movementAnimation === "walking" || entity.movementAnimation === "running") &&
    typeof entity.movementUntil === "number" &&
    entity.movementUntil > now
  );
}

function isEntityAttacking(entity: DisplayEntity, now: number) {
  return typeof entity.attackUntil === "number" && entity.attackUntil > now;
}

function isEntityStruck(entity: DisplayEntity, now: number) {
  return typeof entity.struckUntil === "number" && entity.struckUntil > now;
}

function isEntityDying(entity: DisplayEntity, now: number) {
  return typeof entity.dieUntil === "number" && entity.dieUntil > now;
}

function isEntityReviving(entity: DisplayEntity, now: number) {
  return typeof entity.reviveUntil === "number" && entity.reviveUntil > now;
}

function directionIndex(direction?: string) {
  switch (direction) {
    case "Up":
      return 0;
    case "UpRight":
      return 1;
    case "Right":
      return 2;
    case "DownRight":
      return 3;
    case "Down":
      return 4;
    case "DownLeft":
      return 5;
    case "Left":
      return 6;
    case "UpLeft":
      return 7;
    default:
      return 4;
  }
}

function computeNameplateTop(
  bodyFrame: OriginalSceneSpriteFrameMeta | null,
  hairFrame: OriginalSceneSpriteFrameMeta | null,
  weaponFrame?: OriginalSceneSpriteFrameMeta | null,
) {
  const displayTop = bodyFrame?.y ?? -48;
  return displayTop - 10;
}

function nameplateTopOffset(sprite: ViewportEntitySprite | null) {
  return sprite?.nameplateTop ?? -60;
}

function entityNameplateTopOffset(entity: DisplayEntity, sprite: ViewportEntitySprite | null) {
  const lineAdjustment =
    (entity.kind === "npc" || entity.kind === "monster") && entity.name.includes("_")
      ? -((entity.name.split("_").filter(Boolean).length - 1) * 10) / 2
      : 0;
  return nameplateTopOffset(sprite) + lineAdjustment;
}

function weaponPlacementForDirection(direction?: string) {
  switch (direction) {
    case "Left":
    case "Up":
    case "UpLeft":
    case "DownLeft":
      return "rear";
    default:
      return "front";
  }
}

function assassinRearWeaponsForDirection(
  direction: string | undefined,
  primaryWeapon: ViewportSpriteLayer | null,
  secondaryWeapon: ViewportSpriteLayer | null,
) {
  switch (direction) {
    case "Left":
    case "Up":
    case "UpLeft":
    case "DownLeft":
      return [primaryWeapon].filter((layer): layer is ViewportSpriteLayer => Boolean(layer));
    default:
      return [secondaryWeapon].filter((layer): layer is ViewportSpriteLayer => Boolean(layer));
  }
}

function assassinFrontWeaponsForDirection(
  direction: string | undefined,
  primaryWeapon: ViewportSpriteLayer | null,
  secondaryWeapon: ViewportSpriteLayer | null,
) {
  switch (direction) {
    case "UpRight":
    case "Right":
    case "DownRight":
    case "Down":
      return [primaryWeapon].filter((layer): layer is ViewportSpriteLayer => Boolean(layer));
    default:
      return [secondaryWeapon].filter((layer): layer is ViewportSpriteLayer => Boolean(layer));
  }
}

function questIconForEntity(
  entity: DisplayEntity,
  questLog: DisplayQuest[],
  animationFrameIndex: number,
) {
  if (entity.kind !== "npc") {
    return null;
  }

  const activeQuest = questLog.find((quest) => quest.stage !== "completed") ?? null;
  if (!activeQuest) {
    return null;
  }
  if (!entity.questIds?.includes(activeQuest.questId)) {
    return null;
  }

  const iconKey: QuestIconKey =
    activeQuest.stage === "available"
      ? "exclamationYellow"
      : activeQuest.stage === "inProgress"
        ? "questionWhite"
        : activeQuest.stage === "readyToTurnIn"
          ? "questionYellow"
          : "questionGreen";

  const frames = ORIGINAL_UI.game.questIcons[iconKey];
  return frames[animationFrameIndex % frames.length] ?? null;
}

function classCardForCharacter(
  character: SelectCharacterEntry,
  selected: boolean,
) {
  const card = ORIGINAL_UI.select.classCards[character.classKey];
  return selected ? card.active : card.base;
}

function portraitFramesForCharacter(character: SelectCharacterEntry) {
  const key = `${character.classKey}${character.gender === "male" ? "Male" : "Female"}` as SelectPortraitKey;
  return SELECT_PORTRAIT_ANIMATIONS[key];
}

function selectClassLabel(t: TranslateFn, classKey: SelectCharacterEntry["classKey"]) {
  switch (classKey) {
    case "warrior":
      return t("client.Warrior", [], "Warrior");
    case "wizard":
      return t("client.Wizard", [], "Wizard");
    case "taoist":
      return t("client.Taoist", [], "Taoist");
    case "assassin":
      return t("client.Assassin", [], "Assassin");
    case "archer":
      return t("client.Archer", [], "Archer");
  }
}

function originalItemIconPath(icon: number) {
  return `/original-ui/Items/${icon}.png`;
}

function originalMapLinkIconPath(icon: number) {
  return ORIGINAL_UI.bigMap.mapLinkIcon(icon);
}

function bigMapNpcKey(mapFileName: string | null | undefined, name: string, x: number, y: number) {
  return `${mapFileName ?? ""}|${name}|${x}|${y}`;
}

function withBigMapNpcInfo(mapFileName: string | null | undefined, entity: DisplayEntity): DisplayEntity {
  if (entity.kind !== "npc") return entity;

  const info = BIG_MAP_NPC_INDEX.get(bigMapNpcKey(mapFileName, entity.name, entity.x, entity.y));
  if (!info) {
    return entity;
  }

  return {
    ...entity,
    bigMapIcon: info.icon,
    showOnBigMap: true,
    canTeleportTo: info.teleport,
  };
}

function bigMapNpcRowsForWorld(world: DisplayWorld): BigMapNpcRowView[] {
  const mapFileName = world.mapFileName ?? "";
  const manifestRows = CRYSTAL_BIG_MAP_NPCS.filter((npc) => npc.map === mapFileName);
  if (manifestRows.length > 0) {
    return manifestRows.map((npc, index) => ({
      key: `${npc.map}-${npc.name}-${npc.x}-${npc.y}-${index}`,
      name: npc.name,
      icon: npc.icon,
      x: npc.x,
      y: npc.y,
      canTeleportTo: npc.teleport,
    }));
  }

  return world.entities
    .filter((entity) => entity.kind === "npc")
    .map((entity) => withBigMapNpcInfo(world.mapFileName, entity))
    .filter((entity) => entity.showOnBigMap !== false)
    .map((entity) => ({
      key: entity.objectId,
      name: entity.name,
      icon: entity.bigMapIcon ?? 120,
      x: entity.x,
      y: entity.y,
      canTeleportTo: entity.canTeleportTo === true,
    }));
}

function bigMapNpcDisplayName(name: string) {
  if (!name.includes("_")) {
    return name;
  }

  const parts = name.split("_").filter(Boolean);
  if (parts.length <= 1) {
    return name.replace(/_/g, "");
  }

  return `${parts.slice(0, -1).map((part) => `(${part})`).join("")}${parts.at(-1) ?? ""}`;
}

function statNumber(value: number) {
  return value > 0 ? String(value) : "";
}

function statPair(current?: number, max?: number) {
  if (current === undefined || max === undefined) {
    return "";
  }

  return `${current}/${max}`;
}

function formatBinaryDateTimeLabel(locale: string, value: number, template: string) {
  const date = dateFromBinaryDateTime(value);
  if (!date) {
    return null;
  }

  const formatted = new Intl.DateTimeFormat(locale, {
    year: "numeric",
    month: "2-digit",
    day: "2-digit",
    hour: "2-digit",
    minute: "2-digit",
  }).format(date);

  return template.replace("{0}", formatted);
}

function dateFromBinaryDateTime(value: number) {
  if (!Number.isFinite(value) || value === 0) {
    return null;
  }

  const raw = BigInt(Math.trunc(value));
  const unsigned = BigInt.asUintN(64, raw);
  const ticks = Number(unsigned & 0x3fffffffffffffffn);
  if (!Number.isFinite(ticks) || ticks <= 0) {
    return null;
  }

  const unixMilliseconds = Math.floor(ticks / 10_000 - 62_135_596_800_000);
  const date = new Date(unixMilliseconds);
  return Number.isNaN(date.getTime()) ? null : date;
}

function equipmentSlotForItemKey(key: string): EquipmentSlot | null {
  if (key.includes("helmet")) return "helmet";
  if (key.includes("armour")) return "armour";
  if (key.includes("dagger") || key.includes("weapon")) return "weapon";
  if (key.includes("necklace")) return "necklace";
  if (key.includes("bracelet-left")) return "braceletLeft";
  if (key.includes("bracelet-right")) return "braceletRight";
  if (key.includes("ring-left")) return "ringLeft";
  if (key.includes("ring-right")) return "ringRight";
  if (key.includes("amulet")) return "amulet";
  if (key.includes("boots")) return "boots";
  if (key.includes("belt")) return "belt";
  if (key.includes("stone")) return "stone";
  if (key.includes("mount")) return "mount";
  if (key.includes("torch")) return "torch";
  return null;
}

function displayFieldValue(current?: number, max?: number) {
  if (current === undefined) {
    return "";
  }

  return max === undefined ? String(current) : `${current}/${max}`;
}

function buildViewportMapSprites(
  world: DisplayWorld,
  player: DisplayEntity,
  animationFrameIndex: number,
): ViewportMapSprites {
  if (!world.originalMapRegion) {
    return EMPTY_VIEWPORT_MAP_SPRITES;
  }

  const floorMinX = player.x - VIEWPORT_RANGE_X;
  const floorMaxX = player.x + VIEWPORT_RANGE_X;
  const floorMinY = player.y - VIEWPORT_RANGE_Y;
  const floorMaxY = player.y + VIEWPORT_RANGE_Y;
  const objectMinX = floorMinX - 4;
  const objectMaxX = floorMaxX + 4;
  const objectMinY = floorMinY - 4;
  const objectMaxY = floorMaxY + 25;
  const floor: ViewportMapSprite[] = [];
  const objects: ViewportMapSprite[] = [];

  for (const cell of world.originalMapRegion.cells) {
    const inFloorBounds =
      cell.x >= floorMinX && cell.x <= floorMaxX && cell.y >= floorMinY && cell.y <= floorMaxY;
    const inObjectBounds =
      cell.x >= objectMinX && cell.x <= objectMaxX && cell.y >= objectMinY && cell.y <= objectMaxY;

    appendViewportMapSprite(
      floor,
      objects,
      world.originalMapRegion,
      cell.back,
      cell,
      player,
      animationFrameIndex,
      inFloorBounds,
      inObjectBounds,
    );
    appendViewportMapSprite(
      floor,
      objects,
      world.originalMapRegion,
      cell.middle,
      cell,
      player,
      animationFrameIndex,
      inFloorBounds,
      inObjectBounds,
    );
    appendViewportMapSprite(
      floor,
      objects,
      world.originalMapRegion,
      cell.front,
      cell,
      player,
      animationFrameIndex,
      inFloorBounds,
      inObjectBounds,
    );
    appendViewportMapSprite(
      floor,
      objects,
      world.originalMapRegion,
      cell.tileAnimation,
      cell,
      player,
      animationFrameIndex,
      false,
      inObjectBounds,
    );
  }

  return {
    floor,
    objects,
  };
}

function appendViewportMapSprite(
  floorTarget: ViewportMapSprite[],
  objectTarget: ViewportMapSprite[],
  region: OriginalMapRegion,
  spriteId: string | null | undefined,
  cell: { x: number; y: number },
  player: DisplayEntity,
  animationFrameIndex: number,
  inFloorBounds: boolean,
  inObjectBounds: boolean,
) {
  if (!spriteId) {
    return;
  }

  const sprite = region.sprites[spriteId];
  if (!sprite || !sprite.frames.length) {
    return;
  }

  const target =
    sprite.drawMode === "floor"
      ? inFloorBounds
        ? floorTarget
        : null
      : inObjectBounds
        ? objectTarget
        : null;

  if (!target) {
    return;
  }

  const frame = sprite.frames[animationFrameIndex % sprite.frames.length] ?? sprite.frames[0];
  if (!frame) {
    return;
  }

  const cellLeft = VIEWPORT_TILE_LEFT_ORIGIN + (cell.x - player.x) * VIEWPORT_CELL_WIDTH;
  const cellTop = VIEWPORT_TILE_TOP_ORIGIN + (cell.y - player.y) * VIEWPORT_CELL_HEIGHT;
  const crystalOffset = crystalMapFrameOffset(frame);
  const useCrystalOffset = sprite.drawMode === "object" && crystalMapFrameUsesOffset(frame);

  target.push({
    key: `${spriteId}:${cell.x}:${cell.y}:${animationFrameIndex % sprite.frames.length}`,
    path: frame.path,
    cellX: cell.x,
    cellY: cell.y,
    left: cellLeft + (useCrystalOffset ? crystalOffset.x : 0),
    top:
      sprite.drawMode === "object"
        ? cellTop + VIEWPORT_CELL_HEIGHT - frame.height + (useCrystalOffset ? crystalOffset.y : 0)
        : cellTop,
    width: frame.width,
    height: frame.height,
    zIndex: viewportDepthForCell(cell.x, cell.y, player, sprite.drawMode === "object" ? 1 : 0),
  });
}

function crystalMapFrameUsesOffset(frame: OriginalMapSpriteFrame) {
  return crystalMapFrameHasCrystalOffsetMode(frame.path);
}

function crystalMapFrameOffset(frame: OriginalMapSpriteFrame): ViewportOffset {
  if (!crystalMapFrameHasCrystalOffsetMode(frame.path)) {
    return EMPTY_VIEWPORT_OFFSET;
  }

  if (typeof frame.offsetX === "number" || typeof frame.offsetY === "number") {
    return {
      x: frame.offsetX ?? 0,
      y: frame.offsetY ?? 0,
    };
  }

  // Crystal draws the Bichon torch/fire blend frames with the Lib frame offset enabled.
  // Older packaged starter-map JSON predates offset export; these 100x100 light frames
  // are anchored around the red torch head, not the tile floor or lamp base.
  if (/\/original-map\/WemadeMir2\/Objects\/27(2[3-9]|3[0-2])\.png$/i.test(frame.path)) {
    return { x: -50, y: -100 };
  }

  return EMPTY_VIEWPORT_OFFSET;
}

function crystalMapFrameHasCrystalOffsetMode(path: string) {
  return /\/original-map\/WemadeMir2\/Objects\/27(2[3-9]|3[0-2])\.png$/i.test(path);
}

function viewportDepthForCell(
  x: number,
  y: number,
  player: Pick<DisplayEntity, "x" | "y">,
  layerOffset = 0,
) {
  return VIEWPORT_BASE_Z + (y - player.y) * VIEWPORT_ROW_Z_STRIDE + (x - player.x) * 2 + layerOffset;
}

function buildSceneBackdropTiles(world: DisplayWorld, player: DisplayEntity | null): SceneBackdropTile[] {
  const center = player
    ? { x: player.x, y: player.y }
    : world.sceneView?.center
      ? { x: world.sceneView.center.x, y: world.sceneView.center.y }
      : null;

  if (!center) {
    return [];
  }

  const startX = center.x - VIEWPORT_RANGE_X;
  const endX = center.x + VIEWPORT_RANGE_X;
  const startY = center.y - VIEWPORT_RANGE_Y;
  const endY = center.y + VIEWPORT_RANGE_Y;
  const tiles: SceneBackdropTile[] = [];

  for (let y = startY; y <= endY; y += 1) {
    for (let x = startX; x <= endX; x += 1) {
      const terrain = terrainKindAt(world.terrainPatches, x, y);
      const variation = Math.abs((x * 31 + y * 17) % 2);

      tiles.push({
        key: `${x}:${y}`,
        left: VIEWPORT_TILE_LEFT_ORIGIN + (x - center.x) * VIEWPORT_CELL_WIDTH,
        top: VIEWPORT_TILE_TOP_ORIGIN + (y - center.y) * VIEWPORT_CELL_HEIGHT,
        texture: sceneTextureForTerrain(terrain, variation),
        tint: sceneTintForTerrain(terrain, variation),
      });
    }
  }

  return tiles;
}

function terrainKindAt(
  patches: Array<{ x: number; y: number; width: number; height: number; kind: string }>,
  x: number,
  y: number,
) {
  for (let index = patches.length - 1; index >= 0; index -= 1) {
    const patch = patches[index];
    if (x >= patch.x && x < patch.x + patch.width && y >= patch.y && y < patch.y + patch.height) {
      return patch.kind;
    }
  }

  return patches[0]?.kind ?? "grass";
}

function sceneTextureForTerrain(terrain: string, variation: number) {
  switch (terrain) {
    case "dirt":
      return variation === 0 ? "/debug/map-samples/smtile-0.png" : "/debug/map-samples/smtile-104.png";
    case "road":
      return variation === 0 ? "/debug/map-samples/smtile-32.png" : "/debug/map-samples/smtile-52.png";
    case "water":
      return variation === 0 ? "/debug/map-samples/smtile-0.png" : "/debug/map-samples/tiles-1.png";
    case "stone":
      return variation === 0 ? "/debug/map-samples/tiles-0.png" : "/debug/map-samples/tiles-1.png";
    default:
      return variation === 0 ? "/debug/map-samples/smtile-72.png" : "/debug/map-samples/smtile-80.png";
  }
}

function sceneTintForTerrain(terrain: string, variation: number) {
  switch (terrain) {
    case "dirt":
      return variation === 0 ? "rgba(121, 84, 38, 0.16)" : "rgba(88, 58, 24, 0.10)";
    case "road":
      return variation === 0 ? "rgba(146, 108, 52, 0.14)" : "rgba(117, 85, 39, 0.12)";
    case "water":
      return variation === 0 ? "rgba(34, 84, 106, 0.48)" : "rgba(20, 58, 79, 0.52)";
    case "stone":
      return variation === 0 ? "rgba(76, 74, 66, 0.34)" : "rgba(54, 53, 46, 0.28)";
    default:
      return variation === 0 ? "rgba(58, 96, 36, 0.10)" : "rgba(42, 74, 28, 0.08)";
  }
}

function mapSpriteBlendMode(path: string) {
  return /\/original-map\/WemadeMir2\/Objects\/27(2[3-9]|3[0-2])\.png$/i.test(path) ? "screen" : undefined;
}

function mapSpriteRenderPath(path: string) {
  const frame = bichonTorchLightFrame(path);
  return frame ? `/generated/original-map-blend/WemadeMir2/Objects/${frame}.png` : path;
}

function bichonTorchLightFrame(path: string) {
  const match = path.match(/\/original-map\/WemadeMir2\/Objects\/(27(?:2[3-9]|3[0-2]))\.png$/i);
  return match?.[1] ?? null;
}

function playerFacingChatLines(logs: DisplayLogLine[], activeFilter: ChatFilterKey) {
  const lines = logs
    .filter((line) => line.tone !== "network")
    .filter((line) => matchesChatFilter(line, activeFilter))
    .map((line) => ({
      text: trimLogTimestamp(line.text),
      tone: line.tone === "chat" ? ("chat" as const) : ("system" as const),
      channel: line.channel,
    }))
    .slice(0, 24)
    .reverse();

  return lines.length
    ? lines
    : Array.from({ length: 6 }, () => ({
        text: "",
        tone: "chat" as const,
        channel: "normal" as const,
      }));
}

function trimLogTimestamp(text: string) {
  return text.replace(/^\[\d{1,2}:\d{2}:\d{2}(?:\s?[AP]M)?\]\s*/i, "");
}

function matchesChatFilter(line: DisplayLogLine, activeFilter: ChatFilterKey) {
  switch (activeFilter) {
    case "all":
      return true;
    case "shout":
      return line.channel === "shout" || line.channel === "announcement";
    case "trade":
      return line.channel === "trade";
    case "whisper":
      return line.channel === "whisper";
    case "group":
      return line.channel === "group";
    case "guild":
      return line.channel === "guild";
    case "lover":
    case "mentor":
      return line.channel === "system" || line.channel === "hint";
    default:
      return true;
  }
}

function miniMapBounds(
  world: DisplayWorld,
  player: DisplayEntity | null,
  asset: { src: string; width: number; height: number } | null,
) {
  if (asset && world.originalMapRegion) {
    const mapWidth = Math.max(world.originalMapRegion.mapWidth, 1);
    const mapHeight = Math.max(world.originalMapRegion.mapHeight, 1);
    const scaleX = asset.width / mapWidth;
    const scaleY = asset.height / mapHeight;
    const center = player ?? world.sceneView?.center ?? { x: mapWidth / 2, y: mapHeight / 2 };
    const viewWidth = Math.min(MINI_MAP_VIEW_WIDTH, asset.width);
    const viewHeight = Math.min(MINI_MAP_VIEW_HEIGHT, asset.height);
    const rasterLeft = clampNumber(Math.round(center.x * scaleX - viewWidth / 2), 0, Math.max(asset.width - viewWidth, 0));
    const rasterTop = clampNumber(Math.round(center.y * scaleY - viewHeight / 2), 0, Math.max(asset.height - viewHeight, 0));

    return {
      minX: rasterLeft / Math.max(scaleX, 0.0001),
      minY: rasterTop / Math.max(scaleY, 0.0001),
      width: viewWidth / Math.max(scaleX, 0.0001),
      height: viewHeight / Math.max(scaleY, 0.0001),
      raster: {
        left: -rasterLeft,
        top: -rasterTop,
        width: asset.width,
        height: asset.height,
      },
    };
  }

  if (world.sceneView) {
    const minX = world.sceneView.center.x - Math.floor(world.sceneView.width / 2);
    const minY = world.sceneView.center.y - Math.floor(world.sceneView.height / 2);
    return {
      minX,
      minY,
      width: world.sceneView.width,
      height: world.sceneView.height,
      raster: null,
    };
  }

  if (player) {
    return {
      minX: player.x - 12,
      minY: player.y - 9,
      width: 24,
      height: 18,
      raster: null,
    };
  }

  return null;
}

function originalMiniMapAssetPath(miniMapIndex: number | null) {
  if (!miniMapIndex || miniMapIndex <= 0) {
    return null;
  }

  return MINI_MAP_ASSETS.get(miniMapIndex) ?? null;
}

function originalBigMapAssetPath(bigMapIndex: number | null | undefined) {
  if (!bigMapIndex || bigMapIndex <= 0) {
    return null;
  }

  return MINI_MAP_ASSETS.get(bigMapIndex) ?? null;
}

function bigMapViewport(asset: { src: string; width: number; height: number } | null) {
  const contentWidth = asset ? Math.min(568, asset.width) : 568;
  const contentHeight = asset ? Math.min(380, asset.height) : 380;
  return {
    left: 14 + Math.floor((568 - contentWidth) / 2),
    top: 52 + Math.floor((380 - contentHeight) / 2),
    width: contentWidth,
    height: contentHeight,
    contentWidth,
    contentHeight,
    imageLeft: 0,
    imageTop: 0,
  };
}

function miniMapRasterStyle(raster: { left: number; top: number; width: number; height: number }) {
  return {
    width: `${raster.width}px`,
    height: `${raster.height}px`,
    left: `${raster.left}px`,
    top: `${raster.top}px`,
  };
}

function miniMapTerrainColor(kind: string) {
  switch (kind) {
    case "dirt":
      return "#7d6138";
    case "road":
      return "#9b8754";
    case "water":
      return "#295978";
    case "stone":
      return "#5f5f56";
    default:
      return "#456b2b";
  }
}

function miniMapEntityColor(kind: string) {
  switch (kind) {
    case "selfPlayer":
      return "#ffffff";
    case "player":
      return "#ffffff";
    case "monster":
      return "#ff0000";
    case "npc":
      return "#00ff32";
    default:
      return "#ffffff";
  }
}

function entityNameplateColor(entity: DisplayEntity) {
  return argbToCssColor(entity.nameColourArgb) ?? (entity.kind === "npc" ? "#00ff00" : "#ffffff");
}

function entityNameplateParts(entity: DisplayEntity) {
  const label = entityDisplayName(entity);
  if (entity.kind !== "npc") {
    return { primary: label, secondary: null };
  }
  const parts = label.split(" ").filter(Boolean);
  if (parts.length < 2) {
    return { primary: label, secondary: null };
  }

  return { primary: parts.slice(0, -1).join(" "), secondary: parts.at(-1) ?? null };
}

function argbToCssColor(value: number | undefined) {
  if (value === undefined || value === -1) {
    return undefined;
  }

  const argb = value >>> 0;
  const alpha = (argb >>> 24) & 0xff;
  if (alpha === 0) {
    return undefined;
  }

  const red = (argb >>> 16) & 0xff;
  const green = (argb >>> 8) & 0xff;
  const blue = argb & 0xff;
  if (alpha === 0xff) {
    return `#${red.toString(16).padStart(2, "0")}${green.toString(16).padStart(2, "0")}${blue
      .toString(16)
      .padStart(2, "0")}`;
  }

  return `rgba(${red}, ${green}, ${blue}, ${(alpha / 255).toFixed(3)})`;
}

function clampNumber(value: number, min: number, max: number) {
  if (!Number.isFinite(value)) return min;
  return Math.max(min, Math.min(max, value));
}

function equipmentSlotFromLabel(label: string): EquipmentSlot {
  switch (label) {
    case "Weapon":
      return "weapon";
    case "Armour":
      return "armour";
    case "Helmet":
      return "helmet";
    case "Mount":
      return "mount";
    case "Necklace":
      return "necklace";
    case "Torch":
      return "torch";
    case "BraceletL":
      return "braceletLeft";
    case "BraceletR":
      return "braceletRight";
    case "RingL":
      return "ringLeft";
    case "RingR":
      return "ringRight";
    case "Amulet":
      return "amulet";
    case "Boots":
      return "boots";
    case "Belt":
      return "belt";
    default:
      return "stone";
  }
}

function duraIconForSlot(slot: EquipmentSlot, equipmentItems: DisplayEquipmentItem[]) {
  const item = equipmentItems.find((entry) => entry.slot === slot);
  const ratio = item ? item.durabilityCurrent / Math.max(item.durabilityMax, 1) : 0;
  const level = !item ? "empty" : ratio <= 0.33 ? "danger" : ratio <= 0.66 ? "warning" : "healthy";

  switch (slot) {
    case "weapon":
      return ORIGINAL_UI.game.duraIcons.weapon[level === "empty" ? "healthy" : level];
    case "armour":
      return ORIGINAL_UI.game.duraIcons.armour[level === "empty" ? "healthy" : level];
    case "helmet":
      return ORIGINAL_UI.game.duraIcons.helmet[level === "empty" ? "healthy" : level];
    case "mount":
      return ORIGINAL_UI.game.duraIcons.mount[level === "empty" ? "healthy" : level];
    case "necklace":
      return ORIGINAL_UI.game.duraIcons.necklace[level === "empty" ? "healthy" : level];
    case "torch":
      return ORIGINAL_UI.game.duraIcons.torch[level === "empty" ? "healthy" : level];
    case "braceletLeft":
    case "braceletRight":
      return ORIGINAL_UI.game.duraIcons.bracelet[level === "empty" ? "healthy" : level];
    case "ringLeft":
    case "ringRight":
      return ORIGINAL_UI.game.duraIcons.ring[level === "empty" ? "healthy" : level];
    case "amulet":
      return ORIGINAL_UI.game.duraIcons.amulet[level === "empty" ? "healthy" : level];
    case "boots":
      return ORIGINAL_UI.game.duraIcons.boots[level === "empty" ? "healthy" : level];
    case "belt":
      return ORIGINAL_UI.game.duraIcons.belt[level === "empty" ? "healthy" : level];
    case "stone":
      return ORIGINAL_UI.game.duraIcons.stone.empty;
  }
}
