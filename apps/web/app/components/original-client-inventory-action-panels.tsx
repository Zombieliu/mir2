import type { CSSProperties, MouseEvent } from "react";

import type { DisplayItem, TranslateFn } from "./original-client-types";
import { equipmentSlotForItemKey, originalItemIconPath } from "./original-client-inventory-utils";

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

/* ------------------------------------------------------------------------- */
/* Item-type filter                                                          */
/* ------------------------------------------------------------------------- */

/**
 * Coarse inventory categories used for the in-window type filter. These mirror
 * Crystal's broad item buckets (equipment / consumables / materials / quest)
 * without needing the server `ItemType` enum on the wire — the bucket is
 * inferred from the item key + equip heuristics already used for equipping.
 */
export type InventoryItemFilter = "all" | "equip" | "consumable" | "material" | "scroll" | "quest";

export const INVENTORY_ITEM_FILTERS: InventoryItemFilter[] = [
  "all",
  "equip",
  "consumable",
  "scroll",
  "material",
  "quest",
];

export function inventoryItemFilterLabel(t: TranslateFn, filter: InventoryItemFilter): string {
  switch (filter) {
    case "equip":
      return t("ui.filterEquip", [], "Equip");
    case "consumable":
      return t("ui.filterConsumable", [], "Use");
    case "scroll":
      return t("ui.filterScroll", [], "Scroll");
    case "material":
      return t("ui.filterMaterial", [], "Material");
    case "quest":
      return t("ui.filterQuest", [], "Quest");
    case "all":
    default:
      return t("ui.filterAll", [], "All");
  }
}

/** Classify an item into one of the coarse filter buckets. */
export function inventoryItemFilterFor(item: Pick<DisplayItem, "key" | "name" | "container">): InventoryItemFilter {
  if (item.container === "quest") return "quest";
  if (equipmentSlotForItemKey(item.key)) return "equip";
  const haystack = `${item.key} ${item.name}`;
  if (/Scroll|Book|Tome|Recall|Teleport|TownTeleport/i.test(haystack)) return "scroll";
  if (/Potion|Pill|Drug|Meat|Food|Drink|Apple|Wine|Herb|Antidote|Elixir|Cure/i.test(haystack)) return "consumable";
  if (/Ore|Gem|Crystal|Powder|Fragment|Material|Hide|Leather|Ingot|Wood|Stone|Bone|Feather|Essence|Dust/i.test(haystack)) {
    return "material";
  }
  return "all";
}

/** True when an item should be shown under the given active filter. */
export function inventoryItemMatchesFilter(
  item: Pick<DisplayItem, "key" | "name" | "container">,
  filter: InventoryItemFilter,
): boolean {
  if (filter === "all") return true;
  return inventoryItemFilterFor(item) === filter;
}

/* ------------------------------------------------------------------------- */
/* Inventory toolbar: sort / auto-arrange / type filter                      */
/* ------------------------------------------------------------------------- */

export type InventorySortKey = "name" | "type" | "quantity";

export function InventoryToolbar({
  t,
  activeFilter,
  onFilterChange,
  weightCurrent,
  weightMax,
  freeBagSlots,
  maxBagSlots,
  onSort,
  onAutoArrange,
}: {
  t: TranslateFn;
  activeFilter: InventoryItemFilter;
  onFilterChange: (filter: InventoryItemFilter) => void;
  weightCurrent: number;
  weightMax: number;
  /** Free (empty) inventory slots. */
  freeBagSlots: number;
  /** Total inventory slots. */
  maxBagSlots: number;
  /** Optional server-side reorder of the active bag; greyed when absent. */
  onSort?: (key: InventorySortKey) => void;
  /** Optional compaction of the active bag; greyed when absent. */
  onAutoArrange?: () => void;
}) {
  const usedSlots = Math.max(0, maxBagSlots - freeBagSlots);
  const weightPercent = weightMax > 0 ? Math.min(100, Math.max(0, (weightCurrent / weightMax) * 100)) : 0;
  const overweight = weightMax > 0 && weightCurrent > weightMax;

  return (
    <div style={toolbarStyle.root} aria-label={t("ui.inventoryToolbar", [], "Inventory tools")}>
      <div style={toolbarStyle.filterRow}>
        {INVENTORY_ITEM_FILTERS.map((filter) => (
          <button
            key={filter}
            type="button"
            style={{ ...toolbarStyle.filterButton, ...(filter === activeFilter ? toolbarStyle.filterButtonOn : null) }}
            aria-pressed={filter === activeFilter}
            onClick={() => onFilterChange(filter)}
          >
            {inventoryItemFilterLabel(t, filter)}
          </button>
        ))}
      </div>

      <div style={toolbarStyle.actionRow}>
        <button
          type="button"
          style={{ ...toolbarStyle.actionButton, ...(!onSort ? toolbarStyle.disabled : null) }}
          disabled={!onSort}
          title={t("ui.inventorySortName", [], "Sort by name")}
          onClick={() => onSort?.("name")}
        >
          {t("ui.inventorySort", [], "Sort")}
        </button>
        <button
          type="button"
          style={{ ...toolbarStyle.actionButton, ...(!onAutoArrange ? toolbarStyle.disabled : null) }}
          disabled={!onAutoArrange}
          title={t("ui.inventoryAutoArrange", [], "Auto-arrange")}
          onClick={() => onAutoArrange?.()}
        >
          {t("ui.inventoryArrange", [], "Arrange")}
        </button>
      </div>

      <div style={toolbarStyle.readouts}>
        <div style={toolbarStyle.readoutRow}>
          <span style={toolbarStyle.readoutLabel}>{t("ui.statBagSpace", [], "Bag Space")}</span>
          <span style={toolbarStyle.readoutValue}>{`${usedSlots}/${maxBagSlots}`}</span>
        </div>
        <div style={toolbarStyle.readoutRow}>
          <span style={toolbarStyle.readoutLabel}>{t("ui.statWeight", [], "Weight")}</span>
          <span style={{ ...toolbarStyle.readoutValue, ...(overweight ? toolbarStyle.readoutOver : null) }}>
            {`${Math.trunc(weightCurrent)}/${Math.trunc(weightMax)}`}
          </span>
        </div>
        <div style={toolbarStyle.weightTrack}>
          <span
            style={{
              ...toolbarStyle.weightFill,
              width: `${weightPercent}%`,
              background: overweight ? "#d98a8a" : "#caa64a",
            }}
          />
        </div>
      </div>
    </div>
  );
}

const toolbarStyle: Record<string, CSSProperties> = {
  root: {
    display: "flex",
    flexDirection: "column",
    gap: 4,
    padding: 4,
    fontSize: 10,
    color: "#e3d3af",
  },
  filterRow: { display: "flex", flexWrap: "wrap", gap: 2 },
  filterButton: {
    border: "1px solid rgba(190, 157, 99, 0.45)",
    background: "linear-gradient(180deg, rgba(40, 27, 15, 0.9), rgba(20, 13, 7, 0.9))",
    color: "#cbb38a",
    padding: "1px 5px",
    fontSize: 9,
    cursor: "pointer",
  },
  filterButtonOn: {
    borderColor: "rgba(214, 180, 110, 0.85)",
    background: "linear-gradient(180deg, rgba(95, 53, 24, 0.95), rgba(45, 23, 12, 0.95))",
    color: "#f4dcaf",
  },
  actionRow: { display: "flex", gap: 4 },
  actionButton: {
    flex: 1,
    border: "1px solid rgba(190, 157, 99, 0.5)",
    background: "linear-gradient(180deg, rgba(52, 32, 18, 0.92), rgba(28, 17, 9, 0.92))",
    color: "#e3d3af",
    padding: "2px 0",
    fontSize: 10,
    cursor: "pointer",
  },
  readouts: { display: "flex", flexDirection: "column", gap: 2 },
  readoutRow: { display: "flex", justifyContent: "space-between" },
  readoutLabel: { color: "#a89568" },
  readoutValue: { color: "#f4dcaf", fontWeight: 700 },
  readoutOver: { color: "#d98a8a" },
  weightTrack: {
    height: 4,
    border: "1px solid rgba(190, 157, 99, 0.4)",
    background: "rgba(11, 8, 5, 0.6)",
  },
  weightFill: { display: "block", height: "100%" },
  disabled: { opacity: 0.4, cursor: "default" },
};

/* ------------------------------------------------------------------------- */
/* Slot context menu: use / equip / split / drop / sell                      */
/* ------------------------------------------------------------------------- */

export type InventoryContextAction = "use" | "equip" | "move" | "split" | "drop" | "sell";

export function InventoryContextMenu({
  t,
  item,
  x,
  y,
  canEquip,
  canSplit,
  onAction,
  onClose,
}: {
  t: TranslateFn;
  item: DisplayItem;
  /** Position (relative to the inventory window) for the menu's top-left. */
  x: number;
  y: number;
  /** Whether the item maps to an equipment slot (shows Equip vs Use). */
  canEquip: boolean;
  /** Whether the stack can be split (quantity > 1). */
  canSplit: boolean;
  /** Fired with the chosen action; the host decides which callbacks exist. */
  onAction: (action: InventoryContextAction) => void;
  onClose: () => void;
}) {
  const rows: Array<{ action: InventoryContextAction; label: string; enabled: boolean }> = canEquip
    ? [
        { action: "equip", label: t("ui.equipItem", [], "Equip"), enabled: true },
        { action: "move", label: t("ui.moveItem", [], "Move"), enabled: true },
        { action: "drop", label: t("ui.deleteItem"), enabled: true },
        { action: "sell", label: t("ui.sellItem", [], "Sell Item"), enabled: true },
      ]
    : [
        { action: "use", label: t("ui.useItem", [], "Use"), enabled: true },
        { action: "move", label: t("ui.moveItem", [], "Move"), enabled: true },
        { action: "split", label: t("ui.splitItem", [], "Split Item"), enabled: canSplit },
        { action: "drop", label: t("ui.deleteItem"), enabled: true },
        { action: "sell", label: t("ui.sellItem", [], "Sell Item"), enabled: true },
      ];

  return (
    <div style={{ ...contextStyle.root, left: x, top: y }} role="menu" aria-label={item.name}>
      <div style={contextStyle.header}>{item.name}</div>
      {rows.map((row) => (
        <button
          key={row.action}
          type="button"
          role="menuitem"
          style={{ ...contextStyle.item, ...(row.enabled ? null : contextStyle.disabled) }}
          disabled={!row.enabled}
          onClick={() => {
            if (!row.enabled) return;
            onAction(row.action);
            onClose();
          }}
        >
          {row.label}
        </button>
      ))}
      <button type="button" role="menuitem" style={contextStyle.item} onClick={onClose}>
        {t("ui.close")}
      </button>
    </div>
  );
}

const contextStyle: Record<string, CSSProperties> = {
  root: {
    position: "absolute",
    minWidth: 96,
    zIndex: 60,
    border: "1px solid rgba(214, 180, 110, 0.85)",
    background: "linear-gradient(180deg, rgba(27, 19, 10, 0.98), rgba(11, 8, 5, 0.98))",
    boxShadow: "0 2px 6px rgba(0,0,0,0.6)",
    display: "flex",
    flexDirection: "column",
    padding: 2,
  },
  header: {
    padding: "2px 6px",
    fontSize: 10,
    fontWeight: 700,
    color: "#f4dcaf",
    borderBottom: "1px solid rgba(190, 157, 99, 0.3)",
    marginBottom: 2,
    whiteSpace: "nowrap",
  },
  item: {
    textAlign: "left",
    border: "none",
    background: "transparent",
    color: "#e3d3af",
    padding: "3px 8px",
    fontSize: 11,
    cursor: "pointer",
  },
  disabled: { opacity: 0.4, cursor: "default" },
};
