"use client";

import type { CrystalTooltipDocument } from "../../lib/shared-item-tooltip";
import type { ChatUiControls, CashPreviewLayerDocument } from "../../lib/client-core-runtime";
import type { MapImageRouteSource, MapImageRouteIntent } from "../../lib/client-map-input";

import { memo, useEffect, useLayoutEffect, useRef, useState, type PointerEvent } from "react";

import { useWorldSelector } from "../../lib/world-model";
import type { WorldStore } from "../../lib/world-model";
import type { CharacterTabKey, InventoryTabKey } from "../../lib/original-ui";
import { IS_PLATINUM_176_PROFILE } from "../../lib/content-profile";
import { MainHud, MainHudStatus, HudSpriteFallback } from "./original-client-overlays";
import {
  BeltDialog,
  ChatFilterBar,
  ChatFrame,
  DuraPanel,
  chatPrefixForFilter,
  formatChatMessageForFilter,
  type ChatFilterKey,
  type ChatOptionFilterKey,
} from "./original-client-panels";
import { NpcDialogPanel, ReportPanel } from "./original-client-dialogs";
import { ObjectiveTracker } from "./original-client-objective-tracker";
import { BigMapDialog, MiniMapPanel, hasOriginalMiniMapAsset } from "./original-client-map-panels";
import { GameShopWindow, NpcShopWindow } from "./original-client-game-shop";
import type { NpcGoldBuyQuote } from "../../lib/bevy-npc-shop-buy";
import type { NpcRepairView, NpcRepairSelection } from "../../lib/npc-repair-service";
import type { CashGameShopSource, CashGameShopConfirmation, CashGameShopEntry } from "../../lib/cash-game-shop-ui";
import { InventoryWindow } from "./original-client-inventory-window";
import { CharacterWindow } from "./original-client-character-window";
import type { Mir2InputProfile } from "./original-client-device-profile";
import type { Mir2GamepadFamily } from "./original-client-gamepad-input";
import {
  SystemMenuFeaturePanel,
  SystemMenuPanel,
  type SystemMenuSurfacePanel,
} from "./original-client-system-menu";
import type {
  DisplayEntity,
  DisplayLogLine,
  DisplayNpcShopService,
  DisplayWorld,
  EquipmentActionRef,
  EquipmentSlot,
  ItemActionRef,
  ItemContainer,
  MergeItemRef,
  MoveItemRef,
  TranslateFn,
} from "./original-client-types";

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
  showQuestLog: boolean;
  sharedQuestUiActive?: boolean;
  sharedQuestWorldUiActive?: boolean;
  sharedBagUiActive?: boolean;
  sharedStorageUiActive?: boolean;
  sharedHudUiActive?: boolean;
  hpView?: boolean;
  sharedHudPlan?: import("../../lib/bevy-hud-ui").MainHudPlan | null;
  experienceSpriteOwned?: boolean;
  weightSpriteOwned?: boolean;
  sharedCharacterStatsActive?: boolean;
  inventoryInitialDeleteMode?: boolean;
  onInventoryCompatibilityInteraction?: () => void;
  onQuestUiModalChange?: (blocked: boolean) => void;
  onHpOrbModalChange?: (blocked: boolean) => void;
  activeInventoryTab: InventoryTabKey;
  activeCharacterTab: CharacterTabKey;
  storageServiceOpenVersion: number;
  storagePasswordOpenVersion?: number;
  npcShopService: DisplayNpcShopService | null;
  npcRepairService: "repair" | "special" | null;
  npcRepairView?: NpcRepairView | null;
  onSelectNpcRepair?: (view: NpcRepairView, uniqueId: number) => NpcRepairSelection | null;
  onConfirmNpcRepair?: (selection: NpcRepairSelection) => boolean;
  onToggleNpcRepairHold?: (view: NpcRepairView) => void;
  npcRepairTargetSelection?: NpcRepairSelection | null;
  onRegisterNpcRepairTarget?: (view: NpcRepairView, node: HTMLElement) => () => void;
  onNpcRepairBagPointerDown?: (event: PointerEvent<HTMLButtonElement>, item: ItemActionRef) => void;
  defaultChatExpanded?: boolean;
  chatUi?: ChatUiControls;
  mapImageRouteSource?: MapImageRouteSource | null;
  onMapImageRoute?: (intent: MapImageRouteIntent) => void;
  onMapImageRoutePress?: (source: MapImageRouteSource | null) => void;
  onMapRouteModalChange?: (blocked: boolean) => void;
  onChatMessageChange: (value: string) => void;
  onSendChat: (message: string) => void;
  onRequestTrade: () => void;
  onRentExpandedStorage: () => void;
  storageRentalPrompt?: { id: number; renewing: boolean } | null;
  onConfirmStorageRental?: (id: number) => void;
  onCancelStorageRental?: (id: number) => void;
  onLogout: () => void;
  onToggleCharacter: () => void;
  onToggleInventory: () => void;
  onToggleQuestLog: () => void;
  onToggleOptions?: () => void;
  onCloseCharacter: () => void;
  onCloseInventory: () => void;
  onCloseStorage?: () => void;
  bevyNpcShopUiActive?: boolean;
  bevyNpcShopUiTransitioning?: boolean;
  getBevyNpcShopInputBlocked?: () => boolean;
  npcShopTab?: "buy" | "sell";
  onNpcShopTabChange?: (tab: "buy" | "sell") => void;
  onCloseNpcShopService: () => void;
  onCloseNpcRepairService: () => void;
  onOpenCharacterTab: (tab: CharacterTabKey) => void;
  onOpenInventoryTab: (tab: InventoryTabKey) => void;
  onSelectNpcDialogTarget: (target: string) => void;
  onSubmitNpcInput: (value: string) => void;
  onUseItem: (item: ItemActionRef) => void;
  onDropItem: (item: ItemActionRef) => void;
  onEquipItem: (item: ItemActionRef, slot: EquipmentSlot) => void;
  onRemoveItem: (item: EquipmentActionRef) => void;
  onMoveItem: (item: MoveItemRef, toSlot: number, toContainer?: ItemContainer) => boolean;
  onMergeItem: (from: MergeItemRef, to: MergeItemRef) => void;
  onSplitItem: (item: ItemActionRef, count: number) => void;
  onStoreItem: (item: MoveItemRef, toSlot: number) => boolean;
  onTakeBackItem: (item: MoveItemRef, toSlot: number, toContainer: ItemContainer) => boolean;
  onUnlockStorage: (password: string) => void;
  onSetStoragePassword: (currentPassword: string, newPassword: string) => void;
  onRemoveStoragePassword: (currentPassword: string) => void;
  onSellItem: (item: ItemActionRef, count: number) => void;
  onBuyNpcShopItem: (id: number, quantity: number, panelType: number) => void;
  onQuoteNpcShopItem: (id: number, quantity: number) => NpcGoldBuyQuote | null;
  onDropGold: (amount: number) => void;
  onRepairItem: (item: EquipmentActionRef) => void;
  onSpecialRepairItem: (item: EquipmentActionRef) => void;
  onCastSkill: (skillKey: string) => void;
  onClaimMail: (mailId: number) => void;
  mailOpen?: boolean;
  onToggleMail?: () => void;
  onDeleteMail: (mailId: number) => void;
  onBuyGameShopItem: (gameShopIndex: number, quantity: number, paymentType: "gold" | "credit") => void;
  cashGameShopSource?: CashGameShopSource | null;
  cashGameShopPending?: boolean;
  onConfirmCashGameShopPurchase?: (confirmation: CashGameShopConfirmation) => void;
  onReadCashGameShopItemTooltip?: (item: CashGameShopEntry) => CrystalTooltipDocument | null;
  cashPreviewSourceKey?: string | null;
  onReadCashPreviewLayers?: (item: CashGameShopEntry, direction: number, elapsedMs: number) => CashPreviewLayerDocument | null;
  onTurnCashPreview?: (direction: number, right: boolean) => number | null;
  onGameShopVisibilityChange?: (open: boolean) => void;
  onSendClientCommand: (command: Record<string, unknown>) => void;
  inputProfile: Mir2InputProfile;
  gamepadFamily: Mir2GamepadFamily;
  onStartTutorial: () => void;
};

function GameUiSceneInner({
  t,
  locale,
  runtimeMessage,
  world,
  player,
  logs,
  chatMessage,
  showInventory,
  showCharacter,
  showQuestLog,
  sharedQuestUiActive = false,
  sharedQuestWorldUiActive = false,
  sharedBagUiActive = false,
  sharedStorageUiActive = false,
  sharedHudUiActive = false,
  hpView = true,
  sharedHudPlan = null,
  experienceSpriteOwned = false,
  weightSpriteOwned = false,
  sharedCharacterStatsActive = false,
  inventoryInitialDeleteMode = false,
  onInventoryCompatibilityInteraction,
  onQuestUiModalChange,
  onHpOrbModalChange,
  activeInventoryTab,
  activeCharacterTab,
  storageServiceOpenVersion,
  storagePasswordOpenVersion = 0,
  npcShopService,
  npcRepairService,
  npcRepairView, onSelectNpcRepair, onConfirmNpcRepair,
  onToggleNpcRepairHold, npcRepairTargetSelection, onRegisterNpcRepairTarget, onNpcRepairBagPointerDown,
  defaultChatExpanded = true,
  chatUi, mapImageRouteSource, onMapImageRoute, onMapImageRoutePress, onMapRouteModalChange,
  onChatMessageChange,
  onSendChat,
  onRequestTrade,
  onRentExpandedStorage,
  storageRentalPrompt, onConfirmStorageRental, onCancelStorageRental,
  onLogout,
  onToggleCharacter,
  onToggleInventory,
  onToggleQuestLog,
  onToggleOptions,
  onCloseCharacter,
  onCloseInventory,
  onCloseStorage,
  bevyNpcShopUiActive = false, bevyNpcShopUiTransitioning = false, getBevyNpcShopInputBlocked, npcShopTab, onNpcShopTabChange,
  onCloseNpcShopService,
  onCloseNpcRepairService,
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
  onBuyNpcShopItem, onQuoteNpcShopItem,
  onDropGold,
  onRepairItem,
  onSpecialRepairItem,
  onCastSkill,
  onClaimMail,
  mailOpen = false,
  onToggleMail,
  onDeleteMail,
  onBuyGameShopItem,
  cashGameShopSource, cashGameShopPending, onConfirmCashGameShopPurchase, onReadCashGameShopItemTooltip, onGameShopVisibilityChange,
  cashPreviewSourceKey, onReadCashPreviewLayers, onTurnCashPreview,
  onSendClientCommand,
  inputProfile,
  gamepadFamily,
  onStartTutorial,
}: GameUiSceneProps) {
  const [showDuraPanel, setShowDuraPanel] = useState(false);
  const [showBelt, setShowBelt] = useState(true);
  const [beltVertical, setBeltVertical] = useState(false);
  const [activeChatFilter, setActiveChatFilter] = useState<ChatFilterKey>("all");
  const [hiddenChatFilters, setHiddenChatFilters] = useState<ChatOptionFilterKey[]>([]);
  const [transparentChat, setTransparentChat] = useState(false);
  const [chatExpanded, setChatExpanded] = useState(defaultChatExpanded);
  const [showChatSettings, setShowChatSettings] = useState(false);
  const effectiveChatSettingsOpen=chatUi?.document.open??showChatSettings;
  const chatUiEpoch=chatUi?.document.epoch;
  const chatUiRef=useRef(chatUi);chatUiRef.current=chatUi;
  const [chatPointerHeld,setChatPointerHeld]=useState(false);const chatPointerHeldRef=useRef(false);
  const showMailPanel = mailOpen;
  const [showBigMap, setShowBigMap] = useState(false);
  const [showMinimap, setShowMinimap] = useState(true);
  const [showReportPanel, setShowReportPanel] = useState(false);
  const [showSystemMenu, setShowSystemMenu] = useState(false);
  const [showGameShop, setShowGameShop] = useState(false);
  const gameShopOpenRef = useRef(false);
  const gameShopVisibilityRef = useRef(onGameShopVisibilityChange);
  gameShopVisibilityRef.current = onGameShopVisibilityChange;
  function changeGameShopOpen(open: boolean) {
    gameShopOpenRef.current = open; gameShopVisibilityRef.current?.(open); setShowGameShop(open);
  }
  useEffect(() => () => {gameShopOpenRef.current = false; gameShopVisibilityRef.current?.(false);}, []);
  const storageRentalDialogRef = useRef<HTMLDivElement>(null);
  const storageRentalCancelRef = useRef(onCancelStorageRental);
  storageRentalCancelRef.current = onCancelStorageRental;
  useEffect(() => {
    if (!storageRentalPrompt) return;
    const prior = document.activeElement instanceof HTMLElement ? document.activeElement : null;
    const dialog = storageRentalDialogRef.current;
    dialog?.querySelector<HTMLButtonElement>("[data-rental-cancel]")?.focus();
    const key = (event: KeyboardEvent) => {
      if (event.key === "Escape") { event.preventDefault(); event.stopPropagation(); storageRentalCancelRef.current?.(storageRentalPrompt.id); }
      if (event.key === "Tab") {
        const buttons = dialog?.querySelectorAll<HTMLButtonElement>("button:not(:disabled)");
        if (!buttons?.length) return;
        const index = Array.from(buttons).indexOf(document.activeElement as HTMLButtonElement);
        event.preventDefault(); buttons[(index + (event.shiftKey ? buttons.length - 1 : 1)) % buttons.length].focus();
      }
    };
    document.addEventListener("keydown", key, true);
    return () => { document.removeEventListener("keydown", key, true); if (prior?.isConnected) prior.focus(); };
  }, [storageRentalPrompt?.id]);
  useEffect(() => {
    const handler = (event: Event) => {
      const action = (event as CustomEvent<string>).detail;
      if (action === "gameShop") changeGameShopOpen(!gameShopOpenRef.current);
      else if (action === "menu") setShowSystemMenu(current => !current);
      else if (action === "belt") setShowBelt(current => !current);
      else if (action === "beltFlip") setBeltVertical(current => !current);
      else if (action === "minimap") setShowMinimap(current => !current);
      else if (action === "bigmap") setShowBigMap(current => !current);
      else if (action === "closeAll") {
        changeGameShopOpen(false); setShowBigMap(false); setShowSystemMenu(false); setShowSystemMenuFeaturePanel(null);
        setShowReportPanel(false); const chat=chatUiRef.current;if(chat?.document.open)chat.cancel(chat.document.epoch);
        setShowChatSettings(false); setShowDuraPanel(false);
      }
    };
    window.addEventListener("mir2:hud-host-action", handler);
    return () => window.removeEventListener("mir2:hud-host-action", handler);
  }, []);
  const previousQuestLogOpenRef = useRef(showQuestLog);
  const [showSystemMenuFeaturePanel, setShowSystemMenuFeaturePanel] = useState<SystemMenuSurfacePanel | null>(null);
  const [dismissedDialogKey, setDismissedDialogKey] = useState<string | null>(null);

  useEffect(() => {
    const openingDiary = showQuestLog && !previousQuestLogOpenRef.current;
    previousQuestLogOpenRef.current = showQuestLog;
    if (inputProfile === "touch" && openingDiary && showSystemMenu) {
      setShowSystemMenu(false);
    }
  }, [inputProfile, showQuestLog, showSystemMenu]);

  const dialogKey = world.activeNpcDialog
    ? JSON.stringify([
        world.activeNpcDialog.npcObjectId,
        world.activeNpcDialog.title,
        world.activeNpcDialog.body,
        world.activeNpcDialog.footer,
        world.activeNpcDialog.links,
        world.activeNpcDialog.input ?? null,
      ])
    : null;
  const visibleDialog =
    !sharedQuestUiActive && world.activeNpcDialog && dialogKey !== dismissedDialogKey ? world.activeNpcDialog : null;
  // Mail has one Page-owned presentation; do not report its own-open as a competing modal.
  const questUiModalBlocked = Boolean(showBigMap || showReportPanel || showSystemMenu
    || showSystemMenuFeaturePanel || showGameShop || effectiveChatSettingsOpen || chatPointerHeld || storageRentalPrompt);
  const hpOrbModalBlocked = Boolean(questUiModalBlocked || showDuraPanel || visibleDialog);
  const mapRouteModalBlocked = Boolean(showReportPanel || showSystemMenu || showSystemMenuFeaturePanel
    || showGameShop || effectiveChatSettingsOpen || chatPointerHeld || storageRentalPrompt || visibleDialog);
  useLayoutEffect(() => {
    onMapRouteModalChange?.(mapRouteModalBlocked);
    return () => onMapRouteModalChange?.(false);
  }, [mapRouteModalBlocked, onMapRouteModalChange]);
  useLayoutEffect(() => {
    onHpOrbModalChange?.(hpOrbModalBlocked);
    return () => onHpOrbModalChange?.(false);
  }, [hpOrbModalBlocked, onHpOrbModalChange]);
  useEffect(() => {
    onQuestUiModalChange?.(questUiModalBlocked);
    return () => onQuestUiModalChange?.(false);
  }, [onQuestUiModalChange, questUiModalBlocked]);
  const gamepadUiOpen = Boolean(
    storageRentalPrompt ||
    showMailPanel ||
      showBigMap ||
      showReportPanel ||
      showSystemMenu ||
      showSystemMenuFeaturePanel ||
      showGameShop ||
      visibleDialog ||
      showInventory ||
      showCharacter ||
      showQuestLog,
  );


  function changeChatSettings(open:boolean) {
    const ui=chatUi;
    if(open){onHpOrbModalChange?.(true);onQuestUiModalChange?.(true);onMapRouteModalChange?.(true);}
    if(ui&&chatUiEpoch!==undefined) {if(open&&!effectiveChatSettingsOpen)ui.open(chatUiEpoch);else if(!open&&effectiveChatSettingsOpen)ui.cancel(chatUiEpoch);}
    else setShowChatSettings(open);
  }
  function resizeChatWindow() {
    if(chatUi&&chatUiEpoch!==undefined){chatUi.resize(chatUiEpoch);setChatExpanded(true);}
    else setChatExpanded(current=>!current);
  }
  function changeChatPointerHold(held:boolean) {
    chatPointerHeldRef.current=held;setChatPointerHeld(held);
    const base=Boolean(showBigMap||showReportPanel||showSystemMenu||showSystemMenuFeaturePanel||showGameShop||effectiveChatSettingsOpen||storageRentalPrompt);
    onQuestUiModalChange?.(held||base);
    onHpOrbModalChange?.(held||base||showDuraPanel||Boolean(visibleDialog));
    onMapRouteModalChange?.(held||Boolean(showReportPanel||showSystemMenu||showSystemMenuFeaturePanel
      ||showGameShop||effectiveChatSettingsOpen||storageRentalPrompt||visibleDialog));
  }

  function selectChatFilter(filter: ChatFilterKey) {
    const previousPrefix = chatPrefixForFilter(activeChatFilter);
    const nextPrefix = chatPrefixForFilter(filter);
    setActiveChatFilter(filter);

    if (chatMessage === "" || chatMessage === previousPrefix) {
      onChatMessageChange(nextPrefix);
      return;
    }

    if (previousPrefix && chatMessage.startsWith(previousPrefix)) {
      onChatMessageChange(`${nextPrefix}${chatMessage.slice(previousPrefix.length)}`);
    }
  }

  function sendActiveChatMessage() {
    const message = formatChatMessageForFilter(activeChatFilter, chatMessage);
    if (!message) return;
    onSendChat(message);
    onChatMessageChange(chatPrefixForFilter(activeChatFilter));
  }

  function toggleHiddenChatFilter(filter: ChatOptionFilterKey) {
    setHiddenChatFilters((current) =>
      current.includes(filter)
        ? current.filter((entry) => entry !== filter)
        : [...current, filter],
    );
  }

  function toggleAllHiddenChatFilters() {
    setHiddenChatFilters((current) =>
      current.length === 8
        ? []
        : ["normal", "whisper", "shout", "system", "lover", "mentor", "group", "guild"],
    );
  }

  useEffect(() => {
    if (!dialogKey) {
      setDismissedDialogKey(null);
    } else if (dialogKey !== dismissedDialogKey) {
      setDismissedDialogKey(null);
    }
  }, [dialogKey, dismissedDialogKey]);

  return (
    <div
      className={`game-ui-scene ${hasOriginalMiniMapAsset(world.miniMapIndex) ? "with-mini-map" : "without-mini-map"}`}
      data-gamepad-ui-open={gamepadUiOpen ? "true" : "false"}
      data-chat-expanded={chatExpanded ? "true" : "false"}
    >
      {!sharedQuestWorldUiActive && <ObjectiveTracker questLog={world.questLog} playerClass={player?.classKey ?? null} />}
      {showMinimap && <MiniMapPanel
        t={t}
        world={world}
        player={player}
        routeSource={mapImageRouteSource}
        onImageRoute={onMapImageRoute}
        onImageRoutePress={onMapImageRoutePress}
        showMailPanel={showMailPanel}
        showBigMap={showBigMap}
        onToggleMail={() => onToggleMail?.()}
        onToggleBigMap={() => setShowBigMap((current) => !current)}
        showMailAction={!IS_PLATINUM_176_PROFILE}
      />}
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
        chatUi={chatUi}
        showSettings={effectiveChatSettingsOpen}
        onSelectFilter={selectChatFilter}
        onRequestTrade={onRequestTrade}
        onToggleExpanded={resizeChatWindow}
        onToggleSettings={()=>changeChatSettings(!effectiveChatSettingsOpen)}
        onToggleReport={() => setShowReportPanel((current) => !current)}
      />
      <ChatFrame
        t={t}
        runtimeMessage={runtimeMessage}
        logs={logs}
        chatMessage={chatMessage}
        hints={world.interactionHints}
        activeFilter={activeChatFilter}
        hiddenFilters={hiddenChatFilters}
        chatUi={chatUi} onInputHoldChange={changeChatPointerHold}
        expanded={chatExpanded}
        showSettings={effectiveChatSettingsOpen}
        transparent={transparentChat}
        onChatMessageChange={onChatMessageChange}
        onSendChat={sendActiveChatMessage}
        onCloseSettings={()=>changeChatSettings(false)}
        onToggleHiddenFilter={toggleHiddenChatFilter}
        onToggleAllHiddenFilters={toggleAllHiddenChatFilters}
        onToggleTransparent={() => setTransparentChat((current) => !current)}
      />
      {sharedHudUiActive ? <><MainHudStatus t={t} mapTitle={world.mapTitle} world={world} />
        <HudSpriteFallback plan={sharedHudPlan} experienceOwned={experienceSpriteOwned} weightOwned={weightSpriteOwned} /></> : <MainHud
        t={t}
        connected={world.connected}
        mapTitle={world.mapTitle}
        player={player}
        world={world}
        hpView={hpView}
        showCharacter={showCharacter}
        showInventory={showInventory}
        showQuestLog={showQuestLog}
        activeCharacterTab={activeCharacterTab}
        activeInventoryTab={activeInventoryTab}
        onToggleCharacter={onToggleCharacter}
        onToggleInventory={onToggleInventory}
        onToggleQuestLog={onToggleQuestLog}
        onToggleOptions={onToggleOptions}
        onOpenCharacterTab={onOpenCharacterTab}
        onOpenInventoryTab={onOpenInventoryTab}
        onDropGold={() => onDropGold(100)}
        onLogout={onLogout}
        showGameShop={showGameShop}
        onToggleGameShop={() => changeGameShopOpen(!gameShopOpenRef.current)}
        showGameShopAction={!IS_PLATINUM_176_PROFILE}
        showMenu={showSystemMenu}
        onToggleMenu={() => setShowSystemMenu((current) => !current)}
      />}
      {showBigMap ? (
        <BigMapDialog
          t={t}
          world={world}
          player={player}
          routeSource={mapImageRouteSource}
          onImageRoute={onMapImageRoute}
          onImageRoutePress={onMapImageRoutePress}
          onClose={() => setShowBigMap(false)}
        />
      ) : null}
      {showReportPanel ? <ReportPanel t={t} logs={logs} onClose={() => setShowReportPanel(false)} /> : null}
      {showSystemMenu ? (
        <SystemMenuPanel
          t={t}
          playerName={player?.name ?? null}
          mapTitle={world.mapTitle}
          mapFileName={world.mapFileName}
          inSafeZone={world.inSafeZone}
          inputProfile={inputProfile}
          gamepadFamily={gamepadFamily}
          questLogOpen={showQuestLog}
          onStartTutorial={() => {
            setShowSystemMenu(false);
            onStartTutorial();
          }}
          onOpenPanel={(panel) => {
            setShowSystemMenuFeaturePanel(panel);
            setShowSystemMenu(false);
          }}
          onToggleQuestLog={() => {
            onToggleQuestLog();
            setShowSystemMenu(false);
          }}
          onClose={() => setShowSystemMenu(false)}
          onLogout={onLogout}
          isPlatinum176={IS_PLATINUM_176_PROFILE}
        />
      ) : null}
      {showSystemMenuFeaturePanel ? (
        <SystemMenuFeaturePanel
          t={t}
          feature={showSystemMenuFeaturePanel}
          playerName={player?.name ?? null}
          world={world}
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
          source={cashGameShopSource}
          purchasePending={cashGameShopPending}
          onConfirmPurchase={onConfirmCashGameShopPurchase}
          onReadItemTooltip={onReadCashGameShopItemTooltip}
          previewSourceKey={cashPreviewSourceKey}
          onReadPreviewLayers={onReadCashPreviewLayers}
          onTurnPreview={onTurnCashPreview}
          onClose={() => changeGameShopOpen(false)}
        />
      ) : null}
      {npcShopService && !(bevyNpcShopUiActive && npcShopTab === "buy") ? (
        <NpcShopWindow
          key={`${npcShopService.serviceRevision}:${npcShopService.catalogRevision}`}
          t={t}
          npcName={npcShopService.npcName}
          gold={world.gold}
          initialTab={npcShopService.supportsBuy ? "buy" : "sell"}
          tab={npcShopTab}
          onTabChange={(tab) => { if (tab === "buy" || tab === "sell") onNpcShopTabChange?.(tab); }}
          inputBlocked={bevyNpcShopUiActive || bevyNpcShopUiTransitioning}
          getInputBlocked={getBevyNpcShopInputBlocked}
          availableTabs={[
            ...(npcShopService.supportsBuy ? (["buy"] as const) : []),
            ...(npcShopService.supportsSell ? (["sell"] as const) : []),
          ]}
          buyItems={npcShopService.buyItems}
          onQuoteBuy={(id, quantity) => onQuoteNpcShopItem(Number(id), quantity)}
          sellItems={world.inventoryItems.map((item) => ({
            id: item.uniqueId,
            name: item.name,
            icon: item.icon,
            price: Math.max(0, Number(item.sellValue ?? 0)),
            count: item.quantity,
            description: item.description,
          }))}
          onBuy={(id, quantity) =>
            onBuyNpcShopItem(Number(id), quantity, npcShopService.panelType)
          }
          onSell={(id, quantity) => {
            const item = world.inventoryItems.find((entry) => entry.uniqueId === Number(id));
            if (item) onSellItem(item, quantity);
          }}
          onClose={onCloseNpcShopService}
        />
      ) : null}
      {npcRepairService ? (
        <NpcShopWindow
          key={npcRepairService}
          t={t}
          npcName={npcRepairView?.name || (npcRepairService === "special" ? t("ui.shopSpecialRepair", [], "Special Repair") : t("ui.shopRepair", [], "Repair"))}
          gold={npcRepairView?.gold}
          repairView={npcRepairView}
          onSelectRepair={onSelectNpcRepair}
          onConfirmRepair={onConfirmNpcRepair}
          onToggleRepairHold={onToggleNpcRepairHold}
          repairTargetSelection={npcRepairTargetSelection}
          onRegisterRepairTarget={onRegisterNpcRepairTarget}
          initialTab={npcRepairService}
          availableTabs={[npcRepairService]}
          repairItems={
            npcRepairService === "repair"
              ? npcRepairView?.rows.map((item) => ({
                  id: item.uniqueId ?? `unknown:${item.slot}`,
                  name: item.name,
                  icon: item.icon,
                  price: item.quote?.displayedTotal ?? 0,
                  description: item.reason === "gold" ? t("client.LowGold", [], "Not enough gold.")
                    : item.reason === "unknown" ? t("ui.itemStateSync", [], "Item state is not ready. Wait for it to sync, then try again.")
                    : item.reason === "busy" ? t("ui.itemBusy", [], "Waiting for the previous repair result.")
                    : item.reason === "locked" ? t("ui.itemLocked", [], "This item is reserved by another operation.") : undefined,
                  durabilityCurrent: item.input?.currentDura,
                  durabilityMax: item.input?.maxDura,
                  disabled: item.disabled,
                }))
              : undefined
          }
          specialRepairItems={
            npcRepairService === "special"
              ? npcRepairView?.rows.map((item) => ({
                  id: item.uniqueId ?? `unknown:${item.slot}`,
                  name: item.name,
                  icon: item.icon,
                  price: item.quote?.displayedTotal ?? 0,
                  description: item.reason === "gold" ? t("client.LowGold", [], "Not enough gold.")
                    : item.reason === "unknown" ? t("ui.itemStateSync", [], "Item state is not ready. Wait for it to sync, then try again.")
                    : item.reason === "busy" ? t("ui.itemBusy", [], "Waiting for the previous repair result.")
                    : item.reason === "locked" ? t("ui.itemLocked", [], "This item is reserved by another operation.") : undefined,
                  durabilityCurrent: item.input?.currentDura,
                  durabilityMax: item.input?.maxDura,
                  disabled: item.disabled,
                }))
              : undefined
          }
          onClose={onCloseNpcRepairService}
        />
      ) : null}
      {visibleDialog ? (
        <NpcDialogPanel
          t={t}
          dialog={visibleDialog}
          playerClass={player?.classKey ?? null}
          onClose={() => {
            onSelectNpcDialogTarget("@Exit");
            setDismissedDialogKey(dialogKey);
          }}
          onSelectTarget={onSelectNpcDialogTarget}
          onSubmitInput={onSubmitNpcInput}
        />
      ) : null}
      {showInventory && !sharedBagUiActive && !sharedStorageUiActive ? (
        <InventoryWindow
          initialDeleteMode={inventoryInitialDeleteMode}
          onCompatibilityInteraction={onInventoryCompatibilityInteraction}
          repairMode={npcRepairService !== null}
          onRepairPointerDown={onNpcRepairBagPointerDown}
          t={t}
          locale={locale}
          activeTab={activeInventoryTab}
          world={world}
          storageServiceOpenVersion={storageServiceOpenVersion}
          storagePasswordOpenVersion={storagePasswordOpenVersion}
          onClose={onCloseInventory}
          onCloseStorage={onCloseStorage}
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
      {showCharacter && !sharedCharacterStatsActive ? (
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
      {storageRentalPrompt ? (
        <div data-ui-interactive="true" data-storage-rental-confirmation={storageRentalPrompt.id}
          onPointerDown={event => event.stopPropagation()} onClick={event => event.stopPropagation()}
          style={{ position: "absolute", inset: 0, zIndex: 1000, background: "rgba(0,0,0,.45)", display: "grid", placeItems: "center", pointerEvents: "auto" }}>
          <div ref={storageRentalDialogRef} role="dialog" aria-modal="true" aria-labelledby="storage-rental-title"
            style={{ width: 340, maxWidth: "90%", padding: 20, background: "#201a12", border: "1px solid #967642", color: "#f2dfb6" }}>
            <strong id="storage-rental-title">{t("client.Storage", [], "Storage")}</strong>
            <p>{locale.startsWith("zh")
              ? (storageRentalPrompt.renewing ? "是否支付 1,000,000 金币，将扩展仓库续租 10 天？" : "是否支付 1,000,000 金币，租用扩展仓库 10 天？")
              : (storageRentalPrompt.renewing ? "Extend your storage rental for 10 days at a cost of 1,000,000 gold?" : "Rent extra storage for 10 days at a cost of 1,000,000 gold?")}</p>
            <div style={{ display: "flex", gap: 12 }}>
              <button type="button" style={{ minHeight: 44, flex: 1 }} disabled={!onConfirmStorageRental}
                onClick={() => onConfirmStorageRental?.(storageRentalPrompt.id)}>{t("client.OK", [], "OK")}</button>
              <button type="button" data-rental-cancel style={{ minHeight: 44, flex: 1 }}
                onClick={() => onCancelStorageRental?.(storageRentalPrompt.id)}>{t("client.Cancel", [], "Cancel")}</button>
            </div>
          </div>
        </div>
      ) : null}
    </div>
  );
}

// Memoized: GameUiScene does not receive motionNow, so it should not re-render on
// every motion-clock tick (30 Hz in-game). Props change only on server world pushes,
// user input, or window-open state changes — all far below 30 Hz.
export const GameUiScene = memo(GameUiSceneInner);

// Render-perf Stage 5c (opt-in, flag-gated via `?selectorHud=1`): the store-bound
// HUD. Instead of receiving `world` as a prop from the parent shell — whose fresh
// identity busts `memo(GameUiScene)` on every coalesced flush — this wrapper
// SUBSCRIBES to the world store directly with `useWorldSelector`. The `world` prop
// then no longer flows through the parent's render, so the parent re-rendering for
// any other reason cannot force the HUD to re-render; only an actual world-store
// change does. (A direct whole-world selector is identity-stable per store
// snapshot, so no `isEqual` is needed; the follow-up — narrowing MainHud to
// `hp/mp/level/gold/weight` and MiniMap to `entities` — rides on this same store
// and further cuts renders.) The legacy `world={world}` prop path is left fully
// intact in the shell behind the OFF-by-default flag for instant rollback.
type GameUiSceneStoreBoundProps = Omit<GameUiSceneProps, "world"> & { store: WorldStore };

export function GameUiSceneStoreBound({ store, ...rest }: GameUiSceneStoreBoundProps) {
  const world = useWorldSelector(store, (s) => s) as DisplayWorld;
  return <GameUiScene world={world} {...rest} />;
}
