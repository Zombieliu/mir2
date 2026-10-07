"use client";

import { useEffect, useLayoutEffect, useMemo, useRef, useState, type CSSProperties } from "react";

import { ORIGINAL_UI } from "../../lib/original-ui";
import { originalAssetPath } from "../../lib/asset-url";
import { loadOriginalSceneSpriteLibrary, type OriginalSceneSpriteLibraryMeta } from "../../lib/original-scene-sprite-meta";
import { composeCashPreviewFrames, readCashPreviewLayerDocument, type CashPreviewLayerDocument } from "../../lib/cash-preview-rendering";
import { handleSceneAssetImageError, handleSceneAssetImageLoad } from "./original-client-scene-rendering";
import { cashGameShopConfirmationCurrent, cashGameShopQuantityLimit, makeCashGameShopConfirmation,
  type CashGameShopConfirmation, type CashGameShopEntry, type CashGameShopSource } from "../../lib/cash-game-shop-ui";
import { originalItemIconPath } from "./original-client-inventory-utils";
import type { ItemTooltipGrade } from "./original-client-item-tooltip";
import type { CrystalTooltipDocument } from "../../lib/shared-item-tooltip";
import { OriginalCrystalItemTooltip } from "./original-client-crystal-item-tooltip";
import type { NpcGoldBuyQuote } from "../../lib/bevy-npc-shop-buy";
import type { NpcRepairView, NpcRepairSelection } from "../../lib/npc-repair-service";
import { SpriteButton } from "./original-client-overlays";

type TranslateFn = (
  key: string,
  params?: Array<string | number>,
  fallback?: string,
) => string;

type EntityClassKey = "warrior" | "wizard" | "taoist" | "assassin" | "archer";

type CrystalGameShopEntry = CashGameShopEntry;

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
  source = null,
  purchasePending = false,
  onConfirmPurchase,
  onReadItemTooltip,
  previewSourceKey = null,
  onReadPreviewLayers,
  onTurnPreview,
  onClose,
}: {
  t: TranslateFn;
  gold: number;
  credits: number;
  playerClass: EntityClassKey;
  /** Retained for caller compatibility; authoritative purchases use the captured confirmation. */
  onBuy?: (gameShopIndex: number, quantity: number, paymentType: GameShopPaymentType) => void;
  source?: CashGameShopSource | null;
  purchasePending?: boolean;
  onConfirmPurchase?: (confirmation: CashGameShopConfirmation) => void;
  onReadItemTooltip?: (item: CashGameShopEntry) => CrystalTooltipDocument | null;
  /** Identity of the current owner, selected catalogue and verified equipment source. */
  previewSourceKey?: string | null;
  onReadPreviewLayers?: (item: CashGameShopEntry, direction: number, elapsedMs: number) => CashPreviewLayerDocument | null;
  onTurnPreview?: (direction: number, right: boolean) => number | null;
  onClose: () => void;
}) {
  const [sectionFilter, setSectionFilter] = useState<GameShopSectionFilter>("all");
  const [classFilter, setClassFilter] = useState<GameShopClassFilter>(playerClass);
  const [categoryFilter, setCategoryFilter] = useState("Show All");
  const [search, setSearch] = useState("");
  const [page, setPage] = useState(0);
  const [paymentType, setPaymentType] = useState<GameShopPaymentType>("gold");
  const [quantities, setQuantities] = useState<Record<number, number>>({});
  const [preview, setPreview] = useState<{ item: CrystalGameShopEntry; cellLeft: number; epoch: number } | null>(null);
  const previewEpochRef = useRef(0);
  const [activeTooltip, setActiveTooltip] = useState<CrystalGameShopEntry | null>(null);
  const [confirmation, setConfirmation] = useState<CashGameShopConfirmation | null>(null);
  const submittedConfirmation = useRef<CashGameShopConfirmation | null>(null);
  const catalog = source?.entries ?? [];
  const confirmationCurrent = confirmation !== null && cashGameShopConfirmationCurrent(confirmation, source);

  const sectionItems = useMemo(
    () => applyGameShopSectionFilter(catalog, sectionFilter),
    [catalog, sectionFilter],
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
      ...Array.from(new Set(searchedItems.map((item) => item.category))),
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
  useEffect(() => { setActiveTooltip(null); }, [source?.entries, sectionFilter, classFilter, categoryFilter, currentPage, search]);
  useEffect(() => { setPreview(null); }, [sectionFilter, classFilter, categoryFilter, search]);

  const setQuantity = (gameShopIndex: number, nextQuantity: number) => {
    const item = catalog.find((candidate) => candidate.game_shop_index === gameShopIndex);
    setQuantities((current) => ({
      ...current,
      [gameShopIndex]: item ? clampGameShopQuantity(item, nextQuantity) : Math.max(1, Math.min(99, nextQuantity)),
    }));
  };

  const showPreview = (item: CrystalGameShopEntry, cellLeft: number) => {
    if (!source?.entries.includes(item) || previewEpochRef.current >= Number.MAX_SAFE_INTEGER) return;
    setPreview({ item, cellLeft, epoch: ++previewEpochRef.current });
  };
  const requestConfirmation = (item: CrystalGameShopEntry) => {
    if (purchasePending || !onConfirmPurchase) return;
    const captured = makeCashGameShopConfirmation(source, item.game_shop_index,
      clampGameShopQuantity(item, quantities[item.game_shop_index] ?? 1), paymentType);
    if (captured) setConfirmation(captured);
  };

  return (
    <section className="game-shop-window" aria-label={t("ui.gameShop")} onKeyDown={event => {
      if (event.key === "Escape" && confirmation) { event.stopPropagation(); setConfirmation(null); }
    }}>
      <style>{'.game-shop-cell-frame > .original-item-tooltip { opacity: 1; visibility: visible; transform: translateY(0); }'}</style>
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
            quantity={clampGameShopQuantity(item, quantities[item.game_shop_index] ?? 1)}
            onQuantityChange={(nextQuantity) => setQuantity(item.game_shop_index, nextQuantity)}
             onBuy={() => requestConfirmation(item)}
             buyDisabled={purchasePending || !onConfirmPurchase || !makeCashGameShopConfirmation(source,
               item.game_shop_index, clampGameShopQuantity(item, quantities[item.game_shop_index] ?? 1), paymentType)}
             inputPending={purchasePending || confirmation !== null}
            onPreview={(cellLeft) => showPreview(item, cellLeft)}
            onReadItemTooltip={onReadItemTooltip}
            tooltipActive={activeTooltip === item}
            onTooltipActiveChange={active => setActiveTooltip(current => active ? item : current === item ? null : current)}
            t={t}
          />
        ))}
      </div>
      {catalog.length === 0 ? <div role="status" style={{ position: "absolute", left: 160, top: 160, width: 500, color: "#e7d9b0" }}>
        {t("ui.gameShopCatalogUnknown", [], "Waiting for the current shop catalogue.")}
      </div> : null}
      {preview ? (
        <GameShopViewer
          key={`${preview.item.game_shop_index}:${preview.epoch}`}
          item={preview.item}
          previewSourceKey={previewSourceKey}
          onReadPreviewLayers={onReadPreviewLayers}
          onTurnPreview={onTurnPreview}
          left={preview.cellLeft < 350 ? 416 : 151}
          top={115}
          t={t}
          onClose={() => setPreview(null)}
        />
      ) : null}
      <div className="game-shop-total credits">{formatGameShopPrice(source?.wallet.credit ?? credits)}</div>
      <div className="game-shop-total gold">{formatGameShopPrice(source?.wallet.gold ?? gold)}</div>
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
      {purchasePending ? <div role="status" style={{ position: "absolute", left: 160, bottom: 12, color: "#e7d9b0" }}>
        {t("ui.gameShopPurchasePending", [], "Purchase pending. Waiting for its receipt.")}
      </div> : null}
      {confirmation ? <div role="dialog" aria-modal="true" aria-label={t("ui.gameShopConfirm", [], "Confirm purchase")}
        style={{ position: "absolute", inset: 0, zIndex: 1000, background: "rgba(0,0,0,.75)", display: "grid", placeItems: "center" }}>
        <div style={{ width: 340, background: "#21170e", border: "1px solid #96743b", padding: 18, color: "#f0dfb5" }}>
          <p>{confirmation.itemName} × {confirmation.count * confirmation.quantity}</p>
          <p>{formatGameShopPrice(confirmation.total)} {confirmation.payment === "gold" ? "Gold" : "Credits"}</p>
          {!confirmationCurrent ? <p role="status">{t("ui.gameShopChanged", [], "The shop or balance changed. Review the purchase again.")}</p> : null}
          <div style={{ display: "flex", gap: 12 }}>
            <button type="button" autoFocus style={{ minHeight: 44, minWidth: 100 }} onClick={() => setConfirmation(null)}>
              {t("ui.cancel", [], "Cancel")}</button>
            <button type="button" style={{ minHeight: 44, minWidth: 100 }}
              disabled={purchasePending || !confirmationCurrent || !onConfirmPurchase}
              onClick={() => {
                const captured = confirmation;
                if (!captured || purchasePending || !onConfirmPurchase || submittedConfirmation.current === captured
                  || !cashGameShopConfirmationCurrent(captured, source)) return;
                submittedConfirmation.current = captured;
                setConfirmation(null);
                onConfirmPurchase(captured);
              }}>{t("ui.confirm", [], "Confirm")}</button>
          </div>
        </div>
      </div> : null}
    </section>
  );
}

function GameShopCell({
  item,
  index,
  quantity,
  onQuantityChange,
  onBuy,
  buyDisabled,
  inputPending,
  onPreview,
  onReadItemTooltip,
  tooltipActive,
  onTooltipActiveChange,
  t,
}: {
  item: CrystalGameShopEntry;
  index: number;
  quantity: number;
  onQuantityChange: (quantity: number) => void;
  onBuy: () => void;
  buyDisabled: boolean;
  inputPending: boolean;
  onPreview: (cellLeft: number) => void;
  onReadItemTooltip?: (item: CashGameShopEntry) => CrystalTooltipDocument | null;
  tooltipActive: boolean;
  onTooltipActiveChange: (active: boolean) => void;
  t: TranslateFn;
}) {
  const info = gameShopItemInfo(item);
  const left = index < 4 ? 152 + index * 132 : 152 + (index - 4) * 132;
  const top = index < 4 ? 115 : 275;
  const hasPreview = Boolean(info && GAME_SHOP_PREVIEW_ITEM_TYPES.has(info.item_type));
  const displayName = truncateGameShopName(item.item_name);
  const document = tooltipActive ? onReadItemTooltip?.(item) : null;

  return (
    <div className="game-shop-cell-frame" style={{ left, top }} tabIndex={0} role="group" aria-label={item.item_name}
      onMouseEnter={() => onTooltipActiveChange(true)} onMouseLeave={() => onTooltipActiveChange(false)}
      onFocus={() => onTooltipActiveChange(true)} onBlur={event => {if (!event.currentTarget.contains(event.relatedTarget as Node | null)) onTooltipActiveChange(false);}}>
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
      <div className="game-shop-cell-stock-value">{formatGameShopStock(item.stock, item.stock_level)}</div>
      <div className="game-shop-cell-count">{item.count}</div>
      <div className="game-shop-cell-quantity-down">
        <SpriteButton sprite={ORIGINAL_UI.gameShop.previousButton} label={t("ui.down", [], "Down")} disabled={inputPending || quantity <= 1} onClick={() => onQuantityChange(quantity - 1)} />
      </div>
      <div className="game-shop-cell-quantity">{quantity}</div>
      <div className="game-shop-cell-quantity-up">
        <SpriteButton sprite={ORIGINAL_UI.gameShop.nextButton} label={t("ui.up", [], "Up")} disabled={inputPending || quantity >= gameShopMaxQuantity(item)} onClick={() => onQuantityChange(quantity + 1)} />
      </div>
      <div className="game-shop-cell-credit-price">{item.can_buy_credit ? formatGameShopPrice(item.credit_price * quantity) : ""}</div>
      <div className="game-shop-cell-gold-price">{item.can_buy_gold ? formatGameShopPrice(item.gold_price * quantity) : ""}</div>
      {hasPreview ? (
        <div className="game-shop-cell-preview">
          <SpriteButton sprite={ORIGINAL_UI.gameShop.previewButton} label={t("ui.preview", [], "Preview")} onClick={() => onPreview(left)} />
        </div>
      ) : null}
      <div className={hasPreview ? "game-shop-cell-buy with-preview" : "game-shop-cell-buy"}>
        <SpriteButton sprite={ORIGINAL_UI.gameShop.buyButton} label={t("ui.buy", [], "Buy")} disabled={buyDisabled} onClick={onBuy} />
      </div>
      {document ? <OriginalCrystalItemTooltip document={document} align={index % 4 > 1 ? "left" : "right"} /> : null}
    </div>
  );
}

function GameShopViewer({
  item, left, top, t, previewSourceKey, onReadPreviewLayers, onTurnPreview, onClose,
}: {
  item: CrystalGameShopEntry;
  left: number;
  top: number;
  t: TranslateFn;
  previewSourceKey: string | null;
  onReadPreviewLayers?: (item: CashGameShopEntry, direction: number, elapsedMs: number) => CashPreviewLayerDocument | null;
  onTurnPreview?: (direction: number, right: boolean) => number | null;
  onClose: () => void;
}) {
  const [direction, setDirection] = useState(6);
  const [elapsedMs, setElapsedMs] = useState(0);
  const [loaded, setLoaded] = useState<{ key: string; libraries: ReadonlyMap<string, OriginalSceneSpriteLibraryMeta> } | null>(null);
  useEffect(() => {
    const tick = () => {
      if (document.visibilityState === "visible") setElapsedMs(Math.floor(performance.now()));
    };
    tick();
    const timer = window.setInterval(tick, 150);
    document.addEventListener("visibilitychange", tick);
    return () => { window.clearInterval(timer); document.removeEventListener("visibilitychange", tick); };
  }, []);
  const layerDocument = useMemo(() => {
    if (!previewSourceKey || !onReadPreviewLayers) return null;
    try { return readCashPreviewLayerDocument(onReadPreviewLayers(item, direction, elapsedMs), direction); }
    catch { return null; }
  }, [item, direction, elapsedMs, previewSourceKey, onReadPreviewLayers]);
  const libraries = Array.from(new Set(layerDocument?.layers.map(layer => layer.library) ?? [])).sort();
  const metadataKey = previewSourceKey && layerDocument?.known ? JSON.stringify([previewSourceKey, libraries]) : "";
  useEffect(() => {
    if (!metadataKey) return;
    let current = true;
    const names: string[] = JSON.parse(metadataKey)[1];
    void Promise.all(names.map(async library => {
      try { return [library, await loadOriginalSceneSpriteLibrary(library)] as const; }
      catch { return null; }
    })).then(results => {
      if (!current) return;
      const next = new Map<string, OriginalSceneSpriteLibraryMeta>();
      for (const result of results) if (result) next.set(result[0], result[1]);
      setLoaded({ key: metadataKey, libraries: next });
    });
    return () => { current = false; };
  }, [metadataKey]);
  const drawing = composeCashPreviewFrames(layerDocument,
    loaded && loaded.key === metadataKey ? loaded.libraries : new Map<string, OriginalSceneSpriteLibraryMeta>());
  const [imageStates, setImageStates] = useState<ReadonlyMap<string, "ready" | "failed">>(new Map());
  const imageKey = (layer: typeof drawing.layers[number]) => JSON.stringify([metadataKey, layer.library, layer.frame,
    layer.path, layer.width, layer.height]);
  const liveImagesRef = useRef({ active: true, metadataKey, keys: new Set<string>() });
  liveImagesRef.current.metadataKey = metadataKey;
  liveImagesRef.current.keys = new Set(drawing.layers.map(imageKey));
  useEffect(() => { liveImagesRef.current.active = true; return () => { liveImagesRef.current.active = false; }; }, []);
  function recordPreviewImage(key: string, state: "ready" | "failed") {
    const live = liveImagesRef.current;
    if (!live.active || live.metadataKey !== metadataKey || !live.keys.has(key)) return;
    setImageStates(before => {
      if (before.get(key) === state) return before;
      const next = new Map(before.size < 256 ? before : [...before].filter(([entry]) => live.keys.has(entry)));
      next.set(key, state); return next;
    });
  }
  const previewComplete = drawing.complete && drawing.layers.every(layer => imageStates.get(imageKey(layer)) === "ready");
  const turn = (right: boolean) => {
    if (!previewSourceKey || !onTurnPreview) return;
    try {
      const next = onTurnPreview(direction, right);
      if (typeof next === "number" && Number.isInteger(next) && next >= 1 && next <= 8) setDirection(next);
    } catch { /* Unsupported shared facade keeps the current local direction. */ }
  };
  return (
    <div className="game-shop-viewer" style={{ left, top, width: 260, height: 308,
      border: 0, background: "transparent" }} role="dialog" aria-label={t("ui.preview", [], "Preview")}
      data-item-name={item.item_name} data-game-shop-index={item.game_shop_index}
      data-direction={direction} data-preview-complete={previewComplete}>
      <img src={originalAssetPath("/original-ui/Title/785.png")} alt="" draggable={false}
        style={{ position: "absolute", inset: 0, width: 260, height: 308, pointerEvents: "none", imageRendering: "pixelated" }} />
      <div style={{ position: "absolute", inset: 0, overflow: "hidden", pointerEvents: "none" }} aria-hidden="true">
        {drawing.layers.map((layer, index) => (
          <img key={imageKey(layer)} src={originalAssetPath(layer.path)} alt="" draggable={false}
            data-mir2-original-src={originalAssetPath(layer.path)} onError={event => {
              recordPreviewImage(imageKey(layer), "failed"); handleSceneAssetImageError(event);
            }}
            onLoad={event => {
              handleSceneAssetImageLoad(event);
               const matches = event.currentTarget.naturalWidth === layer.width
                 && event.currentTarget.naturalHeight === layer.height;
               if (!matches) event.currentTarget.style.visibility = "hidden";
               recordPreviewImage(imageKey(layer), matches ? "ready" : "failed");
            }} style={{ position: "absolute", left: layer.left, top: layer.top,
               width: layer.width, height: layer.height, zIndex: index, imageRendering: "pixelated",
               visibility: imageStates.get(imageKey(layer)) === "ready" ? "visible" : "hidden" }} />
        ))}
      </div>
      {!previewComplete ? <div role="status" style={{ position: "absolute", left: 12, top: 244, width: 236,
        fontSize: 11, color: "#fff4c8", textAlign: "center", pointerEvents: "none" }}>
        {t("ui.gameShopPreviewUnknown", [], "Preview frames are unavailable.")}
      </div> : null}
      <button type="button" className="game-shop-viewer-close" onClick={onClose} aria-label={t("ui.close")}>x</button>
      <div className="game-shop-viewer-controls">
        <div className="game-shop-viewer-left">
          <SpriteButton sprite={ORIGINAL_UI.gameShop.previousButton} label={t("ui.previous", [], "Previous")}
            disabled={!previewSourceKey || !onTurnPreview} onClick={() => turn(false)} />
        </div>
        <div className="game-shop-viewer-right">
          <SpriteButton sprite={ORIGINAL_UI.gameShop.nextButton} label={t("ui.next", [], "Next")}
            disabled={!previewSourceKey || !onTurnPreview} onClick={() => turn(true)} />
        </div>
      </div>
    </div>
  );
}

function applyGameShopSectionFilter(items: readonly CrystalGameShopEntry[], section: GameShopSectionFilter) {
  switch (section) {
    case "top":
      return items.filter((item) => item.top_item);
    case "deals":
      return items.filter((item) => item.deal);
    case "new":
      return items.filter((item) => isNewGameShopItem(item.date_binary_datetime));
    case "all":
    default:
      return items;
  }
}

function gameShopClassMatches(itemClass: string, classFilter: GameShopClassFilter) {
  const name = itemClass.trim().toLowerCase();
  return classFilter === "all" || !name || name === "all" || name === "show all" || name === classFilter;
}

function gameShopItemInfo(item: CrystalGameShopEntry): CrystalItemEntry {
  return { image: item.info.image, item_type: item.info.item_type };
}

function compareGameShopItems(left: CrystalGameShopEntry, right: CrystalGameShopEntry) {
  return left.item_name.localeCompare(right.item_name) || left.game_shop_index - right.game_shop_index;
}

function truncateGameShopName(name: string) {
  return name.length > 17 ? name.slice(0, 17) : name;
}

function formatGameShopPrice(value: number) {
  return value.toLocaleString("en-US");
}

function gameShopMaxQuantity(item: CrystalGameShopEntry) {
  return cashGameShopQuantityLimit(item);
}

function clampGameShopQuantity(item: CrystalGameShopEntry, quantity: number) {
  return Math.max(1, Math.min(gameShopMaxQuantity(item), Math.floor(quantity)));
}

function formatGameShopStock(stock: number, stockLevel: number) {
  if (stock === 0) return "\u221e";
  if (stockLevel >= 99) return "99+";
  return String(Math.max(0, stockLevel));
}

function isNewGameShopItem(dateBinary: string) {
  try {
    const raw = BigInt(dateBinary);
    if (raw === 0n) return false;
    const unsigned = BigInt.asUintN(64, raw);
    const ticks = unsigned & 0x3fffffffffffffffn;
    const nowTicks = BigInt(Date.now()) * 10000n + 621355968000000000n;
    return ticks > nowTicks - 7n * 24n * 60n * 60n * 10_000_000n;
  } catch {
    return false;
  }
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
  /** Unit price in the merchant currency; display only for shared purchases. */
  price: number;
  /** Stack size sold per purchase (>=1). */
  count?: number;
  /** Item rarity → colours the name like the original client. */
  grade?: ItemTooltipGrade;
  /** Optional remaining stock; undefined / <0 renders as unlimited. */
  stock?: number;
  description?: string;
  /** Legacy resale/other services keep their existing controls. */
  requiresGoldBuyPlan?: boolean;
  /** Pearl stock always requires a current shared quote; no legacy price fallback. */
  requiresPearlBuyPlan?: boolean;
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
  /** Gold remains available for gold purchases, selling and repairs. */
  gold?: number;
  currency?: "gold" | "pearls";
  /** Nullable signed wallet fact. Display normalization never grants purchase authority. */
  pearlBalance?: number | null;
  /** Initial tab; defaults to "buy". */
  initialTab?: NpcShopTab;
  tab?: NpcShopTab;
  onTabChange?: (tab: NpcShopTab) => void;
  inputBlocked?: boolean;
  getInputBlocked?: () => boolean;
  /** Which tabs the merchant supports; defaults to all four. */
  availableTabs?: NpcShopTab[];

  buyItems?: NpcShopGood[];
  /** Previously-bought items available to buy back at their sell price. */
  buyBackItems?: NpcShopGood[];
  sellItems?: NpcShopOwnedItem[];
  repairItems?: NpcShopOwnedItem[];
  specialRepairItems?: NpcShopOwnedItem[];
  repairView?: NpcRepairView | null;
  onSelectRepair?: (view: NpcRepairView, uniqueId: number) => NpcRepairSelection | null;
  onConfirmRepair?: (selection: NpcRepairSelection) => boolean;
  repairTargetSelection?: NpcRepairSelection | null;
  onToggleRepairHold?: (view: NpcRepairView) => void;
  onRegisterRepairTarget?: (view: NpcRepairView, node: HTMLElement) => () => void;

  onBuy?: (id: number | string, quantity: number) => void;
  onBuyBack?: (id: number | string, quantity: number) => void;
  onQuoteBuy?: (id: number | string, quantity: number) => NpcGoldBuyQuote | null;
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
  currency = "gold", pearlBalance = null,
  initialTab = "buy",
  tab: controlledTab, onTabChange, inputBlocked = false, getInputBlocked,
  availableTabs = ["buy", "sell", "repair", "special"],
  buyItems,
  buyBackItems,
  sellItems,
  repairItems,
  specialRepairItems,
  repairView, onSelectRepair, onConfirmRepair,
  repairTargetSelection, onToggleRepairHold, onRegisterRepairTarget,
  onBuy,
  onBuyBack, onQuoteBuy,
  onSell,
  onClose,
}: NpcShopWindowProps) {
  const tabs = availableTabs.length ? availableTabs : (["buy"] as NpcShopTab[]);
  const [localTab, setLocalTab] = useState<NpcShopTab>(tabs.includes(initialTab) ? initialTab : tabs[0]);
  const tab = controlledTab !== undefined && tabs.includes(controlledTab) ? controlledTab : localTab;
  const blocked = () => inputBlocked || getInputBlocked?.() === true;
  const [showBuyBack, setShowBuyBack] = useState(false);
  const [selectedId, setSelectedId] = useState<number | string | null>(null);
  const [quantity, setQuantity] = useState(1);
  const [repairSelection, setRepairSelection] = useState<NpcRepairSelection | null>(null);
  const repairTab = tab === "repair" || tab === "special";
  const repairSelectionCurrent = repairSelection !== null && repairSelection.stamp === repairView?.stamp
    && repairSelection.uniqueId === selectedId;
  useEffect(() => { setRepairSelection(null); setSelectedId(null); }, [repairView?.stamp]);
  const repairTargetRef = useRef<HTMLDivElement | null>(null);
  useLayoutEffect(() => {
    const node = repairTargetRef.current;
    if (repairTab && repairView && node) return onRegisterRepairTarget?.(repairView, node);
  }, [repairTab, repairView?.stamp, onRegisterRepairTarget]);

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

  const pearlBuy = tab === "buy" && (currency === "pearls"
    || (selected as NpcShopGood | null)?.requiresPearlBuyPlan === true);
  const sharedBuy = tab === "buy" && !showBuyBack && (pearlBuy || Boolean(onQuoteBuy)
    && (selected as NpcShopGood | null)?.requiresGoldBuyPlan !== false);
  const pearlsKnown = typeof pearlBalance === "number" && Number.isSafeInteger(pearlBalance);
  const displayedPearls = pearlsKnown ? Math.max(0, pearlBalance!) : null;
  const quoteCanBuy = (quote: NpcGoldBuyQuote | null | undefined, usesPearls: boolean) =>
    quote?.canBuy === true && (usesPearls ? pearlsKnown && quote.currency === "pearls"
      && typeof quote.totalPearls === "number" && Number.isSafeInteger(quote.totalPearls) && quote.totalPearls >= 0
      : quote.currency !== "pearls");
  const firstQuote = sharedBuy && selected && !blocked() ? onQuoteBuy?.(selected.id, quantity) : null;
  const maxQuantity = tab === "sell" ? Math.max(1, (selected as NpcShopOwnedItem | null)?.count ?? 1)
    : sharedBuy ? Math.max(1, firstQuote?.maxQuantity ?? 1) : 99;
  const effectiveQuantity = Math.max(1, Math.min(maxQuantity, quantity));
  const supportsQuantity = tab === "buy" || tab === "sell";
  const buyQuote = sharedBuy && selected && !blocked() ? onQuoteBuy?.(selected.id, effectiveQuantity) : null;
  const selectedRepairQuote = repairTab ? repairView?.rows.find(row => row.uniqueId === selectedId)?.quote ?? null : null;
  const total = repairTab ? selectedRepairQuote?.displayedTotal ?? null : pearlBuy
    ? buyQuote?.currency === "pearls" ? buyQuote.totalPearls ?? null : null
    : sharedBuy ? buyQuote?.totalGold ?? null
    : selected ? Math.max(0, Math.trunc(selected.price)) * (supportsQuantity ? effectiveQuantity : 1) : 0;
  const goldKnown = typeof gold === "number";
  const affordable = repairTab ? selectedRepairQuote?.affordable === true : pearlBuy
    ? quoteCanBuy(buyQuote, true) : !goldKnown || tab !== "buy" || total !== null && total <= gold!;

  function confirm() {
    if (blocked() || !selected || selected.disabled) return;
    // Render quotes describe the controls. Only a fresh quote can authorize this edge.
    if (sharedBuy || pearlBuy) {
      const currentQuote = onQuoteBuy?.(selected.id, effectiveQuantity);
      if (!quoteCanBuy(currentQuote, pearlBuy) || blocked()) return;
    }
    if (repairTab) {
      if (!selected.disabled && repairSelectionCurrent && repairSelection && selectedRepairQuote?.affordable) {
        onConfirmRepair?.(repairSelection); setRepairSelection(null); setSelectedId(null);
      }
      return;
    }
    switch (tab) {
      case "buy":
        if (showBuyBack) onBuyBack?.(selected.id, effectiveQuantity);
        else onBuy?.(selected.id, effectiveQuantity);
        break;
      case "sell":
        onSell?.(selected.id, effectiveQuantity);
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
        : onConfirmRepair;
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
  const confirmEnabled = !blocked() && Boolean(actionHandler) && Boolean(selected) && !selected?.disabled && affordable
    && (!(sharedBuy || pearlBuy) || quoteCanBuy(buyQuote, pearlBuy)) && (!repairTab || repairSelectionCurrent);

  if (repairTab) {
    const targetSelection = repairTargetSelection?.stamp === repairView?.stamp ? repairTargetSelection : null;
    const target = targetSelection ? repairView?.rows.find(row => row.uniqueId === targetSelection.uniqueId) : null;
    const quote = target?.quote;
    const sprite = (base: number, hover: number, pressed: number) => ({ base: originalAssetPath(`/original-ui/Title/${base}.png`),
      hover: originalAssetPath(`/original-ui/Title/${hover}.png`), pressed: originalAssetPath(`/original-ui/Title/${pressed}.png`) });
    return <section className="npc-shop-window" data-ui-interactive="true" data-shop-tab={tab}
      aria-label={npcName || actionLabel} style={{ position: "absolute", left: 264, top: 224, width: 176, height: 147, zIndex: 30 }}>
      <img src={originalAssetPath("/original-ui/Prguse2/351.png")} alt="" draggable={false}
        style={{ position: "absolute", width: 176, height: 147, pointerEvents: "none" }} />
      <div style={{ position: "absolute", left: 30, top: 10, width: 140, height: 20, fontSize: 10, color: "#f4ecd6" }}>
        {actionLabel}: {quote ? quote.displayedTotal.toLocaleString("en-US") : "—"}
      </div>
      <div ref={repairTargetRef} aria-label={t("ui.npcRepairTarget", [], "Drag a bag item here to repair")}
        title={target?.name || t("ui.npcRepairTarget", [], "Drag a bag item here to repair")}
        style={{ position: "absolute", left: 20, top: 55, width: 75, height: 75, touchAction: "none" }}>
        {target ? <img src={originalItemIconPath(target.icon)} alt={target.name} draggable={false}
          style={{ position: "absolute", left: 18, top: 17, width: 36, height: 32, objectFit: "contain", pointerEvents: "none" }} /> : null}
      </div>
      <div data-npc-repair-control="hold" style={{ position: "absolute", left: 114, top: 36, width: 48, height: 25 }}>
        <SpriteButton sprite={{ ...sprite(293, 294, 295), active: originalAssetPath("/original-ui/Title/295.png") }} active={repairView?.hold === true}
          label={t("ui.npcRepairHold", [], "Hold") + (repairView?.hold ? " (on)" : " (off)")}
          disabled={!repairView || blocked() || !onToggleRepairHold}
          onClick={() => { if (repairView && !blocked()) onToggleRepairHold?.(repairView); }} />
      </div>
      <div data-npc-repair-control="confirm" style={{ position: "absolute", left: 114, top: 62, width: 48, height: 25 }}>
        <SpriteButton sprite={sprite(290, 291, 292)} label={actionLabel}
          disabled={!targetSelection || !target || target.disabled || blocked() || !onConfirmRepair}
          onClick={() => { if (targetSelection && target && !target.disabled && !blocked()) onConfirmRepair?.(targetSelection); }} />
      </div>
      <div style={{ position: "absolute", left: 14, top: 132, width: 148, color: target?.reason === "gold" ? "#ff8888" : "#f4ecd6", fontSize: 10 }}>
        {!repairView ? t("ui.itemStateSync", [], "Item state is not ready.")
          : target?.reason === "gold" ? t("client.LowGold", [], "Not enough gold.")
          : target ? `${target.input?.currentDura}/${target.input?.maxDura}` : t("ui.npcRepairBagDrag", [], "Drag from your bag.")}
      </div>
      <div data-npc-repair-control="close" style={{ position: "absolute", left: 145, top: 106, width: 25, height: 25 }}>
        <SpriteButton sprite={ORIGINAL_UI.inventory.closeButton} label={t("ui.close", [], "Close")}
          onClick={() => { if (!blocked()) onClose(); }} />
      </div>
    </section>;
  }

  return (
    <section className="npc-shop-window" aria-label={t("ui.shopTitle", [], "Shop")} data-shop-tab={tab} aria-disabled={inputBlocked} inert={inputBlocked || undefined} style={shopStyle.window}>
      <div style={shopStyle.header}>
        <strong style={shopStyle.title}>{npcName?.trim() || t("ui.shopTitle", [], "Shop")}</strong>
        <div className="npc-shop-close" style={shopStyle.close}>
          <SpriteButton sprite={ORIGINAL_UI.inventory.closeButton} label={t("ui.close", [], "Close")} onClick={() => { if (!blocked()) onClose(); }} />
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
              if (blocked()) return;
              if (controlledTab !== undefined) onTabChange?.(entry); else setLocalTab(entry);
              setSelectedId(null);
              setRepairSelection(null);
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
              if (blocked()) return;
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
              if (blocked()) return;
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
          {t("client.Price", [], "Price")}
        </span>
      </div>

      <div className="npc-shop-list" style={shopStyle.list}>
        {rows.length ? (
          rows.map((row) => {
            const isSelected = row.id === selectedId;
            const colour = row.grade ? NPC_SHOP_GRADE_COLOUR[row.grade] : "#f4ecd6";
            const pearlRow = tab === "buy" && !showBuyBack && (currency === "pearls"
              || (row as NpcShopGood).requiresPearlBuyPlan === true);
            const rowQuote = pearlRow && !blocked() ? onQuoteBuy?.(row.id, 1) : null;
            const unaffordable = pearlRow ? pearlsKnown && rowQuote?.currency === "pearls"
              && typeof rowQuote.totalPearls === "number" && rowQuote.totalPearls > displayedPearls!
              : goldKnown && tab === "buy" && !showBuyBack && Math.trunc(row.price) > gold!;
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
                disabled={inputBlocked || rowDisabled}
                onClick={() => {
                  if (blocked()) return;
                  setSelectedId(row.id);
                  setQuantity(1);
                }}
                onDoubleClick={() => {
                  if (blocked()) return;
                  setSelectedId(row.id);
                  if (row.disabled) return;
                  if (tab === "buy") {
                    const usesPearls = currency === "pearls" || (row as NpcShopGood).requiresPearlBuyPlan === true;
                    const usesSharedQuote = usesPearls || !showBuyBack && Boolean(onQuoteBuy)
                      && (row as NpcShopGood).requiresGoldBuyPlan !== false;
                    if (usesSharedQuote) {
                      const currentQuote = onQuoteBuy?.(row.id, 1);
                      if (!quoteCanBuy(currentQuote, usesPearls) || blocked()) return;
                    } else if (!confirmEnabledFor(row, tab, showBuyBack, gold)) return;
                    (showBuyBack ? onBuyBack : onBuy)?.(row.id, 1);
                  } else if (tab === "sell" && confirmEnabledFor(row, tab, showBuyBack, gold)) onSell?.(row.id, 1);
                }}
              >
                <span style={shopStyle.rowIconArea}>
                  <img style={shopStyle.rowIcon} src={originalItemIconPath(row.icon)} alt="" draggable={false} />
                  {row.count && row.count > 1 ? <span style={shopStyle.rowCount}>{row.count}</span> : null}
                </span>
                <span style={shopStyle.rowName}>
                  <span style={{ color: colour, fontWeight: 700 }}>{row.name}</span>
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
              onClick={() => { if (!blocked()) setQuantity((current) => Math.max(1, Math.min(maxQuantity, current) - 1)); }}
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
                if (blocked()) return;
                const parsed = Number.parseInt(event.target.value, 10);
                setQuantity(Number.isFinite(parsed) ? parsed : 1);
              }}
            />
            <button
              type="button"
              style={shopStyle.stepButton}
              disabled={effectiveQuantity >= maxQuantity}
              aria-label={t("ui.splitIncrease", [], "More")}
              onClick={() => { if (!blocked()) setQuantity((current) => Math.min(maxQuantity, Math.max(1, current) + 1)); }}
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
          title={!affordable ? pearlBuy
            ? t("ui.shopLowPearls", [], "Not enough pearls, or the pearl balance is unavailable.")
            : t("client.LowGold", [], "Not enough gold.") : undefined}
          onClick={confirm}
        >
          {actionLabel}
        </button>
      </div>

      <div style={shopStyle.footer}>
        <span style={shopStyle.footerLabel}>{t("ui.shopTotal", [], "Total")}</span>
        <span style={{ ...shopStyle.footerValue, ...(!affordable ? shopStyle.rowPriceBad : null) }}>{total === null ? "—" : formatGold(total)}</span>
        <span style={shopStyle.footerSpacer} />
        <span style={shopStyle.footerLabel}>{pearlBuy ? t("client.Pearls", [], "Pearls") : t("client.Gold", [], "Gold")}</span>
        <span style={shopStyle.footerValue}>{pearlBuy ? displayedPearls === null ? "—" : formatGold(displayedPearls)
          : goldKnown ? formatGold(gold!) : "—"}</span>
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
