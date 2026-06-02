"use client";

import { type CSSProperties } from "react";

import { ORIGINAL_UI } from "../../lib/original-ui";
import { SpriteButton } from "./original-client-overlays";

type TranslateFn = (
  key: string,
  params?: Array<string | number>,
  fallback?: string,
) => string;

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
  /** Accept an incoming `requested` trade invite. */
  onAccept?: () => void;
  /** Lock in the viewer's side of an `open` trade. */
  onConfirm?: () => void;
  /** Cancel / decline the trade entirely. */
  onCancel?: () => void;
  onClose: () => void;
};

const FRAME = ORIGINAL_UI.mail;

export function TradeWindow({
  t,
  trade,
  myGold,
  myItemCount,
  myConfirmed,
  onAccept,
  onConfirm,
  onCancel,
  onClose,
}: TradeWindowProps) {
  const partner = trade?.partner?.trim() ?? "";
  const state = trade?.state ?? (partner ? "open" : "idle");
  const requested = state === "requested";
  const partnerConfirmed = trade?.confirmed === true;
  const selfConfirmed = myConfirmed === true;
  const bothConfirmed = partnerConfirmed && selfConfirmed;

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
        top={92}
        title={t("ui.tradeYourOffer", [], "Your Offer")}
        gold={myGold ?? 0}
        items={myItemCount ?? 0}
        confirmed={selfConfirmed}
        accent="#caa64a"
        t={t}
      />
      <TradeSide
        top={218}
        title={partner ? t("ui.tradePartnerOffer", [partner], `${partner}'s Offer`) : t("ui.tradePartner", [], "Partner")}
        gold={trade?.partnerGold ?? 0}
        items={trade?.partnerItemCount ?? 0}
        confirmed={partnerConfirmed}
        accent="#9c8d6f"
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
            disabled={!onConfirm || !partner || selfConfirmed}
            style={{
              ...style.actionButton,
              ...style.actionButtonPrimary,
              ...(!onConfirm || !partner || selfConfirmed ? style.actionButtonDisabled : null),
            }}
            onClick={() => onConfirm?.()}
          >
            {selfConfirmed ? t("ui.tradeConfirmed", [], "Confirmed") : t("ui.tradeConfirm", [], "Confirm")}
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
  confirmed,
  accent,
  t,
}: {
  top: number;
  title: string;
  gold: number;
  items: number;
  confirmed: boolean;
  accent: string;
  t: TranslateFn;
}) {
  return (
    <div style={{ ...style.side, top }}>
      <div style={style.sideHead}>
        <span style={{ ...style.sideTitle, color: accent }}>{title}</span>
        <span style={{ ...style.sideBadge, ...(confirmed ? style.sideBadgeOn : null) }}>
          {confirmed ? t("ui.tradeLocked", [], "Locked") : t("ui.tradeOpenState", [], "Open")}
        </span>
      </div>
      <div style={style.sideRow}>
        <span style={style.sideLabel}>{t("ui.gold", [], "Gold")}</span>
        <span style={style.sideValue}>{formatNumber(gold)}</span>
      </div>
      <div style={style.sideRow}>
        <span style={style.sideLabel}>{t("ui.tradeItems", [], "Items")}</span>
        <span style={style.sideValue}>{items}</span>
      </div>
    </div>
  );
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
  sideRow: { display: "flex", justifyContent: "space-between", padding: "2px 0", fontSize: 11 },
  sideLabel: { color: "#a89568" },
  sideValue: { color: "#f4dcaf", fontWeight: 700 },
  status: {
    position: "absolute",
    left: 12,
    top: 348,
    width: 288,
    minHeight: 30,
    fontSize: 11,
    color: "#d6c6a5",
    lineHeight: 1.3,
  },
  actions: { position: "absolute", left: 12, top: 408, width: 288, display: "flex", gap: 6 },
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
  actionButtonDisabled: { opacity: 0.45, cursor: "default" },
};
