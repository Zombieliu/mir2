"use client";

import { useEffect, useMemo, useState, type CSSProperties } from "react";

import { ORIGINAL_UI } from "../../lib/original-ui";
import { CITY_CURRENCY_LABELS } from "../../lib/stage5-window-adapters";
import { SpriteButton } from "./original-client-overlays";

type TranslateFn = (
  key: string,
  params?: Array<string | number>,
  fallback?: string,
) => string;

/**
 * A single market / auction listing. Derived from the loosely-typed
 * `stage5Systems.auction` array; the adapter fills in defensively.
 */
export type MarketListing = {
  id: string;
  itemName: string;
  seller: string;
  price: number;
  /** Optional item icon index into `/original-ui/Items/<icon>.png`. */
  icon?: number;
  count?: number;
  /** Listing state label, e.g. "Open" / "Sold". */
  state?: string;
  /** True when the listing belongs to the viewer (so they can cancel it). */
  mine?: boolean;
  /** Item category label used by the type filter ("Weapon", "Armour", …). */
  type?: string;
  /** Required level, used by the level filter and as a row hint. */
  level?: number;
  /** Remaining-time label for auctions/consignments (Crystal Expiry column). */
  expiry?: string;
  /** Current highest bid for auction listings. */
  highestBid?: number;
  /** Whether this listing is an auction (bid) rather than a fixed-price sale. */
  auction?: boolean;
  /** Whether this listing has sold and is awaiting collection (Consign tab). */
  sold?: boolean;
  /**
   * Currency the listing is priced in: `"gold"` (default) or a city
   * reputation token key (`"feitian"`, `"bichon"`). Net-new / optional —
   * legacy gold listings omit it.
   */
  currency?: string;
  /** Human-readable currency label for the price column (e.g. "飞天城币"). */
  currencyLabel?: string;
};

/** Which board the window is showing. */
export type MarketMode = "browse" | "mine" | "auction";

/** Sort options for the listing table. */
export type MarketSortKey = "name" | "price" | "seller" | "level" | "expiry";

export type MarketWindowProps = {
  t: TranslateFn;
  listings: MarketListing[];
  /** Viewer gold, used to gate the buy button. */
  gold?: number;
  /**
   * Viewer's per-city reputation currency wallet, keyed by city
   * (`"feitian"`, `"bichon"`). Net-new / optional — used to gate the buy
   * button and display balances for listings priced in city currency.
   */
  cityCurrencies?: Record<string, number>;
  /** Distinct item types offered in the type filter (defaults to derived set). */
  itemTypes?: string[];
  onBuy?: (listingId: string) => void;
  onCancel?: (listingId: string) => void;
  /** Place a bid on an auction listing. */
  onBid?: (listingId: string) => void;
  /** Collect gold/items from a sold consignment. */
  onCollect?: (listingId: string) => void;
  /** Opens the host's "list an item for sale" flow. */
  onList?: () => void;
  onSearch?: (query: string) => void;
  onRefresh?: () => void;
  onClose: () => void;
};

const FRAME = ORIGINAL_UI.mail;

const ROWS_PER_PAGE = 9;

const MARKET_TABS: { key: MarketMode; labelKey: string; fallback: string }[] = [
  { key: "browse", labelKey: "ui.marketTabBrowse", fallback: "Market" },
  { key: "auction", labelKey: "ui.marketTabAuction", fallback: "Auction" },
  { key: "mine", labelKey: "ui.marketTabMine", fallback: "My Listings" },
];

export function MarketWindow({
  t,
  listings,
  gold,
  cityCurrencies,
  itemTypes,
  onBuy,
  onCancel,
  onBid,
  onCollect,
  onList,
  onSearch,
  onRefresh,
  onClose,
}: MarketWindowProps) {
  const [mode, setMode] = useState<MarketMode>("browse");
  const [query, setQuery] = useState("");
  const [typeFilter, setTypeFilter] = useState("");
  const [maxLevel, setMaxLevel] = useState("");
  const [sortKey, setSortKey] = useState<MarketSortKey>("price");
  const [sortAsc, setSortAsc] = useState(true);
  const [page, setPage] = useState(0);
  const [selectedId, setSelectedId] = useState<string | null>(null);

  const derivedTypes = useMemo(() => {
    if (itemTypes && itemTypes.length > 0) return itemTypes;
    const set = new Set<string>();
    for (const listing of listings) {
      if (listing.type) set.add(listing.type);
    }
    return [...set].sort();
  }, [itemTypes, listings]);

  const filtered = useMemo(() => {
    const needle = query.trim().toLowerCase();
    const levelCap = maxLevel ? Number.parseInt(maxLevel, 10) : null;
    const result = listings.filter((listing) => {
      if (mode === "mine" && !listing.mine) return false;
      if (mode === "auction" && !listing.auction) return false;
      if (mode === "browse" && listing.auction) return false;
      if (needle && !listing.itemName.toLowerCase().includes(needle) && !listing.seller.toLowerCase().includes(needle)) {
        return false;
      }
      if (typeFilter && listing.type !== typeFilter) return false;
      if (levelCap !== null && Number.isFinite(levelCap) && typeof listing.level === "number" && listing.level > levelCap) {
        return false;
      }
      return true;
    });
    return sortListings(result, sortKey, sortAsc);
  }, [listings, mode, query, typeFilter, maxLevel, sortKey, sortAsc]);

  const pageCount = Math.max(1, Math.ceil(filtered.length / ROWS_PER_PAGE));
  const currentPage = Math.min(page, pageCount - 1);
  const visible = filtered.slice(currentPage * ROWS_PER_PAGE, currentPage * ROWS_PER_PAGE + ROWS_PER_PAGE);

  const selected = useMemo(() => {
    if (selectedId) {
      const match = filtered.find((listing) => listing.id === selectedId);
      if (match) return match;
    }
    return visible[0] ?? filtered[0] ?? null;
  }, [filtered, selectedId, visible]);

  useEffect(() => {
    setPage(0);
  }, [mode, typeFilter, maxLevel, query]);

  useEffect(() => {
    if (page > pageCount - 1) {
      setPage(pageCount - 1);
    }
  }, [page, pageCount]);

  // Affordability is checked against the listing's own currency: gold listings
  // use the gold balance; city-currency listings use the matching wallet entry.
  const selectedBalance = (() => {
    if (selected == null) return undefined;
    if (selected.currency && selected.currency !== "gold") {
      return cityCurrencies?.[selected.currency];
    }
    return gold;
  })();
  const canAfford =
    selected != null && typeof selectedBalance === "number"
      ? selectedBalance >= selected.price
      : true;
  const isAuction = mode === "auction" || selected?.auction === true;

  const toggleSort = (key: MarketSortKey) => {
    if (key === sortKey) {
      setSortAsc((prev) => !prev);
    } else {
      setSortKey(key);
      setSortAsc(true);
    }
  };

  return (
    <section
      aria-label={t("ui.market", [], "Market")}
      data-market-count={listings.length}
      data-market-mode={mode}
      data-market-selected={selected?.id ?? ""}
      style={style.window}
    >
      <img style={style.frame} src={FRAME.frame} alt="" draggable={false} />
      <img style={style.title} src={FRAME.title} alt="" draggable={false} />
      <div style={style.titleText}>{t("ui.market", [], "Market")}</div>
      <div style={style.close}>
        <SpriteButton sprite={FRAME.closeButton} label={t("ui.close", [], "Close")} onClick={onClose} />
      </div>
      <div style={style.help}>
        <SpriteButton sprite={FRAME.helpButton} label={t("ui.help", [], "Help")} onClick={() => undefined} />
      </div>

      <div style={style.tabs} role="tablist" aria-label={t("ui.market", [], "Market")}>
        {MARKET_TABS.map((entry) => {
          const active = entry.key === mode;
          return (
            <button
              key={entry.key}
              type="button"
              role="tab"
              aria-selected={active}
              data-market-tab={entry.key}
              onClick={() => setMode(entry.key)}
              style={{ ...style.tab, ...(active ? style.tabActive : null) }}
            >
              {t(entry.labelKey, [], entry.fallback)}
            </button>
          );
        })}
      </div>

      <form
        style={style.searchRow}
        onSubmit={(event) => {
          event.preventDefault();
          onSearch?.(query.trim());
        }}
      >
        <input
          style={style.input}
          value={query}
          onChange={(event) => setQuery(event.target.value)}
          placeholder={t("ui.marketSearchPlaceholder", [], "Search item or seller")}
          aria-label={t("ui.marketSearch", [], "Search")}
          autoComplete="off"
          spellCheck={false}
        />
        <button
          type="submit"
          disabled={!onSearch}
          style={{ ...style.smallButton, ...(!onSearch ? style.actionButtonDisabled : null) }}
        >
          {t("ui.marketSearch", [], "Search")}
        </button>
      </form>

      <div style={style.filterRow}>
        <select
          style={style.select}
          value={typeFilter}
          onChange={(event) => setTypeFilter(event.target.value)}
          aria-label={t("ui.marketFilterType", [], "Type")}
        >
          <option value="">{t("ui.marketTypeAll", [], "All types")}</option>
          {derivedTypes.map((type) => (
            <option key={type} value={type}>
              {type}
            </option>
          ))}
        </select>
        <input
          style={style.levelInput}
          value={maxLevel}
          inputMode="numeric"
          onChange={(event) => setMaxLevel(event.target.value.replace(/[^0-9]/g, ""))}
          placeholder={t("ui.marketMaxLevel", [], "Max Lv")}
          aria-label={t("ui.marketMaxLevel", [], "Max level")}
          maxLength={3}
        />
      </div>

      <div style={style.listHead}>
        <SortHeader style={style.colName} label={t("ui.marketItem", [], "Item")} active={sortKey === "name"} asc={sortAsc} onClick={() => toggleSort("name")} />
        <SortHeader style={style.colSeller} label={t("ui.marketSeller", [], "Seller")} active={sortKey === "seller"} asc={sortAsc} onClick={() => toggleSort("seller")} />
        <SortHeader style={style.colPrice} label={isAuction ? t("ui.marketBidCol", [], "Bid") : t("ui.marketPrice", [], "Price")} active={sortKey === "price"} asc={sortAsc} onClick={() => toggleSort("price")} />
      </div>
      <div style={style.list} aria-label={t("ui.market", [], "Market")}>
        {visible.length === 0 ? (
          <div style={style.empty}>
            {mode === "mine"
              ? t("ui.marketMineEmpty", [], "You have no active listings.")
              : t("ui.marketEmpty", [], "No listings available.")}
          </div>
        ) : (
          visible.map((listing) => {
            const isSelected = selected?.id === listing.id;
            const priceShown = listing.auction && typeof listing.highestBid === "number" ? listing.highestBid : listing.price;
            return (
              <button
                key={listing.id}
                type="button"
                data-listing-id={listing.id}
                aria-pressed={isSelected}
                onClick={() => setSelectedId(listing.id)}
                style={{ ...style.row, ...(isSelected ? style.rowSelected : null) }}
              >
                {typeof listing.icon === "number" ? (
                  <img style={style.rowIcon} src={iconPath(listing.icon)} alt="" draggable={false} />
                ) : (
                  <span style={style.rowIconFallback} aria-hidden="true" />
                )}
                <span style={style.colName}>
                  {listing.itemName}
                  {listing.count && listing.count > 1 ? <span style={style.countTag}>{`x${listing.count}`}</span> : null}
                  {listing.sold ? <span style={style.soldTag}>{t("ui.marketSold", [], "Sold")}</span> : null}
                </span>
                <span style={{ ...style.colSeller, ...(listing.mine ? style.sellerSelf : null) }}>
                  {listing.mine ? t("ui.marketYou", [], "You") : listing.seller}
                </span>
                <span style={style.colPrice}>
                  {formatNumber(priceShown)}
                  {listing.currencyLabel ? <span style={style.countTag}>{listing.currencyLabel}</span> : null}
                </span>
              </button>
            );
          })
        )}
      </div>

      <div style={style.pagePrev}>
        <SpriteButton
          sprite={FRAME.previousButton}
          label={t("ui.previous", [], "Previous")}
          onClick={() => setPage((current) => Math.max(0, current - 1))}
        />
      </div>
      <div style={style.pageLabel}>{`${currentPage + 1} / ${pageCount}`}</div>
      <div style={style.pageNext}>
        <SpriteButton
          sprite={FRAME.nextButton}
          label={t("ui.next", [], "Next")}
          onClick={() => setPage((current) => Math.min(pageCount - 1, current + 1))}
        />
      </div>

      <div style={style.detail} data-listing-detail={selected?.id ?? ""}>
        {selected ? (
          <>
            <div style={style.detailName}>{selected.itemName}</div>
            <div style={style.detailRow}>
              <span style={style.detailLabel}>{t("ui.marketSeller", [], "Seller")}</span>
              <span style={style.detailValue}>{selected.mine ? t("ui.marketYou", [], "You") : selected.seller}</span>
            </div>
            {selected.auction ? (
              <div style={style.detailRow}>
                <span style={style.detailLabel}>{t("ui.marketBidCol", [], "Bid")}</span>
                <span style={{ ...style.detailValue, color: canAfford ? "#f0d69b" : "#e08a6a" }}>
                  {formatNumber(typeof selected.highestBid === "number" ? selected.highestBid : selected.price)}
                </span>
              </div>
            ) : (
              <div style={style.detailRow}>
                <span style={style.detailLabel}>{t("ui.marketPrice", [], "Price")}</span>
                <span style={{ ...style.detailValue, color: canAfford ? "#f0d69b" : "#e08a6a" }}>
                  {formatNumber(selected.price)}
                </span>
              </div>
            )}
            {selected.expiry ? (
              <div style={style.detailRow}>
                <span style={style.detailLabel}>{t("ui.marketExpiry", [], "Expiry")}</span>
                <span style={style.detailValue}>{selected.expiry}</span>
              </div>
            ) : (
              <div style={style.detailRow}>
                <span style={style.detailLabel}>{t("ui.marketState", [], "State")}</span>
                <span style={style.detailValue}>{selected.state ?? t("ui.marketOpen", [], "Open")}</span>
              </div>
            )}
            {typeof gold === "number" ? (
              <div style={style.detailRow}>
                <span style={style.detailLabel}>{t("ui.gold", [], "Gold")}</span>
                <span style={style.detailValue}>{formatNumber(gold)}</span>
              </div>
            ) : null}
            {cityCurrencies
              ? Object.entries(cityCurrencies).map(([key, amount]) => (
                  <div key={key} style={style.detailRow}>
                    <span style={style.detailLabel}>{CITY_CURRENCY_LABELS[key] ?? key}</span>
                    <span style={style.detailValue}>{formatNumber(amount)}</span>
                  </div>
                ))
              : null}
          </>
        ) : (
          <div style={style.empty}>{t("ui.marketSelectHint", [], "Select a listing.")}</div>
        )}
      </div>

      <div style={style.actions}>
        {mode === "mine" ? (
          <>
            <button
              type="button"
              disabled={!selected || !selected.sold || !onCollect}
              style={{ ...style.actionButton, ...(!selected || !selected.sold || !onCollect ? style.actionButtonDisabled : null) }}
              onClick={() => selected && onCollect?.(selected.id)}
            >
              {t("ui.marketCollect", [], "Collect")}
            </button>
            <button
              type="button"
              disabled={!selected || !selected.mine || !onCancel}
              style={{ ...style.actionButton, ...(!selected || !selected.mine || !onCancel ? style.actionButtonDisabled : null) }}
              onClick={() => selected && onCancel?.(selected.id)}
            >
              {t("ui.marketCancel", [], "Cancel")}
            </button>
          </>
        ) : isAuction ? (
          <button
            type="button"
            disabled={!selected || selected.mine || !onBid}
            style={{ ...style.actionButton, ...style.actionButtonWide, ...(!selected || selected.mine || !onBid ? style.actionButtonDisabled : null) }}
            onClick={() => selected && onBid?.(selected.id)}
          >
            {t("ui.marketBid", [], "Place Bid")}
          </button>
        ) : (
          <button
            type="button"
            disabled={!selected || selected.mine || !onBuy || !canAfford}
            style={{
              ...style.actionButton,
              ...style.actionButtonWide,
              ...(!selected || selected.mine || !onBuy || !canAfford ? style.actionButtonDisabled : null),
            }}
            onClick={() => selected && onBuy?.(selected.id)}
          >
            {t("ui.marketBuy", [], "Buy")}
          </button>
        )}
      </div>

      <div style={style.footer}>
        <button
          type="button"
          disabled={!onList}
          style={{ ...style.actionButton, ...(!onList ? style.actionButtonDisabled : null) }}
          onClick={() => onList?.()}
        >
          {t("ui.marketConsign", [], "Consign")}
        </button>
        <button
          type="button"
          disabled={!onRefresh}
          style={{ ...style.actionButton, ...(!onRefresh ? style.actionButtonDisabled : null) }}
          onClick={() => onRefresh?.()}
        >
          {t("ui.refresh", [], "Refresh")}
        </button>
      </div>
    </section>
  );
}

function SortHeader({
  style: cellStyle,
  label,
  active,
  asc,
  onClick,
}: {
  style: CSSProperties;
  label: string;
  active: boolean;
  asc: boolean;
  onClick: () => void;
}) {
  return (
    <button type="button" onClick={onClick} style={{ ...style.sortHeader, ...cellStyle, ...(active ? style.sortHeaderActive : null) }}>
      {label}
      {active ? <span style={style.sortArrow}>{asc ? "▲" : "▼"}</span> : null}
    </button>
  );
}

function sortListings(listings: MarketListing[], key: MarketSortKey, asc: boolean): MarketListing[] {
  const dir = asc ? 1 : -1;
  return [...listings].sort((a, b) => {
    let cmp = 0;
    switch (key) {
      case "name":
        cmp = a.itemName.localeCompare(b.itemName);
        break;
      case "seller":
        cmp = a.seller.localeCompare(b.seller);
        break;
      case "level":
        cmp = (a.level ?? 0) - (b.level ?? 0);
        break;
      case "expiry":
        cmp = (a.expiry ?? "").localeCompare(b.expiry ?? "");
        break;
      case "price":
      default:
        cmp = priceOf(a) - priceOf(b);
        break;
    }
    return cmp * dir;
  });
}

function priceOf(listing: MarketListing): number {
  return listing.auction && typeof listing.highestBid === "number" ? listing.highestBid : listing.price;
}

function iconPath(icon: number) {
  return `/original-ui/Items/${icon}.png`;
}

function formatNumber(value: number) {
  return value.toLocaleString("en-US");
}

const style: Record<string, CSSProperties> = {
  window: {
    position: "absolute",
    left: 250,
    top: 5,
    width: FRAME.width,
    height: FRAME.height,
    zIndex: 29,
    color: "#f0eee8",
    fontSize: 12,
    textShadow: "1px 1px 0 #000",
    fontFamily: "inherit",
  },
  frame: { position: "absolute", inset: 0, width: FRAME.width, height: FRAME.height, pointerEvents: "none" },
  title: { position: "absolute", left: 18, top: 9 },
  titleText: {
    position: "absolute",
    left: 18,
    top: 8,
    height: 16,
    lineHeight: "16px",
    fontSize: 12,
    fontWeight: 700,
    color: "#f4dcaf",
    letterSpacing: 0.5,
  },
  close: { position: "absolute", left: 288, top: 3 },
  help: { position: "absolute", left: 262, top: 3 },
  tabs: { position: "absolute", left: 10, top: 28, width: 292, display: "flex", gap: 3 },
  tab: {
    flex: 1,
    border: "1px solid rgba(190, 157, 99, 0.5)",
    background: "linear-gradient(180deg, rgba(52, 32, 18, 0.92), rgba(28, 17, 9, 0.92))",
    color: "#cbb38a",
    padding: "3px 0",
    fontSize: 10,
    cursor: "pointer",
  },
  tabActive: {
    background: "linear-gradient(180deg, rgba(120, 74, 34, 0.96), rgba(70, 40, 20, 0.96))",
    color: "#f8e6bb",
    borderColor: "rgba(214, 180, 110, 0.85)",
  },
  searchRow: { position: "absolute", left: 10, top: 52, width: 292, display: "flex", gap: 5 },
  filterRow: { position: "absolute", left: 10, top: 76, width: 292, display: "flex", gap: 5 },
  select: {
    flex: 1,
    border: "1px solid rgba(190, 157, 99, 0.5)",
    background: "rgba(11, 8, 5, 0.7)",
    color: "#f0eee8",
    padding: "3px 4px",
    fontSize: 10,
    fontFamily: "inherit",
  },
  levelInput: {
    flex: "0 0 64px",
    border: "1px solid rgba(190, 157, 99, 0.5)",
    background: "rgba(11, 8, 5, 0.7)",
    color: "#f0eee8",
    padding: "3px 6px",
    fontSize: 10,
    fontFamily: "inherit",
  },
  listHead: {
    position: "absolute",
    left: 12,
    top: 100,
    width: 288,
    display: "flex",
    padding: "0 6px 3px",
    fontSize: 9,
    color: "#a89568",
    textTransform: "uppercase",
    letterSpacing: 0.5,
    borderBottom: "1px solid rgba(190, 157, 99, 0.28)",
  },
  sortHeader: {
    border: "none",
    background: "none",
    color: "#a89568",
    font: "inherit",
    fontSize: 9,
    textTransform: "uppercase",
    letterSpacing: 0.5,
    cursor: "pointer",
    padding: 0,
    display: "flex",
    alignItems: "center",
    gap: 2,
    textShadow: "1px 1px 0 #000",
  },
  sortHeaderActive: { color: "#f4dcaf" },
  sortArrow: { fontSize: 7 },
  list: {
    position: "absolute",
    left: 10,
    top: 116,
    width: 292,
    height: 160,
    display: "flex",
    flexDirection: "column",
    gap: 2,
    overflow: "hidden",
  },
  empty: { color: "#cbb38a", padding: "8px 4px", fontSize: 11 },
  row: {
    display: "flex",
    alignItems: "center",
    gap: 6,
    width: "100%",
    height: 22,
    padding: "0 6px",
    border: "1px solid transparent",
    background: "rgba(20, 13, 7, 0.4)",
    color: "#e3d3af",
    textAlign: "left",
    cursor: "pointer",
  },
  rowSelected: { background: "rgba(95, 53, 24, 0.5)", borderColor: "rgba(214, 180, 110, 0.7)" },
  rowIcon: { width: 16, height: 16, imageRendering: "pixelated", flex: "0 0 auto" },
  rowIconFallback: {
    width: 16,
    height: 16,
    flex: "0 0 auto",
    border: "1px solid rgba(190, 157, 99, 0.4)",
    background: "rgba(0, 0, 0, 0.4)",
  },
  colName: {
    flex: "1 1 auto",
    minWidth: 0,
    overflow: "hidden",
    textOverflow: "ellipsis",
    whiteSpace: "nowrap",
    display: "flex",
    alignItems: "center",
    gap: 4,
  },
  countTag: { fontSize: 9, color: "#b7a884" },
  soldTag: { fontSize: 9, color: "#8be07a" },
  colSeller: { flex: "0 0 84px", overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" },
  sellerSelf: { color: "#8be07a" },
  colPrice: { flex: "0 0 64px", textAlign: "right", color: "#f0d69b", justifyContent: "flex-end" },
  pagePrev: { position: "absolute", left: 132, top: 280 },
  pageLabel: {
    position: "absolute",
    left: 150,
    top: 280,
    width: 64,
    textAlign: "center",
    fontSize: 11,
    color: "#cbb38a",
    lineHeight: "16px",
  },
  pageNext: { position: "absolute", left: 214, top: 280 },
  detail: {
    position: "absolute",
    left: 12,
    top: 300,
    width: 288,
    height: 74,
    overflow: "hidden",
    border: "1px solid rgba(190, 157, 99, 0.32)",
    background: "linear-gradient(180deg, rgba(27, 19, 10, 0.78), rgba(11, 8, 5, 0.7))",
    padding: "6px 8px",
  },
  detailName: { color: "#f8e6bb", fontSize: 12, fontWeight: 700, marginBottom: 4 },
  detailRow: { display: "flex", justifyContent: "space-between", fontSize: 11, marginBottom: 2 },
  detailLabel: { color: "#a89568" },
  detailValue: { color: "#e3d3af" },
  actions: { position: "absolute", left: 12, top: 380, width: 288, display: "flex", gap: 6 },
  footer: { position: "absolute", left: 12, top: 410, width: 288, display: "flex", gap: 6 },
  smallButton: {
    flex: "0 0 64px",
    border: "1px solid rgba(190, 157, 99, 0.5)",
    background: "rgba(52, 32, 18, 0.86)",
    color: "#f4dcaf",
    padding: "4px 0",
    fontSize: 11,
    cursor: "pointer",
  },
  input: {
    flex: 1,
    border: "1px solid rgba(190, 157, 99, 0.5)",
    background: "rgba(11, 8, 5, 0.7)",
    color: "#f0eee8",
    padding: "4px 8px",
    fontSize: 11,
  },
  actionButton: {
    flex: 1,
    border: "1px solid rgba(190, 157, 99, 0.56)",
    background: "linear-gradient(180deg, rgba(95, 53, 24, 0.95), rgba(45, 23, 12, 0.95))",
    color: "#f4dcaf",
    padding: "4px 0",
    fontSize: 11,
    cursor: "pointer",
  },
  actionButtonWide: { flex: 1 },
  actionButtonDisabled: { opacity: 0.45, cursor: "default" },
};
