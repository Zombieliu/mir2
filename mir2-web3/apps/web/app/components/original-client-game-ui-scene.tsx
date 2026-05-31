"use client";

import { useEffect, useState } from "react";

import type { CharacterTabKey, InventoryTabKey } from "../../lib/original-ui";
import { MainHud } from "./original-client-overlays";
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
import { MailPanel, NpcDialogPanel, ReportPanel } from "./original-client-dialogs";
import { BigMapDialog, MiniMapPanel, hasOriginalMiniMapAsset } from "./original-client-map-panels";
import { GameShopWindow } from "./original-client-game-shop";
import { InventoryWindow } from "./original-client-inventory-window";
import { CharacterWindow } from "./original-client-character-window";
import { OptionsDialog } from "./original-client-options-dialog";
import { SkillBar } from "./original-client-skill-bar";
import { BuffBar } from "./original-client-buff-bar";
import { TradeDialog, TradeRequestPrompt } from "./original-client-trade";
import { NpcShopDialog } from "./original-client-npc-shop";
import {
  SystemMenuFeaturePanel,
  SystemMenuPanel,
  type SystemMenuSurfacePanel,
  type SystemMenuTransferOption,
} from "./original-client-system-menu";
import {
  ItemDragProvider,
  type ItemDragPayload,
  type ItemDropTarget,
} from "./original-client-drag-item";
import type { ViewportEntitySprite } from "./original-client-scene-rendering";
import type {
  DisplayEntity,
  DisplayItem,
  DisplayLogLine,
  DisplayWorld,
  EquipmentActionRef,
  EquipmentSlot,
  ItemActionRef,
  MergeItemRef,
  MoveItemRef,
  NpcShopHandlers,
  TradeHandlers,
  TranslateFn,
} from "./original-client-types";

type GameUiSceneProps = {
  t: TranslateFn;
  locale: string;
  runtimeMessage: string;
  world: DisplayWorld;
  player: DisplayEntity | null;
  paperdollSprite: ViewportEntitySprite | null;
  logs: DisplayLogLine[];
  chatMessage: string;
  showInventory: boolean;
  showCharacter: boolean;
  activeInventoryTab: InventoryTabKey;
  activeCharacterTab: CharacterTabKey;
  storageServiceOpenVersion: number;
  onChatMessageChange: (value: string) => void;
  onSendChat: (message: string) => void;
  onRequestTrade: () => void;
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
  onSendMail: (name: string, message: string, gold: number) => void;
  onBuyGameShopItem: (gameShopIndex: number, quantity: number, paymentType: "gold" | "credit") => void;
  onRunStage5Command: (action: string, args?: string[]) => void;
  onSendClientCommand: (command: Record<string, unknown>) => void;
  tradeHandlers: TradeHandlers;
  shopHandlers: NpcShopHandlers;
  transferOptions: SystemMenuTransferOption[];
};

export function GameUiScene({
  t,
  locale,
  runtimeMessage,
  world,
  player,
  paperdollSprite,
  logs,
  chatMessage,
  showInventory,
  showCharacter,
  activeInventoryTab,
  activeCharacterTab,
  storageServiceOpenVersion,
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
  onSendMail,
  onBuyGameShopItem,
  onRunStage5Command,
  onSendClientCommand,
  tradeHandlers,
  shopHandlers,
  transferOptions,
}: GameUiSceneProps) {
  const [showDuraPanel, setShowDuraPanel] = useState(false);
  const [showBelt, setShowBelt] = useState(true);
  const [beltVertical, setBeltVertical] = useState(false);
  const [activeChatFilter, setActiveChatFilter] = useState<ChatFilterKey>("all");
  const [hiddenChatFilters, setHiddenChatFilters] = useState<ChatOptionFilterKey[]>([]);
  const [transparentChat, setTransparentChat] = useState(false);
  const [chatExpanded, setChatExpanded] = useState(true);
  const [showChatSettings, setShowChatSettings] = useState(false);
  const [showMailPanel, setShowMailPanel] = useState(false);
  const [showBigMap, setShowBigMap] = useState(false);
  const [showReportPanel, setShowReportPanel] = useState(false);
  const [showSystemMenu, setShowSystemMenu] = useState(false);
  const [showGameShop, setShowGameShop] = useState(false);
  const [showOptions, setShowOptions] = useState(false);
  const [showSystemMenuFeaturePanel, setShowSystemMenuFeaturePanel] = useState<SystemMenuSurfacePanel | null>(null);
  const [dismissedDialogKey, setDismissedDialogKey] = useState<string | null>(null);

  const dialogKey = world.activeNpcDialog
    ? `${world.activeNpcDialog.npcObjectId}:${world.activeNpcDialog.title}:${world.worldTick}`
    : null;
  const visibleDialog =
    world.activeNpcDialog && dialogKey !== dismissedDialogKey ? world.activeNpcDialog : null;

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

  // Map a completed drag (source -> drop target) onto the existing Crystal item
  // commands. Stacks of the same item key merge; everything else moves/equips/
  // stores/unequips. Unsupported source/target combinations are a no-op so the
  // dragged item simply snaps back, matching Crystal's reject-on-invalid-drop.
  function findInventoryItemAt(container: DisplayItem["container"], slot: number) {
    return world.inventoryItems.find((item) => item.container === container && item.slot === slot) ?? null;
  }

  function resolveItemDrop(payload: ItemDragPayload, target: ItemDropTarget) {
    const source = payload.source;

    if (target.kind === "inventory") {
      if (source.kind === "equipment") {
        onRemoveItem({ slot: source.equipmentSlot });
        return;
      }
      if (source.kind === "trade") {
        tradeHandlers.retrieve(source.slot, target.slot);
        return;
      }
      if (source.kind === "storage") {
        onTakeBackItem({ uniqueId: payload.uniqueId, slot: source.slot, container: "storage" }, target.slot);
        return;
      }
      if (source.kind === "inventory") {
        if (source.container !== target.container || source.slot === target.slot) return;
        const occupant = findInventoryItemAt(target.container, target.slot);
        if (occupant && occupant.key === payload.key && occupant.uniqueId !== payload.uniqueId) {
          onMergeItem(
            { uniqueId: payload.uniqueId, slot: source.slot, container: source.container },
            { uniqueId: occupant.uniqueId, slot: occupant.slot, container: occupant.container },
          );
          return;
        }
        onMoveItem({ uniqueId: payload.uniqueId, slot: source.slot, container: source.container }, target.slot);
      }
      return;
    }

    if (target.kind === "storage") {
      if (source.kind === "inventory") {
        onStoreItem({ uniqueId: payload.uniqueId, slot: source.slot, container: source.container }, target.slot);
        return;
      }
      if (source.kind === "storage") {
        if (source.slot === target.slot) return;
        const occupant = world.storageItems.find((item) => item.slot === target.slot) ?? null;
        if (occupant && occupant.key === payload.key && occupant.uniqueId !== payload.uniqueId) {
          onMergeItem(
            { uniqueId: payload.uniqueId, slot: source.slot, container: "storage" },
            { uniqueId: occupant.uniqueId, slot: occupant.slot, container: "storage" },
          );
          return;
        }
        onMoveItem({ uniqueId: payload.uniqueId, slot: source.slot, container: "storage" }, target.slot);
      }
      return;
    }

    if (target.kind === "equipment") {
      if (source.kind === "inventory") {
        onEquipItem(
          { key: payload.key, uniqueId: payload.uniqueId, slot: source.slot, container: source.container },
          target.equipmentSlot,
        );
      }
      return;
    }

    if (target.kind === "belt") {
      if (source.kind === "belt") {
        if (source.slot === target.slot) return;
        const occupant = world.beltItems.find((item) => item.slot === target.slot) ?? null;
        if (occupant && occupant.key === payload.key && occupant.uniqueId !== payload.uniqueId) {
          onMergeItem(
            { uniqueId: payload.uniqueId, slot: source.slot, container: "belt" },
            { uniqueId: occupant.uniqueId, slot: occupant.slot, container: "belt" },
          );
          return;
        }
        onMoveItem({ uniqueId: payload.uniqueId, slot: source.slot, container: "belt" }, target.slot);
      }
      return;
    }

    if (target.kind === "trade") {
      if (source.kind === "inventory") {
        const from = source.container === "bag2" ? 40 + source.slot : source.slot;
        tradeHandlers.deposit(from, target.slot, {
          icon: payload.icon,
          name: payload.name,
          count: payload.quantity,
        });
      }
      return;
    }

    if (target.kind === "npcSell") {
      if (source.kind === "inventory" || source.kind === "belt") {
        onSellItem(
          {
            key: payload.key,
            uniqueId: payload.uniqueId,
            slot: source.slot,
            container: source.kind === "belt" ? "belt" : source.container,
          },
          payload.quantity,
        );
      }
      return;
    }

    if (target.kind === "ground") {
      if (source.kind === "inventory" || source.kind === "belt") {
        onDropItem({ key: payload.key, uniqueId: payload.uniqueId, slot: source.slot, container: source.kind === "belt" ? "belt" : source.container });
      }
    }
  }

  return (
    <ItemDragProvider onResolveDrop={resolveItemDrop}>
    <div
      className={`game-ui-scene ${hasOriginalMiniMapAsset(world.miniMapIndex) ? "with-mini-map" : "without-mini-map"}`}
      data-item-drop-kind="none"
    >
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
      <BuffBar t={t} buffs={world.activeBuffs} />
      <SkillBar t={t} knownSkills={world.knownSkills} onCastSkill={onCastSkill} />
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
        showOptions={showOptions}
        onToggleOptions={() => setShowOptions((current) => !current)}
      />
      {showMailPanel ? (
        <MailPanel
          t={t}
          mail={world.stage5Systems?.mail ?? []}
          onClaim={onClaimMail}
          onDelete={onDeleteMail}
          onSendMail={onSendMail}
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
      {showOptions ? (
        <OptionsDialog
          t={t}
          chatTransparent={transparentChat}
          onToggleChatTransparent={() => setTransparentChat((current) => !current)}
          onClose={() => setShowOptions(false)}
        />
      ) : null}
      {world.trade ? (
        <TradeDialog
          t={t}
          trade={world.trade}
          onSetGold={tradeHandlers.setGold}
          onSetLocked={tradeHandlers.setLocked}
          onCancel={tradeHandlers.cancel}
        />
      ) : null}
      {world.incomingTradeRequestFrom ? (
        <TradeRequestPrompt
          t={t}
          fromName={world.incomingTradeRequestFrom}
          onReply={tradeHandlers.reply}
        />
      ) : null}
      {world.npcShop ? (
        <NpcShopDialog
          t={t}
          shop={world.npcShop}
          gold={world.gold}
          onBuy={shopHandlers.buy}
          onClose={shopHandlers.close}
        />
      ) : null}
      {visibleDialog ? (
        <NpcDialogPanel
          t={t}
          dialog={visibleDialog}
          onClose={() => {
            onSelectNpcDialogTarget("@Exit");
            setDismissedDialogKey(dialogKey);
          }}
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
          paperdollSprite={paperdollSprite}
          world={world}
          onRemoveItem={onRemoveItem}
          onRepairItem={onRepairItem}
          onSpecialRepairItem={onSpecialRepairItem}
          onCastSkill={onCastSkill}
        />
      ) : null}
    </div>
    </ItemDragProvider>
  );
}
