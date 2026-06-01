"use client";

import { useState } from "react";

import { ORIGINAL_UI } from "../../lib/original-ui";
import { useItemDrag } from "./original-client-drag-item";
import { SpriteButton } from "./original-client-overlays";
import type { DisplayTradeItem, DisplayTradeState, TranslateFn } from "./original-client-types";

export type TradeDialogProps = {
  t: TranslateFn;
  trade: DisplayTradeState;
  onSetGold: (amount: number) => void;
  onSetLocked: (locked: boolean) => void;
  onCancel: () => void;
};

const TRADE = ORIGINAL_UI.trade;

function tradeCellPosition(slot: number) {
  const x = slot % 5;
  const y = Math.floor(slot / 5);
  // Crystal: Location = new Point(x * 36 + 10 + x, y * 32 + 39 + y).
  return { left: x * 36 + TRADE.cellOriginX + x, top: y * 32 + TRADE.cellOriginY + y };
}

// Crystal player-to-player trade: two 204x152 Prguse/389 frames side by side.
// My side (left) is drag-driven deposit/retrieve; their side (right) is
// read-only. Each side has its own gold field and a lock; both locking
// completes the trade server-side.
export function TradeDialog({ t, trade, onSetGold, onSetLocked, onCancel }: TradeDialogProps) {
  const itemDrag = useItemDrag();
  const [goldInput, setGoldInput] = useState(String(trade.myGold));

  const commitGold = () => {
    const amount = Number.parseInt(goldInput || "0", 10);
    onSetGold(Number.isFinite(amount) ? amount : 0);
  };

  const renderItem = (item: DisplayTradeItem) => (
    <img src={`/original-ui/Items/${item.icon}.png`} alt="" draggable={false} />
  );

  return (
    <section className="trade-dialog" role="dialog" aria-label={t("ui.trade", [], "Trade")}>
      {/* My side */}
      <div className="trade-frame-side mine" style={{ width: TRADE.width, height: TRADE.height }}>
        <img className="trade-frame-bg" src={TRADE.frame} alt="" draggable={false} />
        <div className="trade-frame-title">{t("ui.yourOffer", [], "Your Offer")}</div>
        <div className="trade-frame-close">
          <SpriteButton sprite={TRADE.closeButton} label={t("ui.cancel", [], "Cancel")} onClick={onCancel} />
        </div>
        {trade.myItems.map((item, slot) => {
          const pos = tradeCellPosition(slot);
          return (
            <div
              key={`my-${slot}`}
              className="trade-cell"
              data-item-drop-kind="trade"
              data-item-drop-slot={slot}
              style={{ left: pos.left, top: pos.top, width: TRADE.cellSize, height: TRADE.cellSize }}
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
                  {renderItem(item)}
                  {item.count > 1 ? <span className="trade-item-count">{item.count}</span> : null}
                </button>
              ) : null}
            </div>
          );
        })}
        <input
          className="trade-frame-gold"
          type="number"
          min={0}
          value={goldInput}
          disabled={trade.myLocked}
          aria-label={t("ui.gold", [], "Gold")}
          onChange={(event) => setGoldInput(event.target.value)}
          onBlur={commitGold}
          onKeyDown={(event) => {
            if (event.key === "Enter") commitGold();
          }}
        />
        <div className="trade-frame-confirm">
          <SpriteButton
            sprite={TRADE.confirmButton}
            label={trade.myLocked ? t("ui.unlock", [], "Unlock") : t("ui.lock", [], "Lock")}
            onClick={() => onSetLocked(!trade.myLocked)}
            active={trade.myLocked}
          />
        </div>
      </div>

      {/* Their side */}
      <div className="trade-frame-side theirs" style={{ width: TRADE.width, height: TRADE.height }}>
        <img className="trade-frame-bg" src={TRADE.frame} alt="" draggable={false} />
        <div className={`trade-frame-title ${trade.theirLocked ? "locked" : ""}`}>
          {trade.partnerName}
          {trade.theirLocked ? ` · ${t("ui.locked", [], "Locked")}` : ""}
        </div>
        {trade.theirItems.map((item, slot) => {
          const pos = tradeCellPosition(slot);
          return (
            <div
              key={`their-${slot}`}
              className="trade-cell readonly"
              style={{ left: pos.left, top: pos.top, width: TRADE.cellSize, height: TRADE.cellSize }}
            >
              {item ? (
                <div className="trade-item" title={item.name}>
                  {renderItem(item)}
                  {item.count > 1 ? <span className="trade-item-count">{item.count}</span> : null}
                </div>
              ) : null}
            </div>
          );
        })}
        <div className="trade-frame-gold readonly">{trade.theirGold}</div>
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
