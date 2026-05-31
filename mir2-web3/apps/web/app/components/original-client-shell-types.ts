import type { ClientScreen, CharacterTabKey, InventoryTabKey } from "../../lib/original-ui";
import type { Mir2Language } from "../../lib/localization";
import type { SuiWalletSummary } from "../../lib/client-login-runtime";
import type { SystemMenuTransferOption } from "./original-client-system-menu";
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
  TradeHandlers,
  NpcShopHandlers,
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

export type OriginalClientShellProps = {
  language: Mir2Language;
  screen: ClientScreen;
  runtimePhase: string;
  runtimeMessage: string;
  wsState: string;
  reconnectStatus: GatewayReconnectStatus;
  world: DisplayWorld;
  player: DisplayEntity | null;
  predictedPlayerPosition: PredictedPlayerMotion | null;
  getLivePlayerRenderPosition?: () => PredictedPlayerMotion | null;
  selectedEntity: DisplayEntity | null;
  sortedEntities: DisplayEntity[];
  viewportEntities: Array<DisplayEntity & { dx: number; dy: number }>;
  viewportTiles: Array<{ x: number; y: number; dx: number; dy: number }>;
  sceneInteractionReady: boolean;
  bevyEntityRendererReady: boolean;
  bevyRuntimeBackend: "webgpu" | "webgl2" | null;
  onSceneAssetReadinessChange: (readiness: SceneAssetReadiness) => void;
  onBevyEntityRenderStateChange: (state: BevyEntityRenderState) => void;
  logs: DisplayLogLine[];
  accountId: string;
  password: string;
  chatMessage: string;
  loginBusy: boolean;
  loginError: string | null;
  suiWallets: SuiWalletSummary[];
  walletPickerOpen: boolean;
  dubheWalletUrl: string;
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
  onSendMail: (name: string, message: string, gold: number, itemUniqueIds?: number[]) => void;
  onBuyGameShopItem: (gameShopIndex: number, quantity: number, paymentType: "gold" | "credit") => void;
  onRunStage5Command: (action: string, args?: string[]) => void;
  onSendClientCommand: (command: Record<string, unknown>) => void;
  tradeHandlers: TradeHandlers;
  shopHandlers: NpcShopHandlers;
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
