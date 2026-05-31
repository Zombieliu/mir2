"use client";

import { useEffect, useState } from "react";

import { ORIGINAL_UI, type InventoryTabKey } from "../../lib/original-ui";
import {
  sameItemDragSource,
  useItemDrag,
  type InventoryDragContainer,
  type ItemDragPayload,
  type ItemDragSource,
} from "./original-client-drag-item";
import {
  InventoryDeletePanel,
  InventoryGoldDropPanel,
  InventorySellPanel,
  InventorySplitPanel,
} from "./original-client-inventory-action-panels";
import {
  equipmentSlotForItemKey,
  formatBinaryDateTimeLabel,
  originalItemIconPath,
} from "./original-client-inventory-utils";
import { OriginalItemTooltip } from "./original-client-item-tooltip";
import { SpriteButton } from "./original-client-overlays";
import { StoragePasswordPanel, type StoragePasswordPanelMode } from "./original-client-storage-password-panel";
import type {
  DisplayItem,
  DisplayWorld,
  EquipmentSlot,
  ItemActionRef,
  MergeItemRef,
  MoveItemRef,
  TranslateFn,
} from "./original-client-types";

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
  const itemDrag = useItemDrag();
  const [deleteMode, setDeleteMode] = useState(false);
  const [sellMode, setSellMode] = useState(false);
  const [storageMode, setStorageMode] = useState<"store" | "takeBack" | null>(null);
  const [storagePageIndex, setStoragePageIndex] = useState<0 | 1>(0);
  const [showStoragePasswordPanel, setShowStoragePasswordPanel] = useState(false);
  const [storagePasswordPanelMode, setStoragePasswordPanelMode] = useState<StoragePasswordPanelMode>("unlock");
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
            data-item-drop-kind="inventory"
            data-item-drop-container={activeTab}
            data-item-drop-slot={slotIndex}
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
              className={`inventory-item-card ${
                itemDrag && sameItemDragSource(itemDrag.draggingSource, inventoryItemSource(item)) ? "item-dragging" : ""
              }`}
              style={{ left: slot.x, top: slot.y }}
              aria-label={item.name}
              data-item-drop-kind="inventory"
              data-item-drop-container={item.container}
              data-item-drop-slot={item.slot}
              onPointerDown={(event) =>
                itemDrag?.beginDrag(event, inventoryItemPayload(item), () => activateInventoryItem(item))
              }
              onClick={(event) => {
                if (event.detail !== 0 && itemDrag) return;
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
              <OriginalItemTooltip
                t={t}
                name={item.name}
                description={item.description}
                quantity={item.quantity}
                durabilityCurrent={item.durabilityCurrent}
                durabilityMax={item.durabilityMax}
                grade={item.grade}
                attack={item.attack}
                defence={item.defence}
                addedAttack={item.addedAttack}
                addedDefence={item.addedDefence}
                weight={item.weight}
                addedStats={item.addedStats}
                requiredType={item.requiredType}
                requiredClass={item.requiredClass}
                requiredAmount={item.requiredAmount}
                price={item.price}
                align={slot.x > 210 ? "left" : "right"}
              />
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
	                  className="storage-slot" data-item-drop-kind="storage" data-item-drop-slot={absoluteSlot}
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
                  className={`storage-item-card ${itemDrag && sameItemDragSource(itemDrag.draggingSource, storageItemSource(item)) ? "item-dragging " : ""}${
                    pendingMoveItem?.container === "storage" && pendingMoveItem.slot === item.slot
                      ? "selected"
                      : ""
                  }`}
                  style={{ left: slot.x, top: slot.y }}
                  aria-label={item.name}
                  data-item-drop-kind="storage"
                  data-item-drop-slot={item.slot}
                  onPointerDown={(event) => itemDrag?.beginDrag(event, storageItemPayload(item))}
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
                  <OriginalItemTooltip
                    t={t}
                    name={item.name}
                    description={item.description}
                    quantity={item.quantity}
                    durabilityCurrent={item.durabilityCurrent}
                    durabilityMax={item.durabilityMax}
                grade={item.grade}
                attack={item.attack}
                defence={item.defence}
                addedAttack={item.addedAttack}
                addedDefence={item.addedDefence}
                weight={item.weight}
                addedStats={item.addedStats}
                requiredType={item.requiredType}
                requiredClass={item.requiredClass}
                requiredAmount={item.requiredAmount}
                price={item.price}
                    align={slot.x > 270 ? "left" : "right"}
                  />
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
            <StoragePasswordPanel
              t={t}
              title={storagePasswordPanelTitle}
              prompt={storagePasswordPanelPrompt}
              statusText={storageStatusText}
              lastSetText={storagePasswordLastSetText}
              mode={effectiveStoragePasswordPanelMode}
              hasStoragePassword={Boolean(world.hasStoragePassword)}
              storageLocked={storageLocked}
              storagePassword={storagePassword}
              newStoragePassword={newStoragePassword}
              confirmStoragePassword={confirmStoragePassword}
              passwordConfirmationMismatch={passwordConfirmationMismatch}
              passwordRules={storagePasswordRules}
              selectedMode={storagePasswordPanelMode}
              onModeChange={(mode) => {
                setStoragePasswordPanelMode(mode);
                setNewStoragePassword("");
                setConfirmStoragePassword("");
              }}
              onStoragePasswordChange={setStoragePassword}
              onNewStoragePasswordChange={setNewStoragePassword}
              onConfirmStoragePasswordChange={setConfirmStoragePassword}
              onSubmit={submitStoragePasswordPanel}
              canSubmit={storagePasswordPanelCanSubmit()}
              onClose={closeStoragePasswordPanel}
            />
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
        <InventoryDeletePanel
          t={t}
          item={pendingDeleteItem}
          onConfirm={() => confirmDeleteItem(pendingDeleteItem)}
          onClose={() => setPendingDeleteItem(null)}
        />
      ) : null}
      {pendingSellItem ? (
        <InventorySellPanel
          t={t}
          item={pendingSellItem}
          onConfirm={() => confirmSellItem(pendingSellItem)}
          onClose={() => setPendingSellItem(null)}
        />
      ) : null}
      {pendingSplitItem ? (
        <InventorySplitPanel
          t={t}
          item={pendingSplitItem}
          splitCount={splitCount}
          onSplitCountChange={setSplitCount}
          onConfirm={() => confirmSplitItem(pendingSplitItem)}
          onClose={() => setPendingSplitItem(null)}
        />
      ) : null}
      {pendingGoldDrop ? (
        <InventoryGoldDropPanel
          t={t}
          goldDropAmount={goldDropAmount}
          onGoldDropAmountChange={setGoldDropAmount}
          availableGold={world.gold}
          onConfirm={confirmGoldDrop}
          onClose={() => setPendingGoldDrop(false)}
        />
      ) : null}
      {deleteFeedback ? <div className="inventory-delete-feedback">{deleteFeedback}</div> : null}
    </div>
  );
}

function inventoryItemSource(item: DisplayItem): ItemDragSource {
  return {
    kind: "inventory",
    container: item.container as InventoryDragContainer,
    slot: item.slot,
  };
}

function inventoryItemPayload(item: DisplayItem): ItemDragPayload {
  return {
    uniqueId: item.uniqueId,
    key: item.key,
    name: item.name,
    icon: item.icon,
    quantity: item.quantity,
    source: inventoryItemSource(item),
  };
}

function storageItemSource(item: DisplayItem): ItemDragSource {
  return { kind: "storage", slot: item.slot };
}

function storageItemPayload(item: DisplayItem): ItemDragPayload {
  return {
    uniqueId: item.uniqueId,
    key: item.key,
    name: item.name,
    icon: item.icon,
    quantity: item.quantity,
    source: storageItemSource(item),
  };
}
