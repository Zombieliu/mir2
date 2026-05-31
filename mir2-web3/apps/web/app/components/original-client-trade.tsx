"use client";

import { useState } from "react";

import { useItemDrag } from "./original-client-drag-item";
import type { DisplayTradeState, TranslateFn } from "./original-client-types";

export type TradeDialogProps = {
  t: TranslateFn;
  trade: DisplayTradeState;
  onSetGold: (amount: number) => void;
  onSetLocked: (locked: boolean) => void;
  onCancel: () => void;
};

// Crystal player-to-player trade: two item grids (mine = drag-driven deposit/
// retrieve, theirs = read-only), independent gold, and a lock toggle. Both
// sides locking completes the trade server-side.
export function TradeDialog({ t, trade, onSetGold, onSetLocked, onCancel }: TradeDialogProps) {
  const itemDrag = useItemDrag();
  const [goldInput, setGoldInput] = useState(String(trade.myGold));

  const commitGold = () => {
    const amount = Number.parseInt(goldInput || "0", 10);
    onSetGold(Number.isFinite(amount) ? amount : 0);
  };

  return (
    <section className="trade-dialog" role="dialog" aria-label={t("ui.trade", [], "Trade")}>
      <div className="trade-dialog-head">
        <span className="trade-dialog-title">
          {t("ui.trade", [], "Trade")} — {trade.partnerName}
        </span>
        <button type="button" className="trade-dialog-close" aria-label={t("ui.cancel", [], "Cancel")} onClick={onCancel}>
          ×
        </button>
      </div>

      <div className="trade-grids">
        <div className="trade-side mine">
          <div className={`trade-side-label ${trade.myLocked ? "locked" : ""}`}>
            {t("ui.yourOffer", [], "Your Offer")}
            {trade.myLocked ? ` · ${t("ui.locked", [], "Locked")}` : ""}
          </div>
          <div className="trade-grid">
            {trade.myItems.map((item, slot) => (
              <div
                key={`my-${slot}`}
                className="trade-slot"
                data-item-drop-kind="trade"
                data-item-drop-slot={slot}
              >
                {item ? (
                  <button
                    type="button"
                    className="trade-item"
                    aria-label={item.name}
                    title={item.name}
                    onPointerDown={(event) =>
                      itemDrag?.beginDrag(event, {
                        uniqueId: -1,
                        key: `trade:${slot}`,
                        name: item.name,
                        icon: item.icon,
                        quantity: item.count,
                        source: { kind: "trade", slot },
                      })
                    }
                  >
                    <img src={`/original-ui/Items/${item.icon}.png`} alt="" draggable={false} />
                    {item.count > 1 ? <span className="trade-item-count">{item.count}</span> : null}
                  </button>
                ) : null}
              </div>
            ))}
          </div>
        </div>

        <div className="trade-side theirs">
          <div className={`trade-side-label ${trade.theirLocked ? "locked" : ""}`}>
            {trade.partnerName}
            {trade.theirLocked ? ` · ${t("ui.locked", [], "Locked")}` : ""}
          </div>
          <div className="trade-grid">
            {trade.theirItems.map((item, slot) => (
              <div key={`their-${slot}`} className="trade-slot readonly">
                {item ? (
                  <div className="trade-item" title={item.name}>
                    <img src={`/original-ui/Items/${item.icon}.png`} alt="" draggable={false} />
                    {item.count > 1 ? <span className="trade-item-count">{item.count}</span> : null}
                  </div>
                ) : null}
              </div>
            ))}
          </div>
        </div>
      </div>

      <div className="trade-gold-row">
        <label className="trade-gold mine">
          <span>{t("ui.gold", [], "Gold")}</span>
          <input
            type="number"
            min={0}
            value={goldInput}
            disabled={trade.myLocked}
            onChange={(event) => setGoldInput(event.target.value)}
            onBlur={commitGold}
            onKeyDown={(event) => {
              if (event.key === "Enter") commitGold();
            }}
          />
        </label>
        <div className="trade-gold theirs">
          <span>{t("ui.gold", [], "Gold")}</span>
          <b>{trade.theirGold}</b>
        </div>
      </div>

      <div className="trade-actions">
        <button
          type="button"
          className={`trade-lock ${trade.myLocked ? "active" : ""}`}
          onClick={() => onSetLocked(!trade.myLocked)}
        >
          {trade.myLocked ? t("ui.unlock", [], "Unlock") : t("ui.lock", [], "Lock")}
        </button>
        <button type="button" className="trade-cancel" onClick={onCancel}>
          {t("ui.cancel", [], "Cancel")}
        </button>
      </div>
    </section>
  );
}

export type TradeRequestPromptProps = {
  t: TranslateFn;
  fromName: string;
  onReply: (accept: boolean) => void;
};

export function TradeRequestPrompt({ t, fromName, onReply }: TradeRequestPromptProps) {
  return (
    <section className="trade-request-prompt" role="dialog" aria-label={t("ui.tradeRequest", [], "Trade Request")}>
      <div className="trade-request-text">
        {t("ui.tradeRequestFrom", [fromName], `${fromName} wants to trade.`)}
      </div>
      <div className="trade-request-actions">
        <button type="button" onClick={() => onReply(true)}>
          {t("ui.accept", [], "Accept")}
        </button>
        <button type="button" onClick={() => onReply(false)}>
          {t("ui.decline", [], "Decline")}
        </button>
      </div>
    </section>
  );
}
