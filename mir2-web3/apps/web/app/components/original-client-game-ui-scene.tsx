"use client";

import { memo, useEffect, useLayoutEffect, useRef, useState } from "react";

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
  sharedBagUiActive?: boolean;
  sharedStorageUiActive?: boolean;
  sharedHudUiActive?: boolean;
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
  defaultChatExpanded?: boolean;
  onChatMessageChange: (value: string) => void;
  onSendChat: (message: string) => void;
  onRequestTrade: () => void;
  onRentExpandedStorage: () => void;
  onLogout: () => void;
  onToggleCharacter: () => void;
  onToggleInventory: () => void;
  onToggleQuestLog: () => void;
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
  sharedBagUiActive = false,
  sharedStorageUiActive = false,
  sharedHudUiActive = false,
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
  defaultChatExpanded = true,
  onChatMessageChange,
  onSendChat,
  onRequestTrade,
  onRentExpandedStorage,
  onLogout,
  onToggleCharacter,
  onToggleInventory,
  onToggleQuestLog,
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
  const showMailPanel = mailOpen;
  const [showBigMap, setShowBigMap] = useState(false);
  const [showReportPanel, setShowReportPanel] = useState(false);
  const [showSystemMenu, setShowSystemMenu] = useState(false);
  const [showGameShop, setShowGameShop] = useState(false);
  useEffect(() => {
    const handler = (event: Event) => {
      const action = (event as CustomEvent<string>).detail;
      if (action === "gameShop") setShowGameShop(current => !current);
      else if (action === "menu") setShowSystemMenu(current => !current);
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
    || showSystemMenuFeaturePanel || showGameShop || showChatSettings);
  const hpOrbModalBlocked = Boolean(questUiModalBlocked || showDuraPanel || visibleDialog);
  useLayoutEffect(() => {
    onHpOrbModalChange?.(hpOrbModalBlocked);
    return () => onHpOrbModalChange?.(false);
  }, [hpOrbModalBlocked, onHpOrbModalChange]);
  useEffect(() => {
    onQuestUiModalChange?.(questUiModalBlocked);
    return () => onQuestUiModalChange?.(false);
  }, [onQuestUiModalChange, questUiModalBlocked]);
  const gamepadUiOpen = Boolean(
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
      <ObjectiveTracker questLog={world.questLog} playerClass={player?.classKey ?? null} />
      <MiniMapPanel
        t={t}
        world={world}
        player={player}
        showMailPanel={showMailPanel}
        showBigMap={showBigMap}
        onToggleMail={() => onToggleMail?.()}
        onToggleBigMap={() => setShowBigMap((current) => !current)}
        showMailAction={!IS_PLATINUM_176_PROFILE}
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
        onSelectFilter={selectChatFilter}
        onRequestTrade={onRequestTrade}
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
        hiddenFilters={hiddenChatFilters}
        expanded={chatExpanded}
        showSettings={showChatSettings}
        transparent={transparentChat}
        onChatMessageChange={onChatMessageChange}
        onSendChat={sendActiveChatMessage}
        onCloseSettings={() => setShowChatSettings(false)}
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
        showCharacter={showCharacter}
        showInventory={showInventory}
        showQuestLog={showQuestLog}
        activeCharacterTab={activeCharacterTab}
        activeInventoryTab={activeInventoryTab}
        onToggleCharacter={onToggleCharacter}
        onToggleInventory={onToggleInventory}
        onToggleQuestLog={onToggleQuestLog}
        onOpenCharacterTab={onOpenCharacterTab}
        onOpenInventoryTab={onOpenInventoryTab}
        onDropGold={() => onDropGold(100)}
        onLogout={onLogout}
        showGameShop={showGameShop}
        onToggleGameShop={() => setShowGameShop((current) => !current)}
        showGameShopAction={!IS_PLATINUM_176_PROFILE}
        showMenu={showSystemMenu}
        onToggleMenu={() => setShowSystemMenu((current) => !current)}
      />}
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
          onClose={() => setShowGameShop(false)}
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
          npcName={npcRepairService === "special" ? t("ui.shopSpecialRepair", [], "Special Repair") : t("ui.shopRepair", [], "Repair")}
          gold={world.gold}
          initialTab={npcRepairService}
          availableTabs={[npcRepairService]}
          repairItems={
            npcRepairService === "repair"
              ? world.equipmentItems.map((item) => ({
                  id: item.slot,
                  name: item.name,
                  icon: item.icon,
                  price: 0,
                  description: item.description,
                  durabilityCurrent: item.durabilityCurrent,
                  durabilityMax: item.durabilityMax,
                  disabled: item.durabilityMax <= 0 || item.durabilityCurrent >= item.durabilityMax,
                }))
              : undefined
          }
          specialRepairItems={
            npcRepairService === "special"
              ? world.equipmentItems.map((item) => ({
                  id: item.slot,
                  name: item.name,
                  icon: item.icon,
                  price: 0,
                  description: item.description,
                  durabilityCurrent: item.durabilityCurrent,
                  durabilityMax: item.durabilityMax,
                  disabled: item.durabilityMax <= 0 || item.durabilityCurrent >= item.durabilityMax,
                }))
              : undefined
          }
          onRepair={(id) => {
            const item = world.equipmentItems.find((entry) => entry.slot === id);
            if (item) onRepairItem({ slot: item.slot });
          }}
          onSpecialRepair={(id) => {
            const item = world.equipmentItems.find((entry) => entry.slot === id);
            if (item) onSpecialRepairItem({ slot: item.slot });
          }}
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
