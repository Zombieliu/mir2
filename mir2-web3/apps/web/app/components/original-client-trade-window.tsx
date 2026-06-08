"use client";

import { useState, type CSSProperties, type ReactNode } from "react";

import { ORIGINAL_UI } from "../../lib/original-ui";
import { SpriteButton } from "./original-client-overlays";

type TranslateFn = (
  key: string,
  params?: Array<string | number>,
  fallback?: string,
) => string;

/** A single item placed into one side of a trade (one of the 5x2 grid cells). */
export type TradeItemSlot = {
  /** Stable id for React keys (item unique id / slot index). */
  id: string | number;
  name?: string;
  /** Icon index into `/original-ui/Items/<icon>.png`. */
  icon?: number;
  count?: number;
  /** Per-item estimated value, summed into the side's total when present. */
  value?: number;
};

/**
 * Mirrors the stage-5 `trade` slice that the page maintains from the
 * `TradeRequest` / `TradeAccept` / `TradeGold` / `TradeItem` / `TradeConfirm`
 * packets:
 *
 *   { partner, state, partnerGold, partnerItemCount, confirmed }
 *
 * `state` is "requested" (an incoming offer awaiting accept) or "open" (the
 * trade window is live). The player's own side (gold offered / items locked)
 * is supplied separately by the host so the confirm button can gate on it.
 */
export type TradeSummary = {
  partner?: string;
  state?: string;
  partnerGold?: number;
  partnerItemCount?: number;
  /** Whether the *partner* has pressed confirm. */
  confirmed?: boolean;
  /** Optional partner item slots, mirroring Crystal's GuestTrade 5x2 grid. */
  partnerItems?: TradeItemSlot[];
  /**
   * Currency the partner's offered amount is denominated in: `"gold"`
   * (default) or a city reputation token key (`"feitian"`, `"bichon"`).
   * Net-new / optional.
   */
  partnerCurrency?: string;
  /** Human-readable label for the partner's offer currency. */
  partnerCurrencyLabel?: string;
};

export type TradeWindowProps = {
  t: TranslateFn;
  trade: TradeSummary | null;
  /** Gold the viewer has put up for the trade. */
  myGold?: number;
  /** Number of items the viewer has placed into the trade. */
  myItemCount?: number;
  /** Whether the viewer has already pressed confirm. */
  myConfirmed?: boolean;
  /** Optional viewer item slots, mirroring Crystal's Trade 5x2 grid. */
  myItems?: TradeItemSlot[];
  /** Total gold the viewer can still offer (caps the gold input). */
  availableGold?: number;
  /** Accept an incoming `requested` trade invite. */
  onAccept?: () => void;
  /** Lock in the viewer's side of an `open` trade. */
  onConfirm?: () => void;
  /** Cancel / decline the trade entirely. */
  onCancel?: () => void;
  /** Set the gold the viewer offers (Crystal gold-label amount box). */
  onSetGold?: (amount: number) => void;
  onClose: () => void;
};

const FRAME = ORIGINAL_UI.mail;

// Crystal's trade grid is 5 columns x 2 rows = 10 cells per side.
const TRADE_SLOT_COUNT = 10;

export function TradeWindow({
  t,
  trade,
  myGold,
  myItemCount,
  myConfirmed,
  myItems,
  availableGold,
  onAccept,
  onConfirm,
  onCancel,
  onSetGold,
  onClose,
}: TradeWindowProps) {
  const partner = trade?.partner?.trim() ?? "";
  const state = trade?.state ?? (partner ? "open" : "idle");
  const requested = state === "requested";
  const partnerConfirmed = trade?.confirmed === true;
  const selfConfirmed = myConfirmed === true;
  const bothConfirmed = partnerConfirmed && selfConfirmed;

  const myGoldValue = myGold ?? 0;
  const partnerGoldValue = trade?.partnerGold ?? 0;
  const myItemsList = myItems ?? [];
  const partnerItemsList = trade?.partnerItems ?? [];
  const myItemsCount = myItems ? myItems.length : myItemCount ?? 0;
  const partnerItemsCount = trade?.partnerItems ? trade.partnerItems.length : trade?.partnerItemCount ?? 0;

  const myTotal = myGoldValue + sumValue(myItemsList);
  const partnerTotal = partnerGoldValue + sumValue(partnerItemsList);

  const [goldDraft, setGoldDraft] = useState("");

  // Editing gold is only allowed before locking; the input commits on submit.
  const goldLocked = selfConfirmed || !onSetGold || !partner;

  return (
    <section
      aria-label={t("ui.trade", [], "Trade")}
      data-trade-state={state}
      data-trade-partner={partner}
      style={style.window}
    >
      <img style={style.frame} src={FRAME.frame} alt="" draggable={false} />
      <img style={style.title} src={FRAME.title} alt="" draggable={false} />
      <div style={style.titleText}>{t("ui.trade", [], "Trade")}</div>
      <div style={style.close}>
        <SpriteButton sprite={FRAME.closeButton} label={t("ui.close", [], "Close")} onClick={onClose} />
      </div>

      <div style={style.banner}>
        {partner
          ? requested
            ? t("ui.tradeIncoming", [partner], `${partner} wants to trade with you.`)
            : t("ui.tradeWith", [partner], `Trading with ${partner}`)
          : t("ui.tradeNone", [], "No active trade.")}
      </div>

      <TradeSide
        top={86}
        title={t("ui.tradeYourOffer", [], "Your Offer")}
        gold={myGoldValue}
        items={myItemsList}
        itemCount={myItemsCount}
        confirmed={selfConfirmed}
        accent="#caa64a"
        total={myTotal}
        t={t}
      >
        <form
          style={style.goldEditRow}
          onSubmit={(event) => {
            event.preventDefault();
            const amount = Number.parseInt(goldDraft, 10);
            if (Number.isFinite(amount) && amount >= 0) {
              const capped = typeof availableGold === "number" ? Math.min(amount, availableGold) : amount;
              onSetGold?.(capped);
              setGoldDraft("");
            }
          }}
        >
          <input
            style={{ ...style.goldInput, ...(goldLocked ? style.goldInputDisabled : null) }}
            value={goldDraft}
            disabled={goldLocked}
            inputMode="numeric"
            onChange={(event) => setGoldDraft(event.target.value.replace(/[^0-9]/g, ""))}
            placeholder={t("ui.tradeGoldPlaceholder", [], "Offer gold")}
            aria-label={t("ui.tradeSetGold", [], "Set gold")}
            autoComplete="off"
            spellCheck={false}
            maxLength={12}
          />
          <button
            type="submit"
            disabled={goldLocked || goldDraft.length === 0}
            style={{ ...style.goldButton, ...(goldLocked || goldDraft.length === 0 ? style.actionButtonDisabled : null) }}
          >
            {t("ui.tradeSetGold", [], "Set")}
          </button>
        </form>
      </TradeSide>
      <TradeSide
        top={250}
        title={partner ? t("ui.tradePartnerOffer", [partner], `${partner}'s Offer`) : t("ui.tradePartner", [], "Partner")}
        gold={partnerGoldValue}
        items={partnerItemsList}
        itemCount={partnerItemsCount}
        confirmed={partnerConfirmed}
        accent="#9c8d6f"
        total={partnerTotal}
        t={t}
      />

      <div style={style.status}>
        {bothConfirmed
          ? t("ui.tradeBothConfirmed", [], "Both sides confirmed. Completing trade...")
          : selfConfirmed
            ? t("ui.tradeWaitingPartner", [], "Waiting for partner to confirm.")
            : partnerConfirmed
              ? t("ui.tradePartnerConfirmed", [], "Partner confirmed. Review and confirm to finish.")
              : t("ui.tradeReviewHint", [], "Place your offer, then confirm.")}
      </div>

      <div style={style.actions}>
        {requested ? (
          <button
            type="button"
            disabled={!onAccept}
            style={{ ...style.actionButton, ...(!onAccept ? style.actionButtonDisabled : null) }}
            onClick={() => onAccept?.()}
          >
            {t("ui.tradeAccept", [], "Accept")}
          </button>
        ) : (
          <button
            type="button"
            disabled={!onConfirm || !partner}
            aria-pressed={selfConfirmed}
            style={{
              ...style.actionButton,
              ...style.actionButtonPrimary,
              ...(selfConfirmed ? style.actionButtonLocked : null),
              ...(!onConfirm || !partner ? style.actionButtonDisabled : null),
            }}
            onClick={() => onConfirm?.()}
          >
            {selfConfirmed ? t("ui.tradeUnlock", [], "Unlock") : t("ui.tradeConfirm", [], "Confirm")}
          </button>
        )}
        <button
          type="button"
          disabled={!onCancel || !partner}
          style={{ ...style.actionButton, ...(!onCancel || !partner ? style.actionButtonDisabled : null) }}
          onClick={() => onCancel?.()}
        >
          {requested ? t("ui.tradeDecline", [], "Decline") : t("ui.cancel", [], "Cancel")}
        </button>
      </div>
    </section>
  );
}

function TradeSide({
  top,
  title,
  gold,
  items,
  itemCount,
  confirmed,
  accent,
  total,
  t,
  children,
}: {
  top: number;
  title: string;
  gold: number;
  items: TradeItemSlot[];
  itemCount: number;
  confirmed: boolean;
  accent: string;
  total: number;
  t: TranslateFn;
  children?: ReactNode;
}) {
  return (
    <div style={{ ...style.side, top }}>
      <div style={style.sideHead}>
        <span style={{ ...style.sideTitle, color: accent }}>{title}</span>
        <span style={{ ...style.sideBadge, ...(confirmed ? style.sideBadgeOn : null) }}>
          {confirmed ? t("ui.tradeLocked", [], "Locked") : t("ui.tradeOpenState", [], "Open")}
        </span>
      </div>
      <ItemGrid items={items} t={t} />
      <div style={style.sideRow}>
        <span style={style.sideLabel}>{t("ui.gold", [], "Gold")}</span>
        <span style={style.sideValue}>{formatNumber(gold)}</span>
      </div>
      {children}
      <div style={style.sideFooter}>
        <span style={style.sideMeta}>{t("ui.tradeItemsCount", [itemCount], `${itemCount} items`)}</span>
        {total > 0 ? (
          <span style={style.sideTotal}>{t("ui.tradeTotalValue", [formatNumber(total)], `Total ≈ ${formatNumber(total)}`)}</span>
        ) : null}
      </div>
    </div>
  );
}

function ItemGrid({ items, t }: { items: TradeItemSlot[]; t: TranslateFn }) {
  return (
    <div style={style.grid} role="list" aria-label={t("ui.tradeItems", [], "Items")}>
      {Array.from({ length: TRADE_SLOT_COUNT }).map((_, index) => {
        const item = items[index];
        return (
          <div
            key={item ? `item-${item.id}` : `empty-${index}`}
            role="listitem"
            data-trade-slot={index}
            title={item?.name}
            style={style.cell}
          >
            {item && typeof item.icon === "number" ? (
              <img style={style.cellIcon} src={iconPath(item.icon)} alt="" draggable={false} />
            ) : item ? (
              <span style={style.cellText}>{(item.name ?? "?").slice(0, 3)}</span>
            ) : null}
            {item && item.count && item.count > 1 ? <span style={style.cellCount}>{item.count}</span> : null}
          </div>
        );
      })}
    </div>
  );
}

function sumValue(items: TradeItemSlot[]): number {
  return items.reduce((sum, item) => sum + (typeof item.value === "number" ? item.value : 0), 0);
}

function iconPath(icon: number) {
  return `/original-ui/Items/${icon}.png`;
}

function formatNumber(value: number) {
  return Math.max(0, Math.trunc(value)).toLocaleString("en-US");
}

const style: Record<string, CSSProperties> = {
  window: {
    position: "absolute",
    left: 356,
    top: 120,
    width: FRAME.width,
    height: FRAME.height,
    zIndex: 36,
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
  banner: {
    position: "absolute",
    left: 12,
    top: 36,
    width: 288,
    padding: "6px 8px",
    border: "1px solid rgba(190, 157, 99, 0.32)",
    background: "rgba(11, 8, 5, 0.5)",
    fontSize: 11,
    color: "#e3d3af",
    lineHeight: 1.3,
  },
  side: {
    position: "absolute",
    left: 12,
    width: 288,
    border: "1px solid rgba(190, 157, 99, 0.32)",
    background: "linear-gradient(180deg, rgba(27, 19, 10, 0.78), rgba(11, 8, 5, 0.7))",
    padding: "6px 8px",
  },
  sideHead: {
    display: "flex",
    justifyContent: "space-between",
    alignItems: "center",
    borderBottom: "1px solid rgba(190, 157, 99, 0.24)",
    paddingBottom: 4,
    marginBottom: 5,
  },
  sideTitle: { fontSize: 12, fontWeight: 700 },
  sideBadge: {
    fontSize: 9,
    padding: "1px 6px",
    border: "1px solid rgba(190, 157, 99, 0.4)",
    borderRadius: 2,
    color: "#cbb38a",
  },
  sideBadgeOn: { color: "#8be07a", borderColor: "rgba(139, 224, 122, 0.6)" },
  grid: {
    display: "grid",
    gridTemplateColumns: "repeat(5, 1fr)",
    gap: 3,
    marginBottom: 5,
  },
  cell: {
    position: "relative",
    height: 24,
    border: "1px solid rgba(190, 157, 99, 0.3)",
    background: "rgba(0, 0, 0, 0.4)",
    display: "flex",
    alignItems: "center",
    justifyContent: "center",
    overflow: "hidden",
  },
  cellIcon: { width: 20, height: 20, imageRendering: "pixelated" },
  cellText: { fontSize: 9, color: "#cbb38a" },
  cellCount: {
    position: "absolute",
    right: 1,
    bottom: 0,
    fontSize: 8,
    lineHeight: "9px",
    padding: "0 1px",
    color: "#f8e6bb",
    background: "rgba(0, 0, 0, 0.6)",
  },
  sideRow: { display: "flex", justifyContent: "space-between", padding: "2px 0", fontSize: 11 },
  sideLabel: { color: "#a89568" },
  sideValue: { color: "#f4dcaf", fontWeight: 700 },
  goldEditRow: { display: "flex", gap: 4, marginTop: 2 },
  goldInput: {
    flex: 1,
    border: "1px solid rgba(190, 157, 99, 0.5)",
    background: "rgba(11, 8, 5, 0.7)",
    color: "#f0eee8",
    padding: "2px 6px",
    fontSize: 10,
    fontFamily: "inherit",
  },
  goldInputDisabled: { opacity: 0.45 },
  goldButton: {
    flex: "0 0 auto",
    border: "1px solid rgba(190, 157, 99, 0.5)",
    background: "rgba(52, 32, 18, 0.86)",
    color: "#f4dcaf",
    padding: "2px 10px",
    fontSize: 10,
    cursor: "pointer",
  },
  sideFooter: {
    display: "flex",
    justifyContent: "space-between",
    alignItems: "baseline",
    marginTop: 4,
    paddingTop: 3,
    borderTop: "1px solid rgba(190, 157, 99, 0.18)",
  },
  sideMeta: { fontSize: 10, color: "#a89568" },
  sideTotal: { fontSize: 10, color: "#f0d69b", fontWeight: 700 },
  status: {
    position: "absolute",
    left: 12,
    top: 382,
    width: 288,
    minHeight: 24,
    fontSize: 11,
    color: "#d6c6a5",
    lineHeight: 1.3,
  },
  actions: { position: "absolute", left: 12, top: 412, width: 288, display: "flex", gap: 6 },
  actionButton: {
    flex: 1,
    border: "1px solid rgba(190, 157, 99, 0.56)",
    background: "linear-gradient(180deg, rgba(95, 53, 24, 0.95), rgba(45, 23, 12, 0.95))",
    color: "#f4dcaf",
    padding: "5px 0",
    fontSize: 11,
    cursor: "pointer",
  },
  actionButtonPrimary: {
    background: "linear-gradient(180deg, rgba(120, 74, 34, 0.96), rgba(70, 40, 20, 0.96))",
    borderColor: "rgba(214, 180, 110, 0.85)",
  },
  actionButtonLocked: {
    background: "linear-gradient(180deg, rgba(70, 96, 52, 0.95), rgba(36, 52, 26, 0.95))",
    borderColor: "rgba(139, 224, 122, 0.7)",
    color: "#dcf2cf",
  },
  actionButtonDisabled: { opacity: 0.45, cursor: "default" },
};
