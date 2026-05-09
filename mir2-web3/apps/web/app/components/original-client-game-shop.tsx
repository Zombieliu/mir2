"use client";

import { useEffect, useMemo, useState } from "react";

import { ORIGINAL_UI } from "../../lib/original-ui";
import {
  CRYSTAL_GAME_SHOP_ITEM_INFO_BY_INDEX,
  CRYSTAL_GAME_SHOP_ITEMS,
} from "../../lib/generated/crystal-game-shop-data";
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
        <SpriteButton sprite={ORIGINAL_UI.gameShop.upButton} label={t("ui.up", [], "Up")} onClick={() => undefined} />
      </div>
      <div className="game-shop-scroll thumb">
        <SpriteButton sprite={ORIGINAL_UI.gameShop.positionBar} label={t("ui.scroll", [], "Scroll")} onClick={() => undefined} />
      </div>
      <div className="game-shop-scroll down">
        <SpriteButton sprite={ORIGINAL_UI.gameShop.downButton} label={t("ui.down", [], "Down")} onClick={() => undefined} />
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
        <SpriteButton sprite={ORIGINAL_UI.gameShop.previousButton} label={t("ui.previous", [], "Previous")} onClick={() => setPage((current) => Math.max(0, current - 1))} />
      </div>
      <div className="game-shop-page-button next">
        <SpriteButton sprite={ORIGINAL_UI.gameShop.nextButton} label={t("ui.next", [], "Next")} onClick={() => setPage((current) => Math.min(pageCount - 1, current + 1))} />
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

function originalItemIconPath(icon: number) {
  return `/original-ui/Items/${icon}.png`;
}
