"use client";

import { useState } from "react";

import type { DisplayNpcShop, TranslateFn } from "./original-client-types";

export type NpcShopDialogProps = {
  t: TranslateFn;
  shop: DisplayNpcShop;
  gold: number;
  onBuy: (itemIndex: number, count: number, panelType: number) => void;
  onClose: () => void;
};

// Crystal NPCGoodsDialog (buy) / NPCDropDialog (sell) / repair. Buy renders the
// NPC's goods grid with resolved icons + prices and a quantity buy; sell/repair
// show a banner because those reuse the existing inventory Sell and character
// Repair flows.
export function NpcShopDialog({ t, shop, gold, onBuy, onClose }: NpcShopDialogProps) {
  const [selectedIndex, setSelectedIndex] = useState(0);
  const [quantity, setQuantity] = useState(1);

  if (shop.mode !== "buy") {
    return (
      <section className="npc-shop-dialog banner" role="dialog" aria-label={shop.mode}>
        <div className="npc-shop-head">
          <span className="npc-shop-title">
            {shop.mode === "sell" ? t("ui.sellItem", [], "Sell") : t("ui.repairItem", [], "Repair")}
          </span>
          <button type="button" className="npc-shop-close" aria-label={t("ui.close")} onClick={onClose}>
            ×
          </button>
        </div>
        {shop.mode === "sell" ? (
          <div className="npc-shop-sell-zone" data-item-drop-kind="npcSell">
            {t("ui.npcSellDrop", [], "Drag items here to sell.")}
          </div>
        ) : (
          <div className="npc-shop-banner-text">
            {t("ui.npcRepairHint", [], "Repair mode active — use Repair on equipped items.")}
          </div>
        )}
      </section>
    );
  }

  const selected = shop.goods[selectedIndex] ?? null;
  const total = selected ? selected.price * Math.max(1, quantity) : 0;
  const affordable = selected !== null && total <= gold;

  return (
    <section className="npc-shop-dialog" role="dialog" aria-label={t("ui.buy", [], "Buy")}>
      <div className="npc-shop-head">
        <span className="npc-shop-title">{t("ui.buy", [], "Buy")}</span>
        <button type="button" className="npc-shop-close" aria-label={t("ui.close")} onClick={onClose}>
          ×
        </button>
      </div>

      <div className="npc-shop-grid">
        {shop.goods.length === 0 ? (
          <div className="npc-shop-empty">{t("ui.noGoods", [], "No goods available.")}</div>
        ) : (
          shop.goods.map((good, index) => (
            <button
              key={`${good.itemIndex}-${index}`}
              type="button"
              className={`npc-shop-good ${index === selectedIndex ? "selected" : ""}`}
              title={good.name}
              data-npc-good-index={good.itemIndex}
              onClick={() => {
                setSelectedIndex(index);
                setQuantity(1);
              }}
            >
              <img className="npc-shop-good-icon" src={`/original-ui/Items/${good.icon}.png`} alt="" draggable={false} />
              <span className="npc-shop-good-name">{good.name}</span>
              <span className="npc-shop-good-price">{good.price}</span>
            </button>
          ))
        )}
      </div>

      <div className="npc-shop-foot">
        <div className="npc-shop-gold">
          {t("ui.gold", [], "Gold")}: {gold}
        </div>
        <div className="npc-shop-buy-row">
          <input
            type="number"
            min={1}
            max={99}
            value={quantity}
            disabled={!selected}
            onChange={(event) => {
              const next = Number.parseInt(event.target.value || "1", 10);
              setQuantity(Math.max(1, Math.min(99, Number.isFinite(next) ? next : 1)));
            }}
          />
          <button
            type="button"
            className="npc-shop-buy"
            disabled={!affordable}
            onClick={() => {
              if (selected) onBuy(selected.itemIndex, quantity, shop.panelType);
            }}
          >
            {t("ui.buy", [], "Buy")} ({total})
          </button>
        </div>
      </div>
    </section>
  );
}
