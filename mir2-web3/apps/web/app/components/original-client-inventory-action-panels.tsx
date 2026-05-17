import type { MouseEvent } from "react";

import type { DisplayItem, TranslateFn } from "./original-client-types";
import { originalItemIconPath } from "./original-client-inventory-utils";

function primaryMouseAction(event: MouseEvent, action: () => void) {
  if (event.button !== 0) return;
  event.preventDefault();
  action();
}

export function InventoryDeletePanel({
  t,
  item,
  onConfirm,
  onClose,
}: {
  t: TranslateFn;
  item: DisplayItem;
  onConfirm: () => void;
  onClose: () => void;
}) {
  return (
    <div className="inventory-delete-panel">
      <strong>{t("ui.deleteItem")}</strong>
      <div className="inventory-delete-preview">
        <img className="original-item-icon inventory-delete-icon" src={originalItemIconPath(item.icon)} alt="" draggable={false} />
      </div>
      <span>{item.name}</span>
      <InventoryActionButtons t={t} onConfirm={onConfirm} onClose={onClose} />
    </div>
  );
}

export function InventorySellPanel({
  t,
  item,
  onConfirm,
  onClose,
}: {
  t: TranslateFn;
  item: DisplayItem;
  onConfirm: () => void;
  onClose: () => void;
}) {
  return (
    <div className="inventory-delete-panel">
      <strong>{t("ui.sellItem", [], "Sell Item")}</strong>
      <div className="inventory-delete-preview">
        <img className="original-item-icon inventory-delete-icon" src={originalItemIconPath(item.icon)} alt="" draggable={false} />
      </div>
      <span>{item.name}</span>
      <InventoryActionButtons t={t} onConfirm={onConfirm} onClose={onClose} />
    </div>
  );
}

export function InventorySplitPanel({
  t,
  item,
  splitCount,
  onSplitCountChange,
  onConfirm,
  onClose,
}: {
  t: TranslateFn;
  item: DisplayItem;
  splitCount: string;
  onSplitCountChange: (value: string) => void;
  onConfirm: () => void;
  onClose: () => void;
}) {
  return (
    <div className="inventory-delete-panel">
      <strong>{t("ui.splitItem", [], "Split Item")}</strong>
      <div className="inventory-delete-preview">
        <img className="original-item-icon inventory-delete-icon" src={originalItemIconPath(item.icon)} alt="" draggable={false} />
      </div>
      <span>{item.name}</span>
      <input
        type="number"
        min="1"
        max={Math.max(1, item.quantity - 1)}
        value={splitCount}
        onChange={(event) => onSplitCountChange(event.target.value)}
      />
      <InventoryActionButtons t={t} onConfirm={onConfirm} onClose={onClose} />
    </div>
  );
}

export function InventoryGoldDropPanel({
  t,
  goldDropAmount,
  onGoldDropAmountChange,
  onConfirm,
  onClose,
}: {
  t: TranslateFn;
  goldDropAmount: string;
  onGoldDropAmountChange: (value: string) => void;
  onConfirm: () => void;
  onClose: () => void;
}) {
  return (
    <div className="inventory-delete-panel">
      <strong>{t("ui.dropGold", [], "Drop Gold")}</strong>
      <input
        type="number"
        min="1"
        value={goldDropAmount}
        onChange={(event) => onGoldDropAmountChange(event.target.value)}
      />
      <InventoryActionButtons t={t} onConfirm={onConfirm} onClose={onClose} />
    </div>
  );
}

function InventoryActionButtons({
  t,
  onConfirm,
  onClose,
}: {
  t: TranslateFn;
  onConfirm: () => void;
  onClose: () => void;
}) {
  return (
    <div className="inventory-delete-actions">
      <button
        type="button"
        onMouseDown={(event) => primaryMouseAction(event, onConfirm)}
        onClick={(event) => {
          if (event.detail !== 0) return;
          onConfirm();
        }}
      >
        {t("ui.confirm", [], "Confirm")}
      </button>
      <button type="button" onClick={onClose}>
        {t("ui.close")}
      </button>
    </div>
  );
}
