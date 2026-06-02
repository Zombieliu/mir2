"use client";

import { useEffect, useMemo, useState, type CSSProperties } from "react";

import { ORIGINAL_UI } from "../../lib/original-ui";
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
};

export type MarketWindowProps = {
  t: TranslateFn;
  listings: MarketListing[];
  /** Viewer gold, used to gate the buy button. */
  gold?: number;
  onBuy?: (listingId: string) => void;
  onCancel?: (listingId: string) => void;
  /** Opens the host's "list an item for sale" flow. */
  onList?: () => void;
  onSearch?: (query: string) => void;
  onRefresh?: () => void;
  onClose: () => void;
};

const FRAME = ORIGINAL_UI.mail;

const ROWS_PER_PAGE = 9;

export function MarketWindow({
  t,
  listings,
  gold,
  onBuy,
  onCancel,
  onList,
  onSearch,
  onRefresh,
  onClose,
}: MarketWindowProps) {
  const [query, setQuery] = useState("");
  const [page, setPage] = useState(0);
  const [selectedId, setSelectedId] = useState<string | null>(null);

  const filtered = useMemo(() => {
    const needle = query.trim().toLowerCase();
    if (!needle) return listings;
    return listings.filter(
      (listing) =>
        listing.itemName.toLowerCase().includes(needle) ||
        listing.seller.toLowerCase().includes(needle),
    );
  }, [listings, query]);

  const pageCount = Math.max(1, Math.ceil(filtered.length / ROWS_PER_PAGE));
  const currentPage = Math.min(page, pageCount - 1);
  const visible = filtered.slice(currentPage * ROWS_PER_PAGE, currentPage * ROWS_PER_PAGE + ROWS_PER_PAGE);

  const selected = useMemo(() => {
    if (selectedId) {
      const match = listings.find((listing) => listing.id === selectedId);
      if (match) return match;
    }
    return visible[0] ?? filtered[0] ?? null;
  }, [filtered, listings, selectedId, visible]);

  useEffect(() => {
    if (page > pageCount - 1) {
      setPage(pageCount - 1);
    }
  }, [page, pageCount]);

  const canAfford = selected != null && typeof gold === "number" ? gold >= selected.price : true;

  return (
    <section
      aria-label={t("ui.market", [], "Market")}
      data-market-count={listings.length}
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

      <div style={style.listHead}>
        <span style={style.colName}>{t("ui.marketItem", [], "Item")}</span>
        <span style={style.colSeller}>{t("ui.marketSeller", [], "Seller")}</span>
        <span style={style.colPrice}>{t("ui.marketPrice", [], "Price")}</span>
      </div>
      <div style={style.list} aria-label={t("ui.market", [], "Market")}>
        {visible.length === 0 ? (
          <div style={style.empty}>{t("ui.marketEmpty", [], "No listings available.")}</div>
        ) : (
          visible.map((listing) => {
            const isSelected = selected?.id === listing.id;
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
                </span>
                <span style={{ ...style.colSeller, ...(listing.mine ? style.sellerSelf : null) }}>
                  {listing.mine ? t("ui.marketYou", [], "You") : listing.seller}
                </span>
                <span style={style.colPrice}>{formatNumber(listing.price)}</span>
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
            <div style={style.detailRow}>
              <span style={style.detailLabel}>{t("ui.marketPrice", [], "Price")}</span>
              <span style={{ ...style.detailValue, color: canAfford ? "#f0d69b" : "#e08a6a" }}>
                {formatNumber(selected.price)}
              </span>
            </div>
            <div style={style.detailRow}>
              <span style={style.detailLabel}>{t("ui.marketState", [], "State")}</span>
              <span style={style.detailValue}>{selected.state ?? t("ui.marketOpen", [], "Open")}</span>
            </div>
            {typeof gold === "number" ? (
              <div style={style.detailRow}>
                <span style={style.detailLabel}>{t("ui.gold", [], "Gold")}</span>
                <span style={style.detailValue}>{formatNumber(gold)}</span>
              </div>
            ) : null}
          </>
        ) : (
          <div style={style.empty}>{t("ui.marketSelectHint", [], "Select a listing.")}</div>
        )}
      </div>

      <div style={style.actions}>
        <button
          type="button"
          disabled={!selected || selected.mine || !onBuy || !canAfford}
          style={{
            ...style.actionButton,
            ...(!selected || selected.mine || !onBuy || !canAfford ? style.actionButtonDisabled : null),
          }}
          onClick={() => selected && onBuy?.(selected.id)}
        >
          {t("ui.marketBuy", [], "Buy")}
        </button>
        <button
          type="button"
          disabled={!selected || !selected.mine || !onCancel}
          style={{
            ...style.actionButton,
            ...(!selected || !selected.mine || !onCancel ? style.actionButtonDisabled : null),
          }}
          onClick={() => selected && onCancel?.(selected.id)}
        >
          {t("ui.marketCancel", [], "Cancel")}
        </button>
      </div>

      <div style={style.footer}>
        <button
          type="button"
          disabled={!onList}
          style={{ ...style.actionButton, ...(!onList ? style.actionButtonDisabled : null) }}
          onClick={() => onList?.()}
        >
          {t("ui.marketList", [], "List Item")}
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
  searchRow: { position: "absolute", left: 10, top: 30, width: 292, display: "flex", gap: 5 },
  listHead: {
    position: "absolute",
    left: 12,
    top: 58,
    width: 288,
    display: "flex",
    padding: "0 6px 3px",
    fontSize: 9,
    color: "#a89568",
    textTransform: "uppercase",
    letterSpacing: 0.5,
    borderBottom: "1px solid rgba(190, 157, 99, 0.28)",
  },
  list: {
    position: "absolute",
    left: 10,
    top: 74,
    width: 292,
    height: 188,
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
  colSeller: { flex: "0 0 84px", overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" },
  sellerSelf: { color: "#8be07a" },
  colPrice: { flex: "0 0 64px", textAlign: "right", color: "#f0d69b" },
  pagePrev: { position: "absolute", left: 132, top: 266 },
  pageLabel: {
    position: "absolute",
    left: 150,
    top: 266,
    width: 64,
    textAlign: "center",
    fontSize: 11,
    color: "#cbb38a",
    lineHeight: "16px",
  },
  pageNext: { position: "absolute", left: 214, top: 266 },
  detail: {
    position: "absolute",
    left: 12,
    top: 288,
    width: 288,
    height: 86,
    overflow: "hidden",
    border: "1px solid rgba(190, 157, 99, 0.32)",
    background: "linear-gradient(180deg, rgba(27, 19, 10, 0.78), rgba(11, 8, 5, 0.7))",
    padding: "6px 8px",
  },
  detailName: { color: "#f8e6bb", fontSize: 12, fontWeight: 700, marginBottom: 4 },
  detailRow: { display: "flex", justifyContent: "space-between", fontSize: 11, marginBottom: 2 },
  detailLabel: { color: "#a89568" },
  detailValue: { color: "#e3d3af" },
  actions: { position: "absolute", left: 12, top: 378, width: 288, display: "flex", gap: 6 },
  footer: { position: "absolute", left: 12, top: 408, width: 288, display: "flex", gap: 6 },
  smallButton: {
    flex: "0 0 72px",
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
  actionButtonDisabled: { opacity: 0.45, cursor: "default" },
};
