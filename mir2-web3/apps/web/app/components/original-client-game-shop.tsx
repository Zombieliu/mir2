"use client";

import { useEffect, useMemo, useState, type CSSProperties } from "react";

import { ORIGINAL_UI } from "../../lib/original-ui";
import {
  CRYSTAL_GAME_SHOP_ITEM_INFO_BY_INDEX,
  CRYSTAL_GAME_SHOP_ITEMS,
} from "../../lib/generated/crystal-game-shop-data";
import { originalItemIconPath } from "./original-client-inventory-utils";
import type { ItemTooltipGrade } from "./original-client-item-tooltip";
import { SpriteButton } from "./original-client-overlays";

type TranslateFn = (
  key: string,
  params?: Array<string | number>,
  fallback?: string,
) => string;

type EntityClassKey = "warrior" | "wizard" | "taoist" | "assassin" | "archer";

type CrystalGameShopEntry = {
  item_index: number;
  game_shop_index: number;
  item_name: string;
  gold_price: number;
  credit_price: number;
  count: number;
  class: string;
  category: string;
  stock: number;
  stock_level: number;
};

type CrystalItemEntry = {
  image: number;
  item_type: number;
};

type GameShopSectionFilter = "all" | "top" | "deals" | "new";
type GameShopClassFilter = "all" | EntityClassKey;
type GameShopPaymentType = "gold" | "credit";

const GAME_SHOP_ITEMS_PER_PAGE = 8;
const GAME_SHOP_CLASS_FILTERS: GameShopClassFilter[] = ["all", "warrior", "assassin", "taoist", "wizard", "archer"];
const GAME_SHOP_PREVIEW_ITEM_TYPES = new Set([1, 2, 19, 37]);

export function GameShopWindow({
  t,
  gold,
  credits,
  playerClass,
  onBuy,
  onClose,
}: {
  t: TranslateFn;
  gold: number;
  credits: number;
  playerClass: EntityClassKey;
  onBuy: (gameShopIndex: number, quantity: number, paymentType: GameShopPaymentType) => void;
  onClose: () => void;
}) {
  const [sectionFilter, setSectionFilter] = useState<GameShopSectionFilter>("all");
  const [classFilter, setClassFilter] = useState<GameShopClassFilter>(playerClass);
  const [categoryFilter, setCategoryFilter] = useState("Show All");
  const [search, setSearch] = useState("");
  const [page, setPage] = useState(0);
  const [paymentType, setPaymentType] = useState<GameShopPaymentType>("gold");
  const [quantities, setQuantities] = useState<Record<number, number>>({});
  const [preview, setPreview] = useState<{ item: CrystalGameShopEntry; cellLeft: number } | null>(null);

  const sectionItems = useMemo(
    () => applyGameShopSectionFilter(CRYSTAL_GAME_SHOP_ITEMS, sectionFilter),
    [sectionFilter],
  );
  const classItems = useMemo(
    () => sectionItems.filter((item) => gameShopClassMatches(item.class, classFilter)),
    [sectionItems, classFilter],
  );
  const searchQuery = search.trim().toLowerCase();
  const searchedItems = useMemo(
    () =>
      classItems.filter((item) =>
        searchQuery ? item.item_name.toLowerCase().includes(searchQuery) : true,
      ),
    [classItems, searchQuery],
  );
  const categories = useMemo(
    () => [
      "Show All",
      ...Array.from(new Set(searchedItems.map((item) => item.category))).sort((left, right) =>
        left.localeCompare(right),
      ),
    ],
    [searchedItems],
  );
  const filteredItems = useMemo(
    () =>
      searchedItems
        .filter((item) => categoryFilter === "Show All" || item.category === categoryFilter)
        .slice()
        .sort(compareGameShopItems),
    [searchedItems, categoryFilter],
  );
  const pageCount = Math.max(1, Math.ceil(filteredItems.length / GAME_SHOP_ITEMS_PER_PAGE));
  const currentPage = Math.min(page, pageCount - 1);
  const visibleItems = filteredItems.slice(
    currentPage * GAME_SHOP_ITEMS_PER_PAGE,
    currentPage * GAME_SHOP_ITEMS_PER_PAGE + GAME_SHOP_ITEMS_PER_PAGE,
  );

  useEffect(() => {
    setClassFilter(playerClass);
  }, [playerClass]);

  useEffect(() => {
    setPage(0);
    if (!categories.includes(categoryFilter)) {
      setCategoryFilter("Show All");
    }
  }, [categories, categoryFilter]);

  useEffect(() => {
    if (page > pageCount - 1) {
      setPage(pageCount - 1);
    }
  }, [page, pageCount]);

  const setQuantity = (gameShopIndex: number, nextQuantity: number) => {
    setQuantities((current) => ({
      ...current,
      [gameShopIndex]: Math.max(1, Math.min(99, nextQuantity)),
    }));
  };

  const showPreview = (item: CrystalGameShopEntry, cellLeft: number) => {
    setPreview({ item, cellLeft });
  };

  return (
    <section className="game-shop-window" aria-label={t("ui.gameShop")}>
      <img className="game-shop-frame" src={ORIGINAL_UI.gameShop.frame} alt="" draggable={false} />
      <img className="game-shop-title" src={ORIGINAL_UI.gameShop.title} alt="GAMESHOP" draggable={false} />
      <div className="game-shop-close">
        <SpriteButton sprite={ORIGINAL_UI.gameShop.closeButton} label={t("ui.close")} onClick={onClose} />
      </div>
      <img className="game-shop-filter-bg" src={ORIGINAL_UI.gameShop.filterBackground} alt="" draggable={false} />
      <div className="game-shop-scroll up">
        <SpriteButton sprite={ORIGINAL_UI.gameShop.upButton} label={t("ui.up", [], "Up")} onClick={() => setPage((current) => Math.max(0, current - 1))} disabled={currentPage <= 0} />
      </div>
      <div className="game-shop-scroll thumb">
        <SpriteButton sprite={ORIGINAL_UI.gameShop.positionBar} label={t("ui.scroll", [], "Scroll")} disabled />
      </div>
      <div className="game-shop-scroll down">
        <SpriteButton sprite={ORIGINAL_UI.gameShop.downButton} label={t("ui.down", [], "Down")} onClick={() => setPage((current) => Math.min(pageCount - 1, current + 1))} disabled={currentPage >= pageCount - 1} />
      </div>
      <div className="game-shop-section all">
        <SpriteButton sprite={ORIGINAL_UI.gameShop.sectionTabs.all} label="All" active={sectionFilter === "all"} onClick={() => setSectionFilter("all")} />
      </div>
      <div className="game-shop-section top">
        <SpriteButton sprite={ORIGINAL_UI.gameShop.sectionTabs.top} label="Top" active={sectionFilter === "top"} onClick={() => setSectionFilter("top")} />
      </div>
      <div className="game-shop-section deals">
        <SpriteButton sprite={ORIGINAL_UI.gameShop.sectionTabs.deals} label="Deals" active={sectionFilter === "deals"} onClick={() => setSectionFilter("deals")} />
      </div>
      <div className="game-shop-section new">
        <SpriteButton sprite={ORIGINAL_UI.gameShop.sectionTabs.newItems} label="New" active={sectionFilter === "new"} onClick={() => setSectionFilter("new")} />
      </div>
      <div className="game-shop-class-tabs">
        {GAME_SHOP_CLASS_FILTERS.map((key, index) => (
          <div key={key} style={{ left: `${index === 0 ? 0 : 29 + (index - 1) * 23}px` }}>
            <SpriteButton
              sprite={ORIGINAL_UI.gameShop.classTabs[key]}
              label={key}
              active={classFilter === key}
              onClick={() => setClassFilter(key)}
            />
          </div>
        ))}
      </div>
      <input
        className="game-shop-search"
        aria-label="Search"
        value={search}
        onChange={(event) => setSearch(event.target.value)}
        spellCheck={false}
      />
      <div className="game-shop-categories">
        {categories.map((category) => (
          <button
            key={category}
            type="button"
            className={category === categoryFilter ? "active" : ""}
            onClick={() => setCategoryFilter(category)}
          >
            {category}
          </button>
        ))}
      </div>
      <div className="game-shop-cells">
        {visibleItems.map((item, index) => (
          <GameShopCell
            key={item.game_shop_index}
            item={item}
            index={index}
            quantity={quantities[item.game_shop_index] ?? 1}
            onQuantityChange={(nextQuantity) => setQuantity(item.game_shop_index, nextQuantity)}
            onBuy={() => onBuy(item.game_shop_index, quantities[item.game_shop_index] ?? 1, paymentType)}
            onPreview={(cellLeft) => showPreview(item, cellLeft)}
            t={t}
          />
        ))}
      </div>
      {preview ? (
        <GameShopViewer
          item={preview.item}
          left={preview.cellLeft < 350 ? 416 : 151}
          top={115}
          t={t}
          onClose={() => setPreview(null)}
        />
      ) : null}
      <div className="game-shop-total credits">{credits}</div>
      <div className="game-shop-total gold">{gold}</div>
      <button type="button" className="game-shop-payment gold" onClick={() => setPaymentType("gold")}>
        <img src={paymentType === "gold" ? ORIGINAL_UI.gameShop.paymentBox.checked : ORIGINAL_UI.gameShop.paymentBox.unchecked} alt="" draggable={false} />
        <span>Gold</span>
      </button>
      <button type="button" className="game-shop-payment credit" onClick={() => setPaymentType("credit")}>
        <img src={paymentType === "credit" ? ORIGINAL_UI.gameShop.paymentBox.checked : ORIGINAL_UI.gameShop.paymentBox.unchecked} alt="" draggable={false} />
        <span>Credits</span>
      </button>
      <div className="game-shop-page">{currentPage + 1} / {pageCount}</div>
      <div className="game-shop-page-button previous">
        <SpriteButton
          sprite={ORIGINAL_UI.gameShop.previousButton}
          label={t("ui.previous", [], "Previous")}
          onClick={() => setPage((current) => Math.max(0, current - 1))}
          disabled={currentPage <= 0}
        />
      </div>
      <div className="game-shop-page-button next">
        <SpriteButton
          sprite={ORIGINAL_UI.gameShop.nextButton}
          label={t("ui.next", [], "Next")}
          onClick={() => setPage((current) => Math.min(pageCount - 1, current + 1))}
          disabled={currentPage >= pageCount - 1}
        />
      </div>
    </section>
  );
}

function GameShopCell({
  item,
  index,
  quantity,
  onQuantityChange,
  onBuy,
  onPreview,
  t,
}: {
  item: CrystalGameShopEntry;
  index: number;
  quantity: number;
  onQuantityChange: (quantity: number) => void;
  onBuy: () => void;
  onPreview: (cellLeft: number) => void;
  t: TranslateFn;
}) {
  const info = gameShopItemInfo(item.item_index);
  const left = index < 4 ? 152 + index * 132 : 152 + (index - 4) * 132;
  const top = index < 4 ? 115 : 275;
  const hasPreview = Boolean(info && GAME_SHOP_PREVIEW_ITEM_TYPES.has(info.item_type));
  const displayName = truncateGameShopName(item.item_name);

  return (
    <div className="game-shop-cell-frame" style={{ left, top }}>
      <img className="game-shop-cell-bg" src={ORIGINAL_UI.gameShop.cellFrame} alt="" draggable={false} />
      <div className="game-shop-cell-name" title={item.item_name}>{displayName}</div>
      {info ? (
        <img
          className="game-shop-cell-icon"
          src={originalItemIconPath(info.image)}
          alt=""
          draggable={false}
        />
      ) : null}
      <div className="game-shop-cell-stock-label">STOCK:</div>
      <div className="game-shop-cell-stock-value">{formatGameShopStock(item.stock)}</div>
      <div className="game-shop-cell-count">{item.count > 1 ? item.count : ""}</div>
      <div className="game-shop-cell-quantity-down">
        <SpriteButton sprite={ORIGINAL_UI.gameShop.previousButton} label={t("ui.down", [], "Down")} onClick={() => onQuantityChange(quantity - 1)} />
      </div>
      <div className="game-shop-cell-quantity">{quantity}</div>
      <div className="game-shop-cell-quantity-up">
        <SpriteButton sprite={ORIGINAL_UI.gameShop.nextButton} label={t("ui.up", [], "Up")} onClick={() => onQuantityChange(quantity + 1)} />
      </div>
      <div className="game-shop-cell-credit-price">{item.credit_price * quantity}</div>
      <div className="game-shop-cell-gold-price">{item.gold_price * quantity}</div>
      {hasPreview ? (
        <div className="game-shop-cell-preview">
          <SpriteButton sprite={ORIGINAL_UI.gameShop.previewButton} label={t("ui.preview", [], "Preview")} onClick={() => onPreview(left)} />
        </div>
      ) : null}
      <div className={hasPreview ? "game-shop-cell-buy with-preview" : "game-shop-cell-buy"}>
        <SpriteButton sprite={ORIGINAL_UI.gameShop.buyButton} label={t("ui.buy", [], "Buy")} onClick={onBuy} />
      </div>
    </div>
  );
}

function GameShopViewer({
  item,
  left,
  top,
  t,
  onClose,
}: {
  item: CrystalGameShopEntry;
  left: number;
  top: number;
  t: TranslateFn;
  onClose: () => void;
}) {
  const [direction, setDirection] = useState(6);
  const info = gameShopItemInfo(item.item_index);

  return (
    <div
      className="game-shop-viewer"
      style={{ left, top }}
      data-item-name={item.item_name}
      data-game-shop-index={item.game_shop_index}
      data-direction={direction}
    >
      <button type="button" className="game-shop-viewer-close" onClick={onClose} aria-label={t("ui.close")}>
        x
      </button>
      <div className="game-shop-viewer-stage">
        {info ? (
          <img
            className="game-shop-viewer-item-icon"
            src={originalItemIconPath(info.image)}
            alt=""
            draggable={false}
          />
        ) : null}
        <div className="game-shop-viewer-figure" data-direction={direction}>
          <div className="game-shop-viewer-head" />
          <div className="game-shop-viewer-body" />
          <div className="game-shop-viewer-item-glow" />
        </div>
      </div>
      <div className="game-shop-viewer-name">{truncateGameShopName(item.item_name)}</div>
      <div className="game-shop-viewer-controls">
        <div className="game-shop-viewer-left">
          <SpriteButton
            sprite={ORIGINAL_UI.gameShop.previousButton}
            label={t("ui.previous", [], "Previous")}
            onClick={() => setDirection((current) => (current === 1 ? 8 : current - 1))}
          />
        </div>
        <div className="game-shop-viewer-right">
          <SpriteButton
            sprite={ORIGINAL_UI.gameShop.nextButton}
            label={t("ui.next", [], "Next")}
            onClick={() => setDirection((current) => (current === 8 ? 1 : current + 1))}
          />
        </div>
      </div>
    </div>
  );
}

function applyGameShopSectionFilter(items: readonly CrystalGameShopEntry[], section: GameShopSectionFilter) {
  switch (section) {
    case "top":
      return items.slice(0, 24);
    case "deals":
      return items.filter((item) => item.gold_price > 0 && item.credit_price > 0);
    case "new":
      return [];
    case "all":
    default:
      return items;
  }
}

function gameShopClassMatches(itemClass: string, classFilter: GameShopClassFilter) {
  return classFilter === "all" || itemClass.toLowerCase() === "all" || itemClass.toLowerCase() === classFilter;
}

function gameShopItemInfo(itemIndex: number): CrystalItemEntry | undefined {
  return CRYSTAL_GAME_SHOP_ITEM_INFO_BY_INDEX[String(itemIndex) as keyof typeof CRYSTAL_GAME_SHOP_ITEM_INFO_BY_INDEX];
}

function compareGameShopItems(left: CrystalGameShopEntry, right: CrystalGameShopEntry) {
  return left.item_name.localeCompare(right.item_name) || left.game_shop_index - right.game_shop_index;
}

function truncateGameShopName(name: string) {
  return name.length > 17 ? `${name.slice(0, 17)}...` : name;
}

function formatGameShopStock(stock: number) {
  if (stock <= 0) return "\u221e";
  if (stock >= 99) return "99+";
  return String(stock);
}

/* ========================================================================= */
/* NPC merchant shop (Buy / Sell / Repair / Special)                         */
/*                                                                           */
/* Mirrors Crystal's NPCGoodsDialog (buy list) + NPCDropDialog (sell/repair) */
/* and PanelType { Buy, Sell, Repair, SpecialRepair }. This is a separate,   */
/* self-contained presentational component from the cash GameShopWindow      */
/* above; every action callback is optional so unwired buttons render grey   */
/* and never crash. Wiring happens later in the host.                        */
/* ========================================================================= */

export type NpcShopTab = "buy" | "sell" | "repair" | "special";

/** An item offered for sale by the merchant. */
export type NpcShopGood = {
  /** Stable id the host uses to identify the offer in onBuy. */
  id: number | string;
  name: string;
  icon: number;
  /** Unit price in gold. */
  price: number;
  /** Stack size sold per purchase (>=1). */
  count?: number;
  /** Item rarity → colours the name like the original client. */
  grade?: ItemTooltipGrade;
  /** Optional remaining stock; undefined / <0 renders as unlimited. */
  stock?: number;
  description?: string;
  /** Marks an offer the player cannot afford / is not allowed to buy. */
  disabled?: boolean;
};

/** A player-owned item shown in the Sell / Repair / Special lists. */
export type NpcShopOwnedItem = {
  /** Stable id the host uses (uniqueId). */
  id: number | string;
  name: string;
  icon: number;
  /** Sell value (Sell tab) or repair cost (Repair/Special tabs) in gold. */
  price: number;
  count?: number;
  grade?: ItemTooltipGrade;
  description?: string;
  durabilityCurrent?: number;
  durabilityMax?: number;
  /** When true the row is shown disabled (e.g. DontSell / DontRepair binds). */
  disabled?: boolean;
};

export type NpcShopWindowProps = {
  t: TranslateFn;
  /** Merchant name shown in the header. */
  npcName?: string;
  /** Player gold, shown in the footer and used to grey unaffordable rows. */
  gold?: number;
  /** Initial tab; defaults to "buy". */
  initialTab?: NpcShopTab;
  /** Which tabs the merchant supports; defaults to all four. */
  availableTabs?: NpcShopTab[];

  buyItems?: NpcShopGood[];
  /** Previously-bought items available to buy back at their sell price. */
  buyBackItems?: NpcShopGood[];
  sellItems?: NpcShopOwnedItem[];
  repairItems?: NpcShopOwnedItem[];
  specialRepairItems?: NpcShopOwnedItem[];

  onBuy?: (id: number | string, quantity: number) => void;
  onBuyBack?: (id: number | string, quantity: number) => void;
  onSell?: (id: number | string, quantity: number) => void;
  onRepair?: (id: number | string) => void;
  onSpecialRepair?: (id: number | string) => void;
  onClose: () => void;
};

const NPC_SHOP_GRADE_COLOUR: Record<ItemTooltipGrade, string> = {
  common: "#f4ecd6",
  rare: "#6fc3ff",
  heroic: "#7be07a",
  legendary: "#f0b94a",
  mythical: "#e07ad0",
};

function npcShopTabLabel(t: TranslateFn, tab: NpcShopTab): string {
  switch (tab) {
    case "sell":
      return t("ui.shopSell", [], "Sell");
    case "repair":
      return t("ui.shopRepair", [], "Repair");
    case "special":
      return t("ui.shopSpecialRepair", [], "Special");
    case "buy":
    default:
      return t("ui.shopBuy", [], "Buy");
  }
}

function formatGold(value: number) {
  return Math.max(0, Math.trunc(value)).toLocaleString("en-US");
}

export function NpcShopWindow({
  t,
  npcName,
  gold,
  initialTab = "buy",
  availableTabs = ["buy", "sell", "repair", "special"],
  buyItems,
  buyBackItems,
  sellItems,
  repairItems,
  specialRepairItems,
  onBuy,
  onBuyBack,
  onSell,
  onRepair,
  onSpecialRepair,
  onClose,
}: NpcShopWindowProps) {
  const tabs = availableTabs.length ? availableTabs : (["buy"] as NpcShopTab[]);
  const [tab, setTab] = useState<NpcShopTab>(tabs.includes(initialTab) ? initialTab : tabs[0]);
  const [showBuyBack, setShowBuyBack] = useState(false);
  const [selectedId, setSelectedId] = useState<number | string | null>(null);
  const [quantity, setQuantity] = useState(1);

  const rows: Array<NpcShopGood | NpcShopOwnedItem> = useMemo(() => {
    if (tab === "buy") return showBuyBack ? buyBackItems ?? [] : buyItems ?? [];
    if (tab === "sell") return sellItems ?? [];
    if (tab === "repair") return repairItems ?? [];
    return specialRepairItems ?? [];
  }, [tab, showBuyBack, buyItems, buyBackItems, sellItems, repairItems, specialRepairItems]);

  const selected = useMemo(
    () => rows.find((row) => row.id === selectedId) ?? null,
    [rows, selectedId],
  );

  // Clamp quantity to the selected stack (sell) — buy quantity is 1..99.
  const maxQuantity = tab === "sell" ? Math.max(1, (selected as NpcShopOwnedItem | null)?.count ?? 1) : 99;
  const effectiveQuantity = Math.max(1, Math.min(maxQuantity, quantity));
  const supportsQuantity = tab === "buy" || tab === "sell";

  const total = selected ? Math.max(0, Math.trunc(selected.price)) * (supportsQuantity ? effectiveQuantity : 1) : 0;
  const goldKnown = typeof gold === "number";
  const affordable = !goldKnown || tab !== "buy" || total <= gold!;

  function confirm() {
    if (!selected) return;
    switch (tab) {
      case "buy":
        if (showBuyBack) onBuyBack?.(selected.id, effectiveQuantity);
        else onBuy?.(selected.id, effectiveQuantity);
        break;
      case "sell":
        onSell?.(selected.id, effectiveQuantity);
        break;
      case "repair":
        onRepair?.(selected.id);
        break;
      case "special":
        onSpecialRepair?.(selected.id);
        break;
    }
  }

  const actionHandler =
    tab === "buy"
      ? showBuyBack
        ? onBuyBack
        : onBuy
      : tab === "sell"
        ? onSell
        : tab === "repair"
          ? onRepair
          : onSpecialRepair;
  const actionLabel =
    tab === "buy"
      ? showBuyBack
        ? t("ui.shopBuyBack", [], "Buy Back")
        : t("ui.shopBuy", [], "Buy")
      : tab === "sell"
        ? t("ui.shopSell", [], "Sell")
        : tab === "repair"
          ? t("ui.shopRepair", [], "Repair")
          : t("ui.shopSpecialRepair", [], "Special Repair");
  const confirmEnabled = Boolean(actionHandler) && Boolean(selected) && !selected?.disabled && affordable;

  return (
    <section className="npc-shop-window" aria-label={t("ui.shopTitle", [], "Shop")} data-shop-tab={tab} style={shopStyle.window}>
      <div style={shopStyle.header}>
        <strong style={shopStyle.title}>{npcName?.trim() || t("ui.shopTitle", [], "Shop")}</strong>
        <div className="npc-shop-close" style={shopStyle.close}>
          <SpriteButton sprite={ORIGINAL_UI.inventory.closeButton} label={t("ui.close", [], "Close")} onClick={onClose} />
        </div>
      </div>

      <div style={shopStyle.tabs}>
        {tabs.map((entry) => (
          <button
            key={entry}
            type="button"
            className="npc-shop-tab"
            data-shop-tab-key={entry}
            style={{ ...shopStyle.tab, ...(entry === tab ? shopStyle.tabOn : null) }}
            aria-pressed={entry === tab}
            onClick={() => {
              setTab(entry);
              setSelectedId(null);
              setQuantity(1);
              if (entry !== "buy") setShowBuyBack(false);
            }}
          >
            {npcShopTabLabel(t, entry)}
          </button>
        ))}
      </div>

      {tab === "buy" && (buyBackItems?.length || showBuyBack) ? (
        <div style={shopStyle.subTabs}>
          <button
            type="button"
            style={{ ...shopStyle.subTab, ...(!showBuyBack ? shopStyle.subTabOn : null) }}
            onClick={() => {
              setShowBuyBack(false);
              setSelectedId(null);
            }}
          >
            {t("ui.shopStock", [], "Stock")}
          </button>
          <button
            type="button"
            style={{ ...shopStyle.subTab, ...(showBuyBack ? shopStyle.subTabOn : null) }}
            onClick={() => {
              setShowBuyBack(true);
              setSelectedId(null);
            }}
          >
            {t("ui.shopBuyBack", [], "Buy Back")}
          </button>
        </div>
      ) : null}

      <div style={shopStyle.listHead}>
        <span style={shopStyle.colItem}>{t("client.Type", [], "Item")}</span>
        <span style={shopStyle.colPrice}>
          {tab === "repair" || tab === "special" ? t("client.Repair", [], "Repair: ").replace(/:\s*$/, "") : t("client.Price", [], "Price")}
        </span>
      </div>

      <div className="npc-shop-list" style={shopStyle.list}>
        {rows.length ? (
          rows.map((row) => {
            const owned = row as NpcShopOwnedItem;
            const isSelected = row.id === selectedId;
            const colour = row.grade ? NPC_SHOP_GRADE_COLOUR[row.grade] : "#f4ecd6";
            const unaffordable = goldKnown && tab === "buy" && !showBuyBack && Math.trunc(row.price) > gold!;
            const rowDisabled = Boolean(row.disabled);
            return (
              <button
                key={`shop-row-${row.id}`}
                type="button"
                className="npc-shop-row"
                data-item-id={String(row.id)}
                data-item-name={row.name}
                data-unit-price={Math.max(0, Math.trunc(row.price))}
                aria-label={row.name}
                style={{
                  ...shopStyle.row,
                  ...(isSelected ? shopStyle.rowSelected : null),
                  ...(rowDisabled ? shopStyle.rowDisabled : null),
                }}
                aria-pressed={isSelected}
                disabled={rowDisabled}
                onClick={() => {
                  setSelectedId(row.id);
                  setQuantity(1);
                }}
                onDoubleClick={() => {
                  setSelectedId(row.id);
                  if (confirmEnabledFor(row, tab, showBuyBack, gold)) {
                    if (tab === "buy") (showBuyBack ? onBuyBack : onBuy)?.(row.id, 1);
                    else if (tab === "sell") onSell?.(row.id, 1);
                    else if (tab === "repair") onRepair?.(row.id);
                    else onSpecialRepair?.(row.id);
                  }
                }}
              >
                <span style={shopStyle.rowIconArea}>
                  <img style={shopStyle.rowIcon} src={originalItemIconPath(row.icon)} alt="" draggable={false} />
                  {row.count && row.count > 1 ? <span style={shopStyle.rowCount}>{row.count}</span> : null}
                </span>
                <span style={shopStyle.rowName}>
                  <span style={{ color: colour, fontWeight: 700 }}>{row.name}</span>
                  {(tab === "repair" || tab === "special") && owned.durabilityMax ? (
                    <span style={shopStyle.rowDura}>{`${owned.durabilityCurrent ?? 0}/${owned.durabilityMax}`}</span>
                  ) : null}
                </span>
                <span style={{ ...shopStyle.rowPrice, ...(unaffordable ? shopStyle.rowPriceBad : null) }}>
                  {formatGold(row.price)}
                </span>
              </button>
            );
          })
        ) : (
          <div style={shopStyle.empty}>{t("ui.shopEmpty", [], "Nothing available.")}</div>
        )}
      </div>

      {selected ? (
        <div style={shopStyle.detail}>
          <div style={shopStyle.detailHead}>
            <img style={shopStyle.detailIcon} src={originalItemIconPath(selected.icon)} alt="" draggable={false} />
            <span style={{ color: selected.grade ? NPC_SHOP_GRADE_COLOUR[selected.grade] : "#f4dcaf", fontWeight: 700 }}>
              {selected.name}
            </span>
          </div>
          {(selected as NpcShopOwnedItem).durabilityMax ? (
            <div style={shopStyle.detailLine}>
              {`${t("client.Durability", [], "Durability:")} ${(selected as NpcShopOwnedItem).durabilityCurrent ?? 0}/${(selected as NpcShopOwnedItem).durabilityMax}`}
            </div>
          ) : null}
          {selected.description ? <div style={shopStyle.detailLine}>{selected.description}</div> : null}
        </div>
      ) : null}

      <div style={shopStyle.controls}>
        {supportsQuantity ? (
          <div style={shopStyle.quantityRow}>
            <button
              type="button"
              style={shopStyle.stepButton}
              disabled={effectiveQuantity <= 1}
              aria-label={t("ui.splitDecrease", [], "Less")}
              onClick={() => setQuantity((current) => Math.max(1, Math.min(maxQuantity, current) - 1))}
            >
              −
            </button>
            <input
              type="number"
              min={1}
              max={maxQuantity}
              value={effectiveQuantity}
              aria-label={t("ui.shopQuantity", [], "Quantity")}
              style={shopStyle.quantityInput}
              onChange={(event) => {
                const parsed = Number.parseInt(event.target.value, 10);
                setQuantity(Number.isFinite(parsed) ? parsed : 1);
              }}
            />
            <button
              type="button"
              style={shopStyle.stepButton}
              disabled={effectiveQuantity >= maxQuantity}
              aria-label={t("ui.splitIncrease", [], "More")}
              onClick={() => setQuantity((current) => Math.min(maxQuantity, Math.max(1, current) + 1))}
            >
              +
            </button>
          </div>
        ) : (
          <div style={shopStyle.quantitySpacer} />
        )}

        <button
          type="button"
          className="npc-shop-confirm"
          style={{ ...shopStyle.confirmButton, ...(!confirmEnabled ? shopStyle.confirmDisabled : null) }}
          disabled={!confirmEnabled}
          title={!affordable ? t("client.LowGold", [], "Not enough gold.") : undefined}
          onClick={confirm}
        >
          {actionLabel}
        </button>
      </div>

      <div style={shopStyle.footer}>
        <span style={shopStyle.footerLabel}>{t("ui.shopTotal", [], "Total")}</span>
        <span style={{ ...shopStyle.footerValue, ...(!affordable ? shopStyle.rowPriceBad : null) }}>{formatGold(total)}</span>
        <span style={shopStyle.footerSpacer} />
        <span style={shopStyle.footerLabel}>{t("client.Gold", [], "Gold")}</span>
        <span style={shopStyle.footerValue}>{goldKnown ? formatGold(gold!) : "—"}</span>
      </div>
    </section>
  );
}

function confirmEnabledFor(
  row: NpcShopGood | NpcShopOwnedItem,
  tab: NpcShopTab,
  showBuyBack: boolean,
  gold: number | undefined,
): boolean {
  if (row.disabled) return false;
  if (tab === "buy" && !showBuyBack && typeof gold === "number") {
    return Math.trunc(row.price) <= gold;
  }
  return true;
}

const shopStyle: Record<string, CSSProperties> = {
  window: {
    position: "absolute",
    left: 24,
    top: 120,
    width: 264,
    height: 392,
    zIndex: 30,
    border: "1px solid rgba(190, 157, 99, 0.55)",
    background: "linear-gradient(180deg, rgba(27, 19, 10, 0.97), rgba(11, 8, 5, 0.97))",
    color: "#f0eee8",
    fontSize: 12,
    textShadow: "1px 1px 0 #000",
    boxShadow: "0 3px 10px rgba(0,0,0,0.5)",
  },
  header: {
    display: "flex",
    alignItems: "center",
    justifyContent: "space-between",
    padding: "4px 6px",
    borderBottom: "1px solid rgba(190, 157, 99, 0.35)",
  },
  title: { fontSize: 12, color: "#f4dcaf", fontWeight: 700 },
  close: {},
  tabs: { display: "flex", gap: 2, padding: "4px 6px 0" },
  tab: {
    flex: 1,
    border: "1px solid rgba(190, 157, 99, 0.5)",
    borderBottom: "none",
    background: "linear-gradient(180deg, rgba(40, 27, 15, 0.9), rgba(20, 13, 7, 0.9))",
    color: "#cbb38a",
    padding: "3px 0",
    fontSize: 10,
    cursor: "pointer",
  },
  tabOn: {
    background: "linear-gradient(180deg, rgba(95, 53, 24, 0.95), rgba(45, 23, 12, 0.95))",
    color: "#f4dcaf",
    fontWeight: 700,
  },
  subTabs: { display: "flex", gap: 2, padding: "4px 6px 0" },
  subTab: {
    flex: 1,
    borderWidth: 1,
    borderStyle: "solid",
    borderColor: "rgba(190, 157, 99, 0.4)",
    background: "rgba(20, 13, 7, 0.9)",
    color: "#a89568",
    padding: "2px 0",
    fontSize: 9,
    cursor: "pointer",
  },
  subTabOn: { borderColor: "rgba(214, 180, 110, 0.8)", color: "#f4dcaf" },
  listHead: {
    display: "flex",
    margin: "4px 6px 0",
    fontSize: 9,
    color: "#a89568",
    borderBottom: "1px solid rgba(190, 157, 99, 0.3)",
    paddingBottom: 2,
  },
  colItem: { flex: 1 },
  colPrice: { width: 70, textAlign: "right" },
  list: {
    margin: "2px 6px",
    height: 198,
    overflowY: "auto",
    display: "flex",
    flexDirection: "column",
    gap: 2,
  },
  empty: { padding: "16px 4px", textAlign: "center", fontSize: 11, color: "#9c8d6f" },
  row: {
    display: "flex",
    alignItems: "center",
    gap: 6,
    padding: "2px 4px",
    borderWidth: 1,
    borderStyle: "solid",
    borderColor: "rgba(190, 157, 99, 0.18)",
    background: "rgba(11, 8, 5, 0.45)",
    cursor: "pointer",
    textAlign: "left",
  },
  rowSelected: { borderColor: "rgba(214, 180, 110, 0.85)", background: "rgba(60, 38, 18, 0.7)" },
  rowDisabled: { opacity: 0.4, cursor: "default" },
  rowIconArea: { position: "relative", width: 28, height: 28, flex: "0 0 28px" },
  rowIcon: { width: 28, height: 28, imageRendering: "pixelated" },
  rowCount: { position: "absolute", right: 0, bottom: 0, fontSize: 9, color: "#f4ecd6", textShadow: "1px 1px 0 #000" },
  rowName: { flex: 1, minWidth: 0, display: "flex", flexDirection: "column", overflow: "hidden", fontSize: 11 },
  rowDura: { fontSize: 9, color: "#a89568" },
  rowPrice: { width: 70, textAlign: "right", color: "#f0d98a", fontSize: 11, fontWeight: 700 },
  rowPriceBad: { color: "#d98a8a" },
  detail: {
    margin: "0 6px 2px",
    padding: "4px 6px",
    minHeight: 34,
    border: "1px solid rgba(190, 157, 99, 0.28)",
    background: "rgba(11, 8, 5, 0.55)",
    fontSize: 10,
    color: "#cbb38a",
  },
  detailHead: { display: "flex", alignItems: "center", gap: 6, fontSize: 11 },
  detailIcon: { width: 20, height: 20, imageRendering: "pixelated" },
  detailLine: { marginTop: 2, lineHeight: 1.3 },
  controls: { display: "flex", alignItems: "center", gap: 6, padding: "4px 6px" },
  quantityRow: { display: "flex", alignItems: "center", gap: 2, flex: "0 0 auto" },
  quantitySpacer: { flex: 1 },
  stepButton: {
    width: 20,
    height: 22,
    border: "1px solid rgba(190, 157, 99, 0.56)",
    background: "rgba(19, 12, 8, 0.92)",
    color: "#f4dcaf",
    fontSize: 14,
    lineHeight: "18px",
    cursor: "pointer",
  },
  quantityInput: {
    width: 42,
    border: "1px solid rgba(190, 157, 99, 0.56)",
    background: "rgba(19, 12, 8, 0.92)",
    color: "#f4dcaf",
    padding: "3px 4px",
    fontSize: 11,
    textAlign: "center",
  },
  confirmButton: {
    flex: 1,
    border: "1px solid rgba(214, 180, 110, 0.85)",
    background: "linear-gradient(180deg, rgba(120, 74, 34, 0.96), rgba(70, 40, 20, 0.96))",
    color: "#f4dcaf",
    padding: "5px 0",
    fontSize: 11,
    cursor: "pointer",
  },
  confirmDisabled: { opacity: 0.45, cursor: "default" },
  footer: {
    display: "flex",
    alignItems: "center",
    gap: 4,
    padding: "5px 6px",
    borderTop: "1px solid rgba(190, 157, 99, 0.35)",
    fontSize: 11,
  },
  footerLabel: { color: "#a89568" },
  footerValue: { color: "#f4dcaf", fontWeight: 700 },
  footerSpacer: { flex: 1 },
};
