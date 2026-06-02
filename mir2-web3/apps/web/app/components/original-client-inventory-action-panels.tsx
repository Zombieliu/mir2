import type { CSSProperties, MouseEvent } from "react";

import type { DisplayItem, TranslateFn } from "./original-client-types";
import { originalItemIconPath } from "./original-client-inventory-utils";

function primaryMouseAction(event: MouseEvent, action: () => void) {
  if (event.button !== 0) return;
  event.preventDefault();
  action();
}

function clampSplit(value: number, max: number) {
  if (!Number.isFinite(value)) return 1;
  return Math.max(1, Math.min(max, Math.trunc(value)));
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
  // A stack can be split into 1..(quantity-1); the remainder stays behind.
  const maxSplit = Math.max(1, item.quantity - 1);
  const parsed = Number.parseInt(splitCount, 10);
  const count = clampSplit(Number.isNaN(parsed) ? 1 : parsed, maxSplit);
  const remaining = item.quantity - count;
  const setCount = (value: number) => onSplitCountChange(String(clampSplit(value, maxSplit)));

  return (
    <div className="inventory-delete-panel">
      <strong>{t("ui.splitItem", [], "Split Item")}</strong>
      <div className="inventory-delete-preview">
        <img className="original-item-icon inventory-delete-icon" src={originalItemIconPath(item.icon)} alt="" draggable={false} />
      </div>
      <span>{item.name}</span>

      <div style={splitStyle.stepperRow}>
        <button
          type="button"
          aria-label={t("ui.splitDecrease", [], "Less")}
          style={splitStyle.stepperButton}
          disabled={count <= 1}
          onClick={() => setCount(count - 1)}
        >
          −
        </button>
        <input
          type="number"
          min="1"
          max={maxSplit}
          value={splitCount}
          aria-label={t("ui.splitItem", [], "Split Item")}
          style={splitStyle.input}
          onChange={(event) => onSplitCountChange(event.target.value)}
          onBlur={() => onSplitCountChange(String(count))}
        />
        <button
          type="button"
          aria-label={t("ui.splitIncrease", [], "More")}
          style={splitStyle.stepperButton}
          disabled={count >= maxSplit}
          onClick={() => setCount(count + 1)}
        >
          +
        </button>
      </div>

      <input
        type="range"
        min={1}
        max={maxSplit}
        step={1}
        value={count}
        aria-label={t("ui.splitItem", [], "Split Item")}
        aria-valuetext={String(count)}
        style={splitStyle.slider}
        onChange={(event) => setCount(Number(event.target.value))}
      />

      <div style={splitStyle.quickRow}>
        <button type="button" style={splitStyle.quickButton} onClick={() => setCount(1)}>
          {t("ui.splitMin", [], "Min")}
        </button>
        <button type="button" style={splitStyle.quickButton} onClick={() => setCount(Math.ceil(item.quantity / 2))}>
          {t("ui.splitHalf", [], "Half")}
        </button>
        <button type="button" style={splitStyle.quickButton} onClick={() => setCount(maxSplit)}>
          {t("ui.splitMax", [], "Max")}
        </button>
      </div>

      <span style={splitStyle.summary}>
        {t("ui.splitSummary", [count, remaining], `Move ${count}, keep ${remaining}`)}
      </span>

      <InventoryActionButtons t={t} onConfirm={onConfirm} onClose={onClose} />
    </div>
  );
}

const splitStyle: Record<string, CSSProperties> = {
  stepperRow: { display: "flex", alignItems: "center", gap: 4 },
  stepperButton: {
    flex: "0 0 22px",
    height: 22,
    border: "1px solid rgba(190, 157, 99, 0.56)",
    background: "rgba(19, 12, 8, 0.92)",
    color: "#f4dcaf",
    fontSize: 14,
    lineHeight: "18px",
    cursor: "pointer",
  },
  input: {
    flex: 1,
    minWidth: 0,
    border: "1px solid rgba(190, 157, 99, 0.56)",
    background: "rgba(19, 12, 8, 0.92)",
    color: "#f4dcaf",
    padding: "4px 6px",
    fontSize: 11,
    textAlign: "center",
  },
  slider: { width: "100%", accentColor: "#caa64a" },
  quickRow: { display: "flex", gap: 4 },
  quickButton: {
    flex: 1,
    border: "1px solid rgba(190, 157, 99, 0.5)",
    background: "linear-gradient(180deg, rgba(52, 32, 18, 0.92), rgba(28, 17, 9, 0.92))",
    color: "#e3d3af",
    padding: "2px 0",
    fontSize: 10,
    cursor: "pointer",
  },
  summary: { fontSize: 10, color: "#cbb38a" },
};

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
  const parsed = Number.parseInt(goldDropAmount, 10);
  const amount = Number.isFinite(parsed) && parsed > 0 ? parsed : 0;
  const setAmount = (value: number) => onGoldDropAmountChange(String(Math.max(1, Math.trunc(value))));

  return (
    <div className="inventory-delete-panel">
      <strong>{t("ui.dropGold", [], "Drop Gold")}</strong>
      <input
        type="number"
        min="1"
        value={goldDropAmount}
        aria-label={t("ui.dropGold", [], "Drop Gold")}
        onChange={(event) => onGoldDropAmountChange(event.target.value)}
      />
      <div style={splitStyle.quickRow}>
        <button type="button" style={splitStyle.quickButton} onClick={() => setAmount(Math.max(1, amount * 10 || 100))}>
          ×10
        </button>
        <button type="button" style={splitStyle.quickButton} onClick={() => setAmount(Math.max(1, Math.floor(amount / 10)))}>
          ÷10
        </button>
        <button type="button" style={splitStyle.quickButton} onClick={() => onGoldDropAmountChange("1")}>
          {t("ui.splitMin", [], "Min")}
        </button>
      </div>
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
