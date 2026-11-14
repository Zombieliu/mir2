import { EventEmitter } from "node:events";
import { ObservationReducer } from "../src/observation.mjs";

export const OWNER = "OwnerGift";
export const UID = 9007199254740997n;
export const packet = (packet, payload = {}) => ({ type: "packet", packet, payload });
export const tradeState = (changes = {}) => ({ settlementNonce: "a".repeat(32), partner: OWNER, offeredItems: [], offeredSlots: {}, offeredUniqueIds: {}, offeredGold: 0, heldGold: 0,
  offeredCurrency: "gold", accepted: false, locked: false, escrowPrepared: false, completed: false, ...changes });
export const world = (changes = {}) => ({ mapFileName: "0", playerObjectId: 1000, selfZoneObjectId: 50001, playerHp: 18, playerMaxHp: 18, gold: 10,
  entities: [{ kind: "selfPlayer", objectId: 1000, name: "AloGift", x: 288, y: 616, hp: 18, maxHp: 18 }, { kind: "player", objectId: 50002, name: OWNER, x: 289, y: 616 }],
  inventoryItems: [{ uniqueId: UID, name: "HPDrug", key: "hp-drug", quantity: 1, container: "bag1", slot: 0 }], beltItems: [], equipmentItems: [], questLog: [], groundDrops: [],
  stage5Systems: { trade: null }, ...changes });

export function giftFixture() {
  const reducer = new ObservationReducer(), data = world(), calls = [];
  const client = new EventEmitter();
  client.worldEpoch = "epoch:gift"; client.activeCharacterIndex = 2;
  client.snapshot = () => reducer.snapshot();
  const apply = event => { reducer.apply(event); client.emit("frame", { event }); };
  const snapshot = () => apply({ type: "worldSnapshot", payload: data });
  const enter = () => { apply(packet("StartGame", { result: 4 })); snapshot(); };
  client.acceptOwnerGiftInvite = async (ownerName, inviteId) => {
    calls.push({ type: "tradeReply", acceptInvite: true, ownerName, inviteId });
    if (client.acceptHook) return client.acceptHook();
    apply(packet("TradeAccept", { partnerName: ownerName }));
    data.stage5Systems.trade = tradeState({ settlementNonce: client.nextTradeId ?? "a".repeat(32) }); snapshot();
    return { accepted: true, tradeId: data.stage5Systems.trade.settlementNonce };
  };
  client.confirmReceiveOnlyGift = async request => {
    calls.push({ type: "tradeConfirm", locked: true, ...request });
    if (client.confirmHook) return client.confirmHook();
    Object.assign(data.stage5Systems.trade, { locked: true, escrowPrepared: true }); snapshot();
    return { sent: true, confirmed: false };
  };
  client.cancelGiftTrade = async () => {
    calls.push({ type: "tradeCancel" });
    if (client.cancelHook) return client.cancelHook();
    apply(packet("TradeCancel", { unlock: false })); data.stage5Systems.trade = null; snapshot();
    return { sent: true, confirmed: false };
  };
  enter();
  return { client, reducer, data, calls, apply, snapshot, enter,
    invite: (name = OWNER) => apply(packet("TradeRequest", { partnerName: name })),
    offerGold: amount => { apply(packet("TradeGold", { partnerGold: amount })); snapshot(); },
    offerItems: items => { apply(packet("TradeItem", { tradeItems: items })); snapshot(); },
    deliver: ({ gold = 0, items = [], confirm = true, full = true } = {}) => {
      if (gold > 0) { apply(packet("GainedGold", { gold })); data.gold += gold; }
      for (const item of items) {
        apply(packet("GainedItem", { item }));
        data.inventoryItems.push({ uniqueId: item.unique_id ?? item.uniqueId, quantity: item.count, key: `crystal-item-${item.item_index}`, name: "Gift potion", container: "bag1", slot: data.inventoryItems.length });
      }
      if (confirm) { apply(packet("TradeConfirm")); data.stage5Systems.trade = null; }
      if (full) snapshot();
    },
  };
}
