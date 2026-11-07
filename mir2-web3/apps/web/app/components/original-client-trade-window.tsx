"use client";

import { useEffect, useRef, useState, type CSSProperties } from "react";

import { r16TradeCell, r16TradeGoldAmount, R16_SOCIAL_LAYOUT } from "../../lib/r16-social-layout";
import { currentSocialItem, sameSocialItem, validSocialItem, validSocialItemSlot, type SocialItemSlot } from "../../lib/social-item-window-model";
import { originalItemIconPath } from "./original-client-inventory-utils";
import { R16SocialButton, R16SocialModal, R16_SOCIAL_ART_STYLE, useR16SocialWindow } from "./r16-social-controls";
import { OriginalItemTooltip } from "./original-client-item-tooltip";
import { OriginalCrystalItemTooltip } from "./original-client-crystal-item-tooltip";
import type { CrystalTooltipDocument } from "../../lib/shared-item-tooltip";

type TranslateFn = (
  key: string,
  params?: Array<string | number>,
  fallback?: string,
) => string;

/** A single item placed into one side of a trade (one of the 5x2 grid cells). */
export type TradeItemSlot = {
  /** Stable id for React keys (item unique id / slot index). */
  id: string | number;
  /** Actual trade-array position. Display IDs never supply this identity. */
  slot?: number;
  uniqueId?: number;
  /** Authentic received item/template metadata; presentation never supplies an action identity. */
  tooltipSource?: Readonly<Record<string, unknown>>;
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
  partnerItems?: Array<TradeItemSlot | null>;
  partnerItemsKnown?: boolean;
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
  playerName?: string | null;
  trade: TradeSummary | null;
  /** Gold the viewer has put up for the trade. */
  myGold?: number;
  /** Number of items the viewer has placed into the trade. */
  myItemCount?: number;
  /** Whether the viewer has already pressed confirm. */
  myConfirmed?: boolean;
  /** Optional viewer item slots, mirroring Crystal's Trade 5x2 grid. */
  myItems?: Array<TradeItemSlot | null>;
  onReadItemTooltip?: (item: TradeItemSlot) => CrystalTooltipDocument | null;
  inventoryItems?: readonly SocialItemSlot[];
  /** Only current, genuinely empty physical bag cells supplied by the host. */
  emptyInventorySlots?: readonly number[];
  itemsReady?: boolean;
  itemActionPending?: boolean;
  /** Host-owned owner/session/source lease; changes retire the selection. */
  itemSourceKey?: string;
  onDepositItem?: (from: number, to: number, uniqueId: number) => void;
  onRetrieveItem?: (from: number, to: number, uniqueId: number) => void;
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

const layout = R16_SOCIAL_LAYOUT.trade;
const invitation = R16_SOCIAL_LAYOUT.invitation;
const TRADE_SLOT_COUNT = 10;

export function TradeWindow({ t, trade, playerName, myGold, myConfirmed, myItems, onReadItemTooltip,
  inventoryItems, emptyInventorySlots, itemsReady, itemActionPending, itemSourceKey,
  onDepositItem, onRetrieveItem, availableGold, onAccept, onConfirm, onCancel, onSetGold, onClose }: TradeWindowProps) {
  const partner = trade?.partner?.trim() ?? "";
  const state = trade?.state ?? (partner ? "open" : "idle");
  const requested = state === "requested";
  const partnerConfirmed = trade?.confirmed === true;
  const selfConfirmed = myConfirmed === true;
  const ownWindow = useR16SocialWindow(layout.own);
  const guestWindow = useR16SocialWindow(layout.guest);
  const [front, setFront] = useState<"own" | "guest">("guest");
  const [goldDraft, setGoldDraft] = useState<string | null>(null);
  const goldScope = useRef<{ partner: string; sourceKey: string | undefined } | null>(null);
  const [itemSelection, setItemSelection] = useState<{ source: "bag" | "offer"; item: SocialItemSlot; key: string } | null>(null);
  const myItemsList = myItems ?? [];
  const ownSlots = tradeGridSlots(myItemsList);
  const offerItems = ownSlots.flatMap((item, slot) => {
    const candidate = item && { slot, uniqueId: item.uniqueId, name: item.name, icon: item.icon, count: item.count };
    return candidate && validSocialItem(candidate, TRADE_SLOT_COUNT) ? [candidate] : [];
  });
  const bagItems = inventoryItems ?? [];
  const emptySlots = emptyInventorySlots ?? [];
  const itemEditing = itemsReady === true && itemActionPending !== true && state === "open" && Boolean(partner)
    && !selfConfirmed && !partnerConfirmed && Boolean(itemSourceKey)
    && inventoryItems !== undefined && emptyInventorySlots !== undefined
    && myItems?.length === TRADE_SLOT_COUNT
    && Array.from(myItems).every((item, slot) => item === null || validSocialItem(item, TRADE_SLOT_COUNT) && item.slot === slot)
    && ownSlots.filter(Boolean).length === offerItems.length
    && new Set(offerItems.map(item => item.uniqueId)).size === offerItems.length
    && bagItems.every(item => validSocialItem(item, 256)) && new Set(bagItems.map(item => item.slot)).size === bagItems.length
    && new Set(bagItems.map(item => item.uniqueId)).size === bagItems.length
    && emptySlots.every(slot => validSocialItemSlot(slot, 256) && !bagItems.some(item => item.slot === slot))
    && new Set(emptySlots).size === emptySlots.length;
  const selectableBag = bagItems.filter(item => !offerItems.some(offer => offer.uniqueId === item.uniqueId));
  const selectedItem = itemSelection;
  const selection = itemEditing && selectedItem !== null && selectedItem.key === itemSourceKey
    && currentSocialItem(selectedItem.source === "bag" ? selectableBag : offerItems, selectedItem.item)
    ? selectedItem : null;
  useEffect(() => { setItemSelection(null); setGoldDraft(null); goldScope.current = null; }, [partner, itemSourceKey, state]);
  useEffect(() => { setItemSelection(null); }, [itemActionPending, itemsReady]);
  useEffect(() => { if (itemSelection && !selection) setItemSelection(null); }, [itemSelection, selection]);
  useEffect(() => {
    const clear = () => setItemSelection(null);
    window.addEventListener("blur", clear);
    return () => window.removeEventListener("blur", clear);
  }, []);
  const placeInOffer = (slot: number) => {
    if (!itemEditing || !validSocialItemSlot(slot, TRADE_SLOT_COUNT)) return;
    const item = offerItems.find(row => row.slot === slot);
    if (item) { setItemSelection({ source: "offer", item, key: itemSourceKey! }); return; }
    if (selection?.source !== "bag" || !onDepositItem || ownSlots[slot] !== null) return;
    onDepositItem(selection.item.slot, slot, selection.item.uniqueId);
    setItemSelection(null);
  };
  // The same host-owned lock, pending and wallet gates remain authoritative.
  const goldLocked = selfConfirmed || itemActionPending === true || !onSetGold || !partner || state !== "open";
  const confirmDisabled = !onConfirm || !partner || itemActionPending === true || !selfConfirmed && trade?.partnerItemsKnown === false;
  const amount = goldDraft === null ? null : r16TradeGoldAmount(goldDraft, availableGold);
  const openGold = () => {
    if (goldLocked) return;
    goldScope.current = { partner, sourceKey: itemSourceKey };
    setGoldDraft(String(typeof availableGold === "number" ? Math.max(0, Math.trunc(availableGold)) : 0));
  };
  const closeGold = () => { setGoldDraft(null); goldScope.current = null; };
  const submitGold = () => {
    if (goldLocked || amount === null || goldScope.current?.partner !== partner || goldScope.current.sourceKey !== itemSourceKey) return;
    if (amount > 0) onSetGold?.(amount);
    closeGold();
  };
  const status = trade?.partnerItemsKnown === false
    ? t("ui.tradeItemDetailsUnavailable", [], "Partner item details are unavailable. Wait for an authoritative update.")
    : itemActionPending ? t("ui.itemActionPending", [], "Waiting for item update...")
    : selfConfirmed && partnerConfirmed ? t("ui.tradeBothConfirmed", [], "Both sides confirmed. Completing trade...")
    : selfConfirmed ? t("ui.tradeWaitingPartner", [], "Waiting for partner to confirm.")
    : partnerConfirmed ? t("ui.tradePartnerConfirmed", [], "Partner confirmed. Review and confirm to finish.") : "";

  return <section aria-label={t("ui.trade", [], "Trade")} data-trade-state={state} data-trade-partner={partner} data-ui-interactive="true"
    style={{ position: "absolute", inset: 0, zIndex: 36, pointerEvents: "none" }}>
    {state === "open" ? <>
      <section data-r16-trade-side="own" aria-label={t("ui.tradeYourOffer", [], "Your Offer")}
        onPointerDown={event => { setFront("own"); ownWindow.pointerDown(event); }}
        onPointerMove={ownWindow.pointerMove} onPointerUp={ownWindow.pointerUp} onLostPointerCapture={ownWindow.pointerUp}
        style={{ ...style.window, ...layout.own, ...ownWindow.position, zIndex: front === "own" ? 2 : 1 }}>
        <img src="/original-ui/Prguse/389.png" alt="" draggable={false} style={R16_SOCIAL_ART_STYLE} />
        <div style={{ ...style.name, ...layout.ownName }}>{playerName || t("ui.tradeYourOffer", [], "Your Offer")}</div>
        <TradeGrid items={myItemsList} t={t} disabled={!itemEditing} selectedSlot={selection?.source === "offer" ? selection.item.slot : undefined}
          onItemClick={placeInOffer} onReadItemTooltip={onReadItemTooltip} />
        <button type="button" data-r16-trade-gold="true" aria-label={t("ui.tradeSetGold", [], "Set gold")}
          disabled={goldLocked} onClick={openGold} style={{ ...style.gold, ...layout.gold, cursor: goldLocked ? "default" : "pointer" }}>
          {formatNumber(myGold ?? 0)}
        </button>
        <R16SocialButton frame={520} rect={layout.confirm} data-r16-trade-confirm="true" locked={selfConfirmed}
          aria-pressed={selfConfirmed} disabled={confirmDisabled}
          label={selfConfirmed ? t("ui.tradeUnlock", [], "Unlock") : t("ui.tradeConfirm", [], "Confirm")} onClick={() => onConfirm?.()} />
        <R16SocialButton library="Prguse2" frame={360} rect={layout.close} data-r16-trade-cancel="true"
          disabled={!onCancel || !partner} label={t("ui.cancel", [], "Cancel")} onClick={() => onCancel?.()} />
        <div aria-live="polite" title={status} style={style.status}>{status}</div>
      </section>
      <section data-r16-trade-side="guest" aria-label={t("ui.tradePartner", [], "Partner")}
        onPointerDown={event => { setFront("guest"); guestWindow.pointerDown(event); }}
        onPointerMove={guestWindow.pointerMove} onPointerUp={guestWindow.pointerUp} onLostPointerCapture={guestWindow.pointerUp}
        style={{ ...style.window, ...layout.guest, ...guestWindow.position, zIndex: front === "guest" ? 2 : 1 }}>
        <img src="/original-ui/Prguse/390.png" alt="" draggable={false} style={R16_SOCIAL_ART_STYLE} />
        <div style={{ ...style.name, ...layout.guestName, textAlign: "center" }}>{partner}</div>
        <TradeGrid items={trade?.partnerItems ?? []} t={t} onReadItemTooltip={onReadItemTooltip} />
        <div style={{ ...style.gold, ...layout.gold }}>{formatNumber(trade?.partnerGold ?? 0)}</div>
      </section>
      {inventoryItems && emptyInventorySlots ? <section data-r16-trade-bag-picker="true"
        aria-label={t("ui.tradeBagItems", [], "Trade bag items")} style={style.itemPicker}>
        <div style={style.pickerHint} aria-live="polite">{itemActionPending
          ? t("ui.itemActionPending", [], "Waiting for item update...")
          : !itemEditing ? t("ui.itemDataUnavailable", [], "Item actions unavailable")
          : selection?.source === "offer" ? t("ui.tradeRetrieveHint", [], "Choose an empty bag slot to retrieve the item.")
          : t("ui.tradeDepositHint", [], "Choose a bag item, then an empty offer slot.")}</div>
        <div style={style.pickerCells}>
          {selectableBag.map(item => <button type="button" key={"bag-" + item.slot + "-" + item.uniqueId} disabled={!itemEditing || !onDepositItem}
            aria-pressed={selection?.source === "bag" && sameSocialItem(selection.item, item)}
            title={item.name + " ×" + item.count + " (" + (item.slot + 1) + ")"} style={style.pickerButton}
            onClick={() => setItemSelection({ source: "bag", item, key: itemSourceKey! })}>
            {item.icon !== undefined ? <img style={style.pickerIcon} src={originalItemIconPath(item.icon)} alt="" draggable={false} /> : null}
            {item.name} ×{item.count}
          </button>)}
          {emptySlots.map(slot => <button type="button" key={"empty-bag-" + slot} style={style.pickerButton}
            disabled={!itemEditing || selection?.source !== "offer" || !onRetrieveItem}
            onClick={() => { if (itemEditing && selection?.source === "offer" && onRetrieveItem) {
              onRetrieveItem(selection.item.slot, slot, selection.item.uniqueId); setItemSelection(null);
            } }}>
            {t("ui.emptyBagSlot", [slot + 1], "Empty bag " + (slot + 1))}
          </button>)}
        </div>
      </section> : null}
    </> : null}
    {requested ? <R16SocialModal label={t("ui.trade", [], "Trade")} rect={invitation.panel}
      background="/original-ui/Prguse/360.png" onCancel={() => (onCancel ?? onClose)()} onConfirm={() => onAccept?.()}>
      <div style={style.invitationText}>{t("ui.tradeIncoming", [partner], partner + " wants to trade with you.")}</div>
      <R16SocialButton frame={206} rect={invitation.accept} data-r16-trade-accept="true" disabled={!onAccept}
        label={t("ui.tradeAccept", [], "Accept")} onClick={() => onAccept?.()} />
      <R16SocialButton frame={210} rect={invitation.decline} data-r16-trade-decline="true" disabled={!onCancel}
        label={t("ui.tradeDecline", [], "Decline")} onClick={() => onCancel?.()} />
    </R16SocialModal> : goldDraft !== null && !goldLocked ? <R16SocialModal label={t("ui.tradeSetGold", [], "Set gold")}
      rect={layout.amount} background="/original-ui/Prguse/238.png" onCancel={closeGold} onConfirm={submitGold}>
      <div style={style.amountTitle}>{t("ui.tradeSetGold", [], "Trade Amount:")}</div>
      <img src={originalItemIconPath(116)} alt="" draggable={false} style={style.amountIcon} />
      <input data-r16-trade-gold-input="true" data-r16-autofocus="true" value={goldDraft} inputMode="numeric"
        aria-label={t("ui.tradeSetGold", [], "Set gold")} autoComplete="off" spellCheck={false} maxLength={12}
        onChange={event => setGoldDraft(event.target.value.replace(/[^0-9]/g, ""))} onFocus={event => event.currentTarget.select()}
        style={{ ...style.amountInput, ...layout.amountField, borderColor: amount === null ? "#f00" : amount === availableGold ? "#ffa500" : "#00c000" }} />
      {amount !== null ? <R16SocialButton frame={200} rect={layout.amountConfirm} data-r16-trade-gold-submit="true"
        label={t("ui.confirm", [], "Confirm")} onClick={submitGold} /> : null}
      <R16SocialButton frame={203} rect={layout.amountCancel} label={t("ui.cancel", [], "Cancel")} onClick={closeGold} />
      <R16SocialButton library="Prguse2" frame={360} rect={layout.amountClose} label={t("ui.close", [], "Close")} onClick={closeGold} />
    </R16SocialModal> : null}
  </section>;
}

function TradeGrid({ items, t, onItemClick, disabled, selectedSlot, onReadItemTooltip }: {
  items: Array<TradeItemSlot | null>; t: TranslateFn; onItemClick?: (slot: number) => void; disabled?: boolean; selectedSlot?: number;
  onReadItemTooltip?: (item: TradeItemSlot) => CrystalTooltipDocument | null;
}) {
  const slots = tradeGridSlots(items);
  const [activeSlot, setActiveSlot] = useState<number | null>(null);
  return <div role="list" aria-label={t("ui.tradeItems", [], "Items")} style={{ position: "absolute", inset: 0, pointerEvents: "none" }}>
    {Array.from({ length: TRADE_SLOT_COUNT }, (_, index) => {
      const item = slots[index];
      const document = activeSlot === index && item ? onReadItemTooltip?.(item) : null;
      const info = item?.tooltipSource?.info as Readonly<Record<string, unknown>> | undefined;
      const cellRect = r16TradeCell(index)!;
      const content = <>{item && typeof item.icon === "number" ? <img style={style.cellIcon} src={originalItemIconPath(item.icon)} alt="" draggable={false} />
        : item ? <span style={style.cellText}>{(item.name ?? "?").slice(0, 3)}</span> : null}
        {item && item.count && item.count > 1 ? <span style={style.cellCount}>{item.count}</span> : null}
        {document ? <OriginalCrystalItemTooltip document={document} align={index >= 6 ? "left" : "right"} />
          : item?.tooltipSource ? <OriginalItemTooltip t={t} name={item.name ?? t("ui.unknownItem", [], "Unknown item")} quantity={item.count}
            description={typeof info?.tooltip === "string" && info.tooltip.length <= 8192 ? info.tooltip : undefined}
            weight={typeof info?.weight === "number" && Number.isInteger(info.weight) && info.weight >= 0 && info.weight <= 255 ? info.weight : undefined}
            align={index >= 6 ? "left" : "right"} /> : null}</>;
      const handlers = { onMouseEnter: () => setActiveSlot(index), onMouseLeave: () => setActiveSlot(null),
        onFocus: () => setActiveSlot(index), onBlur: () => setActiveSlot(null) };
      if (onItemClick) return <button type="button" className="trade-item-card" key={"own-" + index} data-trade-slot={index}
        title={item?.name} aria-label={item?.name ?? t("ui.emptyTradeSlot", [index + 1], "Empty offer " + (index + 1))}
        aria-pressed={selectedSlot === index} disabled={disabled} {...handlers}
        style={{ ...style.cell, ...cellRect, ...(selectedSlot === index ? { outline: "1px solid #f4dcaf" } : null) }}
        onClick={() => onItemClick(index)}>{content}</button>;
      return <div key={item ? "item-" + item.id : "empty-" + index} role="listitem" className="trade-item-card"
        tabIndex={item?.tooltipSource ? 0 : undefined} data-trade-slot={index} title={item?.name}
        style={{ ...style.cell, ...cellRect }} {...handlers}>{content}</div>;
    })}
  </div>;
}

function tradeGridSlots(items: Array<TradeItemSlot | null>): Array<TradeItemSlot | null> {
  const slots: Array<TradeItemSlot | null> = Array(TRADE_SLOT_COUNT).fill(null);
  for (let index = 0; index < items.length; index++) {
    const item = items[index];
    if (!item) continue;
    const slot = item.slot ?? index;
    if (validSocialItemSlot(slot, TRADE_SLOT_COUNT) && slots[slot] === null) slots[slot] = item;
  }
  return slots;
}
function formatNumber(value: number) { return Math.max(0, Math.trunc(value)).toLocaleString("en-US"); }

const style: Record<string, CSSProperties> = {
  window: { position: "absolute", color: "#fff", fontSize: 32 / 3, fontFamily: "Arial, Helvetica, sans-serif",
    textShadow: "1px 1px #000", pointerEvents: "auto", touchAction: "none" },
  name: { position: "absolute", overflow: "hidden", whiteSpace: "nowrap", pointerEvents: "none" },
  gold: { position: "absolute", padding: 0, border: 0, background: "transparent", color: "#fff", textAlign: "right", font: "inherit", textShadow: "inherit" },
  status: { position: "absolute", left: 8, top: 105, width: 190, height: 12, fontSize: 9, color: "#e8d69d", whiteSpace: "nowrap", textOverflow: "ellipsis", overflow: "hidden", pointerEvents: "none" },
  cell: { position: "absolute", display: "flex", alignItems: "center", justifyContent: "center", padding: 0, border: 0,
    background: "transparent", color: "#fff", pointerEvents: "auto" },
  cellIcon: { maxWidth: 36, maxHeight: 32, width: "auto", height: "auto", imageRendering: "pixelated", pointerEvents: "none" },
  cellText: { fontSize: 9, color: "#cbb38a" },
  cellCount: { position: "absolute", right: 1, bottom: 0, fontSize: 9, lineHeight: "10px", color: "#fff", textShadow: "1px 1px #000", pointerEvents: "none" },
  itemPicker: { position: "absolute", ...layout.bagPicker, zIndex: 4, overflowY: "auto", boxSizing: "border-box", padding: 8,
    color: "#f0eee8", border: "1px solid #9c8d6f", background: "rgba(20,13,7,0.96)", pointerEvents: "auto", fontSize: 10 },
  pickerHint: { lineHeight: "14px" },
  pickerCells: { display: "flex", flexWrap: "wrap", gap: 4, marginTop: 4 },
  pickerButton: { display: "flex", alignItems: "center", gap: 3, minHeight: 28, maxWidth: 140, border: "1px solid #9c8d6f",
    background: "#24180c", color: "#f0eee8", fontSize: 10, fontFamily: "inherit", cursor: "pointer" },
  pickerIcon: { width: 20, height: 20, imageRendering: "pixelated" },
  invitationText: { position: "absolute", left: 35, top: 35, width: 390, height: 110, overflow: "auto", overflowWrap: "anywhere", whiteSpace: "pre-wrap" },
  amountTitle: { position: "absolute", left: 19, top: 8, width: 158, height: 14, overflow: "hidden", whiteSpace: "nowrap" },
  amountIcon: { position: "absolute", left: 15, top: 34, width: 38, height: 34, objectFit: "contain", imageRendering: "pixelated" },
  amountInput: { position: "absolute", boxSizing: "border-box", padding: "0 2px", border: "1px solid", background: "#000", color: "#fff", font: "inherit" },
};
