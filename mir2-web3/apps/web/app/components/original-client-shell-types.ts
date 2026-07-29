import type { ClientScreen, CharacterTabKey, InventoryTabKey } from "../../lib/original-ui";
import type { Mir2Language } from "../../lib/localization";
import type { WorldStore } from "../../lib/world-model";
import type { SuiWalletSummary } from "../../lib/client-login-runtime";
import type { SystemMenuTransferOption } from "./original-client-system-menu";
import type { MapStandaloneTileDraw, MapTileDraw } from "./webgl2-map-atlas-layer";
import type {
  DisplayEntity,
  DisplayLogLine,
  DisplayWorld,
  CreateCharacterDraft,
  EntityKind,
  EquipmentActionRef,
  EquipmentSlot,
  ItemActionRef,
  MergeItemRef,
  MoveItemRef,
  PredictedPlayerMotion,
  SelectCharacterEntry,
} from "./original-client-types";

export type GatewayReconnectStatus = {
  mode: "idle" | "scheduled" | "connecting" | "resuming" | "failed";
  attempt: number;
  nextAttemptAt: number | null;
};

export type SceneAssetReadiness = {
  key: string;
  ready: boolean;
  interactionReady?: boolean;
  visualReady?: boolean;
  status: "idle" | "loading" | "ready" | "timeout";
  total: number;
  loaded: number;
  failed: number;
  pending: number;
  durationMs: number;
  failedUrls: string[];
};

export type BevyEntityRenderState = {
  enabled: boolean;
  stageWidth: number;
  stageHeight: number;
  // Viewport center used to derive every entity's dx/dy base. The runtime echoes
  // the applied values in PresentationPose so DOM overlays never commit against
  // a different tile center.
  centerX?: number;
  centerY?: number;
  atlases?: Array<{
    key: string;
    width: number;
    height: number;
    imageUrl?: string;
    rects: Array<{
      key: string;
      x: number;
      y: number;
      width: number;
      height: number;
    }>;
  }>;
  atlasImages?: Array<{
    key: string;
    width: number;
    height: number;
    pixels?: Uint8Array;
  }>;
  entities: Array<{
    objectId: string;
    dead: boolean;
    isSelf?: boolean;
    // Authoritative target cell for the packet-driven Bevy presentation
    // handshake. The runtime only applies a remote segment once these match.
    gridX?: number;
    gridY?: number;
    // Opt-in (?bevyEntityInterp=1) per-entity motion window — present only for
    // NON-self entities under the flag. Lets the Bevy runtime interpolate the
    // sub-cell glide at display Hz instead of the producer folding it into each
    // layer's left/top at the ~33Hz motionNow clock. fromX/Y may be fractional
    // (a move begun mid-glide); motionDurationMs = expiresAt - startedAt of the
    // EntityMotionSnapshot. Absent ⇒ Bevy applies no offset (byte-identical fold).
    motionFromX?: number;
    motionFromY?: number;
    motionToX?: number;
    motionToY?: number;
    motionStartedMs?: number;
    motionDurationMs?: number;
    layers: Array<{
      key: string;
      path: string;
      atlasKey?: string;
      atlasRectKey?: string;
      left: number;
      top: number;
      width: number;
      height: number;
      z: number;
      opacity?: number;
    }>;
  }>;
};

export type BevyMapRenderState = {
  enabled: boolean;
  stageWidth: number;
  stageHeight: number;
  // Exact producer/runtime-generation handshake. Bevy echoes this only after
  // every URL-backed atlas and standalone upload in this state is committed.
  ackKey: string;
  // Monotonic producer revision plus the player tile used to build this draw list.
  // These are presentation provenance only; movement authority remains server-side.
  revision?: number;
  centerX?: number;
  centerY?: number;
  // Atlas page descriptors carrying the source-rect geometry each tile's
  // atlasRectKey indexes into. Mirrors BevyEntityRenderState.atlases.
  atlases?: Array<{
    key: string;
    width: number;
    height: number;
    imageUrl?: string;
    rects: Array<{
      key: string;
      x: number;
      y: number;
      width: number;
      height: number;
    }>;
  }>;
  // Raw RGBA page pixels, uploaded once per page key. Stripped before the state
  // JSON is serialized to the runtime (mirrors BevyEntityRenderState.atlasImages).
  atlasImages?: Array<{
    key: string;
    width: number;
    height: number;
    pixels?: Uint8Array;
  }>;
  // EXACTLY buildMapTileDrawList's output (folds projection + crystal offsets +
  // the sub-tile camera offset into left/top). MapTileDraw uses `rectKey`; the
  // runtime's MapTile deserializes it via #[serde(rename = "rectKey")].
  tiles: MapTileDraw[];
  // Atlas misses are uploaded as standalone images so Bevy can keep ownership
  // of the world y-sort band instead of handing those cells back to DOM.
  standaloneTiles?: MapStandaloneTileDraw[];
  // Visible animation-family frames that must stay resident even though only
  // the current frame appears in standaloneTiles. Rust acknowledges these keys
  // after an atomic map transaction and never creates draw entities for them.
  retainedImageKeys?: string[];
  // Sub-tile camera scroll offset for the root-offset model. In the fold-in
  // model (the one Stage 1 uses) this is (0, 0) because the offset is already
  // baked into each tile's left/top.
  cameraOffset?: { x: number; y: number };
};

export type OriginalClientShellProps = {
  language: Mir2Language;
  screen: ClientScreen;
  runtimePhase: string;
  runtimeMessage: string;
  wsState: string;
  reconnectStatus: GatewayReconnectStatus;
  world: DisplayWorld;
  // Render-perf Stage 5c (opt-in, flag-gated): when `selectorHud` is true the
  // game-screen HUD (GameUiScene) subscribes to this world store via
  // `useWorldSelector` instead of receiving `world` as a prop, so the memoized
  // HUD finally holds across coalesced flushes. Both are OPTIONAL and default to
  // the legacy `world={world}` prop path (byte-identical) when absent/false.
  worldStore?: WorldStore;
  selectorHud?: boolean;
  player: DisplayEntity | null;
  predictedPlayerPosition: PredictedPlayerMotion | null;
  getLivePlayerRenderPosition?: (options?: {
    presentationOwnsInterpolation: boolean;
  }) => PredictedPlayerMotion | null;
  selectedEntity: DisplayEntity | null;
  sortedEntities: DisplayEntity[];
  viewportEntities: Array<DisplayEntity & { dx: number; dy: number }>;
  viewportTiles: Array<{ x: number; y: number; dx: number; dy: number }>;
  sceneInteractionReady: boolean;
  bevyEntityRendererReady: boolean;
  bevyRuntimeBackend: "webgpu" | "webgl2" | null;
  bevyMapRuntimeGeneration: number;
  bevyMapRuntimeReady: boolean;
  bevyMapPresentedImageKeys: ReadonlySet<string>;
  bevyMapImageResidencyVersion: number;
  onSceneAssetReadinessChange: (readiness: SceneAssetReadiness) => void;
  onBevyEntityRenderStateChange: (state: BevyEntityRenderState) => void;
  onBevyMapRenderStateChange: (state: BevyMapRenderState) => readonly string[];
  onBevyMapImagesEvicted?: (keys: string[]) => void;
  logs: DisplayLogLine[];
  accountId: string;
  password: string;
  chatMessage: string;
  loginBusy: boolean;
  loginError: string | null;
  suiWallets: SuiWalletSummary[];
  walletPickerOpen: boolean;
  dubheWalletUrl: string;
  identityProvider: string | null;
  identityLinkBusy: boolean;
  identityLinkStatus: string | null;
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
  onPasskeyLogin: () => void;
  onWalletPickerToggle: () => void;
  onWalletLogin: (walletId: string) => void;
  onIdentityLinkPasskey: () => void;
  onIdentityLinkWallet: (walletId?: string) => void;
  onQuickEnter: () => void;
  onResetClient: () => void;
  onExitSelect: () => void;
  onSendChat: (message: string) => void;
  onRequestTrade: () => void;
  onRentExpandedStorage: () => void;
  onLogout: () => void;
  onCreateCharacter: (draft: CreateCharacterDraft) => void;
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
  onViewportDirectionIntent: (
    direction: string,
    mode: "walk" | "run",
    options?: { discrete?: boolean },
  ) => void;
  onViewportDirectionStop: () => void;
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
