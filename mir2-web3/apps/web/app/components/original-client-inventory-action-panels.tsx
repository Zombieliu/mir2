import { useEffect, useRef, type MouseEvent, type PointerEvent as ReactPointerEvent } from "react";

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
      <MirAmountBox
        t={t}
        value={splitCount}
        onChange={onSplitCountChange}
        min={1}
        max={Math.max(1, item.quantity - 1)}
      />
      <InventoryActionButtons t={t} onConfirm={onConfirm} onClose={onClose} />
    </div>
  );
}

export function InventoryGoldDropPanel({
  t,
  goldDropAmount,
  onGoldDropAmountChange,
  availableGold,
  onConfirm,
  onClose,
}: {
  t: TranslateFn;
  goldDropAmount: string;
  onGoldDropAmountChange: (value: string) => void;
  availableGold?: number;
  onConfirm: () => void;
  onClose: () => void;
}) {
  return (
    <div className="inventory-delete-panel">
      <strong>{t("ui.dropGold", [], "Drop Gold")}</strong>
      <MirAmountBox
        t={t}
        value={goldDropAmount}
        onChange={onGoldDropAmountChange}
        min={1}
        max={typeof availableGold === "number" && availableGold > 0 ? availableGold : undefined}
      />
      <InventoryActionButtons t={t} onConfirm={onConfirm} onClose={onClose} />
    </div>
  );
}

// Crystal's MirAmountBox: a -/+ spinner (press-and-hold to repeat), a drag
// slider, a numeric field, and a Max button. Replaces the bare number inputs
// the split / drop-gold prompts used before.
function MirAmountBox({
  t,
  value,
  onChange,
  min,
  max,
}: {
  t: TranslateFn;
  value: string;
  onChange: (value: string) => void;
  min: number;
  max?: number;
}) {
  const hasBoundedMax = typeof max === "number" && Number.isFinite(max) && max > min;
  const current = clampAmount(Number.parseInt(value || "0", 10), min, max);
  const valueRef = useRef(value);
  valueRef.current = value;

  const setAmount = (next: number) => onChange(String(clampAmount(next, min, max)));
  const stepBy = (delta: number) => {
    const base = clampAmount(Number.parseInt(valueRef.current || "0", 10), min, max);
    onChange(String(clampAmount(base + delta, min, max)));
  };

  const decrement = useHoldRepeat(() => stepBy(-1));
  const increment = useHoldRepeat(() => stepBy(1));

  return (
    <div className="mir-amount-box">
      <div className="mir-amount-row">
        <button
          type="button"
          className="mir-amount-step minus"
          aria-label={t("ui.decrease", [], "Decrease")}
          onPointerDown={decrement.start}
          onPointerUp={decrement.stop}
          onPointerLeave={decrement.stop}
          onPointerCancel={decrement.stop}
        >
          -
        </button>
        <input
          className="mir-amount-value"
          type="number"
          min={min}
          max={hasBoundedMax ? max : undefined}
          value={value}
          onChange={(event) => onChange(event.target.value)}
        />
        <button
          type="button"
          className="mir-amount-step plus"
          aria-label={t("ui.increase", [], "Increase")}
          onPointerDown={increment.start}
          onPointerUp={increment.stop}
          onPointerLeave={increment.stop}
          onPointerCancel={increment.stop}
        >
          +
        </button>
        {hasBoundedMax ? (
          <button
            type="button"
            className="mir-amount-max"
            onClick={() => setAmount(max as number)}
          >
            {t("ui.max", [], "Max")}
          </button>
        ) : null}
      </div>
      {hasBoundedMax ? (
        <input
          className="mir-amount-slider"
          type="range"
          min={min}
          max={max}
          value={current}
          onChange={(event) => setAmount(Number(event.target.value))}
        />
      ) : null}
    </div>
  );
}

function clampAmount(value: number, min: number, max?: number) {
  let next = Number.isFinite(value) ? Math.round(value) : min;
  if (next < min) next = min;
  if (typeof max === "number" && Number.isFinite(max) && next > max) next = max;
  return next;
}

// Press-and-hold accelerating repeat for the -/+ spinner buttons.
function useHoldRepeat(action: () => void) {
  const actionRef = useRef(action);
  actionRef.current = action;
  const timerRef = useRef<number | null>(null);

  const stop = () => {
    if (timerRef.current !== null) {
      window.clearTimeout(timerRef.current);
      timerRef.current = null;
    }
  };

  const start = (event: ReactPointerEvent) => {
    if (event.button !== 0) return;
    event.preventDefault();
    actionRef.current();
    let delay = 320;
    const tick = () => {
      actionRef.current();
      delay = Math.max(45, delay - 45);
      timerRef.current = window.setTimeout(tick, delay);
    };
    timerRef.current = window.setTimeout(tick, delay);
  };

  useEffect(() => stop, []);

  return { start, stop };
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
